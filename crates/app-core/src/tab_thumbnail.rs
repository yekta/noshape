//! Tabs' pictures at the thumbnail size (moved from the app's `tab_manager`): a Part Studio's or an Assembly's
//! shaded render and a drawing's first sheet, as the Tab manager, the Feature Studio Import panel, version
//! properties and the release dialogs show them.

use gpui_kit::RenderImage;
use noshape_model::document::{TabContent, TabId};

/// A Part Studio's (or Assembly's) shaded picture at the thumbnail size, as
/// the Tab manager and the Feature Studio Import panel show them. Runs the
/// evaluation; call it off the UI thread.
pub async fn tab_thumbnail(
    doc: &noshape_model::Document,
    tab: TabId,
    assembly: bool,
    evaluator: &parking_lot::Mutex<noshape_model::assembly::eval::AssemblyEvaluator>,
    gpu: &crate::gpu::Gpu,
) -> Option<std::sync::Arc<RenderImage>> {
    let scene = if assembly {
        let Some(TabContent::Assembly(a)) = doc.tab(tab).map(|t| &t.content) else { return None };
        let result = evaluator.lock().evaluate(doc, a, &Default::default(), None);
        noshape_scene::asm_scene::build(&result, &Default::default(), &mut Default::default(), gpu.uploader()).scene
    } else {
        let result = evaluator.lock().studio(doc, tab, &Default::default())?;
        let opts = crate::scene_builder::SceneOptions { show_planes: false, show_sketches: false, show_origin: false, ..Default::default() };
        crate::scene_builder::build(&result.display, &opts, &mut Default::default(), gpu.uploader()).scene
    };
    let (lo, hi) = scene.bounds()?;
    let (w, h) = crate::gpu::THUMBNAIL_SIZE;
    let camera = crate::start_thumbnail::fit_camera(lo.as_dvec3(), hi.as_dvec3(), w as f64 / h as f64);
    let png = gpu.render_thumbnail(std::sync::Arc::new(scene), camera, crate::start_thumbnail::thumbnail_style()).await.ok()?;
    decode(&png)
}

/// PNG -> GPUI image (BGRA); the white background becomes transparent so the
/// pictures sit on selected rows and the gray preview.
pub fn decode(png: &[u8]) -> Option<std::sync::Arc<RenderImage>> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?;
    let mut rgba = img.to_rgba8();
    for px in rgba.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
        if px[0] >= 250 && px[1] >= 250 && px[2] >= 250 {
            *px = [0; 4];
        }
    }
    Some(std::sync::Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])))
}

/// A drawing's first sheet, rasterized (paper white, ink black) to the
/// thumbnail size: Onshape shows drawings as their sheet in the Tab manager.
/// `versions`: the documents at the versions pinned views show.
pub fn drawing_thumbnail(doc: &noshape_model::Document, versions: &std::collections::HashMap<String, noshape_model::Document>, drawing: &noshape_model::drawing::Drawing, name: String) -> Option<std::sync::Arc<RenderImage>> {
    use noshape_model::drawing::{build, font, sheet::LineStyle, views};
    use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};
    let sources = views::regenerate_sources_with(doc, drawing, &|id| versions.get(id).cloned());
    let info = build::SheetInfo { doc_name: String::new(), drawing_name: name, ..Default::default() };
    let g = build::render_sheet_with(drawing, 0, &sources, &views::ViewCache::new(), false, &info)?.geometry;
    let (w, h) = crate::gpu::THUMBNAIL_SIZE;
    let mut pm = Pixmap::new(w, h)?;
    // Fit the sheet with a small margin; sheet y points up.
    let k = ((w as f64 - 8.) / g.width).min((h as f64 - 8.) / g.height);
    let (ox, oy) = ((w as f64 - g.width * k) * 0.5, (h as f64 + g.height * k) * 0.5);
    let to = |p: [f64; 2]| ((ox + p[0] * k) as f32, (oy - p[1] * k) as f32);
    let mut paper = PathBuilder::new();
    let (x0, y0) = to([0., g.height]);
    paper.push_rect(tiny_skia::Rect::from_xywh(x0, y0, (g.width * k) as f32, (g.height * k) as f32)?);
    let paper = paper.finish()?;
    let mut white = Paint::default();
    white.set_color(Color::WHITE);
    pm.fill_path(&paper, &white, FillRule::Winding, Transform::identity(), None);
    let mut ink = Paint::default();
    ink.set_color(Color::BLACK);
    ink.anti_alias = true;
    let fill = |pts: &[[f64; 2]], pm: &mut Pixmap| {
        let mut pb = PathBuilder::new();
        for (i, p) in pts.iter().enumerate() {
            let (x, y) = to(*p);
            if i == 0 { pb.move_to(x, y) } else { pb.line_to(x, y) }
        }
        pb.close();
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &ink, FillRule::Winding, Transform::identity(), None);
        }
    };
    for f in &g.fills {
        fill(&f.points, &mut pm);
    }
    for t in &g.texts {
        if font::fill_supported(&t.text) {
            let (pts, idx) = font::text_fill(t);
            for tri in idx.chunks(3).filter(|c| c.len() == 3) {
                fill(&[pts[tri[0] as usize], pts[tri[1] as usize], pts[tri[2] as usize]], &mut pm);
            }
        }
    }
    for l in g.lines.iter().filter(|l| l.style != LineStyle::Hidden && l.points.len() >= 2) {
        let mut pb = PathBuilder::new();
        for (i, p) in l.points.iter().enumerate() {
            let (x, y) = to(*p);
            if i == 0 { pb.move_to(x, y) } else { pb.line_to(x, y) }
        }
        if l.closed {
            pb.close();
        }
        let Some(path) = pb.finish() else { continue };
        let stroke = Stroke { width: ((l.style.width_mm() * k) as f32).max(0.6), ..Default::default() };
        pm.stroke_path(&path, &ink, &stroke, Transform::identity(), None);
    }
    // Premultiplied RGBA → the BGRA frame GPUI draws.
    let mut rgba = image::RgbaImage::from_raw(w, h, pm.take())?;
    for px in rgba.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    Some(std::sync::Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])))
}
