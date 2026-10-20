//! Drawing editor: related-entity highlights (help drawing_dimensions "the
//! entities turn blue upon hover", note "hovering a note highlights the
//! edges its leaders touch in blue; selecting a grouped note highlights the
//! rest of its group in yellow", callout "selecting … highlights its
//! target"): the model edges a hovered or selected annotation references
//! are drawn in blue on top; the other members of a selected annotation's
//! group get the accent (yellow) state.

use crate::drawing_scene;
use crate::drawing_view::*;
use noshape_model::drawing::PointRef;
use noshape_model::drawing::build::PickTarget;
use noshape_model::drawing::views::ModelEdge;
use noshape_render::{Drawable, Instance, LineData, LineSegment, Scene, state};

/// The reference highlight color. Provisional: the exact drawing blue is
/// not captured yet (REQUESTS.md asb 10-20 e).
const REFERENCE_BLUE: [u8; 4] = [0x1e, 0x6f, 0xd9, 255];

/// Whether model edge `me` is (or ends at) point reference `r`.
fn edge_matches(me: &ModelEdge, r: &PointRef) -> bool {
    match r {
        PointRef::Vertex { .. } => me.ends.iter().flatten().any(|(p, _)| p == r),
        PointRef::Center { body, name } | PointRef::Midpoint { body, name } | PointRef::Quadrant { body, name, .. } => me.body == *body && *me.name == *name,
        PointRef::Intersection { body, name, body2, name2 } => (me.body == *body && *me.name == *name) || (me.body == *body2 && *me.name == *name2),
    }
}

impl DrawingEditor {
    /// Pick ids of the projected model edges annotation `id` references.
    pub fn reference_edge_picks(&self, id: u64) -> Vec<u32> {
        let Some(b) = &self.built else { return Vec::new() };
        let Some(a) = b.resolved.annotations.iter().find(|a| a.id == id) else { return Vec::new() };
        if a.refs.is_empty() {
            return Vec::new();
        }
        let view = a.kind.view();
        b.picks
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                let PickTarget::Edge { view: v, edge } = t else { return None };
                if view.is_some_and(|x| x != *v) {
                    return None;
                }
                let g = self.geoms.get(v)?;
                let me = g.model_edges.get(g.edges.get(*edge as usize)?.model? as usize)?;
                a.refs.iter().any(|r| edge_matches(me, &r.point)).then_some(i as u32)
            })
            .collect()
    }

    /// Add the related-entity highlights of the hovered / selected
    /// annotations to the scene.
    pub fn push_related_highlights(&self, scene: &mut Scene) {
        let Some(b) = &self.built else { return };
        let mut anns: Vec<u64> = self.selection.iter().filter_map(|s| if let Sel::Annotation(a) = s { Some(*a) } else { None }).collect();
        if let Some(Sel::Annotation(a)) = self.hover.and_then(|h| self.sel_of_pick(h)) {
            anns.push(a);
        }
        if anns.is_empty() {
            return;
        }
        let mut edges: Vec<u32> = anns.iter().flat_map(|a| self.reference_edge_picks(*a)).collect();
        edges.sort_unstable();
        edges.dedup();
        if !edges.is_empty() {
            let mut lines = LineData::default();
            for l in b.geometry.lines.iter().filter(|l| edges.contains(&l.pick)) {
                let width = drawing_scene::px_width(l.style, self.zoom).max(1.5);
                for w in l.points.windows(2) {
                    lines.segments.push(LineSegment { a: [w[0][0] as f32, w[0][1] as f32, 0.6], b: [w[1][0] as f32, w[1][1] as f32, 0.6], color: REFERENCE_BLUE, pick_id: 0, width, style: LineSegment::ON_TOP });
                }
            }
            if !lines.segments.is_empty() {
                let up = self.gpu.uploader();
                scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().layer(18));
            }
        }
        // The rest of a selected annotation's group (yellow).
        let sheet = self.current_sheet();
        for id in self.selection.iter().filter_map(|s| if let Sel::Annotation(a) = s { Some(*a) } else { None }) {
            let Some(g) = sheet.annotations.iter().find(|a| a.id == id).and_then(|a| a.ann_group) else { continue };
            for other in sheet.annotations.iter().filter(|x| x.ann_group == Some(g) && x.id != id) {
                for p in self.picks_of(Sel::Annotation(other.id)) {
                    scene.add_flag(p, state::ACCENT);
                }
            }
        }
    }
}
