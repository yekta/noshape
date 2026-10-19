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
                if let Some(img) = crate::tab_manager::drawing_thumbnail(&doc, &versions, d, tab.name.clone()) {
                    out.push((key(item), img));
                }
                continue;
            }
            (TabContent::Assembly(a), ItemKind::Assembly, _) => {
                let result = evaluator.evaluate(&doc, a, &Default::default(), None);
                crate::assembly_scene::build(&result, &Default::default(), &mut Default::default(), gpu.uploader()).scene
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
        if let Some(img) = crate::tab_manager::decode(&png) {
            out.push((key(item), img));
        }
    }
    out
}
