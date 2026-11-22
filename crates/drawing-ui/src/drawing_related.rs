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
        PointRef::Vertex { .. } | PointRef::Thread { .. } => false,
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
        // A hole callout: every hole it counts, the circles of its size at their centres (live drw-hcallout-fit 12).
        let holes = match &a.kind {
            noshape_model::drawing::AnnotationKind::HoleCallout { radius, .. } => Some((*radius, noshape_model::drawing::build::hole_callout_centers(&self.drawing, self.current_sheet(), &self.sources, a))),
            _ => None,
        };
        let counted = |me: &ModelEdge| {
            let (Some((r, centers)), noshape_model::drawing::views::EdgeGeom::Circle { center, radius, .. }) = (&holes, &me.geom) else { return false };
            (radius - r).abs() <= r * 1e-3 + 1e-7 && centers.iter().any(|c| (c[0] - center[0]).hypot(c[1] - center[1]) <= r * 1e-3 + 1e-7)
        };
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
                (a.refs.iter().any(|r| edge_matches(me, &r.point)) || counted(me)).then_some(i as u32)
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
        // A callout lights the edge its leader is on too, not a face region's boundary (qa workflows-1123 10 s19-22).
        let ends: Vec<(ViewId, [f64; 2], bool)> = match &a.kind {
            AnnotationKind::Note { style, .. } => style.leaders.iter().filter_map(|l| Some((l.view?, l.point, true))).collect(),
            AnnotationKind::Balloon { view, anchor, callout, .. } if !callout.free => vec![(*view, *anchor, false)],
            _ => Vec::new(),
        };
        for (v, p, region) in ends {
            let (Some(g), Some(pl)) = (self.geoms.get(&v), self.placement(v)) else { continue };
            dots.push(pl.to_sheet(p));
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
            if region
                && edges.len() == n0
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
    /// A centre mark's looks (live2 drw-centermark): hovered (Select) or selected, its hole circle light blue at 3 px
    /// and 10 px orange grips at its arm ends with an 11 px blue dot at its centre; while its view is selected, the
    /// circle the view's orange and an orange dot at its centre, no grips (live2 drw-tools-1119 centermark-sel).
    pub(crate) fn centermark_looks(&self) -> CentermarkLooks {
        use noshape_model::drawing::build::centermark_key;
        let Some(b) = &self.built else { return CentermarkLooks::default() };
        let sheet = self.current_sheet();
        let mark_of = |s: &Sel| -> Option<(ViewId, [i64; 2])> {
            match s {
                Sel::AutoCentermark(v, k) => Some((*v, *k)),
                Sel::Annotation(id) => sheet.annotations.iter().find(|a| a.id == *id).and_then(|a| match &a.kind {
                    AnnotationKind::Centermark { view, center, .. } => Some((*view, centermark_key(*center))),
                    _ => None,
                }),
                _ => None,
            }
        };
        let hovered = self.hover.and_then(|h| self.sel_of_pick(h)).filter(|_| matches!(self.tool, Tool::Select));
        let lit: Vec<Sel> = self.selection.iter().copied().chain(hovered).filter(|s| mark_of(s).is_some()).collect();
        let views: Vec<ViewId> = self.selection.iter().filter_map(|s| if let Sel::View(v) = s { Some(*v) } else { None }).collect();
        let grips: Vec<u32> = lit.iter().flat_map(|s| self.picks_of(*s)).collect();
        let dots: Vec<u32> = b.picks.iter().enumerate().filter(|(i, t)| !grips.contains(&(*i as u32)) && match t {
            PickTarget::AutoCentermark { view, .. } => views.contains(view),
            PickTarget::Annotation(id) => sheet.annotations.iter().any(|a| a.id == *id && matches!(&a.kind, AnnotationKind::Centermark { view, .. } if views.contains(view))),
            _ => false,
        }).map(|(i, _)| i as u32).collect();
        let pk = self.pk();
        let circles = lit.iter().filter_map(mark_of).filter(|(v, _)| !views.contains(v)).flat_map(|(v, k)| pk.centermark_circle_picks(v, k)).collect();
        CentermarkLooks { grips, dots, circles }
    }

    pub fn push_centermark_looks(&self, scene: &mut Scene) {
        use noshape_model::drawing::sheet::GripKind;
        let Some(b) = &self.built else { return };
        let CentermarkLooks { grips: picks, dots, circles } = self.centermark_looks();
        if picks.is_empty() && dots.is_empty() && circles.is_empty() {
            return;
        }
        let up = self.gpu.uploader();
        let mut lines = LineData::default();
        for l in b.geometry.lines.iter().filter(|l| circles.contains(&l.pick)) {
            for w in l.points.windows(2) {
                lines.segments.push(LineSegment { a: [w[0][0] as f32, w[0][1] as f32, 0.6], b: [w[1][0] as f32, w[1][1] as f32, 0.6], color: REFERENCE_BLUE, pick_id: 0, width: LIT_LINE_PX, style: LineSegment::ON_TOP });
            }
        }
        if !lines.segments.is_empty() {
            scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().layer(18));
        }
        let mut grips = PointData::default();
        for gr in b.geometry.grips.iter().filter(|g| picks.contains(&g.pick) || (dots.contains(&g.pick) && g.kind == GripKind::Origin)) {
            let (color, size) = match gr.kind {
                GripKind::Origin if dots.contains(&gr.pick) => ([0xcc, 0x92, 0x5c, 255], 11.0),
                GripKind::Origin => (REFERENCE_BLUE, 11.0),
                _ => ([0xcc, 0x92, 0x5c, 255], 10.0),
            };
            grips.points.push(PointSprite { position: [gr.pos[0] as f32, gr.pos[1] as f32, 0.74], color, pick_id: 0, size, style: LineSegment::ON_TOP, _pad: 0 });
        }
        if !grips.points.is_empty() {
            scene.instances.push(Instance::new(Drawable::Points(up.upload_points(&grips))).per_primitive_color().unpickable().layer(23));
        }
    }

    /// A hole table's selected cells (live drw-hole-table-cell-panel 13): their holes' circles and centre marks light
    /// blue at 3 px, their tags on a light blue box.
    pub fn push_hole_cell_looks(&self, scene: &mut Scene) {
        use noshape_model::drawing::font;
        use noshape_render::MeshData;
        let Some(b) = &self.built else { return };
        let (picks, texts) = self.hole_cells_lit();
        if picks.is_empty() && texts.is_empty() {
            return;
        }
        let up = self.gpu.uploader();
        let mut lines = LineData::default();
        for l in b.geometry.lines.iter().filter(|l| picks.contains(&l.pick)) {
            for w in l.points.windows(2) {
                lines.segments.push(LineSegment { a: [w[0][0] as f32, w[0][1] as f32, 0.6], b: [w[1][0] as f32, w[1][1] as f32, 0.6], color: REFERENCE_BLUE, pick_id: 0, width: LIT_LINE_PX, style: LineSegment::ON_TOP });
            }
        }
        if !lines.segments.is_empty() {
            scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().layer(18));
        }
        let mut boxes = MeshData::default();
        for t in texts.iter().filter_map(|i| b.geometry.texts.get(*i)) {
            let base = boxes.positions.len() as u32;
            for p in font::text_box(t) {
                boxes.positions.push([p[0] as f32, p[1] as f32, -0.5]);
                boxes.normals.push([0.0, 0.0, 1.0]);
                boxes.pick_ids.push(0);
            }
            boxes.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        if !boxes.indices.is_empty() {
            scene.instances.push(Instance::new(Drawable::Mesh(up.upload_mesh(&boxes))).with_color(crate::theme::lin(0x90cef1)).unlit().unpickable().layer(-4));
        }
    }

    /// A hovered or selected section or detail view's marks in its parent, light blue: the cutting line (`lines`, `ink`
    /// over its arrows and letters) or the detail circle and its leader (live asbv-14/15, qa workflows-1123 08 s4-6, 09
    /// s16-21).
    pub(crate) fn push_related_marks(&self, scene: &mut Scene, lines: &[u32], ink: &noshape_render::MeshData) {
        let Some(b) = &self.built else { return };
        let details = self.detail_marks_lit();
        let mut picks = lines.to_vec();
        picks.extend(b.picks.iter().enumerate().filter(|(_, t)| matches!(t, PickTarget::DetailCircle(v) if details.contains(v))).map(|(i, _)| i as u32));
        let up = self.gpu.uploader();
        let mut data = LineData::default();
        for (style, piece) in self.tinted_pieces(b, &picks, 0x90cef1) {
            let width = drawing_scene::px_width(style, self.zoom).max(LIT_LINE_PX);
            for w in piece.windows(2) {
                data.segments.push(LineSegment { a: [w[0][0] as f32, w[0][1] as f32, 0.6], b: [w[1][0] as f32, w[1][1] as f32, 0.6], color: REFERENCE_BLUE, pick_id: 0, width, style: LineSegment::ON_TOP });
            }
        }
        if !data.segments.is_empty() {
            scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&data))).per_primitive_color().unpickable().layer(18));
        }
        if !ink.indices.is_empty() {
            scene.instances.push(Instance::new(Drawable::Mesh(up.upload_mesh(ink))).with_color(crate::theme::lin(0x90cef1)).unlit().unpickable().layer(19));
        }
    }

    pub fn push_related_highlights(&self, scene: &mut Scene) {
        let Some(b) = &self.built else { return };
        // The selected hatch regions: a yellow outline with a pale halo (live request 1682, "the yellow surround"); the
        // Hatch region dialog's picked profiles have none (live2 drw-tools-1119 hc.png).
        let mut outlines: Vec<Vec<[f64; 2]>> = Vec::new();
        let sheet = self.current_sheet();
        for h in self.selection.iter().filter_map(|s| if let Sel::Hatch(id) = s { sheet.sketch.hatches.iter().find(|h| h.id == *id) } else { None }) {
            let pl = h.view.and_then(|v| self.placement(v));
            for l in std::iter::once(&h.boundary).chain(h.holes.iter()) {
                let mut v: Vec<[f64; 2]> = l.iter().map(|p| pl.as_ref().map_or(*p, |pl| pl.to_sheet(*p))).collect();
                if let Some(f) = v.first().copied() {
                    v.push(f);
                }
                outlines.push(v);
            }
        }
        if !outlines.is_empty() {
            const CORE: [u8; 4] = [235, 235, 146, 255];
            const HALO: [u8; 4] = [245, 245, 201, 255];
            let mut lines = LineData::default();
            for (color, width) in [(HALO, 6.0), (CORE, 2.0)] {
                for l in &outlines {
                    for w in l.windows(2) {
                        lines.segments.push(LineSegment { a: [w[0][0] as f32, w[0][1] as f32, 0.6], b: [w[1][0] as f32, w[1][1] as f32, 0.6], color, pick_id: 0, width, style: LineSegment::ON_TOP });
                    }
                }
            }
            let up = self.gpu.uploader();
            scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().layer(18));
        }
        // A selected auxiliary view's edge in its parent (Onshape help).
        let aux = self.aux_edge_highlight();
        if !aux.is_empty() {
            let mut lines = LineData::default();
            let width = drawing_scene::px_width(noshape_model::drawing::sheet::LineStyle::Visible, self.zoom).max(LIT_LINE_PX);
            for (a, c) in aux {
                lines.segments.push(LineSegment { a: [a[0] as f32, a[1] as f32, 0.6], b: [c[0] as f32, c[1] as f32, 0.6], color: REFERENCE_BLUE, pick_id: 0, width, style: LineSegment::ON_TOP });
            }
            let up = self.gpu.uploader();
            scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().layer(18));
        }
        let dragged = self.dragging_annotation_id();
        let mut anns: Vec<u64> = self.selection.iter().filter_map(|s| if let Sel::Annotation(a) = s { Some(*a) } else { None }).filter(|a| Some(*a) != dragged).collect();
        // A dimension just placed with the tool still on: no blue, and no dots on an edge's ends (qa workflow 09 s10; a
        // 2 point one keeps its points' discs, dim_kept_points).
        if matches!(self.tool, Tool::Dimension { .. }) {
            anns.clear();
        }
        if let Some(Sel::Annotation(a)) = self.hover.and_then(|h| self.sel_of_pick(h)).filter(|s| dragged.is_none_or(|d| *s != Sel::Annotation(d))) {
            anns.push(a);
        }
        anns.extend(self.placed_callout_lit());
        let (row_callouts, row_geometry) = self.table_row_highlight();
        if anns.is_empty() && row_callouts.is_empty() && row_geometry.is_empty() {
            return;
        }
        // (A centre mark's circle: push_centermark_looks.)
        let mark = |id: &u64| sheet.annotations.iter().any(|a| a.id == *id && matches!(a.kind, AnnotationKind::Centermark { .. }));
        let mut edges: Vec<u32> = anns.iter().filter(|a| !mark(a)).flat_map(|a| self.reference_edge_picks(*a)).collect();
        let touches: Vec<(Vec<u32>, Vec<[f64; 2]>)> = anns.iter().map(|a| self.leader_touches(*a)).collect();
        edges.extend(touches.iter().flat_map(|(e, _)| e.iter().copied()));
        // A hovered BOM row's callouts (live drawing/421: blue).
        edges.extend(row_callouts);
        edges.sort_unstable();
        edges.dedup();
        // ... and its part's edges in the views tan (live drawing/421; a cut
        // list row's members, live 1647; a hole table row's circles).
        if !row_geometry.is_empty() {
            const TAN: [u8; 4] = [0xcc, 0xb2, 0x92, 255];
            let mut lines = LineData::default();
            for l in b.geometry.lines.iter().filter(|l| row_geometry.contains(&l.pick)) {
                let width = drawing_scene::px_width(l.style, self.zoom).max(LIT_LINE_PX);
                for w in l.points.windows(2) {
                    lines.segments.push(LineSegment { a: [w[0][0] as f32, w[0][1] as f32, 0.6], b: [w[1][0] as f32, w[1][1] as f32, 0.6], color: TAN, pick_id: 0, width, style: LineSegment::ON_TOP });
                }
            }
            if !lines.segments.is_empty() {
                let up = self.gpu.uploader();
                scene.instances.push(Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().layer(18));
            }
        }
        if !edges.is_empty() {
            let mut lines = LineData::default();
            for l in b.geometry.lines.iter().filter(|l| edges.contains(&l.pick)) {
                let width = drawing_scene::px_width(l.style, self.zoom).max(LIT_LINE_PX);
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

    /// Placing with the Dimension tool: the dimension being placed and the one just placed (selected) draw their text
    /// bold (live2 drw-tools-1119: p2p, dimg).
    pub(crate) fn bold_placed(&self, built: &mut noshape_model::drawing::build::BuiltSheet, shown: &noshape_model::drawing::Drawing, si: usize) {
        use noshape_model::drawing::sheet::{FontSpan, TextFont, DRAWING_DEFAULT_FONT};
        if !matches!(self.tool, Tool::Dimension { .. }) {
            return;
        }
        let committed = self.drawing.sheets.get(si);
        let new = shown.sheets.get(si).into_iter().flat_map(|s| s.annotations.iter()).filter(|a| committed.is_none_or(|c| !c.annotations.iter().any(|x| x.id == a.id))).map(|a| a.id);
        let ids: Vec<u64> = self.selection.iter().filter_map(|s| if let Sel::Annotation(a) = s { Some(*a) } else { None }).chain(new).collect();
        if ids.is_empty() {
            return;
        }
        let g = &mut built.geometry;
        let spans: Vec<FontSpan> = (0..g.texts.len())
            .filter(|&i| matches!(built.picks.get(g.texts[i].pick as usize), Some(PickTarget::Annotation(id)) if ids.contains(id)))
            .map(|i| {
                let font = g.text_font(i).cloned().unwrap_or_else(|| TextFont { family: DRAWING_DEFAULT_FONT.into(), bold: false, italic: false });
                FontSpan { texts: i..i + 1, font: TextFont { bold: true, ..font } }
            })
            .collect();
        g.fonts.extend(spans);
    }
}

/// [`DrawingEditor::centermark_looks`]: the picks whose grips show (hovered or selected marks), those showing only
/// their centre dot (their view selected), and the circles lit light blue.
#[derive(Default, Debug, PartialEq)]
pub(crate) struct CentermarkLooks {
    pub grips: Vec<u32>,
    pub dots: Vec<u32>,
    pub circles: Vec<u32>,
}
