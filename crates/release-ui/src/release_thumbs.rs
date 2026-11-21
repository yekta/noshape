//! Release dialog pictures (Help images savedraftbutton-relcandidatedialog,
//! add-item-to-release-01: each item row shows a picture of its part,
//! assembly or drawing). Rendered offscreen like the Tab manager's; they
//! are taken when the items are listed and only change on Refresh
//! (Help: "notice the thumbnail of the part still has no fillets. Click
//! the Refresh icon to update the part").

use gpui_kit::*;
use noshape_model::assembly::eval::AssemblyEvaluator;
use noshape_model::document::TabContent;
use noshape_model::release::{ItemKind, ReleaseItem};
use noshape_model::Document;
use std::sync::Arc;

/// A row's picture key: tab, part, configuration and the version a linked
/// item comes from.
pub fn key(item: &ReleaseItem) -> String {
    let version = item.document.as_ref().map(|d| format!("{}@{}", d.path, d.version)).unwrap_or_default();
    format!("{}/{}/{}/{version}", item.tab.0, item.part.as_ref().map(|b| b.to_string()).unwrap_or_default(), serde_json::to_string(&item.config).unwrap_or_default())
}

/// Render the pictures of `items` (in the background).
/// `versions`: the documents at the versions pinned drawing views show.
pub async fn render(doc: Document, versions: std::collections::HashMap<String, Document>, items: Vec<ReleaseItem>, gpu: crate::gpu::Gpu) -> Vec<(String, Arc<RenderImage>)> {
    let mut evaluator = AssemblyEvaluator::default();
    let mut out = Vec::new();
    for item in &items {
        let Some(tab) = doc.tab(item.tab) else { continue };
        let scene = match (&tab.content, item.kind, &item.part) {
            (TabContent::Drawing(d), ItemKind::Drawing, _) => {
                if let Some(img) = crate::tab_thumbnail::drawing_thumbnail(&doc, &versions, d, tab.name.clone()) {
                    out.push((key(item), img));
                }
                continue;
            }
            (TabContent::Assembly(a), ItemKind::Assembly, _) => {
                let result = evaluator.evaluate(&doc, a, &Default::default(), None);
                noshape_scene::asm_scene::build(&result, &Default::default(), &mut Default::default(), gpu.uploader()).scene
            }
            // A part, or with no part the whole Part Studio (Add Items tree).
            (TabContent::PartStudio(_), ItemKind::Part, part) => {
                let Some(result) = evaluator.studio(&doc, item.tab, &item.config) else { continue };
                let mut display = result.display.clone();
                if let Some(part) = part {
                    for b in &mut display.bodies {
                        b.hidden = b.id != *part;
                    }
                }
                let opts = crate::scene_builder::SceneOptions { show_planes: false, show_sketches: false, show_origin: false, ..Default::default() };
                crate::scene_builder::build(&display, &opts, &mut Default::default(), gpu.uploader()).scene
            }
            _ => continue,
        };
        let Some((lo, hi)) = scene.bounds() else { continue };
        let (w, h) = crate::gpu::THUMBNAIL_SIZE;
        let camera = crate::start_thumbnail::fit_camera(lo.as_dvec3(), hi.as_dvec3(), w as f64 / h as f64);
        let Ok(png) = gpu.render_thumbnail(Arc::new(scene), camera, crate::start_thumbnail::thumbnail_style()).await else { continue };
        if let Some(img) = crate::tab_thumbnail::decode(&png) {
            out.push((key(item), img));
        }
    }
    out
}

/// Compare revisions' Revision A (the released one) and Revision B (live docver-1121: Only show differences draws the
/// differing geometry flat, A light blue, B red-orange).
pub const COMPARE_A: u32 = 0xacdbf5;
pub const COMPARE_B: u32 = 0xe67d5e;

/// Compare revisions' pictures of one item, A then B: as they look, and with Only show differences.
#[derive(Clone, Default)]
pub struct Compared {
    pub plain: [Option<Arc<RenderImage>>; 2],
    pub differences: [Option<Arc<RenderImage>>; 2],
}

/// Render part `item` in `docs` (the documents at revision A and at B; None: nothing to show) as `size` px squares.
/// The differences pictures are drawn unlit: in [`COMPARE_A`] / [`COMPARE_B`] when its shape differs, else white.
pub async fn compare(docs: [Option<Document>; 2], item: ReleaseItem, size: u32, gpu: crate::gpu::Gpu) -> Compared {
    let part_display = |doc: &Document| -> Option<noshape_model::display::StudioDisplay> {
        let tab = doc.tab(item.tab)?;
        if !matches!((&tab.content, item.kind), (TabContent::PartStudio(_), ItemKind::Part)) {
            return None;
        }
        let result = AssemblyEvaluator::default().studio(doc, item.tab, &item.config)?;
        let mut display = result.display.clone();
        for b in &mut display.bodies {
            b.hidden = item.part.as_ref().is_some_and(|p| b.id != *p);
        }
        Some(display)
    };
    let shapes: Vec<Option<noshape_model::display::StudioDisplay>> = docs.iter().map(|d| d.as_ref().and_then(part_display)).collect();
    let shown = |d: &noshape_model::display::StudioDisplay| d.bodies.iter().filter(|b| !b.hidden).map(|b| b.mesh.positions.clone()).collect::<Vec<_>>();
    let differs = match (&shapes[0], &shapes[1]) {
        (Some(a), Some(b)) => shown(a) != shown(b),
        _ => true,
    };
    let mut out = Compared::default();
    for (side, display) in shapes.iter().enumerate() {
        let Some(display) = display else { continue };
        let opts = crate::scene_builder::SceneOptions { show_planes: false, show_sketches: false, show_origin: false, ..Default::default() };
        let picture = |display: &noshape_model::display::StudioDisplay, flat: Option<u32>| {
            let mut display = display.clone();
            if let Some(hex) = flat {
                for b in &mut display.bodies {
                    b.color = crate::theme::lin(hex);
                    b.face_colors.clear();
                }
            }
            let mut scene = crate::scene_builder::build(&display, &opts, &mut Default::default(), gpu.uploader()).scene;
            if flat.is_some() {
                // Lines and points are drawn unlit anyway.
                for i in &mut scene.instances {
                    i.unlit = true;
                }
            }
            scene
        };
        // Live rel-1121b: a changed part is filled whole, the older side light blue, the newer red-orange; one whose
        // properties alone changed becomes a plain white line drawing on both sides.
        let flat = Some(match (differs, side) {
            (false, _) => 0xffffff,
            (true, 0) => COMPARE_A,
            (true, _) => COMPARE_B,
        });
        for (scene, slot) in [(picture(display, None), &mut out.plain[side]), (picture(display, flat), &mut out.differences[side])] {
            let Some((lo, hi)) = scene.bounds() else { continue };
            let camera = crate::start_thumbnail::fit_camera(lo.as_dvec3(), hi.as_dvec3(), 1.0);
            let Ok(png) = gpu.render_png(Arc::new(scene), camera, crate::start_thumbnail::thumbnail_style(), size, size).await else { continue };
            *slot = crate::tab_thumbnail::decode(&png);
        }
    }
    out
}
