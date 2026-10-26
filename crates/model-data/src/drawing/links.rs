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

    /// The real view behind a mirror id shown on sheet `si`: (sheet index, id).
    pub fn real_view_of(&self, si: usize, id: ViewId) -> (usize, ViewId) {
        match id.link() {
            Some((uid, real)) => (self.sheet_by_uid(uid).unwrap_or(si), real),
            None => (si, id),
        }
    }
}
