//! Onshape's View cube (upper right of the graphics area): it turns with
//! the camera; clicking a face, edge or corner animates to that view;
//! arrows around it rotate 15° (Shift 90°, Ctrl 5°) and roll; the
//! dropdown under it is the *Camera and render options* menu.

use super::PartStudioView;
use crate::scene_builder::DisplayMode;
use crate::theme::{p, with_alpha};
use glam::{DQuat, DVec2, DVec3};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use noshape_render::{Camera, Projection, StandardView};

/// Side of the cube widget in logical pixels.
const CUBE_PX: f32 = 104.0;
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

    /// Tooltip-style name ("Front", "Top right edge", ...).
    pub fn label(self) -> String {
        let [x, y, z] = self.0;
        let part = |v: i8, neg: &'static str, pos: &'static str| match v {
            -1 => Some(neg),
            1 => Some(pos),
            _ => None,
        };
        let words: Vec<&str> = [part(z, "Bottom", "Top"), part(y, "Front", "Back"), part(x, "Left", "Right")].into_iter().flatten().collect();
        match words.len() {
            1 => words[0].to_string(),
            2 => format!("{} {}", words[0], words[1].to_lowercase()),
            _ => format!("{} {} {}", words[0], words[1].to_lowercase(), words[2].to_lowercase()),
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
        let pal = p();
        let rotation = self.viewport.read(cx).camera.rotation;
        let hover = self.ui.cube_hover;
        let radius = CUBE_PX as f64 * 0.5 - 4.0;
        let polys = cube_polys(rotation, radius);
        // Onshape (§1.10): white faces, rounded edge/corner cells in #cedbe5
        // at 60%, hover #90cef1 at 60%, black labels, XYZ triad.
        let face_fill: Hsla = pal.panel.into();
        let edge_fill: Hsla = with_alpha(gpui_kit::rgb(0xcedbe5), 0.6).into();
        let hover_fill: Hsla = with_alpha(gpui_kit::rgb(0x90cef1), 0.6).into();
        let line: Hsla = gpui_kit::rgb(0x9aa5af).into();
        let polys2 = polys.clone();
        // Triad from the cube's front-left-bottom corner (x red, y green, z blue).
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
                    .text_color(*c)
                    .child(*l)
                    .into_any_element()
            })
            .collect();
        let triad2 = triad.clone();
        // Labels at the centers of faces turned towards the viewer.
        let inv = rotation.inverse();
        let labels: Vec<AnyElement> = FACES
            .iter()
            .filter_map(|(n, name)| {
                let c = inv * CubeCell(*n).dir();
                if c.z < 0.35 {
                    return None;
                }
                let s = radius / 3f64.sqrt();
                let (x, y) = (c.x * s, -c.y * s);
                Some(
                    div()
                        .absolute()
                        .left(px(CUBE_PX * 0.5 + x as f32 - 24.0))
                        .top(px(CUBE_PX * 0.5 + y as f32 - 8.0))
                        .w(px(48.))
                        .text_center()
                        .text_size(px(12.))
                        .text_color(with_alpha(gpui_kit::rgb(0x000000), 0.25 + 0.75 * c.z as f32))
                        .child(*name)
                        .into_any_element(),
                )
            })
            .collect();
        let cube = div()
            .id("view-cube")
            .occlude()
            .relative()
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
                            let mut b = PathBuilder::stroke(px(0.75));
                            b.move_to(pt(poly.corners[0]));
                            for c in &poly.corners[1..] {
                                b.line_to(pt(*c));
                            }
                            b.close();
                            if let Ok(path) = b.build() {
                                window.paint_path(path, line.opacity(0.35));
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
            .children(labels)
            .children(triad_labels)
            .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, window, cx| {
                let _ = window;
                let local = this.ui.cube_origin.map(|o| DVec2::new(f64::from(f32::from(e.position.x - o.x)), f64::from(f32::from(e.position.y - o.y))) - DVec2::splat(CUBE_PX as f64 * 0.5));
                let rot = this.viewport.read(cx).camera.rotation;
                let hit = local.and_then(|l| cube_hit(rot, radius, l));
                if hit != this.ui.cube_hover {
                    this.ui.cube_hover = hit;
                    cx.notify();
                }
            }))
            .on_hover(cx.listener(|this, h: &bool, _, cx| {
                if !*h && this.ui.cube_hover.take().is_some() {
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    let local = this.ui.cube_origin.map(|o| DVec2::new(f64::from(f32::from(e.position.x - o.x)), f64::from(f32::from(e.position.y - o.y))) - DVec2::splat(CUBE_PX as f64 * 0.5));
                    let rot = this.viewport.read(cx).camera.rotation;
                    if let Some(cell) = local.and_then(|l| cube_hit(rot, radius, l)) {
                        this.cube_click(cell, cx);
                    }
                }),
            )
            .child(
                // Records the widget origin for hit testing.
                canvas(
                    {
                        let view = cx.entity();
                        move |bounds, _, cx| {
                            view.update(cx, |this, _| this.ui.cube_origin = Some(bounds.origin));
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            );
        let hover_label = hover.map(|c| c.label());
        let arrow = |id: &'static str, glyph: &'static str, tip: &'static str, dx: f64, dy: f64, roll: f64, cx: &mut Context<Self>| {
            div()
                .id(id)
                .px_1()
                .text_size(px(13.))
                .text_color(gpui_kit::rgb(0xb4c4d3))
                .cursor_pointer()
                .hover(|d| d.text_color(gpui_kit::rgb(0x90cef1)))
                .child(glyph)
                .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(tip).build(window, cx))
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
                            this.roll_view(roll * k, cx);
                        } else {
                            this.rotate_by(&(), dx * k, dy * k, cx);
                        }
                    }),
                )
        };
        v_flex()
            .items_center()
            .child(
                h_flex()
                    .justify_between()
                    .w(px(CUBE_PX + 24.))
                    .child(arrow("cube-roll-l", "↺", "Roll left", 0.0, 0.0, 15.0, cx))
                    .child(arrow("cube-up", "▲", "Rotate up", 0.0, -15.0, 0.0, cx))
                    .child(arrow("cube-roll-r", "↻", "Roll right", 0.0, 0.0, -15.0, cx)),
            )
            .child(
                h_flex()
                    .items_center()
                    .child(arrow("cube-left", "◀", "Rotate left", -15.0, 0.0, 0.0, cx))
                    .child(cube)
                    .child(arrow("cube-right", "▶", "Rotate right", 15.0, 0.0, 0.0, cx)),
            )
            .child(
                h_flex()
                    .gap_1()
                    .items_center()
                    .child(arrow("cube-down", "▼", "Rotate down", 0.0, 15.0, 0.0, cx))
                    .child(self.render_camera_menu(cx)),
            )
            .child(div().h(px(14.)).text_xs().text_color(pal.text_muted).children(hover_label))
            .into_any_element()
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
        // Onshape: the view-mode cube icon with a ▾ under the cube.
        Button::new("camera-menu")
            .ghost()
            .child(h_flex().gap(px(2.)).child(crate::assets::icon("view-cube", 20., cx)).child(crate::ui::glyph("ui-chevron-down", 8., p().text)))
            .h(px(24.))
            .px(px(4.))
            .rounded(px(3.))
            .tooltip("Camera and render options")
            .dropdown_menu(move |mut m, _, _| {
                let item = |label: &str, f: super::ViewAction| {
                    let v = view.clone();
                    PopupMenuItem::new(label.to_string()).on_click(move |_, w, cx| v.update(cx, |this, cx| f(this, w, cx)))
                };
                for (label, sv) in [("Isometric", StandardView::Isometric), ("Dimetric", StandardView::Dimetric), ("Trimetric", StandardView::Trimetric)] {
                    m = m.item(item(label, Box::new(move |this, _, cx| this.view(sv, cx))));
                }
                m = m.separator();
                m = m.item(item("Named views…", Box::new(|this, w, cx| this.toggle_named_views(w, cx))));
                if has_history {
                    m = m.item(item("Previous view", Box::new(|this, _, cx| this.previous_view(cx))));
                }
                m = m.item(item("Zoom to fit", Box::new(|this, _, cx| this.zoom_fit(cx))));
                m = m.item(item("Zoom to window", Box::new(|this, _, cx| this.toggle_zoom_window(cx))));
                if has_selection {
                    m = m.item(item("Zoom to selection", Box::new(|this, _, cx| this.zoom_to_selection(cx))));
                }
                let v = view.clone();
                m = m.item(PopupMenuItem::new("Perspective view").checked(perspective).on_click(move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.viewport.update(cx, |vp, cx| {
                            let p = if vp.camera.projection == Projection::Perspective { Projection::Orthographic } else { Projection::Perspective };
                            vp.set_projection(p, cx);
                        })
                    })
                }));
                let v = view.clone();
                m = m.item(PopupMenuItem::new("Hidden edges visible").checked(hidden_edges).on_click(move |_, w, cx| {
                    v.update(cx, |this, cx| {
                        this.opts.hidden_edges_visible = !this.opts.hidden_edges_visible;
                        let d = this.display.clone();
                        this.apply_display(d, w, cx);
                    })
                }));
                let v = view.clone();
                m = m.item(PopupMenuItem::new("Orient normal to sketch on edit").checked(orient).on_click(move |_, _, cx| {
                    v.update(cx, |this, cx| {
                        this.ui.orient_on_edit = !this.ui.orient_on_edit;
                        cx.notify();
                    })
                }));
                m = m.separator();
                for dm in DisplayMode::ALL {
                    let v = view.clone();
                    m = m.item(PopupMenuItem::new(dm.label()).checked(dm == mode).on_click(move |_, w, cx| v.update(cx, |this, cx| this.set_display_mode(dm, w, cx))));
                }
                m = m.separator();
                m.item(item(if section { "Turn off section view" } else { "Section view" }, Box::new(|this, w, cx| this.toggle_section(w, cx))))
            })
            .into_any_element()
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
        assert_eq!(CubeCell([1, -1, 1]).label(), "Top front right");
        assert_eq!(CubeCell([0, -1, 1]).label(), "Top front");
    }
}
