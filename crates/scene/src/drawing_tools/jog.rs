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

    /// Where arm `k` of the centermark `orig`, dragged near the cursor,
    /// crosses a drawn edge of its view: (sheet mm from the center, the
    /// point). Live A55: a red diamond snap shows there (not with Shift).
    pub fn centermark_edge_snap(&self, orig: &Annotation, k: usize) -> Option<(f64, [f64; 2])> {
        let (c, dirs, _) = self.centermark_arms(orig)?;
        let view = orig.kind.view()?;
        let (pl, g) = (self.placement(view)?, self.geoms.get(&view)?);
        let d = dirs[k];
        let along = dot(sub(self.cursor, c), d);
        let tol = self.tol_mm(8.0);
        let mut best: Option<(f64, [f64; 2])> = None;
        for e in g.edges.iter().filter(|e| e.visible) {
            for w in e.points.windows(2) {
                let (p, q) = (pl.to_sheet(w[0]), pl.to_sheet(w[1]));
                let s = sub(q, p);
                let den = d[0] * s[1] - d[1] * s[0];
                if den.abs() < 1e-12 {
                    continue;
                }
                let r = sub(p, c);
                let (t, u) = ((r[0] * s[1] - r[1] * s[0]) / den, (r[0] * d[1] - r[1] * d[0]) / den);
                if t > 0.5 && (0.0..=1.0).contains(&u) && (t - along).abs() < tol && best.is_none_or(|(b, _)| (t - along).abs() < (b - along).abs()) {
                    best = Some((t, [c[0] + d[0] * t, c[1] + d[1] * t]));
                }
            }
        }
        best
    }

    /// The centermark `orig` with arm `k`'s end dragged to the cursor: all
    /// four arms change by the same amount (the end snapping onto a drawn
    /// edge it crosses), only that arm with Shift (no snap).
    pub fn dragged_centermark(&self, orig: &Annotation, k: usize, shift: bool) -> Annotation {
        let mut a = orig.clone();
        let Some((c, dirs, lens)) = self.centermark_arms(orig) else { return a };
        let snap = if shift { None } else { self.centermark_edge_snap(orig, k) };
        let AnnotationKind::Centermark { view, size, arms, .. } = &mut a.kind else { return a };
        let Some(pl) = self.placement(*view) else { return a };
        let along = snap.map(|s| s.0).unwrap_or_else(|| (self.cursor[0] - c[0]) * dirs[k][0] + (self.cursor[1] - c[1]) * dirs[k][1]);
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

    /// An end grip of a selected centerline under `start`: (centerline,
    /// end 0 / 1). Not an end that follows a model point.
    pub fn centerline_grip_at(&self, start: [f64; 2]) -> Option<(Annotation, usize)> {
        let tol = self.tol_mm(5.0);
        self.selection.iter().find_map(|s| {
            let Sel::Annotation(id) = s else { return None };
            let a = self.resolved_annotation(*id)?;
            let AnnotationKind::Centerline { view, a: pa, b: pb } = &a.kind else { return None };
            let pl = self.placement(*view)?;
            [pl.to_sheet(*pa), pl.to_sheet(*pb)].iter().enumerate().find(|(k, p)| len(sub(start, **p)) < tol && !a.refs.iter().any(|r| r.slot as usize == *k)).map(|(k, _)| (a.clone(), k))
        })
    }

    /// The middle snap of a selected centerline on no model points under
    /// `start` (dragging it moves the centerline; help A51).
    pub fn centerline_middle_at(&self, start: [f64; 2]) -> Option<Annotation> {
        let tol = self.tol_mm(5.0);
        self.selection.iter().find_map(|s| {
            let Sel::Annotation(id) = s else { return None };
            let a = self.resolved_annotation(*id)?;
            let AnnotationKind::Centerline { view, a: pa, b: pb } = &a.kind else { return None };
            let pl = self.placement(*view)?;
            let (p, q) = (pl.to_sheet(*pa), pl.to_sheet(*pb));
            (a.refs.is_empty() && len(sub(start, [(p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0])) < tol).then_some(a)
        })
    }

    /// The centerline `orig` with end `k` dragged to the cursor along its
    /// line: longer or shorter than its references (help A51), at least
    /// 1 mm on the sheet.
    pub fn dragged_centerline(&self, orig: &Annotation, k: usize) -> Annotation {
        let mut a = orig.clone();
        let AnnotationKind::Centerline { view, a: pa, b: pb } = &mut a.kind else { return a };
        let Some(pl) = self.placement(*view) else { return a };
        let (p, q) = (pl.to_sheet(*pa), pl.to_sheet(*pb));
        let (fixed, moved) = if k == 0 { (q, p) } else { (p, q) };
        let u = norm(sub(moved, fixed));
        let t = dot(sub(self.cursor, fixed), u).max(1.0);
        let end = pl.to_view([fixed[0] + u[0] * t, fixed[1] + u[1] * t]);
        if k == 0 {
            *pa = end;
        } else {
            *pb = end;
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
            self.attach_points(&a).into_iter().find(|(_, p)| len(sub(start, *p)) < tol).map(|(k, _)| (a.clone(), k))
        })
    }

    /// A selected dimension's blue grips on its referenced geometry (sheet
    /// mm) and the reference slot each re-attaches: a linear dimension's two
    /// ends; a radial's point on its circle; a 3 point angular's vertex and
    /// leg points; an ordinate's base and measured points (live behaviours
    /// D26).
    pub fn attach_points(&self, a: &Annotation) -> Vec<(usize, [f64; 2])> {
        // A datum's triangle (not one on another annotation).
        if let AnnotationKind::Datum { anchor, on: None, at, .. } = &a.kind {
            let p = at.and_then(|(v, p)| self.placement(v).map(|pl| pl.to_sheet(p))).unwrap_or(*anchor);
            return vec![(0, p)];
        }
        if let Some(f) = self.dimension_frame(a) {
            return vec![(0, f.legs[0].0), (1, f.legs[1].0)];
        }
        let Some(pl) = a.kind.view().and_then(|v| self.placement(v)) else { return Vec::new() };
        match &a.kind {
            AnnotationKind::Centermark { center, .. } => vec![(0, pl.to_sheet(*center))],
            AnnotationKind::RadialDimension { center, radius, angle, .. } => {
                let (c, r) = (pl.to_sheet(*center), pl.len_to_sheet(*radius));
                vec![(0, [c[0] + r * angle.cos(), c[1] + r * angle.sin()])]
            }
            AnnotationKind::AngularDimension { vertex, a: pa, b: pb, three_point: true, .. } => vec![(0, pl.to_sheet(*vertex)), (1, pl.to_sheet(*pa)), (2, pl.to_sheet(*pb))],
            AnnotationKind::OrdinateDimension { origin, point, .. } => vec![(0, pl.to_sheet(*origin)), (1, pl.to_sheet(*point))],
            AnnotationKind::OrdinateGroup { origin, points, .. } => std::iter::once((0, pl.to_sheet(*origin))).chain(points.iter().enumerate().map(|(i, p)| (i + 1, pl.to_sheet(*p)))).collect(),
            _ => Vec::new(),
        }
    }

    /// The dimension `orig` with end `k` re-attached to the snap point under
    /// the cursor (same view), keeping the other end (help: "dragging any of
    /// its grip points to another edge, point or center re-attaches that end").
    pub fn reattached(&self, orig: &Annotation, k: usize) -> Annotation {
        let mut a = orig.clone();
        // A centermark's center onto a circle: that circle's mark (help
        // D40: a dangling one is fixed by its red grip).
        if let AnnotationKind::Centermark { view, center, size, .. } = &mut a.kind {
            if let Some(super::Pick::Circle { view: v, center: c, radius: r, r: rf }) = self.circle_pick(self.cursor)
                && v == *view
            {
                (*center, *size) = (c, r * 1.25);
                a.refs.retain(|x| x.slot != 0);
                if let Some(point) = rf {
                    a.refs.push(noshape_model::drawing::AnnotationRef { slot: 0, point });
                }
            }
            return a;
        }
        // A radial onto another circle: its center and radius.
        if let AnnotationKind::RadialDimension { view, center, radius, .. } = &mut a.kind {
            if let Some(super::Pick::Circle { view: v, center: c, radius: r, r: rf }) = self.circle_pick(self.cursor)
                && v == *view
            {
                (*center, *radius) = (c, r);
                a.refs.retain(|x| x.slot != 0);
                if let Some(point) = rf {
                    a.refs.push(noshape_model::drawing::AnnotationRef { slot: 0, point });
                }
            }
            return a;
        }
        // Midpoints and quadrants too (help D22: available when editing a
        // placed dimension).
        let Some(sp) = self.snap_at_with(self.cursor, true) else { return a };
        // A datum's triangle dropped on view geometry: attached there, the
        // box staying put (qa DWG-DATUM-PASTE-RED: a pasted one is red until then).
        if let AnnotationKind::Datum { anchor, on: None, at, .. } = &mut a.kind {
            if let Some(pl) = self.placement(sp.view) {
                (*anchor, *at) = (pl.to_sheet(sp.p), Some((sp.view, sp.p)));
                a.refs.retain(|r| r.slot != 0);
                if let Some(point) = sp.r {
                    a.refs.push(noshape_model::drawing::AnnotationRef { slot: 0, point });
                }
                a.loose = false;
            }
            return a;
        }
        if a.kind.view() != Some(sp.view) {
            return a;
        }
        let target = match &mut a.kind {
            AnnotationKind::LinearDimension { a: pa, b: pb, .. } => Some(if k == 0 { pa } else { pb }),
            AnnotationKind::AngularDimension { vertex, a: pa, b: pb, three_point: true, .. } => Some(match k {
                0 => vertex,
                1 => pa,
                _ => pb,
            }),
            AnnotationKind::OrdinateDimension { origin, point, .. } => Some(if k == 0 { origin } else { point }),
            AnnotationKind::OrdinateGroup { origin, points, .. } => if k == 0 { Some(origin) } else { points.get_mut(k - 1) },
            _ => None,
        };
        if let Some(t) = target {
            *t = sp.p;
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
    /// The ordinate group member whose value is at sheet point `s` (near
    /// the group's level, `position`), with the measured axis on the sheet.
    fn ordinate_member_at(&self, view: noshape_model::drawing::ViewId, points: &[[f64; 2]], horizontal: bool, shifts: &[f64], position: [f64; 2], s: [f64; 2]) -> Option<(usize, [f64; 2])> {
        let pl = self.placement(view)?;
        let rot = noshape_model::drawing::views::view_rotation(self.current_sheet(), view);
        let u = noshape_model::drawing::views::rotate2(if horizontal { [1.0, 0.0] } else { [0.0, 1.0] }, rot);
        let n = noshape_model::drawing::views::rotate2(if horizontal { [0.0, 1.0] } else { [1.0, 0.0] }, rot);
        let dh = self.drawing.props.dim_text_height;
        if (dot(s, n) - dot(position, n)).abs() > 3.0 * dh {
            return None;
        }
        let along: Vec<f64> = points.iter().map(|p| dot(pl.to_sheet(*p), u)).collect();
        let mut at = noshape_model::drawing::sheet::ordinate_text_positions(&along, dh + 1.5);
        for (t, sh) in at.iter_mut().zip(shifts.iter()) {
            *t += sh;
        }
        noshape_model::drawing::sheet::ordinate_push_apart(&mut at, shifts, dh + 1.5);
        at.iter()
            .enumerate()
            .map(|(i, t)| (i, (t - dot(s, u)).abs()))
            .filter(|(_, d)| *d < dh)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| (i, u))
    }

    /// A jogged ordinate value whose knee (round grip) is at sheet point
    /// `s`: its index, the extension line's direction on the sheet and the
    /// side the values sit on (live request 1662).
    #[allow(clippy::too_many_arguments)]
    fn ordinate_knee_at(&self, view: noshape_model::drawing::ViewId, points: &[[f64; 2]], horizontal: bool, shifts: &[f64], knees: &[f64], position: [f64; 2], s: [f64; 2]) -> Option<(usize, [f64; 2], f64)> {
        use noshape_model::drawing::sheet::{ORDINATE_KNEE, ordinate_push_apart, ordinate_text_positions};
        let pl = self.placement(view)?;
        let rot = noshape_model::drawing::views::view_rotation(self.current_sheet(), view);
        let u = noshape_model::drawing::views::rotate2(if horizontal { [1.0, 0.0] } else { [0.0, 1.0] }, rot);
        let n = noshape_model::drawing::views::rotate2(if horizontal { [0.0, 1.0] } else { [1.0, 0.0] }, rot);
        let dh = self.drawing.props.dim_text_height;
        let pts: Vec<[f64; 2]> = points.iter().map(|p| pl.to_sheet(*p)).collect();
        if pts.is_empty() {
            return None;
        }
        let level = dot(position, n);
        let mean = pts.iter().map(|p| dot(*p, n)).sum::<f64>() / pts.len() as f64;
        let sg = if level >= mean { 1.0 } else { -1.0 };
        let along: Vec<f64> = pts.iter().map(|p| dot(*p, u)).collect();
        let mut at = ordinate_text_positions(&along, dh + 1.5);
        for (t, sh) in at.iter_mut().zip(shifts.iter()) {
            *t += sh;
        }
        ordinate_push_apart(&mut at, shifts, dh + 1.5);
        (0..pts.len())
            .filter(|&i| (at[i] - along[i]).abs() >= 1e-6)
            .map(|i| {
                let l0 = dot(pts[i], n) + sg;
                let k = level - sg * knees.get(i).copied().filter(|k| *k > 0.0).unwrap_or(ORDINATE_KNEE);
                let k = if (k - l0) * sg < 0.0 { l0 } else { k };
                let p = [u[0] * along[i] + n[0] * k, u[1] * along[i] + n[1] * k];
                (i, (p[0] - s[0]).hypot(p[1] - s[1]))
            })
            .filter(|(_, d)| *d < 1.5)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| (i, n, sg))
    }

    pub fn dragged_annotation(&self, orig: &Annotation, start: [f64; 2]) -> Annotation {
        let c = self.cursor;
        let d = sub(c, start);
        let mut a = orig.clone();
        let pl = a.kind.view().and_then(|v| self.placement(v));
        match &mut a.kind {
            AnnotationKind::LinearDimension { a: pa, b: pb, offset, orientation, .. } => {
                if let Some(pl) = pl {
                    let (p, q) = (pl.to_sheet(*pa), pl.to_sheet(*pb));
                    *offset = dim_offset(p, q, *orientation, c);
                    // The value follows the pointer along the line (D24; live
                    // asbdt-1a…c); back near the middle it is centred again.
                    let dir = match orientation {
                        noshape_model::drawing::DimOrientation::Horizontal => [1.0, 0.0],
                        noshape_model::drawing::DimOrientation::Vertical => [0.0, 1.0],
                        noshape_model::drawing::DimOrientation::Aligned => norm(sub(q, p)),
                    };
                    let s = orig.text_along.unwrap_or(0.0) + dot(d, dir);
                    a.text_along = (s.abs() > 1.0).then_some(s);
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
            AnnotationKind::SurfaceFinish { anchor, .. } | AnnotationKind::RevisionCallout { anchor, .. } => {
                *anchor = [anchor[0] + d[0], anchor[1] + d[1]];
            }
            AnnotationKind::Gdt { anchor, extra, .. } => {
                *anchor = [anchor[0] + d[0], anchor[1] + d[1]];
                // Under a dimension's value it stays bound to it, sliding
                // along its line (the drag across ignored; live request
                // 1624); on an extension line it moves freely.
                let seg = extra.on.as_ref().filter(|o| o.centered || o.beside).and_then(|o| self.attach_segment(&o.at));
                if let Some(o) = &mut extra.on {
                    match seg {
                        Some((w0, w1)) => {
                            let u = norm(sub(w1, w0));
                            let s0 = if o.beside { o.offset[0] } else { dot(o.offset, u) };
                            *o = noshape_model::drawing::GdtOn { offset: [s0 + dot(d, u), 0.0], centered: false, beside: true, ..*o };
                        }
                        None => o.offset = [o.offset[0] + d[0], o.offset[1] + d[1]],
                    }
                }
            }
            // An edge to edge centerline (no model points) moves by its
            // middle snap (help A51).
            AnnotationKind::Centerline { a: pa, b: pb, .. } if orig.refs.is_empty() => {
                if let Some(pl) = pl {
                    *pa = pl.to_view(add2(pl.to_sheet(*pa), d));
                    *pb = pl.to_view(add2(pl.to_sheet(*pb), d));
                }
            }
            // A jogged value's knee grip slides along its extension line
            // only: the bend height, the value staying put (live request
            // 1662).
            AnnotationKind::OrdinateGroup { view, points, horizontal, shifts, knees, .. } if self.ordinate_knee_at(*view, points, *horizontal, shifts, knees, orig.position, start).is_some() => {
                let (i, n, sg) = self.ordinate_knee_at(*view, points, *horizontal, shifts, knees, orig.position, start).expect("checked");
                knees.resize(points.len(), 0.0);
                let cur = if knees[i] > 0.0 { knees[i] } else { noshape_model::drawing::sheet::ORDINATE_KNEE };
                knees[i] = (cur - sg * dot(d, n)).max(3.5);
            }
            // A value under the pointer moves sideways alone, its extension
            // line jogging over to it (live request 1603); elsewhere the
            // group moves.
            AnnotationKind::OrdinateGroup { view, points, horizontal, shifts, .. } if self.ordinate_member_at(*view, points, *horizontal, shifts, orig.position, start).is_some() => {
                let (i, u) = self.ordinate_member_at(*view, points, *horizontal, shifts, orig.position, start).expect("checked");
                shifts.resize(points.len(), 0.0);
                shifts[i] += dot(d, u);
            }
            _ => a.position = [orig.position[0] + d[0], orig.position[1] + d[1]],
        }
        // An angular dimension's label goes where it is dragged (the arc
        // follows round to it; live D11).
        if let Some(pl) = a.kind.view().and_then(|v| self.placement(v))
            && let Some((v, pa, pb)) = a.kind.angular_legs()
        {
            let (v, pa, pb) = (pl.to_sheet(v), pl.to_sheet(pa), pl.to_sheet(pb));
            let ang = |p: [f64; 2]| (p[1] - v[1]).atan2(p[0] - v[0]);
            let a0 = ang(pa);
            let a1 = a0 + (ang(pb) - a0).rem_euclid(std::f64::consts::TAU);
            let rel = (ang(c) - (a0 + a1) / 2.0 + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
            if let AnnotationKind::AngularDimension { text_angle, .. } = &mut a.kind {
                *text_angle = (rel.abs() > 1e-9).then_some(rel);
            }
        }
        // Keep the stored (unresolved) inputs of the committed annotation;
        // only the placement changes.
        if let Some(committed) = self.drawing.sheets.get(self.sheet).and_then(|s| s.annotations.iter().find(|x| x.id == a.id)) {
            let mut out = committed.clone();
            out.position = a.position;
            out.text_along = a.text_along;
            match (&mut out.kind, &a.kind) {
                (AnnotationKind::LinearDimension { offset, .. }, AnnotationKind::LinearDimension { offset: o, .. }) => *offset = *o,
                (AnnotationKind::RadialDimension { angle, knee, .. }, AnnotationKind::RadialDimension { angle: g, knee: k, .. }) => {
                    *angle = *g;
                    *knee = *k;
                }
                (AnnotationKind::AngularDimension { radius, a: pa, b: pb, text_angle, .. }, AnnotationKind::AngularDimension { radius: r, a: na, text_angle: ta, .. }) => {
                    *radius = *r;
                    *text_angle = *ta;
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
                (AnnotationKind::Gdt { anchor, extra, .. }, AnnotationKind::Gdt { anchor: n, extra: e, .. }) => {
                    *anchor = *n;
                    extra.on = e.on;
                }
                (AnnotationKind::RevisionCallout { anchor, .. }, AnnotationKind::RevisionCallout { anchor: n, .. }) => *anchor = *n,
                (AnnotationKind::MaxMinDimension { offset, .. }, AnnotationKind::MaxMinDimension { offset: o, .. }) => *offset = *o,
                (AnnotationKind::OrdinateGroup { shifts, knees, .. }, AnnotationKind::OrdinateGroup { shifts: s, knees: k, .. }) => {
                    shifts.clone_from(s);
                    knees.clone_from(k);
                }
                (AnnotationKind::ArcLengthDimension { mid, offset, .. }, AnnotationKind::ArcLengthDimension { mid: m, offset: o, .. }) => {
                    *mid = *m;
                    *offset = *o;
                }
                (AnnotationKind::ForeshortenedRadial { jog, .. }, AnnotationKind::ForeshortenedRadial { jog: j, .. }) => *jog = *j,
                (AnnotationKind::Centerline { a: pa, b: pb, .. }, AnnotationKind::Centerline { a: na, b: nb, .. }) if orig.refs.is_empty() => (*pa, *pb) = (*na, *nb),
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
