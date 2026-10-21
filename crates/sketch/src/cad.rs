//! Shared reader for "Insert DXF or DWG": both formats are read into the
//! format-neutral `cadkit` model (ASCII / binary DXF via `cadkit-dxf`, DWG
//! R13–2018 via `cadkit-dwg`), then model space is walked with block
//! references expanded (INSERT placement, scale, rotation, arrays and nested
//! blocks) and every curve is mapped exactly onto the drawing's XY plane:
//! entities on planes that are mirrored or tilted, or placed by non-uniform
//! block scales, become the ellipses / elliptical arcs they project to.

use crate::dxf::{DxfDrawing, DxfEntity};
use crate::geom::Curve2;
use crate::nurbs::Nurbs;
use crate::sketch::{Result, SketchError};
use cadkit_core::geom_ops::{arbitrary_axes, Transform};
use cadkit_core::{Document, EntityKind, ModelKind, Point3, ReadOptions, Vec3};
use glam::{DVec2, DVec3};
use std::f64::consts::{PI, TAU};

/// Meters per unit of a DXF / DWG `$INSUNITS` code.
pub(crate) fn insunits(code: i64) -> Option<f64> {
    Some(match code {
        1 => 0.0254,
        2 => 0.3048,
        4 => 0.001,
        5 => 0.01,
        6 => 1.0,
        8 => 0.0254e-6,
        9 => 0.0254e-3,
        10 => 0.9144,
        14 => 0.1,
        _ => return None,
    })
}

fn v3(v: Vec3) -> DVec3 {
    DVec3::new(v.x, v.y, v.z)
}

fn p3(p: Point3) -> DVec3 {
    DVec3::new(p.x, p.y, p.z)
}

/// An affine map from entity coordinates to the drawing plane (XY).
#[derive(Clone, Copy)]
struct Map {
    x: DVec3,
    y: DVec3,
    z: DVec3,
    t: DVec3,
}

impl Map {
    fn from(t: &Transform) -> Map {
        Map { x: v3(t.x), y: v3(t.y), z: v3(t.z), t: v3(t.t) }
    }
    fn vec(&self, v: DVec3) -> DVec2 {
        let w = self.x * v.x + self.y * v.y + self.z * v.z;
        DVec2::new(w.x, w.y)
    }
    fn point(&self, p: DVec3) -> DVec2 {
        self.vec(p) + DVec2::new(self.t.x, self.t.y)
    }
}

/// Increases `e` by full turns until it lies after `s`.
fn after(s: f64, mut e: f64) -> f64 {
    while e <= s {
        e += TAU;
    }
    e
}

/// The curve `c + u cos t + v sin t` for `t` in `range` (`None`: the full
/// curve) as an exact sketch curve: a circle / arc when `u`, `v` are an
/// orthogonal pair of equal length, an ellipse / elliptical arc otherwise,
/// a line when it is seen edge-on.
fn conic(c: DVec2, u: DVec2, v: DVec2, range: Option<(f64, f64)>) -> Option<Curve2> {
    let (lu, lv) = (u.length(), v.length());
    let scale = lu.max(lv);
    if scale < 1e-300 {
        return None;
    }
    let cross = u.perp_dot(v);
    // Seen edge-on: a segment along the larger axis.
    if cross.abs() <= 1e-9 * scale * scale {
        let a = if lu >= lv { u } else { v };
        let (lo, hi) = match range {
            None => (-1.0, 1.0),
            Some((t0, t1)) => {
                // Extreme values of the projection onto `a` over the range.
                let f = |t: f64| (u * t.cos() + v * t.sin()).dot(a) / a.length_squared();
                let mut vals = vec![f(t0), f(t1)];
                let phase = v.dot(a).atan2(u.dot(a));
                for k in -4..=4 {
                    let t = phase + k as f64 * PI;
                    if t > t0 && t < t1 {
                        vals.push(f(t));
                    }
                }
                (vals.iter().cloned().fold(f64::INFINITY, f64::min), vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
            }
        };
        let (pa, pb) = (c + a * lo, c + a * hi);
        return ((pb - pa).length() > 1e-12 * scale).then_some(Curve2::Line { a: pa, b: pb });
    }
    let ccw = cross > 0.0;
    // Circle: equal, orthogonal conjugate radii.
    if (lu - lv).abs() <= 1e-9 * scale && u.dot(v).abs() <= 1e-9 * scale * scale {
        let phi = u.to_angle();
        return Some(match range {
            None => Curve2::Circle { center: c, radius: lu },
            Some((t0, t1)) => {
                let (s, e) = if ccw { (phi + t0, phi + t1) } else { (phi - t1, phi - t0) };
                Curve2::Arc { center: c, radius: lu, start_angle: s, end_angle: after(s, e) }
            }
        });
    }
    // Principal axes: the parameter where |u cos t + v sin t| is largest.
    let mut ts = 0.5 * (2.0 * u.dot(v)).atan2(u.length_squared() - v.length_squared());
    let at = |t: f64| u * t.cos() + v * t.sin();
    if at(ts + PI / 2.0).length() > at(ts).length() {
        ts += PI / 2.0;
    }
    let major = at(ts);
    let minor_vec = at(ts + PI / 2.0);
    let minor = minor_vec.length();
    // Curve2 runs center + major cos τ + minor perp(major) sin τ: τ = t - ts
    // when the pair turns counter-clockwise, τ = ts - t when mirrored.
    Some(match range {
        None => Curve2::Ellipse { center: c, major, minor },
        Some((t0, t1)) => {
            let (s, e) = if ccw { (t0 - ts, t1 - ts) } else { (ts - t1, ts - t0) };
            Curve2::EllipticalArc { center: c, major, minor, start_param: s, end_param: after(s, e) }
        }
    })
}

/// A full or partial curve given in its own plane (center, in-plane axes
/// scaled to the radii) mapped to the drawing plane.
fn planar(m: &Map, center: DVec3, u: DVec3, v: DVec3, range: Option<(f64, f64)>) -> Option<Curve2> {
    conic(m.point(center), m.vec(u), m.vec(v), range)
}

/// Arc of a polyline segment from `a` to `b` with a bulge, in the plane with
/// axes `ax`, `ay` (bulges turn counter-clockwise about the plane normal).
fn bulge_segment(m: &Map, a: DVec3, b: DVec3, bulge: f64, ax: DVec3, ay: DVec3) -> Option<Curve2> {
    let (a2, b2) = (DVec2::new(a.dot(ax), a.dot(ay)), DVec2::new(b.dot(ax), b.dot(ay)));
    let chord = b2 - a2;
    if chord.length() < 1e-12 {
        return None;
    }
    if bulge.abs() <= 1e-12 {
        return Some(Curve2::Line { a: m.point(a), b: m.point(b) });
    }
    let sweep = 4.0 * bulge.atan();
    let r = chord.length() / (2.0 * (sweep / 2.0).sin().abs());
    let h = r * (sweep / 2.0).cos();
    let center2 = (a2 + b2) * 0.5 + chord.normalize().perp() * h * sweep.signum();
    let s = (a2 - center2).to_angle();
    // Out-of-plane offset of the polyline (elevation) kept from the vertex.
    let normal = ax.cross(ay);
    let center = ax * center2.x + ay * center2.y + normal * a.dot(normal);
    let range = if sweep > 0.0 { (s, s + sweep) } else { (s + sweep, s) };
    planar(m, center, ax * r, ay * r, Some(range))
}

/// Appends the drawing-plane geometry of one model-space entity.
fn convert(kind: &EntityKind, m: &Map, out: &mut Vec<DxfEntity>) {
    let mut push = |c: Option<Curve2>| {
        if let Some(c) = c {
            out.push(DxfEntity::Curve(c));
        }
    };
    match kind {
        EntityKind::Point { position } => out.push(DxfEntity::Point(m.point(p3(*position)))),
        EntityKind::Line { start, end } => {
            let (a, b) = (m.point(p3(*start)), m.point(p3(*end)));
            if (b - a).length() > 1e-12 {
                out.push(DxfEntity::Curve(Curve2::Line { a, b }));
            }
        }
        EntityKind::Circle { center, radius, normal } => {
            let (ax, ay, _) = arbitrary_axes(*normal);
            push(planar(m, p3(*center), v3(ax) * *radius, v3(ay) * *radius, None));
        }
        EntityKind::Arc { center, radius, start_angle, end_angle, normal } => {
            let (ax, ay, _) = arbitrary_axes(*normal);
            push(planar(m, p3(*center), v3(ax) * *radius, v3(ay) * *radius, Some((*start_angle, after(*start_angle, *end_angle)))));
        }
        EntityKind::Ellipse { center, major_axis, ratio, start_param, end_param, normal } => {
            let u = v3(*major_axis);
            let n = v3(*normal).normalize_or(DVec3::Z);
            let v = n.cross(u).normalize_or_zero() * u.length() * *ratio;
            let full = (end_param - start_param - TAU).abs() < 1e-9 || (end_param - start_param).abs() < 1e-12;
            push(planar(m, p3(*center), u, v, (!full).then(|| (*start_param, after(*start_param, *end_param)))));
        }
        EntityKind::Polyline { vertices, closed, normal } => {
            let (ax, ay, _) = arbitrary_axes(*normal);
            let (ax, ay) = (v3(ax), v3(ay));
            let n = vertices.len();
            let segs = if *closed { n } else { n.saturating_sub(1) };
            for i in 0..segs {
                let (va, vb) = (&vertices[i], &vertices[(i + 1) % n]);
                push(bulge_segment(m, p3(va.position), p3(vb.position), va.bulge, ax, ay));
            }
        }
        EntityKind::Spline { degree, knots, control_points, weights, fit_points, .. } => {
            let degree = *degree as usize;
            // B-splines are affine invariant: map the control points.
            let ctrl: Vec<DVec2> = control_points.iter().map(|p| m.point(p3(*p))).collect();
            if ctrl.len() > degree && knots.len() == ctrl.len() + degree + 1 {
                let w = (weights.len() == ctrl.len()).then(|| weights.clone());
                let n = Nurbs { degree, knots: knots.clone(), ctrl, weights: w };
                if n.is_valid() {
                    out.push(DxfEntity::Curve(Curve2::Nurbs(n)));
                }
            } else {
                let fit: Vec<DVec2> = fit_points.iter().map(|p| m.point(p3(*p))).collect();
                if let Some(n) = Nurbs::interpolate(&fit) {
                    out.push(DxfEntity::Curve(Curve2::Nurbs(n)));
                }
            }
        }
        _ => {}
    }
}

/// Model-space geometry of a document, block references expanded.
/// Dimensions (their extension lines, arrows and text blocks) are
/// annotation, not drawing geometry, and are left out.
pub fn from_document(doc: &Document, format: &str) -> Result<DxfDrawing> {
    let mut out = DxfDrawing { entities: Vec::new(), unit_scale: insunits(doc.units.unit.to_insunits() as i64) };
    let mut doc = doc.clone();
    let not_dimension = |e: &cadkit_core::Entity| !matches!(e.kind, EntityKind::Dimension { .. });
    for m in &mut doc.models {
        m.entities.retain(not_dimension);
    }
    for b in &mut doc.blocks {
        b.entities.retain(not_dimension);
    }
    for (i, _) in doc.models.iter().enumerate().filter(|(_, m)| m.kind == ModelKind::Model) {
        doc.walk(i, 32, |e, ctx| convert(&e.kind, &Map::from(&ctx.transform), &mut out.entities));
    }
    if out.entities.is_empty() {
        return Err(SketchError::OperationFailed(format!("the {format} file contains no supported 2D geometry")));
    }
    Ok(out)
}

/// One space of a CAD file for an imported drawing: model space or a
/// paper-space layout, with display colours (ACI 7 resolves to black, the
/// light-background foreground) and texts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CadSpace {
    pub name: String,
    /// Model space (else a paper-space layout).
    pub model: bool,
    pub entities: Vec<(DxfEntity, (u8, u8, u8))>,
    pub texts: Vec<CadText>,
}

/// A TEXT / MTEXT of a [`CadSpace`] (plain text, lines split by `\n`).
#[derive(Clone, Debug, PartialEq)]
pub struct CadText {
    pub position: DVec2,
    pub height: f64,
    pub rotation: f64,
    pub text: String,
    pub color: (u8, u8, u8),
}

/// Every space of a DWG / DXF file (model space first, then the layouts in
/// file order) and meters per drawing unit.
pub fn parse_cad_spaces(bytes: &[u8]) -> Result<(Vec<CadSpace>, Option<f64>)> {
    let doc = if bytes.starts_with(b"AC10") {
        if !cadkit_dwg::sniff(bytes) {
            return Err(SketchError::OperationFailed("unsupported DWG version (R13 to 2018 are supported)".into()));
        }
        cadkit_dwg::read(bytes, &ReadOptions::default()).map_err(|e| SketchError::OperationFailed(format!("cannot read the DWG file: {e}")))?
    } else {
        let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
        if !cadkit_dxf::sniff(bytes) {
            return Err(SketchError::OperationFailed("not a DXF file".into()));
        }
        cadkit_dxf::read(bytes, &ReadOptions::default()).map_err(|e| SketchError::OperationFailed(format!("cannot read the DXF file: {e}")))?
    };
    let unit_scale = insunits(doc.units.unit.to_insunits() as i64);
    let mut spaces: Vec<CadSpace> = Vec::new();
    for (i, m) in doc.models.iter().enumerate() {
        let model = m.kind == ModelKind::Model;
        let mut space = CadSpace { name: m.name.clone(), model, ..Default::default() };
        doc.walk(i, 32, |e, ctx| {
            if matches!(e.kind, EntityKind::Dimension { .. }) || !e.visible {
                return;
            }
            let color = e.resolved_color_in(&doc, ctx);
            let map = Map::from(&ctx.transform);
            match &e.kind {
                EntityKind::Text { position, height, rotation, value, .. } => space.texts.push(CadText { position: map.point(p3(*position)), height: *height, rotation: *rotation, text: value.clone(), color }),
                EntityKind::MText { position, height, rotation, plain, .. } => space.texts.push(CadText { position: map.point(p3(*position)), height: *height, rotation: *rotation, text: plain.clone(), color }),
                kind => {
                    let mut out = Vec::new();
                    convert(kind, &map, &mut out);
                    space.entities.extend(out.into_iter().map(|c| (c, color)));
                }
            }
        });
        if model {
            spaces.insert(spaces.iter().position(|s| !s.model).unwrap_or(spaces.len()), space);
        } else {
            spaces.push(space);
        }
    }
    Ok((spaces, unit_scale))
}

/// Reads an ASCII or binary DXF file.
pub fn read_dxf(bytes: &[u8]) -> Result<DxfDrawing> {
    if !cadkit_dxf::sniff(bytes) {
        return Err(SketchError::OperationFailed("not a DXF file".into()));
    }
    let doc = cadkit_dxf::read(bytes, &ReadOptions::default()).map_err(|e| SketchError::OperationFailed(format!("cannot read the DXF file: {e}")))?;
    from_document(&doc, "DXF")
}

/// Reads a DWG (`AC10xx` magic) or DXF (ASCII or binary) file, chosen by
/// content.
pub fn parse_cad(bytes: &[u8]) -> Result<DxfDrawing> {
    if bytes.starts_with(b"AC10") {
        return crate::dwg::parse_dwg(bytes);
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    read_dxf(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadkit_core::{Block, Entity};

    fn identity() -> Map {
        Map::from(&Transform::identity())
    }

    fn approx(a: DVec2, b: DVec2) -> bool {
        (a - b).length() < 1e-9
    }

    #[test]
    fn spaces_keep_colours_and_texts() {
        // Model space: a red (ACI 1) line, an ACI 7 circle and a TEXT.
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n62\n1\n10\n0\n20\n0\n11\n10\n21\n0\n0\nCIRCLE\n8\n0\n62\n7\n10\n5\n20\n5\n40\n2\n0\nTEXT\n8\n0\n10\n1\n20\n8\n40\n2.5\n1\nHELLO\n0\nENDSEC\n0\nEOF\n";
        let (spaces, _) = parse_cad_spaces(dxf.as_bytes()).unwrap();
        let model = spaces.iter().find(|s| s.model).expect("model space");
        assert!(model.entities.iter().any(|(e, c)| matches!(e, DxfEntity::Curve(Curve2::Line { .. })) && *c == (255, 0, 0)), "{:?}", model.entities);
        assert!(model.entities.iter().any(|(e, c)| matches!(e, DxfEntity::Curve(Curve2::Circle { .. })) && *c == (0, 0, 0)));
        let t = model.texts.iter().find(|t| t.text == "HELLO").expect("text");
        assert!((t.height - 2.5).abs() < 1e-9 && (t.position - DVec2::new(1.0, 8.0)).length() < 1e-9);
    }

    #[test]
    fn conic_recognises_circles_ellipses_and_edge_on_views() {
        let c = DVec2::new(1.0, 2.0);
        assert_eq!(conic(c, DVec2::new(2.0, 0.0), DVec2::new(0.0, 2.0), None), Some(Curve2::Circle { center: c, radius: 2.0 }));
        // Mirrored quarter arc: clockwise from +X to -Y.
        let Some(Curve2::Arc { start_angle, end_angle, .. }) = conic(c, DVec2::new(2.0, 0.0), DVec2::new(0.0, -2.0), Some((0.0, PI / 2.0))) else { panic!() };
        assert!(((start_angle.rem_euclid(TAU)) - 1.5 * PI).abs() < 1e-12 && (end_angle - start_angle - PI / 2.0).abs() < 1e-12);
        // Non-uniform scale: an ellipse with the larger axis along Y.
        let Some(e @ Curve2::Ellipse { .. }) = conic(c, DVec2::new(1.0, 0.0), DVec2::new(0.0, 3.0), None) else { panic!() };
        let Curve2::Ellipse { major, minor, .. } = e else { unreachable!() };
        assert!((major.length() - 3.0).abs() < 1e-12 && (minor - 1.0).abs() < 1e-12);
        // Elliptical arc points match the conjugate form for both orientations.
        for v in [DVec2::new(0.5, 2.0), DVec2::new(0.5, -2.0)] {
            let u = DVec2::new(1.5, 0.3);
            let arc = conic(c, u, v, Some((0.3, 2.0))).unwrap();
            let (t0, t1) = arc.domain();
            let ends = [arc.point(t0), arc.point(t1)];
            let want = [c + u * 0.3f64.cos() + v * 0.3f64.sin(), c + u * 2.0f64.cos() + v * 2.0f64.sin()];
            assert!(ends.iter().all(|p| want.iter().any(|q| approx(*p, *q))), "{ends:?} {want:?}");
            let mid = arc.point(0.5 * (t0 + t1));
            let wm = c + u * 1.15f64.cos() + v * 1.15f64.sin();
            assert!(approx(mid, wm), "{mid} {wm}");
        }
        // A circle seen edge-on is a segment across its diameter.
        assert_eq!(conic(DVec2::ZERO, DVec2::new(1.0, 0.0), DVec2::ZERO, None), Some(Curve2::Line { a: DVec2::new(-1.0, 0.0), b: DVec2::new(1.0, 0.0) }));
    }

    #[test]
    fn tilted_and_mirrored_planes_project_onto_xy() {
        let m = identity();
        let mut out = Vec::new();
        // Circle on a plane tilted 60° about X: an ellipse with minor r cos 60°.
        let n = Vec3::new(0.0, -(PI / 3.0).sin(), (PI / 3.0).cos());
        convert(&EntityKind::Circle { center: Point3::xy(0.0, 0.0), radius: 2.0, normal: n }, &m, &mut out);
        let DxfEntity::Curve(Curve2::Ellipse { major, minor, .. }) = out[0] else { panic!("{out:?}") };
        assert!((major.length() - 2.0).abs() < 1e-9 && (minor - 1.0).abs() < 1e-9);
        // Polyline with a +1 bulge about -Z: the half circle is above the chord.
        out.clear();
        let vtx = |x: f64, b: f64| cadkit_core::Vertex { bulge: b, ..cadkit_core::Vertex::at(Point3::xy(x, 0.0)) };
        convert(&EntityKind::Polyline { vertices: vec![vtx(0.0, 1.0), vtx(2.0, 0.0)], closed: false, normal: Vec3::new(0.0, 0.0, -1.0) }, &m, &mut out);
        let DxfEntity::Curve(arc) = &out[0] else { panic!() };
        let (t0, t1) = arc.domain();
        assert!(arc.point(0.5 * (t0 + t1)).y > 0.99, "{arc:?}");
    }

    #[test]
    fn block_references_are_expanded_with_placement_scale_rotation_and_arrays() {
        let mut doc = Document::default();
        doc.blocks.push(Block {
            name: "B".into(),
            base_point: Point3::xy(1.0, 0.0),
            entities: vec![
                Entity::new(EntityKind::Line { start: Point3::xy(1.0, 0.0), end: Point3::xy(2.0, 0.0) }),
                Entity::new(EntityKind::Circle { center: Point3::xy(1.0, 0.0), radius: 1.0, normal: Vec3::Z }),
            ],
            ..Default::default()
        });
        let insert = |pos: Point3, scale: Vec3, rotation: f64, columns: u32| {
            Entity::new(EntityKind::Insert { block: "B".into(), position: pos, scale, rotation, normal: Vec3::Z, columns, rows: 1, column_spacing: 10.0, row_spacing: 0.0 })
        };
        doc.models.push(cadkit_core::Model {
            kind: ModelKind::Model,
            entities: vec![insert(Point3::xy(5.0, 5.0), Vec3::new(2.0, 1.0, 1.0), PI / 2.0, 1), insert(Point3::xy(0.0, 0.0), Vec3::new(1.0, 1.0, 1.0), 0.0, 2)],
            ..Default::default()
        });
        let d = from_document(&doc, "DXF").unwrap();
        assert_eq!(d.entities.len(), 6, "{:?}", d.entities);
        // First insert: base (1,0) → (5,5), x scaled by 2 then rotated 90°.
        let DxfEntity::Curve(Curve2::Line { a, b }) = d.entities[0] else { panic!() };
        assert!(approx(a, DVec2::new(5.0, 5.0)) && approx(b, DVec2::new(5.0, 7.0)), "{a} {b}");
        let DxfEntity::Curve(Curve2::Ellipse { center, major, minor }) = d.entities[1] else { panic!("{:?}", d.entities[1]) };
        assert!(approx(center, DVec2::new(5.0, 5.0)) && (major.length() - 2.0).abs() < 1e-9 && major.x.abs() < 1e-9 && (minor - 1.0).abs() < 1e-9);
        // Arrayed insert: two columns 10 apart.
        let lines: Vec<DVec2> = d.entities.iter().filter_map(|e| match e { DxfEntity::Curve(Curve2::Line { a, .. }) => Some(*a), _ => None }).collect();
        assert!(lines.iter().any(|p| approx(*p, DVec2::ZERO)) && lines.iter().any(|p| approx(*p, DVec2::new(10.0, 0.0))), "{lines:?}");
    }
}
