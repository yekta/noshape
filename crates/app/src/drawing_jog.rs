//! Drawing editor: linear dimensions' context menu as Onshape's (live
//! dom/context-menus-drawing.txt "rmb-dw-dimension", help addjog-contextmenu):
//! Edit…, Paste as table, Break / Unbreak dimension, Add jog (on an
//! extension line) | Clear selection, Zoom to fit, Delete; and dragging a
//! jog's snap points (help jog-dimensionline-highlightex).

use crate::drawing_view::*;
use noshape_model::drawing::sheet::{LinearFrame, linear_frame};
use noshape_model::drawing::{Annotation, AnnotationKind, ExtJog};

/// Distance from `c` to segment `a`-`b`.
fn seg_dist(c: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let l2 = d[0] * d[0] + d[1] * d[1];
    let t = if l2 > 1e-18 { (((c[0] - a[0]) * d[0] + (c[1] - a[1]) * d[1]) / l2).clamp(0.0, 1.0) } else { 0.0 };
    let p = [a[0] + d[0] * t, a[1] + d[1] * t];
    ((c[0] - p[0]).powi(2) + (c[1] - p[1]).powi(2)).sqrt()
}

/// A centermark's center, arm ends (+x, +y, −x, −y) and arm lengths.
type CentermarkArms = ([f64; 2], [[f64; 2]; 4], [f64; 4]);

impl DrawingEditor {
    /// A linear dimension's geometry on the sheet (resolved positions).
    pub fn dimension_frame(&self, a: &Annotation) -> Option<LinearFrame> {
        let AnnotationKind::LinearDimension { view, a: pa, b: pb, offset, orientation, .. } = &a.kind else { return None };
        let pl = self.placement(*view)?;
        let p = &self.drawing.props;
        Some(linear_frame(pl.to_sheet(*pa), pl.to_sheet(*pb), *offset, *orientation, &a.jogs, p.lock_jog_45, p.dim_geometry_gap, p.dim_extension))
    }

    /// The resolved annotation `id` (model geometry followed).
    fn resolved_annotation(&self, id: u64) -> Option<Annotation> {
        self.built.as_ref().and_then(|b| b.resolved.annotations.iter().find(|a| a.id == id).cloned()).or_else(|| self.current_sheet().annotations.iter().find(|a| a.id == id).cloned())
    }

    /// The extension line (0 / 1) of linear dimension `a` under the cursor.
    fn extension_line_at(&self, a: &Annotation) -> Option<usize> {
        let f = self.dimension_frame(a)?;
        let tol = self.tol_mm(6.0);
        (0..2).filter_map(|k| {
            let (src, u, l) = f.legs[k];
            // The leg, jogged or not: model point to the dimension line.
            let pts = match a.jogs[k] {
                Some(j) => {
                    let drop = if self.drawing.props.lock_jog_45 { j.shift.abs() } else { j.drop };
                    let j0 = [src[0] + u[0] * l * j.at, src[1] + u[1] * l * j.at];
                    let j1 = [j0[0] + u[0] * drop + f.dir[0] * j.shift, j0[1] + u[1] * drop + f.dir[1] * j.shift];
                    vec![src, j0, j1, [src[0] + u[0] * l + f.dir[0] * j.shift, src[1] + u[1] * l + f.dir[1] * j.shift]]
                }
                None => vec![src, [src[0] + u[0] * l, src[1] + u[1] * l]],
            };
            let d = pts.windows(2).map(|w| seg_dist(self.cursor, w[0], w[1])).fold(f64::INFINITY, f64::min);
            (d < tol).then_some((k, d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(k, _)| k)
    }

    /// Onshape's menu of a linear dimension (None for other annotations).
    pub fn dimension_entries(&self, id: u64) -> Option<Vec<MenuEntry>> {
        let a = self.resolved_annotation(id)?;
        if !matches!(a.kind, AnnotationKind::LinearDimension { .. }) || a.inspection.is_some() {
            return None;
        }
        let mut out = Vec::new();
        let a2 = a.clone();
        out.push(MenuEntry::item("Edit...", move |t, w, cx| t.open_annotation_panel(&a2, w, cx)));
        // Live: always present.
        out.push(MenuEntry::item("Paste as table", |t, _, cx| {
            t.start_paste_table(cx);
        }));
        let unbroken = a.unbroken;
        out.push(MenuEntry::item(if unbroken { "Break dimension" } else { "Unbreak dimension" }, move |t, _, cx| {
            t.edit_sheet(if unbroken { "Break dimension" } else { "Unbreak dimension" }, cx, |s| s.annotations.iter_mut().filter(|x| x.id == id).for_each(|x| x.unbroken = !unbroken))
        }));
        if let Some(k) = self.extension_line_at(&a) {
            if a.jogs[k].is_some() {
                out.push(MenuEntry::item("Remove jog", move |t, _, cx| t.edit_sheet("Remove jog", cx, |s| s.annotations.iter_mut().filter(|x| x.id == id).for_each(|x| x.jogs[k] = None))));
            } else {
                // Live drawing/331–340: a 45° diagonal 0.44 in (11.2 mm)
                // outward, centered on the line; a line shorter than about
                // 44.7 mm first moves the dimension line away from the part.
                let f = self.dimension_frame(&a)?;
                let along = |p: [f64; 2]| p[0] * f.dir[0] + p[1] * f.dir[1];
                let (mine, other) = if k == 0 { (f.dp, f.dq) } else { (f.dq, f.dp) };
                const JOG: f64 = 11.2;
                const MIN_LEG: f64 = 44.7;
                let shift = if along(mine) <= along(other) { -JOG } else { JOG };
                let l = f.legs[k].2;
                let grow = (MIN_LEG - l).max(0.0);
                let at = ((l + grow - JOG) / 2.0) / (l + grow).max(1e-9);
                out.push(MenuEntry::item("Add jog", move |t, _, cx| {
                    t.edit_sheet("Add jog", cx, |s| {
                        for x in s.annotations.iter_mut().filter(|x| x.id == id) {
                            if let AnnotationKind::LinearDimension { offset, .. } = &mut x.kind {
                                *offset += grow * offset.signum();
                            }
                            x.jogs[k] = Some(ExtJog { at, shift, drop: JOG });
                        }
                    })
                }));
            }
        }
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
        Some(out)
    }

    /// An ordinate dimension's menu (live behaviours.md "Ordinate dimensions"):
    /// Add to ordinate dimension group · Edit... · Paste · Paste as table ·
    /// Reset ordinate · Unbreak dimension | Clear selection · Zoom to fit ·
    /// Delete. (Add comment is a cloud feature.)
    pub fn ordinate_entries(&self, id: u64) -> Option<Vec<MenuEntry>> {
        let a = self.resolved_annotation(id)?;
        let AnnotationKind::OrdinateGroup { view, origin, horizontal, .. } = a.kind else { return None };
        let r = a.refs.iter().find(|x| x.slot == 0).map(|x| x.point.clone());
        let mut out = vec![MenuEntry::item("Add to ordinate dimension group", move |t, _, cx| {
            let o = PickedPoint { view, p: origin, r: r.clone() };
            t.set_tool(Tool::Ordinate { origin: Some(o), horizontal: Some(horizontal), group: Some(id) }, cx)
        })];
        let a2 = a.clone();
        out.push(MenuEntry::item("Edit...", move |t, w, cx| t.open_annotation_panel(&a2, w, cx)));
        out.push(MenuEntry::item("Paste", |t, _, cx| t.paste_views(cx)));
        out.push(MenuEntry::item("Paste as table", |t, _, cx| {
            t.start_paste_table(cx);
        }));
        // The values sit on the group's text line (no moved values to reset).
        out.push(MenuEntry::item("Reset ordinate", |_, _, _| {}));
        let unbroken = a.unbroken;
        out.push(MenuEntry::item(if unbroken { "Break dimension" } else { "Unbreak dimension" }, move |t, _, cx| {
            t.edit_sheet(if unbroken { "Break dimension" } else { "Unbreak dimension" }, cx, |s| s.annotations.iter_mut().filter(|x| x.id == id).for_each(|x| x.unbroken = !unbroken))
        }));
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
        Some(out)
    }

    /// A jog snap point of the selected dimension under `start`: (dimension,
    /// extension line, the far point).
    pub fn jog_grip_at(&self, start: [f64; 2]) -> Option<(Annotation, usize, bool)> {
        let tol = self.tol_mm(5.0);
        self.selection.iter().find_map(|s| {
            let Sel::Annotation(id) = s else { return None };
            let a = self.resolved_annotation(*id)?;
            let f = self.dimension_frame(&a)?;
            // jog_points: (start, end) per jogged extension line, in order.
            let mut i = 0;
            for k in 0..2 {
                if a.jogs[k].is_none() {
                    continue;
                }
                for far in [false, true] {
                    let p = f.jog_points.get(i).copied()?;
                    i += 1;
                    if seg_dist(start, p, p) < tol {
                        return Some((a.clone(), k, far));
                    }
                }
            }
            None
        })
    }

    /// A centermark's arm ends (+x, +y, −x, −y) on the sheet, its center and
    /// arm directions.
    fn centermark_arms(&self, a: &Annotation) -> Option<CentermarkArms> {
        let AnnotationKind::Centermark { view, center, size, angle, arms } = &a.kind else { return None };
        let pl = self.placement(*view)?;
        let c = pl.to_sheet(*center);
        let rot = pl.rotation + angle;
        let (u, w) = ([rot.cos(), rot.sin()], [-rot.sin(), rot.cos()]);
        let dirs = [u, w, [-u[0], -u[1]], [-w[0], -w[1]]];
        let lens = [0, 1, 2, 3].map(|i| pl.len_to_sheet(size + arms[i]));
        Some((c, dirs, lens))
    }

    /// An arm-end grip of a selected centermark under `start`: (mark, arm).
    pub fn centermark_grip_at(&self, start: [f64; 2]) -> Option<(Annotation, usize)> {
        let tol = self.tol_mm(5.0);
        self.selection.iter().find_map(|s| {
            let Sel::Annotation(id) = s else { return None };
            let a = self.resolved_annotation(*id)?;
            let (c, dirs, lens) = self.centermark_arms(&a)?;
            (0..4).find(|k| seg_dist(start, [c[0] + dirs[*k][0] * lens[*k], c[1] + dirs[*k][1] * lens[*k]], [c[0] + dirs[*k][0] * lens[*k], c[1] + dirs[*k][1] * lens[*k]]) < tol).map(|k| (a.clone(), k))
        })
    }

    /// The centermark `orig` with arm `k`'s end dragged to the cursor: all
    /// four arms change by the same amount, only that arm with Shift.
    pub fn dragged_centermark(&self, orig: &Annotation, k: usize, shift: bool) -> Annotation {
        let mut a = orig.clone();
        let Some((c, dirs, lens)) = self.centermark_arms(orig) else { return a };
        let AnnotationKind::Centermark { view, size, arms, .. } = &mut a.kind else { return a };
        let Some(pl) = self.placement(*view) else { return a };
        let along = (self.cursor[0] - c[0]) * dirs[k][0] + (self.cursor[1] - c[1]) * dirs[k][1];
        // Sheet mm -> view units.
        let per = pl.len_to_sheet(1.0).max(1e-12);
        let delta = (along.max(0.5) - lens[k]) / per;
        let min = -*size * 0.9;
        if shift {
            arms[k] = (arms[k] + delta).max(min);
        } else {
            for x in arms.iter_mut() {
                *x = (*x + delta).max(min);
            }
        }
        a
    }

    /// An attachment point (light-blue grip) of a selected linear dimension
    /// under `start`: (dimension, end 0 / 1).
    pub fn attach_grip_at(&self, start: [f64; 2]) -> Option<(Annotation, usize)> {
        let tol = self.tol_mm(5.0);
        self.selection.iter().find_map(|s| {
            let Sel::Annotation(id) = s else { return None };
            let a = self.resolved_annotation(*id)?;
            let f = self.dimension_frame(&a)?;
            (0..2).find(|k| seg_dist(start, f.legs[*k].0, f.legs[*k].0) < tol).map(|k| (a.clone(), k))
        })
    }

    /// The dimension `orig` with end `k` re-attached to the snap point under
    /// the cursor (same view), keeping the other end (help: "dragging any of
    /// its grip points to another edge, point or center re-attaches that end").
    pub fn reattached(&self, orig: &Annotation, k: usize) -> Annotation {
        let mut a = orig.clone();
        let Some(sp) = self.snap_at(self.cursor) else { return a };
        if let AnnotationKind::LinearDimension { view, a: pa, b: pb, .. } = &mut a.kind {
            if sp.view != *view {
                return a;
            }
            *(if k == 0 { pa } else { pb }) = sp.p;
            a.refs.retain(|r| r.slot != k as u8);
            if let Some(point) = sp.r {
                a.refs.push(noshape_model::drawing::AnnotationRef { slot: k as u8, point });
            }
        }
        a
    }

    /// The dimension `orig` with jog `k`'s snap point (`far`: the end of
    /// the diagonal, else its start) dragged to the cursor.
    pub fn dragged_jog(&self, orig: &Annotation, k: usize, far: bool) -> Annotation {
        let mut a = orig.clone();
        let (Some(f), Some(mut j)) = (self.dimension_frame(orig), orig.jogs[k]) else { return a };
        let (src, u, l) = f.legs[k];
        let c = self.cursor;
        let rel = [c[0] - src[0], c[1] - src[1]];
        let along_u = rel[0] * u[0] + rel[1] * u[1];
        if far {
            // The diagonal's end: sideways = shift, further along = drop.
            let j0 = l * j.at;
            j.shift = rel[0] * f.dir[0] + rel[1] * f.dir[1];
            j.drop = (along_u - j0).max(0.0);
        } else if l > 1e-9 {
            j.at = (along_u / l).clamp(0.02, 0.98);
        }
        a.jogs[k] = Some(j);
        a
    }
}
