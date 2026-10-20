//! Seeing the model: rendered images of a Part Studio from any view.

use super::{doc_field, s, studio_target, tab_field};
use crate::error::OpError;
use crate::registry::{Args, Image, Op, OpOutput, opt};
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
    vec![Op {
        group: "render",
        verb: "image",
        summary: "Render a Part Studio to a PNG image from a standard or named view",
        description: "Draws the parts, sketches, planes and origin as the 3D view does (Onshape's look: white background, shaded with edges), zoomed to fit. Returned as an image (MCP image content) and, with `file`, written to disk.",
        fields: vec![
            doc_field(),
            tab_field(),
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
    let opts = SceneOptions {
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
