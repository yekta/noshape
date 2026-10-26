//! A selected datum plane's resize handles (live behaviours "icons2 plane
//! resize handles"): a ring at each corner, a circle in the plane of
//! constant screen size. Hovering one turns it darker blue with the move
//! cursor and pre-highlights nothing. Dragging it (only once the plane was
//! picked in the graphics) moves that corner with the pointer: the opposite
//! corner stays, the two others slide along their edges, so width and
//! height change apart. A corner can't pass the opposite one. The size is
//! saved outside the feature, one undo step per drag ("Change size :
//! <plane>").

use super::PartStudioView;
use glam::{DVec2, DVec3};
use gpui_kit::*;
use noshape_model::display::{PlaneDisplay, StudioDisplay};
use noshape_model::reference::Reference;
use noshape_render::camera_geometry::{CameraGeometry, View};
use noshape_render::renderer::Uploader;
use noshape_render::{Drawable, Instance, LineData, LineSegment};
use std::sync::Arc;

/// Ring radius at the stroke's centre and its width (live: ≈ 26 px across,
/// 2 px of (142,176,213) with anti-aliasing).
const RING_R_PX: f64 = 12.0;
const RING_W: f32 = 2.0;
const RING: [u8; 4] = [142, 176, 213, 255];
/// Hovered or dragged: live (51,112,179).
const RING_HOVER: [u8; 4] = [51, 112, 179, 255];
/// How far from a corner the pointer takes its ring (px in the plane).
const RING_HIT_PX: f64 = 14.0;

/// The corners in `PlaneDisplay::corner` units, in the builder's order.
pub(super) const CORNERS: [(f64, f64); 4] = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)];

/// A corner being dragged and the plane's rectangle so far.
#[derive(Clone, Debug)]
pub(super) struct PlaneDrag {
    pub plane: Reference,
    pub corner: usize,
    /// Half width, half height and centre (in the plane's frame).
    pub rect: (f64, f64, DVec2),
}

/// The rings of the selected planes, built for each frame's camera.
pub(super) struct PlaneRings {
    /// Corners, plane axes and the darker ring.
    rings: Vec<([DVec3; 4], DVec3, DVec3, Option<usize>)>,
}

impl CameraGeometry for PlaneRings {
    fn build(&self, view: &View, up: &Uploader) -> Vec<Instance> {
        let mut lines = LineData::default();
        for (corners, x, y, dark) in &self.rings {
            for (i, &c) in corners.iter().enumerate() {
                let Some(wpp) = view.world_per_px(c) else { continue };
                let r = RING_R_PX * wpp;
                let pts: Vec<[f32; 3]> = (0..=48)
                    .map(|k| {
                        let a = k as f64 / 48.0 * std::f64::consts::TAU;
                        let p = c + *x * (r * a.cos()) + *y * (r * a.sin());
                        [p.x as f32, p.y as f32, p.z as f32]
                    })
                    .collect();
                lines.push_polyline(&pts, if *dark == Some(i) { RING_HOVER } else { RING }, 0, RING_W, LineSegment::ON_TOP);
            }
        }
        if lines.segments.is_empty() {
            return Vec::new();
        }
        vec![Instance::new(Drawable::Lines(up.upload_lines(&lines))).per_primitive_color().unpickable().ignore_clip().layer(30)]
    }
}

impl PartStudioView {
    /// The selected, shown datum planes (live: their rings show however the
    /// plane was selected), as being dragged.
    fn ring_planes(&self) -> Vec<PlaneDisplay> {
        if self.sketch.is_some() {
            return Vec::new();
        }
        self.selection.iter().filter_map(|r| self.display.planes.iter().find(|p| &p.reference == r && !p.hidden)).map(|p| self.plane_as_dragged(p)).collect()
    }

    /// `p` with the rectangle of a drag in progress.
    fn plane_as_dragged(&self, p: &PlaneDisplay) -> PlaneDisplay {
        let mut p = p.clone();
        if let Some(d) = self.ui.plane_drag.as_ref().filter(|d| d.plane == p.reference) {
            (p.half_width, p.half_height, p.center) = d.rect;
        }
        p
    }

    /// The display with the dragged plane's rectangle (`display_for_scene`).
    pub(super) fn with_plane_drag(&self, d: Arc<StudioDisplay>) -> Arc<StudioDisplay> {
        let Some(drag) = &self.ui.plane_drag else { return d };
        let mut out = (*d).clone();
        for p in out.planes.iter_mut().filter(|p| p.reference == drag.plane) {
            (p.half_width, p.half_height, p.center) = drag.rect;
        }
        Arc::new(out)
    }

    pub(super) fn plane_ring_geometry(&self) -> Option<Arc<PlaneRings>> {
        let rings: Vec<_> = self
            .ring_planes()
            .iter()
            .map(|p| {
                let dark = match &self.ui.plane_drag {
                    Some(d) if d.plane == p.reference => Some(d.corner),
                    _ => self.ui.plane_ring_hover.as_ref().filter(|(r, _)| *r == p.reference).map(|(_, i)| *i),
                };
                (CORNERS.map(|(u, v)| p.corner(u, v)), p.frame.x, p.frame.y, dark)
            })
            .collect();
        (!rings.is_empty()).then(|| Arc::new(PlaneRings { rings }))
    }

    /// The ring under a viewport point: inside the ellipse its circle
    /// makes on screen, a little wider than drawn.
    pub(super) fn plane_ring_at(&self, pos: DVec2, cx: &App) -> Option<(Reference, usize)> {
        let vp = self.viewport.read(cx);
        let right = vp.camera.right();
        for p in self.ring_planes() {
            for (i, &(u, v)) in CORNERS.iter().enumerate() {
                let c = p.corner(u, v);
                let (Some(cs), Some(rs)) = (vp.project(c), vp.project(c + right * 1e-3)) else { continue };
                let r = RING_HIT_PX * 1e-3 / (rs - cs).length().max(1e-9);
                let (Some(xs), Some(ys)) = (vp.project(c + p.frame.x * r), vp.project(c + p.frame.y * r)) else { continue };
                let (ex, ey) = (xs - cs, ys - cs);
                let det = ex.perp_dot(ey);
                if det.abs() < 1e-9 {
                    continue;
                }
                let d = pos - cs;
                let (a, b) = (d.perp_dot(ey) / det, ex.perp_dot(d) / det);
                if a * a + b * b <= 1.0 {
                    return Some((p.reference.clone(), i));
                }
            }
        }
        None
    }

    /// The pointer moved: the ring under it (darker, move cursor, no
    /// pre-highlight, and the press goes to the drag). Returns whether
    /// the hovered ring changed.
    pub(super) fn plane_ring_pointer(&mut self, pos: DVec2, cx: &mut Context<Self>) -> bool {
        // Planes picked in the graphics, while still selected.
        let sel = self.selection.clone();
        self.ui.planes_picked_in_graphics.retain(|r| sel.contains(r));
        if self.ui.plane_drag.is_some() {
            return false;
        }
        let ring = self.plane_ring_at(pos, cx);
        if ring == self.ui.plane_ring_hover {
            return false;
        }
        // The move cursor on any ring; the press goes to the drag only on a
        // plane picked in the graphics.
        let (on, drags) = (ring.is_some(), ring.as_ref().is_some_and(|(r, _)| self.ui.planes_picked_in_graphics.contains(r)));
        self.ui.plane_ring_hover = ring;
        if self.sketch.is_none() {
            self.viewport.update(cx, |vp, _| {
                vp.tool_captures_left = drags;
                vp.tool_cursor = on.then_some(crate::cursor::AppCursor::Move);
            });
        }
        true
    }

    /// A press on a ring of a plane picked in the graphics starts a resize
    /// (live: from a plane selected in the list it starts a box select).
    pub(super) fn plane_drag_start(&mut self, pos: DVec2, cx: &mut Context<Self>) -> bool {
        let Some((r, corner)) = self.plane_ring_at(pos, cx) else { return false };
        if !self.ui.planes_picked_in_graphics.contains(&r) {
            return false;
        }
        let Some(p) = self.display.planes.iter().find(|p| p.reference == r) else { return false };
        self.ui.plane_drag = Some(PlaneDrag { plane: r, corner, rect: (p.half_width, p.half_height, p.center) });
        true
    }

    /// The dragged corner follows the pointer in the plane; the opposite
    /// corner stays.
    pub(super) fn plane_drag_move(&mut self, pos: DVec2, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(mut d) = self.ui.plane_drag.clone() else { return false };
        let Some(p) = self.display.planes.iter().find(|p| p.reference == d.plane) else { return true };
        let (o, dir) = self.viewport.read(cx).ray(pos);
        let n = p.frame.normal;
        let den = dir.dot(n);
        if den.abs() < 1e-12 {
            return true;
        }
        let q = p.frame.to_local(o + dir * ((p.frame.origin - o).dot(n) / den));
        let (u, v) = CORNERS[d.corner];
        let fixed = p.center - DVec2::new(u * p.half_width, v * p.half_height);
        let (w, h) = ((q.x - fixed.x) * u, (q.y - fixed.y) * v);
        // Past the opposite corner: nothing (live).
        if w <= 0.0 || h <= 0.0 {
            return true;
        }
        d.rect = (0.5 * w, 0.5 * h, (fixed + q) * 0.5);
        self.ui.plane_drag = Some(d);
        let shown = self.display.clone();
        self.apply_display(shown, window, cx);
        true
    }

    /// Release: the new size is saved, one undo step ("Change size : Top").
    pub(super) fn plane_drag_end(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(d) = self.ui.plane_drag.take() else { return false };
        let Some(p) = self.display.planes.iter().find(|p| p.reference == d.plane) else { return true };
        if (p.half_width, p.half_height, p.center) != d.rect {
            let (tab, name) = (self.tab, p.name.clone());
            let (hw, hh, c) = d.rect;
            self.doc.update(cx, |doc, cx| {
                doc.edit_studio(tab, &noshape_ops::studio::change_size_description(&name), cx, |ps| noshape_ops::studio::set_plane_rect(ps, &d.plane, hw, hh, c.x, c.y));
            });
        }
        let shown = self.display.clone();
        self.apply_display(shown, window, cx);
        true
    }
}
