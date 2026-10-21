//! Views linked across sheets (live behaviours "Views linked across sheets",
//! V52): Move to sheet moves a view alone; a view whose parent is on
//! another sheet stays linked to it (scale, orientation, sources follow; no
//! position alignment), and the parent's sheet keeps drawing the link's
//! cutting line / detail circle.
//!
//! Each sheet holds hidden mirrors so that the per-sheet view logic works
//! unchanged: a mirror of the parent chain of every view whose parent is
//! elsewhere ("parent mirror"), and on the parent's sheet a mirror of the
//! linked child ("child mirror", for the indicators drawn on the parent).
//! Mirrors are rebuilt from the real views by [`Drawing::sync_links`] after
//! every change; their ids encode the real view ([`ViewId::link`]).

use super::{Drawing, DrawingView, Sheet, ViewId, SHEET_VIEW};

const GHOST: u64 = 1 << 62;
const CHILD: u64 = 1 << 61;
const ID_BITS: u64 = 24;

impl ViewId {
    /// A hidden mirror of a view on another sheet.
    pub fn is_ghost(self) -> bool {
        self != SHEET_VIEW && self.0 & GHOST != 0
    }

    /// A child mirror (draws only its indicator on the parent).
    pub fn is_child_ghost(self) -> bool {
        self.is_ghost() && self.0 & CHILD != 0
    }

    /// The mirrored view: (sheet uid, view id).
    pub fn link(self) -> Option<(u32, ViewId)> {
        self.is_ghost().then_some((((self.0 >> ID_BITS) & 0xFF_FFFF) as u32, ViewId(self.0 & ((1 << ID_BITS) - 1))))
    }

    /// The parent mirror id of view `id` of sheet `uid`.
    pub fn ghost_of(uid: u32, id: ViewId) -> ViewId {
        ViewId(GHOST | ((uid as u64) << ID_BITS) | (id.0 & ((1 << ID_BITS) - 1)))
    }

    fn child_ghost_of(uid: u32, id: ViewId) -> ViewId {
        ViewId(Self::ghost_of(uid, id).0 | CHILD)
    }
}

impl Sheet {
    /// The views shown on the sheet (no mirrors).
    pub fn real_views(&self) -> impl Iterator<Item = &DrawingView> {
        self.views.iter().filter(|v| !v.id.is_ghost())
    }

    /// The real view behind a parent mirror's id on this sheet (sheet uid,
    /// view) — i.e. a view's parent lives on another sheet.
    pub fn linked_parent(&self, id: ViewId) -> Option<(u32, ViewId)> {
        self.parent_of(id).and_then(|p| p.link())
    }
}

impl Drawing {
    /// Give every sheet a unique, stable uid (new or copied sheets).
    pub fn ensure_sheet_uids(&mut self) {
        let mut used: Vec<u32> = Vec::new();
        let mut next = self.sheets.iter().map(|s| s.uid).max().unwrap_or(0) + 1;
        for s in &mut self.sheets {
            if s.uid == 0 || used.contains(&s.uid) {
                s.uid = next;
                next += 1;
            }
            used.push(s.uid);
        }
    }

    pub fn sheet_by_uid(&self, uid: u32) -> Option<usize> {
        self.sheets.iter().position(|s| s.uid == uid)
    }

    /// Rebuild the mirrors from the real views; views whose linked parent is
    /// gone are deleted (like a deleted parent's children on its own sheet).
    pub fn sync_links(&mut self) {
        self.ensure_sheet_uids();
        // Linked views whose parent no longer exists: deleted (with their
        // own descendants), until nothing changes.
        loop {
            let mut orphan: Option<(usize, ViewId)> = None;
            'find: for (si, s) in self.sheets.iter().enumerate() {
                for v in s.real_views() {
                    if let Some((uid, pid)) = s.linked_parent(v.id)
                        && self.sheet_by_uid(uid).and_then(|i| self.sheets[i].view(pid)).is_none_or(|p| p.id.is_ghost())
                    {
                        orphan = Some((si, v.id));
                        break 'find;
                    }
                }
            }
            let Some((si, id)) = orphan else { break };
            self.sheets[si].remove_view(id);
        }
        // Drop every mirror, then rebuild them.
        for s in &mut self.sheets {
            s.views.retain(|v| !v.id.is_ghost());
        }
        let n = self.sheets.len();
        for si in 0..n {
            // Parent mirrors: the chain above each linked view.
            let wanted: Vec<(u32, ViewId)> = self.sheets[si].views.iter().filter_map(|v| self.sheets[si].parent_of(v.id).and_then(|p| p.link())).collect();
            let mut ghosts: Vec<DrawingView> = Vec::new();
            for (uid, pid) in wanted {
                let mut cur = Some((uid, pid));
                while let Some((u, id)) = cur {
                    let gid = ViewId::ghost_of(u, id);
                    if ghosts.iter().any(|g| g.id == gid) {
                        break;
                    }
                    let Some(src_i) = self.sheet_by_uid(u) else { break };
                    let Some(real) = self.sheets[src_i].view(id) else { break };
                    let mut g = real.clone();
                    g.id = gid;
                    // Its parent: on the same source sheet (mirrored too), or
                    // itself linked elsewhere (the same mirror id).
                    cur = match g.kind.parent_mut() {
                        Some(p) => {
                            let next = match p.link() {
                                Some(l) => l,
                                None => (u, *p),
                            };
                            *p = ViewId::ghost_of(next.0, next.1);
                            // A mirror whose parent is this very sheet's real view.
                            if next.0 == self.sheets[si].uid {
                                *p = next.1;
                                None
                            } else {
                                Some(next)
                            }
                        }
                        None => None,
                    };
                    ghosts.push(g);
                }
            }
            self.sheets[si].views.splice(0..0, ghosts);
        }
        // Child mirrors on the parent's sheet (their indicators).
        let mut adds: Vec<(usize, DrawingView)> = Vec::new();
        for s in &self.sheets {
            for v in s.real_views() {
                let Some((uid, pid)) = s.linked_parent(v.id) else { continue };
                let Some(pi) = self.sheet_by_uid(uid) else { continue };
                let mut g = v.clone();
                g.id = ViewId::child_ghost_of(s.uid, v.id);
                if let Some(p) = g.kind.parent_mut() {
                    *p = pid;
                }
                adds.push((pi, g));
            }
        }
        for (pi, g) in adds {
            self.sheets[pi].views.push(g);
        }
    }

    /// Move to sheet (live: the views move alone, at the same sheet
    /// coordinates, keeping their kind): a moved view whose parent stays
    /// behind stays linked to it (its inherited scale frozen), children left
    /// behind stay linked to the moved view, and links elsewhere follow.
    /// Returns (old id, new id) per moved view.
    pub fn move_views_to_sheet_linked(&mut self, from: usize, ids: &[ViewId], to: usize) -> Result<Vec<(ViewId, ViewId)>, String> {
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
            if v.scale.is_none() && src.parent_of(*old).is_some() {
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

    /// The real view behind a mirror id shown on sheet `si`: (sheet index, id).
    pub fn real_view_of(&self, si: usize, id: ViewId) -> (usize, ViewId) {
        match id.link() {
            Some((uid, real)) => (self.sheet_by_uid(uid).unwrap_or(si), real),
            None => (si, id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawing::{ViewKind, ViewOrientation};

    fn base(id: u64) -> DrawingView {
        DrawingView { id: ViewId(id), kind: ViewKind::Base { source: crate::document::TabId(1), config: Default::default(), orientation: ViewOrientation::Front, part: None }, position: [100.0, 100.0], ..Default::default() }
    }

    #[test]
    fn moving_a_parent_leaves_its_children_linked() {
        let mut d = Drawing::default();
        let s2 = d.add_sheet(0);
        d.sheets[0].views.push(base(1));
        d.sheets[0].views.push(DrawingView { id: ViewId(2), kind: ViewKind::Projected { parent: ViewId(1), direction: Some([0.0, 1.0]) }, position: [100.0, 180.0], ..Default::default() });
        d.sync_links();
        // Move only the parent to sheet 2.
        let map = d.move_views_to_sheet_linked(0, &[ViewId(1)], s2).unwrap();
        let moved = map[0].1;
        assert!(d.sheets[0].view(ViewId(2)).is_some(), "the child stays");
        let (uid, pid) = d.sheets[0].linked_parent(ViewId(2)).expect("linked across sheets");
        assert_eq!((d.sheet_by_uid(uid), pid), (Some(s2), moved));
        // A parent mirror on sheet 1, a child mirror on sheet 2.
        assert_eq!(d.sheets[0].views.iter().filter(|v| v.id.is_ghost()).count(), 1);
        assert_eq!(d.sheets[s2].views.iter().filter(|v| v.id.is_child_ghost()).count(), 1);
        // The parent's changes reach the mirror.
        d.sheets[s2].view_mut(moved).unwrap().scale = Some(2.0);
        d.sync_links();
        let g = d.sheets[0].view(d.sheets[0].parent_of(ViewId(2)).unwrap()).unwrap();
        assert_eq!(g.scale, Some(2.0));
        assert_eq!(super::super::views::view_scale(&d.sheets[0], ViewId(2)), 2.0);
        // Moving it back: no links left.
        d.move_views_to_sheet_linked(s2, &[moved], 0).unwrap();
        assert!(d.sheets.iter().all(|s| s.views.iter().all(|v| !v.id.is_ghost())));
        // Deleting a linked view's parent deletes the linked view.
        let s2b = d.add_sheet(0);
        let back = d.sheets[0].real_views().find(|v| matches!(v.kind, ViewKind::Base { .. })).unwrap().id;
        d.move_views_to_sheet_linked(0, &[back], s2b).unwrap();
        let p = d.sheets[s2b].real_views().next().unwrap().id;
        d.sheets[s2b].remove_view(p);
        d.sync_links();
        assert!(d.sheets[0].real_views().next().is_none());
    }
}
