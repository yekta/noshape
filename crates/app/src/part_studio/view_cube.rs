//! Onshape's View cube (upper right of the graphics area): it turns with
//! the camera; clicking a face or corner animates to that view;
//! arrows around it rotate 15° (Shift 90°, Ctrl 5°) and roll; the
//! dropdown under it is the *Camera and render options* menu. Dragging the
//! cube moves it (webgl bundle `updateDragPosition`): it keeps its offset
//! from the graphics area's top-right corner, at least 15 px from the left,
//! right and top and 40 px from the bottom (`updateCenter`); a press that
//! moves at most LEFT_MOUSE_DRAG_DISTANCE_TOLERANCE (2 px) is a click.

use super::PartStudioView;
use crate::theme::{p, with_alpha};
use glam::{DQuat, DVec2};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::Selectable as _;
use gpui_kit::component::h_flex;
use gpui_kit::*;
use noshape_render::Projection;

// The cube's geometry is shared with its drawing in the 3D frame
// (`noshape_render::overlay::view_cube`, `NOSHAPE_OVERLAY=wgpu`).
pub use noshape_render::overlay::view_cube::{Arrow, CubeCell, WIDGET_PX, widget_hit};

/// Onshape's LEFT_MOUSE_DRAG_DISTANCE_TOLERANCE (px moved, |dx| + |dy|).
const CLICK_TOLERANCE: f32 = 2.0;
/// VIEW_CUBE_LEFT/RIGHT/TOP/BOTTOM_MARGIN_PX.
const MARGINS: [f32; 4] = [15.0, 15.0, 15.0, 40.0];

/// A cube's move from its place, and the press being dragged (per owner).
#[derive(Default)]
struct CubeDrag {
    offset: Point<Pixels>,
    /// Last pointer position, px moved, and the cell and modifiers pressed.
    press: Option<(Point<Pixels>, f32, CubeCell, Modifiers)>,
}

thread_local! {
    static DRAGS: std::cell::RefCell<std::collections::HashMap<EntityId, CubeDrag>> = Default::default();
}

/// `offset` kept inside the margins of a `viewport` (logical px) for a cube
/// placed at its default spot (15 px from the top-right corner).
fn clamp_offset(offset: Point<Pixels>, viewport: DVec2) -> Point<Pixels> {
    let [left, right, top, bottom] = MARGINS;
    let (w, h) = (viewport.x as f32, viewport.y as f32);
    // Default left edge w - 15 - 150, top 15.
    let min_x = left - (w - right - WIDGET_PX);
    let max_y = h - bottom - WIDGET_PX - top;
    point(px(f32::from(offset.x).clamp(min_x.min(0.0), 0.0)), px(f32::from(offset.y).clamp(0.0, max_y.max(0.0))))
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
/// `viewport`: the graphics area's size (logical px), which a moved cube
/// stays in.
pub fn view_cube<V: 'static>(menu: AnyElement, hooks: CubeHooks<V>, viewport: DVec2, cx: &mut Context<V>) -> AnyElement {
    let local = |pos: Point<Pixels>, o: Point<Pixels>| {
        let d = DVec2::new(f64::from(f32::from(pos.x - o.x)), f64::from(f32::from(pos.y - o.y))) - DVec2::from(noshape_render::overlay::view_cube::CENTER);
        DVec2::new(d.x, -d.y)
    };
    let view = cx.entity();
    let id = view.entity_id();
    let (offset, dragging) = DRAGS.with_borrow_mut(|d| {
        let d = d.entry(id).or_default();
        d.offset = clamp_offset(d.offset, viewport);
        (d.offset, d.press.is_some())
    });
    // A cell's action, on a click (a press that hardly moved).
    let act = move |this: &mut V, cell: CubeCell, modifiers: Modifiers, cx: &mut Context<V>| {
        let Some(arrow) = Arrow::of(cell) else {
            (hooks.click)(this, cell, cx);
            return;
        };
        // Onshape: click 15°, Shift+click 90°, Ctrl+click 5°.
        let k = if modifiers.shift {
            6.0
        } else if modifiers.control || modifiers.platform {
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
    };
    div()
        .id("view-cube")
        .relative()
        .left(offset.x)
        .top(offset.y)
        .size(px(WIDGET_PX))
        // Live (dom/cursors-controls.json): the cube's faces and arrows keep
        // the default cursor (they're part of the canvas), no pointer.
        .child(
            canvas(
                {
                    let view = view.clone();
                    move |bounds, _, cx| {
                        // Records the widget origin for hit testing.
                        view.update(cx, |this, cx| {
                            *(hooks.state)(this).1 = Some(bounds.origin);
                            (hooks.placed)(this, bounds.origin, cx);
                        });
                    }
                },
                move |_, _, window, _| {
                    // While pressed, follow the pointer anywhere in the window.
                    if !dragging {
                        return;
                    }
                    let view = view.clone();
                    window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                        if phase != DispatchPhase::Bubble {
                            return;
                        }
                        let moved = DRAGS.with_borrow_mut(|d| {
                            let d = d.entry(id).or_default();
                            let Some((last, dist, _, _)) = &mut d.press else { return false };
                            let delta = e.position - *last;
                            *last = e.position;
                            *dist += f32::from(delta.x).abs() + f32::from(delta.y).abs();
                            // Moves only once it is a drag.
                            if *dist > CLICK_TOLERANCE {
                                d.offset += delta;
                            }
                            true
                        });
                        if moved {
                            view.update(cx, |_, cx| cx.notify());
                        }
                    });
                },
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
                DRAGS.with_borrow_mut(|d| d.entry(id).or_default().press = Some((e.position, 0.0, cell, e.modifiers)));
                cx.notify();
            }),
        )
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(move |this, _: &MouseUpEvent, _, cx| {
                let press = DRAGS.with_borrow_mut(|d| d.entry(id).or_default().press.take());
                if let Some((_, dist, cell, modifiers)) = press {
                    cx.stop_propagation();
                    if dist <= CLICK_TOLERANCE {
                        act(this, cell, modifiers, cx);
                    }
                    cx.notify();
                }
            }),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            cx.listener(move |_, _: &MouseUpEvent, _, cx| {
                if DRAGS.with_borrow_mut(|d| d.entry(id).or_default().press.take()).is_some() {
                    cx.notify();
                }
            }),
        )
        // Occludes the viewport: its click would take the focus back from the
        // just-opened menu, and Esc would no longer close it.
        .child(div().absolute().left(px(106.)).top(px(128.)).occlude().child(menu))
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
        let size = self.viewport.read(cx).size();
        view_cube(menu, hooks, size, cx)
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

    /// The camera menu's picks ([`crate::camera_menu`]).
    fn on_camera_action(&mut self, a: crate::camera_menu::Action, w: &mut Window, cx: &mut Context<Self>) {
        use crate::camera_menu::Action;
        match a {
            Action::View(sv) => self.view(sv, cx),
            Action::GraphicsPrefs => self.open_graphics_prefs(w, cx),
            Action::NamedViews => self.toggle_named_views(w, cx),
            Action::PreviousView => self.previous_view(cx),
            Action::ZoomFit => self.zoom_fit(cx),
            Action::ZoomWindow => self.toggle_zoom_window(cx),
            Action::ZoomSelection => self.zoom_to_selection(cx),
            Action::Perspective => self.viewport.update(cx, |vp, cx| {
                let p = if vp.camera.projection == Projection::Perspective { Projection::Orthographic } else { Projection::Perspective };
                vp.set_projection(p, cx);
            }),
            Action::Orient => {
                self.ui.orient_on_edit = !self.ui.orient_on_edit;
                cx.notify();
            }
            Action::Display(dm) => self.set_display_mode(dm, w, cx),
            Action::HiddenEdges(on) => {
                self.opts.hidden_edges_visible = on;
                let d = self.display.clone();
                self.apply_display(d, w, cx);
            }
            Action::Tangent(t) => {
                self.opts.tangent_edges = t;
                let d = self.display.clone();
                self.apply_display(d, w, cx);
            }
            Action::HighQuality => self.toggle_high_quality(w, cx),
            Action::Boundary => {
                self.opts.highlight_boundary = !self.opts.highlight_boundary;
                let d = self.display.clone();
                self.apply_display(d, w, cx);
            }
            Action::Section if !self.ui.section.is_empty() => self.turn_off_section(w, cx),
            Action::Section => self.toggle_section(w, cx),
            Action::EditSection => self.edit_section(w, cx),
        }
    }

    /// Onshape's "Camera and render options" dropdown (under the cube).
    fn render_camera_menu(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity();
        let mode = self.opts.display_mode;
        let perspective = self.viewport.read(cx).camera.projection == Projection::Perspective;
        let has_selection = !self.selection.is_empty() || !self.selected_features.is_empty();
        let section = !self.ui.section.is_empty();
        // Live (psb 10-21 f): no Section view item while a feature dialog is open.
        let dialog_open = self.edit.is_some();
        let has_history = !self.ui.cam_history.is_empty();
        let orient = self.ui.orient_on_edit;
        let hidden_edges = self.opts.hidden_edges_visible;
        let tangent = self.opts.tangent_edges;
        let high_quality = self.ui.high_quality;
        let boundary = self.opts.highlight_boundary;
        let state = crate::camera_menu::State { perspective, has_history, has_selection, mode, hidden_edges, tangent, high_quality, section_on: section, section_dialog: self.ui.section_dialog.is_some(), show_section: !dialog_open, orient: Some(orient), boundary: Some(boundary) };
        // Onshape: the view-mode cube icon with a ▾ under the cube.
        let pal = p();
        // Live (part-studio/21, 53): no fill at rest, #eaeaea on hover,
        // #e5e5e5 while its menu is open; an 8x4 #333 caret.
        let open = self.ui.camera_menu.as_ref().is_some_and(|m| crate::os_menu::is_open_menu(m, cx));
        let variant = gpui_kit::component::button::ButtonCustomVariant::new(cx).color(with_alpha(pal.panel, 0.35).into()).foreground(pal.text.into()).hover(pal.panel_hover.into()).active(gpui_kit::rgb(0xe5e5e5).into()).shadow(false);
        let button = Button::new("camera-menu")
            .custom(variant)
            // The open style paints the variant's active colour.
            .open(open)
            .child(h_flex().gap(px(4.)).child(crate::assets::icon("view-cube", 20., cx)).child(crate::ui::glyph("camera-caret", 12., gpui_kit::rgb(0x333333))))
            .w(px(44.))
            .h(px(28.))
            .p_0()
            .rounded(px(3.))
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                // Opens under the button, right-aligned with it (live: 222 px,
                // Onshape's context-menu style). The button's bottom-right
                // corner, from the widget's painted origin.
                // Live (part-studio/53): the menu's right border at the
                // widget's right edge + 1.
                let at = this.ui.cube_origin.map(|o| point(o.x + px(WIDGET_PX + 1.), o.y + px(156.))).unwrap_or_default();
                let view = view.clone();
                let menu = crate::os_menu::OsMenu::build(window, cx, move |m, window, cx| crate::camera_menu::build(m.camera(), state, &view, Self::on_camera_action, window, cx));
                this.ui.camera_menu = Some(menu.downgrade());
                crate::os_menu::open_anchored(at, Anchor::TopRight, menu, window, cx);
            }));
        // Live (part-studio/21): the tooltip sits left of the button.
        crate::ui::with_tip(div().id("camera-menu-tip").child(button), "Camera and render options", crate::ui::TipSide::Left).into_any_element()
    }
}

/// Onshape's names for the display modes (camera menu).
#[cfg(test)]
mod tests {
    use super::*;
    use noshape_render::StandardView;
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
