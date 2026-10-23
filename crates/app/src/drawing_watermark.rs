//! Company settings › Release management › Watermarks on drawings (Help
//! enterprise_settings_release_management, image
//! release/example-drawing-watermark): the release state's text across the
//! middle of the sheet, light gray, rising along the sheet's diagonal.

use crate::doc_model::DocModel;
use noshape_model::document::TabId;
use noshape_model::drawing::sheet::{Anchor, Text2};
use noshape_model::release::{ItemKind, ItemState, PackageKind};

/// The help image's watermark gray.
pub const COLOR: u32 = 0xefefef;

/// The watermark of a drawing tab, if any: watermarks on, the drawing
/// revision managed (when "Hide watermark for drawings that are not revision
/// managed" is on) and its state's text not empty. In the workspace a
/// drawing is In progress, or Obsolete once its latest revision is; a
/// version that released it shows Released (Obsolete if obsoleted since).
pub fn text(doc: &DocModel, tab: TabId) -> Option<String> {
    let settings = crate::release_dialog::release_settings();
    if !settings.show_watermark || (settings.hide_watermark_unmanaged && crate::release_dialog::tab_not_revision_managed(doc, tab)) {
        return None;
    }
    let log = crate::release_dialog::release_log(doc);
    let pn = crate::release_dialog::tab_part_number(doc, tab);
    let obsolete = !pn.is_empty() && log.state(&pn) == ItemState::Obsolete;
    let marks = &settings.watermarks;
    let released_here = doc.session.viewing.and_then(|mv| {
        let version = doc.session.store.list_versions().ok()?.into_iter().find(|v| v.microversion == mv)?.id.to_string();
        log.at_version(&version).into_iter().find(|p| p.kind == PackageKind::Release && p.items.iter().any(|i| i.tab == tab && i.kind == ItemKind::Drawing && !i.reverted))
    });
    let text = match (released_here.is_some(), obsolete) {
        (_, true) => &marks.obsolete,
        (true, false) => &marks.released,
        (false, false) => &marks.in_progress,
    };
    (!text.trim().is_empty()).then(|| text.clone())
}

/// The watermark as sheet text: centered, its height 6% of the sheet's,
/// along the diagonal from the bottom left.
pub fn geometry(text: &str, width: f64, height: f64) -> Text2 {
    Text2 { pos: [width / 2.0, height / 2.0], height: height * 0.06, text: text.to_string(), angle: height.atan2(width), anchor: Anchor::Center, pick: u32::MAX }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_watermark_runs_along_the_diagonal() {
        let t = super::geometry("In progress", 279.4, 215.9);
        assert_eq!(t.pos, [139.7, 107.95]);
        assert!((t.angle.to_degrees() - 37.7).abs() < 0.1);
    }
}
