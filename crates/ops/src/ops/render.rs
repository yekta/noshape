//! Seeing the model: rendered images of a Part Studio from any view.

use super::{doc_field, s, studio_target, tab_field};
use crate::error::OpError;
use crate::registry::{Args, Image, Op, OpOutput, opt, req};
use crate::Ctx;
use noshape_render::StandardView;
use noshape_scene::builder::{self, DisplayMode, SceneOptions};
use serde_json::json;

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
    vec![
    Op {
        group: "render",
        verb: "named_view",
        summary: "Save or delete a named view of a Part Studio",
        description: "The Named views dialog's \"+\" (save the view, here a standard orientation zoomed to fit, with an optional section like render_image's) or its remove button (`delete`). An empty name gives \"View 1\", \"View 2\"... render_image and drawing_insert_view take named views.",
        fields: vec![
            doc_field(),
            tab_field(),
            req("name", s(), "View name (empty: the next \"View n\")."),
            opt("view", s(), "Standard orientation to save (default isometric)."),
            opt("section", json!({ "type": "array", "items": { "type": "object" }, "maxItems": 3 }), "Section planes saved with the view (as render_image)."),
            opt("delete", json!({ "type": "boolean" }), "Delete the named view instead."),
        ],
        mutates: true,
        destructive: false,
        run: named_view,
    },
    Op {
        group: "render",
        verb: "image",
        summary: "Render a Part Studio or an Assembly to a PNG image from a standard or named view",
        description: "Draws the parts, sketches, planes and origin as the 3D view does (Onshape's look: white background, shaded with edges), zoomed to fit. With `tab` an Assembly: the solved assembly in its active configuration (standard views; sections on plane:top|front|right; `section_exclude` instance names). The view menu's options as in the app: `perspective`, `tangent_edges`, `hidden_edges`, `boundary_edges`, `isolate` / `transparent` (Make isolate / Make transparent), `normal_to` (View normal to), `high_quality` (View in high quality), and the analysis displays `zebra` (Zebra stripes) and `reflection` (Reflection analysis). Returned as an image (MCP image content) and, with `file`, written to disk.",
        fields: vec![
            doc_field(),
            opt("tab", s(), "Part Studio or Assembly tab, by name or id. Default: the first Part Studio."),
            opt("section", json!({ "type": "array", "items": { "type": "object" }, "maxItems": 3 }), "Section view: up to 3 planes [{\"plane\": a plane / planar face / sketch / mate connector id, \"offset\": length along its normal (default 0), \"flip\": cut the other side}]; the side the normal points to is cut away. A named `view` keeps its own section unless this is given."),
            opt("section_exclude", json!({ "type": "array", "items": { "type": "string" } }), "Section view › Items to exclude: parts (`body:…`) left whole."),
            opt("view", json!({ "type": "string" }), "isometric (default), front, back, left, right, top, bottom, dimetric, trimetric, the name of a saved named view, or \"window\": the window's camera (view_camera; perspective and fit options then don't apply)."),
            opt("width", json!({ "type": "integer", "minimum": 16, "maximum": 4096 }), "Pixels (default 800)."),
            opt("height", json!({ "type": "integer", "minimum": 16, "maximum": 4096 }), "Pixels (default 600)."),
            opt("display_mode", json!({ "type": "string", "enum": ["shaded_with_edges", "shaded", "hidden_line", "translucent"] }), "Render style (default shaded_with_edges)."),
            opt("show_sketches", json!({ "type": "boolean", "default": true }), "Draw sketches."),
            opt("show_planes", json!({ "type": "boolean", "default": false }), "Draw the default and construction planes."),
            opt("perspective", json!({ "type": "boolean", "default": false }), "Perspective view (default orthographic)."),
            opt("tangent_edges", json!({ "type": "string", "enum": ["visible", "phantom", "hidden"], "default": "visible" }), "Tangent edges visible / phantom / hidden."),
            opt("hidden_edges", json!({ "type": "boolean", "default": false }), "Hide/show hidden edges: draw the hidden ones."),
            opt("boundary_edges", json!({ "type": "boolean", "default": false }), "Part Studios: Highlight boundary edges."),
            opt("isolate", json!({ "type": "array", "items": { "type": "string" } }), "Make isolate: these parts (Part Studio ids or names) or instances (Assembly), the rest as ghosts."),
            opt("transparent", json!({ "type": "array", "items": { "type": "string" } }), "Make transparent: these parts or instances as ghosts."),
            opt("normal_to", s(), "Part Studios: View normal to a planar face, plane, sketch or mate connector (replaces `view`)."),
            opt("high_quality", json!({ "type": "boolean", "default": false }), "View in high quality (finer tessellation)."),
            opt("zebra", json!({ "type": "object" }), "Zebra stripes: {\"stripes\": 35, \"flip\": false, \"show_edges\": true}."),
            opt("reflection", json!({ "type": "object" }), "Reflection analysis: {\"image\": an environment map name (\"Blue sky\", default the first) or an image file path, \"rotation\": degrees, \"opposite\": bool, \"blur\": bool, \"show_edges\": bool (default false)}."),
            opt("file", s(), "Also write the PNG here."),
        ],
        mutates: false,
        destructive: false,
        run: image,
    }]
}

fn image(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    // `view: "window"`: the window's display settings where the call gives none.
    let merged = window_display_args(ctx, a)?;
    let held;
    let a = match &merged {
        Some(v) => {
            held = Args { op: a.op, value: v };
            &held
        }
        None => a,
    };
    let width = a.opt_u64("width")?.unwrap_or(800).clamp(16, 4096) as u32;
    let height = a.opt_u64("height")?.unwrap_or(600).clamp(16, 4096) as u32;
    let mode = match a.opt_str("display_mode")? {
        None | Some("shaded_with_edges") => DisplayMode::ShadedWithEdges,
        Some("shaded") => DisplayMode::Shaded,
        Some("hidden_line") => DisplayMode::HiddenLine,
        Some("translucent") => DisplayMode::Translucent,
        Some(_) => return Err(OpError::param("display_mode", "unknown display mode")),
    };
    let vo = view_options(a)?;
    let mut opts = SceneOptions {
        show_planes: a.bool_or("show_planes", false)?,
        show_sketches: a.bool_or("show_sketches", true)?,
        show_origin: a.bool_or("show_planes", false)?,
        display_mode: vo.display_mode(mode),
        tangent_edges: vo.tangent,
        hidden_edges_visible: vo.hidden_edges,
        zebra: vo.zebra,
        ..Default::default()
    };
    let view_name = a.opt_str("view")?.unwrap_or("isometric").to_string();
    if super::configuration::asm::is_assembly(ctx, a)? {
        for k in ["boundary_edges", "normal_to"] {
            if a.get(k).is_some() {
                return Err(OpError::param(k, "Part Studios only"));
            }
        }
        return image_assembly(ctx, a, width, height, vo, mode, &view_name);
    }
    opts.highlight_boundary = a.bool_or("boundary_edges", false)?;
    let (od, tab) = studio_target(ctx, a)?;
    let named = crate::doc::studio(od.doc(), tab)?.named_views.iter().find(|v| v.name.eq_ignore_ascii_case(&view_name)).cloned();
    let view = VIEWS.iter().find(|v| v.0 == view_name).map(|v| v.1);
    let window = window_camera(od, tab, &view_name)?;
    if view.is_none() && named.is_none() && window.is_none() {
        return Err(OpError::param("view", "a standard view (isometric, front, top, ...), a named view of this Part Studio, or \"window\""));
    }
    // View in high quality: the parts tessellated 4x finer (as the app's).
    let r = if vo.high_quality { od.regenerate_with(tab, None, |o| o.relative_deflection *= 0.25)? } else { od.regenerate(tab, None)? };
    // Make isolate / Make transparent: the others / these parts as ghosts.
    let pick = |k: &str| -> Result<Option<Vec<noshape_model::BodyId>>, OpError> {
        let Some(list) = a.opt_array(k)? else { return Ok(None) };
        list.iter()
            .enumerate()
            .map(|(i, v)| {
                let key = v.as_str().unwrap_or_default();
                r.display.bodies.iter().find(|b| crate::refs::body_id(&b.id) == key || b.name == key).map(|b| b.id.clone()).ok_or_else(|| OpError::param(&format!("{k}[{i}]"), "a part (id or name)"))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    };
    if let Some(chosen) = pick("isolate")? {
        opts.ghost_bodies = r.display.bodies.iter().filter(|b| !chosen.contains(&b.id)).map(|b| b.id.clone()).collect();
        opts.faint_planes = true;
    }
    if let Some(chosen) = pick("transparent")? {
        opts.ghost_bodies.extend(chosen);
    }
    let normal_to = match a.opt_str("normal_to")? {
        Some(id) => {
            let rf = crate::refs::parse_str(id).ok_or_else(|| OpError::param("normal_to", "a planar face, plane, sketch or mate connector id"))?;
            let f = crate::studio::frame_of_ref(&r.display, Some(&r.context), &rf).ok_or_else(|| OpError::param("normal_to", "Select a planar face, plane, sketch or mate connector"))?;
            Some(f)
        }
        None => None,
    };
    // Section view: the planes given, else a named view's own.
    if a.get("section").is_some() {
        let (planes, skip) = section_planes(od, &r, a)?;
        opts.section = planes;
        opts.section_skip = skip;
    } else if let Some(v) = &named {
        opts.section = v.section.iter().map(|p| (glam::DVec3::new(p[0], p[1], p[2]), p[3])).collect();
    }
    let _gpu = crate::doc::gpu_lock();
    let off = ctx.offscreen()?;
    let up = off.renderer.uploader();
    opts.reflection = vo.upload_reflection(&up);
    let built = builder::build(&r.display, &opts, &mut off.cache, &up);
    let (lo, hi) = built.scene.bounds().map(|(l, h)| (l.as_dvec3(), h.as_dvec3())).unwrap_or((glam::DVec3::splat(-0.05), glam::DVec3::splat(0.05)));
    let aspect = width as f64 / height as f64;
    let camera = match named {
        _ if window.is_some() => window.expect("checked"),
        Some(v) => {
            let mut c = noshape_scene::png::fit_camera(lo, hi, aspect, None);
            c.rotation = glam::DQuat::from_xyzw(v.rotation[0], v.rotation[1], v.rotation[2], v.rotation[3]);
            c.target = glam::DVec3::from_array(v.target);
            c.distance = v.distance;
            c
        }
        None => noshape_scene::png::fit_camera(lo, hi, aspect, view),
    };
    let mut camera = camera;
    // View normal to: looking at the plane's front, its y up (the app's).
    if let Some(f) = normal_to {
        camera.rotation = noshape_render::Camera::look_rotation(-f.normal, f.y);
        camera.fit_box(lo, hi, aspect);
    }
    if window.is_none() {
        vo.finish_camera(&mut camera, lo, hi, aspect);
    }
    let style = vo.style();
    let png = off.render_png(&built.scene, &camera, &style, width, height).map_err(|e| OpError::new("render_failed", e))?;
    let mut v = json!({ "width": width, "height": height, "view": if normal_to.is_some() { "normal_to".to_string() } else { view_name }, "bytes": png.len() });
    if let Some(f) = a.opt_str("file")? {
        std::fs::write(f, &png).map_err(|e| OpError::new("io", format!("writing {f}: {e}")).with_param("file"))?;
        v["file"] = json!(f);
    }
    Ok(OpOutput { value: v, images: vec![Image { png }] })
}

/// The `section` / `section_exclude` arguments: clip planes (normal, d:
/// geometry with normal · p > d is cut away) and the parts left whole.
/// Section clip planes (normal, d) and the parts left whole.
type Section = (Vec<(glam::DVec3, f64)>, Vec<noshape_model::BodyId>);

fn section_planes(od: &crate::OpenDoc, r: &noshape_model::regen::RegenResult, a: &Args) -> Result<Section, OpError> {
    let (mut planes, mut skip) = (Vec::new(), Vec::new());
    let list = a.opt_array("section")?.cloned().unwrap_or_default();
    if list.len() > 3 {
        return Err(OpError::param("section", "at most 3 section planes"));
    }
    let units = od.doc().settings.units;
    for (i, p) in list.iter().enumerate() {
        let field = format!("section[{i}]");
        let o = p.as_object().ok_or_else(|| OpError::param(&field, "{\"plane\": reference id, \"offset\", \"flip\"}"))?;
        let rf = o.get("plane").and_then(|v| v.as_str()).and_then(crate::refs::parse_str).ok_or_else(|| OpError::param(&format!("{field}.plane"), "a plane, planar face, sketch or mate connector id"))?;
        let f = crate::studio::frame_of_ref(&r.display, Some(&r.context), &rf).ok_or_else(|| OpError::param(&format!("{field}.plane"), "Select a plane, planar face, cylinder, cone, or mate connector."))?;
        let offset = match o.get("offset") {
            Some(v) => crate::ops::sketch::length_value(v, &units).map_err(|e| e.with_param(format!("{field}.offset")))?,
            None => 0.0,
        };
        let n = if o.get("flip").and_then(|v| v.as_bool()) == Some(true) { -f.normal } else { f.normal };
        planes.push((n, n.dot(f.origin + f.normal * offset)));
    }
    for (i, v) in a.opt_array("section_exclude")?.cloned().unwrap_or_default().iter().enumerate() {
        match v.as_str().and_then(crate::refs::parse_str) {
            Some(noshape_model::reference::Reference::Body(b)) => skip.push(b),
            _ => return Err(OpError::param(&format!("section_exclude[{i}]"), "a part id (`body:…`)")),
        }
    }
    Ok((planes, skip))
}

fn named_view(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let (od, tab) = studio_target(ctx, a)?;
    let name = a.str("name")?.trim().to_string();
    if a.bool_or("delete", false)? {
        let ps = crate::doc::studio(od.doc(), tab)?;
        if !ps.named_views.iter().any(|v| v.name == name) {
            return Err(OpError::param("name", "no named view of that name"));
        }
        super::edit_studio(od, tab, &format!("Delete view : {name}"), |ps| {
            ps.named_views.retain(|v| v.name != name);
            Ok(())
        })?;
        return Ok(json!({ "deleted": name }).into());
    }
    let view_name = a.opt_str("view")?.unwrap_or("isometric");
    let view = VIEWS.iter().find(|v| v.0 == view_name).map(|v| v.1).ok_or_else(|| OpError::param("view", "isometric, front, back, left, right, top, bottom, dimetric or trimetric"))?;
    let r = od.regenerate(tab, None)?;
    let (planes, _) = section_planes(od, &r, a)?;
    let (lo, hi) = r.display.bodies.iter().filter(|b| !b.hidden).filter_map(|b| b.bounds).fold(None, |acc: Option<(glam::DVec3, glam::DVec3)>, (l, h)| Some(acc.map_or((l, h), |(a, b)| (a.min(l), b.max(h))))).unwrap_or((glam::DVec3::splat(-0.05), glam::DVec3::splat(0.05)));
    let c = noshape_scene::png::fit_camera(lo, hi, 4.0 / 3.0, Some(view));
    let name = if name.is_empty() {
        let existing: Vec<String> = crate::doc::studio(od.doc(), tab)?.named_views.iter().map(|v| v.name.clone()).collect();
        (1..).map(|n| format!("View {n}")).find(|n| !existing.contains(n)).unwrap_or_default()
    } else {
        name
    };
    let v = noshape_model::partstudio::NamedView { name: name.clone(), target: c.target.to_array(), rotation: c.rotation.to_array(), distance: c.distance, perspective: false, section: planes.iter().map(|(n, d)| [n.x, n.y, n.z, *d]).collect() };
    super::edit_studio(od, tab, &format!("Save view : {name}"), |ps| {
        ps.named_views.retain(|x| x.name != v.name);
        ps.named_views.push(v);
        Ok(())
    })?;
    Ok(json!({ "saved": name, "named_views": crate::doc::studio(od.doc(), tab)?.named_views.iter().map(|v| v.name.clone()).collect::<Vec<_>>() }).into())
}

/// render_image of an Assembly tab: the solved assembly in its active
/// configuration, drawn as the Assembly view does.
fn image_assembly(ctx: &mut Ctx, a: &Args, width: u32, height: u32, vo: ViewOptions, mode: DisplayMode, view_name: &str) -> Result<OpOutput, OpError> {
    use noshape_model::document::TabContent;
    use noshape_scene::asm_scene::{self, AsmGpuCache, AsmSceneOptions};
    let view = VIEWS.iter().find(|v| v.0 == view_name).map(|v| v.1);
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = od.tab_of_kind(a.opt_str("tab")?, "Assembly", |c| matches!(c, TabContent::Assembly(_)))?;
    let window = window_camera(od, tab, view_name)?;
    if view.is_none() && window.is_none() {
        return Err(OpError::param("view", "a standard view (isometric, front, back, left, right, top, bottom, dimetric, trimetric) or \"window\""));
    }
    let Some(TabContent::Assembly(asm)) = od.doc().tab(tab).map(|t| &t.content) else { unreachable!("checked") };
    let values = super::configuration::asm::values(od, tab, asm);
    let eff = crate::assembly::effective(asm, &values);
    let mut opts = AsmSceneOptions { display_mode: mode, tangent_edges: vo.tangent, hidden_edges_visible: vo.hidden_edges, zebra: vo.zebra, hide_edges: vo.hide_edges(), ..Default::default() };
    // Make isolate / Make transparent (instances).
    let tops = |k: &str| -> Result<Option<std::collections::HashSet<noshape_model::assembly::InstanceId>>, OpError> {
        let Some(list) = a.opt_array(k)? else { return Ok(None) };
        list.iter().enumerate().map(|(i, v)| super::assembly::find_instance(asm, v.as_str().unwrap_or_default(), &format!("{k}[{i}]"))).collect::<Result<_, _>>().map(Some)
    };
    opts.isolate = tops("isolate")?;
    opts.ghost_tops = tops("transparent")?.unwrap_or_default();
    // Section view: the default planes (an assembly's own planes).
    let units = od.doc().settings.units;
    for (i, p) in a.opt_array("section")?.cloned().unwrap_or_default().iter().enumerate() {
        use noshape_model::display::PlaneFrame;
        use noshape_model::reference::{DatumPlane, Reference};
        let field = format!("section[{i}]");
        let o = p.as_object().ok_or_else(|| OpError::param(&field, "{\"plane\": \"plane:top|front|right\", \"offset\", \"flip\"}"))?;
        let f = match o.get("plane").and_then(|v| v.as_str()).and_then(crate::refs::parse_str) {
            Some(Reference::Plane(DatumPlane::Top)) => PlaneFrame::TOP,
            Some(Reference::Plane(DatumPlane::Front)) => PlaneFrame::FRONT,
            Some(Reference::Plane(DatumPlane::Right)) => PlaneFrame::RIGHT,
            _ => return Err(OpError::param(&format!("{field}.plane"), "in an assembly: plane:top, plane:front or plane:right")),
        };
        let offset = match o.get("offset") {
            Some(v) => crate::ops::sketch::length_value(v, &units).map_err(|e| e.with_param(format!("{field}.offset")))?,
            None => 0.0,
        };
        let n = if o.get("flip").and_then(|v| v.as_bool()) == Some(true) { -f.normal } else { f.normal };
        opts.section.push((n, n.dot(f.origin + f.normal * offset)));
    }
    for (i, v) in a.opt_array("section_exclude")?.cloned().unwrap_or_default().iter().enumerate() {
        let id = super::assembly::find_instance(asm, v.as_str().unwrap_or_default(), &format!("section_exclude[{i}]"))?;
        opts.section_skip.insert(id);
    }
    let result = od.evaluate_assembly(tab, &eff, &Default::default());
    let _gpu = crate::doc::gpu_lock();
    let off = ctx.offscreen()?;
    let up = off.renderer.uploader();
    opts.reflection = vo.upload_reflection(&up);
    let built = asm_scene::build(&result, &opts, &mut AsmGpuCache::default(), &up);
    let (lo, hi) = built.scene.bounds().map(|(l, h)| (l.as_dvec3(), h.as_dvec3())).unwrap_or((glam::DVec3::splat(-0.05), glam::DVec3::splat(0.05)));
    let aspect = width as f64 / height as f64;
    let mut camera = window.unwrap_or_else(|| noshape_scene::png::fit_camera(lo, hi, aspect, view));
    if window.is_none() {
        vo.finish_camera(&mut camera, lo, hi, aspect);
    }
    let style = vo.style();
    let png = off.render_png(&built.scene, &camera, &style, width, height).map_err(|e| OpError::new("render_failed", e))?;
    let mut v = json!({ "width": width, "height": height, "view": view_name, "bytes": png.len(), "instances": result.occurrences.iter().filter(|o| !o.hidden).count() });
    if let Some(f) = a.opt_str("file")? {
        std::fs::write(f, &png).map_err(|e| OpError::new("io", format!("writing {f}: {e}")).with_param("file"))?;
        v["file"] = json!(f);
    }
    Ok(OpOutput { value: v, images: vec![Image { png }] })
}

/// The call's arguments with the window's isolate / transparent and
/// boundary edges (view_display) for those it doesn't give; None when not
/// `view: "window"` or nothing to add.
fn window_display_args(ctx: &mut Ctx, a: &Args) -> Result<Option<serde_json::Value>, OpError> {
    if a.opt_str("view")? != Some("window") {
        return Ok(None);
    }
    let assembly = super::configuration::asm::is_assembly(ctx, a)?;
    let od = ctx.doc(a.opt_str("doc")?)?;
    let tab = if assembly {
        od.tab_of_kind(a.opt_str("tab")?, "Assembly", |c| matches!(c, noshape_model::document::TabContent::Assembly(_)))?
    } else {
        od.tab_of_kind(a.opt_str("tab")?, "Part Studio", crate::doc::is_part_studio)?
    };
    let Some(d) = od.window.display.get(&tab) else { return Ok(None) };
    let mut v = a.value.clone();
    let Some(m) = v.as_object_mut() else { return Ok(None) };
    if let Some(iso) = &d.isolate
        && !m.contains_key("isolate")
        && !m.contains_key("transparent")
    {
        m.insert(if iso.transparent { "transparent" } else { "isolate" }.into(), json!(iso.ids));
    }
    if !assembly && d.highlight_boundary && !m.contains_key("boundary_edges") {
        m.insert("boundary_edges".into(), json!(true));
    }
    Ok(Some(v))
}

/// `view: "window"`: the window's camera for the tab (view_camera).
fn window_camera(od: &crate::OpenDoc, tab: noshape_model::document::TabId, view: &str) -> Result<Option<noshape_render::Camera>, OpError> {
    if view != "window" {
        return Ok(None);
    }
    od.window.cameras.get(&tab).map(|v| Some(v.camera)).ok_or_else(|| OpError::param("view", "the window has no camera for this tab yet: move it with view_camera first"))
}

/// The 3D view's display options (view menu, analysis tools) shared by
/// Part Studios and assemblies.
struct ViewOptions {
    perspective: bool,
    tangent: builder::TangentEdges,
    hidden_edges: bool,
    high_quality: bool,
    zebra: Option<noshape_render::Zebra>,
    /// Environment, rotation (radians), blur.
    reflection: Option<(noshape_scene::environment_maps::EnvImage, f32, bool)>,
    /// Zebra stripes / Reflection analysis › Show edges.
    show_edges: bool,
}

fn view_options(a: &Args) -> Result<ViewOptions, OpError> {
    let tangent = match a.opt_str("tangent_edges")? {
        None | Some("visible") => builder::TangentEdges::Visible,
        Some("phantom") => builder::TangentEdges::Phantom,
        Some("hidden") => builder::TangentEdges::Removed,
        Some(_) => return Err(OpError::param("tangent_edges", "visible, phantom or hidden")),
    };
    let mut show_edges = true;
    let zebra = match a.object("zebra")? {
        None => None,
        Some(z) => {
            // Onshape's defaults: 35 stripes, not flipped, edges shown.
            let stripes = match z.get("stripes") {
                None => 35,
                Some(v) => v.as_u64().filter(|n| (1..=200).contains(n)).ok_or_else(|| OpError::param("zebra.stripes", "1 to 200"))? as u32,
            };
            show_edges = z.get("show_edges").and_then(|v| v.as_bool()).unwrap_or(true);
            Some(noshape_render::Zebra { stripes, flip: z.get("flip").and_then(|v| v.as_bool()).unwrap_or(false) })
        }
    };
    let reflection = match a.object("reflection")? {
        None => None,
        Some(_) if zebra.is_some() => return Err(OpError::param("reflection", "one of zebra or reflection")),
        Some(r) => {
            use noshape_scene::environment_maps as env;
            let image = match r.get("image").and_then(|v| v.as_str()) {
                None => env::generate(env::NAMES[0]).expect("library image"),
                Some(n) if env::NAMES.contains(&n) => env::generate(n).expect("library image"),
                Some(path) => {
                    let bytes = std::fs::read(path).map_err(|e| OpError::param("reflection.image", &format!("an environment map ({}) or an image file: {e}", env::NAMES.join(", "))))?;
                    env::decode(&bytes).ok_or_else(|| OpError::param("reflection.image", "not a PNG, JPEG, GIF or BMP image"))?
                }
            };
            let deg = r.get("rotation").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let sign = if r.get("opposite").and_then(|v| v.as_bool()).unwrap_or(false) { -1.0 } else { 1.0 };
            // Onshape's dialog starts with the edges hidden.
            show_edges = r.get("show_edges").and_then(|v| v.as_bool()).unwrap_or(false);
            Some((image, (deg.to_radians() * sign) as f32, r.get("blur").and_then(|v| v.as_bool()).unwrap_or(false)))
        }
    };
    Ok(ViewOptions { perspective: a.bool_or("perspective", false)?, tangent, hidden_edges: a.bool_or("hidden_edges", false)?, high_quality: a.bool_or("high_quality", false)?, zebra, reflection, show_edges })
}

impl ViewOptions {
    /// Zebra stripes and Reflection without Show edges hide the edges.
    fn hide_edges(&self) -> bool {
        (self.zebra.is_some() || self.reflection.is_some()) && !self.show_edges
    }

    fn display_mode(&self, mode: DisplayMode) -> DisplayMode {
        if self.hide_edges() && mode == DisplayMode::ShadedWithEdges { DisplayMode::Shaded } else { mode }
    }

    fn upload_reflection(&self, up: &noshape_render::renderer::Uploader) -> Option<noshape_render::Reflection> {
        let (img, rotation, blur) = self.reflection.as_ref()?;
        let environment = up.upload_image(&noshape_render::ImageData { corners: [[0.0; 3]; 4], width: img.width, height: img.height, rgba: img.rgba.clone() });
        Some(noshape_render::Reflection { environment, rotation: *rotation, blur: *blur, lock: false })
    }

    /// Perspective view: the same direction, framed again for perspective.
    fn finish_camera(&self, camera: &mut noshape_render::Camera, lo: glam::DVec3, hi: glam::DVec3, aspect: f64) {
        if self.perspective {
            camera.projection = noshape_render::Projection::Perspective;
            camera.fit_box(lo, hi, aspect);
        }
    }

    fn style(&self) -> noshape_render::Style {
        let mut style = noshape_scene::png::viewport_style(&crate::prefs::load_graphics());
        if self.high_quality {
            style.detail.mesh_error_px = noshape_render::scene::MESH_ERROR_PX_HIGH_QUALITY_STUDIO;
        }
        style
    }
}
