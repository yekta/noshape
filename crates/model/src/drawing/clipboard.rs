//! Copy / paste of drawing views (Onshape: view context menu Copy, Paste;
//! Ctrl+C / Ctrl+V). A copy holds the views with their derived views and
//! the annotations and tables attached to them, self-contained: a view
//! copied without its parent becomes a base view showing the same thing.

use super::views::Sources;
use super::{Drawing, DrawingView, Sheet, TableKind, ViewId};
use serde::{Deserialize, Serialize};

/// Views on the clipboard (JSON on the system clipboard).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ViewClipboard {
    /// Marker so other clipboard JSON is not taken for views.
    pub noshape_drawing_views: u32,
    pub sheet: Sheet,
}

impl ViewClipboard {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    pub fn from_json(text: &str) -> Option<Self> {
        let c: ViewClipboard = serde_json::from_str(text).ok()?;
        (c.noshape_drawing_views == 1 && !c.sheet.views.is_empty()).then_some(c)
    }

    /// Center of the copied views' positions (sheet millimeters).
    fn center(&self) -> [f64; 2] {
        let n = self.sheet.views.len().max(1) as f64;
        let (x, y) = self.sheet.views.iter().fold((0.0, 0.0), |(x, y), v| (x + v.position[0], y + v.position[1]));
        [x / n, y / n]
    }
}

impl Drawing {
    /// Copy views `ids` of sheet `sheet` (with their derived views and attached
    /// annotations and tables).
    pub fn copy_views(&self, sheet: usize, ids: &[ViewId], sources: Option<&Sources>) -> Result<ViewClipboard, String> {
        let src = self.sheets.get(sheet).ok_or("No sheet")?;
        let mut tmp = Drawing { sheets: vec![src.clone(), Sheet { views: vec![], annotations: vec![], tables: vec![], ..src.clone() }], projection: self.projection, units_mm: self.units_mm };
        tmp.move_views_to_sheet(0, ids, 1, sources)?;
        Ok(ViewClipboard { noshape_drawing_views: 1, sheet: tmp.sheets.swap_remove(1) })
    }
}

impl Sheet {
    /// Paste copied views with new ids; centered on `at` (sheet millimeters),
    /// else offset 10 mm from the originals. Returns the new view ids.
    pub fn paste_views(&mut self, clip: &ViewClipboard, at: Option<[f64; 2]>) -> Vec<ViewId> {
        let c = clip.center();
        let d = match at {
            Some(p) => [p[0] - c[0], p[1] - c[1]],
            None => [10.0, -10.0],
        };
        let mut next = self.next_view_id().0;
        let map: Vec<(ViewId, ViewId)> = clip
            .sheet
            .views
            .iter()
            .map(|v| {
                let n = ViewId(next);
                next += 1;
                (v.id, n)
            })
            .collect();
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
        let mut next_ann = self.next_annotation_id();
        for a in &clip.sheet.annotations {
            let mut a = a.clone();
            a.id = next_ann;
            next_ann += 1;
            if let Some(v) = a.kind.view_mut() {
                *v = remap(*v);
            }
            a.position = [a.position[0] + d[0], a.position[1] + d[1]];
            self.annotations.push(a);
        }
        let mut next_tbl = self.next_table_id();
        for t in &clip.sheet.tables {
            let mut t = t.clone();
            t.id = next_tbl;
            next_tbl += 1;
            match &mut t.kind {
                TableKind::Bom { view } | TableKind::Hole { view, .. } => *view = remap(*view),
                TableKind::General { .. } => {}
            }
            t.position = [t.position[0] + d[0], t.position[1] + d[1]];
            self.tables.push(t);
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
}
