//! The graphics area's camera: Zoom to fit, Zoom to selection, Zoom in /
//! out, Zoom to window, Previous view, the standard views, View normal to,
//! Align view with vertical axis, Perspective view; a drawing's Zoom to
//! fit / sheet / window. The camera is the window's (ui_state); with the app
//! open the window's graphics area does the same move, and render_image
//! `view: "window"` draws through it.

use super::{doc_field, s};
use crate::error::OpError;
use crate::registry::{Args, Op, OpOutput, opt, req};
use crate::window::{CameraMove, SheetMove, SheetView, UiRequest, VIEWPORT, ViewCamera};
use crate::{Ctx, OpenDoc};
use glam::{DVec2, DVec3};
use noshape_model::document::{TabContent, TabId};
use noshape_model::reference::Reference;
use noshape_render::{Camera, Projection, StandardView};
use noshape_scene::camera_ops as co;
use serde_json::{Value, json};

const ACTIONS: [&str; 13] = ["get", "zoom_fit", "zoom_sheet", "view", "zoom_selection", "zoom_in", "zoom_out", "zoom_window", "previous", "normal_to", "normal_to_sketch", "align_vertical", "perspective"];

const VIEWS: [(&str, StandardView); 9] = [
    ("isometric", StandardView::Isometric),
    ("front", StandardView::Front),
    ("back", StandardView::Back),
    ("left", StandardView::Left),
    ("right", StandardView::Right),
    ("top", StandardView::Top),
    ("bottom", StandardView::Bottom),
    ("dimetric", StandardView::Dimetric),
    ("trimetric", StandardView::Trimetric),
];

pub fn ops() -> Vec<Op> {
    vec![Op {
        group: "view",
        verb: "camera",
        summary: "Move the graphics area's camera as the view menus do (zoom to fit / selection / window, zoom in / out, previous view, standard views, view normal to, align vertical, perspective)",
        description: "The camera of a Part Studio's or Assembly's graphics area, or a drawing's zoom, as the window has it (kept per tab for the session; `get` reads it). 3D tabs: `zoom_fit` (Zoom to fit: the shown parts, sketches, planes and origin; an assembly's instances), `view` (a standard view zoomed to fit; `view`, default isometric), `zoom_selection` (the `refs`, else the window's selection (ui_select): parts, faces, edges, vertices, sketches, planes; an assembly's instances), `zoom_in` / `zoom_out` (Z / Shift+Z: ×1.75 about the centre, within Onshape's limits), `zoom_window` (`box`: two corners in graphics-area pixels), `previous` (Previous view: back one change), `normal_to` (View normal to the first of `refs`: a planar face, plane, sketch or mate connector; in an assembly a planar face {\"instance\", \"entity\"}; again on the same plane turns to its back), `normal_to_sketch` (View normal to sketch plane: `refs` [the sketch]), `align_vertical` (Align view with vertical axis: `refs` [a straight edge]), `perspective` (`on`). Drawings: `zoom_fit` / `zoom_sheet`, `zoom_in` / `zoom_out` (×2 / ×½), `zoom_window`. The graphics area is 1280×800 px until `viewport` sets it. With the app open the window does the same move at its own size. Returns the camera: the viewing direction and up (world), the target point and the visible height (document units), projection; a drawing's zoom (px per mm) and the sheet point at the centre (mm).",
        fields: vec![
            doc_field(),
            opt("tab", s(), "Part Studio, Assembly or Drawing tab, by name or id. Default: the window's tab."),
            req("action", json!({ "type": "string", "enum": ACTIONS }), "What to do."),
            opt("view", json!({ "type": "string", "enum": VIEWS.map(|v| v.0) }), "view: the standard view (default isometric)."),
            opt("refs", json!({ "type": "array", "items": { "type": ["string", "object"] } }), "zoom_selection / normal_to / normal_to_sketch / align_vertical: entity ids (query_entities); in an assembly instance ids or names, and for normal_to {\"instance\", \"entity\": a planar face id}."),
            opt("box", json!({ "type": "array", "items": { "type": "array", "items": { "type": "number" }, "minItems": 2, "maxItems": 2 }, "minItems": 2, "maxItems": 2 }), "zoom_window: [[x, y], [x, y]] corners in graphics-area pixels from the top left."),
            opt("on", json!({ "type": "boolean" }), "perspective: on or off."),
            opt("viewport", json!({ "type": "array", "items": { "type": "number", "minimum": 16 }, "minItems": 2, "maxItems": 2 }), "The graphics area's size [width, height] in pixels from now on (default 1280×800)."),
        ],
        mutates: false,
        destructive: false,
        run: camera,
    }]
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Studio,
    Assembly,
    Drawing,
}

/// The tab a call names, else the window's tab (a Part Studio, Assembly or
/// Drawing), else the first Part Studio.
fn view_tab(od: &OpenDoc, a: &Args) -> Result<(TabId, Kind), OpError> {
    let kind = |c: &TabContent| match c {
        TabContent::PartStudio(_) => Some(Kind::Studio),
        TabContent::Assembly(_) => Some(Kind::Assembly),
        TabContent::Drawing(_) => Some(Kind::Drawing),
        _ => None,
    };
    let d = od.doc();
    let tab = match a.opt_str("tab")? {
        Some(k) => crate::doc::find_tab(d, k)?,
        None => od.window.active_tab.filter(|t| d.tab(*t).is_some_and(|t| kind(&t.content).is_some())).or_else(|| d.ordered_tabs().into_iter().find(|t| kind(&t.content).is_some()).map(|t| t.id)).ok_or_else(|| OpError::not_found("the document has no Part Studio, Assembly or Drawing").with_param("tab"))?,
    };
    let t = d.tab(tab).expect("found");
    let k = kind(&t.content).ok_or_else(|| OpError::invalid(format!("tab `{}` is a {}: the camera is a Part Studio's, Assembly's or Drawing's", t.name, t.content.kind_name())).with_param("tab"))?;
    Ok((tab, k))
}

fn viewport(a: &Args) -> Result<Option<DVec2>, OpError> {
    let Some(v) = a.opt_array("viewport")? else { return Ok(None) };
    match v.iter().map(Value::as_f64).collect::<Option<Vec<f64>>>().as_deref() {
        Some([w, h]) if *w >= 16.0 && *h >= 16.0 => Ok(Some(DVec2::new(*w, *h))),
        _ => Err(OpError::param("viewport", "[width, height] in pixels, at least 16")),
    }
}

/// The `box` corners (pixels), checked against the graphics area.
fn window_box(a: &Args, size: DVec2) -> Result<(DVec2, DVec2), OpError> {
    let pts = a.opt_array("box")?.ok_or_else(|| OpError::param("box", "zoom_window: [[x, y], [x, y]] in pixels"))?;
    let p = |v: &Value| -> Option<DVec2> {
        let xy = v.as_array()?;
        Some(DVec2::new(xy.first()?.as_f64()?, xy.get(1)?.as_f64()?))
    };
    let (Some(a), Some(b)) = (pts.first().and_then(p), pts.get(1).and_then(p)) else { return Err(OpError::param("box", "[[x, y], [x, y]] in pixels")) };
    if [a, b].iter().any(|q| q.x < 0.0 || q.y < 0.0 || q.x > size.x || q.y > size.y) {
        return Err(OpError::param("box", &format!("corners within the {}×{} px graphics area", size.x, size.y)));
    }
    Ok((a, b))
}

fn camera(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let (tab, kind) = view_tab(od, a)?;
    let action = a.str("action")?;
    if !ACTIONS.contains(&action) {
        return Err(OpError::param("action", &ACTIONS.join(", ")));
    }
    let size = viewport(a)?;
    if kind == Kind::Drawing {
        return sheet(ctx, a, tab, action, size);
    }
    let units = od.doc().settings.units;
    let k = super::query::len_scale(&units);
    // What Zoom to fit frames, and the scene for the selection's bounds.
    let (fit, display, context) = match kind {
        Kind::Studio => {
            let r = od.regenerate(tab, None)?;
            let fit = co::studio_fit_box(&r.display, co::studio_fit_bounds(&r.display, true, None), r.display.show_origin, |pl| !pl.hidden);
            (fit, Some(r.display), Some(r.context))
        }
        _ => {
            let asm = super::assembly::assembly(od, tab)?.clone();
            let result = super::assembly::solve_assembly(od, tab, &asm, &Default::default());
            let fit = asm.instances.iter().filter(|i| !i.hidden).filter_map(|i| result.instance_bounds(i.id)).fold(None, |acc: Option<(DVec3, DVec3)>, (l, h)| Some(acc.map_or((l, h), |(x, y)| (x.min(l), y.max(h)))));
            (fit, None, None)
        }
    };
    let mut vc = od.window.cameras.get(&tab).cloned().unwrap_or_else(|| {
        // A graphics area opens in Trimetric, fitted to its contents.
        let mut camera = Camera::default();
        if let Some((lo, hi)) = fit {
            co::fit_camera_to_box(&mut camera, lo, hi, DVec2::from(VIEWPORT), 0.0);
        }
        ViewCamera { camera, history: Vec::new(), viewport: DVec2::from(VIEWPORT) }
    });
    if let Some(s) = size {
        vc.viewport = s;
    }
    let cam = vc.camera;
    let first_ref = |what: &str| -> Result<&Value, OpError> { a.opt_array("refs")?.and_then(|r| r.first()).ok_or_else(|| OpError::param("refs", what)) };
    let studio_ref = |v: &Value, what: &str| -> Result<Reference, OpError> { v.as_str().and_then(crate::refs::parse_str).ok_or_else(|| OpError::param("refs", what)) };
    let mv = match action {
        "get" => None,
        "zoom_fit" | "zoom_sheet" => {
            let (lo, hi) = fit.ok_or_else(|| OpError::invalid("nothing to zoom to"))?;
            let mut to = cam;
            co::fit_camera_to_box(&mut to, lo, hi, vc.viewport, 0.0);
            vc.set(to);
            Some(CameraMove::Fit(None))
        }
        "view" => {
            let name = a.opt_str("view")?.unwrap_or("isometric");
            let v = VIEWS.iter().find(|v| v.0 == name).ok_or_else(|| OpError::param("view", "isometric, front, back, left, right, top, bottom, dimetric or trimetric"))?.1;
            let mut to = cam;
            to.rotation = Camera::rotation_for(v);
            if let Some((lo, hi)) = fit {
                co::fit_camera_to_box(&mut to, lo, hi, vc.viewport, 0.0);
            }
            vc.set(to);
            Some(CameraMove::Fit(Some(to.rotation)))
        }
        "zoom_selection" => {
            // The `refs`, else the window's selection (ui_select).
            let selected: Vec<Value> = od.window.selection.get(&tab).map(|s| s.iter().map(|k| json!(k)).collect()).unwrap_or_default();
            let list = a.opt_array("refs")?.filter(|l| !l.is_empty()).unwrap_or(&selected);
            if list.is_empty() {
                return Err(OpError::param("refs", "the entities to zoom to (nothing is selected)"));
            }
            let (to, lo, hi) = if kind == Kind::Studio {
                let refs = list.iter().map(|v| studio_ref(v, "entity ids (query_entities)")).collect::<Result<Vec<_>, _>>()?;
                let (lo, hi) = co::selection_bounds(display.as_ref().expect("studio"), &refs).ok_or_else(|| OpError::param("refs", "none of these is shown"))?;
                let mut to = cam;
                co::fit_camera_to_box(&mut to, lo, hi, vc.viewport, 0.0);
                (to, lo, hi)
            } else {
                let asm = super::assembly::assembly(od, tab)?.clone();
                let result = super::assembly::solve_assembly(od, tab, &asm, &Default::default());
                let mut bb: Option<(DVec3, DVec3)> = None;
                for (i, v) in list.iter().enumerate() {
                    let id = super::assembly::find_instance(&asm, v.as_str().unwrap_or_default(), &format!("refs[{i}]"))?;
                    if let Some((l, h)) = result.instance_bounds(id) {
                        bb = Some(bb.map_or((l, h), |(x, y)| (x.min(l), y.max(h))));
                    }
                }
                let (lo, hi) = bb.ok_or_else(|| OpError::param("refs", "none of these is shown"))?;
                (co::fit_instances(&cam, lo, hi, vc.viewport), lo, hi)
            };
            vc.set(to);
            Some(CameraMove::FitBox { lo, hi, sphere: kind == Kind::Assembly })
        }
        "zoom_in" | "zoom_out" => {
            let zoom_in = action == "zoom_in";
            vc.set(co::zoom_step(&cam, zoom_in, fit, vc.viewport));
            Some(CameraMove::Step { zoom_in })
        }
        "zoom_window" => {
            let (p, q) = window_box(a, vc.viewport)?;
            let to = co::zoom_to_window(&cam, p, q, vc.viewport).ok_or_else(|| OpError::param("box", "a box at least 4 px each way"))?;
            vc.set(to);
            Some(CameraMove::Window(p / vc.viewport, q / vc.viewport))
        }
        "previous" => {
            let to = vc.history.pop().ok_or_else(|| OpError::invalid("no previous view"))?;
            vc.camera = to;
            Some(CameraMove::Previous)
        }
        "normal_to" | "normal_to_sketch" => {
            let (normal, up) = if kind == Kind::Studio {
                let what = if action == "normal_to" { "a planar face, plane, sketch or mate connector id" } else { "the sketch (its id)" };
                let r = studio_ref(first_ref(what)?, what)?;
                if action == "normal_to_sketch" && !matches!(r, Reference::Sketch(_)) {
                    return Err(OpError::param("refs", what));
                }
                let f = crate::studio::frame_of_ref(display.as_ref().expect("studio"), context.as_deref(), &r).ok_or_else(|| OpError::param("refs", "Select a planar face, plane, sketch or mate connector"))?;
                (f.normal, f.y)
            } else {
                if action == "normal_to_sketch" {
                    return Err(OpError::param("action", "normal_to_sketch is a Part Studio's"));
                }
                let n = super::assembly::face_normal(od, tab, first_ref("{\"instance\", \"entity\": a planar face id}")?)?;
                (n, co::face_normal_up(n))
            };
            let mut to = cam;
            // The app's: in a Part Studio the plane's y is up; an assembly's face uses Z (Y looking along Z).
            to.rotation = Camera::look_rotation(co::normal_forward(cam.forward(), normal), up);
            vc.set(to);
            Some(CameraMove::Direction { forward: to.forward(), up: to.up() })
        }
        "align_vertical" => {
            if kind != Kind::Studio {
                return Err(OpError::param("action", "align_vertical is a Part Studio's"));
            }
            let r = studio_ref(first_ref("a straight edge id")?, "a straight edge id")?;
            let d = co::linear_edge_direction(display.as_ref().expect("studio"), &r).ok_or_else(|| OpError::param("refs", "a straight edge"))?;
            let (forward, up) = co::align_vertical(&cam, d).ok_or_else(|| OpError::param("refs", "the edge points at the eye"))?;
            let mut to = cam;
            to.rotation = Camera::look_rotation(forward, up);
            vc.set(to);
            Some(CameraMove::Direction { forward, up })
        }
        "perspective" => {
            let on = a.get("on").and_then(Value::as_bool).ok_or_else(|| OpError::param("on", "true or false"))?;
            let mut to = cam;
            to.projection = if on { Projection::Perspective } else { Projection::Orthographic };
            vc.set(to);
            Some(CameraMove::Perspective(on))
        }
        _ => return Err(OpError::param("action", "zoom_fit, view, zoom_selection, zoom_in, zoom_out, zoom_window, previous, normal_to, normal_to_sketch, align_vertical or perspective")),
    };
    let out = camera_json(&vc, k);
    od.window.cameras.insert(tab, vc);
    let shown = match mv {
        Some(mv) => super::show(ctx, a.opt_str("doc")?, UiRequest::Camera { tab, mv })?,
        None => ctx.serving,
    };
    let mut out = out;
    out["tab"] = super::tab_json(ctx.doc(a.opt_str("doc")?)?, tab);
    out["shown_in_app"] = json!(shown);
    Ok(out.into())
}

/// A camera as the API shows it.
pub(crate) fn camera_json(vc: &ViewCamera, k: f64) -> Value {
    let c = &vc.camera;
    let v3 = |v: DVec3| json!([v.x, v.y, v.z]);
    let standard = VIEWS.iter().find(|(_, v)| Camera::rotation_for(*v).dot(c.rotation).abs() > 1.0 - 1e-9).map(|v| v.0);
    json!({
        "view_direction": v3(c.forward()),
        "up": v3(c.up()),
        "target": v3(c.target * k),
        "visible_height": 2.0 * c.half_height() * k,
        "projection": if c.projection == Projection::Perspective { "perspective" } else { "orthographic" },
        "standard_view": standard,
        "viewport": [vc.viewport.x, vc.viewport.y],
        "previous_views": vc.history.len(),
    })
}

fn sheet(ctx: &mut Ctx, a: &Args, tab: TabId, action: &str, size: Option<DVec2>) -> Result<OpOutput, OpError> {
    let od = ctx.doc(a.opt_str("doc")?)?;
    let d = crate::drawing::drawing_of(od.doc(), tab).ok_or_else(|| OpError::invalid("not a drawing").with_param("tab"))?;
    let sh = &d.sheets[d.active_sheet.min(d.sheets.len().saturating_sub(1))];
    let content = co::drawing_content(sh);
    let fitted = |size: [f64; 2]| {
        let (zoom, center) = co::drawing_fit(size, content);
        SheetView { zoom, center, size }
    };
    let mut sv = od.window.sheets.get(&tab).copied().unwrap_or_else(|| fitted(VIEWPORT));
    if let Some(s) = size {
        sv.size = [s.x, s.y];
    }
    let mv = match action {
        "get" => None,
        "zoom_fit" | "zoom_sheet" => {
            sv = fitted(sv.size);
            Some(SheetMove::Fit)
        }
        "zoom_in" | "zoom_out" => {
            let f = if action == "zoom_in" { 2.0 } else { 0.5 };
            (sv.zoom, sv.center) = co::drawing_zoom_about(sv.zoom, sv.center, f, sv.center);
            Some(SheetMove::Step(f))
        }
        "zoom_window" => {
            let s = DVec2::from(sv.size);
            let (p, q) = window_box(a, s)?;
            (sv.zoom, sv.center) = co::drawing_zoom_window(sv.zoom, sv.center, sv.size, p.into(), q.into());
            Some(SheetMove::Window((p / s).into(), (q / s).into()))
        }
        _ => return Err(OpError::param("action", "in a drawing: get, zoom_fit, zoom_sheet, zoom_in, zoom_out or zoom_window")),
    };
    od.window.sheets.insert(tab, sv);
    let shown = match mv {
        Some(mv) => super::show(ctx, a.opt_str("doc")?, UiRequest::SheetZoom { tab, mv })?,
        None => ctx.serving,
    };
    Ok(json!({ "tab": super::tab_json(ctx.doc(a.opt_str("doc")?)?, tab), "zoom": sv.zoom, "center": sv.center, "viewport": sv.size, "shown_in_app": shown }).into())
}
