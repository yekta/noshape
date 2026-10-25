//! The Section view manipulator (live "skb (10-24) assembly section" (1)–(3),
//! the same in Part Studios): a base circle on the plane (dragging it slides
//! the manipulator over the plane), the offset arrow along the normal, and
//! at its tip two rotate rings, each with its axis line through it; the
//! axes lie in the plane, perpendicular to each other. Dragging a ring turns
//! the plane about its axis through the arrow's base, snapping to 5°
//! multiples within 2° (else up to 3 decimals), with a "deg" value box.

use super::PartStudioView;
use super::manipulators::Arrow;
use crate::viewport::ViewportEvent;
use glam::{DQuat, DVec2, DVec3};
use gpui_kit::*;
use noshape_render::{Drawable, Instance, LineData, LineSegment};

/// Ring geometry (screen px): its radius, and the gap from the arrow's apex
/// to the rings' centre.
const RING_PX: f64 = 9.0;
const RING_GAP_PX: f64 = 14.0;
/// The base circle's radius (px).
const BASE_PX: f64 = 8.0;
/// Arrow glyph length (px), as `manipulators.rs` draws it.
const ARROW_PX: f64 = 47.0;
/// Snap window around 5° multiples (live: 82.046 and 92.111 stay free).
const SNAP_DEG: f64 = 2.0;

#[derive(Clone, Copy, Debug)]
pub(super) struct SectionManip {
    pub arrow: Arrow,
    /// The rings' rotation axes, in the plane.
    pub axes: [DVec3; 2],
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

/// A dragged angle (deg) as live shows it: 5° multiples within 2°.
pub(super) fn snap_deg(deg: f64) -> f64 {
    let s = (deg / 5.0).round() * 5.0;
    if (deg - s).abs() < SNAP_DEG { s } else { (deg * 1000.0).round() / 1000.0 }
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

    fn ring_center(&self, wpp: f64) -> DVec3 {
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

    fn base_circle(&self, wpp: f64) -> Vec<DVec3> {
        let n = self.arrow.dir;
        let (u, v) = (self.axes[0], n.cross(self.axes[0]));
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
        Some(SectionManip { arrow, axes: ring_axes(arrow.dir) })
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
        let base = s.base_circle(self.section_wpp(s.arrow.base, cx).unwrap_or(wpp));
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
        best.push((SectionHandle::Base, near(&s.base_circle(bw))));
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
                    None
                }
                SectionHandle::Base => {
                    self.ui.section_base_drag = true;
                    Some(true)
                }
                SectionHandle::Ring(k) => {
                    let axis = s.axes[k];
                    let pivot = s.arrow.base;
                    let from = on_plane(self, *position, axis, pivot, cx)? - pivot;
                    self.ui.section_rot = Some(RingDrag { ring: k, axis, pivot, normal0: s.arrow.dir, from, deg: 0.0, dragging: true });
                    cx.notify();
                    Some(true)
                }
            },
            ViewportEvent::ToolDrag { position, .. } => {
                if self.ui.section_base_drag {
                    // The manipulator slides over the plane; the plane stays.
                    if let Some(q) = on_plane(self, *position, s.arrow.dir, s.arrow.base, cx) {
                        self.ui.section_base = Some(q);
                        self.sync_manipulator(cx);
                        self.refresh_scene(false, cx);
                    }
                    return Some(true);
                }
                let r = self.ui.section_rot.filter(|r| r.dragging)?;
                let Some(q) = on_plane(self, *position, r.axis, r.pivot, cx) else { return Some(true) };
                let w = q - r.pivot;
                let a = r.axis.dot(r.from.cross(w)).atan2(r.from.dot(w));
                let deg = snap_deg(a.to_degrees());
                self.set_section_rotation(RingDrag { deg, ..r }, window, cx);
                Some(true)
            }
            ViewportEvent::ToolUp { .. } => {
                if std::mem::take(&mut self.ui.section_base_drag) {
                    return Some(true);
                }
                let r = self.ui.section_rot.filter(|r| r.dragging)?;
                self.ui.section_rot = Some(RingDrag { dragging: false, ..r });
                cx.notify();
                Some(true)
            }
            _ => None,
        }
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

/// The base circle and the rings: idle light grey (≈ #e6e8e9 on #c2c7cb),
/// the hovered handle blue (≈ #467bb1), the dragged one deeper (≈ #2861b6).
pub(super) fn section_glyph(s: &SectionManip, hover: Option<SectionHandle>, dragging: Option<SectionHandle>, view: &noshape_render::camera_geometry::View, up: &noshape_render::renderer::Uploader) -> Vec<Instance> {
    let f = |v: DVec3| -> [f32; 3] { [v.x as f32, v.y as f32, v.z as f32] };
    let Some(wpp) = view.world_per_px(s.tip()) else { return Vec::new() };
    let bw = view.world_per_px(s.arrow.base).unwrap_or(wpp);
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
    draw(&s.base_circle(bw), SectionHandle::Base);
    for k in 0..2 {
        let (pts, axis) = s.ring(k, wpp);
        draw(&pts, SectionHandle::Ring(k));
        draw(&axis, SectionHandle::Ring(k));
    }
    vec![
        Instance::new(Drawable::Lines(up.upload_lines(&back))).per_primitive_color().unpickable().ignore_clip().layer(40),
        Instance::new(Drawable::Lines(up.upload_lines(&front))).per_primitive_color().unpickable().ignore_clip().layer(41),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    // The glob import brings GPUI's `test` attribute; use the built-in one.
    use std::prelude::v1::test;

    #[test]
    fn rotation_snaps_and_reads_as_live() {
        // Live's ring B log: 82.046, 92.111, 97.206 free; 85, 90, 95 snapped.
        for (raw, shown) in [(82.046, 82.046), (84.0, 85.0), (88.5, 90.0), (92.111, 92.111), (94.2, 95.0), (97.206, 97.206), (15.4, 15.0)] {
            assert!((snap_deg(raw) - shown).abs() < 1e-9, "{raw} -> {}", snap_deg(raw));
        }
        assert_eq!(deg_text(15.0), "15 deg");
        assert_eq!(deg_text(97.686), "97.686 deg");
        // Top: ring A about Y, ring B about X (live).
        let [a, b] = ring_axes(DVec3::Z);
        assert!(a.distance(DVec3::Y) < 1e-12 && b.cross(DVec3::X).length() < 1e-12);
    }
}
