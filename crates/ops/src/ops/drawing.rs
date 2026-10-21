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

const ORIENTATIONS: [(&str, ViewOrientation); 7] = [
    ("front", ViewOrientation::Front),
    ("back", ViewOrientation::Back),
    ("top", ViewOrientation::Top),
    ("bottom", ViewOrientation::Bottom),
    ("left", ViewOrientation::Left),
    ("right", ViewOrientation::Right),
    ("isometric", ViewOrientation::Isometric),
];

/// Dimension tools: (name, tool, what the clicks pick).
const DIMENSIONS: [(&str, DimTool, &str); 9] = [
    ("smart", DimTool::Linear, "the Dimension tool (d): click one edge (a line gives its length, a circle its diameter, an arc its radius) or two entities (points, lines, circles: distance, angle between lines, wall thickness of concentric circles)"),
    ("point_to_point", DimTool::Point2, "2 point linear: two vertices / circle centers"),
    ("point_to_line", DimTool::PointLine, "a point and a line"),
    ("line_to_line", DimTool::LineLine, "two parallel lines"),
    ("radius", DimTool::Radius, "one circle or arc"),
    ("diameter", DimTool::Diameter, "one circle or arc"),
    ("angle", DimTool::Angle, "two lines"),
    ("angle_3_point", DimTool::Angle3, "vertex, then a point on each leg"),
    ("arc_length", DimTool::ArcLength, "one arc"),
];

fn point() -> Value {
    json!({ "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 })
}

fn drawing_field() -> crate::registry::Field {
    req("drawing", s(), "Drawing tab, by name (\"Part 1 Drawing 1\") or id.")
}

fn sheet_field() -> crate::registry::Field {
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
            description: "Create Drawing: a Drawing tab with the template's sheet, placed after the source tab. With `suggested_views` (default true) the source's suggested views (Front, Top, Right projections and an isometric, scaled to fit) are laid out, as the dialog's checkbox does. Returns the tab and its views.",
            fields: vec![
                doc_field(),
                opt("source", s(), "Part Studio or assembly tab (name or id). Omit for an empty drawing."),
                opt("part", s(), "Only this part (`body:…` id from query_parts)."),
                opt("template", s(), "Template file or setup name (`drawing_templates`), e.g. \"ANSI_A_INCH.dwt\", \"ISO A3\". Default: ISO A3 in metric workspaces, ANSI B (inch) otherwise."),
                opt("name", s(), "Tab name (default \"<part> Drawing N\" / \"Drawing N\")."),
                opt("suggested_views", json!({ "type": "boolean", "default": true }), "Lay out the suggested views."),
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
                opt("part", s(), "Only this part (`body:…`)."),
                opt("orientation", json!({ "type": "string", "enum": ORIENTATIONS.iter().map(|o| o.0).collect::<Vec<_>>(), "default": "front" }), "View orientation."),
                req("position", point(), "View center on the sheet (mm)."),
                opt("scale", json!({ "type": "number", "exclusiveMinimum": 0 }), "View scale (paper / model)."),
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
            ],
            mutates: true,
            destructive: false,
            run: dimension,
        },
        Op {
            group: "drawing",
            verb: "export",
            summary: "Export a drawing to PDF, DWG, DXF, DWT or SVG",
            description: "The Export Drawing modal: all sheets (or `sheets`: \"current\" or a range like \"1-3, 5\") to one file; PDF and DWT hold every sheet, other formats with several sheets are zipped (one file per sheet).",
            fields: vec![
                doc_field(),
                drawing_field(),
                req("format", json!({ "type": "string", "enum": ["PDF", "DWG", "DXF", "DWT", "SVG"] }), "Format."),
                req("file", s(), "Output path."),
                opt("sheets", s(), "\"all\" (default), \"current\" or a range \"1-3, 5\"."),
                opt("color", json!({ "type": "string", "enum": ["color", "black_and_white", "grayscale"] }), "PDF color (default color)."),
            ],
            mutates: false,
            destructive: false,
            run: export,
        },
    ]
}

// ------------------------------------------------------------ helpers

fn point_value(v: &Value, param: &str) -> Result<[f64; 2], OpError> {
    let a = v.as_array().filter(|a| a.len() == 2).ok_or_else(|| OpError::param(param, "a point is [x, y] in sheet mm"))?;
    let n = |i: usize| a[i].as_f64().ok_or_else(|| OpError::param(param, "numbers (sheet mm)"));
    Ok([n(0)?, n(1)?])
}

fn drawing_tab(od: &OpenDoc, a: &Args) -> Result<TabId, OpError> {
    doc::tab_of_kind(od.doc(), Some(a.str("drawing")?), "Drawing", |c| matches!(c, TabContent::Drawing(_))).map_err(|e| e.with_param("drawing"))
}

fn sheet_index(d: &Drawing, a: &Args) -> Result<usize, OpError> {
    match a.get("sheet") {
        None => Ok(d.active_sheet.min(d.sheets.len().saturating_sub(1))),
        Some(Value::Number(n)) => n.as_u64().filter(|n| (1..=d.sheets.len() as u64).contains(n)).map(|n| n as usize - 1).ok_or_else(|| OpError::param("sheet", &format!("sheet numbers are 1 to {}", d.sheets.len()))),
        Some(Value::String(s)) => d.sheets.iter().position(|x| x.name == *s).ok_or_else(|| OpError::param("sheet", &format!("no sheet `{s}`; sheets: {}", d.sheets.iter().map(|x| x.name.clone()).collect::<Vec<_>>().join(", ")))),
        Some(_) => Err(OpError::param("sheet", "a number or a name")),
    }
}

fn source_tab(od: &OpenDoc, a: &Args, name: &str) -> Result<TabId, OpError> {
    doc::tab_of_kind(od.doc(), Some(a.str(name)?), "Part Studio or Assembly", |c| matches!(c, TabContent::PartStudio(_) | TabContent::Assembly(_))).map_err(|e| e.with_param(name))
}

fn part_arg(a: &Args) -> Result<Option<noshape_model::BodyId>, OpError> {
    match a.opt_str("part")? {
        None => Ok(None),
        Some(p) => match refs::parse_str(p) {
            Some(noshape_model::reference::Reference::Body(b)) => Ok(Some(b)),
            _ => Err(OpError::param("part", "a part id (`body:…` from query_parts)")),
        },
    }
}

/// Commit an edited drawing as the editor does ("<tab> :: <description>").
fn commit(od: &mut OpenDoc, tab: TabId, sheet: usize, mut d: Drawing, description: &str) -> Result<(), OpError> {
    dr::finish_edit(&mut d, sheet);
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

fn kind_name(k: &ViewKind) -> &'static str {
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
fn variant_name<T: std::fmt::Debug>(v: &T) -> String {
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
            // What the view shows: a base view's orientation, a projected
            // view's side of its parent.
            match &v.kind {
                ViewKind::Base { orientation, .. } => {
                    j["orientation"] = json!(ORIENTATIONS.iter().find(|o| o.1 == *orientation).map(|o| o.0));
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
                }
                _ => {}
            }
            if let Some(l) = &v.label {
                j["label"] = json!(l);
            }
            let pl = m.built.as_ref().and_then(|b| b.placement(v.id)).copied();
            if let Some(pl) = pl {
                j["bounds"] = json!({ "min": pt(pl.min), "max": pt(pl.max) });
            }
            if edges
                && let (Some(pl), Some(g)) = (pl, m.geoms.get(&v.id))
            {
                let list: Vec<Value> = g
                    .model_edges
                    .iter()
                    .filter(|e| !e.seam)
                    .filter_map(|e| match e.geom {
                        EdgeGeom::Line { a, b } => Some(json!({ "type": "line", "a": pt(pl.to_sheet(a)), "b": pt(pl.to_sheet(b)) })),
                        EdgeGeom::Circle { center, radius, face_on: true, .. } => {
                            // Closed: no end vertices, or one seam vertex at both ends.
                            let closed = match (&e.ends[0], &e.ends[1]) {
                                (Some((_, a)), Some((_, b))) => (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-9,
                                _ => true,
                            };
                            let mut c = json!({ "type": if closed { "circle" } else { "arc" }, "center": pt(pl.to_sheet(center)), "radius": r3(pl.len_to_sheet(radius)) });
                            if let (false, Some((_, a)), Some((_, b))) = (closed, &e.ends[0], &e.ends[1]) {
                                c["a"] = pt(pl.to_sheet(*a));
                                c["b"] = pt(pl.to_sheet(*b));
                            }
                            Some(c)
                        }
                        _ => None,
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
            j
        })
        .collect();
    let tables: Vec<Value> = sheet.tables.iter().map(|t| json!({ "id": t.id, "kind": variant_name(&t.kind), "position": pt(t.position) })).collect();
    json!({ "sheet": sheet.name, "size_mm": [r3(w), r3(h)], "views": views, "annotations": annotations, "tables": tables })
}

fn reply(od: &mut OpenDoc, tab: TabId, sheet: usize, edges: bool) -> Result<Value, OpError> {
    let d = dr::drawing_of(od.doc(), tab).cloned().ok_or_else(|| OpError::not_found("the drawing tab is gone"))?;
    let m = SheetModel::build(&od.session, tab, d.clone(), sheet, &[]);
    let mut v = sheet_json(&m, edges);
    v["drawing"] = json!(od.doc().tab(tab).map(|t| t.name.clone()));
    v["tab"] = json!(tab.to_string());
    v["sheets"] = json!(d.sheets.iter().map(|s| s.name.clone()).collect::<Vec<_>>());
    Ok(v)
}

// ------------------------------------------------------------ handlers

fn templates(_: &mut Ctx, _: &Args) -> Result<OpOutput, OpError> {
    Ok(json!({ "templates": dr::template_rows().iter().map(|r| json!({ "file": r.file, "name": r.template.name, "standard": r.standard, "units": if r.template.inches { "inch" } else { "millimeter" } })).collect::<Vec<_>>() }).into())
}

fn create(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let source = match a.opt_str("source")? {
        Some(_) => Some(DrawingSource { tab: source_tab(od, a, "source")?, part: part_arg(a)?, config: Default::default(), sketch: None }),
        None if a.get("part").is_some() => return Err(OpError::param("part", "give `source` with `part`")),
        None => None,
    };
    let metric = !matches!(od.doc().settings.units.length, noshape_units::LengthUnit::Inch | noshape_units::LengthUnit::Foot | noshape_units::LengthUnit::Yard);
    let rows = dr::template_rows();
    let template = match a.opt_str("template")? {
        Some(t) => rows.iter().find(|r| r.file.eq_ignore_ascii_case(t)).or_else(|| rows.iter().find(|r| r.template.name.eq_ignore_ascii_case(t))).map(|r| r.template).ok_or_else(|| OpError::param("template", "unknown template (see drawing_templates)"))?,
        None => rows.iter().find(|r| if metric { r.template.name == "ISO A3" } else { r.file == "ANSI_B_INCH.dwt" }).map(|r| r.template).expect("standard template"),
    };
    // Onshape: "<part> Drawing 1" for a part or assembly, else "Drawing 1".
    let base = match &source {
        Some(s) => {
            let label = match &s.part {
                Some(b) => od.regenerate(s.tab, None).ok().and_then(|r| r.display.bodies.iter().find(|x| &x.id == b).map(|x| x.name.clone())),
                None => od.doc().tab(s.tab).map(|t| t.name.clone()),
            };
            label.map(|n| format!("{n} Drawing")).unwrap_or_else(|| "Drawing".into())
        }
        None => "Drawing".into(),
    };
    let name = match a.opt_str("name")? {
        Some(n) => n.trim().to_string(),
        None => od.doc().next_tab_name(&base),
    };
    let drawing = dr::new_drawing(od.doc(), &template, source.as_ref(), a.bool_or("suggested_views", true)?);
    let mut tab = None;
    od.session.edit_document(&format!("Insert tab : {name}"), |d| tab = Some(crate::docedit::insert_drawing_tab(d, &name, drawing, source.as_ref().map(|s| s.tab))))?;
    let tab = tab.expect("inserted");
    Ok(reply(od, tab, 0, false)?.into())
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
    let part = part_arg(a)?;
    let o = a.opt_str("orientation")?.unwrap_or("front");
    let orientation = ORIENTATIONS.iter().find(|x| x.0 == o).map(|x| x.1).ok_or_else(|| OpError::param("orientation", "unknown orientation"))?;
    let mut opts = ViewOptions::default();
    if let Some(s) = a.opt_f64("scale")? {
        if s <= 0.0 {
            return Err(OpError::param("scale", "must be greater than zero"));
        }
        opts.insert.scale = Some(s);
    }
    let tool = Tool::InsertView { source, orientation, part };
    place_view(od, a, tool, opts, &[SourceKey::studio(source)], "Insert view")
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
    let m = SheetModel::build(&od.session, tab, d.clone(), sheet, &[]);
    let mut picks: Vec<Pick> = Vec::new();
    for (i, c) in clicks.iter().enumerate() {
        let tool = Tool::Dimension { kind, picks: picks.clone() };
        let n = picks.len();
        m.pick(&tool, &[], *c).dimension_click(kind, *c, &mut picks);
        if picks.len() <= n {
            return Err(OpError::param(&format!("picks[{i}]"), "nothing the tool takes there (click on a view edge, or on a vertex / circle center; see drawing_get)"));
        }
    }
    let tool = Tool::Dimension { kind, picks };
    let ann = m.pick(&tool, &[], place).pending_annotation(&ToolValues::default()).ok_or_else(|| OpError::param("picks", "these picks don't make this dimension"))?;
    let id = ann.id;
    let mut d = d;
    d.sheets[sheet].annotations.push(ann);
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

fn export(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    use crate::drawing_export::{DrawingFormat, ExportSettings, bundle, export_files, export_sheets_of, zipped};
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = drawing_tab(od, a)?;
    let d = dr::drawing_of(od.doc(), tab).cloned().expect("drawing");
    let f = a.str("format")?;
    let format = DrawingFormat::ALL.into_iter().filter(|x| !x.raster() && !x.csv()).find(|x| x.label().eq_ignore_ascii_case(f)).ok_or_else(|| OpError::param("format", "PDF, DWG, DXF, DWT or SVG"))?;
    let mut s = ExportSettings { format, ..Default::default() };
    match a.opt_str("sheets")? {
        None | Some("all") => {}
        Some("current") => s.sheets = 1,
        Some(r) => {
            s.sheets = 2;
            s.custom = r.to_string();
        }
    }
    s.color = match a.opt_str("color")? {
        None | Some("color") => 0,
        Some("black_and_white") => 1,
        Some("grayscale") => 2,
        Some(_) => return Err(OpError::param("color", "color, black_and_white or grayscale")),
    };
    let current = d.active_sheet.min(d.sheets.len().saturating_sub(1));
    let sheets = export_sheets_of(&s, d.sheets.len(), current);
    if sheets.is_empty() {
        return Err(OpError::param("sheets", "No sheets to export"));
    }
    let versions = |id: &str| od.session.document_at_version(id).ok().flatten();
    let sources = noshape_model::drawing::views::regenerate_sources_with(od.doc(), &d, &versions);
    let info = dr::sheet_info(&od.session, tab);
    let files = export_files(&d, &sheets, &s, &sources, &noshape_model::drawing::views::ViewCache::new(), &info).map_err(|e| OpError::new("io", format!("Export failed: {e}")))?;
    let name = od.doc().tab(tab).map(|t| t.name.clone()).unwrap_or_else(|| "Drawing".into());
    let bytes = bundle(files, &name, format);
    let path = std::path::Path::new(a.str("file")?);
    std::fs::write(path, &bytes).map_err(|e| OpError::new("io", format!("writing {}: {e}", path.display())).with_param("file"))?;
    Ok(json!({ "file": path.display().to_string(), "bytes": bytes.len(), "sheets": sheets.len(), "zip": zipped(format, sheets.len()) }).into())
}
