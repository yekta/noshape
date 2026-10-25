//! The Section view manipulator in the Part Studio (live part-studio
//! qa4-r5-s9-5*: the assembly's glyph, `asm_scene::push_section_glyph`):
//! the base circle at the plane point (a picked face's centre), the arrow
//! along the cut side and two rotate rings at its tip. Only the handle under
//! the cursor turns blue; the cursor stays the default arrow.
//!
//! - The arrow moves the plane along its normal; the value is the distance
//!   from the drag's start.
//! - A ring turns the plane about one of its in-plane axes through the base
//!   (ring 0 about Y, ring 1 about X), snapping to 5° multiples near one
//!   (behaviours "skb (10-24) assembly section").
//! - The base slides over the model, snapping to what is under the cursor
//!   (behaviours "skb (10-24) section base snap"): a vertex within 22 px, a
//!   circle's centre, the nearest point of a face's edge within ~32 px, the
//!   face itself; off the part it follows the cursor in the plane. On
//!   release the plane moves through the snapped point (keeping its
//!   direction) and the base goes back to its place on the plane.

use super::PartStudioView;
use super::section::SectionPlane;
use crate::assembly_scene::{SectionGlyph, SectionHandle, SECTION_BASE_R_PX, SECTION_RINGS_PX, SECTION_TIP_PX};
use crate::viewport::ViewportEvent;
use glam::{DVec2, DVec3};
use gpui_kit::*;
use noshape_model::reference::Reference;
use noshape_render::{Drawable, Instance, LineData};
use noshape_scene::builder::PickKind;

/// Within this many pixels of a vertex the base snaps onto it (live ≈22).
const VERTEX_SNAP_PX: f64 = 22.0;
/// Over a face, within this many pixels of one of its edges the base snaps
/// onto the edge (live ≈30–35).
const EDGE_SNAP_PX: f64 = 32.0;
/// The snapped base's mate connector disc radius in pixels (live skb-sv:
/// a disc about 36–40 px across).
const SNAP_DISC_PX: f64 = 18.0;

/// The hovered handle and the drag in progress (kept in the dialog).
#[derive(Clone, Debug, Default)]
pub struct ManipState {
    pub hover: Option<SectionHandle>,
    pub drag: Option<SectionDrag>,
}

/// A handle drag: the plane at the press, the press's grab (arrow:
/// parameter along it; ring: start vector around its axis), the dragged
/// value (length from the start / angle) and, for the base, where it is
/// now and what it snapped to.
#[derive(Clone, Debug)]
pub struct SectionDrag {
    pub handle: SectionHandle,
    pub plane0: SectionPlane,
    s0: f64,
    v0: DVec3,
    pub value: f64,
    pub moved: bool,
    base: Option<DVec3>,
    snap: Option<Snap>,
}

/// Where a base drag snapped: the point, the entity lit, the plane of the
/// mate connector glyph drawn there.
#[derive(Clone, Debug)]
struct Snap {
    point: DVec3,
    entity: Reference,
}

/// The manipulator as drawn.
#[derive(Clone, Copy, Debug)]
pub(super) struct SectionManip {
    pub base: DVec3,
    pub dir: DVec3,
    pub x: DVec3,
    pub y: DVec3,
    hovered: Option<SectionHandle>,
    dragging: Option<SectionHandle>,
    start: Option<(DVec3, bool)>,
    /// A snapped base: its connector's origin, Z and X.
    snapped: Option<(DVec3, DVec3, DVec3)>,
}

impl SectionPlane {
    /// The manipulator's base (on the plane).
    pub fn base(&self) -> DVec3 {
        self.origin + self.normal * self.offset
    }

    /// The arrow's direction: the side cut away.
    pub fn dir(&self) -> DVec3 {
        if self.flipped { -self.normal } else { self.normal }
    }

    pub fn y(&self) -> DVec3 {
        self.normal.cross(self.x)
    }
}

/// A ring's rotation axis (in the plane: ring 0 about Y, ring 1 about X).
fn ring_axis(pl: &SectionPlane, k: usize) -> DVec3 {
    if k == 0 { pl.y() } else { pl.x }
}

/// The plane after turning `plane0` by `angle` about a ring's axis through
/// its base.
fn rotated_plane(plane0: &SectionPlane, k: usize, angle: f64) -> SectionPlane {
    let q = glam::DQuat::from_axis_angle(ring_axis(plane0, k), angle);
    let mut pl = plane0.clone();
    pl.origin = plane0.base();
    pl.offset = 0.0;
    pl.normal = (q * plane0.normal).normalize();
    pl.x = (q * plane0.x).normalize();
    pl
}

/// Live: a ring drag catches 5° multiples near them (82.046, 92.111 and
/// 97.206° stayed free; the window is taken as 2°).
fn snap_angle(angle: f64) -> f64 {
    let deg = angle.to_degrees();
    let near = (deg / 5.0).round() * 5.0;
    if (deg - near).abs() < 2.0 { near.to_radians() } else { angle }
}

/// Parameter along the line (point, dir) where the ray (o, r) passes closest.
fn line_param(o: DVec3, r: DVec3, point: DVec3, dir: DVec3) -> Option<f64> {
    let w = point - o;
    let (a, b, c) = (dir.dot(dir), dir.dot(r), r.dot(r));
    let (dd, e) = (dir.dot(w), r.dot(w));
    let den = a * c - b * b;
    (den.abs() > 1e-12).then(|| (b * e - c * dd) / den)
}

/// Where the ray (o, r) meets the plane (point, normal).
fn ray_plane(o: DVec3, r: DVec3, point: DVec3, normal: DVec3) -> Option<DVec3> {
    let den = r.dot(normal);
    (den.abs() > 1e-12).then(|| o + r * ((point - o).dot(normal) / den))
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

/// Centre of an edge polyline that is a circle or an arc of one.
fn circle_center(pts: &[DVec3]) -> Option<DVec3> {
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
    pts.iter().all(|p| (p.distance(center) - r).abs() <= tol && (*p - center).dot(n).abs() <= tol).then_some(center)
}

/// Screen distance from `p` to the segment (a, b).
fn seg_dist(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let d = b - a;
    let u = ((p - a).dot(d) / d.length_squared().max(1e-12)).clamp(0.0, 1.0);
    (a + d * u - p).length()
}

impl PartStudioView {
    /// A pixel's world size at a point.
    fn section_wpp(&self, at: DVec3, cx: &App) -> Option<f64> {
        let vp = self.viewport.read(cx);
        let (p0, p1) = (vp.project(at)?, vp.project(at + vp.camera.right() * 1e-3)?);
        Some(1e-3 / (p1 - p0).length().max(1e-9))
    }

    /// Where a picked entity's section plane puts the manipulator: a
    /// face's area centroid (live: "the centre of the picked face"), else
    /// the plane frame's origin.
    pub(super) fn section_origin(&self, r: &Reference, origin: DVec3) -> DVec3 {
        let p3 = |a: [f32; 3]| DVec3::new(a[0] as f64, a[1] as f64, a[2] as f64);
        let centroid = self.display.bodies.iter().find_map(|b| {
            let fi = b.face_refs.iter().position(|x| x == r)?;
            let &(first, count) = b.mesh.face_ranges.get(fi)?;
            let tris = b.mesh.indices.get(first as usize..(first + count) as usize)?;
            let (mut sum, mut area) = (DVec3::ZERO, 0.0);
            for t in tris.as_chunks::<3>().0 {
                let [a, b2, c] = t.map(|i| b.mesh.positions.get(i as usize).copied().map(p3));
                let (Some(a), Some(b2), Some(c)) = (a, b2, c) else { continue };
                let w = (b2 - a).cross(c - a).length() * 0.5;
                sum += (a + b2 + c) / 3.0 * w;
                area += w;
            }
            (area > 0.0).then(|| sum / area)
        });
        centroid.unwrap_or(origin)
    }

    fn section_state(&self) -> Option<(&SectionPlane, &ManipState)> {
        let d = self.ui.section_dialog.as_ref()?;
        Some((self.ui.section.get(d.active)?, &d.manip))
    }

    /// The manipulator while the dialog is open.
    pub(super) fn section_manip(&self) -> Option<SectionManip> {
        let (pl, st) = self.section_state()?;
        let drag = st.drag.as_ref().filter(|d| d.moved);
        let base = drag.and_then(|d| d.base).unwrap_or(pl.base());
        Some(SectionManip {
            base,
            dir: pl.dir(),
            x: pl.x,
            y: pl.y(),
            hovered: st.hover,
            dragging: drag.map(|d| d.handle),
            start: drag.and_then(|d| match d.handle {
                SectionHandle::Arrow => Some((d.plane0.base(), false)),
                SectionHandle::Ring(_) => Some((d.plane0.dir(), true)),
                SectionHandle::Base => None,
            }),
            snapped: drag.and_then(|d| d.snap.as_ref()).map(|s| (s.point, pl.normal, pl.x)),
        })
    }

    /// The arrow's tip (the value box sits beside it).
    pub(super) fn section_tip(&self, cx: &App) -> Option<DVec3> {
        let (pl, _) = self.section_state()?;
        Some(pl.base() + pl.dir() * SECTION_TIP_PX * self.section_wpp(pl.base(), cx)?)
    }

    /// The value box text: a ring drag's angle ("15 deg", up to 3
    /// decimals), else the plane's offset.
    pub(super) fn section_box_text(&self, cx: &App) -> Option<String> {
        let (pl, st) = self.section_state()?;
        let units = self.doc.read(cx).session.doc.settings.units;
        Some(match st.drag.as_ref().filter(|d| d.moved) {
            Some(d) if matches!(d.handle, SectionHandle::Ring(_)) => noshape_units::format_quantity_with(noshape_units::Quantity::radians(d.value), &units, 3).replace('°', " deg"),
            _ => noshape_units::format_quantity(noshape_units::Quantity::new(pl.offset, noshape_units::Dim::LENGTH), &units),
        })
    }

    /// Where a press starts a drag (the viewport routes it to the tool events).
    pub(super) fn section_hit_segments(&self, m: &SectionManip, cx: &App) -> Vec<(DVec3, DVec3, f64)> {
        let Some(w) = self.section_wpp(m.base, cx) else { return Vec::new() };
        let c = m.base + m.dir * SECTION_RINGS_PX * w;
        let mut out = vec![(m.base, m.base + m.dir * 1e-9, 0.0), (m.base + m.dir * SECTION_BASE_R_PX * w, m.base + m.dir * SECTION_TIP_PX * w, 0.0)];
        out.extend([m.y, m.x].map(|a| (c - a * 20.0 * w, c + a * 20.0 * w, 0.0)));
        out
    }

    /// The handle under a viewport position: the nearest stroke within a
    /// few pixels; where both rings cross (the glyph's centre) none (live).
    fn section_handle_at(&self, m: &SectionManip, p: DVec2, cx: &App) -> Option<SectionHandle> {
        let w = self.section_wpp(m.base, cx)?;
        let vp = self.viewport.read(cx);
        let s = |q: DVec3| vp.project(q);
        let poly = |pts: &[DVec3]| -> Option<f64> {
            let v: Option<Vec<DVec2>> = pts.iter().map(|q| s(*q)).collect();
            v?.windows(2).map(|ab| seg_dist(p, ab[0], ab[1])).min_by(f64::total_cmp)
        };
        let ring = |c: DVec3, u: DVec3, v: DVec3, r: f64| -> Vec<DVec3> { (0..=32).map(|i| c + (u * (i as f64 / 32.0 * std::f64::consts::TAU).cos() + v * (i as f64 / 32.0 * std::f64::consts::TAU).sin()) * r).collect() };
        let mut found: Vec<(SectionHandle, f64)> = Vec::new();
        let sb = s(m.base)?;
        found.push((SectionHandle::Base, ((p - sb).length() - SECTION_BASE_R_PX).max(0.0)));
        let (hb, tip) = (m.base + m.dir * 60.0 * w, m.base + m.dir * SECTION_TIP_PX * w);
        if let Some(d) = poly(&[m.base + m.dir * SECTION_BASE_R_PX * w, tip]) {
            // The head is 20 px wide.
            let head = s(hb).zip(s(tip)).map_or(f64::MAX, |(a, b)| seg_dist(p, a, b));
            found.push((SectionHandle::Arrow, if head < 10.0 { d.min(head - 4.0).max(0.0) } else { d }));
        }
        let c = m.base + m.dir * SECTION_RINGS_PX * w;
        for (k, a) in [(0usize, m.y), (1, m.x)] {
            let v = a.cross(m.dir);
            let d = poly(&[c - a * 20.0 * w, c + a * 20.0 * w]).into_iter().chain(poly(&ring(c, m.dir, v, 6.0 * w))).min_by(f64::total_cmp);
            if let Some(d) = d {
                found.push((SectionHandle::Ring(k), d));
            }
        }
        let rings: Vec<f64> = found.iter().filter(|f| matches!(f.0, SectionHandle::Ring(_))).map(|f| f.1).collect();
        if rings.len() == 2 && rings[0] < 2.0 && rings[1] < 2.0 {
            return None;
        }
        found.into_iter().filter(|f| f.1 < 6.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|f| f.0)
    }

    fn section_manip_state(&mut self) -> Option<&mut ManipState> {
        self.ui.section_dialog.as_mut().map(|d| &mut d.manip)
    }

    fn set_active_section_plane(&mut self, pl: SectionPlane) {
        if let Some(i) = self.ui.section_dialog.as_ref().map(|d| d.active)
            && let Some(slot) = self.ui.section.get_mut(i)
        {
            *slot = pl;
        }
    }

    /// Hover, press, drag and release on the manipulator. Returns true if
    /// the event was consumed.
    pub(super) fn section_manip_event(&mut self, m: SectionManip, ev: &ViewportEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let dragging = self.section_state().is_some_and(|(_, st)| st.drag.is_some());
        match ev {
            ViewportEvent::PointerMove { position, .. } if !dragging => {
                let h = self.section_handle_at(&m, *position, cx);
                if let Some(st) = self.section_manip_state()
                    && st.hover != h
                {
                    st.hover = h;
                    self.refresh_scene(false, cx);
                }
                false
            }
            // A base drag looks at what is under the cursor itself.
            ViewportEvent::Hover(id) if dragging => {
                self.ui.hover_pick = id.and_then(|id| self.built.as_ref()?.picks.get(id).cloned());
                true
            }
            ViewportEvent::ToolDown { position, .. } => {
                let Some(handle) = self.section_handle_at(&m, *position, cx) else { return false };
                let Some((pl, _)) = self.section_state() else { return false };
                let pl = pl.clone();
                let (o, r) = self.viewport.read(cx).ray(*position);
                let (mut s0, mut v0) = (0.0, DVec3::ZERO);
                match handle {
                    SectionHandle::Arrow => s0 = line_param(o, r, pl.base(), pl.dir()).unwrap_or(0.0),
                    SectionHandle::Ring(k) => {
                        let c = m.base + m.dir * SECTION_RINGS_PX * self.section_wpp(m.base, cx).unwrap_or(0.0);
                        v0 = ray_plane(o, r, c, ring_axis(&pl, k)).and_then(|q| (q - c).try_normalize()).unwrap_or(pl.dir());
                    }
                    SectionHandle::Base => {
                        // Live: what the base passes over lights up.
                        self.ui.manip_hover = false;
                        self.viewport.update(cx, |v, _| v.tool_hover = true);
                    }
                }
                if let Some(st) = self.section_manip_state() {
                    st.drag = Some(SectionDrag { handle, plane0: pl, s0, v0, value: 0.0, moved: false, base: None, snap: None });
                }
                true
            }
            ViewportEvent::ToolDrag { position, .. } => {
                let Some(d) = self.section_state().and_then(|(_, st)| st.drag.clone()) else { return false };
                let (o, r) = self.viewport.read(cx).ray(*position);
                let pl0 = &d.plane0;
                let mut d2 = d.clone();
                d2.moved = true;
                match d.handle {
                    SectionHandle::Arrow => {
                        let Some(t) = line_param(o, r, pl0.base(), pl0.dir()) else { return true };
                        let v = t - d.s0;
                        let mut pl = pl0.clone();
                        pl.offset = pl0.offset + v * pl0.dir().dot(pl0.normal);
                        d2.value = v;
                        self.set_active_section_plane(pl);
                    }
                    SectionHandle::Ring(k) => {
                        let c = pl0.base() + pl0.dir() * SECTION_RINGS_PX * self.section_wpp(pl0.base(), cx).unwrap_or(0.0);
                        let axis = ring_axis(pl0, k);
                        let Some(v) = ray_plane(o, r, c, axis).and_then(|q| (q - c).try_normalize()) else { return true };
                        let angle = snap_angle(d.v0.cross(v).dot(axis).atan2(d.v0.dot(v)));
                        d2.value = angle;
                        self.set_active_section_plane(rotated_plane(pl0, k, angle));
                    }
                    SectionHandle::Base => {
                        let snap = self.section_snap(*position, cx);
                        d2.base = snap.as_ref().map(|s| s.point).or_else(|| ray_plane(o, r, pl0.base(), pl0.normal));
                        self.hover = snap.as_ref().map(|s| s.entity.clone());
                        d2.snap = snap;
                    }
                }
                if let Some(st) = self.section_manip_state() {
                    st.drag = Some(d2);
                }
                if d.handle == SectionHandle::Base {
                    self.refresh_scene(false, cx);
                } else {
                    self.apply_section(window, cx);
                }
                true
            }
            ViewportEvent::ToolUp { .. } => {
                let Some(d) = self.section_manip_state().and_then(|st| st.drag.take()) else { return false };
                if d.handle == SectionHandle::Base {
                    self.viewport.update(cx, |v, _| v.tool_hover = false);
                    self.hover = None;
                    // The plane now passes through the snapped point.
                    if let Some(s) = &d.snap {
                        let mut pl = d.plane0.clone();
                        pl.offset = (s.point - pl.origin).dot(pl.normal);
                        self.set_active_section_plane(pl);
                    }
                }
                self.apply_section(window, cx);
                true
            }
            _ => false,
        }
    }

    /// What a base drag snaps to under `p` (see the module doc).
    fn section_snap(&self, p: DVec2, cx: &App) -> Option<Snap> {
        let vp = self.viewport.read(cx);
        let s = |q: DVec3| vp.project(q);
        let (o, r) = vp.ray(p);
        let p3 = |a: &[f32; 3]| DVec3::new(a[0] as f64, a[1] as f64, a[2] as f64);
        let hover = self.ui.hover_pick.as_ref();
        let bodies = || self.display.bodies.iter().filter(|b| !b.hidden);
        // The hovered face (body, face index), or the edge.
        let face = hover.filter(|t| t.kind == PickKind::Face).and_then(|t| bodies().find_map(|b| Some((b, b.face_refs.iter().position(|x| *x == t.reference)?))));
        let edge = hover.filter(|t| t.kind == PickKind::Edge).and_then(|t| bodies().find_map(|b| Some((b, b.edge_refs.iter().position(|x| *x == t.reference)?))));
        // Vertices: the hovered one, those of the hovered face or edge, or
        // any when the cursor is off the part.
        let mut verts: Vec<(DVec3, Reference)> = Vec::new();
        let ends = |b: &noshape_model::display::BodyDisplay, ei: usize, out: &mut Vec<(DVec3, Reference)>| {
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
            // Off the part (nothing, a plane, the origin).
            _ => verts.extend(bodies().flat_map(|b| b.mesh.vertices.iter().map(p3).zip(b.vertex_refs.iter().cloned()))),
        }
        let near_vertex = verts.into_iter().filter_map(|(v, r)| Some(((s(v)? - p).length(), v, r))).filter(|x| x.0 <= VERTEX_SNAP_PX).min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, point, entity)) = near_vertex {
            return Some(Snap { point, entity });
        }
        // An edge: the hovered one, or the hovered face's nearest within
        // reach. A circle gives its centre; another edge the point nearest
        // the cursor (a straight one as a whole line).
        let on_edge = |b: &noshape_model::display::BodyDisplay, ei: usize| -> Option<Snap> {
            let pts: Vec<DVec3> = b.mesh.edges.get(ei)?.iter().map(p3).collect();
            let entity = b.edge_refs.get(ei)?.clone();
            if let Some(c) = circle_center(&pts) {
                return Some(Snap { point: c, entity });
            }
            let (a, z) = (*pts.first()?, *pts.last()?);
            let straight = (z - a).try_normalize().filter(|d| pts.iter().all(|q| (*q - a).cross(*d).length() <= 1e-6 + 1e-4 * a.distance(z)));
            let point = match straight {
                Some(d) => a + d * line_param(o, r, a, d)?,
                None => pts.iter().copied().filter_map(|q| Some(((s(q)? - p).length(), q))).min_by(|x, y| x.0.total_cmp(&y.0))?.1,
            };
            Some(Snap { point, entity })
        };
        if let Some((b, ei)) = edge {
            return on_edge(b, ei);
        }
        let (b, fi) = face?;
        let reach = |ei: usize| -> Option<f64> {
            let v: Option<Vec<DVec2>> = b.mesh.edges.get(ei)?.iter().map(|q| s(p3(q))).collect();
            v?.windows(2).map(|w| seg_dist(p, w[0], w[1])).min_by(f64::total_cmp)
        };
        let nearest = b.mesh.face_edges(fi).into_iter().filter_map(|ei| Some((reach(ei)?, ei))).filter(|x| x.0 <= EDGE_SNAP_PX).min_by(|x, y| x.0.total_cmp(&y.0));
        if let Some((_, ei)) = nearest {
            return on_edge(b, ei);
        }
        // The face under the cursor.
        let &(first, count) = b.mesh.face_ranges.get(fi)?;
        let tris = b.mesh.indices.get(first as usize..(first + count) as usize)?;
        let v = |i: u32| b.mesh.positions.get(i as usize).map(p3);
        let t = tris.as_chunks::<3>().0.iter().filter_map(|t| ray_triangle(o, r, v(t[0])?, v(t[1])?, v(t[2])?)).min_by(f64::total_cmp)?;
        Some(Snap { point: o + r * t, entity: b.face_refs.get(fi)?.clone() })
    }
}

/// The manipulator's glyph for a frame's view (`manipulators::ManipGeometry`).
pub(super) fn section_glyph(m: &SectionManip, view: &noshape_render::camera_geometry::View, up: &noshape_render::renderer::Uploader) -> Vec<Instance> {
    let Some(wpp) = view.world_per_px(m.base) else { return Vec::new() };
    let g = SectionGlyph { base: m.base, dir: m.dir, x: m.x, y: m.y, wpp, view: view.direction(), hovered: m.hovered, dragging: m.dragging, start: m.start };
    let mut disc = LineData::default();
    if let Some((o, z, x)) = m.snapped {
        let f = noshape_model::assembly::solver::Frame::from_axes(o, z, x);
        noshape_scene::asm_scene::push_connector_glyph(&mut disc, &f, SNAP_DISC_PX * wpp);
    }
    let mut lines = LineData::default();
    noshape_scene::asm_scene::push_section_glyph(&mut lines, &|_| 0, &g, m.dir);
    [(disc, 40), (lines, 41)]
        .into_iter()
        .filter(|(l, _)| !l.segments.is_empty())
        .map(|(l, layer)| Instance::new(Drawable::Lines(up.upload_lines(&l))).per_primitive_color().unpickable().ignore_clip().layer(layer))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn ring_drags_catch_five_degree_multiples() {
        // Live ring B log: 82.046 and 92.111 free, 85 / 90 / 95 caught.
        let snap = |d: f64| snap_angle(d.to_radians()).to_degrees();
        assert!((snap(84.0) - 85.0).abs() < 1e-9);
        assert!((snap(91.5) - 90.0).abs() < 1e-9);
        assert!((snap(82.046) - 82.046).abs() < 1e-9);
        assert!((snap(97.686) - 97.686).abs() < 1e-9);
        assert!((snap(-14.0) + 15.0).abs() < 1e-9);
    }

    #[::core::prelude::v1::test]
    fn rotating_turns_the_plane_about_its_base() {
        let pl = SectionPlane { label: "Top".into(), origin: DVec3::new(20.0, 10.0, 0.0), normal: DVec3::Z, x: DVec3::X, offset: 10.0, flipped: false };
        // Ring 0 turns about the plane's Y.
        let r = rotated_plane(&pl, 0, 90f64.to_radians());
        assert!(r.normal.distance(DVec3::X) < 1e-12, "{:?}", r.normal);
        assert!(r.base().distance(DVec3::new(20.0, 10.0, 10.0)) < 1e-12);
        // Ring 1 about its X.
        let r = rotated_plane(&pl, 1, 90f64.to_radians());
        assert!(r.normal.distance(DVec3::NEG_Y) < 1e-12, "{:?}", r.normal);
    }

    #[::core::prelude::v1::test]
    fn circle_centres_of_edge_polylines() {
        let c = DVec3::new(20.0, 10.0, 10.0);
        let circle: Vec<DVec3> = (0..=24).map(|i| c + DVec3::new((i as f64 / 24.0 * std::f64::consts::TAU).cos(), (i as f64 / 24.0 * std::f64::consts::TAU).sin(), 0.0) * 2.0).collect();
        assert!(circle_center(&circle).unwrap().distance(c) < 1e-9);
        let arc: Vec<DVec3> = circle[..7].to_vec();
        assert!(circle_center(&arc).unwrap().distance(c) < 1e-9);
        let line: Vec<DVec3> = (0..8).map(|i| DVec3::X * i as f64).collect();
        assert!(circle_center(&line).is_none());
        let wave: Vec<DVec3> = (0..8).map(|i| DVec3::new(i as f64, (i as f64).sin(), 0.0)).collect();
        assert!(circle_center(&wave).is_none());
    }

    #[::core::prelude::v1::test]
    fn rays_meet_lines_planes_and_triangles() {
        let (o, r) = (DVec3::new(5.0, -10.0, 3.0), DVec3::Y);
        // The X axis line: closest at x = 5.
        assert!((line_param(o, r, DVec3::ZERO, DVec3::X).unwrap() - 5.0).abs() < 1e-12);
        let hit = ray_plane(o, r, DVec3::ZERO, DVec3::Y).unwrap();
        assert!(hit.distance(DVec3::new(5.0, 0.0, 3.0)) < 1e-12);
        let t = ray_triangle(o, r, DVec3::new(0.0, 0.0, 0.0), DVec3::new(10.0, 0.0, 0.0), DVec3::new(0.0, 0.0, 10.0)).unwrap();
        assert!((t - 10.0).abs() < 1e-12);
        assert!(ray_triangle(o, -r, DVec3::new(0.0, 0.0, 0.0), DVec3::new(10.0, 0.0, 0.0), DVec3::new(0.0, 0.0, 10.0)).is_none());
    }
}
