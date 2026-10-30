//! Copy / paste of drawing views and entities (Onshape: context menu Copy,
//! Paste; Ctrl+C / Ctrl+V). A view copy holds the views with their derived
//! views and the annotations and tables attached to them, self-contained:
//! a view copied without its parent becomes a base view showing the same
//! thing. An entity copy holds notes, callouts, GD&T frames, datums,
//! surface finish and weld symbols and tables; pasted where their views
//! don't exist they come loose (callouts lose their leader until attached).

use super::ViewMoves;
use super::views::Sources;
use super::{Drawing, DrawingView, Sheet, TableKind, ViewId};
use serde::{Deserialize, Serialize};

/// Views on the clipboard (JSON on the system clipboard).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ViewClipboard {
    /// Marker so other clipboard JSON is not taken for views.
    pub noshape_drawing_views: u32,
    pub sheet: Sheet,
    /// The point pasted at the cursor (None: the middle of the positions).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<[f64; 2]>,
    /// Where the views were copied: (document, workspace). Live pastes views
    /// only between drawings of one workspace ([`ViewClipboard::refusal`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<(String, String)>,
}

impl ViewClipboard {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(text: &str) -> Option<Self> {
        let c: ViewClipboard = serde_json::from_str(text).ok()?;
        let s = &c.sheet;
        (c.noshape_drawing_views == 1 && !(s.views.is_empty() && s.annotations.is_empty() && s.tables.is_empty() && s.images.is_empty() && s.sketch.sketch.entity_count() == 0)).then_some(c)
    }

    /// Live's message when these views can't be pasted into `document` /
    /// `workspace` (live behaviours V53: nothing is pasted, a blue bubble
    /// that stays until closed). Entities alone and unstamped copies paste.
    pub fn refusal(&self, document: &str, workspace: &str) -> Option<&'static str> {
        let (d, w) = self.origin.as_ref().filter(|_| !self.sheet.views.is_empty())?;
        if d != document {
            Some("Views could not be copied between documents.")
        } else if w != workspace {
            Some("Views could not be copied between workspaces.")
        } else {
            None
        }
    }

    /// Center of the copied views' (else entities') positions (sheet mm).
    fn center(&self) -> [f64; 2] {
        if let Some(a) = self.anchor {
            return a;
        }
        let mut pts: Vec<[f64; 2]> = if self.sheet.views.is_empty() {
            let images = self.sheet.images.iter().map(|i| [i.pos[0] + i.size[0] / 2.0, i.pos[1] + i.size[1] / 2.0]);
            self.sheet.annotations.iter().map(|a| a.position).chain(self.sheet.tables.iter().map(|t| t.position)).chain(images).collect()
        } else {
            self.sheet.views.iter().map(|v| v.position).collect()
        };
        // Sketch geometry alone: around its points.
        if pts.is_empty() {
            let sk = &self.sheet.sketch.sketch;
            pts = sk.entities().filter_map(|e| sk.point(e.id)).map(|p| [p.x, p.y]).collect();
        }
        let n = pts.len().max(1) as f64;
        let (x, y) = pts.iter().fold((0.0, 0.0), |(x, y), p| (x + p[0], y + p[1]));
        [x / n, y / n]
    }

    /// Views copied (else only entities).
    pub fn has_views(&self) -> bool {
        !self.sheet.views.is_empty()
    }
}

/// Whole sheets on the clipboard (Sheets panel › Copy of one or several
/// selected sheets).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SheetClipboard {
    /// Marker so other clipboard JSON is not taken for sheets.
    pub noshape_drawing_sheet: u32,
    pub sheets: Vec<Sheet>,
}

impl SheetClipboard {
    pub fn new(sheets: Vec<Sheet>) -> Self {
        Self { noshape_drawing_sheet: 1, sheets }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(text: &str) -> Option<Self> {
        let c: SheetClipboard = serde_json::from_str(text).ok()?;
        (c.noshape_drawing_sheet == 1).then_some(c)
    }
}

/// Methods on [`Drawing`] that need noshape-model's logic (in scope through the module's glob re-export).
pub trait DrawingClipboard {
    /// Copy annotations and tables of sheet `sheet` (Copy on a note,
    /// callout, GD&T frame, datum, surface finish or weld symbol, table).
    fn copy_entities(&self, sheet: usize, annotations: &[u64], tables: &[u64]) -> Result<ViewClipboard, String> {
        self.copy_selection(sheet, annotations, tables, &[], &[])
    }

    /// Annotations, tables, drawing sketch entities (with their styles;
    /// live request 1598, qa DWG-LINE-COPY) and images (live request 1613).
    fn copy_selection(&self, sheet: usize, annotations: &[u64], tables: &[u64], sketch: &[u32], images: &[u64]) -> Result<ViewClipboard, String>;

    /// Copy views `ids` of sheet `sheet` (with their derived views and attached
    /// annotations and tables).
    fn copy_views(&self, sheet: usize, ids: &[ViewId], sources: Option<&Sources>) -> Result<ViewClipboard, String>;
}

impl DrawingClipboard for Drawing {
    fn copy_selection(&self, sheet: usize, annotations: &[u64], tables: &[u64], sketch: &[u32], images: &[u64]) -> Result<ViewClipboard, String> {
        let src = self.sheets.get(sheet).ok_or("No sheet")?;
        let annotations: Vec<_> = src.annotations.iter().filter(|a| annotations.contains(&a.id)).cloned().collect();
        let tables: Vec<_> = src.tables.iter().filter(|t| tables.contains(&t.id)).cloned().collect();
        let images: Vec<_> = src.images.iter().filter(|i| images.contains(&i.id)).cloned().collect();
        let mut sk = super::sheet_sketch::SheetSketch::default();
        if !sketch.is_empty() {
            let ids: Vec<noshape_sketch::EntityId> = sketch.iter().map(|i| noshape_sketch::EntityId(*i)).collect();
            sk.sketch = src.sketch.sketch.extract(&ids);
            sk.styles = src.sketch.styles.iter().filter(|(k, _)| sketch.contains(k)).map(|(k, v)| (*k, *v)).collect();
        }
        if annotations.is_empty() && tables.is_empty() && images.is_empty() && sk.sketch.entity_count() == 0 {
            return Err("Nothing to copy".into());
        }
        Ok(ViewClipboard { noshape_drawing_views: 1, sheet: Sheet { views: vec![], annotations, tables, images, sketch: sk, ..src.clone() }, anchor: None, origin: None })
    }

    fn copy_views(&self, sheet: usize, ids: &[ViewId], sources: Option<&Sources>) -> Result<ViewClipboard, String> {
        let src = self.sheets.get(sheet).ok_or("No sheet")?;
        let mut tmp = Drawing { sheets: vec![src.clone(), Sheet { views: vec![], annotations: vec![], tables: vec![], images: vec![], sketch: Default::default(), ..src.clone() }], projection: self.projection, units_mm: self.units_mm, props: self.props.clone(), active_sheet: 0, continuation: None, drawn_by: self.drawn_by.clone(), sources_at: None, meta: self.meta.clone() };
        tmp.move_views_to_sheet(0, ids, 1, sources)?;
        Ok(ViewClipboard { noshape_drawing_views: 1, sheet: tmp.sheets.swap_remove(1), anchor: None, origin: None })
    }
}

/// Methods on [`Sheet`] that need noshape-model's logic (in scope through the module's glob re-export).
pub trait SheetPaste {
    /// Paste copied views with new ids; centered on `at` (sheet millimeters),
    /// else offset 10 mm from the originals. Returns the new view ids.
    fn paste_views(&mut self, clip: &ViewClipboard, at: Option<[f64; 2]>) -> Vec<ViewId>;
}

impl SheetPaste for Sheet {
    fn paste_views(&mut self, clip: &ViewClipboard, at: Option<[f64; 2]>) -> Vec<ViewId> {
        let c = clip.center();
        let d = match at {
            Some(p) => [p[0] - c[0], p[1] - c[1]],
            None => [10.0, -10.0],
        };
        let map: Vec<(ViewId, ViewId)> = clip.sheet.views.iter().zip(self.next_view_id().0..).map(|(v, n)| (v.id, ViewId(n))).collect();
        let remap = |id: ViewId| map.iter().find(|(o, _)| *o == id).map(|(_, n)| *n).unwrap_or(id);
        let mut out: Vec<DrawingView> = clip.sheet.views.clone();
        for v in &mut out {
            v.id = remap(v.id);
            if let Some(p) = v.kind.parent_mut() {
                *p = remap(*p);
            }
            // Derived views follow their parent; only roots move freely.
            v.position = [v.position[0] + d[0], v.position[1] + d[1]];
        }
        // Views the pasted entities may refer to: copied ones, else ones of
        // this sheet (an entity copy pasted on its own sheet).
        let exists = |v: ViewId| map.iter().any(|(o, _)| *o == v) || (clip.sheet.views.is_empty() && self.view(v).is_some());
        let mut pasted = Vec::new();
        for (id, a) in (self.next_annotation_id()..).zip(&clip.sheet.annotations) {
            let mut a = a.clone();
            a.id = id;
            a.group = None;
            a.view_group = None;
            a.ann_group = None;
            match a.kind.view() {
                Some(v) if exists(v) => {
                    if let Some(v) = a.kind.view_mut() {
                        *v = remap(*v);
                    }
                }
                // Its view isn't here: the entity comes loose.
                Some(_) => {
                    a.refs.clear();
                    if let super::AnnotationKind::Balloon { callout, .. } = &mut a.kind {
                        callout.free = true;
                    }
                }
                None => {}
            }
            // Leaders to views that aren't here go.
            let keep = |l: &super::Leader| l.view.is_none_or(exists);
            match &mut a.kind {
                super::AnnotationKind::Note { style, .. } => style.leaders.retain(keep),
                super::AnnotationKind::Gdt { leaders, .. } => leaders.retain(keep),
                super::AnnotationKind::Weld { spec, view, target } => {
                    spec.leaders.retain(keep);
                    match view {
                        Some(v) if exists(*v) => *v = remap(*v),
                        // Loose: the arrow points just below-left of the symbol.
                        Some(_) => {
                            *view = None;
                            *target = [a.position[0] - 10.0 + d[0], a.position[1] - 10.0 + d[1]];
                        }
                        None => *target = [target[0] + d[0], target[1] + d[1]],
                    }
                }
                _ => {}
            }
            let leaders: Vec<&mut super::Leader> = match &mut a.kind {
                super::AnnotationKind::Note { style, .. } => style.leaders.iter_mut().collect(),
                super::AnnotationKind::Gdt { leaders, .. } => leaders.iter_mut().collect(),
                super::AnnotationKind::Weld { spec, .. } => spec.leaders.iter_mut().collect(),
                _ => Vec::new(),
            };
            for l in leaders {
                match &mut l.view {
                    Some(v) => *v = remap(*v),
                    None => l.point = [l.point[0] + d[0], l.point[1] + d[1]],
                }
            }
            // A datum: a free-standing copy of the same shape at the paste,
            // attached to nothing (live C9; qa DWG-DATUM-PASTE-ATTACHED).
            if let super::AnnotationKind::Datum { anchor, on, at, .. } = &mut a.kind {
                *anchor = [anchor[0] + d[0], anchor[1] + d[1]];
                (*on, *at) = (None, None);
                a.refs.clear();
                // Red until it's dragged onto geometry (live, request 1581).
                a.loose = true;
            }
            a.position = [a.position[0] + d[0], a.position[1] + d[1]];
            pasted.push(a);
        }
        self.annotations.extend(pasted);
        for (id, t) in (self.next_table_id()..).zip(&clip.sheet.tables) {
            let mut t = t.clone();
            t.id = id;
            match &mut t.kind {
                TableKind::Bom { view, .. } | TableKind::Hole { view, .. } | TableKind::CutList { view, .. } => *view = remap(*view),
                TableKind::General { .. } | TableKind::Revision { .. } | TableKind::Custom { .. } => {}
            }
            t.position = [t.position[0] + d[0], t.position[1] + d[1]];
            self.tables.push(t);
        }
        // Images, with new ids.
        for im in &clip.sheet.images {
            let mut im = im.clone();
            im.id = self.images.iter().map(|i| i.id).max().unwrap_or(0) + 1;
            im.pos = [im.pos[0] + d[0], im.pos[1] + d[1]];
            self.images.push(im);
        }
        // Drawing sketch entities, with their styles.
        let sk = &clip.sheet.sketch;
        if clip.sheet.views.is_empty() && sk.sketch.entity_count() > 0 {
            let olds: Vec<u32> = sk.sketch.entities().filter(|e| e.kind() != noshape_sketch::EntityKind::Point).map(|e| e.id.0).collect();
            let news = self.sketch.sketch.paste(&sk.sketch, noshape_sketch::DVec2::new(d[0], d[1]));
            if olds.len() == news.len() {
                for (o, n) in olds.iter().zip(&news) {
                    if let Some(st) = sk.styles.get(o) {
                        self.sketch.styles.insert(n.0, *st);
                    }
                }
            }
            let _ = self.sketch.sketch.solve();
        }
        let ids = out.iter().map(|v| v.id).collect();
        self.views.extend(out);
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::TabId;
    use crate::drawing::{ViewKind, ViewOrientation};

    #[test]
    fn copy_and_paste_views_keep_their_children() {
        let mut d = Drawing::default();
        let tab = TabId::new_random();
        let s = &mut d.sheets[0];
        s.views.push(DrawingView { id: ViewId(1), kind: ViewKind::Base { source: tab, config: Default::default(), orientation: ViewOrientation::Front, part: None }, position: [100.0, 100.0], ..Default::default() });
        s.views.push(DrawingView { id: ViewId(2), kind: ViewKind::Projected { parent: ViewId(1), direction: Some([1.0, 0.0]) }, position: [180.0, 100.0], ..Default::default() });
        let clip = d.copy_views(0, &[ViewId(1)], None).expect("copy");
        assert_eq!(clip.sheet.views.len(), 2, "the projected child is copied too");
        let back = ViewClipboard::from_json(&clip.to_json()).expect("round trip");
        assert!(ViewClipboard::from_json("{\"a\":1}").is_none());
        let ids = d.sheets[0].paste_views(&back, Some([300.0, 200.0]));
        assert_eq!(ids, vec![ViewId(3), ViewId(4)]);
        let s = &d.sheets[0];
        assert_eq!(s.views.len(), 4);
        // The pasted child points at the pasted parent; the pair is centered on the cursor.
        assert_eq!(s.parent_of(ViewId(4)), Some(ViewId(3)));
        let (a, b) = (s.view(ViewId(3)).unwrap().position, s.view(ViewId(4)).unwrap().position);
        assert!(((a[0] + b[0]) * 0.5 - 300.0).abs() < 1e-9 && (a[1] - 200.0).abs() < 1e-9);
    }

    #[test]
    fn copy_and_paste_sheets() {
        let mut d = Drawing::default();
        d.sheets[0].name = "Sheet1".into();
        d.add_sheet(0);
        let clip = SheetClipboard::new(vec![d.sheets[0].clone()]);
        let json = clip.to_json();
        assert!(ViewClipboard::from_json(&json).is_none(), "a sheet is not a view copy");
        let back = SheetClipboard::from_json(&json).expect("round trip");
        assert!(SheetClipboard::from_json(&ViewClipboard::default().to_json()).is_none());
        // Pasted at the bottom, renamed when the name is taken.
        let i = d.paste_sheet(back.sheets[0].clone());
        assert_eq!(i, d.sheets.len() - 1);
        assert_eq!(d.sheets[i].name, "Sheet1 (1)");
        let j = d.paste_sheet(SheetClipboard::from_json(&json).unwrap().sheets.remove(0));
        assert_eq!(d.sheets[j].name, "Sheet1 (2)");
    }

    /// A sheet line copied and pasted (live request 1598, qa DWG-LINE-COPY):
    /// a second line centred on the cursor, its style kept; a view copy
    /// carries no sketch.
    #[test]
    fn copy_and_paste_sketch_lines() {
        let mut d = Drawing::default();
        let s0 = &mut d.sheets[0];
        let l = s0.sketch.sketch.add_line(noshape_sketch::DVec2::new(10.0, 10.0), noshape_sketch::DVec2::new(30.0, 10.0));
        s0.sketch.styles.insert(l.0, crate::drawing::sheet_sketch::SketchStyle { color: Some([255, 0, 0]), ..Default::default() });
        s0.sketch.sketch.add_circle(noshape_sketch::DVec2::new(100.0, 100.0), 5.0);
        let clip = ViewClipboard::from_json(&d.copy_selection(0, &[], &[], &[l.0], &[]).unwrap().to_json()).expect("a sketch copy round trips");
        assert!(d.copy_entities(0, &[], &[]).is_err());
        let before = d.sheets[0].sketch.sketch.entity_count();
        d.sheets[0].paste_views(&clip, Some([50.0, 50.0]));
        let sk = &d.sheets[0].sketch;
        let lines: Vec<_> = sk.sketch.entities().filter(|e| matches!(e.geometry, noshape_sketch::Geometry::Line { .. })).collect();
        assert_eq!((lines.len(), sk.sketch.entity_count()), (2, before + 3), "a line and its two points");
        let new = lines.iter().find(|e| e.id != l).unwrap();
        let (a, b) = new.endpoints().unwrap();
        let (pa, pb) = (sk.sketch.point(a).unwrap(), sk.sketch.point(b).unwrap());
        assert!(((pa + pb) / 2.0 - noshape_sketch::DVec2::new(50.0, 50.0)).length() < 1e-6, "{pa} {pb}");
        assert_eq!(sk.style(new.id).color, Some([255, 0, 0]));
    }

    /// Copy on an image (live request 1613): it pastes centred on the
    /// cursor with a new id; nothing else of the sheet comes along.
    #[test]
    fn copy_and_paste_an_image() {
        let mut d = Drawing::default();
        let im = |id: u64, pos: [f64; 2]| crate::drawing::SheetImage { id, name: "a.png".into(), data_base64: "AAAA".into(), pos, size: [20.0, 10.0], layer: Default::default() };
        d.sheets[0].images = vec![im(1, [10.0, 10.0]), im(2, [100.0, 100.0])];
        let clip = ViewClipboard::from_json(&d.copy_selection(0, &[], &[], &[], &[2]).unwrap().to_json()).expect("an image copy round trips");
        assert_eq!(clip.sheet.images.len(), 1);
        d.sheets[0].paste_views(&clip, Some([50.0, 50.0]));
        let ims = &d.sheets[0].images;
        assert_eq!(ims.len(), 3);
        assert_eq!(ims[2].id, 3);
        assert_eq!(ims[2].pos, [40.0, 45.0]);
    }

    #[test]
    fn copy_and_paste_entities() {
        use crate::drawing::{Annotation, AnnotationKind};
        let mut d = Drawing::default();
        let s0 = &mut d.sheets[0];
        s0.annotations.push(Annotation { id: 1, kind: AnnotationKind::note("COPY ME", 3.5), position: [50.0, 50.0], ..Default::default() });
        s0.annotations.push(Annotation { id: 2, kind: AnnotationKind::balloon(ViewId(7), [0.0, 0.0], "3"), position: [80.0, 60.0], ..Default::default() });
        let clip = d.copy_entities(0, &[1, 2], &[]).unwrap();
        assert!(!clip.has_views());
        let back = ViewClipboard::from_json(&clip.to_json()).expect("entity clipboards round trip");
        // Pasted on another sheet: new ids, at the cursor; the callout's view
        // isn't there, so it has no leader.
        let mut other = Sheet::default();
        other.paste_views(&back, Some([200.0, 100.0]));
        assert_eq!(other.annotations.len(), 2);
        let note = other.annotations.iter().find(|a| matches!(a.kind, AnnotationKind::Note { .. })).unwrap();
        let call = other.annotations.iter().find(|a| matches!(a.kind, AnnotationKind::Balloon { .. })).unwrap();
        assert!(matches!(&call.kind, AnnotationKind::Balloon { callout, .. } if callout.free));
        // Centered on the cursor, keeping their spacing.
        assert!(((note.position[0] + call.position[0]) / 2.0 - 200.0).abs() < 1e-9);
        assert!(((call.position[0] - note.position[0]) - 30.0).abs() < 1e-9);
    }
}
