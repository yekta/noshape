//! Onshape's View cube (upper right of the graphics area): it turns with
//! the camera; clicking a face or corner animates to that view;
//! arrows around it rotate 15° (Shift 90°, Ctrl 5°) and roll; the
//! dropdown under it is the *Camera and render options* menu.

use super::PartStudioView;
use crate::scene_builder::{DisplayMode, TangentEdges};
use crate::theme::{p, with_alpha};
use glam::{DQuat, DVec2};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::h_flex;
use gpui_kit::*;
use noshape_render::{Projection, StandardView};

// The cube's geometry is shared with its drawing in the 3D frame
// (`noshape_render::overlay::view_cube`, `NOSHAPE_OVERLAY=wgpu`).
pub use noshape_render::overlay::view_cube::{Arrow, CubeCell, WIDGET_PX, widget_hit};

const G: f64 = WIDGET_PX as f64;

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
    /// The widget was laid out at this window position (its drawing in the
    /// 3D frame follows it, `NOSHAPE_OVERLAY=wgpu`).
    pub placed: fn(&mut V, Point<Pixels>, &mut Context<V>),
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
/// top-right corner. The cube itself is drawn into the 3D frame
/// (`noshape_render::overlay::view_cube`, so it turns with the camera without
/// any UI layout): this element is its hit area and reports where it was
/// laid out (`CubeHooks::placed`).
pub fn view_cube<V: 'static>(menu: AnyElement, hooks: CubeHooks<V>, cx: &mut Context<V>) -> AnyElement {
    let local = |pos: Point<Pixels>, o: Point<Pixels>| {
        let d = DVec2::new(f64::from(f32::from(pos.x - o.x)), f64::from(f32::from(pos.y - o.y))) - DVec2::splat(G * 0.5);
        DVec2::new(d.x, -d.y)
    };
    let view = cx.entity();
    div()
        .id("view-cube")
        .relative()
        .size(px(WIDGET_PX))
        // Live (dom/cursors-controls.json): the cube's faces and arrows keep
        // the default cursor (they're part of the canvas), no pointer.
        .child(
            canvas(
                move |bounds, _, cx| {
                    // Records the widget origin for hit testing.
                    view.update(cx, |this, cx| {
                        *(hooks.state)(this).1 = Some(bounds.origin);
                        (hooks.placed)(this, bounds.origin, cx);
                    });
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
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
        let hover = self.ui.cube_hover;
        self.viewport.update(cx, |vp, cx| vp.set_view_cube(Some(hover), cx));
        let menu = self.render_camera_menu(cx);
        let hooks = CubeHooks {
            state: |this: &mut Self| (&mut this.ui.cube_hover, &mut this.ui.cube_origin),
            rotation: |this: &Self, cx| this.viewport.read(cx).camera.rotation,
            click: Self::cube_click,
            rotate: |this, dx, dy, cx| this.rotate_by(&(), dx, dy, cx),
            roll: Self::roll_view,
            placed: |this, at, cx| this.viewport.update(cx, |vp, cx| vp.set_view_cube_origin(at, cx)),
        };
        view_cube(menu, hooks, cx)
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
                let at = this.ui.cube_origin.map(|o| point(o.x + px(WIDGET_PX), o.y + px(156.))).unwrap_or_default();
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
    use noshape_render::Camera;
    use glam::DVec3;
    use noshape_render::overlay::view_cube::{HALF_EDGE, cube_hit, cube_shapes};
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
