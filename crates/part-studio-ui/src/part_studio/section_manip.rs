//! The Section view manipulator (live "skb (10-24) assembly section" (1)–(3),
//! the same in Part Studios): a base circle on the plane (dragging it slides
//! the manipulator over the plane), the offset arrow along the normal, and
//! at its tip two rotate rings, each with its axis line through it; the
//! axes lie in the plane, perpendicular to each other. Dragging a ring turns
//! the plane about its axis through the arrow's base, snapping to 5°
//! multiples within 2° (else up to 3 decimals), with a "deg" value box.
//! Dragging the base snaps it to what is under the cursor; on release the
//! plane moves through the snapped point (live "skb (10-24) section base
//! snap").

use super::PartStudioView;
use super::manipulators::Arrow;
use crate::viewport::ViewportEvent;
use glam::{DQuat, DVec2, DVec3};
use gpui_kit::*;
use noshape_model::display::BodyDisplay;
use noshape_model::reference::Reference;
use noshape_render::{Drawable, Instance, LineData, LineSegment};
use noshape_scene::builder::PickKind;

/// Ring geometry (screen px): its radius, and the gap from the arrow's apex
/// to the rings' centre.
const RING_PX: f64 = 9.0;
const RING_GAP_PX: f64 = 14.0;
/// The base circle's radius (px).
const BASE_PX: f64 = 8.0;
/// Arrow glyph length (px), as `manipulators.rs` draws it.
const ARROW_PX: f64 = super::manipulators::ARROW_SHAFT_PX + super::manipulators::ARROW_HEAD_PX;
/// A dragged base snaps onto a vertex within this many px (live ≈22).
const VERTEX_SNAP_PX: f64 = 22.0;
/// Over a face, onto one of its edges within this many px (live ≈30–35).
const EDGE_SNAP_PX: f64 = 32.0;
/// The snapped base's connector disc radius (px; live skb-sv: ≈36 px
/// across).
const SNAP_DISC_PX: f64 = 18.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct SectionManip {
    pub arrow: Arrow,
    /// The rings' rotation axes, in the plane.
    pub axes: [DVec3; 2],
    /// The arrow base where the last arrow drag started, while its value
    /// box is shown (live: a "⊥" there, led to the base).
    pub start: Option<DVec3>,
    /// A base drag snapped here (live skb-sv s009–s141: an implicit mate
    /// connector's disc and triad under the base ring).
    pub snapped: Option<(DVec3, Option<DVec3>)>,
}

/// While the base is dragged over a face (live ui-snapv/26–28): its
/// implicit connector points (corners, edge midpoints) and its centroid
/// with the face's normal.
#[derive(Clone, Debug)]
pub(super) struct FaceMarks {
    pub corners: Vec<DVec3>,
    pub mids: Vec<DVec3>,
    pub centroid: DVec3,
    pub normal: DVec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SectionHandle {
    Base,
    Arrow,
    Ring(usize),
}

/// A ring drag (and the last one, for a typed angle).
#[derive(Clone, Copy, Debug)]
pub(super) struct RingDrag {
    pub ring: usize,
    pub axis: DVec3,
    pub pivot: DVec3,
    /// The plane normal and the cursor's direction from the pivot at the
    /// start.
    pub normal0: DVec3,
    pub from: DVec3,
    /// The angle shown in the value box (deg).
    pub deg: f64,
    pub dragging: bool,
}

/// In-plane rotation axes: the plane's projections of world Y, then Z, then
/// X (live, Top: ring A turns about Y, ring B about X).
pub(super) fn ring_axes(n: DVec3) -> [DVec3; 2] {
    let a = [DVec3::Y, DVec3::Z, DVec3::X]
        .iter()
        .map(|w| *w - n * n.dot(*w))
        .find(|p| p.length() > 0.5)
        .unwrap_or_else(|| n.any_orthonormal_vector())
        .normalize();
    [a, n.cross(a).normalize()]
}

/// A dragged angle (deg) as live shows it: [`crate::viewport::angular_snap_deg`].
pub(super) fn snap_deg(deg: f64, radius_px: f64, shift: bool) -> f64 {
    crate::viewport::angular_snap_deg(deg, radius_px, shift)
}

/// "15 deg", "97.686 deg".
pub(super) fn deg_text(deg: f64) -> String {
    let t = format!("{deg:.3}");
    let t = t.trim_end_matches('0').trim_end_matches('.');
    format!("{t} deg")
}

impl SectionManip {
    fn tip(&self) -> DVec3 {
        self.arrow.base + self.arrow.dir * self.arrow.length
    }

    pub(super) fn ring_center(&self, wpp: f64) -> DVec3 {
        self.tip() + self.arrow.dir * (ARROW_PX + RING_GAP_PX + RING_PX) * wpp
    }

    /// Ring `k`'s points (a circle about its axis) and its axis line.
    fn ring(&self, k: usize, wpp: f64) -> (Vec<DVec3>, [DVec3; 2]) {
        let (c, ax) = (self.ring_center(wpp), self.axes[k]);
        let (u, v) = (self.arrow.dir, ax.cross(self.arrow.dir));
        let r = RING_PX * wpp;
        let pts = (0..=24).map(|i| {
            let t = i as f64 / 24.0 * std::f64::consts::TAU;
            c + (u * t.cos() + v * t.sin()) * r
        });
        (pts.collect(), [c - ax * r * 1.6, c + ax * r * 1.6])
    }

    /// The base circle faces the viewer (`view` its direction; live: round
    /// in any view).
    fn base_circle(&self, wpp: f64, view: DVec3) -> Vec<DVec3> {
        let u = view.any_orthonormal_vector();
        let v = view.cross(u).normalize();
        let r = BASE_PX * wpp;
        (0..=24)
            .map(|i| {
                let t = i as f64 / 24.0 * std::f64::consts::TAU;
                self.arrow.base + (u * t.cos() + v * t.sin()) * r
            })
            .collect()
    }
}

impl PartStudioView {
    /// The rotate glyph's centre in the viewport, where the value box sits
    /// (live asm/2370–2371: 40 px right of it, its bottom level with it).
    pub(super) fn section_box_anchor(&self, cx: &App) -> Option<(DVec3, DVec2)> {
        let s = self.section_manip()?;
        let c = s.ring_center(self.section_wpp(s.tip(), cx)?);
        Some((c, self.viewport.read(cx).project(c)?))
    }

    /// World size of a screen pixel at `p`.
    fn section_wpp(&self, p: DVec3, cx: &App) -> Option<f64> {
        let vp = self.viewport.read(cx);
        let right = vp.camera.right();
        let (p0, p1) = (vp.project(p)?, vp.project(p + right * 1e-3)?);
        Some(1e-3 / (p1 - p0).length().max(1e-9))
    }

    /// The manipulator of the active section plane while the dialog is open.
    pub(super) fn section_manip(&self) -> Option<SectionManip> {
        let arrow = self.section_arrow()?;
        let offset = self.ui.section_dialog.as_ref().and_then(|d| self.ui.section.get(d.active)).map(|p| p.offset);
        let start = match (self.ui.section_box, self.ui.section_rot, self.ui.section_drag0, offset) {
            (true, None, Some(s0), Some(o)) => Some(arrow.base - arrow.dir * (o - s0)),
            _ => None,
        };
        let snapped = self.ui.section_snap.as_ref().map(|s| (s.0, s.2));
        Some(SectionManip { arrow, axes: ring_axes(arrow.dir), start, snapped })
    }

    /// Segments the viewport hit-tests (all handles).
    pub(super) fn section_hit_segments(&self, s: &SectionManip, cx: &App) -> Vec<(DVec3, DVec3, f64)> {
        let tip = s.tip();
        let mut out = vec![(tip, tip + s.arrow.dir * 1e-3, ARROW_PX)];
        let Some(wpp) = self.section_wpp(tip, cx) else { return out };
        for k in 0..2 {
            let (pts, axis) = s.ring(k, wpp);
            out.extend(pts.windows(2).map(|w| (w[0], w[1], 0.0)));
            out.push((axis[0], axis[1], 0.0));
        }
        let base = s.base_circle(self.section_wpp(s.arrow.base, cx).unwrap_or(wpp), self.viewport.read(cx).camera.forward());
        out.extend(base.windows(2).map(|w| (w[0], w[1], 0.0)));
        out
    }

    /// The handle under a viewport position (the nearest within 8 px).
    pub(super) fn section_handle_at(&self, s: &SectionManip, p: DVec2, cx: &App) -> Option<SectionHandle> {
        let vp = self.viewport.read(cx);
        let seg = |a: DVec3, b: DVec3| -> Option<f64> {
            let (a, b) = (vp.project(a)?, vp.project(b)?);
            let d = b - a;
            let u = ((p - a).dot(d) / d.length_squared().max(1e-9)).clamp(0.0, 1.0);
            Some((a + d * u - p).length())
        };
        let near = |pts: &[DVec3]| pts.windows(2).filter_map(|w| seg(w[0], w[1])).fold(f64::INFINITY, f64::min);
        let tip = s.tip();
        let wpp = self.section_wpp(tip, cx)?;
        let mut best: Vec<(SectionHandle, f64)> = vec![(SectionHandle::Arrow, seg(tip, tip + s.arrow.dir * ARROW_PX * wpp).unwrap_or(f64::INFINITY))];
        for k in 0..2 {
            let (pts, axis) = s.ring(k, wpp);
            best.push((SectionHandle::Ring(k), near(&pts).min(seg(axis[0], axis[1]).unwrap_or(f64::INFINITY))));
        }
        let bw = self.section_wpp(s.arrow.base, cx).unwrap_or(wpp);
        best.push((SectionHandle::Base, near(&s.base_circle(bw, self.viewport.read(cx).camera.forward()))));
        best.into_iter().filter(|(_, d)| *d < 8.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(h, _)| h)
    }

    /// Ring and base-circle drags (None: the arrow's, handled as before).
    pub(super) fn section_manip_event(&mut self, s: &SectionManip, ev: &ViewportEvent, window: &mut Window, cx: &mut Context<Self>) -> Option<bool> {
        let on_plane = |this: &Self, p: DVec2, n: DVec3, o: DVec3, cx: &App| -> Option<DVec3> {
            let (ro, rd) = this.viewport.read(cx).ray(p);
            let den = rd.dot(n);
            (den.abs() > 1e-9).then(|| ro + rd * ((o - ro).dot(n) / den))
        };
        match ev {
            ViewportEvent::ToolDown { position, .. } => match self.section_handle_at(s, *position, cx)? {
                SectionHandle::Arrow => {
                    self.ui.section_rot = None;
                    self.ui.section_offset_edit = None;
                    self.ui.section_box = true;
                    self.ui.section_drag0 = self.ui.section_dialog.as_ref().and_then(|d| self.ui.section.get(d.active)).map(|p| p.offset);
                    None
                }
                SectionHandle::Base => {
                    // What is under the cursor keeps being picked: the base
                    // snaps to it.
                    self.ui.section_base_drag = true;
                    self.viewport.update(cx, |v, _| v.tool_hover = true);
                    Some(true)
                }
                SectionHandle::Ring(k) => {
                    let axis = s.axes[k];
                    let pivot = s.arrow.base;
                    let from = on_plane(self, *position, axis, pivot, cx)? - pivot;
                    self.ui.section_offset_edit = None;
                    self.ui.section_box = true;
                    self.ui.section_drag0 = None;
                    self.ui.section_rot = Some(RingDrag { ring: k, axis, pivot, normal0: s.arrow.dir, from, deg: 0.0, dragging: true });
                    cx.notify();
                    Some(true)
                }
            },
            ViewportEvent::ToolDrag { position, .. } => {
                if self.ui.section_base_drag {
                    // The manipulator slides over the plane, snapping; the
                    // plane stays until the release.
                    self.ui.section_snap_pos = Some(*position);
                    let at = self.section_plane_point().unwrap_or(s.arrow.base);
                    if let Some(q) = on_plane(self, *position, s.arrow.dir, at, cx) {
                        self.ui.section_base = Some(q);
                    }
                    self.section_base_snap(cx);
                    return Some(true);
                }
                let r = self.ui.section_rot.filter(|r| r.dragging)?;
                let Some(q) = on_plane(self, *position, r.axis, r.pivot, cx) else { return Some(true) };
                let w = q - r.pivot;
                let a = r.axis.dot(r.from.cross(w)).atan2(r.from.dot(w));
                let wpp = {
                    let v = self.viewport.read(cx);
                    v.camera.world_per_pixel(v.size().y)
                };
                let deg = snap_deg(a.to_degrees(), w.length() / wpp.max(1e-12), window.modifiers().shift);
                self.set_section_rotation(RingDrag { deg, ..r }, window, cx);
                self.open_section_box(window, cx);
                Some(true)
            }
            // Live: during a base drag only the snapped entity lights up.
            ViewportEvent::Hover(id) if self.ui.section_base_drag => {
                self.ui.hover_pick = id.and_then(|id| self.built.as_ref()?.picks.get(id).cloned());
                self.section_base_snap(cx);
                Some(true)
            }
            ViewportEvent::ToolUp { .. } => {
                if std::mem::take(&mut self.ui.section_base_drag) {
                    // The plane now passes through the snapped point; the
                    // base goes back to its place (live).
                    self.viewport.update(cx, |v, _| v.tool_hover = false);
                    let snap = self.ui.section_snap.take();
                    self.ui.section_base = None;
                    self.ui.section_snap_pos = None;
                    self.hover = None;
                    if let Some((q, _, _)) = snap
                        && let Some(i) = self.ui.section_dialog.as_ref().map(|d| d.active)
                        && let Some(pl) = self.ui.section.get_mut(i)
                    {
                        pl.offset = (q - pl.origin).dot(pl.normal);
                    }
                    self.apply_section(window, cx);
                    self.sync_manipulator(cx);
                    self.refresh_scene(false, cx);
                    return Some(true);
                }
                // Live: the box then takes typing, its value selected.
                let Some(r) = self.ui.section_rot.filter(|r| r.dragging) else {
                    if self.manip_drag.is_some() {
                        self.open_section_box(window, cx);
                    }
                    return None;
                };
                self.ui.section_rot = Some(RingDrag { dragging: false, ..r });
                self.open_section_box(window, cx);
                Some(true)
            }
            _ => None,
        }
    }

    /// The hovered face's implicit points while the base is dragged.
    pub(super) fn section_face_marks(&self) -> Option<FaceMarks> {
        if !self.ui.section_base_drag {
            return None;
        }
        let t = self.ui.hover_pick.as_ref().filter(|t| t.kind == PickKind::Face)?;
        let p3 = |a: &[f32; 3]| DVec3::new(a[0] as f64, a[1] as f64, a[2] as f64);
        let (b, fi) = self.display.bodies.iter().filter(|b| !b.hidden).find_map(|b| Some((b, b.face_refs.iter().position(|x| *x == t.reference)?)))?;
        let mut corners: Vec<DVec3> = Vec::new();
        let mut mids = Vec::new();
        for ei in b.mesh.face_edges(fi) {
            let Some(pl) = b.mesh.edges.get(ei) else { continue };
            let pts: Vec<DVec3> = pl.iter().map(p3).collect();
            for q in [pts.first(), pts.last()].into_iter().flatten() {
                if !corners.iter().any(|c| c.distance(*q) < 1e-9) {
                    corners.push(*q);
                }
            }
            // The midpoint along the edge.
            let len: f64 = pts.windows(2).map(|w| w[0].distance(w[1])).sum();
            let mut acc = 0.0;
            for w in pts.windows(2) {
                let l = w[0].distance(w[1]);
                if acc + l >= len * 0.5 && l > 0.0 {
                    mids.push(w[0].lerp(w[1], (len * 0.5 - acc) / l));
                    break;
                }
                acc += l;
            }
        }
        let &(first, count) = b.mesh.face_ranges.get(fi)?;
        let idx = b.mesh.indices.get(first as usize..(first + count) as usize)?;
        let v = |i: u32| b.mesh.positions.get(i as usize).map(p3);
        let (mut sum, mut area, mut normal) = (DVec3::ZERO, 0.0, DVec3::ZERO);
        for tri in idx.as_chunks::<3>().0 {
            let (a, bb, c) = (v(tri[0])?, v(tri[1])?, v(tri[2])?);
            let n = (bb - a).cross(c - a);
            let w = n.length() * 0.5;
            sum += (a + bb + c) / 3.0 * w;
            area += w;
            normal += n;
        }
        (area > 0.0).then(|| FaceMarks { corners, mids, centroid: sum / area, normal: normal.normalize_or(DVec3::Z) })
    }

    /// A point of the active section plane.
    fn section_plane_point(&self) -> Option<DVec3> {
        let pl = self.ui.section.get(self.ui.section_dialog.as_ref()?.active)?;
        Some(pl.origin + pl.normal * pl.offset)
    }

    /// Snap the dragged base at its last position and light what it
    /// snapped to.
    fn section_base_snap(&mut self, cx: &mut Context<Self>) {
        let snap = self.ui.section_snap_pos.and_then(|p| self.section_snap(p, cx));
        let hover = snap.as_ref().map(|s| s.1.clone());
        let changed = self.hover != hover || self.ui.section_snap.as_ref().map(|s| s.0) != snap.as_ref().map(|s| s.0);
        self.ui.section_snap = snap;
        self.hover = hover;
        if changed {
            self.sync_manipulator(cx);
            self.refresh_scene(false, cx);
        }
    }

    /// What a base drag snaps to under `p` (live "skb (10-24) section base
    /// snap"): a vertex within 22 px (of the hovered face or edge, or any
    /// off the part); a hovered edge, or the hovered face's edge within
    /// ~32 px: a circle's centre, else the edge's point nearest the cursor
    /// (a straight edge as a whole line); else the hovered face's point.
    /// Off the part nothing but vertices snaps.
    /// The third value is the snapped entity's implicit connector Z (live
    /// ui-snapv 10-26: a face's normal, a circle's axis, a vertex world Z),
    /// None for an edge hovered on its own (the section plane's normal).
    fn section_snap(&self, p: DVec2, cx: &App) -> Option<(DVec3, Reference, Option<DVec3>)> {
        let vp = self.viewport.read(cx);
        let s = |q: DVec3| vp.project(q);
        let (o, r) = vp.ray(p);
        let p3 = |a: &[f32; 3]| DVec3::new(a[0] as f64, a[1] as f64, a[2] as f64);
        let hover = self.ui.hover_pick.as_ref();
        let bodies = || self.display.bodies.iter().filter(|b| !b.hidden);
        let face = hover.filter(|t| t.kind == PickKind::Face).and_then(|t| bodies().find_map(|b| Some((b, b.face_refs.iter().position(|x| *x == t.reference)?))));
        let edge = hover.filter(|t| t.kind == PickKind::Edge).and_then(|t| bodies().find_map(|b| Some((b, b.edge_refs.iter().position(|x| *x == t.reference)?))));
        let mut verts: Vec<(DVec3, Reference)> = Vec::new();
        let ends = |b: &BodyDisplay, ei: usize, out: &mut Vec<(DVec3, Reference)>| {
            let Some(pl) = b.mesh.edges.get(ei) else { return };
            for e in [pl.first(), pl.last()].into_iter().flatten() {
                if let Some(vi) = b.mesh.vertices.iter().position(|v| p3(v).distance(p3(e)) < 1e-7)
                    && let Some(vr) = b.vertex_refs.get(vi)
                {
                    out.push((p3(&b.mesh.vertices[vi]), vr.clone()));
                }
            }
        };
        match (hover, face, edge) {
            (Some(t), _, _) if t.kind == PickKind::Vertex => verts.extend(t.anchor.map(|a| (a, t.reference.clone()))),
            (_, Some((b, fi)), _) => b.mesh.face_edges(fi).into_iter().for_each(|ei| ends(b, ei, &mut verts)),
            (_, _, Some((b, ei))) => ends(b, ei, &mut verts),
            _ => verts.extend(bodies().flat_map(|b| b.mesh.vertices.iter().map(p3).zip(b.vertex_refs.iter().cloned()))),
        }
        let near_vertex = verts.into_iter().filter_map(|(v, r)| Some(((s(v)? - p).length(), v, r))).filter(|x| x.0 <= VERTEX_SNAP_PX).min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, q, r)) = near_vertex {
            return Some((q, r, Some(DVec3::Z)));
        }
        // A face's outward normal (its first triangle; faces snapped here are
        // planar in practice).
        let face_normal = |b: &BodyDisplay, fi: usize| -> Option<DVec3> {
            let &(first, _) = b.mesh.face_ranges.get(fi)?;
            let t = b.mesh.indices.get(first as usize..first as usize + 3)?;
            let v = |i: u32| b.mesh.positions.get(i as usize).map(p3);
            let (a, bb, c) = (v(t[0])?, v(t[1])?, v(t[2])?);
            (bb - a).cross(c - a).try_normalize()
        };
        let on_edge = |b: &BodyDisplay, ei: usize, face_z: Option<DVec3>| -> Option<(DVec3, Reference, Option<DVec3>)> {
            let pts: Vec<DVec3> = b.mesh.edges.get(ei)?.iter().map(p3).collect();
            let entity = b.edge_refs.get(ei)?.clone();
            if let Some((c, n)) = circle_center(&pts) {
                return Some((c, entity, Some(n)));
            }
            let (a, z) = (*pts.first()?, *pts.last()?);
            let straight = (z - a).try_normalize().filter(|d| pts.iter().all(|q| (*q - a).cross(*d).length() <= 1e-6 + 1e-4 * a.distance(z)));
            let q = match straight {
                Some(d) => a + d * line_param(o, r, a, d)?,
                None => pts.iter().copied().filter_map(|q| Some(((s(q)? - p).length(), q))).min_by(|x, y| x.0.total_cmp(&y.0))?.1,
            };
            Some((q, entity, face_z))
        };
        if let Some((b, ei)) = edge {
            return on_edge(b, ei, None);
        }
        let (b, fi) = face?;
        let reach = |ei: usize| -> Option<f64> {
            let v: Option<Vec<DVec2>> = b.mesh.edges.get(ei)?.iter().map(|q| s(p3(q))).collect();
            v?.windows(2).map(|w| seg_dist(p, w[0], w[1])).min_by(f64::total_cmp)
        };
        let nearest = b.mesh.face_edges(fi).into_iter().filter_map(|ei| Some((reach(ei)?, ei))).filter(|x| x.0 <= EDGE_SNAP_PX).min_by(|x, y| x.0.total_cmp(&y.0));
        if let Some((_, ei)) = nearest {
            return on_edge(b, ei, face_normal(b, fi));
        }
        let &(first, count) = b.mesh.face_ranges.get(fi)?;
        let tris = b.mesh.indices.get(first as usize..(first + count) as usize)?;
        let v = |i: u32| b.mesh.positions.get(i as usize).map(p3);
        let t = tris.as_chunks::<3>().0.iter().filter_map(|t| ray_triangle(o, r, v(t[0])?, v(t[1])?, v(t[2])?)).min_by(f64::total_cmp)?;
        Some((o + r * t, b.face_refs.get(fi)?.clone(), face_normal(b, fi)))
    }

    /// Turn the active plane to `r.deg` about `r.axis` from where the drag
    /// started; a turned plane loses its "(Top)" name (live).
    pub(super) fn set_section_rotation(&mut self, r: RingDrag, window: &mut Window, cx: &mut Context<Self>) {
        let Some(i) = self.ui.section_dialog.as_ref().map(|d| d.active) else { return };
        if let Some(pl) = self.ui.section.get_mut(i) {
            pl.normal = (DQuat::from_axis_angle(r.axis, r.deg.to_radians()) * r.normal0).normalize();
            pl.origin = r.pivot;
            pl.offset = 0.0;
            if r.deg != 0.0 {
                pl.label = format!("Section plane {}", i + 1);
            }
        }
        self.ui.section_rot = Some(r);
        self.apply_section(window, cx);
    }
}

/// Parameter along the line (point, dir) where the ray (o, r) passes closest.
fn line_param(o: DVec3, r: DVec3, point: DVec3, dir: DVec3) -> Option<f64> {
    let w = point - o;
    let (a, b, c) = (dir.dot(dir), dir.dot(r), r.dot(r));
    let (dd, e) = (dir.dot(w), r.dot(w));
    let den = a * c - b * b;
    (den.abs() > 1e-12).then(|| (b * e - c * dd) / den)
}

/// Ray parameter where (o, r) hits the triangle (a, b, c) (Möller–Trumbore).
fn ray_triangle(o: DVec3, r: DVec3, a: DVec3, b: DVec3, c: DVec3) -> Option<f64> {
    let (e1, e2) = (b - a, c - a);
    let p = r.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-18 {
        return None;
    }
    let s = o - a;
    let u = s.dot(p) / det;
    let q = s.cross(e1);
    let v = r.dot(q) / det;
    let t = e2.dot(q) / det;
    (u >= 0.0 && v >= 0.0 && u + v <= 1.0 && t > 0.0).then_some(t)
}

/// Centre and axis of an edge polyline that is a circle or an arc of one.
fn circle_center(pts: &[DVec3]) -> Option<(DVec3, DVec3)> {
    if pts.len() < 4 {
        return None;
    }
    let (a, b, c) = (pts[0], pts[pts.len() / 3], pts[2 * pts.len() / 3]);
    let (ab, ac) = (b - a, c - a);
    let n = ab.cross(ac);
    if n.length_squared() < 1e-24 {
        return None;
    }
    let center = a + (n.cross(ab) * ac.length_squared() + ac.cross(n) * ab.length_squared()) / (2.0 * n.length_squared());
    let r = center.distance(a);
    let n = n.normalize();
    let tol = 1e-3 * r + 1e-7;
    pts.iter().all(|p| (p.distance(center) - r).abs() <= tol && (*p - center).dot(n).abs() <= tol).then_some((center, n))
}

/// Screen distance from `p` to the segment (a, b).
fn seg_dist(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let d = b - a;
    let u = ((p - a).dot(d) / d.length_squared().max(1e-12)).clamp(0.0, 1.0);
    (a + d * u - p).length()
}

/// The base circle and the rings: idle light grey (≈ #e6e8e9 on #c2c7cb),
/// the hovered handle blue (≈ #467bb1), the dragged one deeper (≈ #2861b6).
pub(super) fn section_glyph(s: &SectionManip, hover: Option<SectionHandle>, dragging: Option<SectionHandle>, face: Option<&FaceMarks>, view: &noshape_render::camera_geometry::View, up: &noshape_render::renderer::Uploader) -> Vec<Instance> {
    let f = |v: DVec3| -> [f32; 3] { [v.x as f32, v.y as f32, v.z as f32] };
    let Some(wpp) = view.world_per_px(s.tip()) else { return Vec::new() };
    let bw = view.world_per_px(s.arrow.base).unwrap_or(wpp);
    let mut disc = LineData::default();
    if let Some((o, z)) = s.snapped
        && let Some(w) = view.world_per_px(o)
    {
        // The snapped entity's implicit connector (live ui-snapv: a face's
        // normal up, a vertex world-aligned), X the first world axis off Z
        // (X, then Y, then Z: a +X face gets X = +Y).
        let z = z.unwrap_or(s.arrow.dir);
        let x = [DVec3::X, DVec3::Y, DVec3::Z].into_iter().map(|a| a - z * z.dot(a)).find(|a| a.length() > 0.1).unwrap_or(DVec3::X);
        let f = noshape_model::assembly::solver::Frame::from_axes(o, z, x);
        noshape_scene::asm_scene::push_connector_glyph(&mut disc, &f, SNAP_DISC_PX * w);
    }
    // The hovered face's implicit points (live ui-snapv/26–28): white discs
    // with a grey rim lying in the face, about 11 px across at its corners
    // and 8 px at its edges' midpoints, and at its centroid a quartered
    // black-and-white disc.
    if let Some(m) = face {
        let z = m.normal;
        let x = [DVec3::X, DVec3::Y, DVec3::Z].into_iter().map(|a| a - z * z.dot(a)).find(|a| a.length() > 0.1).unwrap_or(DVec3::X).normalize();
        let y = z.cross(x);
        let mut flat_disc = |c: DVec3, r_px: f64, fill: &dyn Fn(f64) -> [u8; 4], rim: [u8; 4]| {
            let Some(w) = view.world_per_px(c) else { return };
            let (r, lift) = (r_px * w, z * (0.02 * w));
            let at = |rad: f64, a: f64| f(c + (x * a.cos() + y * a.sin()) * rad + lift);
            for k in 0..48 {
                let a = k as f64 / 48.0 * std::f64::consts::TAU;
                disc.push_polyline(&[f(c + lift), at(r, a)], fill(a), 0, 1.5, LineSegment::ON_TOP);
            }
            let ring: Vec<[f32; 3]> = (0..=32).map(|k| at(r, k as f64 / 32.0 * std::f64::consts::TAU)).collect();
            disc.push_polyline(&ring, rim, 0, 1.0, LineSegment::ON_TOP);
        };
        const WHITE: [u8; 4] = [0xfd, 0xfd, 0xfd, 255];
        const GREY: [u8; 4] = [0x9f, 0x9f, 0x9f, 255];
        for q in &m.corners {
            flat_disc(*q, 5.5, &|_| WHITE, GREY);
        }
        for q in &m.mids {
            flat_disc(*q, 4.0, &|_| WHITE, GREY);
        }
        let quarters = |a: f64| if ((a / std::f64::consts::FRAC_PI_2) as usize).is_multiple_of(2) { [0x10, 0x10, 0x10, 255] } else { WHITE };
        flat_disc(m.centroid, 4.5, &quarters, [0x10, 0x10, 0x10, 255]);
    }
    let mut back = LineData::default();
    let mut front = LineData::default();
    let colors = |h: SectionHandle| {
        if dragging == Some(h) {
            (crate::theme::rgb8(0x2861b6), crate::theme::rgb8(0x2861b6))
        } else if hover == Some(h) {
            (crate::theme::rgb8(0x467bb1), crate::theme::rgb8(0x467bb1))
        } else {
            (crate::theme::rgb8(0xc2c7cb), crate::theme::rgb8(0xe6e8e9))
        }
    };
    let mut draw = |pts: &[DVec3], h: SectionHandle| {
        let (edge, core) = colors(h);
        let p: Vec<[f32; 3]> = pts.iter().map(|q| f(*q)).collect();
        back.push_polyline(&p, edge, 0, 3.0, LineSegment::ON_TOP);
        front.push_polyline(&p, core, 0, 1.5, LineSegment::ON_TOP);
    };
    draw(&s.base_circle(bw, view.direction()), SectionHandle::Base);
    for k in 0..2 {
        let (pts, axis) = s.ring(k, wpp);
        draw(&pts, SectionHandle::Ring(k));
        draw(&axis, SectionHandle::Ring(k));
    }
    // The drag start (live asm/2370–2371): a 2 px #2861b6 leader from the
    // base and a 30 px tick across it.
    if let Some(st) = s.start
        && let Some(sw) = view.world_per_px(st)
        && st.distance(s.arrow.base) > 1e-9
    {
        let c = crate::theme::rgb8(0x2861b6);
        let across = (st - s.arrow.base).cross(view.direction()).try_normalize().unwrap_or_else(|| s.axes[0]) * 15.0 * sw;
        front.push_polyline(&[f(s.arrow.base), f(st)], c, 0, 2.0, LineSegment::ON_TOP);
        front.push_polyline(&[f(st - across), f(st + across)], c, 0, 2.0, LineSegment::ON_TOP);
    }
    [(disc, 39), (back, 40), (front, 41)]
        .into_iter()
        .filter(|(l, _)| !l.segments.is_empty())
        .map(|(l, layer)| Instance::new(Drawable::Lines(up.upload_lines(&l))).per_primitive_color().unpickable().ignore_clip().layer(layer))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    // The glob import brings GPUI's `test` attribute; use the built-in one.
    use std::prelude::v1::test;

    #[test]
    fn rotation_snaps_and_reads_as_live() {
        // Live's ring B log: 82.046, 92.111, 97.206 free; 85, 90, 95 snapped.
        // Near the ring (5° steps, ±2°).
        for (raw, shown) in [(82.046, 82.046), (84.0, 85.0), (88.5, 90.0), (92.111, 92.111), (94.2, 95.0), (97.206, 97.206), (15.4, 15.0)] {
            assert!((snap_deg(raw, 100.0, false) - shown).abs() < 1e-9, "{raw} -> {}", snap_deg(raw, 100.0, false));
        }
        // Live's rule: 2.5° stays, 3.1° snaps to 5°; farther out whole
        // degrees (5° is 52 px at 600 px), then tenths; Shift never snaps.
        assert!((snap_deg(2.5, 100.0, false) - 2.5).abs() < 1e-9 && (snap_deg(3.1, 100.0, false) - 5.0).abs() < 1e-9);
        assert!((snap_deg(3.4, 600.0, false) - 3.0).abs() < 1e-9 && (snap_deg(3.6, 600.0, false) - 4.0).abs() < 1e-9);
        assert!((snap_deg(3.44, 4000.0, false) - 3.4).abs() < 1e-9);
        assert!((snap_deg(3.1, 100.0, true) - 3.1).abs() < 1e-9);
        assert_eq!(deg_text(15.0), "15 deg");
        assert_eq!(deg_text(97.686), "97.686 deg");
        // Top: ring A about Y, ring B about X (live).
        let [a, b] = ring_axes(DVec3::Z);
        assert!(a.distance(DVec3::Y) < 1e-12 && b.cross(DVec3::X).length() < 1e-12);
    }
}
