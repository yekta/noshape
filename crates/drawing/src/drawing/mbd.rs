//! MBD annotations in drawings (live behaviours "MBD datums in drawings",
//! V5 / CR18 / A65): a Part Studio's Model definitions datums appear once,
//! in the first view of the suggested set that shows their face edge-on, as
//! ordinary drawing datums (filled triangle on the edge, a stem, the boxed
//! letter). They stay linked: the anchor follows the face, the box moves
//! only along the face normal, Delete hides them (the MBD status panel lists
//! them and "Show" brings them back where they were), and a view's
//! "Show annotations from model" off hides its own.

pub use noshape_model_data::drawing::mbd::*;
use super::views::{self, Sources, ViewFrame, ViewGeometry};
use super::{Annotation, AnnotationKind, Drawing, Sheet, ViewId};
use crate::annotations::{Annotation as ModelAnnotation, AnnotationKind as ModelKind};
use crate::document::{Document, TabContent, TabId};
use glam::DVec3;
use std::collections::HashMap;
use std::sync::Arc;



/// Is `normal` edge-on (perpendicular to the line of sight) in `frame`?
fn edge_on(frame: &ViewFrame, normal: DVec3) -> bool {
    normal.normalize_or_zero().dot(frame.dir.normalize()).abs() < 1e-6
}

/// Does a view along `frame` show model annotation `m`: a datum's face
/// edge-on, a dimension's plane face-on (drawing.md CR18).
pub fn shows(m: &ModelAnnotation, frame: &ViewFrame) -> bool {
    let n = DVec3::from_array(m.normal);
    match m.kind {
        ModelKind::Datum { .. } => edge_on(frame, n),
        ModelKind::Dimension { .. } => n.normalize_or_zero().dot(frame.dir.normalize()).abs() > 1.0 - 1e-6,
    }
}

/// What a measured entity is, geometrically.
#[derive(Clone, Copy, Debug)]
enum Prim {
    Point(DVec3),
    /// A line through a point along a direction; a cylinder's axis keeps
    /// its radius.
    Line(DVec3, DVec3, f64),
    /// A plane through a point with its normal.
    Plane(DVec3, DVec3),
}

fn prim(ctx: &crate::regen::context::Context, r: &crate::reference::Reference) -> Option<Prim> {
    use crate::reference::Reference as R;
    use noshape_kernel::{Curve, ShapeType, Surface};
    for b in &ctx.bodies {
        match r {
            R::Face(n) => {
                let Some(&i) = b.find_faces(n).first() else { continue };
                let f = b.shape.sub_shape(ShapeType::Face, i).ok()?;
                return match f.face_surface().ok()? {
                    Surface::Plane(p) => Some(Prim::Plane(f.centroid().ok().unwrap_or(p.origin), p.normal.normalize())),
                    Surface::Cylinder { origin, axis, radius, .. } => Some(Prim::Line(origin, axis.normalize(), radius)),
                    Surface::Cone { origin, axis, .. } => Some(Prim::Line(origin, axis.normalize(), 0.0)),
                    Surface::Sphere { center, .. } => Some(Prim::Point(center)),
                    Surface::Torus { center, axis, major_radius, .. } => Some(Prim::Line(center, axis.normalize(), major_radius)),
                    _ => None,
                };
            }
            R::Edge(n) => {
                let Some(&i) = b.find_edges(n).first() else { continue };
                let e = b.shape.sub_shape(ShapeType::Edge, i).ok()?;
                return match e.edge_curve().ok()? {
                    Curve::Line { origin, direction } => Some(Prim::Line(origin, direction.normalize(), 0.0)),
                    Curve::Circle { center, axis, radius, .. } => Some(Prim::Line(center, axis.normalize(), radius)),
                    _ => None,
                };
            }
            R::Vertex(n) => {
                let Some(&i) = b.find_vertices(n).first() else { continue };
                return Some(Prim::Point(b.shape.sub_shape(ShapeType::Vertex, i).ok()?.vertex_point().ok()?));
            }
            _ => return None,
        }
    }
    None
}

/// An MBD dimension's geometry: the measured points of a linear one (their
/// distance its value), an angle's vertex and a point on each leg, a
/// radius's center with its axis and radius.
#[derive(Clone, Copy, Debug)]
pub enum MbdShape {
    Linear { a: DVec3, b: DVec3 },
    Angle { vertex: DVec3, a: DVec3, b: DVec3 },
    Radius { center: DVec3, axis: DVec3, radius: f64 },
}

/// Closest points of two entities (the first near `near`), with the radii
/// the lines carry.
fn closest(p: Prim, q: Prim, near: DVec3) -> Option<(DVec3, DVec3, f64, f64)> {
    let foot = |o: DVec3, d: DVec3, x: DVec3| o + d * (x - o).dot(d);
    let on_plane = |o: DVec3, n: DVec3, x: DVec3| x - n * (x - o).dot(n);
    Some(match (p, q) {
        (Prim::Point(a), Prim::Point(b)) => (a, b, 0.0, 0.0),
        (Prim::Point(a), Prim::Line(o, d, r)) => (a, foot(o, d, a), 0.0, r),
        (Prim::Line(o, d, r), Prim::Point(b)) => {
            let a = foot(o, d, b);
            (a, b, r, 0.0)
        }
        (Prim::Point(a), Prim::Plane(o, n)) => (a, on_plane(o, n, a), 0.0, 0.0),
        (Prim::Plane(o, n), Prim::Point(b)) => (on_plane(o, n, b), b, 0.0, 0.0),
        (Prim::Line(o1, d1, r1), Prim::Line(o2, d2, r2)) => {
            if d1.cross(d2).length() < 1e-9 {
                let a = foot(o1, d1, near);
                (a, foot(o2, d2, a), r1, r2)
            } else {
                let w = o1 - o2;
                let (b, d, e) = (d1.dot(d2), d1.dot(w), d2.dot(w));
                let den = 1.0 - b * b;
                let (s, t) = ((b * e - d) / den, (e - b * d) / den);
                (o1 + d1 * s, o2 + d2 * t, r1, r2)
            }
        }
        (Prim::Line(o, d, r), Prim::Plane(po, n)) => {
            let a = foot(o, d, near);
            (a, on_plane(po, n, a), r, 0.0)
        }
        (Prim::Plane(po, n), Prim::Line(o, d, r)) => {
            let b = foot(o, d, near);
            (on_plane(po, n, b), b, 0.0, r)
        }
        (Prim::Plane(o1, n1), Prim::Plane(o2, n2)) => {
            if n1.cross(n2).length() > 1e-6 {
                return None;
            }
            let a = on_plane(o1, n1, near);
            (a, on_plane(o2, n2, a), 0.0, 0.0)
        }
    })
}

/// Measure MBD dimension `m` on the model: its shape and value (meters,
/// or radians for an angle); None when its entities are gone or don't
/// make that measure.
pub fn measure(ctx: &crate::regen::context::Context, m: &ModelAnnotation) -> Option<(MbdShape, f64)> {
    use crate::annotations::Measure;
    let ModelKind::Dimension { measure, refs, .. } = &m.kind else { return None };
    let near = DVec3::from_array(m.at);
    match measure {
        Measure::Radius => match prim(ctx, refs.first()?)? {
            Prim::Line(o, d, r) if r > 0.0 => {
                let c = o + d * (near - o).dot(d);
                Some((MbdShape::Radius { center: c, axis: d, radius: r }, r))
            }
            _ => None,
        },
        Measure::Angle => {
            let (Prim::Plane(o1, n1), Prim::Plane(o2, n2)) = (prim(ctx, refs.first()?)?, prim(ctx, refs.get(1)?)?) else { return None };
            let line = n1.cross(n2);
            if line.length() < 1e-9 {
                return None;
            }
            let line = line.normalize();
            // A point on both planes nearest the text.
            let (d1, d2) = (n1.dot(o1), n2.dot(o2));
            let base = (n1 * (d1 - d2 * n1.dot(n2)) + n2 * (d2 - d1 * n1.dot(n2))) / (1.0 - n1.dot(n2).powi(2));
            let vertex = base + line * (near - base).dot(line);
            // Each leg runs in its face, away from the line, toward the face.
            let leg = |n: DVec3, o: DVec3| {
                let d = line.cross(n).normalize();
                if (o - vertex).dot(d) < 0.0 { -d } else { d }
            };
            let (l1, l2) = (leg(n1, o1), leg(n2, o2));
            let value = l1.dot(l2).clamp(-1.0, 1.0).acos();
            Some((MbdShape::Angle { vertex, a: vertex + l1 * 0.01, b: vertex + l2 * 0.01 }, value))
        }
        _ => {
            let (a, b, r1, r2) = closest(prim(ctx, refs.first()?)?, prim(ctx, refs.get(1)?)?, near)?;
            let d = b - a;
            let len = d.length();
            if len < 1e-12 {
                return None;
            }
            let u = d / len;
            // Minimum / maximum: the cylinders' near or far sides.
            let (a, b) = match measure {
                Measure::Minimum => (a + u * r1, b - u * r2),
                Measure::Maximum => (a - u * r1, b + u * r2),
                _ => (a, b),
            };
            Some((MbdShape::Linear { a, b }, (b - a).length()))
        }
    }
}

/// A new MBD dimension's plane (`plane`: top, front or right; else the
/// first of front, top, right that contains the measurement) and text
/// position (`position`, else 10 mm off its middle in that plane).
pub fn place_dimension(ctx: &crate::regen::context::Context, m: &mut ModelAnnotation, plane: Option<&str>, position: Option<DVec3>) -> Result<(), String> {
    let normals = [("front", DVec3::Y), ("top", DVec3::Z), ("right", DVec3::X)];
    m.at = position.unwrap_or(DVec3::ZERO).to_array();
    let (shape, _) = measure(ctx, m).ok_or("those entities don't make that dimension")?;
    let (mid, along) = match shape {
        MbdShape::Linear { a, b } => ((a + b) * 0.5, (b - a).normalize()),
        MbdShape::Angle { vertex, a, b } => (vertex, (a - b).normalize_or_zero()),
        MbdShape::Radius { center, axis, .. } => (center, axis),
    };
    let fits = |n: DVec3| match shape {
        MbdShape::Radius { axis, .. } => n.cross(axis).length() < 1e-6,
        MbdShape::Angle { vertex, a, b } => (a - vertex).cross(b - vertex).normalize_or_zero().cross(n).length() < 1e-6,
        MbdShape::Linear { .. } => along.dot(n).abs() < 1e-6,
    };
    let n = match plane {
        Some(p) => normals.iter().find(|(k, _)| *k == p).map(|x| x.1).ok_or("plane: top, front or right")?,
        None => normals.iter().map(|x| x.1).find(|n| fits(*n)).unwrap_or(DVec3::Y),
    };
    if !fits(n) && plane.is_some() {
        return Err("the dimension doesn't lie in that plane".into());
    }
    m.normal = n.to_array();
    let side = n.cross(along).normalize_or_zero();
    m.up = side.to_array();
    if position.is_none() {
        m.at = (mid + side * 0.01).to_array();
    }
    // Measured again from its text position (the closest points near it).
    measure(ctx, m).ok_or("those entities don't make that dimension")?;
    Ok(())
}

/// The drawing annotation an MBD dimension shows as in view `view` (its
/// frame and placement `to_sheet`), its value from the model.
fn dimension_kind(ctx: &crate::regen::context::Context, m: &ModelAnnotation, view: ViewId, frame: &ViewFrame, to_sheet: &dyn Fn([f64; 2]) -> [f64; 2]) -> Option<(AnnotationKind, [f64; 2])> {
    let (shape, _) = measure(ctx, m)?;
    let at = frame.project(DVec3::from_array(m.at));
    let st = to_sheet(at);
    Some(match shape {
        MbdShape::Linear { a, b } => {
            let (pa, pb) = (frame.project(a), frame.project(b));
            let (sa, sb) = (to_sheet(pa), to_sheet(pb));
            let d = [sb[0] - sa[0], sb[1] - sa[1]];
            let l = d[0].hypot(d[1]).max(1e-12);
            let n = [-d[1] / l, d[0] / l];
            let base = |p: [f64; 2]| p[0] * n[0] + p[1] * n[1];
            let offset = base(st) - base(sa).max(base(sb));
            (AnnotationKind::LinearDimension { view, a: pa, b: pb, offset, orientation: super::DimOrientation::Aligned, override_text: None }, st)
        }
        MbdShape::Angle { vertex, a, b } => {
            let (v, pa, pb) = (frame.project(vertex), frame.project(a), frame.project(b));
            let (sv, sl) = (to_sheet(v), to_sheet(at));
            let radius = (sl[0] - sv[0]).hypot(sl[1] - sv[1]).max(8.0);
            // Counterclockwise from a to b (the smaller turn).
            let ang = |p: [f64; 2]| (p[1] - v[1]).atan2(p[0] - v[0]);
            let turn = (ang(pb) - ang(pa)).rem_euclid(std::f64::consts::TAU);
            let (a2, b2) = if turn <= std::f64::consts::PI { (pa, pb) } else { (pb, pa) };
            (AnnotationKind::AngularDimension { view, vertex: v, a: a2, b: b2, radius, three_point: false, flip_a: false, flip_b: false, text_angle: None }, st)
        }
        MbdShape::Radius { center, radius, .. } => {
            let c = frame.project(center);
            let sc = to_sheet(c);
            (AnnotationKind::RadialDimension { view, center: c, radius, diameter: false, angle: (st[1] - sc[1]).atan2(st[0] - sc[0]), dim_line: false, knee: None }, st)
        }
    })
}

/// An MBD dimension's tolerance and precision as a drawing format.
fn dimension_format(m: &ModelAnnotation, base: &super::DimFormat) -> super::DimFormat {
    let ModelKind::Dimension { tolerance: t, measure, .. } = &m.kind else { return base.clone() };
    let angle = *measure == crate::annotations::Measure::Angle;
    // Deviations are typed in the document unit (mm here) or degrees.
    let v = |s: &str| {
        let x = s.trim().trim_end_matches("mm").trim_end_matches("deg").trim_end_matches('°').trim().parse::<f64>().unwrap_or(0.0);
        if angle { x.to_radians() } else { x * 0.001 }
    };
    let (u, l) = (v(&t.upper), v(&t.lower));
    let tolerance = match t.kind.as_str() {
        "Symmetrical" => super::Tolerance::Symmetric { value: u },
        "Deviation" => super::Tolerance::Deviation { upper: u, lower: -l },
        "Limits" => super::Tolerance::Limits { upper: u, lower: -l },
        "Min" => super::Tolerance::Min,
        "Max" => super::Tolerance::Max,
        "Basic" => super::Tolerance::Basic,
        _ => super::Tolerance::None,
    };
    super::DimFormat { tolerance, precision: t.precision.or(base.precision), ..base.clone() }
}

/// Suggested views of a part with MBD data (live: "Suggested views provide
/// only necessary views when MBD data is present"; behaviours "MBD datums
/// in drawings", requests 1721, 1734): each MBD item goes to the first
/// standard view that shows it (third angle: Front, Top, Right; first
/// angle: Front, Left, Top), unless a view chosen already does; only those
/// views stay. Live: a datum on the top face and one on the front face gave
/// Front and Top (ANSI); one on the back face the Left view alone, one on
/// the bottom or left face the Front view alone, dimensions in the top
/// plane the Top view alone (ISO). Without the Front view, the views are
/// base views, one alone centred at the largest scale whose view fits a
/// quarter of the sheet (live: 60 × 20 at 1:1, 40 × 20 at 2:1). Without MBD
/// data, or on a layout other than the suggested one, nothing changes.
pub fn trim_suggested_views(drawing: &mut Drawing, doc: &Document) {
    use super::{Projection, ViewKind, ViewOrientation as O};
    let Some(sheet) = drawing.sheets.first() else { return };
    if sheet.views.len() != 4 || !sheet.views.iter().map(|v| v.id.0).eq(1..=4) {
        return;
    }
    let Some(ViewKind::Base { source, config, part, .. }) = sheet.view(ViewId(1)).map(|v| v.kind.clone()) else { return };
    let Some(TabContent::PartStudio(ps)) = doc.tab(source).map(|t| &t.content) else { return };
    if ps.annotations.is_empty() {
        return;
    }
    let candidates = if drawing.projection == Projection::ThirdAngle { [O::Front, O::Top, O::Right] } else { [O::Front, O::Left, O::Top] };
    let mut chosen: Vec<O> = Vec::new();
    for m in &ps.annotations {
        let shown = |o: O| shows(m, &views::orientation_frame(o));
        if chosen.iter().any(|&o| shown(o)) {
            continue;
        }
        if let Some(o) = candidates.into_iter().find(|&o| shown(o)) {
            chosen.push(o);
        }
    }
    if chosen.is_empty() {
        return;
    }
    let frames: Vec<Option<ViewFrame>> = (1..=4).map(|i| views::view_frame(drawing, sheet, ViewId(i))).collect();
    let same = |f: &Option<ViewFrame>, o: O| f.as_ref().is_some_and(|f| f.dir.dot(views::orientation_frame(o).dir) > 0.999);
    if chosen.contains(&O::Front) && chosen.iter().all(|&o| (1..=3).any(|i| same(&frames[i - 1], o))) {
        // The suggested layout's views that are chosen.
        let keep: Vec<u64> = (1..=3u64).filter(|&i| chosen.iter().any(|&o| same(&frames[i as usize - 1], o))).collect();
        drawing.sheets[0].views.retain(|v| keep.contains(&v.id.0));
        return;
    }
    // Base views of the chosen orientations, side by side, centred.
    let (w, h) = drawing.sheets[0].size.size_mm();
    let src = super::create::DrawingSource { tab: source, part: part.clone(), config: config.clone(), sketch: None, flat: false };
    let (lo, hi) = super::create::source_bounds(doc, &src).unwrap_or((DVec3::splat(-0.05), DVec3::splat(0.05)));
    let size = |o: O| {
        let f = views::orientation_frame(o);
        let e = (hi - lo) * 1000.0;
        let span = |d: DVec3| (d.x * e.x).abs() + (d.y * e.y).abs() + (d.z * e.z).abs();
        [span(f.right), span(f.up)]
    };
    let gap = 25.0;
    let (sum_w, max_h) = chosen.iter().fold((gap * (chosen.len() as f64 - 1.0), 0.0f64), |(sw, mh), &o| (sw + size(o)[0], mh.max(size(o)[1])));
    let scale = super::create::SCALES.iter().copied().find(|s| (sum_w - gap * (chosen.len() as f64 - 1.0)) * s <= w * 0.25 * chosen.len() as f64 && max_h * s <= h * 0.25).unwrap_or(1.0);
    let total = chosen.iter().map(|&o| size(o)[0] * scale).sum::<f64>() + gap * (chosen.len() as f64 - 1.0);
    let template = drawing.sheets[0].view(ViewId(1)).cloned().expect("front view");
    let mut x = (w - total) / 2.0;
    let mut out = Vec::new();
    for (i, &o) in chosen.iter().enumerate() {
        let vw = size(o)[0] * scale;
        out.push(super::DrawingView { id: ViewId(i as u64 + 1), kind: ViewKind::Base { source, config: config.clone(), orientation: o, part: part.clone() }, position: [x + vw / 2.0, h / 2.0], ..template.clone() });
        x += vw + gap;
    }
    let sheet = &mut drawing.sheets[0];
    sheet.scale = scale;
    sheet.views = out;
}

/// Place the source Part Studios' datums in a new drawing (Suggested views):
/// each in the first view, in sheet order, that shows its face edge-on and
/// shows annotations from the model.
pub fn place_model_datums(drawing: &mut Drawing, doc: &Document) {
    let mut placed: Vec<(TabId, u32)> = drawing.sheets.iter().flat_map(|s| &s.annotations).filter_map(|a| a.model.as_ref().map(|m| (m.tab, m.id))).collect();
    // Each one goes to one view, in live's priority (help creating_a_drawing:
    // "top, right, front, then parent views over children, then insertion
    // order").
    let mut views_by_priority: Vec<(Priority, usize, ViewId)> = Vec::new();
    for (si, sheet) in drawing.sheets.iter().enumerate() {
        for (vi, v) in sheet.views.iter().enumerate().filter(|(_, v)| v.model_annotations) {
            let Some(frame) = views::view_frame(drawing, sheet, v.id) else { continue };
            views_by_priority.push(((direction_rank(frame.dir), u8::from(!matches!(v.kind, super::ViewKind::Base { .. })), si, vi), si, v.id));
        }
    }
    views_by_priority.sort_by_key(|(rank, _, _)| *rank);
    let mut add: Vec<(usize, Annotation)> = Vec::new();
    for (_, si, vid) in views_by_priority {
        let sheet = &drawing.sheets[si];
        let Some(v) = sheet.view(vid) else { continue };
        let Some((key, _)) = views::view_source(sheet, v.id) else { continue };
        let Some(TabContent::PartStudio(ps)) = doc.tab(key.tab).map(|t| &t.content) else { continue };
        let Some(frame) = views::view_frame(drawing, sheet, v.id) else { continue };
        for m in &ps.annotations {
            if placed.contains(&(key.tab, m.id)) || !shows(m, &frame) {
                continue;
            }
            placed.push((key.tab, m.id));
            // Laid out on the sheet when it is built (`resolve_model_annotations`).
            let kind = match &m.kind {
                ModelKind::Datum { letter } => AnnotationKind::Datum { anchor: v.position, label: letter.clone(), on: None, at: None },
                ModelKind::Dimension { .. } => AnnotationKind::LinearDimension { view: v.id, a: [0.0; 2], b: [0.0; 2], offset: 0.0, orientation: super::DimOrientation::Aligned, override_text: None },
            };
            add.push((si, Annotation { kind, position: v.position, model: Some(ModelRef { tab: key.tab, id: m.id, view: v.id, offset: DEFAULT_OFFSET, hidden: false }), ..Default::default() }));
        }
    }
    for (si, mut a) in add {
        let sheet = &mut drawing.sheets[si];
        a.id = sheet.next_annotation_id();
        sheet.annotations.push(a);
    }
}

/// A view's rank for model annotations: direction, derived, sheet, index.
type Priority = (u8, u8, usize, usize);

/// A view's place in the model-annotation priority by its direction: Top,
/// Right, Front, then the others.
fn direction_rank(dir: DVec3) -> u8 {
    let d = dir.normalize_or_zero();
    if d.dot(DVec3::NEG_Z) > 0.999 {
        0
    } else if d.dot(DVec3::NEG_X) > 0.999 {
        1
    } else if d.dot(DVec3::Y) > 0.999 {
        2
    } else {
        3
    }
}

/// Where a model datum sits now: its anchor on the face's edge and its box
/// square to the edge (sheet mm), from the view's geometry and its sheet
/// placement `to_sheet` (view coordinates → sheet). The model symbol lies in
/// the face, edge-on here, so it can't choose the side: live puts the box
/// below the edge (drawing/1070, mbd-drawing-json: A under the Front view's
/// top edge, inside the view; B under the Top view's bottom edge). A
/// vertical edge keeps the face's outward side.
pub fn model_datum_points(m: &ModelAnnotation, frame: &ViewFrame, to_sheet: &dyn Fn([f64; 2]) -> [f64; 2], offset: f64) -> Option<([f64; 2], [f64; 2])> {
    let at = DVec3::from_array(m.at);
    let n = DVec3::from_array(m.normal).normalize_or_zero();
    if n == DVec3::ZERO {
        return None;
    }
    let a = to_sheet(frame.project(at));
    let b = to_sheet(frame.project(at + n * 0.001));
    let mut d = [b[0] - a[0], b[1] - a[1]];
    let l = (d[0] * d[0] + d[1] * d[1]).sqrt();
    if l < 1e-9 {
        return None;
    }
    if d[1] > 1e-6 * l {
        d = [-d[0], -d[1]];
    }
    Some((a, [a[0] + d[0] / l * offset, a[1] + d[1] / l * offset]))
}

/// The model datums of `sheet` laid out for drawing: hidden ones, those of
/// views showing no model annotations and those whose face is gone are left
/// out; the others get their anchor and box position.
pub fn resolve_model_annotations(sheet: &mut Sheet, geoms: &HashMap<ViewId, Arc<ViewGeometry>>, sources: &Sources, to_sheet: &dyn Fn(ViewId, [f64; 2]) -> [f64; 2]) {
    let show: HashMap<ViewId, bool> = sheet.views.iter().map(|v| (v.id, v.model_annotations)).collect();
    let keys: HashMap<ViewId, views::SourceKey> = sheet.views.iter().filter_map(|v| views::view_source(sheet, v.id).map(|k| (v.id, k.0))).collect();
    sheet.annotations.retain_mut(|a| {
        let Some(m) = &a.model else { return true };
        if m.hidden || !show.get(&m.view).copied().unwrap_or(false) {
            return false;
        }
        let Some(frame) = geoms.get(&m.view).and_then(|g| g.frame) else { return false };
        let Some(src) = keys.get(&m.view).and_then(|k| sources.get(k)) else { return false };
        let Some(ma) = src.model_annotations.iter().find(|x| x.id == m.id) else { return false };
        let view = m.view;
        if let ModelKind::Dimension { .. } = &ma.kind {
            // Its value, ends and text from the model (a dimension whose
            // entities are gone isn't drawn).
            let Some((kind, pos)) = dimension_kind(&src.context, ma, view, &frame, &|p| to_sheet(view, p)) else { return false };
            a.kind = kind;
            a.position = pos;
            a.format = dimension_format(ma, &a.format);
            return true;
        }
        let Some((anchor, pos)) = model_datum_points(ma, &frame, &|p| to_sheet(view, p), m.offset) else { return false };
        let ModelKind::Datum { letter } = &ma.kind else { return false };
        a.kind = AnnotationKind::Datum { anchor, label: letter.clone(), on: None, at: None };
        a.position = pos;
        true
    });
}

/// One MBD requirement of the drawing's sources, for the MBD status panel.
#[derive(Clone, Debug, PartialEq)]
pub struct MbdRow {
    pub tab: TabId,
    pub id: u32,
    pub name: String,
    /// Shown in some view of the drawing.
    pub visible: bool,
    /// A dimension (the Characteristics table), else a datum (Datums).
    pub dimension: bool,
}

/// An MBD dimension's Characteristics cells: Type, Nominal Value,
/// Tolerance, Upper Limit, Lower Limit (`value` in mm or degrees; live
/// request 1721: "Distance 30.0 · ±0.1 · 30.1 · 29.9", "Radius 3.000 · --
/// · -- · --").
pub fn characteristic_cells(m: &ModelAnnotation, value: Option<f64>) -> Option<[String; 5]> {
    let ModelKind::Dimension { measure, tolerance: t, .. } = &m.kind else { return None };
    let places = t.precision.map_or(3, usize::from);
    let num = |x: f64| format!("{x:.places$}");
    let dev = |s: &str| s.trim().trim_end_matches("mm").trim_end_matches("deg").trim_end_matches('°').trim().parse::<f64>().unwrap_or(0.0);
    let (u, l) = (dev(&t.upper), dev(&t.lower));
    let none = || "--".to_string();
    let (tol, upper, lower) = match (t.kind.as_str(), value) {
        ("Symmetrical", Some(v)) => (format!("±{}", num(u)), num(v + u), num(v - u)),
        ("Deviation" | "Limits", Some(v)) => (format!("+{} / -{}", num(u), num(l)), num(v + u), num(v - l)),
        ("Min" | "Max" | "Basic", _) => (t.kind.clone(), none(), none()),
        _ => (none(), none(), none()),
    };
    Some([measure.label().to_string(), value.map_or_else(none, num), tol, upper, lower])
}

/// The MBD datums of the Part Studios the drawing's views show (all sheets,
/// or `sheet` only), with whether each is visible in some view.
pub fn mbd_rows(drawing: &Drawing, doc: &Document, sheet: Option<usize>) -> Vec<MbdRow> {
    let mut tabs: Vec<TabId> = Vec::new();
    for (si, s) in drawing.sheets.iter().enumerate() {
        if sheet.is_some_and(|x| x != si) {
            continue;
        }
        for v in &s.views {
            if let Some((k, _)) = views::view_source(s, v.id)
                && !tabs.contains(&k.tab)
            {
                tabs.push(k.tab);
            }
        }
    }
    let mut out = Vec::new();
    for tab in tabs {
        let Some(TabContent::PartStudio(ps)) = doc.tab(tab).map(|t| &t.content) else { continue };
        for m in &ps.annotations {
            let name = match &m.kind {
                ModelKind::Datum { letter } => letter.clone(),
                ModelKind::Dimension { measure, .. } => measure.label().to_string(),
            };
            let visible = drawing.sheets.iter().any(|s| {
                s.annotations.iter().any(|a| a.model.as_ref().is_some_and(|r| r.tab == tab && r.id == m.id && !r.hidden && s.view(r.view).is_some_and(|v| v.model_annotations)))
            });
            out.push(MbdRow { tab, id: m.id, name, visible, dimension: matches!(m.kind, ModelKind::Dimension { .. }) });
        }
    }
    out
}

/// Methods on [`Drawing`] that need noshape-model's logic (in scope through the module's glob re-export).
pub trait ModelAnnotations {
    /// MBD status › Show: un-hide a model datum where it was (false when its
    /// view shows no model annotations: live's info toast).
    fn show_model_annotation(&mut self, tab: TabId, id: u32) -> Option<bool>;
}

impl ModelAnnotations for Drawing {
    fn show_model_annotation(&mut self, tab: TabId, id: u32) -> Option<bool> {
        for s in &mut self.sheets {
            let shows: HashMap<ViewId, bool> = s.views.iter().map(|v| (v.id, v.model_annotations)).collect();
            for a in &mut s.annotations {
                if let Some(m) = &mut a.model
                    && m.tab == tab
                    && m.id == id
                {
                    if !shows.get(&m.view).copied().unwrap_or(false) {
                        return Some(false);
                    }
                    m.hidden = false;
                    return Some(true);
                }
            }
        }
        None
    }
}
