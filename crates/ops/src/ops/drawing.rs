//! Drawings: create a Drawing tab, read its sheets (views, their model
//! edges in sheet coordinates, annotations), place views and dimensions.
//! Placing runs the drawing editor's own tools (`noshape_scene::drawing_tools`)
//! on the clicks you give, in sheet millimeters, on the sheet built as the
//! editor builds it (`crate::drawing`).

use super::{doc_field, s};
use crate::drawing::{self as dr, SheetModel};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::{Ctx, OpenDoc, doc, refs};
use noshape_model::document::{TabContent, TabId};
use noshape_model::drawing::create::DrawingSource;
use noshape_model::drawing::views::{EdgeGeom, SourceKey};
use noshape_model::drawing::{Drawing, ViewId, ViewKind, ViewOrientation};
use noshape_scene::drawing_tools::{DimTool, Pick, Tool, ToolValues, ViewOptions};
use serde_json::{Value, json};

pub(super) const ORIENTATIONS: [(&str, ViewOrientation); 7] = [
    ("front", ViewOrientation::Front),
    ("back", ViewOrientation::Back),
    ("top", ViewOrientation::Top),
    ("bottom", ViewOrientation::Bottom),
    ("left", ViewOrientation::Left),
    ("right", ViewOrientation::Right),
    ("isometric", ViewOrientation::Isometric),
];

/// Dimension tools: (name, tool, what the clicks pick).
const DIMENSIONS: [(&str, DimTool, &str); 12] = [
    ("smart", DimTool::Linear, "the Dimension tool (d): click one edge (a line gives its length, a circle its diameter, an arc its radius) or two entities (points, lines, circles: distance, angle between lines, wall thickness of concentric circles)"),
    ("point_to_point", DimTool::Point2, "2 point linear: two vertices / circle centers"),
    ("point_to_line", DimTool::PointLine, "a point and a line"),
    ("line_to_line", DimTool::LineLine, "two parallel lines"),
    ("radius", DimTool::Radius, "one circle or arc"),
    ("diameter", DimTool::Diameter, "one circle or arc"),
    ("angle", DimTool::Angle, "two lines"),
    ("angle_3_point", DimTool::Angle3, "vertex, then a point on each leg"),
    ("arc_length", DimTool::ArcLength, "one arc"),
    ("chamfer", DimTool::Chamfer, "the chamfer edge, then the edge it meets (Drawing properties decide a callout or two linear dimensions)"),
    ("max_min", DimTool::MaxMin, "a circle (where on it picks maximum or minimum), then a point or another circle"),
    ("foreshorten", DimTool::Foreshorten, "a hole or cylinder axis seen side-on, then the edge or point the diameter reaches"),
];

pub(super) fn point() -> Value {
    json!({ "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 })
}

pub(super) fn drawing_field() -> crate::registry::Field {
    req("drawing", s(), "Drawing tab, by name (\"Part 1 Drawing 1\") or id.")
}

pub(super) fn sheet_field() -> crate::registry::Field {
    opt("sheet", json!({ "type": ["integer", "string"] }), "Sheet: number (1 = first) or name (\"Sheet1\"). Default: the sheet the drawing shows.")
}

pub fn ops() -> Vec<Op> {
    vec![
        Op {
            group: "drawing",
            verb: "templates",
            summary: "List the drawing templates",
            description: "Create Drawing's standard templates: file, sheet setup, standard (ANSI / ISO) and units.",
            fields: vec![],
            mutates: false,
            destructive: false,
            run: templates,
        },
        Op {
            group: "drawing",
            verb: "create",
            summary: "Create a Drawing tab of a part, Part Studio or assembly",
            description: "Create Drawing: a Drawing tab with the template's sheet, placed after the source tab. With `suggested_views` the source's suggested views (Front, Top, Right projections and an isometric, scaled to fit) are laid out, as the dialog's checkbox does. Returns the tab and its views.",
            fields: vec![
                doc_field(),
                opt("source", s(), "Part Studio or assembly tab (name or id). Omit for an empty drawing."),
                opt("part", s(), "Only this part (`body:…` id from query_parts, or its name)."),
                opt("sketch", s(), "Only this sketch of the source Part Studio (feature id or name): Create Drawing of Sketch N."),
                opt("configuration", json!({ "type": "object" }), "Configuration of the source: {input id: value}."),
                opt("template", s(), "Existing templates: file or setup name (`drawing_templates`), e.g. \"ANSI_A_INCH.dwt\", \"ISO A3\". Default ANSI_A_INCH.dwt, the dialog's preselected row."),
                opt("custom_template", json!({ "type": "object" }), CUSTOM_TEMPLATE_DOC),
                opt("name", s(), "Tab name (default \"<part> Drawing N\" / \"<sketch> Drawing N\" / \"Drawing N\")."),
                opt("suggested_views", json!({ "type": "boolean", "default": false }), "Suggested views (true) or No views (false, the dialog's default; place views with drawing_insert_view)."),
            ],
            mutates: true,
            destructive: false,
            run: create,
        },
        Op {
            group: "drawing",
            verb: "get",
            summary: "A drawing sheet's views (with their edges in sheet coordinates), annotations and tables",
            description: "Coordinates are sheet millimeters from the sheet's lower-left corner. Each view lists its kind, parent, scale, sheet bounds and the model edges as drawn (lines with end points, circles/arcs with center and radius, all in sheet mm) so you can click them with drawing_dimension.",
            fields: vec![doc_field(), drawing_field(), sheet_field(), opt("edges", json!({ "type": "boolean", "default": true }), "Include each view's edges.")],
            mutates: false,
            destructive: false,
            run: get,
        },
        Op {
            group: "drawing",
            verb: "insert_view",
            summary: "Insert a base view of a part or assembly",
            description: "Insert view: the view centered at `position` (sheet mm) in the given orientation, at `scale` (paper per model, e.g. 0.5 for 1:2; default the sheet scale). One edit (\"Insert view\").",
            fields: vec![
                doc_field(),
                drawing_field(),
                sheet_field(),
                req("source", s(), "Part Studio or assembly tab."),
                opt("part", s(), "Only this part (`body:…` id or name)."),
                opt("sketch", s(), "Insert a sketch of the source Part Studio (a sketch view, facing its plane)."),
                opt("flat", json!({ "type": "boolean", "default": false }), "A sheet metal part's flat pattern."),
                opt("orientation", json!({ "type": "string", "enum": ORIENTATIONS.iter().map(|o| o.0).collect::<Vec<_>>(), "default": "front" }), "View orientation."),
                opt("named_view", s(), "View orientation › a named view of the source Part Studio (instead of `orientation`)."),
                req("position", point(), "View center on the sheet (mm)."),
                opt("scale", json!({ "type": "number", "exclusiveMinimum": 0 }), "View scale (paper / model)."),
                opt("simplification", json!({ "type": "string", "enum": ["none", "auto", "absolute", "ratio_to_studio", "ratio_to_part"], "default": "auto" }), "View simplification: None, Automatic, Absolute size, Ratio to Part Studio / assembly size, Ratio to part size."),
                opt("simplification_value", s(), "Its value: a length (\"1 mm\", default 1 mm) or a percentage (\"1 %\", default 1 %)."),
                opt("sketch_appearances", json!({ "type": "boolean", "default": false }), "Show sketch appearances."),
                opt("model_annotations", json!({ "type": "boolean", "default": true }), "Show annotations from model."),
                opt("explode_position", s(), "Assemblies: Explode/Position, an exploded view or named position name (default \"Default\")."),
                opt("display_state", s(), "Assemblies: Display state name (default Show all)."),
            ],
            mutates: true,
            destructive: false,
            run: insert_view,
        },
        Op {
            group: "drawing",
            verb: "projected_view",
            summary: "Project a view from a parent view",
            description: "Projected view: the parent view projected toward `position` (sheet mm): above/below/left/right of the parent gives the orthographic projection (aligned with the parent), diagonal gives an isometric. One edit (\"Projected view\").",
            fields: vec![doc_field(), drawing_field(), sheet_field(), req("parent", json!({ "type": "integer" }), "Parent view id (drawing_get)."), req("position", point(), "Where to place it (mm).")],
            mutates: true,
            destructive: false,
            run: projected_view,
        },
        Op {
            group: "drawing",
            verb: "dimension",
            summary: "Dimension view geometry (the drawing Dimension tools)",
            description: Box::leak(
                format!(
                    "Clicks the dimension tool on the view geometry at `picks` (sheet mm; click on an edge, or on a vertex / circle center to pick the point; drawing_get gives edge coordinates), then places the dimension at `place`. Linear dimensions are horizontal, vertical or aligned by where `place` is, as with the mouse. The dimension is associative to the model. Kinds:\n{}",
                    DIMENSIONS.iter().map(|(n, _, h)| format!("- {n}: {h}")).collect::<Vec<_>>().join("\n")
                )
                .into_boxed_str(),
            ),
            fields: vec![
                doc_field(),
                drawing_field(),
                sheet_field(),
                opt("kind", json!({ "type": "string", "enum": DIMENSIONS.iter().map(|d| d.0).collect::<Vec<_>>(), "default": "smart" }), "Dimension tool."),
                req("picks", json!({ "type": "array", "items": point(), "minItems": 1, "maxItems": 3 }), "The picking clicks (sheet mm)."),
                req("place", point(), "Where the dimension text goes (sheet mm)."),
                opt("mid_quad", json!({ "type": "boolean", "default": false }), "Shift+Q (Quad & mid snap point): the clicks also snap to edge midpoints and circle quadrant points, for this dimension."),
            ],
            mutates: true,
            destructive: false,
            run: dimension,
        },
        Op {
            group: "drawing",
            verb: "update",
            summary: "Update an out-of-date drawing from the current model",
            description: "After the model changes, a drawing's views keep the geometry they were made from and the drawing is out of date (`out_of_date` in drawing_get; the app's yellow Update button). Update remakes every view from the current document. One edit (\"Update\") when out of date; otherwise nothing changes.",
            fields: vec![doc_field(), drawing_field(), sheet_field()],
            mutates: true,
            destructive: false,
            run: update,
        },
        Op {
            group: "drawing",
            verb: "export",
            summary: "Export a drawing (PDF, DWG, DXF, DWT, SVG, PNG, JPEG, TIFF, inspection lists)",
            description: "The Export Drawing modal with every option. PDF holds every sheet, DWT the first two, DXF / DWG a layout per sheet of `sheets`; SVG and the raster formats (300 dpi) the active sheet. `version` from the format's list (PDF 1.4–1.7, default 1.5; DWG / DXF Release 11-12 … 2018, default 2000 / Release 14; DWT 2013 / 2018). Inspection list / Drawing definition list are the CSV lists.",
            fields: vec![
                doc_field(),
                drawing_field(),
                req("format", json!({ "type": "string", "enum": crate::drawing_export::DrawingFormat::ALL.iter().map(|f| f.label()).collect::<Vec<_>>() }), "Format."),
                opt("file", s(), "Output path (required unless `options` is \"store\"; then only its name is used, default \"<drawing>.<ext>\")."),
                opt("options", json!({ "type": "string", "enum": ["download", "download_and_store", "store"], "default": "download" }), "Options: write `file`, and / or keep the file as a file tab right after the drawing (\"Store <file>\")."),
                opt("version", s(), "The format's Version (e.g. \"1.7\", \"2018\", \"Release 14\")."),
                opt("sheets", s(), "DWG / DXF: \"all\" (default), \"current\" or a range \"1-3, 5\" (PDF: every sheet; DWT: the first two; SVG and rasters: the active sheet)."),
                opt("overridden_dimensions", json!({ "type": "string", "enum": ["show_underlines", "hide_underlines"], "default": "show_underlines" }), "Overridden dimensions."),
                opt("text", json!({ "type": "string", "enum": ["normal", "selectable", "export_as_text", "explode_notes", "explode_all"] }), "PDF Text (normal / selectable) or DWG / DXF / DWT Text (export_as_text / explode_notes / explode_all)."),
                opt("color", json!({ "type": "string", "enum": ["color", "black_and_white", "grayscale"] }), "PDF Color (default color; only PDF has it)."),
                opt("units", json!({ "type": "string", "enum": crate::drawing_export::UNITS.iter().map(|u| u.0).collect::<Vec<_>>() }), "DWG / DXF Units (default \"Use drawing units\")."),
                opt("splines_as_polylines", json!({ "type": "boolean" }), "Export splines as polylines."),
                opt("z_zero", json!({ "type": "boolean", "default": true }), "DWG / DXF / DWT: Set z-height to zero and normals to positive."),
                opt("hide_inspection_items", json!({ "type": "boolean" }), "Hide inspection items."),
                opt("exclude_off_sheet", json!({ "type": "boolean" }), "DWG / DXF / DWT: Exclude off-sheet content."),
            ],
            mutates: true,
            destructive: false,
            run: export,
        },
    ]
}

// ------------------------------------------------------------ helpers

pub(super) fn point_value(v: &Value, param: &str) -> Result<[f64; 2], OpError> {
    let a = v.as_array().filter(|a| a.len() == 2).ok_or_else(|| OpError::param(param, "a point is [x, y] in sheet mm"))?;
    let n = |i: usize| a[i].as_f64().ok_or_else(|| OpError::param(param, "numbers (sheet mm)"));
    Ok([n(0)?, n(1)?])
}

pub(super) fn drawing_tab(od: &OpenDoc, a: &Args) -> Result<TabId, OpError> {
    doc::tab_of_kind(od.doc(), Some(a.str("drawing")?), "Drawing", |c| matches!(c, TabContent::Drawing(_))).map_err(|e| e.with_param("drawing"))
}

pub(super) fn sheet_index(d: &Drawing, a: &Args) -> Result<usize, OpError> {
    match a.get("sheet") {
        None => Ok(d.active_sheet.min(d.sheets.len().saturating_sub(1))),
        Some(Value::Number(n)) => n.as_u64().filter(|n| (1..=d.sheets.len() as u64).contains(n)).map(|n| n as usize - 1).ok_or_else(|| OpError::param("sheet", &format!("sheet numbers are 1 to {}", d.sheets.len()))),
        Some(Value::String(s)) => d.sheets.iter().position(|x| x.name == *s).ok_or_else(|| OpError::param("sheet", &format!("no sheet `{s}`; sheets: {}", d.sheets.iter().map(|x| x.name.clone()).collect::<Vec<_>>().join(", ")))),
        Some(_) => Err(OpError::param("sheet", "a number or a name")),
    }
}

pub(super) fn source_tab(od: &OpenDoc, a: &Args, name: &str) -> Result<TabId, OpError> {
    doc::tab_of_kind(od.doc(), Some(a.str(name)?), "Part Studio or Assembly", |c| matches!(c, TabContent::PartStudio(_) | TabContent::Assembly(_))).map_err(|e| e.with_param(name))
}

/// The `part` argument: a `body:…` id, or the name a part of the Part
/// Studio `source` shows.
pub(super) fn part_arg(od: &mut OpenDoc, source: TabId, a: &Args) -> Result<Option<noshape_model::BodyId>, OpError> {
    match a.opt_str("part")? {
        None => Ok(None),
        Some(p) => match refs::parse_str(p) {
            Some(noshape_model::reference::Reference::Body(b)) => Ok(Some(b)),
            _ if matches!(od.doc().tab(source).map(|t| &t.content), Some(TabContent::PartStudio(_))) => {
                let r = od.regenerate(source, None)?;
                Ok(Some(super::query::find_part(&r, p)?.id.clone()))
            }
            _ => Err(OpError::param("part", "a part id (`body:…` from query_parts) or name of a Part Studio's part")),
        },
    }
}

/// Commit an edited drawing as the editor does ("<tab> :: <description>").
pub(super) fn commit(od: &mut OpenDoc, tab: TabId, sheet: usize, mut d: Drawing, description: &str) -> Result<(), OpError> {
    dr::finish_edit(&mut d, sheet);
    dr::stamp_sources(&od.session, &mut d);
    od.session.edit_tab(tab, description, |c| {
        if let TabContent::Drawing(x) = c {
            *x = d;
        }
    })?;
    Ok(())
}

fn r3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn pt(p: [f64; 2]) -> Value {
    json!([r3(p[0]), r3(p[1])])
}

pub(super) fn kind_name(k: &ViewKind) -> &'static str {
    match k {
        ViewKind::Base { .. } => "base",
        ViewKind::Projected { .. } => "projected",
        ViewKind::Section { .. } => "section",
        ViewKind::AlignedSection { .. } => "aligned_section",
        ViewKind::Detail { .. } => "detail",
        ViewKind::Auxiliary { .. } => "auxiliary",
    }
}

/// The annotation kind's name (`LinearDimension`, `Note`...).
pub(super) fn variant_name<T: std::fmt::Debug>(v: &T) -> String {
    let s = format!("{v:?}");
    s.split([' ', '{', '(']).next().unwrap_or("").to_string()
}

/// A sheet as JSON: views (with their drawn model edges), annotations, tables.
fn sheet_json(m: &SheetModel, edges: bool) -> Value {
    let Some(sheet) = m.drawing.sheets.get(m.sheet) else { return Value::Null };
    let (w, h) = sheet.size.size_mm();
    let views: Vec<Value> = sheet
        .views
        .iter()
        .map(|v| {
            let mut j = json!({
                "id": v.id.0,
                "kind": kind_name(&v.kind),
                "position": pt(v.position),
                "scale": noshape_model::drawing::views::view_scale(sheet, v.id),
            });
            if let Some(p) = sheet.parent_of(v.id) {
                j["parent"] = json!(p.0);
            }
            // Show annotations from model (child views inherit it).
            if !v.model_annotations {
                j["model_annotations"] = json!(false);
            }
            // A detail view's circle in its parent (sheet mm) and its label
            // on a leader (drawing_edit_detail).
            if let ViewKind::Detail { parent, center, radius, .. } = &v.kind
                && let Some(ppl) = m.built.as_ref().and_then(|b| b.placement(*parent))
            {
                let c = ppl.to_sheet(*center);
                let mut d = json!({ "center": pt(c), "radius": ppl.len_to_sheet(*radius) });
                if let Some(o) = v.detail_label_at {
                    d["label_at"] = pt([c[0] + o[0], c[1] + o[1]]);
                }
                j["detail"] = d;
            }
            // Section hatch regions and the ones restyled with Edit hatch
            // section... (drawing_edit_hatch).
            if let Some(regions) = m.geoms.get(&v.id).filter(|g| !g.section_segments.is_empty()).and_then(|g| noshape_model::drawing::views::section_regions(&g.section_segments)) {
                j["hatch_regions"] = json!(regions.len());
            }
            if !v.section_hatches.is_empty()
                && let Some(pl) = m.built.as_ref().and_then(|b| b.placement(v.id))
            {
                let name = |h: &noshape_model::drawing::SectionHatch| h.pattern.and_then(|p| noshape_model::drawing::hatch_pat::pattern(h.standard, p)).map(|p| p.name).unwrap_or("None");
                j["section_hatches"] = json!(v.section_hatches.iter().map(|h| json!({ "at": pt(pl.to_sheet(h.at)), "pattern": name(h) })).collect::<Vec<_>>());
            }
            // What the view shows: a base view's orientation, a projected
            // view's side of its parent.
            match &v.kind {
                ViewKind::Base { orientation, .. } => {
                    j["orientation"] = json!(ORIENTATIONS.iter().find(|o| o.1 == *orientation).map(|o| o.0));
                    // The Insert view panel's choices, when not the defaults.
                    if let Some(n) = &v.named_view {
                        j["named_view"] = json!(n);
                    }
                    if let Some(k) = v.sketch {
                        j["sketch"] = json!(k.to_string());
                    }
                    if v.flat {
                        j["flat"] = json!(true);
                    }
                    if v.sketch_appearances {
                        j["sketch_appearances"] = json!(true);
                    }
                    if let Some(s) = v.simplification.filter(|s| *s != noshape_model::drawing::Simplification::Auto) {
                        j["simplification"] = json!(format!("{s:?}"));
                    }
                    if let Some(d) = &v.display_state {
                        j["display_state"] = json!(d);
                    }
                }
                ViewKind::Projected { parent, direction } => {
                    let dir = direction.or_else(|| sheet.view(*parent).map(|p| [v.position[0] - p.position[0], v.position[1] - p.position[1]]));
                    if let Some(d) = dir {
                        let side = if d[0].abs() > 1e-9 && d[1].abs() > 1e-9 && (d[0].abs() / d[1].abs() - 1.0).abs() < 0.5 {
                            "diagonal (isometric)"
                        } else if d[0].abs() >= d[1].abs() {
                            if d[0] > 0.0 { "right of parent" } else { "left of parent" }
                        } else if d[1] > 0.0 {
                            "above parent"
                        } else {
                            "below parent"
                        };
                        j["projection"] = json!(side);
                    }
                    // What it looks along (Onshape names the view by it: "Top", "Right"...).
                    if let Some(o) = noshape_model::drawing::views::standard_orientation(&m.drawing, sheet, v.id) {
                        j["orientation"] = json!(ORIENTATIONS.iter().find(|x| x.1 == o).map(|x| x.0));
                    }
                }
                _ => {}
            }
            if let Some(l) = &v.label {
                j["label"] = json!(l);
            }
            // The view menu's state, when not the defaults.
            if let Some(n) = &v.name {
                j["name"] = json!(n);
            }
            if v.rotation.abs() > 1e-12 {
                j["rotation"] = json!(r3(v.rotation.to_degrees()));
            }
            j["tangent_edges"] = json!(noshape_scene::drawing_tools::view_edit::TangentMode::of(v).label().to_lowercase());
            if v.style != noshape_model::drawing::RenderStyle::HiddenLinesRemoved {
                j["style"] = json!(variant_name(&v.style));
            }
            if v.unaligned {
                j["unaligned"] = json!(true);
            }
            if v.flip {
                j["flipped"] = json!(true);
            }
            if v.show_offset_cut_lines {
                j["offset_cut_lines"] = json!(true);
            }
            if v.pinned.is_some() {
                j["pinned"] = json!(true);
            }
            for (k, n) in [("hidden_parts", v.hidden_parts.len()), ("shown_sketches", v.shown_sketches.len()), ("hidden_edges", v.hidden_edges.len()), ("edge_styles", v.edge_styles.len())] {
                if n > 0 {
                    j[k] = json!(n);
                }
            }
            // Crop, breaks and broken-out sections (view tools).
            if let Some(c) = &v.crop {
                j["crop"] = json!(match c {
                    noshape_model::drawing::Crop::Circle { .. } => "circle",
                    noshape_model::drawing::Crop::Polygon(_) => "polygon",
                    noshape_model::drawing::Crop::Spline(_) => "spline",
                });
            }
            if !v.breaks.is_empty() {
                j["breaks"] = json!(v.breaks.len());
            }
            if !v.broken_out.is_empty() {
                j["broken_out"] = v.broken_out.iter().map(|b| if b.depth_ref.is_some() { json!({ "up_to_point": true }) } else { json!({ "depth_mm": r3(b.depth * 1000.0) }) }).collect();
            }
            let pl = m.built.as_ref().and_then(|b| b.placement(v.id)).copied();
            if let Some(pl) = pl {
                j["bounds"] = json!({ "min": pt(pl.min), "max": pt(pl.max) });
            }
            if edges
                && let (Some(pl), Some(g)) = (pl, m.geoms.get(&v.id))
            {
                // Other drawn curves (circles seen at an angle, splines):
                // their drawn points, at most 24 per edge.
                let curve = |mi: usize| -> Option<Value> {
                    let pts: Vec<[f64; 2]> = g.edges.iter().filter(|d| d.model == Some(mi as u32)).flat_map(|d| d.points.iter().copied()).collect();
                    if pts.len() < 2 {
                        return None;
                    }
                    let step = pts.len().div_ceil(24).max(1);
                    let mut sampled: Vec<Value> = pts.iter().step_by(step).map(|p| pt(pl.to_sheet(*p))).collect();
                    if !(pts.len() - 1).is_multiple_of(step) {
                        sampled.push(pt(pl.to_sheet(pts[pts.len() - 1])));
                    }
                    Some(json!({ "type": "curve", "points": sampled }))
                };
                let list: Vec<Value> = g
                    .model_edges
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| !e.seam)
                    .filter_map(|(mi, e)| match e.geom {
                        EdgeGeom::Line { a, b } => Some(json!({ "type": "line", "a": pt(pl.to_sheet(a)), "b": pt(pl.to_sheet(b)) })),
                        EdgeGeom::Circle { center, radius, face_on: true, .. } => {
                            let closed = noshape_scene::drawing_tools::closed_circle(&e.ends);
                            let mut c = json!({ "type": if closed { "circle" } else { "arc" }, "center": pt(pl.to_sheet(center)), "radius": r3(pl.len_to_sheet(radius)) });
                            if let (false, Some((_, a)), Some((_, b))) = (closed, &e.ends[0], &e.ends[1]) {
                                c["a"] = pt(pl.to_sheet(*a));
                                c["b"] = pt(pl.to_sheet(*b));
                            }
                            Some(c)
                        }
                        _ => curve(mi),
                    })
                    .collect();
                j["edges"] = json!(list);
            }
            j
        })
        .collect();
    let annotations: Vec<Value> = sheet
        .annotations
        .iter()
        .map(|a| {
            let mut j = json!({ "id": a.id, "kind": variant_name(&a.kind), "position": pt(a.position) });
            // The text the sheet shows (dimension values, note text).
            if let Some(b) = &m.built {
                let texts: Vec<&str> = b.geometry.texts.iter().filter(|t| matches!(b.picks.get(t.pick as usize), Some(noshape_model::drawing::build::PickTarget::Annotation(x)) if *x == a.id)).map(|t| t.text.as_str()).collect();
                if !texts.is_empty() {
                    j["text"] = json!(texts.join(" "));
                }
            }
            if let Some(v) = a.kind.view() {
                j["view"] = json!(v.0);
            }
            if let Some(n) = a.inspection {
                j["inspection"] = json!(n);
            }
            // The annotation menus' state, when not the defaults.
            // Follows model geometry (Detach from geometry clears it).
            if !a.refs.is_empty() {
                j["attached"] = json!(true);
            }
            let jogs: Vec<usize> = (0..2).filter(|k| a.jogs[*k].is_some()).collect();
            if !jogs.is_empty() {
                j["jogs"] = json!(jogs);
            }
            if a.unbroken {
                j["unbroken"] = json!(true);
            }
            // Each leader's path on the sheet (landing, nodes, point): a
            // note's in order, a callout's by `all_leaders` (0 = the leader
            // to its anchor, no path while the callout is in space); other
            // annotations' added leaders.
            use noshape_scene::drawing_tools::annotation_edit as ae;
            let ls = ae::all_leaders(a).or_else(|| ae::leaders(a).cloned()).unwrap_or_default();
            let paths = ae::leader_paths(m.built.as_deref(), a);
            // A callout in space without added leaders has none to list.
            let free_callout = paths.is_empty() && ls.len() == 1 && matches!(a.kind, noshape_model::drawing::AnnotationKind::Balloon { .. });
            if !ls.is_empty() && !free_callout {
                j["leaders"] = ls
                    .iter()
                    .enumerate()
                    .map(|(i, l)| {
                        let mut v = json!({ "leader": i, "nodes": l.nodes.len() });
                        if let Some((_, p)) = paths.iter().find(|(k, _)| *k == i) {
                            v["path"] = p.points.iter().map(|q| pt(*q)).collect();
                        }
                        v
                    })
                    .collect();
            }
            if let Some(v) = a.view_group {
                j["view_group"] = json!(v.0);
            }
            if let Some(g) = a.ann_group {
                j["annotation_group"] = json!(g);
            }
            match &a.kind {
                noshape_model::drawing::AnnotationKind::Centermark { angle, .. } if angle.abs() > 1e-12 => j["angle"] = json!(r3(angle.to_degrees())),
                noshape_model::drawing::AnnotationKind::Balloon { callout, .. } if !callout.stacked.is_empty() => j["stacked"] = json!(callout.stacked.len()),
                _ => {}
            }
            if a.target.as_ref().is_some_and(|o| o.hide_dims) {
                j["dimensions_hidden"] = json!(true);
            }
            j
        })
        .collect();
    // Tables with their text as drawn (property fields expanded).
    let si = m.sheet;
    let fields = |k: &str| noshape_model::drawing::build::field_value(&m.drawing, si, &m.info, &m.sources, k);
    let placeholder = if m.drawing.props.hide_placeholder { "" } else { "----" };
    let expand = |s: &str| noshape_model::drawing::format::expand_fields_with(s, &fields, placeholder);
    let tables: Vec<Value> = sheet
        .tables
        .iter()
        .map(|t| {
            let view = t.view();
            let geo = view.and_then(|v| m.geoms.get(&v)).map(|g| g.as_ref());
            let source = view.and_then(|v| noshape_model::drawing::views::view_source(sheet, v)).and_then(|(k, _)| m.sources.get(&k));
            let origin = match &t.kind {
                noshape_model::drawing::TableKind::Hole { origin, origin_ref, .. } => match (origin_ref, geo, source) {
                    (Some(r), Some(g), Some(src)) => noshape_model::drawing::views::resolve_in_view(&src.context, g, r).map(|(p, _)| p).or(Some(*origin)),
                    _ => Some(*origin),
                },
                _ => None,
            };
            let data = noshape_model::drawing::tables::table_data_with(&m.drawing, t, geo, source, origin, &m.info, &expand);
            let mut j = json!({ "id": t.id, "kind": variant_name(&t.kind), "position": pt(t.position), "rows": data.rows });
            if let Some(h) = data.header {
                j["header"] = json!(h);
            }
            if let Some(ti) = data.title {
                j["title"] = json!(ti);
            }
            // A BOM's rows by part (the table shows numbers and descriptions only).
            if let (noshape_model::drawing::TableKind::Bom { structured, bottom_to_top, .. }, Some(s)) = (&t.kind, source) {
                let mut names: Vec<String> = if s.assembly && *structured {
                    s.bom_structured.iter().map(|r| r.name.clone()).collect()
                } else if s.assembly {
                    s.bom.iter().map(|r| r.name.clone()).collect()
                } else {
                    s.parts.iter().map(|p| p.name.clone()).collect()
                };
                if *bottom_to_top {
                    names.reverse();
                }
                j["row_names"] = json!(names);
            }
            // The table menus' state.
            j["corner"] = json!(variant_name(&t.corner));
            j["text_height"] = json!(t.text_height);
            if !t.merges.is_empty() {
                j["merges"] = t.merges.iter().map(|g| json!({ "row": g.row, "column": g.col, "rows": g.rows, "columns": g.cols })).collect();
            }
            if let Some(l) = m.built.as_ref().and_then(|b| b.table_layout(t.id)) {
                j["column_widths"] = l.xs.windows(2).map(|w| json!(r3(w[1] - w[0]))).collect();
                j["bounds"] = json!({ "min": pt(l.min()), "max": pt(l.max()) });
            }
            if let Some(r) = t.rows {
                j["part_rows"] = json!(r);
            }
            match &t.kind {
                noshape_model::drawing::TableKind::Bom { structured, bottom_to_top, .. } => {
                    j["structured"] = json!(structured);
                    j["bottom_to_top"] = json!(bottom_to_top);
                }
                noshape_model::drawing::TableKind::CutList { bottom_to_top, .. } => j["bottom_to_top"] = json!(bottom_to_top),
                noshape_model::drawing::TableKind::Hole { flip_x, flip_y, precision, .. } => {
                    j["flip_x"] = json!(flip_x);
                    j["flip_y"] = json!(flip_y);
                    j["precision"] = json!(precision);
                }
                _ => {}
            }
            j
        })
        .collect();
    let hatches: Vec<Value> = sheet
        .sketch
        .hatches
        .iter()
        .map(|h| {
            let standard = ["ansi", "iso", "general"].get(h.standard).copied().unwrap_or("ansi");
            let mut j = json!({ "id": h.id, "standard": standard, "pattern": noshape_model::drawing::hatch_pat::pattern(h.standard, h.pattern).map(|p| p.name), "scale": h.scale });
            if let Some(v) = h.view {
                j["view"] = json!(v.0);
            }
            if let Some(c) = h.color {
                j["color"] = json!(c);
            }
            j
        })
        .collect();
    let images: Vec<Value> = sheet.images.iter().map(|i| json!({ "id": i.id, "name": i.name, "position": pt(i.pos), "size": pt(i.size) })).collect();
    let sketch_entities = sheet.sketch.sketch.entities().count();
    // Title block field cells: the stored text and the lines shown.
    let title_fields: Vec<Value> = m
        .built
        .as_ref()
        .map(|b| {
            b.title_cells
                .iter()
                .filter_map(|&(pick, r)| {
                    let Some(noshape_model::drawing::build::PickTarget::TitleField(key)) = b.picks.get(pick as usize) else { return None };
                    let shown: Vec<&str> = b.geometry.texts.iter().filter(|t| t.pick == pick).map(|t| t.text.as_str()).collect();
                    Some(json!({ "field": key, "value": sheet.title_field(key).unwrap_or(""), "shown": shown, "cell": [r3(r[0]), r3(r[1]), r3(r[2]), r3(r[3])] }))
                })
                .collect()
        })
        .unwrap_or_default();
    json!({ "sheet": sheet.name, "size_mm": [r3(w), r3(h)], "views": views, "annotations": annotations, "tables": tables, "hatches": hatches, "images": images, "sketch_entities": sketch_entities, "title_fields": title_fields })
}

pub(super) fn reply(od: &mut OpenDoc, tab: TabId, sheet: usize, edges: bool) -> Result<Value, OpError> {
    let d = dr::drawing_of(od.doc(), tab).cloned().ok_or_else(|| OpError::not_found("the drawing tab is gone"))?;
    let m = SheetModel::build(&od.session, tab, d.clone(), sheet, &[]);
    let mut v = sheet_json(&m, edges);
    v["drawing"] = json!(od.doc().tab(tab).map(|t| t.name.clone()));
    v["tab"] = json!(tab.to_string());
    v["sheets"] = json!(d.sheets.iter().map(|s| s.name.clone()).collect::<Vec<_>>());
    v["out_of_date"] = json!(m.out_of_date);
    Ok(v)
}

// ------------------------------------------------------------ handlers

fn templates(_: &mut Ctx, _: &Args) -> Result<OpOutput, OpError> {
    Ok(json!({ "templates": dr::template_rows().iter().map(|r| json!({ "file": r.file, "name": r.template.name, "standard": r.standard, "units": if r.template.inches { "inch" } else { "millimeter" } })).collect::<Vec<_>>() }).into())
}

fn create(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let source = match a.opt_str("source")? {
        Some(_) => {
            let tab = source_tab(od, a, "source")?;
            let sketch = match a.opt_str("sketch")? {
                Some(_) => {
                    let ps = doc::studio(od.doc(), tab).map_err(|e| e.with_param("source"))?;
                    let id = super::feature_arg(ps, a, "sketch")?;
                    if ps.feature(id).is_none_or(|f| f.type_id != "sketch") {
                        return Err(OpError::param("sketch", "not a sketch"));
                    }
                    Some(id)
                }
                None => None,
            };
            if sketch.is_some() && a.get("part").is_some() {
                return Err(OpError::param("sketch", "give `part` or `sketch`, not both"));
            }
            Some(DrawingSource { tab, part: part_arg(od, tab, a)?, config: super::config_arg(a)?, sketch })
        }
        None => {
            if let Some(k) = ["part", "sketch", "configuration"].into_iter().find(|k| a.get(k).is_some()) {
                return Err(OpError::param(k, "give `source` with it"));
            }
            None
        }
    };
    let metric = !matches!(od.doc().settings.units.length, noshape_units::LengthUnit::Inch | noshape_units::LengthUnit::Foot | noshape_units::LengthUnit::Yard);
    let rows = dr::template_rows();
    let template = match (a.opt_str("template")?, a.object("custom_template")?) {
        (Some(_), Some(_)) => return Err(OpError::param("custom_template", "give `template` or `custom_template`, not both")),
        (Some(t), None) => rows.iter().find(|r| r.file.eq_ignore_ascii_case(t)).or_else(|| rows.iter().find(|r| r.template.name.eq_ignore_ascii_case(t))).map(|r| r.template).ok_or_else(|| OpError::param("template", "unknown template (see drawing_templates)"))?,
        (None, Some(c)) => custom_template(c, metric).map_err(|e| e.under("custom_template"))?,
        // Live drawing/01, qa5 D1: the dialog preselects ANSI_A_INCH.dwt whatever the units.
        (None, None) => rows.iter().find(|r| r.file == "ANSI_A_INCH.dwt").map(|r| r.template).expect("standard template"),
    };
    // Onshape: "<part> Drawing 1" for a part or assembly, "<sketch> Drawing 1", else "Drawing 1".
    let base = match &source {
        Some(s) => {
            let label = match (&s.part, s.sketch) {
                (Some(b), _) => od.regenerate(s.tab, None).ok().and_then(|r| r.display.bodies.iter().find(|x| &x.id == b).map(|x| x.name.clone())),
                (None, Some(k)) => doc::studio(od.doc(), s.tab).ok().and_then(|ps| ps.feature(k)).map(|f| f.name.clone()),
                (None, None) => od.doc().tab(s.tab).map(|t| t.name.clone()),
            };
            label.map(|n| format!("{n} Drawing")).unwrap_or_else(|| "Drawing".into())
        }
        None => "Drawing".into(),
    };
    let name = match a.opt_str("name")? {
        Some(n) => n.trim().to_string(),
        None => od.doc().next_tab_name(&base),
    };
    let mut drawing = dr::new_drawing(od.doc(), &template, source.as_ref(), a.bool_or("suggested_views", false)?);
    // The views are made from the document as it is now (see `old_state`).
    drawing.sources_at = od.session.head().ok().map(|m| m.to_string());
    let mut tab = None;
    od.session.edit_document(&format!("Insert tab : {name}"), |d| tab = Some(crate::docedit::insert_drawing_tab(d, &name, drawing, source.as_ref().map(|s| s.tab))))?;
    let tab = tab.expect("inserted");
    Ok(reply(od, tab, 0, false)?.into())
}

const CUSTOM_TEMPLATE_DOC: &str = "The Custom template tab instead of `template`: {\"standard\": \"ANSI\" | \"ISO\" | \"JIS\" (default ISO in metric workspaces, else ANSI), \"language\": English (default), French, German, Spanish, Italian, Japanese, Korean, ChineseSimplified, ChineseTraditional or the list's own name, \"size\": ANSI A (default), A Portrait, B, C, D; ISO / JIS A4 (default), A4 Portrait, A3, A2, A1, A0, \"units\": Millimeters, Centimeter, Meter, Inches, Feet and inches, Foot, Yard, \"decimal_separator\": \"period\" | \"comma\", \"projection\": \"third_angle\" | \"first_angle\", \"border\": bool (default true), \"horizontal_zones\" / \"vertical_zones\": 0–20 even (default per size), \"start_zones\": \"bottom_right\" (default) | \"top_left\", \"title_block\": bool (default true)}. Choosing a standard sets the units, separator and projection like the dialog (ANSI: Inches, period, third angle; ISO: Millimeters, comma, first angle; JIS: Millimeters, comma, third angle).";

/// Create Drawing › Custom template from its JSON options (the dialog's
/// choices applied in its order: standard first, then the rest).
fn custom_template(c: &serde_json::Map<String, Value>, metric: bool) -> Result<noshape_model::drawing::templates::DrawingTemplate, OpError> {
    use noshape_model::drawing::tb_lang::TitleLanguage;
    let text = |k: &str| -> Result<Option<&str>, OpError> {
        match c.get(k) {
            None => Ok(None),
            Some(v) => v.as_str().map(Some).ok_or_else(|| OpError::param(k, "a string")),
        }
    };
    let flag = |k: &str| -> Result<Option<bool>, OpError> {
        match c.get(k) {
            None => Ok(None),
            Some(v) => v.as_bool().map(Some).ok_or_else(|| OpError::param(k, "true or false")),
        }
    };
    let mut ct = dr::CustomTemplate::new(metric);
    if let Some(st) = text("standard")? {
        let st = ["ANSI", "ISO", "JIS"].into_iter().find(|x| x.eq_ignore_ascii_case(st)).ok_or_else(|| OpError::param("standard", "ANSI, ISO or JIS"))?;
        ct.set_standard(st);
    }
    if let Some(l) = text("language")? {
        let norm = |x: &str| x.replace([' ', '_', '(', ')'], "").to_lowercase();
        ct.language = TitleLanguage::ALL.iter().position(|t| norm(&format!("{t:?}")) == norm(l) || t.name() == l).ok_or_else(|| OpError::param("language", "unknown language"))?;
    }
    if let Some(sz) = text("size")? {
        let sizes = dr::custom_sizes(ct.standard);
        let i = sizes.iter().position(|(name, size)| name.eq_ignore_ascii_case(sz) || size.list_name().eq_ignore_ascii_case(sz)).ok_or_else(|| OpError::param("size", &format!("one of {}", sizes.iter().map(|(_, s)| s.list_name()).collect::<Vec<_>>().join(", "))))?;
        ct.set_size(i);
    }
    if let Some(u) = text("units")? {
        ct.units = dr::CUSTOM_UNITS.iter().find(|(_, n)| n.eq_ignore_ascii_case(u)).map(|(d, _)| *d).ok_or_else(|| OpError::param("units", &format!("one of {}", dr::CUSTOM_UNITS.map(|(_, n)| n).join(", "))))?;
    }
    if let Some(d) = text("decimal_separator")? {
        ct.comma = match d {
            "period" => false,
            "comma" => true,
            _ => return Err(OpError::param("decimal_separator", "period or comma")),
        };
    }
    if let Some(p) = text("projection")? {
        ct.first_angle = match p {
            "third_angle" => false,
            "first_angle" => true,
            _ => return Err(OpError::param("projection", "third_angle or first_angle")),
        };
    }
    if let Some(b) = flag("border")? {
        ct.border = b;
    }
    for (i, k) in ["horizontal_zones", "vertical_zones"].into_iter().enumerate() {
        if let Some(v) = c.get(k) {
            ct.zones[i] = v.as_u64().map(|n| n as u32).filter(|n| dr::ZONE_COUNTS.contains(n)).ok_or_else(|| OpError::param(k, "an even number 0–20"))?;
        }
    }
    if let Some(s) = text("start_zones")? {
        ct.top_left = match s {
            "bottom_right" => false,
            "top_left" => true,
            _ => return Err(OpError::param("start_zones", "bottom_right or top_left")),
        };
    }
    if let Some(t) = flag("title_block")? {
        ct.title_block = t;
    }
    Ok(ct.template().expect("size in range"))
}

fn get(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = drawing_tab(od, a)?;
    let d = dr::drawing_of(od.doc(), tab).expect("drawing");
    let sheet = sheet_index(d, a)?;
    Ok(reply(od, tab, sheet, a.bool_or("edges", true)?)?.into())
}

/// Place the view the active view tool previews at `position`.
fn place_view(od: &mut OpenDoc, a: &Args, tool: Tool, opts: ViewOptions, extra: &[SourceKey], description: &str) -> Result<OpOutput, OpError> {
    let tab = drawing_tab(od, a)?;
    let d = dr::drawing_of(od.doc(), tab).cloned().expect("drawing");
    let sheet = sheet_index(&d, a)?;
    let pos = point_value(a.get("position").expect("required"), "position")?;
    let m = SheetModel::build(&od.session, tab, d.clone(), sheet, extra);
    let v = m.pick(&tool, &[], pos).pending_view(&opts).ok_or_else(|| OpError::param("position", "the view can't be placed there (a projected view goes at least 15 mm from its parent)"))?;
    let id = v.id;
    let mut d = d;
    d.sheets[sheet].views.push(v);
    commit(od, tab, sheet, d, description)?;
    let mut out = reply(od, tab, sheet, false)?;
    out["view"] = json!(id.0);
    Ok(out.into())
}

fn insert_view(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let source = source_tab(od, a, "source")?;
    let part = part_arg(od, source, a)?;
    let o = a.opt_str("orientation")?.unwrap_or("front");
    let orientation = ORIENTATIONS.iter().find(|x| x.0 == o).map(|x| x.1).ok_or_else(|| OpError::param("orientation", "unknown orientation"))?;
    let mut opts = ViewOptions::default();
    if let Some(s) = a.opt_f64("scale")? {
        if s <= 0.0 {
            return Err(OpError::param("scale", "must be greater than zero"));
        }
        opts.insert.scale = Some(s);
    }
    insert_panel(od, a, source, &mut opts.insert)?;
    let tool = Tool::InsertView { source, orientation, part };
    place_view(od, a, tool, opts, &[SourceKey::studio(source)], "Insert view")
}

/// The Insert view panel's options from the arguments.
fn insert_panel(od: &mut OpenDoc, a: &Args, source: TabId, o: &mut noshape_scene::drawing_tools::InsertOpts) -> Result<(), OpError> {
    use noshape_model::drawing::Simplification;
    let doc = od.doc();
    let is_studio = matches!(doc.tab(source).map(|t| &t.content), Some(TabContent::PartStudio(_)));
    if a.get("sketch").is_some() {
        let ps = crate::doc::studio(doc, source).map_err(|_| OpError::param("sketch", "sketch views come from a Part Studio"))?;
        let id = super::feature_arg(ps, a, "sketch")?;
        if ps.feature(id).is_none_or(|f| f.type_id != "sketch") {
            return Err(OpError::param("sketch", "not a sketch"));
        }
        if a.get("part").is_some() {
            return Err(OpError::param("sketch", "give `part` or `sketch`, not both"));
        }
        o.sketch = Some(id);
    }
    if let Some(n) = a.opt_str("named_view")? {
        let names: Vec<String> = dr::named_views(doc, source).iter().map(|v| v.name.clone()).collect();
        let frame = dr::named_view_frame(doc, source, n).ok_or_else(|| OpError::param("named_view", &format!("the source's named views: {}", names.join(", "))))?;
        if a.get("orientation").is_some() {
            return Err(OpError::param("named_view", "give `orientation` or `named_view`, not both"));
        }
        o.named = Some((n.to_string(), frame));
    }
    o.flat = a.bool_or("flat", false)?;
    if o.flat && !is_studio {
        return Err(OpError::param("flat", "flat patterns are of Part Studio sheet metal parts"));
    }
    o.sketch_appearances = a.bool_or("sketch_appearances", false)?;
    o.model_annotations = a.bool_or("model_annotations", true)?;
    let kind = match a.opt_str("simplification")?.unwrap_or("auto") {
        "none" => Simplification::None,
        "auto" => Simplification::Auto,
        "absolute" => Simplification::Absolute(0.001),
        "ratio_to_studio" => Simplification::RatioToStudio(0.01),
        "ratio_to_part" => Simplification::RatioToPart(0.01),
        _ => return Err(OpError::param("simplification", "none, auto, absolute, ratio_to_studio or ratio_to_part")),
    };
    o.simplification = match a.opt_str("simplification_value")? {
        Some(t) => {
            let mm = !matches!(doc.settings.units.length, noshape_units::LengthUnit::Inch | noshape_units::LengthUnit::Foot | noshape_units::LengthUnit::Yard);
            if matches!(kind, Simplification::None | Simplification::Auto) {
                return Err(OpError::param("simplification_value", "only for absolute and ratio simplifications"));
            }
            dr::parse_simplification(kind, t, mm).ok_or_else(|| OpError::param("simplification_value", "a length (\"1 mm\") or a percentage (\"1 %\")"))?
        }
        None => kind,
    };
    if let Some(p) = a.opt_str("explode_position")? {
        let Some(TabContent::Assembly(asm)) = doc.tab(source).map(|t| &t.content) else { return Err(OpError::param("explode_position", "assemblies only")) };
        let choices = dr::asm_positions(asm);
        o.asm_position = choices.iter().find(|c| c.0 == p).map(|c| c.1).ok_or_else(|| OpError::param("explode_position", &format!("one of {}", choices.iter().map(|c| c.0.as_str()).collect::<Vec<_>>().join(", "))))?;
    }
    if let Some(d) = a.opt_str("display_state")? {
        let Some(TabContent::Assembly(asm)) = doc.tab(source).map(|t| &t.content) else { return Err(OpError::param("display_state", "assemblies only")) };
        let names: Vec<&str> = asm.display_states.iter().map(|s| s.name.as_str()).collect();
        if !names.contains(&d) {
            return Err(OpError::param("display_state", &format!("one of {}", names.join(", "))));
        }
        o.display_state = Some(d.to_string());
    }
    Ok(())
}

fn projected_view(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let parent = ViewId(a.opt_u64("parent")?.ok_or_else(|| OpError::param("parent", "a view id"))?);
    let tab = drawing_tab(od, a)?;
    let d = dr::drawing_of(od.doc(), tab).expect("drawing");
    let sheet = sheet_index(d, a)?;
    match d.sheets[sheet].view(parent) {
        None => return Err(OpError::param("parent", "no such view on the sheet")),
        // Help H:projected_view: not from detail views.
        Some(v) if matches!(v.kind, ViewKind::Detail { .. }) => return Err(OpError::param("parent", "Projected views can't be created from detail views")),
        Some(_) => {}
    }
    place_view(od, a, Tool::Projected { parent: Some(parent) }, ViewOptions::default(), &[], "Projected view")
}

fn dimension(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = drawing_tab(od, a)?;
    let d = dr::drawing_of(od.doc(), tab).cloned().expect("drawing");
    let sheet = sheet_index(&d, a)?;
    let name = a.opt_str("kind")?.unwrap_or("smart");
    let kind = DIMENSIONS.iter().find(|x| x.0 == name).map(|x| x.1).ok_or_else(|| OpError::param("kind", "unknown dimension kind"))?;
    let clicks = a.array("picks")?.iter().enumerate().map(|(i, p)| point_value(p, &format!("picks[{i}]"))).collect::<Result<Vec<_>, _>>()?;
    let place = point_value(a.get("place").expect("required"), "place")?;
    let mid_quad = a.bool_or("mid_quad", false)?;
    let m = SheetModel::build(&od.session, tab, d.clone(), sheet, &[]);
    let mut picks: Vec<Pick> = Vec::new();
    for (i, c) in clicks.iter().enumerate() {
        let tool = Tool::Dimension { kind, picks: picks.clone() };
        let n = picks.len();
        let pk = m.pick_snapping(&tool, &[], *c, mid_quad);
        match kind {
            DimTool::MaxMin | DimTool::Foreshorten => pk.more_dimension_pick(kind, &mut picks),
            _ => pk.dimension_click(kind, *c, &mut picks),
        }
        if picks.len() <= n {
            return Err(OpError::param(&format!("picks[{i}]"), "nothing the tool takes there (click on a view edge, or on a vertex / circle center; see drawing_get)"));
        }
    }
    let tool = Tool::Dimension { kind, picks };
    let pk = m.pick(&tool, &[], place);
    let ann = pk.pending_annotation(&ToolValues::default()).ok_or_else(|| OpError::param("picks", "these picks don't make this dimension"))?;
    let id = ann.id;
    // Chamfer, Dimensions style: the two legs as linear dimensions.
    let anns = pk.chamfer_as_dimensions(&ann).unwrap_or_else(|| vec![ann]);
    let mut d = d;
    d.sheets[sheet].annotations.extend(anns);
    commit(od, tab, sheet, d, kind.name())?;
    let mut out = reply(od, tab, sheet, false)?;
    out["annotation"] = json!(id);
    // The value as the sheet shows it.
    let m = SheetModel::build(&od.session, tab, dr::drawing_of(od.doc(), tab).cloned().expect("drawing"), sheet, &[]);
    if let Some(b) = &m.built {
        let texts: Vec<String> = b.geometry.texts.iter().filter(|t| matches!(b.picks.get(t.pick as usize), Some(noshape_model::drawing::build::PickTarget::Annotation(x)) if *x == id)).map(|t| t.text.clone()).collect();
        out["text"] = json!(texts.join(" "));
    }
    Ok(out.into())
}

fn update(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = drawing_tab(od, a)?;
    let mut d = dr::drawing_of(od.doc(), tab).cloned().expect("drawing");
    let sheet = sheet_index(&d, a)?;
    if dr::old_state(&od.session, &d).is_some() {
        // The views are made from the current document from now on.
        d.sources_at = od.session.head().ok().map(|m| m.to_string());
        commit(od, tab, sheet, d, "Update")?;
    }
    Ok(reply(od, tab, sheet, false)?.into())
}

fn export(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use crate::drawing_export::{self as de, DrawingFormat, ExportExtras, ExportSettings};
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = drawing_tab(od, a)?;
    let d = dr::drawing_of(od.doc(), tab).cloned().expect("drawing");
    let f = a.str("format")?;
    let format = DrawingFormat::ALL.into_iter().find(|x| x.label().eq_ignore_ascii_case(f) || x.ext().eq_ignore_ascii_case(f)).ok_or_else(|| OpError::param("format", "one of the listed formats"))?;
    let mut s = ExportSettings { format, version: de::default_version(format), ..Default::default() };
    if let Some(v) = a.opt_str("version")? {
        let list = format.versions();
        s.version = list.iter().position(|x| x.eq_ignore_ascii_case(v)).ok_or_else(|| OpError::param("version", &if list.is_empty() { "this format has no version choice".to_string() } else { format!("one of {}", list.join(", ")) }))?;
    }
    match a.opt_str("sheets")? {
        None | Some("all") => {}
        Some("current") => s.sheets = 1,
        Some(r) => {
            s.sheets = 2;
            s.custom = r.to_string();
        }
    }
    s.overridden = match a.opt_str("overridden_dimensions")? {
        None | Some("show_underlines") => 0,
        Some("hide_underlines") => 1,
        Some(_) => return Err(OpError::param("overridden_dimensions", "show_underlines or hide_underlines")),
    };
    s.text = match (a.opt_str("text")?, format.cad()) {
        (None, _) | (Some("normal"), false) | (Some("export_as_text"), true) => 0,
        (Some("selectable"), false) => 1,
        (Some("explode_notes"), true) => 1,
        (Some("explode_all"), true) => 2,
        _ => return Err(OpError::param("text", if format.cad() { "export_as_text, explode_notes or explode_all" } else { "normal or selectable" })),
    };
    s.color = match a.opt_str("color")? {
        None | Some("color") => 0,
        Some("black_and_white") => 1,
        Some("grayscale") => 2,
        Some(_) => return Err(OpError::param("color", "color, black_and_white or grayscale")),
    };
    if let Some(u) = a.opt_str("units")? {
        s.units = de::UNITS.iter().position(|x| x.0.eq_ignore_ascii_case(u)).ok_or_else(|| OpError::param("units", "one of the listed units"))?;
    }
    s.splines = a.bool_or("splines_as_polylines", false)?;
    s.z_zero = a.bool_or("z_zero", true)?;
    s.hide_inspection = a.bool_or("hide_inspection_items", false)?;
    s.exclude_off_sheet = a.bool_or("exclude_off_sheet", false)?;
    let current = d.active_sheet.min(d.sheets.len().saturating_sub(1));
    let sheets = de::export_sheets_of(&s, d.sheets.len(), current);
    if !format.csv() && sheets.is_empty() {
        return Err(OpError::param("sheets", "No sheets to export"));
    }
    let versions = |id: &str| od.session.document_at_version(id).ok().flatten();
    let sources = noshape_model::drawing::views::regenerate_sources_with(od.doc(), &d, &versions);
    let name = od.doc().tab(tab).map(|t| t.name.clone()).unwrap_or_else(|| "Drawing".into());
    let bytes = if format.csv() {
        // The inspection panel's rows (hole callouts from the model).
        let rows = d.inspection_rows_with(&|si, an| noshape_model::drawing::build::hole_callout_lines(&d, &d.sheets[si], &sources, an), format == DrawingFormat::DefinitionList);
        noshape_model::drawing::inspection::rows_csv(&rows).into_bytes()
    } else {
        let info = dr::sheet_info(&od.session, tab);
        let files = de::export_files(&d, &sheets, &s, &sources, &noshape_model::drawing::views::ViewCache::new(), &info, &ExportExtras::default()).map_err(|e| OpError::new("io", format!("Export failed: {e}")))?;
        let files = if format.raster() { files.into_iter().map(|(n, svg)| de::rasterize(&svg, format).map(|b| (n, b))).collect::<Result<Vec<_>, String>>().map_err(|e| OpError::new("io", format!("Export failed: {e}")))? } else { files };
        de::bundle(files, &name, format)
    };
    let zip = !format.csv() && de::zipped(format, sheets.len());
    let (download, store) = super::export::options_arg(a)?;
    let ext = if zip { "zip" } else { format.ext() };
    let path = match a.opt_str("file")? {
        Some(f) if download => std::path::PathBuf::from(f),
        Some(f) => super::export::store_dir()?.join(std::path::Path::new(f).file_name().unwrap_or_default()),
        None if !download => super::export::store_dir()?.join(format!("{name}.{ext}")),
        None => return Err(OpError::param("file", "required (or options \"store\")")),
    };
    let path = path.as_path();
    std::fs::write(path, &bytes).map_err(|e| OpError::new("io", format!("writing {}: {e}", path.display())).with_param("file"))?;
    let stored = if store { Some(super::export::store_tab(od, tab, path)?) } else { None };
    if !download {
        let _ = std::fs::remove_file(path);
    }
    let mut reply = json!({ "bytes": bytes.len(), "sheets": sheets.len(), "zip": zip });
    if download {
        reply["file"] = json!(path.display().to_string());
    }
    if let Some(t) = stored {
        reply["stored_tab"] = json!(t);
    }
    Ok(reply.into())
}
