//! Drawing editor: view labels as items of their own (live behaviours
//! "Auxiliary view reference indicator and view labels" (4), asb/u-940…953).
//! Hovered, a label shows a small orange square at each end; a click selects
//! the label alone (an orange box behind the black text, white squares
//! outlined in orange), and a drag moves it freely (`DrawingView.label_at`).
//! Its right-click menu is the view's with Copy first and without the
//! replication items. Ctrl+C / Ctrl+V and Alt+drag make a plain Note of its
//! text (the Note defaults), centred on the cursor with its bottom there.

use crate::drawing_view::*;
use gpui_kit::{App, ClipboardItem, Context};
use noshape_model::drawing::build::PickTarget;
use noshape_model::drawing::clipboard::ViewClipboard;
use noshape_model::drawing::sheet::Text2;
use noshape_model::drawing::{Annotation, AnnotationKind, ViewId, font, note};
use noshape_render::scene::Scene;

fn sub2(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

/// Live's label highlight (u-941: rgb 204,146,92).
const ORANGE: [u8; 4] = [0xcc, 0x92, 0x5c, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

impl DrawingEditor {
    /// The label lines under view `v`, top first.
    fn label_texts(&self, v: ViewId) -> Vec<&Text2> {
        let Some(b) = &self.built else { return vec![] };
        b.geometry.texts.iter().filter(|t| matches!(b.picks.get(t.pick as usize), Some(PickTarget::ViewLabel(x)) if *x == v)).collect()
    }

    /// The box around view `v`'s label (x0, y0, x1, y1).
    pub fn label_box(&self, v: ViewId) -> Option<[f64; 4]> {
        let corners: Vec<[f64; 2]> = self.label_texts(v).into_iter().flat_map(font::text_box).collect();
        let first = corners.first()?;
        Some(corners.iter().fold([first[0], first[1], first[0], first[1]], |b, p| [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])]))
    }

    /// A Note with view `v`'s label text and the Note defaults, its bottom
    /// centre at `at` (sheet mm).
    pub fn label_note(&self, v: ViewId, at: [f64; 2]) -> Option<Annotation> {
        let text = self.label_texts(v).iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join("\n");
        if text.is_empty() {
            return None;
        }
        let props = &self.drawing.props;
        let h = props.ann_type(noshape_model::drawing::sheet::AnnType::Notes).height.unwrap_or(props.ann_text_height);
        let style = note::NoteStyle::default();
        let b = note::layout(&text, h, &style, [0.0, 0.0]).bbox;
        let position = [at[0] - (b[0] + b[2]) / 2.0, at[1] - b[1]];
        Some(Annotation { id: self.current_sheet().next_annotation_id(), kind: AnnotationKind::Note { text, height_mm: h, style }, position, ..Default::default() })
    }

    /// The selected labels' views.
    fn selected_labels(&self) -> Vec<ViewId> {
        self.selection.iter().filter_map(|s| if let Sel::ViewLabel(v) = s { Some(*v) } else { None }).collect()
    }

    /// Ctrl+C with labels selected: their text as Notes. True when used.
    pub fn copy_labels(&mut self, cx: &mut Context<Self>) -> bool {
        let notes: Vec<Annotation> = self.selected_labels().into_iter().filter_map(|v| self.label_note(v, [0.0, 0.0])).collect();
        if notes.is_empty() {
            return false;
        }
        let mut sheet = self.current_sheet().clone();
        sheet.views.clear();
        sheet.tables.clear();
        sheet.annotations = notes;
        let clip = ViewClipboard { noshape_drawing_views: 1, sheet, anchor: Some([0.0, 0.0]), origin: None };
        cx.write_to_clipboard(ClipboardItem::new_string(clip.to_json()));
        true
    }

    /// Alt+drag on a label: a Note copy follows the cursor and drops at the
    /// release point; the label stays.
    pub fn alt_copy_label(&mut self, pick: Option<u32>, start: [f64; 2], cx: &mut Context<Self>) -> Option<Drag> {
        let Some(PickTarget::ViewLabel(v)) = self.built.as_ref()?.picks.get(pick? as usize).cloned() else { return None };
        let b = self.label_box(v)?;
        let a = self.label_note(v, [(b[0] + b[2]) / 2.0, b[1]])?;
        let id = a.id;
        let orig = a.clone();
        self.edit_sheet("Copy view label", cx, |s| s.annotations.push(a));
        self.selection = vec![Sel::Annotation(id)];
        Some(Drag::MoveAnnotation { start, orig })
    }

    /// A drag on a label moves it (the view stays).
    pub fn label_drag(&self, v: ViewId, start: [f64; 2]) -> Drag {
        let orig = self.current_sheet().view(v).and_then(|x| x.label_at);
        Drag::MoveViewLabel { view: v, start, orig }
    }

    /// The label's right-click menu: Copy, then the view's menu without its
    /// Copy and the replication items (live u-944).
    pub fn label_entries(&self, v: ViewId, cx: &App) -> Vec<MenuEntry> {
        let mut out = vec![MenuEntry::item("Copy", |t, _, cx| {
            t.copy_labels(cx);
        })];
        out.extend(self.onshape_view_entries(v, cx).into_iter().filter(|e| match e {
            MenuEntry::Item { label, .. } | MenuEntry::Submenu(label, _) => label.as_ref() != "Copy" && !label.starts_with("Select for replication") && !label.starts_with("Replicate annotations"),
            MenuEntry::Separator => true,
        }));
        out
    }

    /// The hovered / selected / dragged labels' look, over the sheet.
    pub fn push_label_highlights(&self, scene: &mut Scene) {
        use noshape_render::{Drawable, Instance, LineSegment, MeshData, PointData, PointSprite};
        let hovered = self.hover.and_then(|h| match self.built.as_ref()?.picks.get(h as usize) {
            Some(PickTarget::ViewLabel(v)) => Some(*v),
            _ => None,
        });
        let dragged = if let Drag::MoveViewLabel { view, .. } = &self.drag { Some(*view) } else { None };
        let selected = self.selected_labels();
        let mut boxes = MeshData::default();
        let mut points = PointData::default();
        let mut grip = |p: [f64; 2], fill: [u8; 4]| {
            points.points.push(PointSprite { position: [p[0] as f32, p[1] as f32, 0.7], color: ORANGE, pick_id: 0, size: 9.0, style: LineSegment::ON_TOP, _pad: 0 });
            if fill != ORANGE {
                points.points.push(PointSprite { position: [p[0] as f32, p[1] as f32, 0.71], color: fill, pick_id: 0, size: 6.0, style: LineSegment::ON_TOP, _pad: 0 });
            }
        };
        let views: Vec<ViewId> = selected.iter().copied().chain(hovered).chain(dragged).collect();
        for (i, v) in views.iter().enumerate() {
            if views[..i].contains(v) {
                continue;
            }
            let Some(b) = self.label_box(*v) else { continue };
            let mid = (b[1] + b[3]) / 2.0;
            let on = selected.contains(v) || dragged == Some(*v);
            // While dragging, the grips stay where the label was (u-945).
            let back = match &self.drag {
                Drag::MoveViewLabel { view, start, .. } if view == v => sub2(*start, self.cursor),
                _ => [0.0, 0.0],
            };
            let ends = [[b[0] + back[0], mid + back[1]], [b[2] + back[0], mid + back[1]]];
            for e in ends {
                grip(e, if on { WHITE } else { ORANGE });
            }
            if on {
                let base = boxes.positions.len() as u32;
                for p in [[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]]] {
                    boxes.positions.push([p[0] as f32, p[1] as f32, -0.5]);
                    boxes.normals.push([0.0, 0.0, 1.0]);
                    boxes.pick_ids.push(0);
                }
                boxes.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
        let up = self.gpu.uploader();
        if !boxes.indices.is_empty() {
            scene.instances.push(Instance::new(Drawable::Mesh(up.upload_mesh(&boxes))).with_color(crate::theme::lin(0xcc925c)).unlit().unpickable().layer(-4));
        }
        if !points.points.is_empty() {
            scene.instances.push(Instance::new(Drawable::Points(up.upload_points(&points))).per_primitive_color().unpickable().layer(22));
        }
    }

    /// The selected hole callouts' picks, which don't take the selection colour: live draws a selected callout with
    /// an orange box behind its text and its leader black, its holes light blue (live q-escsel,
    /// drw-hole-callout-palette after Esc; the holes: `push_related_highlights`).
    pub fn hole_callout_selection_picks(&self) -> Vec<u32> {
        let sheet = self.current_sheet();
        self.selection
            .iter()
            .filter(|s| matches!(s, Sel::Annotation(id) if sheet.annotations.iter().any(|a| a.id == *id && matches!(a.kind, AnnotationKind::HoleCallout { .. }))))
            .flat_map(|s| self.picks_of(*s))
            .collect()
    }

    /// [`Self::hole_callout_selection_picks`]' look: one orange box around each callout's text.
    pub fn push_hole_callout_selection(&self, scene: &mut Scene) {
        use noshape_render::{Drawable, Instance, MeshData};
        let Some(b) = &self.built else { return };
        let sheet = self.current_sheet();
        let mut boxes = MeshData::default();
        for s in &self.selection {
            let Sel::Annotation(id) = s else { continue };
            if !sheet.annotations.iter().any(|a| a.id == *id && matches!(a.kind, AnnotationKind::HoleCallout { .. })) {
                continue;
            }
            let picks = self.picks_of(*s);
            let corners: Vec<[f64; 2]> = b.geometry.texts.iter().filter(|t| picks.contains(&t.pick) && !t.text.trim().is_empty()).flat_map(font::text_box).collect();
            if corners.is_empty() {
                continue;
            }
            let (x0, y0) = corners.iter().fold((f64::INFINITY, f64::INFINITY), |m, p| (m.0.min(p[0]), m.1.min(p[1])));
            let (x1, y1) = corners.iter().fold((f64::NEG_INFINITY, f64::NEG_INFINITY), |m, p| (m.0.max(p[0]), m.1.max(p[1])));
            let base = boxes.positions.len() as u32;
            for p in [[x0, y0], [x1, y0], [x1, y1], [x0, y1]] {
                boxes.positions.push([p[0] as f32, p[1] as f32, -0.5]);
                boxes.normals.push([0.0, 0.0, 1.0]);
                boxes.pick_ids.push(0);
            }
            boxes.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        if !boxes.indices.is_empty() {
            let mesh = self.gpu.uploader().upload_mesh(&boxes);
            scene.instances.push(Instance::new(Drawable::Mesh(mesh)).with_color(crate::theme::lin(0xcc925c)).unlit().unpickable().layer(-4));
        }
    }
}
