//! Drawing slice 3: grips of selected annotations (dimension jogs and
//! attachment points, centermark arms) and annotations dragged by the
//! pointer, from `crates/app/src/drawing_jog.rs` and `drawing_tools.rs`.

use super::geom::*;
use super::{Sel, SheetPick};
use noshape_model::drawing::sheet::{LinearFrame, linear_frame};
use noshape_model::drawing::{Annotation, AnnotationKind};

/// Distance from `c` to segment `a`-`b`.
/// An arc length dimension placed (or dragged) at sheet point `at`: its
/// `mid` (view coordinates, the point on the circle toward `at`, so the
/// side of the arc the pointer is on is measured: across the arc's ends,
/// the conjugate; help drawing_-_arc_length_dimension) and its offset from
/// the arc (sheet mm).
pub fn arc_length_place(pl: &noshape_model::drawing::build::ViewPlacement, center: [f64; 2], radius: f64, at: [f64; 2]) -> ([f64; 2], f64) {
    let c = pl.to_sheet(center);
    let v = sub(at, c);
    let offset = (len(v) - pl.len_to_sheet(radius)).max(2.0);
    let mid_sheet = if len(v) > 1e-9 { [c[0] + v[0] / len(v) * pl.len_to_sheet(radius), c[1] + v[1] / len(v) * pl.len_to_sheet(radius)] } else { c };
    (pl.to_view(mid_sheet), offset)
}

pub fn seg_dist(c: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let l2 = d[0] * d[0] + d[1] * d[1];
    let t = if l2 > 1e-18 { (((c[0] - a[0]) * d[0] + (c[1] - a[1]) * d[1]) / l2).clamp(0.0, 1.0) } else { 0.0 };
    let p = [a[0] + d[0] * t, a[1] + d[1] * t];
    ((c[0] - p[0]).powi(2) + (c[1] - p[1]).powi(2)).sqrt()
}

/// A centermark's center, arm ends (+x, +y, −x, −y) and arm lengths.
pub type CentermarkArms = ([f64; 2], [[f64; 2]; 4], [f64; 4]);

impl SheetPick<'_> {
    /// A linear dimension's geometry on the sheet (resolved positions).
    pub fn dimension_frame(&self, a: &Annotation) -> Option<LinearFrame> {
        let AnnotationKind::LinearDimension { view, a: pa, b: pb, offset, orientation, .. } = &a.kind else { return None };
        let pl = self.placement(*view)?;
        let p = &self.drawing.props;
        Some(linear_frame(pl.to_sheet(*pa), pl.to_sheet(*pb), *offset, *orientation, &a.jogs, p.lock_jog_45, p.dim_geometry_gap, p.dim_extension))
    }

    /// The resolved annotation `id` (model geometry followed).
    pub fn resolved_annotation(&self, id: u64) -> Option<Annotation> {
        self.built.and_then(|b| b.resolved.annotations.iter().find(|a| a.id == id).cloned()).or_else(|| self.current_sheet().annotations.iter().find(|a| a.id == id).cloned())
    }

    /// The extension line (0 / 1) of linear dimension `a` under the cursor.
    pub fn extension_line_at(&self, a: &Annotation) -> Option<usize> {
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
    pub fn centermark_arms(&self, a: &Annotation) -> Option<CentermarkArms> {
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
        // Midpoints and quadrants too (help D22: available when editing a
        // placed dimension).
        let Some(sp) = self.snap_at_with(self.cursor, true) else { return a };
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

    /// The annotation `orig` (resolved) dragged from `start` to the cursor.
    pub fn dragged_annotation(&self, orig: &Annotation, start: [f64; 2]) -> Annotation {
        let c = self.cursor;
        let d = sub(c, start);
        let mut a = orig.clone();
        let pl = a.kind.view().and_then(|v| self.placement(v));
        match &mut a.kind {
            AnnotationKind::LinearDimension { a: pa, b: pb, offset, orientation, .. } => {
                if let Some(pl) = pl {
                    *offset = dim_offset(pl.to_sheet(*pa), pl.to_sheet(*pb), *orientation, c);
                }
            }
            AnnotationKind::RadialDimension { view, center, radius, angle, knee, .. } => {
                if let Some(pl) = pl {
                    let v = sub(c, pl.to_sheet(*center));
                    *angle = v[1].atan2(v[0]);
                    *knee = self.radial_knee(*view, *center, *radius, c);
                }
            }
            AnnotationKind::AngularDimension { vertex, radius, a: pa, b: pb, three_point, .. } => {
                if let Some(pl) = &pl {
                    *radius = len(sub(c, pl.to_sheet(*vertex))).max(3.0);
                    // 3 point angular: across a leg, the arc measures 360° minus the angle.
                    if *three_point && !in_sweep(pl.to_sheet(*vertex), pl.to_sheet(*pa), pl.to_sheet(*pb), c) {
                        std::mem::swap(pa, pb);
                        for r in &mut a.refs {
                            r.slot = match r.slot {
                                1 => 2,
                                2 => 1,
                                s => s,
                            };
                        }
                    }
                }
            }
            AnnotationKind::MaxMinDimension { a: pa, b: pb, offset, orientation, .. } => {
                if let Some(pl) = pl {
                    *offset = dim_offset(pl.to_sheet(*pa), pl.to_sheet(*pb), *orientation, c);
                }
            }
            AnnotationKind::ArcLengthDimension { center, radius, mid, offset, .. } => {
                if let Some(pl) = pl {
                    (*mid, *offset) = arc_length_place(&pl, *center, *radius, c);
                }
            }
            AnnotationKind::ForeshortenedRadial { jog, .. } => {
                // Dragging moves the false center.
                if let Some(pl) = pl {
                    *jog = pl.to_view(add2(pl.to_sheet(*jog), d));
                }
            }
            AnnotationKind::SurfaceFinish { anchor, .. } | AnnotationKind::Gdt { anchor, .. } | AnnotationKind::RevisionCallout { anchor, .. } => {
                *anchor = [anchor[0] + d[0], anchor[1] + d[1]];
            }
            _ => a.position = [orig.position[0] + d[0], orig.position[1] + d[1]],
        }
        // Keep the stored (unresolved) inputs of the committed annotation;
        // only the placement changes.
        if let Some(committed) = self.drawing.sheets.get(self.sheet).and_then(|s| s.annotations.iter().find(|x| x.id == a.id)) {
            let mut out = committed.clone();
            out.position = a.position;
            match (&mut out.kind, &a.kind) {
                (AnnotationKind::LinearDimension { offset, .. }, AnnotationKind::LinearDimension { offset: o, .. }) => *offset = *o,
                (AnnotationKind::RadialDimension { angle, knee, .. }, AnnotationKind::RadialDimension { angle: g, knee: k, .. }) => {
                    *angle = *g;
                    *knee = *k;
                }
                (AnnotationKind::AngularDimension { radius, a: pa, b: pb, .. }, AnnotationKind::AngularDimension { radius: r, a: na, .. }) => {
                    *radius = *r;
                    // Legs swapped by the drag: swap the stored legs and their references too.
                    let swapped = matches!(&orig.kind, AnnotationKind::AngularDimension { a: oa, b: ob, .. } if na == ob && oa != ob);
                    if swapped {
                        std::mem::swap(pa, pb);
                        for r in &mut out.refs {
                            r.slot = match r.slot {
                                1 => 2,
                                2 => 1,
                                s => s,
                            };
                        }
                    }
                }
                (AnnotationKind::SurfaceFinish { anchor, .. }, AnnotationKind::SurfaceFinish { anchor: n, .. }) => *anchor = *n,
                (AnnotationKind::Gdt { anchor, .. }, AnnotationKind::Gdt { anchor: n, .. }) => *anchor = *n,
                (AnnotationKind::RevisionCallout { anchor, .. }, AnnotationKind::RevisionCallout { anchor: n, .. }) => *anchor = *n,
                (AnnotationKind::MaxMinDimension { offset, .. }, AnnotationKind::MaxMinDimension { offset: o, .. }) => *offset = *o,
                (AnnotationKind::ArcLengthDimension { mid, offset, .. }, AnnotationKind::ArcLengthDimension { mid: m, offset: o, .. }) => {
                    *mid = *m;
                    *offset = *o;
                }
                (AnnotationKind::ForeshortenedRadial { jog, .. }, AnnotationKind::ForeshortenedRadial { jog: j, .. }) => *jog = *j,
                _ => {}
            }
            return out;
        }
        a
    }
}

#[cfg(test)]
mod arc_length_tests {
    use noshape_model::drawing::ViewId;
    use noshape_model::drawing::build::ViewPlacement;

    /// The quarter arc from 0° to 90° (radius 10 mm in a 1:1 view): placed
    /// beside it, `mid` is on it; placed across its ends, on the conjugate.
    #[test]
    fn arc_length_follows_the_side_the_pointer_is_on() {
        let pl = ViewPlacement::new(ViewId(1), [0.0, 0.0], [100.0, 100.0], 1.0);
        let c = pl.to_sheet([0.0, 0.0]);
        let r = pl.len_to_sheet(0.01);
        let angle = |at: [f64; 2]| {
            let (m, off) = super::arc_length_place(&pl, [0.0, 0.0], 0.01, at);
            assert!(off >= 2.0);
            m[1].atan2(m[0]).to_degrees().rem_euclid(360.0)
        };
        let k = (r + 8.0) / 2f64.sqrt();
        assert!((angle([c[0] + k, c[1] + k]) - 45.0).abs() < 1.0);
        assert!((angle([c[0] - k, c[1] - k]) - 225.0).abs() < 1.0);
    }
}
