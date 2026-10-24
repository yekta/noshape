//! Part Studio "Export as DXF/DWG…" of a sketch or a planar face (context
//! menus): the curves as flat sheet geometry in the plane's coordinates,
//! millimeters. Shared by the app's menus and the agent API.

use noshape_model::display::{PlaneFrame, SketchDisplay, StudioDisplay};
use noshape_model::drawing::sheet::{LineStyle, Polyline2, SheetGeometry};
use noshape_model::reference::Reference;

/// Flat sheet (millimeters, plane coordinates) of world polylines (`true`:
/// drawn as hidden lines).
pub fn plane_sheet(frame: &PlaneFrame, polylines: Vec<(Vec<[f32; 3]>, bool)>) -> SheetGeometry {
    let local = |p: [f32; 3]| {
        let d = glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64) - frame.origin;
        [d.dot(frame.x) * 1000.0, d.dot(frame.y) * 1000.0]
    };
    let lines: Vec<Polyline2> = polylines.into_iter().map(|(pts, hidden)| Polyline2 { points: pts.into_iter().map(local).collect(), style: if hidden { LineStyle::Hidden } else { LineStyle::Visible }, closed: false, pick: 0 }).collect();
    let (mut w, mut h) = (0.0f64, 0.0f64);
    for l in &lines {
        for p in &l.points {
            w = w.max(p[0].abs());
            h = h.max(p[1].abs());
        }
    }
    let mut g = SheetGeometry::new(w * 2.0, h * 2.0);
    g.lines = lines;
    g
}

/// A sketch as sheet geometry: its curves (construction ones as hidden
/// lines), not the external references.
pub fn sketch_sheet(sk: &SketchDisplay) -> SheetGeometry {
    plane_sheet(&sk.frame, sk.curves.iter().filter(|c| c.points.len() >= 2 && !c.external).map(|c| (c.points.clone(), c.construction)).collect())
}

/// A planar face as sheet geometry: its boundary in the face's plane.
pub fn face_sheet(ctx: &noshape_model::regen::context::Context, display: &StudioDisplay, face: &Reference) -> Option<SheetGeometry> {
    let Reference::Face(n) = face else { return None };
    let frame = ctx.bodies.iter().find_map(|b| {
        let fi = *b.find_faces(n).first()?;
        let pl = b.shape.sub_shape(noshape_kernel::ShapeType::Face, fi).ok()?.face_plane().ok()??;
        let (normal, x) = (pl.unit_normal(), pl.unit_x());
        Some(PlaneFrame { origin: pl.origin, x, y: normal.cross(x), normal })
    })?;
    let b = display.bodies.iter().find(|b| b.face_refs.contains(face))?;
    let fi = b.face_refs.iter().position(|x| x == face)?;
    let lines = b.mesh.face_edges(fi).into_iter().filter_map(|e| b.mesh.edges.get(e).map(|pl| (pl.clone(), false))).collect();
    Some(plane_sheet(&frame, lines))
}
