//! Part Studio "Export as DXF/DWG…" of a sketch or a planar face (context
//! menus): its curves exactly in the plane's coordinates, written as live's (the flat export's tables, every curve on
//! MODELSKETCH_VISIBLE); as sheet geometry for SVG. Shared by the app's menus and the agent API.

use glam::{DVec2, DVec3};
use noshape_model::display::{PlaneFrame, SketchDisplay};
use noshape_model::drawing::sheet::{LineStyle, Polyline2, SheetGeometry};
use noshape_model::reference::Reference;
use noshape_model::regen::context::Context;
use noshape_model::sheetmetal::flat::{FlatCurve, outline_curves, sketch_flat_curve};

/// Samples of a conic or spline written out (the writer fits or matches them exactly).
const SAMPLES: usize = 96;

/// The sheet being built: each curve exact in f64, a conic's centre and conjugate semi-diameters kept so the
/// writer puts its CIRCLE / ARC / ELLIPSE (not the display's f32 polyline: qa SKETCH-DXF-EXPORT-POLYLINES).
struct Sheet<'a> {
    frame: &'a PlaneFrame,
    g: SheetGeometry,
}

impl Sheet<'_> {
    fn local(&self, p: DVec3) -> [f64; 2] {
        let d = p - self.frame.origin;
        [d.dot(self.frame.x) * 1000.0, d.dot(self.frame.y) * 1000.0]
    }

    fn vector(&self, v: DVec3) -> [f64; 2] {
        [v.dot(self.frame.x) * 1000.0, v.dot(self.frame.y) * 1000.0]
    }

    /// One curve: its points (world), whether it is closed, its conic (world centre and semi-diameters), whether
    /// it is free-form (a spline).
    fn put(&mut self, points: Vec<DVec3>, closed: bool, conic: Option<[DVec3; 3]>, free: bool) {
        let li = self.g.lines.len();
        let points = points.into_iter().map(|p| self.local(p)).collect();
        if let Some([c, a, b]) = conic {
            self.g.conic_lines.push((li, [self.local(c), self.vector(a), self.vector(b)]));
        }
        if free {
            self.g.freeform_lines.push(li);
        }
        self.g.lines.push(Polyline2 { points, style: LineStyle::Visible, closed, pick: 0 });
    }

    fn done(mut self) -> SheetGeometry {
        let (mut w, mut h) = (0.0f64, 0.0f64);
        for p in self.g.lines.iter().flat_map(|l| &l.points) {
            w = w.max(p[0].abs());
            h = h.max(p[1].abs());
        }
        (self.g.width, self.g.height) = (w * 2.0, h * 2.0);
        self.g
    }
}

/// Samples of a curve over `[t0, t1]` (both ends; a closed curve's end left out).
fn samples(t0: f64, t1: f64, closed: bool, at: impl Fn(f64) -> DVec3) -> Vec<DVec3> {
    let n = SAMPLES;
    let last = if closed { n - 1 } else { n };
    (0..=last).map(|i| at(t0 + (t1 - t0) * i as f64 / n as f64)).collect()
}

/// A sketch as sheet geometry (SVG): its curves, not construction ones or the external references.
pub fn sketch_sheet(ctx: &Context, sk: &SketchDisplay) -> SheetGeometry {
    use noshape_sketch::Curve2;
    let mut s = Sheet { frame: &sk.frame, g: SheetGeometry::new(0.0, 0.0) };
    for r in ctx.sketches.iter().filter(|r| r.feature == sk.feature) {
        let w = |p: DVec2| r.frame.to_world(p);
        let v = |p: DVec2| r.frame.to_world(p) - r.frame.to_world(DVec2::ZERO);
        for e in r.sketch.entities().filter(|e| !e.external && !e.construction && r.holds(e.id.0)) {
            let Some(c) = r.sketch.curve(e.id) else { continue };
            let (t0, t1) = c.domain();
            match &c {
                Curve2::Line { a, b } => s.put(vec![w(*a), w(*b)], false, None, false),
                Curve2::Circle { center, radius } | Curve2::Arc { center, radius, .. } => {
                    let closed = matches!(c, Curve2::Circle { .. });
                    s.put(samples(t0, t1, closed, |t| w(c.point(t))), closed, Some([w(*center), v(DVec2::X * *radius), v(DVec2::Y * *radius)]), false)
                }
                Curve2::Ellipse { center, major, minor } | Curve2::EllipticalArc { center, major, minor, .. } => {
                    let closed = matches!(c, Curve2::Ellipse { .. });
                    let b = major.perp().normalize_or_zero() * *minor;
                    s.put(samples(t0, t1, closed, |t| w(c.point(t))), closed, Some([w(*center), v(*major), v(b)]), false)
                }
                Curve2::Nurbs(_) => {
                    let closed = c.point(t0).distance(c.point(t1)) < 1e-12;
                    s.put(samples(t0, t1, closed, |t| w(c.point(t))), closed, None, true)
                }
            }
        }
    }
    s.done()
}

/// A planar face as sheet geometry: its boundary in the face's plane.
pub fn face_sheet(ctx: &Context, face: &Reference) -> Option<SheetGeometry> {
    let Reference::Face(n) = face else { return None };
    let shape = ctx.bodies.iter().find_map(|b| b.shape.sub_shape(noshape_kernel::ShapeType::Face, *b.find_faces(n).first()?).ok())?;
    let pl = shape.face_plane().ok()??;
    let (normal, x) = (pl.unit_normal(), pl.unit_x());
    let frame = PlaneFrame { origin: pl.origin, x, y: normal.cross(x), normal };
    let mut s = Sheet { frame: &frame, g: SheetGeometry::new(0.0, 0.0) };
    for e in shape.edges() {
        let Ok(info) = e.edge_info() else { continue };
        if info.degenerated {
            continue;
        }
        let (t0, t1) = (info.first_param, info.last_param);
        let along = |t: f64| e.point_at(t).unwrap_or(info.start);
        match info.curve {
            noshape_kernel::Curve::Line { .. } => s.put(vec![info.start, info.end], false, None, false),
            noshape_kernel::Curve::Circle { center, axis, x_dir, radius } => {
                s.put(samples(t0, t1, info.closed, along), info.closed, Some([center, x_dir * radius, axis.cross(x_dir) * radius]), false)
            }
            noshape_kernel::Curve::Ellipse { center, axis, x_dir, major_radius, minor_radius } => {
                s.put(samples(t0, t1, info.closed, along), info.closed, Some([center, x_dir * major_radius, axis.cross(x_dir) * minor_radius]), false)
            }
            _ => s.put(samples(t0, t1, info.closed, along), info.closed, None, true),
        }
    }
    Some(s.done())
}

/// A sketch's curves for its DXF / DWG (live qa rt-1119: no construction curves, no points, no external references),
/// in its plane's coordinates.
pub fn sketch_curves(ctx: &Context, sketch: noshape_model::reference::FeatureId) -> Vec<FlatCurve> {
    let mut out = Vec::new();
    for r in ctx.sketches.iter().filter(|r| r.feature == sketch) {
        out.extend(r.sketch.entities().filter(|e| !e.external && !e.construction && r.holds(e.id.0)).filter_map(|e| Some(sketch_flat_curve(&r.sketch.curve(e.id)?, |p| p))));
    }
    out
}

/// A planar face's boundary for its DXF / DWG, in the face's plane: exact, or its edges sampled into lines where one
/// can't be written exactly (a periodic spline, an offset curve).
pub fn face_curves(ctx: &Context, face: &Reference) -> Option<Vec<FlatCurve>> {
    let Reference::Face(n) = face else { return None };
    let shape = ctx.bodies.iter().find_map(|b| b.shape.sub_shape(noshape_kernel::ShapeType::Face, *b.find_faces(n).first()?).ok())?;
    let plane = shape.face_plane().ok()??;
    if let Some(c) = outline_curves(std::slice::from_ref(&shape), &plane) {
        return Some(c);
    }
    let mut out = Vec::new();
    for e in shape.edges() {
        let Ok(info) = e.edge_info() else { continue };
        let pts = samples(info.first_param, info.last_param, false, |t| e.point_at(t).unwrap_or(info.start));
        out.extend(pts.windows(2).map(|w| FlatCurve::Line { a: plane.to_local(w[0]), b: plane.to_local(w[1]) }));
    }
    Some(out)
}

/// Curves as a DXF at the Export as DXF/DWG… dialog's defaults (Release 14, millimeters).
pub fn dxf(curves: &[FlatCurve]) -> Result<Vec<u8>, crate::OpError> {
    noshape_model::sheetmetal::flat_dxf::curves_dxf(curves, &noshape_model::sheetmetal::flat_dxf::FlatExport::default()).map_err(|e| crate::OpError::new("export", e))
}
