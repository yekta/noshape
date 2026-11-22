//! Mate connector glyphs as live's (behaviours "icons2 (10-26) plane and
//! mate connector hover"): hovered in the graphics or by its feature-list
//! row, or selected, a connector's wedge turns amber and its owner part's
//! edges the secondary yellow-green.

use super::PartStudioView;
use crate::scene_builder::{self, ConnectorLook};
use gpui_kit::*;
use noshape_model::BodyId;
use noshape_model::reference::Reference;
use noshape_render::Instance;

impl PartStudioView {
    /// A connector's look from the selection and the hover.
    fn connector_look(&self, r: &Reference, selection: &[Reference], cx: &App) -> ConnectorLook {
        // Its own dialog's connector stays pale, hovered or not (live mc-infer-1122).
        if matches!(r, Reference::MateConnector { feature, .. } if self.edit.as_ref().is_some_and(|e| e.dialog.read(cx).feature.id == *feature)) {
            return ConnectorLook::Placed;
        }
        if selection.contains(r) {
            return ConnectorLook::Selected;
        }
        let row = matches!(r, Reference::MateConnector { feature, .. } if self.ui.list_hover == Some(*feature));
        if row || self.hover.as_ref() == Some(r) { ConnectorLook::Hovered } else { ConnectorLook::Rest }
    }

    /// The connector's owner part: its Owner entity, else the part its
    /// origin entity lies on.
    pub(super) fn connector_owner(&self, r: &Reference, cx: &App) -> Option<BodyId> {
        let Reference::MateConnector { feature, .. } = r else { return None };
        let f = self.studio(cx)?.feature(*feature)?;
        if let Some(Reference::Body(b)) = f.params.query("ownerPart").first() {
            return Some(b.clone());
        }
        let origin = f.params.query("originQuery").first()?;
        let on = match origin {
            Reference::Implicit { on, .. } => on.as_ref(),
            o => o,
        };
        self.display.bodies.iter().find(|b| b.face_refs.contains(on) || b.edge_refs.contains(on) || b.vertex_refs.contains(on)).map(|b| b.id.clone())
    }

    /// Whether the connector's origin is a vertex (lit with it, live b3/ps-mc-*).
    fn connector_on_vertex(&self, r: &Reference, cx: &App) -> bool {
        let Reference::MateConnector { feature, .. } = r else { return false };
        self.studio(cx).and_then(|s| s.feature(*feature)).is_some_and(|f| matches!(f.params.query("originQuery").first(), Some(Reference::Vertex(_))))
    }

    /// The glyphs (per frame) and the lit connectors' owner-part overlays.
    pub(super) fn connector_overlays(&self, selection: &[Reference], cx: &App) -> (Option<std::sync::Arc<scene_builder::ConnectorGlyphs>>, Vec<Instance>) {
        let pick = |r: &Reference| self.built.as_ref().and_then(|b| b.picks.ids_for(r).first().copied()).unwrap_or(0);
        let glyphs = scene_builder::connector_glyphs(&self.display, |r| self.connector_look(r, selection, cx), pick);
        let lit: Vec<_> = self.display.connectors.iter().filter(|c| !matches!(self.connector_look(&c.reference, selection, cx), ConnectorLook::Rest | ConnectorLook::Placed)).collect();
        let up = self.gpu.uploader();
        let owners = lit.iter().filter_map(|c| self.connector_owner(&c.reference, cx)).filter_map(|b| scene_builder::owner_secondary(&self.display, &b, up));
        let origins = lit.iter().filter(|c| self.connector_on_vertex(&c.reference, cx)).map(|c| scene_builder::connector_origin_lit(c.origin, up));
        let owners: Vec<Instance> = owners.chain(origins).collect();
        (glyphs, owners)
    }
}
