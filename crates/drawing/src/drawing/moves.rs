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
                    if let Some(o) = sources.and_then(|s| views::source_origin(s, &key, part.as_ref(), &frame, scale)) {
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

/// How a view follows its parent's move.
enum Follow {
    /// Aligned with it along `axis` (unit, sheet): only the move across the axis.
    Across([f64; 2]),
    /// The whole move (an isometric, a detail view).
    Whole,
    /// Not at all ("Suppress alignment with parent", a parent on another sheet).
    Stays,
}

fn follow(sheet: &Sheet, id: ViewId) -> Follow {
    let Some(v) = sheet.view(id) else { return Follow::Stays };
    if v.unaligned || sheet.linked_parent(id).is_some() {
        return Follow::Stays;
    }
    let turn = sheet.parent_of(id).map(|p| views::view_rotation(sheet, p)).unwrap_or(0.0);
    let axis = match &v.kind {
        // An isometric (a diagonal projection) stays where it is (live2 drw-tools-1119 vmv.png).
        ViewKind::Projected { .. } => match views::projected_direction(sheet, id) {
            Some(d) if d[0] == 0.0 || d[1] == 0.0 => Some(views::rotate2(d, turn)),
            Some(_) => return Follow::Stays,
            None => None,
        },
        ViewKind::Section { .. } | ViewKind::AlignedSection { .. } => views::section_cut(sheet, id).map(|c| views::rotate2(c.toward_view, turn)),
        ViewKind::Auxiliary { edge_dir, .. } => {
            let n = [-edge_dir[1], edge_dir[0]];
            let l = n[0].hypot(n[1]);
            (l > 1e-12).then(|| [n[0] / l, n[1] / l])
        }
        _ => None,
    };
    axis.map_or(Follow::Whole, Follow::Across)
}

fn across(m: [f64; 2], axis: [f64; 2]) -> [f64; 2] {
    let t = m[0] * axis[0] + m[1] * axis[1];
    [m[0] - axis[0] * t, m[1] - axis[1] * t]
}

/// Where the views of a sheet go when view `id` is dragged by `d` (sheet mm), as live (live2 feel/drawing.json
/// followUp F4): a view aligned with its parent follows the parent's move only across its axis (Front dragged: Top
/// follows in x, Right in y), and a view dragged across its axis takes its parent along (Right dragged down moves Front
/// down, Top staying). Returns each moved view with its new position.
pub fn dragged_view_positions(sheet: &Sheet, id: ViewId, d: [f64; 2]) -> Vec<(ViewId, [f64; 2])> {
    fn go(sheet: &Sheet, v: ViewId, m: [f64; 2], from: Option<ViewId>, moved: &mut Vec<(ViewId, [f64; 2])>) {
        if m[0].hypot(m[1]) < 1e-12 || sheet.view(v).is_none() || moved.iter().any(|(x, _)| *x == v) {
            return;
        }
        moved.push((v, m));
        for c in sheet.views.iter().map(|x| x.id).filter(|c| sheet.parent_of(*c) == Some(v) && Some(*c) != from && !c.is_ghost()) {
            match follow(sheet, c) {
                Follow::Across(axis) => go(sheet, c, across(m, axis), Some(v), moved),
                Follow::Whole => go(sheet, c, m, Some(v), moved),
                Follow::Stays => {}
            }
        }
        if let (Some(p), Follow::Across(axis)) = (sheet.parent_of(v).filter(|p| Some(*p) != from && !p.is_ghost()), follow(sheet, v)) {
            go(sheet, p, across(m, axis), Some(v), moved);
        }
    }
    let mut moved = Vec::new();
    go(sheet, id, d, None, &mut moved);
    moved.into_iter().filter_map(|(v, m)| sheet.view(v).map(|x| (v, [x.position[0] + m[0], x.position[1] + m[1]]))).collect()
}

/// Drag view `id` by `d` (sheet mm): the views as [`dragged_view_positions`], with the notes and callouts grouped with a
/// moved view or attached to its geometry (help A20).
pub fn drag_view(sheet: &mut Sheet, id: ViewId, d: [f64; 2]) {
    let mut moved: Vec<(ViewId, [f64; 2])> = Vec::new();
    for (v, p) in dragged_view_positions(sheet, id, d) {
        if let Some(x) = sheet.view_mut(v) {
            moved.push((v, [p[0] - x.position[0], p[1] - x.position[1]]));
            x.position = p;
        }
    }
    for a in sheet.annotations.iter_mut() {
        if let Some((_, m)) = a.moves_with_view().and_then(|v| moved.iter().find(|(x, _)| *x == v)) {
            a.position = [a.position[0] + m[0], a.position[1] + m[1]];
        }
    }
}
