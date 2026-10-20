//! Onshape's View cube (upper right of the graphics area): it turns with
//! the camera; clicking a face or corner animates to that view;
//! arrows around it rotate 15° (Shift 90°, Ctrl 5°) and roll; the
//! dropdown under it is the *Camera and render options* menu.

use super::PartStudioView;
use crate::scene_builder::{DisplayMode, TangentEdges};
use crate::theme::{p, with_alpha};
use glam::{DQuat, DVec2, DVec3};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::h_flex;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_render::{Camera, Projection, StandardView};

/// Side of the whole widget (cube, arrows and the view menu button): Onshape's
/// `VIEW_CUBE_SCREEN_SIZE_PX`.
const WIDGET_PX: f32 = 150.0;

// Onshape's view cube geometry (its WebGL view manipulator), in pixels from
// the widget's center with y up.
const G: f64 = WIDGET_PX as f64;
/// The arrow tips touch the widget's edge.
const ARROW_OUT: f64 = 0.5 * G;
/// Half of `VIEW_CUBE_ARROW_WIDTH` and `VIEW_CUBE_ARROW_HEIGHT`.
const ARROW_HALF_W: f64 = 0.05 * G;
const ARROW_HALF_H: f64 = 0.065 * G;
/// `VIEW_CUBE_ARROW_ARC_WIDTH`: the roll arcs' thickness.
const ARC_W: f64 = 0.04 * G;
/// Half the cube's edge: what's left inside the arrows, less room for the
/// 12 px axis labels.
const HALF_EDGE: f64 = ((0.5 - 2.0 * 0.05) / 1.6 - 12.0 / G / 1.5) * G;
/// Each face is a rounded square: white on the near side, a light frame
/// seen through on the far side (`VIEW_CUBE_FRONT_SIZE`, `_BACK_SIZE`,
/// `_CORNER_SIZE` of the edge length).
const FRONT_SIZE: f64 = 0.75;
const BACK_SIZE: f64 = 0.97;
const CORNER_SIZE: f64 = 0.2;
/// The axis lines start `VIEW_CUBE_AXIS_GAP` outside the cube's corner.
const AXIS_GAP: f64 = 1.0;
/// Axis parts behind the cube (`VIEW_CUBE_HIDDEN_AXIS_OPACITY`).
const HIDDEN_AXIS_OPACITY: f32 = 0.3;

/// A clickable region of the cube, identified by the view direction it
/// stands for: one non-zero component = face, three = corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CubeCell(pub [i8; 3]);

impl CubeCell {
    fn dir(self) -> DVec3 {
        DVec3::new(self.0[0] as f64, self.0[1] as f64, self.0[2] as f64)
    }

    /// Camera rotation for this cell: a face's standard view, or for a
    /// corner Onshape's trimetric view from that side (30° off the side
    /// face nearest the corner's front, 30° above or below the ground).
    pub fn rotation(self) -> DQuat {
        let view = match self.0 {
            [0, -1, 0] => Some(StandardView::Front),
            [0, 1, 0] => Some(StandardView::Back),
            [-1, 0, 0] => Some(StandardView::Left),
            [1, 0, 0] => Some(StandardView::Right),
            [0, 0, 1] => Some(StandardView::Top),
            [0, 0, -1] => Some(StandardView::Bottom),
            _ => None,
        };
        if let Some(v) = view {
            return Camera::rotation_for(v);
        }
        let quarter = match (self.0[0] > 0, self.0[1] > 0) {
            (true, false) => 0.0,
            (true, true) => 1.0,
            (false, true) => 2.0,
            (false, false) => 3.0,
        };
        let t = (quarter * 90.0 + 30.0f64).to_radians();
        let eye = DVec3::new(0.75f64.sqrt() * t.sin(), -(0.75f64.sqrt()) * t.cos(), 0.5 * self.0[2].signum() as f64);
        Camera::look_rotation(-eye, DVec3::Z)
    }
}

/// Face labels by outward normal (Onshape: Front looks along +Y).
const FACES: [([i8; 3], &str); 6] = [([0, -1, 0], "Front"), ([0, 1, 0], "Back"), ([-1, 0, 0], "Left"), ([1, 0, 0], "Right"), ([0, 0, 1], "Top"), ([0, 0, -1], "Bottom")];

/// A face's in-plane axes: its label runs along the first, upright along
/// the second.
fn face_axes(n: [i8; 3]) -> (DVec3, DVec3) {
    match n {
        [0, -1, 0] => (DVec3::X, DVec3::Z),
        [0, 1, 0] => (DVec3::NEG_X, DVec3::Z),
        [-1, 0, 0] => (DVec3::NEG_Y, DVec3::Z),
        [1, 0, 0] => (DVec3::Y, DVec3::Z),
        [0, 0, 1] => (DVec3::X, DVec3::Y),
        _ => (DVec3::X, DVec3::NEG_Y),
    }
}

/// The rotate and roll arrows around the cube. While hovered, an arrow
/// sits in the owner's hover slot as a pseudo cell ([`Arrow::cell`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Arrow {
    Up,
    Down,
    Left,
    Right,
    Cw,
    Ccw,
}

/// Marks an arrow's pseudo cell (no cube cell has a component this big).
const ARROW_TAG: i8 = 9;

impl Arrow {
    const ALL: [Arrow; 6] = [Arrow::Up, Arrow::Down, Arrow::Left, Arrow::Right, Arrow::Cw, Arrow::Ccw];

    fn cell(self) -> CubeCell {
        CubeCell([ARROW_TAG, self as i8, 0])
    }

    fn of(cell: CubeCell) -> Option<Arrow> {
        if cell.0[0] != ARROW_TAG {
            return None;
        }
        Self::ALL.get(cell.0[1] as usize).copied()
    }

    /// Outline polygons, from the widget center with y up: a triangle on
    /// each side; a roll arrow is an arc band with its head at the outer end.
    fn polys(self) -> Vec<Vec<DVec2>> {
        let (o, w) = (ARROW_OUT, ARROW_HALF_W);
        let b = o - ARROW_HALF_H;
        let tri = |p: [[f64; 2]; 3]| vec![p.iter().map(|q| DVec2::from(*q)).collect::<Vec<_>>()];
        let arc = |a0: f64, a1: f64| {
            let (r0, r1) = (o - w - ARC_W / 2.0, o - w + ARC_W / 2.0);
            let at = |k: usize| a0 + (a1 - a0) * k as f64 / 6.0;
            let mut p: Vec<DVec2> = (0..=6).map(|k| DVec2::from_angle(at(k)) * r1).collect();
            p.extend((0..=6).rev().map(|k| DVec2::from_angle(at(k)) * r0));
            p
        };
        let head = |sx: f64| {
            let d = DVec2::new(sx, 1.0) * std::f64::consts::FRAC_1_SQRT_2;
            let q = std::f64::consts::FRAC_PI_4 - ARROW_HALF_H / o;
            vec![d * o, d * (o - 2.0 * w), DVec2::new(sx * q.cos(), q.sin()) * (o - w)]
        };
        use std::f64::consts::PI;
        match self {
            Arrow::Up => tri([[0.0, o], [-w, b], [w, b]]),
            Arrow::Down => tri([[0.0, -o], [w, -b], [-w, -b]]),
            Arrow::Left => tri([[-o, 0.0], [-b, -w], [-b, w]]),
            Arrow::Right => tri([[o, 0.0], [b, w], [b, -w]]),
            Arrow::Cw => vec![head(1.0), arc(0.25 * PI, 0.4 * PI)],
            Arrow::Ccw => vec![head(-1.0), arc(0.6 * PI, 0.75 * PI)],
        }
    }
}

/// One visible piece of the cube, in pixels from the widget center with y up.
#[derive(Clone, Debug)]
struct CubeShape {
    cell: CubeCell,
    poly: Vec<DVec2>,
    /// A far face's frame (light) rather than a white near face or corner.
    frame: bool,
}

/// A rounded square in the plane through `c` spanned by `u` and `v`:
/// half side `a`, corner radius `r`, 9 segments per corner.
fn rounded_square(c: DVec3, u: DVec3, v: DVec3, a: f64, r: f64) -> Vec<DVec3> {
    let mut out = Vec::with_capacity(40);
    for (k, (su, sv)) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)].into_iter().enumerate() {
        let corner = c + u * (su * (a - r)) + v * (sv * (a - r));
        for j in 0..=9 {
            let t = (k as f64 + j as f64 / 9.0) * std::f64::consts::FRAC_PI_2;
            out.push(corner + (u * t.cos() + v * t.sin()) * r);
        }
    }
    out
}

/// The cube's visible pieces for a camera rotation, back to front, like
/// Onshape's back-face-culled mesh: the far faces' frames seen through
/// the gaps, a white disk under each near corner, the near faces' white
/// squares.
fn cube_shapes(rotation: DQuat) -> Vec<CubeShape> {
    let inv = rotation.inverse();
    let h = HALF_EDGE;
    let (mut far, mut corners, mut near) = (Vec::new(), Vec::new(), Vec::new());
    for (n, _) in FACES {
        let cell = CubeCell(n);
        let z = (inv * cell.dir()).z;
        if z.abs() < 1e-6 {
            continue;
        }
        let (u, v) = face_axes(n);
        let size = if z > 0.0 { FRONT_SIZE } else { BACK_SIZE };
        let poly = rounded_square(cell.dir() * h, u, v, size * h, CORNER_SIZE * 2.0 * h).into_iter().map(|p| (inv * p).truncate()).collect();
        if z > 0.0 {
            near.push(CubeShape { cell, poly, frame: false });
        } else {
            far.push(CubeShape { cell, poly, frame: true });
        }
    }
    for i in 0..8 {
        let s = |bit: i32| if i & bit == 0 { -1i8 } else { 1 };
        let cell = CubeCell([s(1), s(2), s(4)]);
        let d = cell.dir().normalize();
        if (inv * d).z <= 1e-6 {
            continue;
        }
        let center = cell.dir() * (0.8 * h);
        let u = d.cross(DVec3::Z).normalize();
        let v = d.cross(u);
        let poly = (0..36)
            .map(|k| {
                let t = k as f64 / 36.0 * std::f64::consts::TAU;
                (inv * (center + (u * t.cos() + v * t.sin()) * (0.24 * h))).truncate()
            })
            .collect();
        corners.push(CubeShape { cell, poly, frame: false });
    }
    far.into_iter().chain(corners).chain(near).collect()
}

/// Front-most cube cell under a point (pixels from the widget center, y up).
pub fn cube_hit(rotation: DQuat, p: DVec2) -> Option<CubeCell> {
    cube_shapes(rotation).into_iter().rev().find(|s| point_in_poly(p, &s.poly)).map(|s| s.cell)
}

/// What's under a widget point: an arrow's pseudo cell or a cube cell.
fn widget_hit(rotation: DQuat, p: DVec2) -> Option<CubeCell> {
    Arrow::ALL.into_iter().find(|a| a.polys().iter().any(|q| point_in_poly(p, q))).map(Arrow::cell).or_else(|| cube_hit(rotation, p))
}

fn point_in_poly(p: DVec2, poly: &[DVec2]) -> bool {
    let mut inside = false;
    let mut j = poly.len().saturating_sub(1);
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Whether the cube hides a point (pixels, world axes) from a viewer
/// towards `eye`.
fn behind_cube(p: DVec3, eye: DVec3) -> bool {
    let h = HALF_EDGE;
    let (mut t0, mut t1) = (0.0f64, f64::INFINITY);
    for i in 0..3 {
        if eye[i].abs() < 1e-12 {
            if p[i].abs() > h {
                return false;
            }
        } else {
            let (a, b) = ((-h - p[i]) / eye[i], (h - p[i]) / eye[i]);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
    }
    t0 < t1
}

/// See [`CubeHooks::state`].
pub type CubeState<'a> = (&'a mut Option<CubeCell>, &'a mut Option<Point<Pixels>>);

/// What a view owning a [`view_cube`] provides: where it keeps the cube's
/// hover cell and screen origin, its camera rotation, and the camera moves.
pub struct CubeHooks<V: 'static> {
    /// The owner's hover cell and the widget's last painted origin.
    pub state: fn(&mut V) -> CubeState<'_>,
    pub rotation: fn(&V, &App) -> DQuat,
    /// Animate to a face's or corner's view.
    pub click: fn(&mut V, CubeCell, &mut Context<V>),
    /// Rotate by (yaw, pitch) degrees (triangle arrows).
    pub rotate: fn(&mut V, f64, f64, &mut Context<V>),
    /// Roll by degrees about the line of sight (arc arrows).
    pub roll: fn(&mut V, f64, &mut Context<V>),
}

impl<V: 'static> Clone for CubeHooks<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V: 'static> Copy for CubeHooks<V> {}

fn fill_polys(window: &mut Window, polys: &[Vec<DVec2>], at: &impl Fn(DVec2) -> Point<Pixels>, color: Hsla) {
    let mut b = PathBuilder::fill();
    for poly in polys.iter().filter(|p| p.len() >= 3) {
        b.move_to(at(poly[0]));
        for q in &poly[1..] {
            b.line_to(at(*q));
        }
        b.close();
    }
    if let Ok(path) = b.build() {
        window.paint_path(path, color);
    }
}

/// Onshape's 150 px View cube widget with its arrows and `menu` (the
/// camera and render options button) at the bottom right. Shared by the
/// Part Studio and the Assembly: place it 15 px from the graphics area's
/// top-right corner.
pub fn view_cube<V: 'static>(rotation: DQuat, hover: Option<CubeCell>, menu: AnyElement, hooks: CubeHooks<V>, cx: &mut Context<V>) -> AnyElement {
    let inv = rotation.inverse();
    let eye = rotation * DVec3::Z;
    let to_view = move |w: DVec3| (inv * w).truncate();
    // Onshape (tokens): faces and corners white, frames and arrows
    // #cedbe5 at 60%, the hovered piece #90cef1 at 60%, black labels.
    let white: Hsla = gpui_kit::rgb(0xffffff).into();
    let harbor: Hsla = with_alpha(gpui_kit::rgb(0xcedbe5), 0.6).into();
    let highlight: Hsla = with_alpha(gpui_kit::rgb(0x90cef1), 0.6).into();
    let lit = move |cell: CubeCell, base: Hsla| if hover == Some(cell) { highlight } else { base };
    let arrows: Vec<(Vec<Vec<DVec2>>, Hsla)> = Arrow::ALL.into_iter().map(|a| (a.polys(), lit(a.cell(), harbor))).collect();
    let pieces = cube_shapes(rotation);
    // Hidden = behind the cube and under something drawn: the rounded
    // corners leave the axes visible next to them (live).
    let covered = |p: DVec3| behind_cube(p, eye) && pieces.iter().any(|s| point_in_poly(to_view(p), &s.poly));
    let shapes: Vec<(Vec<DVec2>, Hsla)> = pieces.iter().map(|s| (s.poly.clone(), lit(s.cell, if s.frame { harbor } else { white }))).collect();
    // Labels drawn *on* the near faces, like Onshape's label textures:
    // glyph outlines laid out in the face plane and projected.
    let label_polys: Vec<Vec<Vec<DVec2>>> = FACES
        .iter()
        .filter(|(n, _)| (inv * CubeCell(*n).dir()).z > 1e-6)
        .map(|(n, name)| {
            let (right, up) = face_axes(*n);
            let layout = noshape_sketch::text::layout_text(name, noshape_sketch::text::DEFAULT_FONT, false, false);
            // Ascender height in cube units (a face spans -1..1).
            let asc = 0.4;
            let base = CubeCell(*n).dir() - right * (layout.width * asc * 0.5) - up * (asc * 0.36);
            layout.polylines(5).into_iter().map(|c| c.into_iter().map(|q| to_view((base + right * (q.x * asc) + up * (q.y * asc)) * HALF_EDGE)).collect()).collect()
        })
        .collect();
    // The triad runs along the cube's edges from just outside its
    // back-bottom-left corner; what the cube hides shows through faintly.
    let origin = DVec3::splat(-(HALF_EDGE + AXIS_GAP));
    let mut axis_runs: Vec<(DVec2, DVec2, Hsla, bool)> = Vec::new();
    let mut axis_labels: Vec<AnyElement> = Vec::new();
    for (axis, rgb, name) in [(DVec3::X, 0xcd0000u32, "X"), (DVec3::Y, 0x009400, "Y"), (DVec3::Z, 0x0000d0, "Z")] {
        let color: Hsla = gpui_kit::rgb(rgb).into();
        let len = 2.0 * HALF_EDGE;
        let steps = 48;
        let hidden = |k: usize| covered(origin + axis * (len * (k as f64 + 0.5) / steps as f64));
        let mut k0 = 0;
        while k0 < steps {
            let h = hidden(k0);
            let mut k1 = k0 + 1;
            while k1 < steps && hidden(k1) == h {
                k1 += 1;
            }
            let at = |k: usize| to_view(origin + axis * (len * k as f64 / steps as f64));
            axis_runs.push((at(k0), at(k1), if h { Hsla { a: HIDDEN_AXIS_OPACITY, ..color } } else { color }, h));
            k0 = k1;
        }
        // Past the tip by half the label's diagonal plus 3 px; faded out
        // as the axis turns end-on (8° to 2°), faint behind the cube.
        let at = origin + axis * (len + (7.5f64.hypot(12.0) / 2.0 + 3.0));
        let t = (((eye.dot(axis).abs().min(1.0).acos()).to_degrees() - 2.0) / 6.0).clamp(0.0, 1.0);
        let fade = (t * t * (3.0 - 2.0 * t)) as f32;
        let alpha = if covered(at) { fade.min(HIDDEN_AXIS_OPACITY) } else { fade };
        let v = to_view(at);
        axis_labels.push(
            div()
                .absolute()
                .left(px(WIDGET_PX * 0.5 + v.x as f32 - 10.0))
                .top(px(WIDGET_PX * 0.5 - v.y as f32 - 8.0))
                .w(px(20.))
                .h(px(16.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .line_height(px(16.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(Hsla { a: alpha, ..color })
                .child(name)
                .into_any_element(),
        );
    }
    let local = |pos: Point<Pixels>, o: Point<Pixels>| {
        let d = DVec2::new(f64::from(f32::from(pos.x - o.x)), f64::from(f32::from(pos.y - o.y))) - DVec2::splat(G * 0.5);
        DVec2::new(d.x, -d.y)
    };
    let view = cx.entity();
    div()
        .id("view-cube")
        .relative()
        .size(px(WIDGET_PX))
        .when(hover.is_some(), |d| d.cursor_pointer())
        .child(
            canvas(
                move |bounds, _, cx| {
                    // Records the widget origin for hit testing.
                    view.update(cx, |this, _| *(hooks.state)(this).1 = Some(bounds.origin));
                },
                move |bounds, _, window, _| {
                    let o = bounds.origin;
                    let at = |v: DVec2| point(o.x + px((G * 0.5 + v.x) as f32), o.y + px((G * 0.5 - v.y) as f32));
                    for (polys, color) in &arrows {
                        fill_polys(window, polys, &at, *color);
                    }
                    for (poly, color) in &shapes {
                        fill_polys(window, std::slice::from_ref(poly), &at, *color);
                    }
                    for polys in &label_polys {
                        fill_polys(window, polys, &at, with_alpha(gpui_kit::rgb(0x000000), 0.85).into());
                    }
                    // Hidden axis parts show through first, then the visible ones.
                    for hidden_pass in [true, false] {
                        for (a, b, color, hidden) in &axis_runs {
                            if *hidden != hidden_pass {
                                continue;
                            }
                            let mut p = PathBuilder::stroke(px(1.));
                            p.move_to(at(*a));
                            p.line_to(at(*b));
                            if let Ok(path) = p.build() {
                                window.paint_path(path, *color);
                            }
                        }
                    }
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .children(axis_labels)
        .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, _, cx| {
            let rot = (hooks.rotation)(this, cx);
            let (hover, origin) = (hooks.state)(this);
            let hit = origin.and_then(|o| widget_hit(rot, local(e.position, o)));
            if hit != *hover {
                *hover = hit;
                cx.notify();
            }
        }))
        .on_hover(cx.listener(move |this, h: &bool, _, cx| {
            if !*h && (hooks.state)(this).0.take().is_some() {
                cx.notify();
            }
        }))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                let rot = (hooks.rotation)(this, cx);
                let Some(o) = *(hooks.state)(this).1 else { return };
                // Anything but the cube and its arrows is the graphics area's.
                let Some(cell) = widget_hit(rot, local(e.position, o)) else { return };
                cx.stop_propagation();
                let Some(arrow) = Arrow::of(cell) else {
                    (hooks.click)(this, cell, cx);
                    return;
                };
                // Onshape: click 15°, Shift+click 90°, Ctrl+click 5°.
                let k = if e.modifiers.shift {
                    6.0
                } else if e.modifiers.control || e.modifiers.platform {
                    1.0 / 3.0
                } else {
                    1.0
                };
                match arrow {
                    Arrow::Up => (hooks.rotate)(this, 0.0, -15.0 * k, cx),
                    Arrow::Down => (hooks.rotate)(this, 0.0, 15.0 * k, cx),
                    Arrow::Left => (hooks.rotate)(this, -15.0 * k, 0.0, cx),
                    Arrow::Right => (hooks.rotate)(this, 15.0 * k, 0.0, cx),
                    Arrow::Ccw => (hooks.roll)(this, 15.0 * k, cx),
                    Arrow::Cw => (hooks.roll)(this, -15.0 * k, cx),
                }
            }),
        )
        .child(div().absolute().left(px(106.)).top(px(128.)).child(menu))
        .into_any_element()
}

impl PartStudioView {
    /// Animate to a cube cell's view (remembered for Previous view).
    pub(super) fn cube_click(&mut self, cell: CubeCell, cx: &mut Context<Self>) {
        self.push_camera_history(cx);
        let rot = cell.rotation();
        self.viewport.update(cx, |vp, cx| vp.animate_to_rotation_fit(rot, cx));
    }

    /// Roll the view about the line of sight (cube arc arrows).
    pub(super) fn roll_view(&mut self, deg: f64, cx: &mut Context<Self>) {
        let dir = if deg > 0.0 { crate::navigation::RotateDir::CounterClockwise } else { crate::navigation::RotateDir::Clockwise };
        self.viewport.update(cx, |vp, cx| vp.rotate_step(dir, deg.abs(), cx));
    }

    pub(super) fn render_view_cube(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let rotation = self.viewport.read(cx).camera.rotation;
        let menu = self.render_camera_menu(cx);
        let hooks = CubeHooks {
            state: |this: &mut Self| (&mut this.ui.cube_hover, &mut this.ui.cube_origin),
            rotation: |this: &Self, cx| this.viewport.read(cx).camera.rotation,
            click: Self::cube_click,
            rotate: |this, dx, dy, cx| this.rotate_by(&(), dx, dy, cx),
            roll: Self::roll_view,
        };
        view_cube(rotation, self.ui.cube_hover, menu, hooks, cx)
    }

    /// Open Onshape's Graphics preferences dialog.
    pub(super) fn open_graphics_prefs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dialog = cx.new(crate::graphics_prefs_dialog::GraphicsPrefsDialog::new);
        let sub = cx.subscribe(&dialog, |this: &mut Self, _, _: &crate::graphics_prefs_dialog::GraphicsPrefsEvent, cx| {
            this.ui.graphics_dialog = None;
            cx.notify();
        });
        window.focus(&dialog.focus_handle(cx), cx);
        self.ui.graphics_dialog = Some((dialog, sub));
        cx.notify();
    }

    /// Onshape's "Camera and render options" dropdown (under the cube).
    fn render_camera_menu(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity();
        let mode = self.opts.display_mode;
        let perspective = self.viewport.read(cx).camera.projection == Projection::Perspective;
        let has_selection = !self.selection.is_empty() || !self.selected_features.is_empty();
        let section = !self.ui.section.is_empty();
        let has_history = !self.ui.cam_history.is_empty();
        let orient = self.ui.orient_on_edit;
        let hidden_edges = self.opts.hidden_edges_visible;
        let tangent = self.opts.tangent_edges;
        let high_quality = self.ui.high_quality;
        let boundary = self.opts.highlight_boundary;
        // Onshape: the view-mode cube icon with a ▾ under the cube.
        let pal = p();
        let variant = gpui_kit::component::button::ButtonCustomVariant::new(cx).color(with_alpha(pal.panel, 0.35).into()).foreground(pal.text.into()).hover(pal.panel_hover.into()).active(pal.panel_active.into()).shadow(false);
        let button = Button::new("camera-menu")
            .custom(variant)
            .child(h_flex().gap(px(4.)).child(crate::assets::icon("view-cube", 20., cx)).child(crate::ui::glyph("ui-caret-down", 9., pal.text)))
            .w(px(44.))
            .h(px(28.))
            .p_0()
            .rounded(px(3.))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                // Opens under the button, right-aligned with it (live: 222 px,
                // Onshape's context-menu style). The button's bottom-right
                // corner, from the widget's painted origin.
                let at = this.ui.cube_origin.map(|o| point(o.x + px(WIDGET_PX), o.y + px(154.))).unwrap_or_default();
                let view = view.clone();
                let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, window, cx| {
                m = m.camera();
                // Onshape (live, dom/menu-camera-render.txt): views | named and
                // previous views | zoom | projection, sketch orientation |
                // display, hidden edges, tangent edges (each a ✓ submenu
                // showing the current choice) | high quality | section view.
                let item = |label: &str, f: super::ViewAction| {
                    let v = view.clone();
                    crate::os_menu::OsMenuItem::new(label.to_string()).on_click(move |_, w, cx| v.update(cx, |this, cx| f(this, w, cx)))
                };
                for (label, sv) in [("Isometric", StandardView::Isometric), ("Dimetric", StandardView::Dimetric), ("Trimetric", StandardView::Trimetric)] {
                    m = m.item(item(label, Box::new(move |this, _, cx| this.view(sv, cx))));
                }
                m = m.separator();
                m = m.item(item("Graphics preferences…", Box::new(|this, w, cx| this.open_graphics_prefs(w, cx))).icon_name("settings"));
                m = m.separator();
                m = m.item(item("Named views…", Box::new(|this, w, cx| this.toggle_named_views(w, cx))));
                m = m.item(item("Previous view", Box::new(|this, _, cx| this.previous_view(cx))).disabled(!has_history));
                m = m.separator();
                m = m.item(item("Zoom to fit", Box::new(|this, _, cx| this.zoom_fit(cx))));
                m = m.item(item("Zoom to window", Box::new(|this, _, cx| this.toggle_zoom_window(cx))));
                if has_selection {
                    m = m.item(item("Zoom to selection", Box::new(|this, _, cx| this.zoom_to_selection(cx))));
                }
                m = m.separator();
                let v = view.clone();
                m = m.item(crate::os_menu::OsMenuItem::new("Perspective view").checked(perspective).on_click(move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.viewport.update(cx, |vp, cx| {
                            let p = if vp.camera.projection == Projection::Perspective { Projection::Orthographic } else { Projection::Perspective };
                            vp.set_projection(p, cx);
                        })
                    })
                }));
                let v = view.clone();
                m = m.item(crate::os_menu::OsMenuItem::new("Orient normal to sketch on edit").checked(orient).on_click(move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.ui.orient_on_edit = !this.ui.orient_on_edit;
                        cx.notify();
                    })
                }));
                m = m.separator();
                let check = || Some(gpui_kit::component::Icon::new(gpui_kit::component::IconName::Check));
                let v = view.clone();
                m = m.submenu_with_icon(check(), display_label(mode), window, cx, move |mut sm, _, _| {
                    for dm in [DisplayMode::ShadedWithEdges, DisplayMode::Shaded, DisplayMode::HiddenLine, DisplayMode::Translucent] {
                        let v = v.clone();
                        sm = sm.item(crate::os_menu::OsMenuItem::new(display_label(dm)).checked(dm == mode).on_click(move |_, w, cx| v.update(cx, |this, cx| this.set_display_mode(dm, w, cx))));
                    }
                    sm
                });
                let v = view.clone();
                m = m.submenu_with_icon(check(), if hidden_edges { "Hidden edges visible" } else { "Hidden edges removed" }, window, cx, move |mut sm, _, _| {
                    for on in [true, false] {
                        let v = v.clone();
                        sm = sm.item(crate::os_menu::OsMenuItem::new(if on { "Hidden edges visible" } else { "Hidden edges removed" }).checked(on == hidden_edges).on_click(move |_, w, cx| {
                            v.update(cx, |this, cx| {
                                this.opts.hidden_edges_visible = on;
                                let d = this.display.clone();
                                this.apply_display(d, w, cx);
                            })
                        }));
                    }
                    sm
                });
                let v = view.clone();
                m = m.submenu_with_icon(check(), tangent.label(), window, cx, move |mut sm, _, _| {
                    for t in TangentEdges::ALL {
                        let v = v.clone();
                        sm = sm.item(crate::os_menu::OsMenuItem::new(t.label()).checked(t == tangent).on_click(move |_, w, cx| {
                            v.update(cx, |this, cx| {
                                this.opts.tangent_edges = t;
                                let d = this.display.clone();
                                this.apply_display(d, w, cx);
                            })
                        }));
                    }
                    sm
                });
                m = m.separator();
                m = m.item(crate::os_menu::OsMenuItem::new("View in high quality").checked(high_quality).on_click({
                    let v = view.clone();
                    move |_, w, cx| v.update(cx, |this, cx| this.toggle_high_quality(w, cx))
                }));
                // Live (dom/menu-camera-render): its own group.
                m = m.separator();
                m = m.item(crate::os_menu::OsMenuItem::new("Highlight boundary edges").checked(boundary).on_click({
                    let v = view.clone();
                    move |_, w, cx| {
                        v.update(cx, |this, cx| {
                            this.opts.highlight_boundary = !this.opts.highlight_boundary;
                            let d = this.display.clone();
                            this.apply_display(d, w, cx);
                        })
                    }
                }));
                m = m.separator();
                m.item(item(if section { "Turn off section view" } else { "Section view…" }, Box::new(|this, w, cx| this.toggle_section(w, cx))).icon_name("menu-section-view"))
                });
                crate::os_menu::open_anchored(at, Anchor::TopRight, menu, window, cx);
            }));
        // Live (part-studio/21): the tooltip sits left of the button.
        crate::ui::with_tip(div().id("camera-menu-tip").child(button), "Camera and render options", crate::ui::TipSide::Left).into_any_element()
    }
}

/// Onshape's names for the display modes (camera menu).
fn display_label(m: DisplayMode) -> &'static str {
    match m {
        DisplayMode::ShadedWithEdges => "Shaded with edges",
        DisplayMode::Shaded => "Shaded without edges",
        DisplayMode::HiddenLine => "Unshaded",
        DisplayMode::Translucent => "Translucent",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The glob import brings GPUI's `test` attribute; use the built-in one.
    use std::prelude::v1::test;

    #[test]
    fn front_view_hits() {
        let rot = Camera::rotation_for(StandardView::Front);
        assert_eq!(cube_hit(rot, DVec2::ZERO), Some(CubeCell([0, -1, 0])));
        // Past the white square: the back face's frame, seen through.
        assert_eq!(cube_hit(rot, DVec2::new(0.0, 26.0)), Some(CubeCell([0, 1, 0])));
        // The upper right corner's disk.
        assert_eq!(cube_hit(rot, DVec2::new(23.6, 23.6)), Some(CubeCell([1, -1, 1])));
        assert_eq!(cube_hit(rot, DVec2::new(40.0, 0.0)), None);
        assert_eq!(widget_hit(rot, DVec2::new(0.0, 70.0)), Some(Arrow::Up.cell()));
        assert_eq!(widget_hit(rot, DVec2::from_angle(0.3 * std::f64::consts::PI) * 67.5), Some(Arrow::Cw.cell()));
        assert_eq!(Arrow::of(Arrow::Ccw.cell()), Some(Arrow::Ccw));
        assert_eq!(Arrow::of(CubeCell([1, -1, 1])), None);
    }

    #[test]
    fn isometric_corner_and_rotations() {
        let rot = Camera::rotation_for(StandardView::Isometric);
        // The corner nearest the viewer projects to the center.
        assert_eq!(cube_hit(rot, DVec2::ZERO), Some(CubeCell([1, -1, 1])));
        // Three near faces, three far frames, four corner disks.
        assert_eq!(cube_shapes(rot).len(), 10);
        assert!(CubeCell([0, 0, 1]).rotation().abs_diff_eq(Camera::rotation_for(StandardView::Top), 1e-9));
        // Corners give Onshape's trimetric views.
        assert!(CubeCell([1, -1, 1]).rotation().abs_diff_eq(Camera::rotation_for(StandardView::Trimetric), 1e-6));
        let fwd = CubeCell([-1, 1, -1]).rotation() * DVec3::NEG_Z;
        assert!(fwd.z > 0.0 && fwd.x > 0.0 && fwd.y < 0.0, "{fwd}");
        // The roll arcs sit around the cube, which stays inside them.
        assert!((HALF_EDGE - 29.5).abs() < 1e-9);
    }
}
