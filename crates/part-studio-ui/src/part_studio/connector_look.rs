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
    fn connector_look(&self, r: &Reference, selection: &[Reference]) -> ConnectorLook {
        if selection.contains(r) {
            return ConnectorLook::Selected;
        }
        let row = matches!(r, Reference::MateConnector { feature, .. } if self.ui.list_hover == Some(*feature));
        if row || self.hover.as_ref() == Some(r) { ConnectorLook::Hovered } else { ConnectorLook::Rest }
    }

    /// The connector's owner part: its Owner entity, else the part its
    /// origin entity lies on.
    fn connector_owner(&self, r: &Reference, cx: &App) -> Option<BodyId> {
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

    /// A selected connector's owner part tinted yellow-green: its colour mixed 0.6 toward MATE_CONNECTOR_SECONDARY
    /// (242,255,128) before lighting (live "psui (2026-10-27, P2) interaction 3.4": the default blue's top face
    /// (142,187,214) becomes (186,214,166); hovered it stays as it is).
    pub(super) fn tint_selected_owners(&self, scene: &mut noshape_render::Scene, selection: &[Reference], cx: &App) {
        let owners: Vec<BodyId> = self
            .display
            .connectors
            .iter()
            .filter(|c| self.connector_look(&c.reference, selection) == ConnectorLook::Selected)
            .filter_map(|c| self.connector_owner(&c.reference, cx))
            .collect();
        if owners.is_empty() {
            return;
        }
        // Scene body indices: 1-based among the shown bodies, in display order (scene builder `build`).
        let shown: Vec<&BodyId> = self.display.bodies.iter().filter(|b| !b.hidden).map(|b| &b.id).collect();
        let to_srgb = |c: f32| if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
        let to_lin = |c: f32| if c <= 0.040_45 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
        const SECONDARY: [f32; 3] = [242.0 / 255.0, 1.0, 128.0 / 255.0];
        for b in owners {
            let Some(k) = shown.iter().position(|x| **x == b) else { continue };
            let body = k as u32 + 1;
            for i in scene.instances.iter_mut().filter(|i| i.body == body && matches!(i.drawable, noshape_render::Drawable::Mesh(_)) && !i.pick_only && !i.back_silhouette && i.color[3] >= 0.999) {
                for (c, s) in i.color.iter_mut().take(3).zip(SECONDARY) {
                    *c = to_lin(to_srgb(*c) * 0.4 + s * 0.6);
                }
            }
        }
    }

    /// The glyphs (per frame) and the lit connectors' owner-part overlays.
    pub(super) fn connector_overlays(&self, selection: &[Reference], cx: &App) -> (Option<std::sync::Arc<scene_builder::ConnectorGlyphs>>, Vec<Instance>) {
        let glyphs = scene_builder::connector_glyphs(&self.display, |r| self.connector_look(r, selection));
        let owners: Vec<Instance> = self
            .display
            .connectors
            .iter()
            .filter(|c| self.connector_look(&c.reference, selection) != ConnectorLook::Rest)
            .filter_map(|c| self.connector_owner(&c.reference, cx))
            .filter_map(|b| scene_builder::owner_secondary(&self.display, &b, self.gpu.uploader()))
            .collect();
        (glyphs, owners)
    }
}
