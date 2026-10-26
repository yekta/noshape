//! Moving views between sheets: needs the views' geometry ([`super::views`]), so it is logic on top of the
//! drawing data in `noshape_model_data::drawing`.

use super::views;
use super::*;

/// View moves on a [`Drawing`] (in scope through `noshape_model::drawing::*`).
pub trait ViewMoves {
    /// Move views of sheet `from` (each with every view derived from it,
    /// the annotations and tables attached to them) to sheet `to`. Views get
    /// new ids on the target sheet; the returned pairs map old to new ids.
    ///
    /// A moved view whose parent stays behind becomes an independent base
    /// view showing the same projection (explicit frame, effective scale and
    /// rotation; a detail view becomes a circle-cropped view). Section views
    /// need their parent's cutting line, so they only move with their parent.
    /// `sources` (when given) keeps converted detail views' content in place.
    fn move_views_to_sheet(&mut self, from: usize, ids: &[ViewId], to: usize, sources: Option<&views::Sources>) -> Result<Vec<(ViewId, ViewId)>, String>;

    /// Move to sheet (live: the views move alone, at the same sheet
    /// coordinates, keeping their kind): a moved view whose parent stays
    /// behind stays linked to it (its inherited scale frozen), children left
    /// behind stay linked to the moved view, and links elsewhere follow.
    /// Returns (old id, new id) per moved view.
    fn move_views_to_sheet_linked(&mut self, from: usize, ids: &[ViewId], to: usize) -> Result<Vec<(ViewId, ViewId)>, String>;
}

impl ViewMoves for Drawing {
    /// Move views of sheet `from` (each with every view derived from it,
    /// the annotations and tables attached to them) to sheet `to`. Views get
    /// new ids on the target sheet; the returned pairs map old to new ids.
    ///
    /// A moved view whose parent stays behind becomes an independent base
    /// view showing the same projection (explicit frame, effective scale and
    /// rotation; a detail view becomes a circle-cropped view). Section views
    /// need their parent's cutting line, so they only move with their parent.
    /// `sources` (when given) keeps converted detail views' content in place.
    fn move_views_to_sheet(&mut self, from: usize, ids: &[ViewId], to: usize, sources: Option<&views::Sources>) -> Result<Vec<(ViewId, ViewId)>, String> {
        if from == to || from >= self.sheets.len() || to >= self.sheets.len() {
            return Err("Choose another sheet".into());
        }
        let src = &self.sheets[from];
        let mut moved: Vec<ViewId> = Vec::new();
        for id in ids {
            if src.view(*id).is_none() {
                return Err("View not found".into());
            }
            for d in src.descendants(*id) {
                if !moved.contains(&d) {
                    moved.push(d);
                }
            }
        }
        // Views in sheet order (parents before children).
        let order: Vec<ViewId> = src.views.iter().map(|v| v.id).filter(|v| moved.contains(v)).collect();
        let mut out_views: Vec<DrawingView> = Vec::new();
        for id in &order {
            let mut v = src.view(*id).cloned().expect("listed above");
            let orphan = src.parent_of(*id).is_some_and(|p| !moved.contains(&p));
            if orphan {
                if matches!(v.kind, ViewKind::Section { .. } | ViewKind::AlignedSection { .. }) {
                    return Err("A section view can only move together with its parent view".into());
                }
                let frame = views::view_frame(self, src, *id).ok_or("The view cannot be computed")?;
                let (key, part) = views::view_source(src, *id).ok_or("The view has no source")?;
                let scale = views::view_scale(src, *id);
                let rotation = views::view_rotation(src, *id);
                let root_orientation = match views::root_view(src, *id).and_then(|r| src.view(r)).map(|r| &r.kind) {
                    Some(ViewKind::Base { orientation, .. }) => *orientation,
                    _ => ViewOrientation::Front,
                };
                if let ViewKind::Detail { center, radius, label, .. } = &v.kind {
                    v.crop = Some(Crop::Circle { center: *center, radius: *radius });
                    if v.label.is_none() {
                        v.label = Some(format!("DETAIL {label}"));
                    }
                    // Keep the detail content where it was on the sheet.
                    if let Some(o) = sources.and_then(|s| views::source_origin(s, &key, part.as_ref(), &frame)) {
                        let k = scale * 1000.0;
                        let d = views::rotate2([(o[0] - center[0]) * k, (o[1] - center[1]) * k], rotation);
                        v.position = [v.position[0] + d[0], v.position[1] + d[1]];
                    }
                }
                v.kind = ViewKind::Base { source: key.tab, config: key.config, orientation: root_orientation, part };
                v.explode = key.explode;
                v.custom_frame = Some([frame.dir.to_array(), frame.up.to_array()]);
                v.scale = Some(scale);
                v.rotation = rotation;
            }
            out_views.push(v);
        }
        // New ids on the target sheet.
        let dst = &self.sheets[to];
        let mut next = dst.next_view_id().0;
        let map: Vec<(ViewId, ViewId)> = order
            .iter()
            .map(|id| {
                let n = ViewId(next);
                next += 1;
                (*id, n)
            })
            .collect();
        let remap = |id: ViewId| map.iter().find(|(o, _)| *o == id).map(|(_, n)| *n).unwrap_or(id);
        for v in &mut out_views {
            v.id = remap(v.id);
            if let Some(p) = v.kind.parent_mut() {
                *p = remap(*p);
            }
        }
        let mut anns: Vec<Annotation> = Vec::new();
        for (id, a) in (dst.next_annotation_id()..).zip(src.annotations.iter().filter(|a| a.kind.view().is_some_and(|v| moved.contains(&v)))) {
            let mut a = a.clone();
            a.id = id;
            if let Some(v) = a.kind.view_mut() {
                *v = remap(*v);
            }
            anns.push(a);
        }
        let mut tbls: Vec<Table> = Vec::new();
        for (id, t) in (dst.next_table_id()..).zip(src.tables.iter().filter(|t| t.view().is_some_and(|v| moved.contains(&v)))) {
            let mut t = t.clone();
            t.id = id;
            match &mut t.kind {
                TableKind::Bom { view, .. } | TableKind::Hole { view, .. } | TableKind::CutList { view, .. } => *view = remap(*view),
                TableKind::General { .. } | TableKind::Revision { .. } | TableKind::Custom { .. } => {}
            }
            tbls.push(t);
        }
        let src = &mut self.sheets[from];
        src.views.retain(|v| !moved.contains(&v.id));
        src.annotations.retain(|a| a.kind.view().is_none_or(|v| !moved.contains(&v)));
        src.tables.retain(|t| t.view().is_none_or(|v| !moved.contains(&v)));
        for a in &mut src.annotations {
            if let AnnotationKind::Gdt { leaders, .. } = &mut a.kind {
                leaders.retain(|l| l.view.is_none_or(|v| !moved.contains(&v)));
            }
        }
        let dst = &mut self.sheets[to];
        dst.views.extend(out_views);
        dst.annotations.extend(anns);
        dst.tables.extend(tbls);
        Ok(map)
    }

    /// Move to sheet (live: the views move alone, at the same sheet
    /// coordinates, keeping their kind): a moved view whose parent stays
    /// behind stays linked to it (its inherited scale frozen), children left
    /// behind stay linked to the moved view, and links elsewhere follow.
    /// Returns (old id, new id) per moved view.
    fn move_views_to_sheet_linked(&mut self, from: usize, ids: &[ViewId], to: usize) -> Result<Vec<(ViewId, ViewId)>, String> {
        if from == to || from >= self.sheets.len() || to >= self.sheets.len() {
            return Err("Choose another sheet".into());
        }
        self.ensure_sheet_uids();
        let (src_uid, dst_uid) = (self.sheets[from].uid, self.sheets[to].uid);
        let ids: Vec<ViewId> = ids.iter().copied().filter(|i| !i.is_ghost() && self.sheets[from].view(*i).is_some()).collect();
        if ids.is_empty() {
            return Err("View not found".into());
        }
        // New ids on the target sheet, in sheet order.
        let order: Vec<ViewId> = self.sheets[from].real_views().map(|v| v.id).filter(|v| ids.contains(v)).collect();
        let mut next = self.sheets[to].next_view_id().0;
        let map: Vec<(ViewId, ViewId)> = order
            .iter()
            .map(|id| {
                let n = ViewId(next);
                next += 1;
                (*id, n)
            })
            .collect();
        let remap = |id: ViewId| map.iter().find(|(o, _)| *o == id).map(|(_, n)| *n);
        let src = &self.sheets[from];
        let mut out_views = Vec::new();
        for (old, new) in &map {
            let mut v = src.view(*old).cloned().expect("listed above");
            // Its scale no longer follows a parent left behind (live: the
            // moved section kept its own scale).
            if v.scale.is_none() && !v.sheet_scale && src.parent_of(*old).is_some() {
                v.scale = Some(super::views::view_scale(src, *old));
            }
            v.id = *new;
            if let Some(p) = v.kind.parent_mut() {
                *p = match (remap(*p), p.link()) {
                    // The parent moves too.
                    (Some(n), _) => n,
                    // Linked to the target sheet: home again.
                    (None, Some((u, real))) if u == dst_uid => real,
                    (None, Some(_)) => *p,
                    // The parent stays behind: linked.
                    (None, None) => ViewId::ghost_of(src_uid, *p),
                };
            }
            out_views.push(v);
        }
        // Annotations and tables of the moved views go with them.
        let moved: Vec<ViewId> = map.iter().map(|m| m.0).collect();
        let dst = &self.sheets[to];
        let mut anns = Vec::new();
        for (id, a) in (dst.next_annotation_id()..).zip(src.annotations.iter().filter(|a| a.kind.view().is_some_and(|v| moved.contains(&v)))) {
            let mut a = a.clone();
            a.id = id;
            if let Some(v) = a.kind.view_mut() {
                *v = remap(*v).unwrap_or(*v);
            }
            anns.push(a);
        }
        let mut tbls = Vec::new();
        for (id, t) in (dst.next_table_id()..).zip(src.tables.iter().filter(|t| t.view().is_some_and(|v| moved.contains(&v)))) {
            let mut t = t.clone();
            t.id = id;
            match &mut t.kind {
                super::TableKind::Bom { view, .. } | super::TableKind::Hole { view, .. } | super::TableKind::CutList { view, .. } => *view = remap(*view).unwrap_or(*view),
                super::TableKind::General { .. } | super::TableKind::Revision { .. } | super::TableKind::Custom { .. } => {}
            }
            tbls.push(t);
        }
        let src = &mut self.sheets[from];
        src.views.retain(|v| !moved.contains(&v.id));
        src.annotations.retain(|a| a.kind.view().is_none_or(|v| !moved.contains(&v)));
        src.tables.retain(|t| t.view().is_none_or(|v| !moved.contains(&v)));
        // Children left behind: linked to the moved view.
        for v in src.views.iter_mut() {
            if let Some(p) = v.kind.parent_mut()
                && let Some(n) = remap(*p)
            {
                *p = ViewId::ghost_of(dst_uid, n);
            }
        }
        let dst = &mut self.sheets[to];
        dst.views.extend(out_views);
        dst.annotations.extend(anns);
        dst.tables.extend(tbls);
        // Links from other sheets to a moved view follow it.
        for s in &mut self.sheets {
            let here = s.uid;
            for v in s.views.iter_mut().filter(|v| !v.id.is_ghost()) {
                if let Some(p) = v.kind.parent_mut()
                    && let Some((u, real)) = p.link()
                    && u == src_uid
                    && let Some(n) = remap(real)
                {
                    *p = if here == dst_uid { n } else { ViewId::ghost_of(dst_uid, n) };
                }
            }
        }
        self.sync_links();
        Ok(map)
    }
}
