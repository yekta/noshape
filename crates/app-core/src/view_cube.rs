//! Onshape's View cube (upper right of the graphics area): it turns with
//! the camera; clicking a face or corner animates to that view;
//! arrows around it rotate 15° (Shift 90°, Ctrl 5°) and roll; the
//! dropdown under it is the *Camera and render options* menu. Dragging the
//! cube moves it (webgl bundle `updateDragPosition`): it keeps its offset
//! from the graphics area's top-right corner, at least 15 px from the left,
//! right and top and 40 px from the bottom (`updateCenter`); a press that
//! moves at most LEFT_MOUSE_DRAG_DISTANCE_TOLERANCE (2 px) is a click.

use glam::{DQuat, DVec2};
use gpui_kit::*;

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
        // Live 10-27: it takes the pointer ~8 px up (its disc is ~7 px), then the Top face.
        assert_eq!(cube_hit(rot, DVec2::new(0.0, 8.5)), Some(CubeCell([1, -1, 1])));
        assert_eq!(cube_hit(rot, DVec2::new(0.0, 12.0)), Some(CubeCell([0, 0, 1])));
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
