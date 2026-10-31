//! Drawing editor: related-entity highlights (help drawing_dimensions "the
//! entities turn blue upon hover", note "hovering a note highlights the
//! edges its leaders touch in blue; selecting a grouped note highlights the
//! rest of its group in yellow", callout "selecting … highlights its
//! target"): the model edges a hovered or selected annotation references
//! are drawn in blue on top; the other members of a selected annotation's
//! group get the accent (yellow) state.

use crate::drawing_scene;
use crate::drawing_view::*;
use noshape_model::drawing::{AnnotationKind, PointRef, ViewId};
use noshape_model::drawing::build::PickTarget;
use noshape_model::drawing::views::ModelEdge;
use noshape_render::{Drawable, Instance, LineData, LineSegment, PointData, PointSprite, Scene, state};

/// The reference highlight color (live drawing/400–437: #90CEF1, about
/// 2 px wide, on hover and while selected).
const REFERENCE_BLUE: [u8; 4] = [0x90, 0xce, 0xf1, 255];

/// Whether model edge `me` is the edge point reference `r` lies on
/// (vertices are shown as points instead).
fn edge_matches(me: &ModelEdge, r: &PointRef) -> bool {
    match r {
        PointRef::Vertex { .. } => false,
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

    /// Sheet positions of the vertices annotation `id` references.
    pub fn reference_vertex_points(&self, id: u64) -> Vec<[f64; 2]> {
        let Some(b) = &self.built else { return Vec::new() };
        let Some(a) = b.resolved.annotations.iter().find(|a| a.id == id) else { return Vec::new() };
        let Some(view) = a.kind.view() else { return Vec::new() };
        let (Some(g), Some(pl)) = (self.geoms.get(&view), self.placement(view)) else { return Vec::new() };
        let mut out: Vec<[f64; 2]> = Vec::new();
        for r in a.refs.iter().filter(|r| matches!(r.point, PointRef::Vertex { .. })) {
            if let Some(p) = g.model_edges.iter().flat_map(|me| me.ends.iter().flatten()).find(|(p, _)| *p == r.point).map(|(_, at)| pl.to_sheet(*at))
                && !out.contains(&p)
            {
                out.push(p);
            }
        }
        out
    }

    /// The edges a note's leaders touch and their attach points (live
    /// drawing/436–437: the edge #90CEF1 with a dot where the leader ends);
    /// a callout's leader to the anchor: the dot (429–430).
    pub fn leader_touches(&self, id: u64) -> (Vec<u32>, Vec<[f64; 2]>) {
        use noshape_model::drawing::AnnotationKind;
        let (mut edges, mut dots) = (Vec::new(), Vec::new());
        let Some(b) = &self.built else { return (edges, dots) };
        // Resolved: the points follow the model as drawn.
        let Some(a) = b.resolved.annotations.iter().find(|a| a.id == id) else { return (edges, dots) };
        let ends: Vec<(ViewId, [f64; 2], bool)> = match &a.kind {
            AnnotationKind::Note { style, .. } => style.leaders.iter().filter_map(|l| Some((l.view?, l.point, true))).collect(),
            AnnotationKind::Balloon { view, anchor, callout, .. } if !callout.free => vec![(*view, *anchor, false)],
            _ => Vec::new(),
        };
        for (v, p, with_edge) in ends {
            let (Some(g), Some(pl)) = (self.geoms.get(&v), self.placement(v)) else { continue };
            dots.push(pl.to_sheet(p));
            if !with_edge {
                continue;
            }
            // Leaders snap onto the edge (view units, meters).
            let n0 = edges.len();
            for (i, e) in g.edges.iter().enumerate() {
                if e.points.windows(2).any(|w| noshape_scene::drawing_tools::seg_dist(p, w[0], w[1]) < 1e-5)
                    && let Some(k) = b.picks.iter().position(|t| *t == PickTarget::Edge { view: v, edge: i as u32 })
                {
                    edges.push(k as u32);
                }
            }
            // A leader inside a face region: the region's whole boundary,
            // its holes too (live drawing/427-428).
            if edges.len() == n0
                && let Some((outer, holes)) = self.pk().view_region_at(v, pl.to_sheet(p))
            {
                let loops: Vec<&Vec<[f64; 2]>> = std::iter::once(&outer).chain(holes.iter()).collect();
                // Loops are tessellated to 1e-5; the edges' points lie on the curves.
                let on_loop = |q: [f64; 2]| loops.iter().any(|l| (0..l.len()).any(|i| noshape_scene::drawing_tools::seg_dist(q, l[i], l[(i + 1) % l.len()]) < 2e-5));
                for (i, e) in g.edges.iter().enumerate() {
                    if e.visible
                        && e.points.iter().all(|q| on_loop(*q))
                        && let Some(k) = b.picks.iter().position(|t| *t == PickTarget::Edge { view: v, edge: i as u32 })
                    {
                        edges.push(k as u32);
                    }
                }
            }
        }
        (edges, dots)
    }

    /// Add the related-entity highlights of the hovered / selected
    /// annotations to the scene.
    pub fn push_related_highlights(&self, scene: &mut Scene) {
        let Some(b) = &self.built else { return };
        // The Hatch region dialog's closed profiles (live drawing/469).
        let outlines = self.hatch_profile_outlines();
        if !outlines.is_empty() {
            let mut lines = LineData::default();
            for l in &outlines {
                for w in l.windows(2) {
                    lines.segments.push(LineSegment { a: [w[0][0] as f32, w[0][1] as f32, 0.6], b: [w[1][0] as f32, w[1][1] as f32, 0.6], color: REFERENCE_BLUE, pick_id: 0, width: 4.0, style: LineSegment::ON_TOP });
                }
            }
            let up = self.gpu.uploader();
            scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().layer(18));
        }
        // A selected auxiliary view's edge in its parent (Onshape help).
        let aux = self.aux_edge_highlight();
        if !aux.is_empty() {
            let mut lines = LineData::default();
            let width = drawing_scene::px_width(noshape_model::drawing::sheet::LineStyle::Visible, self.zoom).max(2.0);
            for (a, c) in aux {
                lines.segments.push(LineSegment { a: [a[0] as f32, a[1] as f32, 0.6], b: [c[0] as f32, c[1] as f32, 0.6], color: REFERENCE_BLUE, pick_id: 0, width, style: LineSegment::ON_TOP });
            }
            let up = self.gpu.uploader();
            scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().layer(18));
        }
        let mut anns: Vec<u64> = self.selection.iter().filter_map(|s| if let Sel::Annotation(a) = s { Some(*a) } else { None }).collect();
        if let Some(Sel::Annotation(a)) = self.hover.and_then(|h| self.sel_of_pick(h)) {
            anns.push(a);
        }
        if anns.is_empty() && self.table_row_highlight().is_empty() {
            return;
        }
        let mut edges: Vec<u32> = anns.iter().flat_map(|a| self.reference_edge_picks(*a)).collect();
        let touches: Vec<(Vec<u32>, Vec<[f64; 2]>)> = anns.iter().map(|a| self.leader_touches(*a)).collect();
        edges.extend(touches.iter().flat_map(|(e, _)| e.iter().copied()));
        // A hovered BOM row's callouts (live drawing/421: blue).
        edges.extend(self.table_row_highlight());
        edges.sort_unstable();
        edges.dedup();
        if !edges.is_empty() {
            let mut lines = LineData::default();
            for l in b.geometry.lines.iter().filter(|l| edges.contains(&l.pick)) {
                let width = drawing_scene::px_width(l.style, self.zoom).max(2.0);
                for w in l.points.windows(2) {
                    lines.segments.push(LineSegment { a: [w[0][0] as f32, w[0][1] as f32, 0.6], b: [w[1][0] as f32, w[1][1] as f32, 0.6], color: REFERENCE_BLUE, pick_id: 0, width, style: LineSegment::ON_TOP });
                }
            }
            if !lines.segments.is_empty() {
                let up = self.gpu.uploader();
                scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().layer(18));
            }
        }
        let mut points: Vec<[f64; 2]> = anns.iter().flat_map(|a| self.reference_vertex_points(*a)).collect();
        points.extend(touches.iter().flat_map(|(_, d)| d.iter().copied()));
        if !points.is_empty() {
            let mut data = PointData::default();
            for p in points {
                data.points.push(PointSprite { position: [p[0] as f32, p[1] as f32, 0.65], color: REFERENCE_BLUE, pick_id: 0, size: 9.0, style: LineSegment::ON_TOP, _pad: 0 });
            }
            let up = self.gpu.uploader();
            scene.instances.push(Instance::new(Drawable::Points(up.upload_points(&data))).per_primitive_color().unpickable().layer(18));
        }
        // The rest of a hovered / selected annotation's group, and of a datum
        // target's series (A1 → A2, A3; help C10) (yellow).
        let sheet = self.current_sheet();
        let series = |a: &noshape_model::drawing::Annotation| match &a.kind {
            AnnotationKind::DatumTarget { label, .. } => Some(label.trim_end_matches(|c: char| c.is_ascii_digit()).to_string()).filter(|s| !s.is_empty()),
            _ => None,
        };
        for id in anns.iter().copied() {
            let Some(a) = sheet.annotations.iter().find(|a| a.id == id) else { continue };
            let (g, letter) = (a.ann_group, series(a));
            for other in sheet.annotations.iter().filter(|x| x.id != id && ((g.is_some() && x.ann_group == g) || (letter.is_some() && series(x) == letter))) {
                for p in self.picks_of(Sel::Annotation(other.id)) {
                    scene.add_flag(p, state::ACCENT);
                }
            }
        }
    }
}
