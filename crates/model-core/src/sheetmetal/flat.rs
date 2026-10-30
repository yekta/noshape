//! Flat pattern: unfold a sheet metal body into the plane of its base wall.

use super::SheetMetalData;
use crate::drawing::sheet::{Anchor, LineStyle, Polyline2, SheetGeometry, Text2};
use crate::reference::BodyId;
use crate::regen::context::{Body, Context};
use glam::{DMat4, DVec2, DVec3};
use noshape_kernel as k;
use noshape_kernel::{Shape, ShapeType};
use std::collections::VecDeque;

/// A bend line of the flat pattern (center of the developed bend region).
#[derive(Clone, Debug, PartialEq)]
pub struct FlatBendLine {
    pub a: DVec2,
    pub b: DVec2,
    /// Bend toward the viewer (the flat is seen from the reference side).
    pub up: bool,
    pub angle: f64,
    pub radius: f64,
    pub k_factor: f64,
    /// Developed length (width of the bend region in the flat).
    pub allowance: f64,
    /// Index into [`SheetMetalData::bends`].
    pub bend: usize,
}

/// A form shown in the flat pattern: its outline and center mark.
#[derive(Clone, Debug, PartialEq)]
pub struct FlatForm {
    /// Closed outlines (a form without a flat sketch: its tools' widest
    /// sections), drawn as its up / down outline.
    pub loops: Vec<Vec<DVec2>>,
    /// The form studio's flat sketch (live 1623): open curves, `true` a
    /// tear (a cut through the sheet: the sketch's own geometry, solid,
    /// SHEETMETAL_FORM_TEAR), else an outline (its construction geometry,
    /// the up / down dash-dot).
    pub curves: Vec<(Vec<DVec2>, bool)>,
    pub center: DVec2,
    /// It stands out toward the viewer (the flat's reference side): live
    /// draws "up" forms always, "down" ones with hidden lines only.
    pub up: bool,
}

/// A counterbore / countersink head shown in the flat pattern: the head's
/// circle around the through hole (live: Flat pattern views ›
/// Counterbore/Countersink up / down).
#[derive(Clone, Debug, PartialEq)]
pub struct FlatHead {
    pub center: DVec2,
    pub radius: f64,
    /// The head is on the side the flat is seen from: live draws it always,
    /// one on the other side with hidden lines only.
    pub up: bool,
}

/// The unfolded part: outline loops and bend lines in the flat plane.
#[derive(Clone, Debug)]
pub struct FlatPattern {
    /// Flat plane in model space (origin, viewing normal = reference side
    /// outward normal of the base wall, X axis).
    pub frame: k::Plane,
    /// Closed outline loops in plane coordinates (meters); outer boundaries
    /// and holes. The first loop has the largest extent.
    pub loops: Vec<Vec<DVec2>>,
    pub bend_lines: Vec<FlatBendLine>,
    /// Form outlines and center marks (drawn over the flat region).
    pub forms: Vec<FlatForm>,
    /// Counterbore / countersink heads (the flat itself shows the through hole).
    pub heads: Vec<FlatHead>,
    /// The flat region as planar face(s) in model space.
    pub shape: Option<Shape>,
    pub thickness: f64,
    /// Non-fatal problems (closed loops, unresolved walls ...).
    pub warnings: Vec<String>,
    /// The part it was built for ([`body_key`]): a std-built flat (made when
    /// its sheet metal feature rebuilt the part) holds only for that shape.
    pub built_for: Option<[f64; 8]>,
}

impl FlatPattern {
    /// Model space → the flat view's place: the flat plane's X / Y on the
    /// world's, its reference side at z 0 facing +Z (live REST
    /// isFlattenedBody of "qa5 sm": z −2…0; the flat view shows it from Top).
    fn flat_view_matrix(&self) -> DMat4 {
        let (x, y, n) = (self.frame.unit_x(), self.frame.unit_y(), self.frame.unit_x().cross(self.frame.unit_y()));
        DMat4::from_cols(x.extend(0.0), y.extend(0.0), n.extend(0.0), self.frame.origin.extend(1.0)).inverse()
    }

    /// A model point in the flat view's place (the base wall's points land
    /// on the flat).
    pub fn to_flat_view(&self, p: DVec3) -> DVec3 {
        self.flat_view_matrix().transform_point3(p)
    }

    /// A model direction in the flat view's place.
    pub fn to_flat_view_vector(&self, v: DVec3) -> DVec3 {
        self.flat_view_matrix().transform_vector3(v)
    }

    /// The flat as a solid in the flat view's place: the sheet under z 0,
    /// `thickness` deep.
    pub fn flat_view_solid(&self) -> Option<Shape> {
        let face = self.shape.as_ref()?.transformed(&mat_transform(&self.flat_view_matrix())).ok()?;
        k::ops::extrude(&face, DVec3::new(0.0, 0.0, -self.thickness)).ok().map(|r| r.shape)
    }

    /// The bend lines in the flat view's place (on the top, z 0).
    pub fn flat_view_bend_lines(&self) -> Vec<(DVec3, DVec3)> {
        self.bend_lines.iter().map(|b| (b.a.extend(0.0), b.b.extend(0.0))).collect()
    }

    /// Bounding box of the outline (min, max) in plane coordinates.
    pub fn bounds(&self) -> (DVec2, DVec2) {
        let mut lo = DVec2::splat(f64::INFINITY);
        let mut hi = DVec2::splat(f64::NEG_INFINITY);
        for p in self.loops.iter().flatten() {
            lo = lo.min(*p);
            hi = hi.max(*p);
        }
        if lo.x > hi.x { (DVec2::ZERO, DVec2::ZERO) } else { (lo, hi) }
    }

    /// Overall size (x, y) of the flat.
    pub fn size(&self) -> DVec2 {
        let (a, b) = self.bounds();
        b - a
    }

    /// Area of the flat region (outer loops minus holes).
    pub fn area(&self) -> f64 {
        if let Some(s) = &self.shape {
            let a: f64 = s.faces().iter().filter_map(|f| f.area().ok()).sum();
            if a > 0.0 {
                return a;
            }
        }
        // Fallback: signed areas (holes are oriented the other way only by
        // chance), so use the largest loop minus the others.
        let mut areas: Vec<f64> = self.loops.iter().map(|l| polygon_area(l).abs()).collect();
        areas.sort_by(|a, b| b.total_cmp(a));
        areas.first().copied().unwrap_or(0.0) - areas.iter().skip(1).sum::<f64>()
    }

    /// 2D sheet geometry (millimeters) for the drawing exporters: outline as
    /// visible lines, bend lines dashed with "UP/DOWN angle R" labels.
    pub fn to_sheet_geometry(&self) -> SheetGeometry {
        let margin = 10.0;
        let (lo, hi) = self.bounds();
        let size = (hi - lo) * 1000.0;
        let mut g = SheetGeometry::new(size.x + 2.0 * margin, size.y + 2.0 * margin);
        let map = |p: DVec2| -> [f64; 2] {
            let q = (p - lo) * 1000.0;
            [q.x + margin, q.y + margin]
        };
        for l in &self.loops {
            if l.len() < 2 {
                continue;
            }
            g.lines.push(Polyline2 { points: l.iter().map(|p| map(*p)).collect(), style: LineStyle::Visible, closed: true, pick: 0 });
        }
        let text_h = (size.x.max(size.y) * 0.02).clamp(2.0, 5.0);
        for b in &self.bend_lines {
            let (pa, pb) = (map(b.a), map(b.b));
            g.lines.push(Polyline2 { points: vec![pa, pb], style: LineStyle::Hidden, closed: false, pick: 0 });
            let mid = [(pa[0] + pb[0]) * 0.5, (pa[1] + pb[1]) * 0.5];
            let mut ang = (pb[1] - pa[1]).atan2(pb[0] - pa[0]);
            if ang > std::f64::consts::FRAC_PI_2 + 1e-9 {
                ang -= std::f64::consts::PI;
            } else if ang < -std::f64::consts::FRAC_PI_2 + 1e-9 {
                ang += std::f64::consts::PI;
            }
            let label = format!("{} {:.0}° R{}", if b.up { "UP" } else { "DOWN" }, b.angle.to_degrees(), trim_num(b.radius * 1000.0));
            let off = text_h * 0.4;
            let pos = [mid[0] - ang.sin() * off, mid[1] + ang.cos() * off];
            g.texts.push(Text2 { pos, height: text_h, text: label, angle: ang, anchor: Anchor::BottomCenter, pick: 0 });
        }
        for f in &self.forms {
            for l in &f.loops {
                if l.len() >= 2 {
                    g.lines.push(Polyline2 { points: l.iter().map(|p| map(*p)).collect(), style: LineStyle::Thin, closed: true, pick: 0 });
                }
            }
            // Live 1623: tears solid, outlines dash-dot.
            for (l, tear) in f.curves.iter().filter(|c| c.0.len() >= 2) {
                g.lines.push(Polyline2 { points: l.iter().map(|p| map(*p)).collect(), style: if *tear { LineStyle::Thin } else { LineStyle::Centerline }, closed: false, pick: 0 });
            }
            let c = map(f.center);
            let arm = text_h * 0.6;
            g.lines.push(Polyline2 { points: vec![[c[0] - arm, c[1]], [c[0] + arm, c[1]]], style: LineStyle::Centerline, closed: false, pick: 0 });
            g.lines.push(Polyline2 { points: vec![[c[0], c[1] - arm], [c[0], c[1] + arm]], style: LineStyle::Centerline, closed: false, pick: 0 });
        }
        g
    }
}

fn trim_num(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

fn polygon_area(p: &[DVec2]) -> f64 {
    let n = p.len();
    (0..n).map(|i| p[i].perp_dot(p[(i + 1) % n])).sum::<f64>() * 0.5
}

/// A part's shape fingerprint: face and edge counts and its box (a later
/// feature that cut a hole or moved it changes it).
pub fn body_key(shape: &Shape) -> [f64; 8] {
    let b = shape.bounding_box(false).map(|b| [b.min.x, b.min.y, b.min.z, b.max.x, b.max.y, b.max.z]).unwrap_or([0.0; 6]);
    [shape.count(ShapeType::Face) as f64, shape.count(ShapeType::Edge) as f64, b[0], b[1], b[2], b[3], b[4], b[5]]
}

/// Flat pattern of the sheet metal body with id `id` in a context: the std
/// sheet metal feature's own flat while the part is as that feature built it,
/// else unfolded from the part (a later Hole's through hole and counterbore
/// heads, as live's flat shows them).
pub fn flat_pattern_for(ctx: &Context, id: &BodyId) -> Result<FlatPattern, String> {
    let body = ctx.body(id).ok_or_else(|| "The part no longer exists".to_string())?;
    if let Some(f) = ctx.sheet_metal_flats.get(id)
        && f.built_for.is_none_or(|k| k == body_key(&body.shape))
    {
        return Ok((**f).clone());
    }
    let data = ctx.sheet_metal.get(id).ok_or_else(|| format!("{} is not a sheet metal part", body.name))?;
    flat_pattern(body, data)
}

fn mat_transform(m: &DMat4) -> k::Transform {
    k::Transform::Matrix(*m)
}

/// Unfold `body` using its sheet metal model.
pub fn flat_pattern(body: &Body, data: &SheetMetalData) -> Result<FlatPattern, String> {
    let t = data.thickness;
    let nw = data.walls.len();
    // Resolve wall faces in the current body.
    let wall_faces: Vec<Vec<Shape>> = data
        .walls
        .iter()
        .map(|w| {
            let mut v: Vec<usize> = w.faces.iter().flat_map(|n| body.find_faces(n)).collect();
            v.sort_unstable();
            v.dedup();
            v.into_iter().filter_map(|i| body.shape.sub_shape(ShapeType::Face, i).ok()).filter(|f| f.is_planar()).collect()
        })
        .collect();
    let base = (0..nw).find(|&i| !wall_faces[i].is_empty()).ok_or_else(|| "The sheet metal part has no walls to flatten".to_string())?;
    let base_face = &wall_faces[base][0];
    let base_plane = base_face.face_plane().map_err(|e| e.to_string())?.ok_or_else(|| "Base wall is not planar".to_string())?;
    // Seen as Onshape shows it (live SM1 DXF, samples/qa5-sm-flat.dxf, is in model
    // x/y): from the definition side, turned by the smallest rotation taking that
    // side's normal to +z, and moved along it only.
    let normal = base_plane.unit_normal();
    let view = if data.view_back { -normal } else { normal };
    let to_z = glam::DQuat::from_rotation_arc(view, DVec3::Z).inverse();
    let x = to_z * DVec3::X;
    let frame = k::Plane::new(view * base_plane.origin.dot(view), view, x);

    let mut warnings = Vec::new();
    let mut xf: Vec<Option<DMat4>> = vec![None; nw];
    let mut tree_bends: Vec<usize> = Vec::new();
    let mut used = vec![false; data.bends.len()];
    let mut pieces: Vec<Shape> = Vec::new();
    let mut lines: Vec<FlatBendLine> = Vec::new();
    let mut next_root = Some(base);
    let mut comp_offset = 0.0f64;
    let gap = (10.0 * t).max(0.005);
    while let Some(root) = next_root {
        // Root placement: the base wall stays; other components are moved
        // into the base plane next to what is already laid out.
        let root_m = if root == base {
            DMat4::IDENTITY
        } else {
            let f = &wall_faces[root][0];
            let Ok(Some(pl)) = f.face_plane() else {
                break;
            };
            let from = k::Plane::new(f.centroid().unwrap_or(pl.origin), pl.unit_normal(), pl.unit_x());
            let to = k::Plane::new(frame.origin, normal, x);
            k::Transform::frame_to_frame(&from, &to).matrix()
        };
        xf[root] = Some(root_m);
        let mut comp: Vec<usize> = vec![root];
        let mut queue = VecDeque::from([root]);
        while let Some(w) = queue.pop_front() {
            let mw = xf[w].unwrap_or(DMat4::IDENTITY);
            for (bi, b) in data.bends.iter().enumerate() {
                if used[bi] {
                    continue;
                }
                let unfold = b.unfold_matrix(t);
                if b.parent == w {
                    used[bi] = true;
                    tree_bends.push(bi);
                    if let Some(c) = b.child {
                        if xf[c].is_some() {
                            warnings.push("A closed loop of bends was left unflattened (add a rip)".into());
                            tree_bends.pop();
                            continue;
                        }
                        xf[c] = Some(mw * unfold);
                        comp.push(c);
                        queue.push_back(c);
                    }
                } else if b.child == Some(w) {
                    used[bi] = true;
                    if xf[b.parent].is_some() {
                        warnings.push("A closed loop of bends was left unflattened (add a rip)".into());
                        continue;
                    }
                    tree_bends.push(bi);
                    xf[b.parent] = Some(mw * unfold.inverse());
                    comp.push(b.parent);
                    queue.push_back(b.parent);
                }
            }
        }
        // Pieces of this component.
        let mut comp_pieces: Vec<Shape> = Vec::new();
        for &w in &comp {
            let m = xf[w].unwrap_or(DMat4::IDENTITY);
            for f in &wall_faces[w] {
                match f.transformed(&mat_transform(&m)) {
                    Ok(s) => comp_pieces.push(s),
                    Err(e) => warnings.push(format!("A wall could not be placed in the flat: {e}")),
                }
            }
        }
        for &bi in &tree_bends {
            let b = &data.bends[bi];
            if !comp.contains(&b.parent) || lines.iter().any(|l| l.bend == bi) {
                continue;
            }
            let m = xf[b.parent].unwrap_or(DMat4::IDENTITY);
            let ba = b.allowance(t);
            let p0 = b.tangent_point(t, b.s0);
            let p1 = b.tangent_point(t, b.s1);
            let off = -b.dir_a * ba;
            let quad = [p0, p1, p1 + off, p0 + off].map(|p| m.transform_point3(p));
            if ba > 1e-12 {
                match k::make_polyline(&quad, true).and_then(|w| k::make_face_from_wire(&w, true)) {
                    Ok(f) => comp_pieces.push(f),
                    Err(e) => warnings.push(format!("A bend region could not be built: {e}")),
                }
            }
            let ca = m.transform_point3(p0 + off * 0.5);
            let cb = m.transform_point3(p1 + off * 0.5);
            lines.push(FlatBendLine {
                a: frame.to_local(ca),
                b: frame.to_local(cb),
                // Up: toward the viewer of the definition side (live SM1: flanges and hem rising from the top are Up).
                up: (b.is_up() != data.view_back) != data.flip_direction_up,
                angle: b.angle,
                radius: b.radius,
                k_factor: b.k_factor,
                allowance: ba,
                bend: bi,
            });
        }
        // Shift the component so it does not overlap earlier ones.
        if root != base && !comp_pieces.is_empty() {
            let (lo, _) = local_bounds(&frame, &comp_pieces);
            let shift = x * (comp_offset + gap - lo.x);
            let sm = DMat4::from_translation(shift);
            comp_pieces = comp_pieces.into_iter().filter_map(|p| p.transformed(&mat_transform(&sm)).ok()).collect();
            let d2 = frame.to_local(frame.origin + shift) - frame.to_local(frame.origin);
            for l in lines.iter_mut().filter(|l| comp.contains(&data.bends[l.bend].parent)) {
                l.a += d2;
                l.b += d2;
            }
            for &w in &comp {
                if let Some(m) = xf[w].as_mut() {
                    *m = sm * *m;
                }
            }
        }
        if !comp_pieces.is_empty() {
            let (_, hi) = local_bounds(&frame, &comp_pieces);
            comp_offset = comp_offset.max(hi.x);
        }
        pieces.extend(comp_pieces);
        next_root = (0..nw).find(|&i| xf[i].is_none() && !wall_faces[i].is_empty());
        if next_root.is_some() {
            warnings.push("The part has walls not connected by bends; they are laid out separately".into());
        }
    }
    // Form footprints fill the flat (shown as outlines); relief cutouts are removed.
    let mut forms = Vec::new();
    let mut cuts: Vec<Shape> = Vec::new();
    for mk in &data.marks {
        let Some(m) = xf.get(mk.wall).copied().flatten() else { continue };
        let loops3: Vec<Vec<DVec3>> = mk.loops.iter().map(|l| l.iter().map(|p| m.transform_point3(*p)).collect()).collect();
        let face = match &mk.fill {
            Some(f) => f.transformed(&mat_transform(&m)).ok(),
            None => loops3.first().filter(|l| l.len() >= 3).and_then(|l| k::make_polyline(l, true).and_then(|w| k::make_face_from_wire(&w, true)).ok()),
        };
        match mk.kind {
            super::FlatMarkKind::Form => {
                if let Some(f) = face {
                    pieces.extend(f.faces());
                }
                let center = mk.center().map(|c| frame.to_local(m.transform_point3(c))).unwrap_or_default();
                let up = mk.direction.is_none_or(|d| m.transform_vector3(d).dot(frame.normal) > 0.0);
                forms.push(FlatForm { loops: loops3.iter().map(|l| l.iter().map(|p| frame.to_local(*p)).collect()).collect(), curves: Vec::new(), center, up });
            }
            super::FlatMarkKind::Cut => cuts.extend(face),
        }
    }
    // Counterbore / countersink heads: a circle on one sheet side around a
    // smaller one on the other. The flat shows the through hole (the
    // reference side's bigger circle is filled back to it) and the head.
    let mut heads = Vec::new();
    let resolve = |names: &[crate::naming::Name]| -> Vec<Shape> {
        let mut v: Vec<usize> = names.iter().flat_map(|n| body.find_faces(n)).collect();
        v.sort_unstable();
        v.dedup();
        v.into_iter().filter_map(|i| body.shape.sub_shape(ShapeType::Face, i).ok()).filter(|f| f.is_planar()).collect()
    };
    for (w, wall) in data.walls.iter().enumerate() {
        let Some(m) = xf[w] else { continue };
        let circles = |faces: &[Shape]| -> Vec<(DVec2, f64)> {
            faces
                .iter()
                .flat_map(|f| {
                    let mut ls: Vec<Vec<DVec2>> = f.wires().iter().map(|w| wire_points(w).into_iter().map(|p| frame.to_local(m.transform_point3(p))).collect()).collect();
                    // The outer boundary is not a hole.
                    ls.sort_by(|a, b| extent(b).total_cmp(&extent(a)));
                    ls.into_iter().skip(1).filter_map(|l| circle_of(&l)).collect::<Vec<_>>()
                })
                .collect()
        };
        let back = circles(&resolve(&wall.back));
        if back.is_empty() {
            continue;
        }
        let tol = 1e-6;
        for (c, rf) in circles(&wall_faces[w]) {
            let Some(&(_, rb)) = back.iter().find(|(cb, rb)| cb.distance(c) < tol && (rf - rb).abs() > 10.0 * tol) else { continue };
            let on_front = rf > rb;
            if on_front {
                let disk = |r: f64| k::make_circle(frame.to_world(c), frame.unit_normal(), r).and_then(|e| k::make_wire(&[e])).and_then(|w| k::make_face_from_wire(&w, true));
                match (disk(rf), disk(rb)) {
                    (Ok(head), Ok(hole)) => {
                        pieces.push(head);
                        cuts.push(hole);
                    }
                    _ => warnings.push("A counterbore could not be drawn in the flat".into()),
                }
            }
            heads.push(FlatHead { center: c, radius: rf.max(rb), up: on_front != data.view_back });
        }
    }
    warnings.dedup();
    if pieces.is_empty() {
        return Err("Nothing to flatten".into());
    }
    // Union of all planar pieces into one region.
    let mut shape = match union_planar(&pieces) {
        Ok(s) => Some(s),
        Err(e) => {
            warnings.push(format!("The flat outline could not be merged ({e}); showing the pieces"));
            None
        }
    };
    if let (Some(s), false) = (&shape, cuts.is_empty()) {
        match k::boolean(k::BooleanOp::Cut, std::slice::from_ref(s), &cuts, &k::BooleanOptions { fuzzy: 1e-7, ..Default::default() }) {
            Ok(r) if r.shape.count(ShapeType::Face) > 0 => shape = Some(k::unify_same_domain(&r.shape).map(|u| u.shape).unwrap_or(r.shape)),
            _ => warnings.push("A relief cutout could not be removed from the flat".into()),
        }
    }
    let faces_src: Vec<Shape> = match &shape {
        Some(s) => s.faces(),
        None => pieces.clone(),
    };
    let mut loops: Vec<Vec<DVec2>> = Vec::new();
    for f in &faces_src {
        for w in f.wires() {
            let l = wire_loop(&w, &frame);
            if l.len() >= 3 {
                loops.push(l);
            }
        }
    }
    loops.sort_by(|a, b| extent(b).total_cmp(&extent(a)));
    Ok(FlatPattern { frame, loops, bend_lines: lines, forms, heads, shape, thickness: t, warnings, built_for: None })
}

/// A flat pattern from planar pieces already laid flat in `frame` (walls and
/// developed bend strips, built from the std sheet metal definition: its
/// placement is live's, the model's own coordinates; see
/// custom/host/sheetmetal.rs `flat_of`).
pub fn flat_from_pieces(frame: k::Plane, pieces: &[Shape], bend_lines: Vec<FlatBendLine>, thickness: f64) -> Result<FlatPattern, String> {
    if pieces.is_empty() {
        return Err("Nothing to flatten".into());
    }
    let mut warnings = Vec::new();
    let shape = match union_planar(pieces) {
        Ok(s) => Some(s),
        Err(e) => {
            warnings.push(format!("The flat outline could not be merged ({e}); showing the pieces"));
            None
        }
    };
    let faces_src: Vec<Shape> = match &shape {
        Some(s) => s.faces(),
        None => pieces.to_vec(),
    };
    let mut loops: Vec<Vec<DVec2>> = Vec::new();
    for f in &faces_src {
        for w in f.wires() {
            let l = wire_loop(&w, &frame);
            if l.len() >= 3 {
                loops.push(l);
            }
        }
    }
    loops.sort_by(|a, b| extent(b).total_cmp(&extent(a)));
    Ok(FlatPattern { frame, loops, bend_lines, forms: Vec::new(), heads: Vec::new(), shape, thickness, warnings, built_for: None })
}

fn extent(l: &[DVec2]) -> f64 {
    let mut lo = DVec2::splat(f64::INFINITY);
    let mut hi = DVec2::splat(f64::NEG_INFINITY);
    for p in l {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    (hi - lo).length()
}

fn local_bounds(frame: &k::Plane, shapes: &[Shape]) -> (DVec2, DVec2) {
    let mut lo = DVec2::splat(f64::INFINITY);
    let mut hi = DVec2::splat(f64::NEG_INFINITY);
    for s in shapes {
        for v in s.vertices() {
            if let Ok(p) = v.vertex_point() {
                let q = frame.to_local(p);
                lo = lo.min(q);
                hi = hi.max(q);
            }
        }
    }
    (lo, hi)
}

/// Fuse coplanar faces and merge them into maximal faces.
fn union_planar(pieces: &[Shape]) -> Result<Shape, String> {
    if pieces.len() == 1 {
        return Ok(pieces[0].clone());
    }
    let opts = k::BooleanOptions { fuzzy: 1e-7, ..Default::default() };
    let fused = k::fuse_all(pieces, &opts).map_err(|e| e.to_string())?;
    // Sew the split faces into a shell so coplanar neighbors can be merged.
    let faces = fused.shape.faces();
    let sewn = k::sew(&faces, 1e-7, false).map(|r| r.shape).unwrap_or(fused.shape);
    let unified = k::unify_same_domain(&sewn).map(|r| r.shape).unwrap_or(sewn);
    let total_in: f64 = pieces.iter().filter_map(|p| p.area().ok()).sum();
    let total_out: f64 = unified.faces().iter().filter_map(|f| f.area().ok()).sum();
    // Overlapping pieces (collisions in the flat) shrink the area; keep the
    // union anyway, it is still the flat outline.
    if total_out <= 0.0 || total_out > total_in * (1.0 + 1e-6) {
        return Err(format!("area {total_out} vs {total_in}, {} faces", unified.faces().len()));
    }
    Ok(unified)
}

/// Sample the edges of a wire into one closed polyline (plane coordinates).
/// Center and radius of a sampled loop that is a circle.
fn circle_of(l: &[DVec2]) -> Option<(DVec2, f64)> {
    if l.len() < 8 {
        return None;
    }
    let (lo, hi) = l.iter().fold((DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
    let c = (lo + hi) * 0.5;
    let r = l.iter().map(|p| p.distance(c)).sum::<f64>() / l.len() as f64;
    (r > 1e-9 && l.iter().all(|p| (p.distance(c) - r).abs() < 1e-4 * r + 1e-8)).then_some((c, r))
}

fn wire_loop(w: &Shape, frame: &k::Plane) -> Vec<DVec2> {
    wire_points(w).into_iter().map(|p| frame.to_local(p)).collect()
}

/// Sample the edges of a wire into one chained polyline (model space).
pub fn wire_points(w: &Shape) -> Vec<DVec3> {
    let mut segs: Vec<Vec<DVec3>> = Vec::new();
    for e in w.edges() {
        let Ok(info) = e.edge_info() else { continue };
        if info.degenerated {
            continue;
        }
        let pts = match info.curve {
            k::Curve::Line { .. } => vec![info.start, info.end],
            _ => {
                let n = 32;
                (0..=n).filter_map(|i| e.point_at(info.first_param + (info.last_param - info.first_param) * i as f64 / n as f64).ok()).collect()
            }
        };
        if pts.len() >= 2 {
            segs.push(pts);
        }
    }
    chain_loop(segs)
}

fn chain_loop(mut segs: Vec<Vec<DVec3>>) -> Vec<DVec3> {
    if segs.is_empty() {
        return Vec::new();
    }
    let scale = segs.iter().flatten().map(|p| p.length()).fold(1.0f64, f64::max);
    let tol = 1e-6 * scale;
    let mut out = segs.remove(0);
    while !segs.is_empty() {
        let end = *out.last().unwrap();
        let mut found = None;
        for (i, s) in segs.iter().enumerate() {
            if (s[0] - end).length() < tol {
                found = Some((i, false));
                break;
            }
            if (s[s.len() - 1] - end).length() < tol {
                found = Some((i, true));
                break;
            }
        }
        let Some((i, rev)) = found else { break };
        let mut s = segs.remove(i);
        if rev {
            s.reverse();
        }
        out.extend(s.into_iter().skip(1));
    }
    if out.len() > 1 && (out[0] - out[out.len() - 1]).length() < tol {
        out.pop();
    }
    out
}
