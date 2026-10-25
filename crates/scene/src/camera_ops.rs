//! Camera operations shared by the app's graphics areas and the agent API
//! (`view_camera`): what Zoom to fit, Zoom to window, Zoom in / out, Zoom to
//! selection, View normal to and Align view with vertical axis do to a
//! camera, and what they frame. The app animates to the result; the API
//! keeps it as the window's camera.

use glam::{DVec2, DVec3};
use noshape_model::display::StudioDisplay;
use noshape_model::reference::{FeatureId, Reference};
use noshape_render::camera::{Camera, Projection};

/// Z / Shift+Z zoom by this factor (Onshape's `zoomOut` / `zoomIn`).
pub const DEFAULT_ZOOM_FACTOR: f64 = 1.75;
/// Onshape's `canZoom`: the model stays at least this many pixels across…
pub const MIN_MODEL_PIXEL_SIZE: f64 = 100.0;
/// …and at most this many pixels per metre.
pub const MAX_PIXELS_PER_METRE: f64 = 16_777_216.0;

/// World size of one pixel at `p` (perspective: at its depth).
pub fn pixel_size_at(camera: &Camera, p: DVec3, viewport: DVec2) -> f64 {
    let wpp = camera.world_per_pixel(viewport.y);
    match camera.projection {
        Projection::Orthographic => wpp,
        Projection::Perspective => {
            let depth = (p - camera.eye()).dot(camera.forward());
            wpp * (depth / camera.distance.max(1e-12)).max(1e-6)
        }
    }
}

/// Keep the zoom within Onshape's limits for the model `bounds`.
pub fn clamp_zoom(camera: &mut Camera, bounds: Option<(DVec3, DVec3)>, viewport: DVec2) {
    let Some((lo, hi)) = bounds else { return };
    let diameter = (hi - lo).length();
    if diameter <= 0.0 {
        return;
    }
    let px = pixel_size_at(camera, (lo + hi) * 0.5, viewport);
    let s = if diameter / px <= MIN_MODEL_PIXEL_SIZE {
        diameter / px / MIN_MODEL_PIXEL_SIZE
    } else if 1.0 / px >= MAX_PIXELS_PER_METRE {
        1.0 / (MAX_PIXELS_PER_METRE * px)
    } else {
        return;
    };
    camera.distance *= s;
}

/// Camera after Z / Shift+Z (`zoomOut` / `zoomIn`): ×1.75 about the view centre.
pub fn zoom_step(camera: &Camera, zoom_in: bool, bounds: Option<(DVec3, DVec3)>, viewport: DVec2) -> Camera {
    let mut to = *camera;
    to.distance *= if zoom_in { 1.0 / DEFAULT_ZOOM_FACTOR } else { DEFAULT_ZOOM_FACTOR };
    clamp_zoom(&mut to, bounds, viewport);
    to
}

/// Zoom to fit: the box fills the view of `size` pixels, an overlay `inset`
/// pixels wide at the left kept clear when the rest is wide enough.
pub fn fit_camera_to_box(camera: &mut Camera, lo: DVec3, hi: DVec3, size: DVec2, inset: f64) {
    let inset = if size.x - inset >= size.x * 0.45 { inset.max(0.0) } else { 0.0 };
    let width = (size.x - inset).max(1.0);
    let aspect = if size.y > 0.0 { width / size.y } else { 1.0 };
    camera.fit_box(lo, hi, aspect);
    if inset > 0.0 && size.y > 0.0 {
        // Move the model right, into the middle of the uncovered part.
        camera.pan(DVec2::new(inset * 0.5, 0.0), size.y);
    }
}

/// Zoom to window: the box from `a` to `b` (view pixels) fills the view;
/// None for a box under 4 px.
pub fn zoom_to_window(camera: &Camera, a: DVec2, b: DVec2, size: DVec2) -> Option<Camera> {
    let (w, h) = ((a.x - b.x).abs(), (a.y - b.y).abs());
    if w < 4.0 || h < 4.0 || size.x <= 0.0 || size.y <= 0.0 {
        return None;
    }
    let mut to = *camera;
    // Bring the box center to the view center, then zoom about it.
    to.pan(size * 0.5 - (a + b) * 0.5, size.y);
    to.zoom_at((w / size.x).max(h / size.y), size * 0.5, size);
    Some(to)
}

/// View normal to a plane of normal `normal`: look into it, or from the
/// other side when already looking along it.
pub fn normal_forward(current_forward: DVec3, normal: DVec3) -> DVec3 {
    if current_forward.dot(-normal) > 0.999 { normal } else { -normal }
}

/// The up direction for viewing a face of normal `n` (assemblies): Y when
/// looking along Z, else Z.
pub fn face_normal_up(n: DVec3) -> DVec3 {
    if n.z.abs() > 0.99 { DVec3::Y } else { DVec3::Z }
}

/// Align view with vertical axis: the straight edge along `dir` becomes
/// vertical on screen (same viewing direction, the up that keeps it
/// closest to the current one). None when the edge points at the eye.
pub fn align_vertical(camera: &Camera, dir: DVec3) -> Option<(DVec3, DVec3)> {
    let fwd = camera.forward();
    let mut up = (dir - fwd * dir.dot(fwd)).try_normalize()?;
    if up.dot(camera.up()) < 0.0 {
        up = -up;
    }
    Some((fwd, up))
}

/// A Part Studio's parts and sketches (shown ones; `editing` is the sketch
/// being edited, always counted): what Zoom to fit starts from.
pub fn studio_fit_bounds(display: &StudioDisplay, show_sketches: bool, editing: Option<FeatureId>) -> Option<(DVec3, DVec3)> {
    let mut acc = display.bounds();
    let mut add = |q: DVec3| acc = Some(acc.map_or((q, q), |(lo, hi)| (lo.min(q), hi.max(q))));
    let p3 = |a: [f32; 3]| DVec3::new(a[0] as f64, a[1] as f64, a[2] as f64);
    for sk in &display.sketches {
        if editing != Some(sk.feature) && (sk.hidden || !show_sketches) {
            continue;
        }
        sk.curves.iter().flat_map(|c| c.points.iter()).for_each(|q| add(p3(*q)));
        sk.points.iter().for_each(|q| add(p3(q.position)));
    }
    acc
}

/// `base` (parts and sketches), plus the origin point if `origin` and the
/// planes `planes` takes (the default ones span ±75 mm).
pub fn studio_fit_box(display: &StudioDisplay, base: Option<(DVec3, DVec3)>, origin: bool, planes: impl Fn(&noshape_model::display::PlaneDisplay) -> bool) -> Option<(DVec3, DVec3)> {
    let mut acc = base;
    let mut add = |q: DVec3| acc = Some(acc.map_or((q, q), |(lo, hi)| (lo.min(q), hi.max(q))));
    if origin {
        add(DVec3::ZERO);
    }
    for pl in display.planes.iter().filter(|p| planes(p)) {
        let f = &pl.frame;
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            add(f.origin + f.x * (sx * pl.half_width) + f.y * (sy * pl.half_height));
        }
    }
    acc
}

/// Bounding box of the referenced geometry (parts, faces, edges, vertices,
/// sketches, planes, the origin): what Zoom to selection frames.
pub fn selection_bounds(display: &StudioDisplay, refs: &[Reference]) -> Option<(DVec3, DVec3)> {
    let mut lo = DVec3::splat(f64::INFINITY);
    let mut hi = DVec3::splat(f64::NEG_INFINITY);
    let mut add = |q: DVec3| {
        lo = lo.min(q);
        hi = hi.max(q);
    };
    let p3 = |a: [f32; 3]| DVec3::new(a[0] as f64, a[1] as f64, a[2] as f64);
    for r in refs {
        match r {
            Reference::Body(b) => {
                if let Some((a, c)) = display.body(b).and_then(|b| b.bounds) {
                    add(a);
                    add(c);
                }
            }
            Reference::SketchEntity { sketch, .. } | Reference::SketchRegion { sketch, .. } | Reference::Sketch(sketch) => {
                if let Some(sk) = display.sketches.iter().find(|s| s.feature == *sketch) {
                    sk.curves.iter().flat_map(|c| c.points.iter()).for_each(|q| add(p3(*q)));
                    sk.points.iter().for_each(|q| add(p3(q.position)));
                }
            }
            Reference::Plane(_) | Reference::FeaturePlane(_) => {
                if let Some(pl) = display.planes.iter().find(|p| &p.reference == r) {
                    for (u, v) in [(-1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (1.0, -1.0)] {
                        add(pl.frame.origin + pl.frame.x * (u * pl.half_width) + pl.frame.y * (v * pl.half_height));
                    }
                }
            }
            Reference::Origin => add(DVec3::ZERO),
            r => {
                for b in &display.bodies {
                    if let Some(fi) = b.face_refs.iter().position(|x| x == r)
                        && let Some(&(first, count)) = b.mesh.face_ranges.get(fi)
                    {
                        for &i in &b.mesh.indices[first as usize..(first + count) as usize] {
                            if let Some(q) = b.mesh.positions.get(i as usize) {
                                add(p3(*q));
                            }
                        }
                    } else if let Some(ei) = b.edge_refs.iter().position(|x| x == r) {
                        b.mesh.edges.get(ei).into_iter().flatten().for_each(|q| add(p3(*q)));
                    } else if let Some(vi) = b.vertex_refs.iter().position(|x| x == r)
                        && let Some(q) = b.mesh.vertices.get(vi)
                    {
                        add(p3(*q));
                    }
                }
            }
        }
    }
    (lo.x <= hi.x).then_some((lo, hi))
}

/// A drawing sheet's Zoom to fit: the zoom (pixels per mm) and the sheet
/// point at the canvas centre for a canvas of `size` pixels showing the
/// content box `(x0, y0, w, h)` (sheet mm). Live (drawing/156, ISO A3):
/// 40 px clear on each side, the bottom measured as if the canvas ran under
/// the 30 px tab bar.
pub fn drawing_fit(size: [f64; 2], content: (f64, f64, f64, f64)) -> (f64, [f64; 2]) {
    const UNDER: f64 = 30.0;
    let (x0, y0, w, h) = content;
    let zoom = ((size[0] - 80.0) / w).min((size[1] + UNDER - 80.0) / h).clamp(0.05, 200.0);
    (zoom, [x0 + w / 2.0, y0 + h / 2.0 + UNDER / 2.0 / zoom])
}

/// A drawing zoomed by `factor` about the sheet point `at` (it stays under
/// the cursor): the new zoom and centre.
pub fn drawing_zoom_about(zoom: f64, center: [f64; 2], factor: f64, at: [f64; 2]) -> (f64, [f64; 2]) {
    let z = (zoom * factor).clamp(0.05, 200.0);
    let k = zoom / z;
    // The point under the cursor keeps its place: c' = at + (c - at)·zoom/z.
    (z, [at[0] + (center[0] - at[0]) * k, at[1] + (center[1] - at[1]) * k])
}

/// A drawing's Zoom to window: the box from `a` to `b` (canvas pixels of a
/// `size` canvas) fills the drawing area; a click (under 4 px) zooms in 2x
/// there. The new zoom and centre.
pub fn drawing_zoom_window(zoom: f64, center: [f64; 2], size: [f64; 2], a: [f64; 2], b: [f64; 2]) -> (f64, [f64; 2]) {
    let (w, h) = ((b[0] - a[0]).abs(), (b[1] - a[1]).abs());
    let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
    let factor = if w < 4.0 || h < 4.0 { 2.0 } else { (size[0] / w).min(size[1] / h) };
    ((zoom * factor).clamp(0.05, 400.0), drawing_to_sheet(zoom, center, size, mid))
}

/// The sheet point (mm) under canvas pixel `local` of a `size` canvas.
pub fn drawing_to_sheet(zoom: f64, center: [f64; 2], size: [f64; 2], local: [f64; 2]) -> [f64; 2] {
    [center[0] + (local[0] - size[0] / 2.0) / zoom, center[1] - (local[1] - size[1] / 2.0) / zoom]
}

/// An assembly's Zoom to selection: the instances' box `lo..hi` framed as
/// its bounding sphere in a `size` view.
pub fn fit_instances(camera: &Camera, lo: DVec3, hi: DVec3, size: DVec2) -> Camera {
    let aspect = if size.y > 0.0 { size.x / size.y } else { 1.0 };
    let mut to = *camera;
    to.fit_sphere((lo + hi) * 0.5, ((hi - lo).length() * 0.5).max(1e-4), aspect);
    to
}

/// A straight edge's points: a sketch curve's polyline, else a part edge's.
pub fn edge_points(display: &StudioDisplay, r: &Reference) -> Option<Vec<DVec3>> {
    let p3 = |a: [f32; 3]| DVec3::new(a[0] as f64, a[1] as f64, a[2] as f64);
    // Sketch curves: their displayed polyline.
    if let Reference::SketchEntity { sketch, entity } = r {
        let c = display.sketches.iter().find(|s| s.feature == *sketch)?.curves.iter().find(|c| c.entity == *entity)?;
        return Some(c.points.iter().map(|p| p3(*p)).collect());
    }
    display.bodies.iter().find_map(|b| {
        let ei = b.edge_refs.iter().position(|x| x == r)?;
        Some(b.mesh.edges.get(ei)?.iter().map(|p| p3(*p)).collect())
    })
}

/// Direction of a straight edge (None: curved, or not shown).
pub fn linear_edge_direction(display: &StudioDisplay, r: &Reference) -> Option<DVec3> {
    let pts = edge_points(display, r)?;
    let (a, b) = (*pts.first()?, *pts.last()?);
    let d = (b - a).try_normalize()?;
    let straight = pts.iter().all(|p| (*p - a).cross(d).length() <= 1e-6 + 1e-4 * a.distance(b));
    straight.then_some(d)
}

/// What a drawing sheet's Zoom to fit frames (x0, y0, w, h in sheet mm):
/// the paper, or an imported model space's content (it has no paper).
pub fn drawing_content(sheet: &noshape_model::drawing::Sheet) -> (f64, f64, f64, f64) {
    match sheet.imported.as_ref().filter(|m| m.model).and_then(|m| m.extents()) {
        Some(b) => (b[0], b[1], (b[2] - b[0]).max(1.0), (b[3] - b[1]).max(1.0)),
        None => {
            let (w, h) = sheet.size.size_mm();
            (0.0, 0.0, w, h)
        }
    }
}

/// Select other…: the faces the ray from `origin` along `dir` passes
/// through, nearest first (each face once, at its nearest hit).
pub fn faces_under(display: &StudioDisplay, origin: DVec3, dir: DVec3) -> Vec<(f64, Reference)> {
    let p3 = |a: [f32; 3]| DVec3::new(a[0] as f64, a[1] as f64, a[2] as f64);
    // Möller–Trumbore: the distance along the ray to triangle (a, b, c).
    let hit = |a: DVec3, b: DVec3, c: DVec3| -> Option<f64> {
        let (e1, e2) = (b - a, c - a);
        let p = dir.cross(e2);
        let det = e1.dot(p);
        if det.abs() < 1e-18 {
            return None;
        }
        let s = origin - a;
        let u = s.dot(p) / det;
        if !(0.0..=1.0).contains(&u) {
            return None;
        }
        let q = s.cross(e1);
        let v = dir.dot(q) / det;
        if v < 0.0 || u + v > 1.0 {
            return None;
        }
        Some(e2.dot(q) / det)
    };
    let mut out: Vec<(f64, Reference)> = Vec::new();
    for b in display.bodies.iter().filter(|b| !b.hidden) {
        for (fi, &(first, count)) in b.mesh.face_ranges.iter().enumerate() {
            let Some(r) = b.face_refs.get(fi) else { continue };
            let idx = &b.mesh.indices[first as usize..(first + count) as usize];
            let near = idx
                .as_chunks::<3>()
                .0
                .iter()
                .filter_map(|t| {
                    let v = |i: u32| b.mesh.positions.get(i as usize).copied().map(p3);
                    hit(v(t[0])?, v(t[1])?, v(t[2])?)
                })
                .filter(|t| *t > 0.0)
                .fold(f64::INFINITY, f64::min);
            if near.is_finite() {
                out.push((near, r.clone()));
            }
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}
