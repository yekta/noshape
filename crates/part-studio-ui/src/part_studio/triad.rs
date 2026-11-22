//! The translate triad of Transform ▸ Translate by XYZ (and Copy here…):
//! live part-studio/102-copy-here.png. Three screen-sized white arrows along
//! the world axes with hollow heads, and three small planar handles between
//! them, at the centre of the entities' bounding box moved by the current
//! translation. Dragging an arrow sets its X / Y / Z translation, dragging a
//! planar handle the two in its plane. Routing curve's Single mode puts it on
//! the active point (live 1722), whose offsets it drags; clicking another
//! point makes that one active. Its Multiple mode (std routingCurve.fs
//! addMultiPointsManipulator / addMultiTriadManipulator, live2 routing-multi): a click on a point toggles it in
//! Selected points; the full triad (rings at the tips, a centre ball) sits at their centre, or at the Edit reference
//! moved by multiDx/Dy/Dz; its arrows and planes move the selected points along its axes, a ring turns only the triad
//! (multiRotationMatrix).

use super::PartStudioView;
use super::manipulators::Manip;
use crate::viewport::ViewportEvent;
use glam::{DVec2, DVec3};
use gpui_kit::component::Sizable as _;
use gpui_kit::*;
use noshape_model::params::ParamValue;
use noshape_model::reference::Reference;
use noshape_render::{Drawable, Instance, LineData, LineSegment};
use noshape_units::{Dim, Quantity};

/// The full triad's screen sizes (px), measured on live2 routing-multi; its planar handles span this part of the shafts.
const SHAFT_PX: f64 = 45.0;
const HEAD_PX: f64 = 20.0;
const PLANE_FROM: f64 = 0.32;
const PLANE_TO: f64 = 0.58;
/// Transform's (live client: MANIPULATOR_TRIAD_LINEAR_SHAFT_LENGTH_PX 60, MANIPULATOR_LINEAR_HEAD_LENGTH_PX 30 and
/// HEAD_RADIUS_PX 10; MANIPULATOR_TRIAD_PLANAR_OFFSET_PX 20 and SIDE_LENGTH_PX 25: live manip-feel-1122 F2's XY square
/// ~45 px right of the centre in iso).
const TRANSFORM_SHAFT_PX: f64 = 60.0;
const TRANSFORM_HEAD_PX: f64 = 30.0;
const TRANSFORM_HEAD_RADIUS_PX: f64 = 10.0;
const TRANSFORM_PLANE_PX: (f64, f64) = (20.0, 45.0);
const PARAMS: [&str; 3] = ["dx", "dy", "dz"];
/// The full triad's rings: their centre past each arrow's tip, their radius and their tick's half length (live2
/// routing-multi h-*.png).
const RING_GAP_PX: f64 = 8.0;
const RING_PX: f64 = 7.0;
const TICK_PX: f64 = 20.0;
/// The full triad's centre ball (radius, px).
const BALL_PX: f64 = 8.0;
/// Handles: 0–2 the arrows, 3–5 the planes, 6–8 the rings (at arrow k's tip), 9 the centre ball.
const RING0: usize = 6;
const BALL: usize = 9;
const WORLD: [DVec3; 3] = [DVec3::X, DVec3::Y, DVec3::Z];

#[derive(Clone, Copy, Debug)]
pub(super) struct Triad {
    /// Where the triad is drawn (the entities' centre moved by the translation).
    pub origin: DVec3,
    /// The current (dx, dy, dz) in meters.
    pub offset: DVec3,
    /// The array item whose dx / dy / dz it sets (Routing curve's point).
    pub item: Option<(&'static str, usize)>,
    /// Routing curve's Multiple mode: the triad moves every selected point.
    pub multi: bool,
    /// Drawn and grabbable (Multiple mode with no point selected has only the points).
    pub shown: bool,
    /// Its X, Y, Z (the world's unless a ring turned it: Routing curve Multiple's multiRotationMatrix).
    pub axes: [DVec3; 3],
    /// The full manipulator: rings at the arrow tips and the centre ball (Routing curve Multiple).
    pub full: bool,
}

impl Triad {
    fn shaft_px(&self) -> f64 {
        if self.full { SHAFT_PX } else { TRANSFORM_SHAFT_PX }
    }

    fn head_px(&self) -> f64 {
        if self.full { HEAD_PX } else { TRANSFORM_HEAD_PX }
    }

    fn head_radius_px(&self) -> f64 {
        if self.full { HEAD_PX * 0.4 } else { TRANSFORM_HEAD_RADIUS_PX }
    }

    /// Where the planar handles start and end along both their axes (px).
    fn plane_px(&self) -> (f64, f64) {
        if self.full { (SHAFT_PX * PLANE_FROM, SHAFT_PX * PLANE_TO) } else { TRANSFORM_PLANE_PX }
    }

    /// A planar handle's corners for axes `i` and `j`.
    fn plane_quad(&self, i: usize, j: usize, wpp: f64) -> [DVec3; 4] {
        let (a, b) = (self.axes[i] * wpp, self.axes[j] * wpp);
        let (p0, p1) = self.plane_px();
        let o = self.origin;
        [o + a * p0 + b * p0, o + a * p1 + b * p0, o + a * p1 + b * p1, o + a * p0 + b * p1]
    }

    /// The field axis `i` drags.
    fn path(&self, i: usize) -> String {
        match self.item {
            Some((arr, k)) => format!("{arr}.{k}.{}", PARAMS[i]),
            None => PARAMS[i].to_string(),
        }
    }
}

/// A handle: 0–2 = the X / Y / Z arrows, 3–5 = the YZ / XZ / XY planes.
#[derive(Clone, Debug)]
pub(super) struct TriadDrag {
    pub handle: usize,
    /// The offset and the grabbed point when the drag started.
    pub start: DVec3,
    pub grab: DVec3,
    pub item: Option<(&'static str, usize)>,
    /// Multiple mode: each selected point's index and its offsets when the drag started.
    pub points: Vec<(usize, DVec3)>,
    /// The triad's origin and axes when the drag started, and multiDx/Dy/Dz then.
    pub origin: DVec3,
    pub axes: [DVec3; 3],
    pub multi_offset: DVec3,
    /// What the drag has done so far: a ring's turn (radians, about its axis), an arrow's move along it (meters).
    pub value: f64,
}

/// Routing curve Multiple's value box after an arrow or ring drag (live2 routing-multi rz-45-*, tx-after: "45 deg",
/// "10.2 mm", its text selected; a typed value and Enter set the drag's total, the box stays): the drag it shows and,
/// once released, its field.
pub(super) struct TriadBox {
    pub drag: TriadDrag,
    /// The Edit reference it was made with: another reference drops it (live2 e8).
    pub reference: Option<Reference>,
    pub input: Option<(Entity<gpui_kit::component::input::InputState>, Subscription)>,
}

/// The axis ring `k` (at arrow k's tip, in the plane of axes k and k + 1) turns the triad about (live2 routing-multi:
/// the ring at Z's tip turns X toward −Z; the assembly triad's `ManipHandle::ring_axis`).
fn ring_axis(k: usize) -> usize {
    (k + 2) % 3
}

/// The two axes of planar handle `h` (3 = YZ, 4 = XZ, 5 = XY).
fn plane_axes(h: usize) -> (usize, usize) {
    [(1, 2), (0, 2), (0, 1)][h - 3]
}

impl PartStudioView {
    /// The box of the parts a dialog's `field` holds (Transform's: std
    /// findCenter takes its centre), before the feature.
    pub(super) fn parts_box(&self, field: &str, cx: &App) -> Option<(DVec3, DVec3)> {
        let d = self.edit.as_ref()?.dialog.read(cx);
        let ctx = self.ui.before_context.as_ref().or(self.context.as_ref())?;
        let mut lo = DVec3::splat(f64::MAX);
        let mut hi = DVec3::splat(f64::MIN);
        for r in d.feature.params.query(field) {
            let id = match r {
                Reference::Body(b) => Some(b.clone()),
                r => self.body_of(r),
            };
            let Some(id) = id else { continue };
            // The box of what the part draws (the display's, before the feature); OCCT's quick box bounds a B-spline
            // face by its control points, which can sit far off the part.
            let shown = self.ui.preview_before.as_deref().unwrap_or(&self.display).body(&id).and_then(|b| b.bounds);
            let quick = || ctx.bodies.iter().find(|b| b.id == id).and_then(|b| b.shape.bounding_box(false).ok()).map(|bb| (bb.min, bb.max));
            if let Some((a, c)) = shown.or_else(quick) {
                lo = lo.min(a);
                hi = hi.max(c);
            }
        }
        (lo.x <= hi.x).then_some((lo, hi))
    }

    pub(super) fn current_triad(&self, cx: &App) -> Option<Triad> {
        let d = self.edit.as_ref()?.dialog.read(cx);
        let p = &d.feature.params;
        if d.feature.type_id != "transform" || !p.is("transformType", "TRANSLATION_3D") {
            return None;
        }
        let (lo, hi) = self.parts_box("entities", cx)?;
        let units = self.doc.read(cx).session.doc.settings.units;
        let value = |id: &str| p.expr(id).and_then(|x| x.cached.map(|q| q.value).or_else(|| noshape_units::evaluate_as(&x.text, Dim::LENGTH, &units, &self.display.variables).ok().map(|q| q.value))).unwrap_or(0.0);
        let offset = DVec3::new(value("dx"), value("dy"), value("dz"));
        // At the parts' bounding box's centre: std's transformCopy.fs findCenter (box3dCenter), live2 manip-feel-1118 m6.
        Some(Triad { origin: (lo + hi) * 0.5 + offset, offset, item: None, multi: false, shown: true, axes: WORLD, full: false })
    }

    /// Routing curve's points (index, position) whose reference ours can
    /// place: the origin, or a vertex (`curveBaseCSys` unset).
    pub(super) fn routing_points(&self, cx: &App) -> Vec<(usize, DVec3)> {
        let Some(e) = self.edit.as_ref() else { return Vec::new() };
        let d = e.dialog.read(cx);
        let p = &d.feature.params;
        if d.feature.type_id != "routingCurve" || !p.query("curveBaseCSys").is_empty() {
            return Vec::new();
        }
        let units = self.doc.read(cx).session.doc.settings.units;
        let num = |it: &noshape_model::params::ParamValues, id: &str| {
            it.expr(id).and_then(|x| x.cached.map(|q| q.value).or_else(|| noshape_units::evaluate_as(&x.text, Dim::LENGTH, &units, &self.display.variables).ok().map(|q| q.value))).unwrap_or(0.0)
        };
        p.array("points")
            .iter()
            .enumerate()
            .filter_map(|(k, it)| {
                let base = match it.enum_("referenceType") {
                    "ORIGIN" | "" => DVec3::ZERO,
                    "VERTEX" => self.vertex_position(it.query("vertex").first()?)?,
                    _ => return None,
                };
                Some((k, base + DVec3::new(num(it, "dx"), num(it, "dy"), num(it, "dz"))))
            })
            .collect()
    }

    /// Routing curve in Multiple mode (std's triad in it moves the selected points).
    fn routing_multi(&self, cx: &App) -> bool {
        self.edit.as_ref().is_some_and(|e| e.dialog.read(cx).feature.params.is("pointsEditType", "MULTI"))
    }

    /// Multiple mode's Selected points (`selectedIndices`' `indexValue`s).
    pub(super) fn routing_selected(&self, cx: &App) -> Vec<usize> {
        let Some(e) = self.edit.as_ref() else { return Vec::new() };
        e.dialog.read(cx).feature.params.array("selectedIndices").iter().filter_map(index_value).collect()
    }

    /// A point's own offsets (its `dx`, `dy`, `dz`).
    fn routing_offset(&self, k: usize, cx: &App) -> Option<DVec3> {
        let e = self.edit.as_ref()?;
        let d = e.dialog.read(cx);
        let it = d.feature.params.array("points").get(k)?;
        let units = self.doc.read(cx).session.doc.settings.units;
        let num = |id: &str| it.expr(id).and_then(|x| x.cached.map(|q| q.value).or_else(|| noshape_units::evaluate_as(&x.text, Dim::LENGTH, &units, &self.display.variables).ok().map(|q| q.value))).unwrap_or(0.0);
        Some(DVec3::new(num("dx"), num("dy"), num("dz")))
    }

    /// Multiple mode: a click on point `k` adds it to Selected points or takes it out.
    fn routing_toggle(&mut self, k: usize, cx: &mut Context<Self>) {
        let Some(e) = &self.edit else { return };
        let mut items = e.dialog.read(cx).feature.params.array("selectedIndices").to_vec();
        match items.iter().position(|it| index_value(it) == Some(k)) {
            Some(i) => {
                items.remove(i);
            }
            None => {
                let mut it = noshape_model::params::ParamValues::default();
                it.set("indexValue", ParamValue::Expr(noshape_units::ExprValue::new(k.to_string())));
                items.push(it);
            }
        }
        e.dialog.update(cx, |dlg, cx| dlg.set_value("selectedIndices", ParamValue::Array(items), cx));
    }

    /// The active point of a routing curve in Single mode (`pointIndex`).
    fn routing_active(&self, cx: &App) -> usize {
        let Some(e) = self.edit.as_ref() else { return 0 };
        e.dialog.read(cx).feature.params.expr("pointIndex").and_then(|x| x.cached.map(|q| q.value).or_else(|| x.text.trim().parse().ok())).map_or(0, |v| v.max(0.0) as usize)
    }

    /// Multiple mode's multiDx / multiDy / multiDz and its turn (multiRotationMatrix's rows: the triad's axes).
    fn routing_multi_frame(&self, cx: &App) -> Option<(DVec3, [DVec3; 3])> {
        let p = &self.edit.as_ref()?.dialog.read(cx).feature.params;
        let units = self.doc.read(cx).session.doc.settings.units;
        let num = |id: &str| p.expr(id).and_then(|x| x.cached.map(|q| q.value).or_else(|| noshape_units::evaluate_as(&x.text, Dim::LENGTH, &units, &self.display.variables).ok().map(|q| q.value))).unwrap_or(0.0);
        let axes = match p.get("multiRotationMatrix") {
            Some(ParamValue::Text(t)) => parse_matrix(t).unwrap_or(WORLD),
            _ => WORLD,
        };
        Some((DVec3::new(num("multiDx"), num("multiDy"), num("multiDz")), axes))
    }

    /// Routing curve: Multiple mode's full triad, else Single mode's on the active point.
    pub(super) fn routing_triad(&self, cx: &App) -> Option<Triad> {
        let pts = self.routing_points(cx);
        if self.routing_multi(cx) {
            if pts.is_empty() {
                return None;
            }
            let sel = self.routing_selected(cx);
            let chosen: Vec<DVec3> = pts.iter().filter(|(i, _)| sel.contains(i)).map(|(_, p)| *p).collect();
            let (moved, axes) = self.routing_multi_frame(cx)?;
            // Std's: at the selected points' centre of mass, or at the Edit reference moved by multiDx/Dy/Dz (live2
            // routing-multi e4: on the vertex itself, world-aligned once the reference is picked).
            let reference = self.edit.as_ref()?.dialog.read(cx).feature.params.query("multiEditReference").first().cloned();
            let origin = match reference {
                Some(r) => self.vertex_position(&r)? + moved,
                None => chosen.iter().copied().sum::<DVec3>() / chosen.len().max(1) as f64,
            };
            return Some(Triad { origin, offset: origin, item: None, multi: true, shown: !chosen.is_empty(), axes, full: true });
        }
        let k = self.routing_active(cx);
        let &(_, at) = pts.iter().find(|(i, _)| *i == k)?;
        let e = self.edit.as_ref()?;
        let it = e.dialog.read(cx).feature.params.array("points").get(k)?.clone();
        let base = match it.enum_("referenceType") {
            "VERTEX" => self.vertex_position(it.query("vertex").first()?)?,
            _ => DVec3::ZERO,
        };
        Some(Triad { origin: at, offset: at - base, item: Some(("points", k)), multi: false, shown: true, axes: WORLD, full: false })
    }

    /// Routing curve: the point (other than Single mode's active one) under the cursor.
    pub(super) fn routing_point_at(&self, p: DVec2, cx: &App) -> Option<usize> {
        let k = if self.routing_multi(cx) { usize::MAX } else { self.routing_active(cx) };
        let vp = self.viewport.read(cx);
        self.routing_points(cx).into_iter().filter(|(i, _)| *i != k).filter_map(|(i, w)| Some((i, (vp.project(w)? - p).length()))).filter(|(_, d)| *d < 8.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(i, _)| i)
    }

    /// World length of a screen pixel at the triad.
    fn triad_wpp(&self, t: &Triad, cx: &App) -> Option<f64> {
        let vp = self.viewport.read(cx);
        let right = vp.camera.right();
        let (p0, p1) = (vp.project(t.origin)?, vp.project(t.origin + right * 1e-3)?);
        Some(1e-3 / (p1 - p0).length().max(1e-9))
    }

    /// The handle under a viewport position.
    pub(super) fn triad_handle_at(&self, t: &Triad, p: DVec2, cx: &App) -> Option<usize> {
        if !t.shown {
            return None;
        }
        let wpp = self.triad_wpp(t, cx)?;
        let vp = self.viewport.read(cx);
        let s = |w: DVec3| vp.project(w);
        let o = t.origin;
        let axis = |i: usize| t.axes[i];
        if t.full {
            // The ball, then the rings (a ring or its tick).
            if s(o).is_some_and(|c| c.distance(p) <= BALL_PX + 2.0) {
                return Some(BALL);
            }
            for k in 0..3 {
                let (c, side) = (o + axis(k) * (SHAFT_PX + HEAD_PX + RING_GAP_PX) * wpp, axis((k + 1) % 3));
                let (Some(sc), Some(a), Some(b)) = (s(c), s(c - side * TICK_PX * wpp), s(c + side * TICK_PX * wpp)) else { continue };
                let u = ((p - a).dot(b - a) / (b - a).length_squared().max(1e-9)).clamp(0.0, 1.0);
                if sc.distance(p) <= RING_PX + 4.0 || (a + (b - a) * u).distance(p) <= 4.0 {
                    return Some(RING0 + k);
                }
            }
        }
        // Planar handles first: they sit between the shafts.
        for h in 3..6 {
            let (i, j) = plane_axes(h);
            let q: Option<Vec<DVec2>> = t.plane_quad(i, j, wpp).iter().map(|w| s(*w)).collect();
            if let Some(q) = q
                && inside(&q, p)
            {
                return Some(h);
            }
        }
        let so = s(o)?;
        (0..3)
            .filter_map(|i| {
                let tip = s(o + axis(i) * (t.shaft_px() + t.head_px()) * wpp)?;
                let d = tip - so;
                let u = ((p - so).dot(d) / d.length_squared().max(1e-9)).clamp(0.0, 1.0);
                let dist = (so + d * u - p).length();
                (dist < 8.0 && u > 0.15).then_some((i, dist))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// Hit segments for the viewport (it starts a tool drag on them).
    pub(super) fn triad_hit_segments(&self, t: &Triad, cx: &App) -> Vec<(DVec3, DVec3, f64)> {
        let Some(wpp) = self.triad_wpp(t, cx).filter(|_| t.shown) else { return Vec::new() };
        let o = t.origin;
        let axis = |i: usize| t.axes[i];
        let mut out: Vec<(DVec3, DVec3, f64)> = (0..3).map(|i| (o + axis(i) * 0.15 * t.shaft_px() * wpp, o + axis(i) * (t.shaft_px() + t.head_px()) * wpp, 0.0)).collect();
        for h in 3..6 {
            let (i, j) = plane_axes(h);
            let q = t.plane_quad(i, j, wpp);
            out.push((q[0], q[2], 0.0));
        }
        if t.full {
            for k in 0..3 {
                let (c, side) = (o + axis(k) * (SHAFT_PX + HEAD_PX + RING_GAP_PX) * wpp, axis((k + 1) % 3));
                out.push((c - side * TICK_PX * wpp, c + side * TICK_PX * wpp, 0.0));
            }
            out.push((o, o + axis(0) * 1e-9, 0.0));
        }
        // The planes' other diagonals: the whole square grabs (live manip-feel-1122 F2).
        for h in 3..6 {
            let (i, j) = plane_axes(h);
            let q = t.plane_quad(i, j, wpp);
            out.push((q[1], q[3], 0.0));
        }
        out
    }

    /// The point a drag of `handle` follows under the cursor: on the arrow's axis, in the planar handle's plane, in the
    /// ring's plane, or (the ball) in the view's plane.
    fn triad_point(&self, origin: DVec3, axes: &[DVec3; 3], handle: usize, p: DVec2, cx: &App) -> Option<DVec3> {
        let (ro, rd) = self.viewport.read(cx).ray(p);
        if handle < 3 {
            let a = axes[handle];
            let w0 = origin - ro;
            let (b, d, e) = (a.dot(rd), a.dot(w0), rd.dot(w0));
            let denom = 1.0 - b * b;
            return (denom.abs() > 1e-9).then(|| origin + a * ((b * e - d) / denom));
        }
        let n = match handle {
            BALL => rd,
            h if h >= RING0 => axes[ring_axis(h - RING0)],
            h => {
                let (i, j) = plane_axes(h);
                axes[i].cross(axes[j])
            }
        };
        let den = rd.dot(n);
        (den.abs() > 1e-9).then(|| ro + rd * ((origin - ro).dot(n) / den))
    }

    /// A ring drag: the triad turned about the ring's axis (5° near a multiple, std's angular snap), the points left
    /// where they are (live2 routing-multi rz-*: only multiRotationMatrix changes).
    fn triad_turn(&mut self, d: &TriadDrag, q: DVec3, position: DVec2, cx: &mut Context<Self>) {
        let n = d.axes[ring_axis(d.handle - RING0)];
        let (v0, v1) = (d.grab - d.origin, q - d.origin);
        let deg = v0.cross(v1).dot(n).atan2(v0.dot(v1)).to_degrees();
        let radius = self.viewport.read(cx).project(d.origin).map_or(0.0, |c| c.distance(position));
        let turn = (super::manipulators::snap_degrees(deg.abs(), radius) * deg.signum()).to_radians();
        if let Some(t) = &mut self.ui.triad_drag {
            t.value = turn;
        }
        self.triad_apply_turn(d, turn, cx);
    }

    /// The triad turned `turn` (radians) from where drag `d` started, about its ring's axis.
    fn triad_apply_turn(&mut self, d: &TriadDrag, turn: f64, cx: &mut Context<Self>) {
        let r = glam::DQuat::from_axis_angle(d.axes[ring_axis(d.handle - RING0)], turn);
        let axes = d.axes.map(|a| r * a);
        if let Some(e) = &self.edit {
            e.dialog.update(cx, |dlg, cx| dlg.set_value("multiRotationMatrix", ParamValue::Text(matrix_text(&axes)), cx));
        }
    }

    /// Multiple mode's arrow, plane or ball drag: the move along the triad's (turned) axes, each on the 0.1 unit step
    /// (live2 routing-multi tx: 10.2 mm along X turned 45° about Y moved the points +7.2125 X, −7.2125 Z); std's
    /// multiTriadManipulatorHandler adds it to every selected point's offsets and keeps the triad's place in
    /// multiDx/Dy/Dz (from the Edit reference, else the world's origin).
    fn triad_move_points(&mut self, d: &TriadDrag, position: DVec2, window: &mut Window, cx: &mut Context<Self>) {
        let Some(q) = self.triad_point(d.grab, &d.axes, d.handle, position, cx) else { return };
        let units = self.doc.read(cx).session.doc.settings.units;
        let unit = units.si_factor(Dim::LENGTH);
        let step = |v: f64| (v / unit * 10.0).round() / 10.0 * unit;
        let raw = q - d.grab;
        let delta = match d.handle {
            h if h < 3 => d.axes[h] * step(raw.dot(d.axes[h])),
            BALL => DVec3::new(step(raw.x), step(raw.y), step(raw.z)),
            h => {
                let (i, j) = plane_axes(h);
                d.axes[i] * step(raw.dot(d.axes[i])) + d.axes[j] * step(raw.dot(d.axes[j]))
            }
        };
        if let Some(t) = self.ui.triad_drag.as_mut().filter(|t| t.handle < 3) {
            t.value = delta.dot(d.axes[t.handle]);
        }
        self.triad_apply_move(d, delta, window, cx);
    }

    /// Multiple's ball: a free move that snaps to the hovered face's or edge's inference point, as a mate connector's
    /// placement (white dots, the chosen one a small triad); snapped, the triad takes that point's frame. The selected
    /// points follow it, translated, not turned; multiDx/Dy/Dz is its place from the Edit reference, else the origin
    /// (live2 routing-multi ball-a: the +X face's centre (80, 0, 5); ball-b: off the reference vertex by (10, 10, 0)).
    fn triad_ball_move(&mut self, d: &TriadDrag, position: DVec2, window: &mut Window, cx: &mut Context<Self>) {
        // The point nearest the drag's pointer (the viewport sends no PointerMove while a tool drags).
        self.ui.pointer = Some(position);
        if self.update_inference(window.modifiers().shift, window, cx) {
            self.refresh_scene(false, cx);
        }
        if !self.triad_ball_snap(d, window, cx) {
            self.set_triad_axes(&d.axes, cx);
            self.triad_move_points(d, position, window, cx);
        }
    }

    /// Snapped: the triad on the chosen point, in its frame; false with no point chosen.
    fn triad_ball_snap(&mut self, d: &TriadDrag, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(frame) = self.inference_choice().map(|p| p.frame) else { return false };
        let (x, z) = (frame.x(), frame.z());
        self.set_triad_axes(&[x, z.cross(x), z], cx);
        self.triad_apply_move(d, frame.origin - d.origin, window, cx);
        true
    }

    /// The ball's snap once the hovered entity's points come back.
    pub(super) fn triad_ball_snap_now(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = self.ui.triad_drag.clone().filter(|d| d.handle == BALL && !d.points.is_empty()) else { return };
        self.triad_ball_snap(&d, window, cx);
    }

    /// Whether Multiple's ball is being dragged (its inference points shown).
    pub(super) fn triad_ball_dragged(&self) -> bool {
        self.ui.triad_drag.as_ref().is_some_and(|d| d.handle == BALL && !d.points.is_empty())
    }

    fn set_triad_axes(&mut self, axes: &[DVec3; 3], cx: &mut Context<Self>) {
        let text = matrix_text(axes);
        let Some(e) = &self.edit else { return };
        if matches!(e.dialog.read(cx).feature.params.get("multiRotationMatrix"), Some(ParamValue::Text(t)) if *t == text) {
            return;
        }
        e.dialog.update(cx, |dlg, cx| dlg.set_value("multiRotationMatrix", ParamValue::Text(text), cx));
    }

    /// The selected points (and multiDx/Dy/Dz) moved `delta` from where drag `d` started.
    fn triad_apply_move(&mut self, d: &TriadDrag, delta: DVec3, window: &mut Window, cx: &mut Context<Self>) {
        let units = self.doc.read(cx).session.doc.settings.units;
        let exact = |v: f64| noshape_units::format_quantity_exact(Quantity::new((v * 1e9).round() / 1e9, Dim::LENGTH), &units);
        let has_reference = self.edit.as_ref().is_some_and(|e| !e.dialog.read(cx).feature.params.query("multiEditReference").is_empty());
        let moved = if has_reference { d.multi_offset + delta } else { d.origin + delta };
        let Some(e) = &self.edit else { return };
        e.dialog.update(cx, |dlg, cx| {
            for (k, start) in &d.points {
                for i in 0..3 {
                    dlg.set_expr_text(&format!("points.{k}.{}", PARAMS[i]), &exact(start[i] + delta[i]), window, cx);
                }
            }
            for (i, id) in ["multiDx", "multiDy", "multiDz"].into_iter().enumerate() {
                dlg.set_expr_text(id, &exact(moved[i]), window, cx);
            }
        });
    }

    /// The value box's text: the drag's or the released box's turn ("45 deg") or move ("10.2 mm").
    fn triad_box_text(&self, cx: &App) -> Option<(usize, String)> {
        let d = self.ui.triad_drag.as_ref().filter(|d| d.handle < 3 || (RING0..BALL).contains(&d.handle)).or(self.ui.triad_box.as_ref().map(|b| &b.drag))?;
        let units = self.doc.read(cx).session.doc.settings.units;
        let text = if d.handle >= RING0 { super::section_manip::deg_text(d.value.to_degrees()) } else { noshape_units::format_quantity(Quantity::new(d.value, Dim::LENGTH), &units) };
        Some((d.handle, text))
    }

    /// Where the box sits: an arrow's beside its tip, a ring's above it (live2 tx-after: 12 px right of the X tip's ring,
    /// level with it; rz-45-held: its corner 26 px left of and 100 px above the ring).
    fn triad_box_corner(&self, handle: usize, cx: &App) -> Option<DVec2> {
        let t = self.current_manip(cx)?.triad().filter(|t| t.multi && t.shown)?;
        let wpp = self.triad_wpp(&t, cx)?;
        let k = if handle >= RING0 { handle - RING0 } else { handle };
        let tip = self.viewport.read(cx).project(t.origin + t.axes[k] * (SHAFT_PX + HEAD_PX + RING_GAP_PX) * wpp)?;
        Some(if handle >= RING0 { tip + DVec2::new(-26.0, -100.0) } else { tip + DVec2::new(12.0, -13.0) })
    }

    /// The box as a field, focused with its text selected.
    pub(super) fn open_triad_box(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_kit::component::input::{InputEvent, InputState};
        let Some((_, text)) = self.triad_box_text(cx) else { return };
        let input = cx.new(|cx| InputState::new(window, cx).default_value(text));
        let sub = cx.subscribe_in(&input, window, |this: &mut Self, st, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                let typed = st.read(cx).value().to_string();
                this.commit_triad_box(&typed, matches!(ev, InputEvent::PressEnter { .. }), window, cx);
            }
        });
        input.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        if let Some(b) = &mut self.ui.triad_box {
            b.input = Some((input, sub));
        }
        cx.notify();
    }

    /// A typed value is the drag's total from where it started (live2 e7: 9.7 mm dragged, "10" + Enter: 10 mm in all; a
    /// bare number in the workspace unit); after Enter the box stays, its new text selected; leaving it closes it.
    fn commit_triad_box(&mut self, typed: &str, enter: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(b) = self.ui.triad_box.take() else { return };
        let d = b.drag;
        let units = self.doc.read(cx).session.doc.settings.units;
        let vars = self.scope_variables(None, cx);
        let dim = if d.handle >= RING0 { Dim::ANGLE } else { Dim::LENGTH };
        let text = noshape_model::quantity_text::commit_text(typed, dim, &units, &vars);
        let value = noshape_units::evaluate_as(&text, dim, &units, &vars).map(|q| q.value).unwrap_or(d.value);
        if d.handle >= RING0 {
            self.triad_apply_turn(&d, value, cx);
        } else {
            self.triad_apply_move(&d, d.axes[d.handle] * value, window, cx);
        }
        if !enter {
            window.focus(&self.focus, cx);
            self.rebuild_scene(cx);
            return;
        }
        self.ui.triad_box = Some(TriadBox { drag: TriadDrag { value, ..d }, reference: b.reference, input: None });
        self.open_triad_box(window, cx);
        self.rebuild_scene(cx);
    }

    /// The value box over the graphics: the drag's value while it lasts, then its field (Esc closes it; the next Esc
    /// cancels the dialog).
    pub(super) fn triad_value_box(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        // Another Edit reference drops it (live2 e8).
        let reference = self.edit.as_ref().and_then(|e| e.dialog.read(cx).feature.params.query("multiEditReference").first().cloned());
        if self.ui.triad_box.as_ref().is_some_and(|b| b.reference != reference) {
            self.ui.triad_box = None;
        }
        let (handle, text) = self.triad_box_text(cx)?;
        let at = self.triad_box_corner(handle, cx)?;
        let frame = div().absolute().left(px(at.x as f32)).top(px(at.y as f32)).w(px(75.)).h(px(26.)).border_1().border_color(gpui_kit::rgb(0x1651b0)).bg(gpui_kit::rgb(0xffffff));
        let input = self.ui.triad_box.as_ref().filter(|_| self.ui.triad_drag.is_none()).and_then(|b| b.input.as_ref()).map(|i| i.0.clone());
        let Some(input) = input else {
            return Some(frame.flex().items_center().justify_end().px(px(3.)).text_size(px(13.33)).text_color(crate::theme::p().text).child(text).into_any_element());
        };
        let field = gpui_kit::component::input::Input::new(&input).xsmall().appearance(false).text_size(px(13.33)).text_align(TextAlign::Right);
        Some(
            frame
                .id("triad-value-field")
                .debug_selector(|| "triad-value-field".into())
                .occlude()
                .px(px(3.))
                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                    if ev.keystroke.key == "escape" {
                        cx.stop_propagation();
                        this.ui.triad_box = None;
                        window.focus(&this.focus, cx);
                        this.rebuild_scene(cx);
                    }
                }))
                .child(gpui_kit::base::input::input_frame(div().size_full().flex().items_center().child(field)))
                .into_any_element(),
        )
    }

    pub(super) fn triad_event(&mut self, t: Triad, ev: &ViewportEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match ev {
            ViewportEvent::ToolDown { position, .. } => {
                // Routing curve: a click on another point makes it the active one (Multiple: toggles it; the triad's
                // handles over a point come first, its ball sitting on a lone selected one).
                let on_handle = t.full && self.triad_handle_at(&t, *position, cx).is_some();
                if let Some(k) = self.routing_point_at(*position, cx).filter(|_| !on_handle) {
                    if t.multi {
                        self.routing_toggle(k, cx);
                    } else if let Some(e) = &self.edit {
                        e.dialog.update(cx, |dlg, cx| dlg.set_value("pointIndex", ParamValue::Expr(noshape_units::ExprValue::new(k.to_string())), cx));
                    }
                    self.rebuild_scene(cx);
                    return true;
                }
                let Some(handle) = self.triad_handle_at(&t, *position, cx) else { return false };
                let Some(grab) = self.triad_point(t.origin, &t.axes, handle, *position, cx) else { return false };
                let points = if t.multi { self.routing_selected(cx).into_iter().filter_map(|k| Some((k, self.routing_offset(k, cx)?))).collect() } else { Vec::new() };
                let multi_offset = if t.multi { self.routing_multi_frame(cx).map_or(DVec3::ZERO, |f| f.0) } else { DVec3::ZERO };
                self.ui.triad_drag = Some(TriadDrag { handle, start: t.offset, grab, item: t.item, points, origin: t.origin, axes: t.axes, multi_offset, value: 0.0 });
                // Multiple's ball snaps to what is under the pointer (live2 routing-multi ball-a): keep picking.
                if t.multi && handle == BALL {
                    self.viewport.update(cx, |v, _| v.tool_hover = true);
                    self.sync_pick_filter(cx);
                }
                // A new drag drops the last one's box.
                self.ui.triad_box = None;
                true
            }
            ViewportEvent::ToolDrag { position, .. } => {
                let Some(d) = self.ui.triad_drag.clone() else { return false };
                if d.handle >= RING0 && d.handle < BALL {
                    let Some(q) = self.triad_point(d.origin, &d.axes, d.handle, *position, cx) else { return true };
                    self.triad_turn(&d, q, *position, cx);
                    return true;
                }
                if t.multi && d.handle == BALL {
                    self.triad_ball_move(&d, *position, window, cx);
                    return true;
                }
                if t.multi {
                    self.triad_move_points(&d, *position, window, cx);
                    return true;
                }
                // The plane / axis through where the drag started.
                let origin = d.grab;
                let Some(q) = self.triad_point(origin, &d.axes, d.handle, *position, cx) else { return true };
                let delta = q - d.grab;
                let axes: Vec<usize> = if d.handle < 3 { vec![d.handle] } else { let (i, j) = plane_axes(d.handle); vec![i, j] };
                let units = self.doc.read(cx).session.doc.settings.units;
                if let Some(e) = &self.edit {
                    e.dialog.update(cx, |dlg, cx| {
                        // Live (dom/transform-xyz-after-x-drag.txt): one decimal
                        // of the workspace unit ("13.9 mm").
                        let unit = units.si_factor(Dim::LENGTH);
                        let text = |v: f64| noshape_units::format_quantity_with(Quantity::new((v / unit * 10.0).round() / 10.0 * unit, Dim::LENGTH), &units, 1);
                        for i in axes {
                            dlg.set_expr_text(&t.path(i), &text(d.start[i] + delta[i]), window, cx);
                        }
                    });
                }
                true
            }
            ViewportEvent::ToolUp { .. } => {
                let Some(d) = self.ui.triad_drag.take() else { return false };
                if d.handle == BALL {
                    self.viewport.update(cx, |v, _| v.tool_hover = false);
                    self.ui.inference = None;
                    self.sync_pick_filter(cx);
                }
                if t.multi || d.handle >= RING0 {
                    // An arrow or a ring that moved keeps its value box, its text selected (live2 rz-45-released).
                    if t.multi && (d.handle < 3 || (RING0..BALL).contains(&d.handle)) && d.value != 0.0 {
                        let reference = self.edit.as_ref().and_then(|e| e.dialog.read(cx).feature.params.query("multiEditReference").first().cloned());
                        self.ui.triad_box = Some(TriadBox { drag: d, reference, input: None });
                        self.open_triad_box(window, cx);
                    }
                    self.rebuild_scene(cx);
                    return true;
                }
                // Transform's triad leaves the focus where it was (live manip-feel-1122 F2, manip2.json: nothing focused
                // after; qa FEEL-TRIAD-RELEASE-FOCUS); a routing curve's point focuses its field.
                let i = if d.handle < 3 { d.handle } else { plane_axes(d.handle).0 };
                let field = Triad { item: d.item, ..t }.path(i);
                if let (Some(e), Some(_)) = (&self.edit, d.item) {
                    e.dialog.update(cx, |dlg, cx| dlg.focus_field_selected(&field, window, cx));
                }
                // The triad turns white again.
                self.rebuild_scene(cx);
                true
            }
            _ => false,
        }
    }
}

/// A Selected points item's point index.
fn index_value(it: &noshape_model::params::ParamValues) -> Option<usize> {
    it.expr("indexValue").and_then(|x| x.cached.map(|q| q.value).or_else(|| x.text.trim().parse().ok())).map(|v: f64| v.max(0.0) as usize)
}

/// multiRotationMatrix's text ("[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]", live's) as the triad's axes,
/// its columns (std draws `transform(matrix(multiRotationMatrix), …)`; live2 routing-multi ball-a: snapped to the +X
/// face, "[[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]", the face's normal its third column).
fn parse_matrix(t: &str) -> Option<[DVec3; 3]> {
    let v: Vec<f64> = t.split(['[', ']', ',']).map(str::trim).filter(|x| !x.is_empty()).map(str::parse).collect::<Result<_, _>>().ok()?;
    (v.len() == 9 && v.iter().all(|x| x.is_finite())).then(|| [DVec3::new(v[0], v[3], v[6]), DVec3::new(v[1], v[4], v[7]), DVec3::new(v[2], v[5], v[8])])
}

fn matrix_text(axes: &[DVec3; 3]) -> String {
    let n = |x: f64| {
        let x = (x * 1e12).round() / 1e12;
        let x = if x == 0.0 { 0.0 } else { x };
        if x.fract() == 0.0 { format!("{x:.1}") } else { format!("{x}") }
    };
    let row = |i: usize| format!("[{}, {}, {}]", n(axes[0][i]), n(axes[1][i]), n(axes[2][i]));
    format!("[{}, {}, {}]", row(0), row(1), row(2))
}

/// Whether `p` is inside the convex quad `q`.
fn inside(q: &[DVec2], p: DVec2) -> bool {
    let n = q.len();
    let mut sign = 0.0;
    for k in 0..n {
        let (a, b) = (q[k], q[(k + 1) % n]);
        let c = (b - a).perp_dot(p - a);
        if c.abs() < 1e-12 {
            continue;
        }
        if sign == 0.0 {
            sign = c.signum();
        } else if c.signum() != sign {
            return false;
        }
    }
    true
}

impl Manip {
    pub(super) fn triad(self) -> Option<Triad> {
        match self {
            Manip::Triad(t) => Some(t),
            _ => None,
        }
    }
}

/// The triad's glyph for a frame's view (built by the renderer, see `manipulators::ManipGeometry`): `hover` the hovered
/// or dragged handle, `turn` a ring drag's (handle, origin and axes at its start, angle so far).
pub(super) fn triad_glyph(t: &Triad, hover: Option<usize>, dragging: bool, turn: Option<(usize, DVec3, [DVec3; 3], f64)>, view: &noshape_render::camera_geometry::View, up: &noshape_render::renderer::Uploader) -> Vec<Instance> {
    let Some(wpp) = view.world_per_px(t.origin).filter(|_| t.shown) else { return Vec::new() };
    let f = |v: DVec3| -> [f32; 3] { [v.x as f32, v.y as f32, v.z as f32] };
    let view_dir = view.direction();
    let o = t.origin;
    let axis = |i: usize| t.axes[i];
    let mut back = LineData::default();
    let mut front = LineData::default();
    // Transform's: like the extrude arrow, white strokes over a light grey outline; a hovered handle blue, outline
    // rgb(63,111,172) and inner rgb(151,177,217) (live part-studio/444). The full triad (live2 routing-multi): light grey
    // hollow strokes, edge ≈ rgb(152,158,164), core rgb(215,217,219), ~4 px; the hovered handle solid rgb(107,144,195)
    // edged rgb(85,128,180).
    let (gray, white, blue, blue_inner, edge_px, core_px) = if t.full {
        (crate::theme::rgb8(0x989ea4), crate::theme::rgb8(0xd7d9db), crate::theme::rgb8(0x5580b4), crate::theme::rgb8(0x6b90c3), 4.0, 2.0)
    } else {
        (crate::theme::rgb8(0xa8adb2), crate::theme::rgb8(0xffffff), crate::theme::rgb8(0x3f6fac), crate::theme::rgb8(0x97b1d9), 3.0, 1.5)
    };
    // While dragging, the other handles go translucent (part-studio/445).
    let fade = |c: [u8; 4], on: bool| if dragging && !on { [c[0], c[1], c[2], 70] } else { c };
    let mut stroke = |pts: &[DVec3], on: bool| {
        let p: Vec<[f32; 3]> = pts.iter().map(|w| f(*w)).collect();
        back.push_polyline(&p, fade(if on { blue } else { gray }, on), 0, edge_px, LineSegment::ON_TOP);
        front.push_polyline(&p, fade(if on { blue_inner } else { white }, on), 0, core_px, LineSegment::ON_TOP);
    };
    let circle = |c: DVec3, u: DVec3, v: DVec3, r: f64| (0..=24).map(|k| {
        let a = k as f64 / 24.0 * std::f64::consts::TAU;
        c + (u * a.cos() + v * a.sin()) * r
    }).collect::<Vec<_>>();
    for i in 0..3 {
        let dir = axis(i);
        let base = o + dir * t.shaft_px() * wpp;
        let apex = base + dir * t.head_px() * wpp;
        let side = dir.cross(view_dir).try_normalize().unwrap_or_else(|| dir.any_orthonormal_vector());
        let (l, r) = (base + side * t.head_radius_px() * wpp, base - side * t.head_radius_px() * wpp);
        let on = hover == Some(i);
        stroke(&[o + dir * if t.full { BALL_PX * wpp } else { 0.0 }, base], on);
        stroke(&[l, apex, r, l], on);
    }
    for h in 3..6 {
        let (i, j) = plane_axes(h);
        let quad = t.plane_quad(i, j, wpp);
        stroke(&[quad[0], quad[1], quad[2], quad[3], quad[0]], hover == Some(h));
    }
    if t.full {
        // Each tip's ring, in the plane of its axis and the next, and its tick across.
        for k in 0..3 {
            let (c, side) = (o + axis(k) * (SHAFT_PX + HEAD_PX + RING_GAP_PX) * wpp, axis((k + 1) % 3));
            let on = hover == Some(RING0 + k);
            stroke(&circle(c, axis(k), side, RING_PX * wpp), on);
            stroke(&[c - side * TICK_PX * wpp, c + side * TICK_PX * wpp], on);
        }
        let (u, v) = (view_dir.any_orthonormal_vector(), view_dir.cross(view_dir.any_orthonormal_vector()));
        stroke(&circle(o, u, v, BALL_PX * wpp), hover == Some(BALL));
    }
    // A ring drag: a blue arc at the ring's distance from where the ring was, a tick across its start (live2
    // routing-multi rz-45-held).
    // An arrow drag: a blue line from where the triad was, a tick across its start (live2 routing-multi tx-after).
    if let Some((h, o0, axes0, moved)) = turn.filter(|x| x.0 < 3 && x.3 != 0.0 && t.full) {
        let (u, side) = (axes0[h], axes0[(h + 1) % 3]);
        front.push_polyline(&[f(o0), f(o0 + u * moved)], blue, 0, 2.0, LineSegment::ON_TOP);
        front.push_polyline(&[f(o0 - side * TICK_PX * wpp), f(o0 + side * TICK_PX * wpp)], blue, 0, 2.0, LineSegment::ON_TOP);
    }
    if let Some((h, o0, axes0, angle)) = turn.filter(|t| t.0 >= RING0 && t.0 < BALL) {
        let k = h - RING0;
        let n = axes0[ring_axis(k)];
        let (u, r) = (axes0[k], (SHAFT_PX + HEAD_PX + RING_GAP_PX) * wpp);
        let v = n.cross(u);
        let steps = ((angle.abs() / std::f64::consts::TAU * 96.0).ceil() as usize).max(1);
        let arc: Vec<[f32; 3]> = (0..=steps).map(|s| {
            let a = angle * s as f64 / steps as f64;
            f(o0 + (u * a.cos() + v * a.sin()) * r)
        }).collect();
        let start = o0 + u * r;
        front.push_polyline(&arc, blue, 0, 2.0, LineSegment::ON_TOP);
        front.push_polyline(&[f(start - u * TICK_PX * wpp), f(start + u * TICK_PX * wpp)], blue, 0, 2.0, LineSegment::ON_TOP);
    }
    vec![
        Instance::new(Drawable::Lines(up.upload_lines(&back))).per_primitive_color().unpickable().ignore_clip().layer(40),
        Instance::new(Drawable::Lines(up.upload_lines(&front))).per_primitive_color().unpickable().ignore_clip().layer(41),
    ]
}
