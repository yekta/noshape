//! Onshape's View cube (upper right of the graphics area): it turns with
//! the camera; clicking a face, edge or corner animates to that view;
//! arrows around it rotate 15° (Shift 90°, Ctrl 5°) and roll; the
//! dropdown under it is the *Camera and render options* menu.

use super::PartStudioView;
use crate::scene_builder::{DisplayMode, TangentEdges};
use crate::theme::{p, with_alpha};
use glam::{DQuat, DVec2, DVec3};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::h_flex;
use gpui_kit::*;
use noshape_render::{Camera, Projection, StandardView};

/// Side of the whole widget (cube, arrows and the view menu button): Onshape's
/// `VIEW_CUBE_SCREEN_SIZE_PX`.
const WIDGET_PX: f32 = 150.0;
/// Side of the cube's hit box in logical pixels.
const CUBE_PX: f32 = 120.0;
/// The cube center sits slightly below the widget's center (measured live).
const CUBE_CENTER_Y: f32 = 78.0;
/// Fraction of a face (from its center) that belongs to the face cell;
/// the rest are edge and corner cells.
const FACE_FRACTION: f64 = 0.56;

/// A clickable region of the cube, identified by the view direction it
/// stands for: one non-zero component = face, two = edge, three = corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CubeCell(pub [i8; 3]);

impl CubeCell {
    fn dir(self) -> DVec3 {
        DVec3::new(self.0[0] as f64, self.0[1] as f64, self.0[2] as f64)
    }

    /// Camera rotation looking at the cube from this cell's direction.
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
        match view {
            Some(v) => Camera::rotation_for(v),
            None => Camera::look_rotation(-self.dir().normalize(), DVec3::Z),
        }
    }
}

/// Face labels by outward normal (Onshape: Front looks along +Y).
const FACES: [([i8; 3], &str); 6] = [([0, -1, 0], "Front"), ([0, 1, 0], "Back"), ([-1, 0, 0], "Left"), ([1, 0, 0], "Right"), ([0, 0, 1], "Top"), ([0, 0, -1], "Bottom")];

/// A cell polygon in widget coordinates (origin at the widget center,
/// y down, cube half-diagonal fits in `radius`).
#[derive(Clone, Debug)]
pub struct CellPoly {
    pub cell: CubeCell,
    pub corners: [DVec2; 4],
}

/// Visible cell polygons for a camera rotation, back to front.
pub fn cube_polys(rotation: DQuat, radius: f64) -> Vec<CellPoly> {
    let inv = rotation.inverse();
    let s = radius / 3f64.sqrt();
    let to_screen = |w: DVec3| {
        let c = inv * w;
        DVec2::new(c.x * s, -c.y * s)
    };
    let mut faces: Vec<(f64, usize)> = FACES
        .iter()
        .enumerate()
        .filter_map(|(i, (n, _))| {
            let z = (inv * CubeCell(*n).dir()).z;
            (z > 1e-3).then_some((z, i))
        })
        .collect();
    faces.sort_by(|a, b| a.0.total_cmp(&b.0));
    let bands = [(-1.0, -FACE_FRACTION), (-FACE_FRACTION, FACE_FRACTION), (FACE_FRACTION, 1.0)];
    let mut out = Vec::new();
    for (_, fi) in faces {
        let n = FACES[fi].0;
        let axis = n.iter().position(|v| *v != 0).unwrap_or(0);
        let (ua, va) = ((axis + 1) % 3, (axis + 2) % 3);
        for (iu, (u0, u1)) in bands.iter().enumerate() {
            for (iv, (v0, v1)) in bands.iter().enumerate() {
                let mut cell = n;
                cell[ua] = iu as i8 - 1;
                cell[va] = iv as i8 - 1;
                let pt = |u: f64, v: f64| {
                    let mut w = CubeCell(n).dir();
                    w[ua] = u;
                    w[va] = v;
                    to_screen(w)
                };
                out.push(CellPoly { cell: CubeCell(cell), corners: [pt(*u0, *v0), pt(*u1, *v0), pt(*u1, *v1), pt(*u0, *v1)] });
            }
        }
    }
    out
}

/// Front-most cell under a widget-local point.
pub fn cube_hit(rotation: DQuat, radius: f64, p: DVec2) -> Option<CubeCell> {
    cube_polys(rotation, radius).into_iter().rev().find(|c| point_in_quad(p, &c.corners)).map(|c| c.cell)
}

fn point_in_quad(p: DVec2, q: &[DVec2; 4]) -> bool {
    let mut sign = 0.0f64;
    for i in 0..4 {
        let (a, b) = (q[i], q[(i + 1) % 4]);
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

/// See [`CubeHooks::state`].
pub type CubeState<'a> = (&'a mut Option<CubeCell>, &'a mut Option<Point<Pixels>>);

/// What a view owning a [`view_cube`] provides: where it keeps the cube's
/// hover cell and screen origin, its camera rotation, and the camera moves.
pub struct CubeHooks<V: 'static> {
    /// The owner's hover cell and the cube's last painted origin.
    pub state: fn(&mut V) -> CubeState<'_>,
    pub rotation: fn(&V, &App) -> DQuat,
    /// Animate to a face, edge or corner view.
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

/// Onshape's 150 px View cube widget with its arrows and `menu` (the
/// camera and render options button) at the bottom right. Shared by the
/// Part Studio and the Assembly: place it 15 px from the graphics area's
/// top-right corner.
pub fn view_cube<V: 'static>(rotation: DQuat, hover: Option<CubeCell>, menu: AnyElement, hooks: CubeHooks<V>, cx: &mut Context<V>) -> AnyElement {
    let pal = p();
    let radius = CUBE_PX as f64 * 0.5 - 5.0;
    let polys = cube_polys(rotation, radius);
    // Onshape (§1.10): white faces, rounded edge/corner cells in #cedbe5
    // at 60%, hover #90cef1 at 60%, black labels, XYZ triad.
    let face_fill: Hsla = pal.panel.into();
    let edge_fill: Hsla = with_alpha(gpui_kit::rgb(0xcedbe5), 0.6).into();
    let hover_fill: Hsla = with_alpha(gpui_kit::rgb(0x90cef1), 0.6).into();
    let polys2 = polys.clone();
    // Triad from the cube's back-bottom-left corner (x red, y green, z blue).
    let inv_t = rotation.inverse();
    let s_t = radius / 3f64.sqrt();
    let scr = move |w: DVec3| {
        let c = inv_t * w;
        DVec2::new(c.x * s_t, -c.y * s_t)
    };
    let t_origin = DVec3::new(-1.0, -1.0, -1.0);
    let triad: Vec<(DVec2, DVec2, Hsla, &'static str)> = [(DVec3::X, 0xcd0000u32, "X"), (DVec3::Y, 0x009400, "Y"), (DVec3::Z, 0x0000d0, "Z")]
        .into_iter()
        .map(|(axis, c, l)| (scr(t_origin), scr(t_origin + axis * 2.55), Hsla::from(gpui_kit::rgb(c)), l))
        .collect();
    let triad_labels: Vec<AnyElement> = triad
        .iter()
        .map(|(_, tip, c, l)| {
            let d = *tip * 1.08;
            div()
                .absolute()
                .left(px(CUBE_PX * 0.5 + d.x as f32 - 5.0))
                .top(px(CUBE_PX * 0.5 + d.y as f32 - 8.0))
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(*c)
                .child(*l)
                .into_any_element()
        })
        .collect();
    let triad2 = triad.clone();
    // Labels drawn *on* the faces turned towards the viewer, like
    // Onshape: glyph outlines laid out in the face plane and projected.
    let inv = rotation.inverse();
    let label_polys: Vec<(Vec<Vec<DVec2>>, f32)> = FACES
        .iter()
        .filter_map(|(n, name)| {
            let c = inv * CubeCell(*n).dir();
            if c.z < 0.2 {
                return None;
            }
            // Face axes: text runs left to right along `right`, upright along `up`.
            let (right, up) = match *n {
                [0, -1, 0] => (DVec3::X, DVec3::Z),
                [0, 1, 0] => (DVec3::NEG_X, DVec3::Z),
                [-1, 0, 0] => (DVec3::NEG_Y, DVec3::Z),
                [1, 0, 0] => (DVec3::Y, DVec3::Z),
                [0, 0, 1] => (DVec3::X, DVec3::Y),
                _ => (DVec3::X, DVec3::NEG_Y),
            };
            let layout = noshape_sketch::text::layout_text(name, noshape_sketch::text::DEFAULT_FONT, false, false);
            // Ascender height in cube units (the face spans -1..1).
            let asc = 0.46;
            let base = CubeCell(*n).dir() - right * (layout.width * asc * 0.5) - up * (asc * 0.36);
            let polys: Vec<Vec<DVec2>> = layout.polylines(5).into_iter().map(|c| c.into_iter().map(|q| scr(base + right * (q.x * asc) + up * (q.y * asc))).collect()).collect();
            Some((polys, c.z as f32))
        })
        .collect();
    let cube = div()
        .id("view-cube")
        .occlude()
        .absolute()
        .left(px(WIDGET_PX * 0.5 - CUBE_PX * 0.5))
        .top(px(CUBE_CENTER_Y - CUBE_PX * 0.5))
        .size(px(CUBE_PX))
        .cursor_pointer()
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let o = bounds.origin;
                    let half = CUBE_PX * 0.5;
                    let pt = |v: DVec2| point(o.x + px(half + v.x as f32), o.y + px(half + v.y as f32));
                    for poly in &polys2 {
                        let mut b = PathBuilder::fill();
                        b.move_to(pt(poly.corners[0]));
                        for c in &poly.corners[1..] {
                            b.line_to(pt(*c));
                        }
                        b.close();
                        let is_face = poly.cell.0.iter().filter(|v| **v != 0).count() == 1;
                        let fill = if hover == Some(poly.cell) {
                            hover_fill
                        } else if is_face {
                            face_fill
                        } else {
                            edge_fill
                        };
                        if let Ok(path) = b.build() {
                            window.paint_path(path, fill);
                        }
                    }
                    for (polys, facing) in &label_polys {
                        let mut b = PathBuilder::fill();
                        for poly in polys {
                            if poly.len() < 3 {
                                continue;
                            }
                            b.move_to(pt(poly[0]));
                            for q in &poly[1..] {
                                b.line_to(pt(*q));
                            }
                            b.close();
                        }
                        if let Ok(path) = b.build() {
                            window.paint_path(path, with_alpha(gpui_kit::rgb(0x000000), 0.3 + 0.7 * facing));
                        }
                    }
                    for (a, tip, c, _) in &triad2 {
                        let mut b = PathBuilder::stroke(px(1.));
                        b.move_to(pt(*a));
                        b.line_to(pt(*tip));
                        if let Ok(path) = b.build() {
                            window.paint_path(path, *c);
                        }
                    }
                },
            )
            .absolute()
            .size_full(),
        )
        .children(triad_labels)
        .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, window, cx| {
            let _ = window;
            let rot = (hooks.rotation)(this, cx);
            let (hover, origin) = (hooks.state)(this);
            let local = origin.map(|o| DVec2::new(f64::from(f32::from(e.position.x - o.x)), f64::from(f32::from(e.position.y - o.y))) - DVec2::splat(CUBE_PX as f64 * 0.5));
            let hit = local.and_then(|l| cube_hit(rot, radius, l));
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
                cx.stop_propagation();
                let rot = (hooks.rotation)(this, cx);
                let origin = *(hooks.state)(this).1;
                let local = origin.map(|o| DVec2::new(f64::from(f32::from(e.position.x - o.x)), f64::from(f32::from(e.position.y - o.y))) - DVec2::splat(CUBE_PX as f64 * 0.5));
                if let Some(cell) = local.and_then(|l| cube_hit(rot, radius, l)) {
                    (hooks.click)(this, cell, cx);
                }
            }),
        )
        .child(
            // Records the widget origin for hit testing.
            canvas(
                {
                    let view = cx.entity();
                    move |bounds, _, cx| {
                        view.update(cx, |this, _| *(hooks.state)(this).1 = Some(bounds.origin));
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        );
    // Onshape: four triangle nudge arrows around the cube and two curved
    // roll arrows above it, `#cedbe5` at 60% (hover `#90cef1` at 60%),
    // as tinted glyphs positioned in the 150 px widget.
    let arrow_color = with_alpha(gpui_kit::rgb(0xcedbe5), 0.6);
    let arrow_hover = with_alpha(gpui_kit::rgb(0x90cef1), 0.6);
    let arrow = |id: &'static str, icon: &'static str, size: f32, cx_: f32, cy: f32, tip: &'static str, dx: f64, dy: f64, roll: f64, cx: &mut Context<V>| {
        div()
            .id(id)
            .absolute()
            .left(px(cx_ - size * 0.5))
            .top(px(cy - size * 0.5))
            .size(px(size))
            .flex()
            .items_center()
            .justify_center()
            .text_color(arrow_color)
            .cursor_pointer()
            .hover(|d| d.text_color(arrow_hover))
            .child(crate::ui::glyph(icon, size, arrow_color))
            .tooltip(crate::ui::tooltip(tip))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    // Onshape: click 15°, Shift+click 90°, Ctrl+click 5°.
                    let k = if e.modifiers.shift {
                        6.0
                    } else if e.modifiers.control || e.modifiers.platform {
                        1.0 / 3.0
                    } else {
                        1.0
                    };
                    if roll != 0.0 {
                        (hooks.roll)(this, roll * k, cx);
                    } else {
                        (hooks.rotate)(this, dx * k, dy * k, cx);
                    }
                }),
            )
    };
    div()
        .relative()
        .size(px(WIDGET_PX))
        .child(arrow("cube-up", "cube-up", 13., 75., 6., "Rotate up", 0.0, -15.0, 0.0, cx))
        .child(arrow("cube-down", "cube-down", 13., 75., 144., "Rotate down", 0.0, 15.0, 0.0, cx))
        .child(arrow("cube-left", "cube-left", 13., 6., 76., "Rotate left", -15.0, 0.0, 0.0, cx))
        .child(arrow("cube-right", "cube-right", 13., 144., 76., "Rotate right", 15.0, 0.0, 0.0, cx))
        .child(arrow("cube-roll-l", "cube-roll-left", 30., 36., 24., "Roll counterclockwise", 0.0, 0.0, 15.0, cx))
        .child(arrow("cube-roll-r", "cube-roll-right", 30., 114., 24., "Roll clockwise", 0.0, 0.0, -15.0, cx))
        .child(cube)
        .child(div().absolute().left(px(106.)).top(px(128.)).child(menu))
        .into_any_element()
}

impl PartStudioView {
    /// Animate to a cube cell's view (remembered for Previous view).
    pub(super) fn cube_click(&mut self, cell: CubeCell, cx: &mut Context<Self>) {
        self.push_camera_history(cx);
        let rot = cell.rotation();
        self.viewport.update(cx, |vp, cx| {
            let mut to = vp.camera;
            to.rotation = rot;
            vp.animate_to(to, cx);
        });
    }

    /// Roll the view about the line of sight (cube arc arrows).
    pub(super) fn roll_view(&mut self, deg: f64, cx: &mut Context<Self>) {
        self.viewport.update(cx, |vp, cx| {
            let mut to = vp.camera;
            let q = DQuat::from_axis_angle(to.forward(), deg.to_radians());
            to.rotation = (q * to.rotation).normalize();
            vp.animate_to(to, cx);
        });
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
        // Onshape: the view-mode cube icon with a ▾ under the cube.
        let pal = p();
        let variant = gpui_kit::component::button::ButtonCustomVariant::new(cx).color(with_alpha(pal.panel, 0.35).into()).foreground(pal.text.into()).hover(pal.panel_hover.into()).active(pal.panel_active.into()).shadow(false);
        Button::new("camera-menu")
            .custom(variant)
            .child(h_flex().gap(px(4.)).child(crate::assets::icon("view-cube", 20., cx)).child(crate::ui::glyph("ui-chevron-down", 8., pal.text)))
            .w(px(44.))
            .h(px(28.))
            .p_0()
            .rounded(px(3.))
            .tooltip("Camera and render options")
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                // Opens under the button, right-aligned with it (live: 222 px,
                // Onshape's context-menu style). The button's bottom-right
                // corner, from the cube's painted origin.
                // The widget's origin is the cube's minus its inset in the widget.
                let at = this.ui.cube_origin.map(|o| {
                    let widget = point(o.x - px(WIDGET_PX * 0.5 - CUBE_PX * 0.5), o.y - px(CUBE_CENTER_Y - CUBE_PX * 0.5));
                    point(widget.x + px(150.), widget.y + px(158.))
                }).unwrap_or_default();
                let view = view.clone();
                let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, window, cx| {
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
                m = m.separator();
                m.item(item(if section { "Turn off section view" } else { "Section view…" }, Box::new(|this, w, cx| this.toggle_section(w, cx))))
                });
                crate::os_menu::open_anchored(at, Anchor::TopRight, menu, window, cx);
            }))
            .into_any_element()
    }
}

/// Onshape's names for the display modes (camera menu).
fn display_label(m: DisplayMode) -> &'static str {
    match m {
        DisplayMode::ShadedWithEdges => "Shaded with edges",
        DisplayMode::Shaded => "Shaded without edges",
        DisplayMode::HiddenLine => "Unshaded",
        DisplayMode::Translucent => "Translucent",
        DisplayMode::Wireframe => "Wireframe",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The glob import brings GPUI's `test` attribute; use the built-in one.
    use std::prelude::v1::test;

    #[test]
    fn front_view_shows_front_face_in_the_middle() {
        let rot = Camera::rotation_for(StandardView::Front);
        assert_eq!(cube_hit(rot, 40.0, DVec2::ZERO), Some(CubeCell([0, -1, 0])));
        // Upper band of the front face is the front-top edge.
        assert_eq!(cube_hit(rot, 40.0, DVec2::new(0.0, -21.0)), Some(CubeCell([0, -1, 1])));
        // Upper right corner.
        assert_eq!(cube_hit(rot, 40.0, DVec2::new(21.0, -21.0)), Some(CubeCell([1, -1, 1])));
        // Outside the cube.
        assert_eq!(cube_hit(rot, 40.0, DVec2::new(60.0, 0.0)), None);
    }

    #[test]
    fn isometric_corner_and_rotations() {
        let rot = Camera::rotation_for(StandardView::Isometric);
        // The corner nearest the viewer projects to the center.
        assert_eq!(cube_hit(rot, 40.0, DVec2::ZERO), Some(CubeCell([1, -1, 1])));
        assert_eq!(cube_polys(rot, 40.0).len(), 27);
        // Face cells reproduce the standard views.
        assert!(CubeCell([0, 0, 1]).rotation().abs_diff_eq(Camera::rotation_for(StandardView::Top), 1e-9));
        // Edge cells look along the negated diagonal.
        let fwd = CubeCell([1, -1, 0]).rotation() * DVec3::NEG_Z;
        assert!(fwd.abs_diff_eq(DVec3::new(-1.0, 1.0, 0.0).normalize(), 1e-9));
    }
}
