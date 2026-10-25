//! Drawing editor: note leaders as live drw2 10-21 g ("Note leaders: grips,
//! nodes, Add leader"). The bend drags the landing horizontally ("Move by
//! grip"); a right-click exactly on a leader line gives Paste, Copy, Paste
//! as table, Remove leaders, Add node, Clear selection, Zoom to fit, Delete
//! (Add node puts a node at the clicked point); on a node: Paste, Copy,
//! Paste as table, Add leader (a branch from the node), Remove leaders,
//! Remove node, Clear selection, Zoom to fit, Delete. The note's own Add
//! leader branches from the existing landing's bend.

use crate::drawing_view::*;
use gpui_kit::*;
use noshape_model::drawing::sheet::LeaderPath;
use noshape_model::drawing::{Annotation, AnnotationKind, Leader, LeaderStart, remove_leader, remove_leader_node};

/// What a sheet point hits on a note's leaders.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LeaderHit {
    /// Leader `leader`'s line, segment `seg` of its path, at `p`.
    Stroke { id: u64, leader: usize, seg: usize, p: [f64; 2] },
    Node { id: u64, leader: usize, node: usize },
    /// The end of a leader's landing (leaders from the note's side).
    Bend { id: u64, leader: usize },
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Distance from `p` to the segment `a`–`b`.
fn seg_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let l2 = d[0] * d[0] + d[1] * d[1];
    let t = if l2 > 0.0 { (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / l2).clamp(0.0, 1.0) } else { 0.0 };
    dist(p, [a[0] + d[0] * t, a[1] + d[1] * t])
}

/// The note's leaders, mutable.
fn note_leaders(a: &mut Annotation) -> Option<&mut Vec<Leader>> {
    match &mut a.kind {
        AnnotationKind::Note { style, .. } => Some(&mut style.leaders),
        _ => None,
    }
}

impl DrawingEditor {
    /// A note's leader paths on the sheet.
    pub fn note_leader_paths_of(&self, a: &Annotation) -> Vec<LeaderPath> {
        self.pk().note_leader_paths(a)
    }

    /// The note leader under sheet point `c`: a bend or node first, else a
    /// line (live: exactly on the stroke; a few px off is the sheet).
    pub fn leader_hit(&self, c: [f64; 2]) -> Option<LeaderHit> {
        let (near, on_line) = (self.tol_mm(5.0), self.tol_mm(3.0));
        let mut stroke = None;
        for a in &self.current_sheet().annotations {
            for (leader, path) in self.note_leader_paths_of(a).iter().enumerate() {
                let id = a.id;
                for (k, p) in path.points.iter().enumerate() {
                    if dist(*p, c) >= near {
                        continue;
                    }
                    if path.head == 2 && k == 1 {
                        return Some(LeaderHit::Bend { id, leader });
                    }
                    if k >= path.head && k + 1 < path.points.len() {
                        return Some(LeaderHit::Node { id, leader, node: k - path.head });
                    }
                }
                if stroke.is_none()
                    && let Some(seg) = path.points.windows(2).position(|w| seg_dist(c, w[0], w[1]) < on_line)
                {
                    stroke = Some(LeaderHit::Stroke { id, leader, seg, p: c });
                }
            }
        }
        stroke
    }

    /// Edit note `id`'s leaders (one undo step named `what`).
    fn edit_leaders(&mut self, what: &str, id: u64, cx: &mut Context<Self>, f: impl FnOnce(&mut Vec<Leader>)) {
        self.edit_sheet(what, cx, |s| {
            if let Some(l) = s.annotations.iter_mut().find(|a| a.id == id).and_then(note_leaders) {
                f(l);
            }
        });
    }

    /// The right-click menu on a leader line or node.
    pub fn leader_entries(&self, hit: LeaderHit, cx: &App) -> Vec<MenuEntry> {
        let (id, leader) = match hit {
            LeaderHit::Stroke { id, leader, .. } | LeaderHit::Node { id, leader, .. } | LeaderHit::Bend { id, leader } => (id, leader),
        };
        let mut out: Vec<MenuEntry> = crate::drawing_props::paste_entry(cx).into_iter().collect();
        out.push(MenuEntry::item("Copy", move |t, _, cx| {
            t.selection = vec![Sel::Annotation(id)];
            t.copy_entities(cx);
        }));
        out.push(MenuEntry::item("Paste as table", |t, _, cx| {
            t.start_paste_table(cx);
        }));
        if let LeaderHit::Node { node, .. } = hit {
            out.push(MenuEntry::item("Add leader", move |t, _, cx| t.start_branch(id, LeaderStart::Node(leader, node), cx)));
        }
        out.push(MenuEntry::item("Remove leaders", move |t, _, cx| t.edit_leaders("Remove leaders", id, cx, |l| l.clear())));
        match hit {
            LeaderHit::Node { node, .. } => out.push(MenuEntry::item("Remove node", move |t, _, cx| t.edit_leaders("Remove node", id, cx, |l| remove_leader_node(l, leader, node)))),
            LeaderHit::Stroke { seg, p, .. } => {
                let head = self.current_sheet().annotations.iter().find(|a| a.id == id).and_then(|a| self.note_leader_paths_of(a).get(leader).map(|p| p.head)).unwrap_or(2);
                out.push(MenuEntry::item("Add node", move |t, _, cx| {
                    t.edit_leaders("Add node", id, cx, |l| noshape_scene::drawing_tools::annotation_edit::add_node(l, leader, seg, head, p));
                    t.selection = vec![Sel::Annotation(id)];
                    t.leader_sel = Some((id, leader));
                }));
            }
            LeaderHit::Bend { .. } => {}
        }
        out.push(MenuEntry::Separator);
        out.push(MenuEntry::item("Clear selection", |t, _, cx| {
            t.selection.clear();
            t.redraw(cx);
        }));
        out.push(MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)));
        out.push(MenuEntry::item("Delete", move |t, _, cx| t.edit_leaders("Delete leader", id, cx, |l| remove_leader(l, leader))));
        out
    }

    /// Add leader: a new leader of note `id` from `from` follows the pointer
    /// until a click ends it.
    pub fn start_branch(&mut self, id: u64, from: LeaderStart, cx: &mut Context<Self>) {
        self.leader_from = Some(from);
        self.set_tool(Tool::GdtLeader { id }, cx);
    }

    /// The note's Add leader: from the existing landing's bend (live: that
    /// side, wherever the pointer is), else from the note's side.
    pub fn note_add_leader(&mut self, id: u64, cx: &mut Context<Self>) {
        let start = self.current_sheet().annotations.iter().find(|a| a.id == id).map(noshape_scene::drawing_tools::annotation_edit::note_leader_start).unwrap_or_default();
        self.start_branch(id, start, cx);
    }

    /// Where the pending branch of note `id` starts (sheet mm).
    pub fn branch_point(&self, id: u64) -> Option<[f64; 2]> {
        let a = self.current_sheet().annotations.iter().find(|a| a.id == id)?;
        let paths = self.note_leader_paths_of(a);
        let AnnotationKind::Note { style, .. } = &a.kind else { return None };
        match self.leader_from? {
            LeaderStart::Bend(j) => paths.get(j).map(|p| p.bend()),
            LeaderStart::Node(j, n) => style.leaders.get(j).and_then(|l| l.nodes.get(n)).copied(),
            LeaderStart::Side => None,
        }
    }

    /// A drag starting on a leader's bend: the landing's length.
    pub fn leader_bend_drag(&self, start: [f64; 2]) -> Option<Drag> {
        let Some(LeaderHit::Bend { id, leader }) = self.leader_hit(start) else { return None };
        let orig = self.current_sheet().annotations.iter().find(|a| a.id == id)?.clone();
        Some(Drag::LeaderBend { id, leader, start, orig })
    }

    /// The bend follows the pointer horizontally only (live (2)); the leader
    /// keeps its side.
    pub fn leader_bend_move(&mut self, drag: &Drag, cx: &mut Context<Self>) {
        let Drag::LeaderBend { id, leader, start, orig } = drag else { return };
        let Some(path) = self.note_leader_paths_of(orig).get(*leader).cloned() else { return };
        let right = path.points[1][0] >= path.points[0][0];
        let base = (path.points[1][0] - path.points[0][0]).abs();
        let dx = self.cursor[0] - start[0];
        let landing = (base + if right { dx } else { -dx }).max(1.0);
        let mut dr = self.drawing.clone();
        if let Some(a) = dr.sheets.get_mut(self.sheet).and_then(|s| s.annotations.iter_mut().find(|a| a.id == *id)) {
            *a = orig.clone();
            if let Some(ld) = note_leaders(a).and_then(|l| l.get_mut(*leader)) {
                ld.landing = Some(landing);
                ld.right = Some(right);
            }
        }
        self.set_preview(Some(dr), cx);
    }

    /// A note's right-click menu in live's order (drw2 10-21 g (5)): Edit
    /// note..., Explode note, Paste, Copy, Paste as table, Add leader, Remove
    /// leaders, Move to ▸, Bring to front, Send to back | Clear selection,
    /// Zoom to fit, Delete (Add comment is cloud-only).
    pub fn note_entries(&self, a: &Annotation, cx: &App) -> Vec<MenuEntry> {
        let AnnotationKind::Note { style, .. } = &a.kind else { return Vec::new() };
        let (id, pos) = (a.id, a.position);
        let mut out = vec![MenuEntry::item("Edit note...", move |t, w, cx| t.open_note_panel(Some(id), pos, w, cx))];
        let on = style.explode;
        out.push(MenuEntry::check("Explode note", on, move |t, _, cx| {
            t.edit_sheet("Explode note", cx, |s| {
                if let Some(AnnotationKind::Note { style, .. }) = s.annotations.iter_mut().find(|x| x.id == id).map(|x| &mut x.kind) {
                    style.explode = !on;
                }
            })
        }));
        out.extend(crate::drawing_props::paste_entry(cx));
        out.push(MenuEntry::item("Copy", move |t, _, cx| {
            if !t.selection.contains(&Sel::Annotation(id)) {
                t.selection = vec![Sel::Annotation(id)];
            }
            t.copy_entities(cx);
        }));
        out.push(MenuEntry::item("Paste as table", |t, _, cx| {
            t.start_paste_table(cx);
        }));
        out.push(MenuEntry::item("Add leader", move |t, _, cx| t.note_add_leader(id, cx)));
        if !style.leaders.is_empty() {
            out.push(MenuEntry::item("Remove leaders", move |t, _, cx| t.edit_leaders("Remove leaders", id, cx, |l| l.clear())));
        }
        // Move to ▸ a format layer (or back to the drawing).
        let cur = a.layer;
        let layers = noshape_model::drawing::SheetLayer::FORMAT.iter().copied().chain((!cur.is_drawing()).then_some(noshape_model::drawing::SheetLayer::Drawing));
        let items = layers
            .filter(|l| *l != cur)
            .map(|l| {
                MenuEntry::item(l.name(), move |t, _, cx| {
                    let ids: Vec<u64> = t.selection.iter().filter_map(|s| if let Sel::Annotation(x) = s { Some(*x) } else { None }).chain([id]).collect();
                    t.edit_sheet("Move to", cx, |s| s.annotations.iter_mut().filter(|x| ids.contains(&x.id)).for_each(|x| x.layer = l));
                })
            })
            .collect();
        out.push(MenuEntry::Submenu("Move to".into(), items));
        // The annotation list is the draw order: front = last.
        let list = &self.current_sheet().annotations;
        let at = list.iter().position(|x| x.id == id);
        let (front, back) = (at == Some(list.len().saturating_sub(1)), at == Some(0));
        out.push(MenuEntry::item("Bring to front", move |t, _, cx| t.order_annotation(id, true, cx)).disabled(front));
        out.push(MenuEntry::item("Send to back", move |t, _, cx| t.order_annotation(id, false, cx)).disabled(back));
        out.push(MenuEntry::Separator);
        out.push(MenuEntry::item("Clear selection", |t, _, cx| {
            t.selection.clear();
            t.redraw(cx);
        }));
        out.push(MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)));
        out.push(MenuEntry::item("Delete", move |t, _, cx| {
            t.selection.clear();
            t.edit_sheet("Delete annotation", cx, |s| s.annotations.retain(|x| x.id != id));
        }));
        out
    }

    /// Bring to front / Send to back: the annotation drawn last / first.
    fn order_annotation(&mut self, id: u64, front: bool, cx: &mut Context<Self>) {
        self.edit_sheet(if front { "Bring to front" } else { "Send to back" }, cx, |s| {
            if let Some(i) = s.annotations.iter().position(|a| a.id == id) {
                let a = s.annotations.remove(i);
                if front {
                    s.annotations.push(a);
                } else {
                    s.annotations.insert(0, a);
                }
            }
        });
    }

    /// The selected leader (live (1)): amber, every node and the bend an
    /// amber dot, the anchor an amber ring with a blue centre.
    pub fn leader_overlay(&self, o: &mut crate::drawing_scene::Overlay) {
        const AMBER: [u8; 4] = [204, 146, 92, 255];
        const BLUE: [u8; 4] = [0, 136, 204, 255];
        let Some((id, leader)) = self.leader_sel else { return };
        if !self.selection.contains(&Sel::Annotation(id)) {
            return;
        }
        let Some(a) = self.current_sheet().annotations.iter().find(|a| a.id == id) else { return };
        let Some(path) = self.note_leader_paths_of(a).into_iter().nth(leader) else { return };
        o.tinted.push((path.points.clone(), AMBER, 1.5));
        let n = path.points.len();
        for p in &path.points[path.head - 1..n - 1] {
            o.dots.push((*p, AMBER));
        }
        let t = path.points[n - 1];
        // Outside the blue centre dot.
        let r = self.tol_mm(7.0);
        let ring: Vec<[f64; 2]> = (0..=24).map(|i| {
            let a = i as f64 / 24.0 * std::f64::consts::TAU;
            [t[0] + r * a.cos(), t[1] + r * a.sin()]
        }).collect();
        o.tinted.push((ring, AMBER, 1.5));
        o.dots.push((t, BLUE));
    }

    /// The Add leader rubber band's arrowhead at `to`, pointing away from `from`.
    pub fn rubber_band_arrow(&self, o: &mut crate::drawing_scene::Overlay, from: [f64; 2], to: [f64; 2]) {
        let d = [to[0] - from[0], to[1] - from[1]];
        let l = d[0].hypot(d[1]);
        if l < 1e-9 {
            return;
        }
        let (u, w) = ([d[0] / l, d[1] / l], [-d[1] / l, d[0] / l]);
        let (len, half) = (3.0, 1.0);
        let base = [to[0] - u[0] * len, to[1] - u[1] * len];
        o.tinted.push((vec![to, [base[0] + w[0] * half, base[1] + w[1] * half], [base[0] - w[0] * half, base[1] - w[1] * half], to], [0, 0, 0, 255], 1.2));
    }
}
