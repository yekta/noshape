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
        summary: "Render a Part Studio to a PNG image from a standard or named view",
        description: "Draws the parts, sketches, planes and origin as the 3D view does (Onshape's look: white background, shaded with edges), zoomed to fit. Returned as an image (MCP image content) and, with `file`, written to disk.",
        fields: vec![
            doc_field(),
            tab_field(),
            opt("section", json!({ "type": "array", "items": { "type": "object" }, "maxItems": 3 }), "Section view: up to 3 planes [{\"plane\": a plane / planar face / sketch / mate connector id, \"offset\": length along its normal (default 0), \"flip\": cut the other side}]; the side the normal points to is cut away. A named `view` keeps its own section unless this is given."),
            opt("section_exclude", json!({ "type": "array", "items": { "type": "string" } }), "Section view › Items to exclude: parts (`body:…`) left whole."),
            opt("view", json!({ "type": "string" }), "isometric (default), front, back, left, right, top, bottom, dimetric, trimetric, or the name of a saved named view."),
            opt("width", json!({ "type": "integer", "minimum": 16, "maximum": 4096 }), "Pixels (default 800)."),
            opt("height", json!({ "type": "integer", "minimum": 16, "maximum": 4096 }), "Pixels (default 600)."),
            opt("display_mode", json!({ "type": "string", "enum": ["shaded_with_edges", "shaded", "hidden_line", "translucent"] }), "Render style (default shaded_with_edges)."),
            opt("show_sketches", json!({ "type": "boolean", "default": true }), "Draw sketches."),
            opt("show_planes", json!({ "type": "boolean", "default": false }), "Draw the default and construction planes."),
            opt("file", s(), "Also write the PNG here."),
        ],
        mutates: false,
        destructive: false,
        run: image,
    }]
}

fn image(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let width = a.opt_u64("width")?.unwrap_or(800).clamp(16, 4096) as u32;
    let height = a.opt_u64("height")?.unwrap_or(600).clamp(16, 4096) as u32;
    let mode = match a.opt_str("display_mode")? {
        None | Some("shaded_with_edges") => DisplayMode::ShadedWithEdges,
        Some("shaded") => DisplayMode::Shaded,
        Some("hidden_line") => DisplayMode::HiddenLine,
        Some("translucent") => DisplayMode::Translucent,
        Some(_) => return Err(OpError::param("display_mode", "unknown display mode")),
    };
    let mut opts = SceneOptions {
        show_planes: a.bool_or("show_planes", false)?,
        show_sketches: a.bool_or("show_sketches", true)?,
        show_origin: a.bool_or("show_planes", false)?,
        display_mode: mode,
        ..Default::default()
    };
    let view_name = a.opt_str("view")?.unwrap_or("isometric").to_string();
    let (od, tab) = studio_target(ctx, a)?;
    let named = crate::doc::studio(od.doc(), tab)?.named_views.iter().find(|v| v.name.eq_ignore_ascii_case(&view_name)).cloned();
    let view = VIEWS.iter().find(|v| v.0 == view_name).map(|v| v.1);
    if view.is_none() && named.is_none() {
        return Err(OpError::param("view", "a standard view (isometric, front, top, ...) or a named view of this Part Studio"));
    }
    let r = od.regenerate(tab, None)?;
    // Section view: the planes given, else a named view's own.
    if a.get("section").is_some() {
        let (planes, skip) = section_planes(od, &r, a)?;
        opts.section = planes;
        opts.section_skip = skip;
    } else if let Some(v) = &named {
        opts.section = v.section.iter().map(|p| (glam::DVec3::new(p[0], p[1], p[2]), p[3])).collect();
    }
    let off = ctx.offscreen()?;
    let up = off.renderer.uploader();
    let built = builder::build(&r.display, &opts, &mut off.cache, &up);
    let (lo, hi) = built.scene.bounds().map(|(l, h)| (l.as_dvec3(), h.as_dvec3())).unwrap_or((glam::DVec3::splat(-0.05), glam::DVec3::splat(0.05)));
    let aspect = width as f64 / height as f64;
    let camera = match named {
        Some(v) => {
            let mut c = noshape_scene::png::fit_camera(lo, hi, aspect, None);
            c.rotation = glam::DQuat::from_xyzw(v.rotation[0], v.rotation[1], v.rotation[2], v.rotation[3]);
            c.target = glam::DVec3::from_array(v.target);
            c.distance = v.distance;
            c
        }
        None => noshape_scene::png::fit_camera(lo, hi, aspect, view),
    };
    let style = noshape_scene::png::viewport_style(&noshape_scene::GraphicsPrefs::default());
    let png = off.render_png(&built.scene, &camera, &style, width, height).map_err(|e| OpError::new("render_failed", e))?;
    let mut v = json!({ "width": width, "height": height, "view": view_name, "bytes": png.len() });
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
