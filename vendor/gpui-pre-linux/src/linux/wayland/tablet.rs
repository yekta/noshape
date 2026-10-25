//! NOSHAPE PATCH (agent nav2): graphics tablets (`zwp_tablet_v2`), so a pen
//! reaches the app the way Chrome reports it to a page
//! (ui/ozone/platform/wayland/host/wayland_tablet_tool.cc): pen-like tools
//! (pen, eraser, brush, pencil, airbrush) in contact become touches, which is
//! what Onshape's graphics do with `pointerType` "pen", unless the app treats
//! the pen as a mouse (`gpui::set_pen_as_mouse`); finger tools are touches,
//! mouse and lens tools a mouse. A hovering tool moves the mouse. Each tool
//! frame is dispatched in Chrome's order: proximity in, tip down / up (left
//! button), barrel buttons (BTN_STYLUS middle, BTN_STYLUS2 right), motion,
//! proximity out.
//!
//! Without this the compositor emulated a pointer for the pen, so a pen drag
//! was a left-button drag (a box selection) instead of a rotation.

use super::*;
use wayland_protocols::wp::tablet::zv2::client::{
    zwp_tablet_manager_v2, zwp_tablet_pad_dial_v2, zwp_tablet_pad_group_v2, zwp_tablet_pad_ring_v2,
    zwp_tablet_pad_strip_v2, zwp_tablet_pad_v2, zwp_tablet_seat_v2, zwp_tablet_tool_v2, zwp_tablet_v2,
};

const BTN_STYLUS: u32 = 0x14b;
const BTN_STYLUS2: u32 = 0x14c;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolKind {
    Pen,
    Finger,
    Mouse,
}

#[derive(Default)]
struct Frame {
    proximity_in: Option<wl_surface::WlSurface>,
    down: bool,
    up: bool,
    buttons: Vec<(MouseButton, bool)>,
    location: Option<Point<Pixels>>,
    proximity_out: bool,
}

struct Tool {
    kind: ToolKind,
    window: Option<WaylandWindowStatePtr>,
    position: Point<Pixels>,
    /// Buttons held (the tip is the left button).
    held: Vec<MouseButton>,
    /// The touch id of the current contact (as a touch).
    contact: u64,
    frame: Frame,
}

#[derive(Default)]
pub(crate) struct TabletState {
    seat: Option<zwp_tablet_seat_v2::ZwpTabletSeatV2>,
    tools: HashMap<ObjectId, Tool>,
    contacts: u64,
}

impl TabletState {
    /// Ask for the seat's tablets once the seat is known.
    pub(crate) fn bind_seat(
        &mut self,
        manager: Option<&zwp_tablet_manager_v2::ZwpTabletManagerV2>,
        seat: &wl_seat::WlSeat,
        qh: &QueueHandle<WaylandClientStatePtr>,
    ) {
        if self.seat.is_none()
            && let Some(manager) = manager
        {
            self.seat = Some(manager.get_tablet_seat(seat, qh, ()));
        }
    }
}

impl Tool {
    fn as_touch(&self) -> bool {
        self.kind == ToolKind::Finger || (self.kind == ToolKind::Pen && !gpui::pen_as_mouse())
    }

    /// The pressed button reported with mouse moves (X11's order).
    fn pressed_button(&self) -> Option<MouseButton> {
        [MouseButton::Left, MouseButton::Middle, MouseButton::Right]
            .into_iter()
            .find(|b| self.held.contains(b))
    }
}

fn touch(id: u64, phase: TouchPhase, position: Point<Pixels>) -> PlatformInput {
    PlatformInput::Touch(gpui::TouchEvent {
        id: gpui::TouchId(id),
        phase,
        position,
        predicted_position: None,
        force: None,
    })
}

/// The inputs for one tool frame, in Chrome's order.
fn frame_inputs(
    tool: &mut Tool,
    contacts: &mut u64,
    modifiers: Modifiers,
    window_for: &mut dyn FnMut(&wl_surface::WlSurface) -> Option<WaylandWindowStatePtr>,
) -> Vec<(WaylandWindowStatePtr, PlatformInput)> {
    let frame = std::mem::take(&mut tool.frame);
    let mut out = Vec::new();
    if let Some(surface) = &frame.proximity_in {
        tool.window = window_for(surface);
        // Chrome's proximity-in carries the frame's location, so a tip that
        // lands in the same frame presses there.
        if let Some(position) = frame.location {
            tool.position = position;
        }
    }
    let mut presses: Vec<(MouseButton, bool)> = Vec::new();
    if frame.down {
        presses.push((MouseButton::Left, true));
    }
    if frame.up {
        presses.push((MouseButton::Left, false));
    }
    presses.extend(frame.buttons);
    let as_touch = tool.as_touch();
    for (button, pressed) in presses {
        let Some(window) = tool.window.clone() else { break };
        if pressed {
            if tool.held.contains(&button) {
                continue;
            }
            tool.held.push(button);
            if as_touch {
                let phase = if tool.held.len() == 1 {
                    *contacts += 1;
                    tool.contact = (1 << 48) + *contacts;
                    TouchPhase::Started
                } else {
                    TouchPhase::Moved
                };
                out.push((window, touch(tool.contact, phase, tool.position)));
            } else {
                out.push((
                    window,
                    PlatformInput::MouseDown(MouseDownEvent {
                        button,
                        position: tool.position,
                        modifiers,
                        click_count: 1,
                        first_mouse: false,
                    }),
                ));
            }
        } else {
            if !tool.held.contains(&button) {
                continue;
            }
            tool.held.retain(|b| *b != button);
            if as_touch {
                let phase = if tool.held.is_empty() { TouchPhase::Ended } else { TouchPhase::Moved };
                out.push((window, touch(tool.contact, phase, tool.position)));
            } else {
                out.push((
                    window,
                    PlatformInput::MouseUp(MouseUpEvent {
                        button,
                        position: tool.position,
                        modifiers,
                        click_count: 1,
                    }),
                ));
            }
        }
    }
    if let Some(position) = frame.location {
        tool.position = position;
        if let Some(window) = tool.window.clone() {
            if as_touch && !tool.held.is_empty() {
                out.push((window, touch(tool.contact, TouchPhase::Moved, position)));
            } else {
                out.push((
                    window,
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position,
                        pressed_button: if as_touch { None } else { tool.pressed_button() },
                        modifiers,
                    }),
                ));
            }
        }
    }
    if frame.proximity_out {
        if let Some(window) = tool.window.take() {
            if as_touch && !tool.held.is_empty() {
                out.push((window.clone(), touch(tool.contact, TouchPhase::Cancelled, tool.position)));
            }
            out.push((
                window,
                PlatformInput::MouseExited(MouseExitEvent {
                    position: tool.position,
                    pressed_button: None,
                    modifiers,
                }),
            ));
        }
        tool.held.clear();
    }
    out
}

delegate_noop!(WaylandClientStatePtr: ignore zwp_tablet_manager_v2::ZwpTabletManagerV2);
delegate_noop!(WaylandClientStatePtr: ignore zwp_tablet_v2::ZwpTabletV2);
delegate_noop!(WaylandClientStatePtr: ignore zwp_tablet_pad_ring_v2::ZwpTabletPadRingV2);
delegate_noop!(WaylandClientStatePtr: ignore zwp_tablet_pad_strip_v2::ZwpTabletPadStripV2);
delegate_noop!(WaylandClientStatePtr: ignore zwp_tablet_pad_dial_v2::ZwpTabletPadDialV2);

impl Dispatch<zwp_tablet_seat_v2::ZwpTabletSeatV2, ()> for WaylandClientStatePtr {
    fn event(
        this: &mut Self,
        _: &zwp_tablet_seat_v2::ZwpTabletSeatV2,
        event: zwp_tablet_seat_v2::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwp_tablet_seat_v2::Event::ToolAdded { id } = event {
            let client = this.get_client();
            let mut state = client.borrow_mut();
            state.tablet.tools.insert(
                id.id(),
                Tool {
                    kind: ToolKind::Pen,
                    window: None,
                    position: point(px(0.0), px(0.0)),
                    held: Vec::new(),
                    contact: 0,
                    frame: Frame::default(),
                },
            );
        }
    }

    event_created_child!(WaylandClientStatePtr, zwp_tablet_seat_v2::ZwpTabletSeatV2, [
        zwp_tablet_seat_v2::EVT_TABLET_ADDED_OPCODE => (zwp_tablet_v2::ZwpTabletV2, ()),
        zwp_tablet_seat_v2::EVT_TOOL_ADDED_OPCODE => (zwp_tablet_tool_v2::ZwpTabletToolV2, ()),
        zwp_tablet_seat_v2::EVT_PAD_ADDED_OPCODE => (zwp_tablet_pad_v2::ZwpTabletPadV2, ()),
    ]);
}

impl Dispatch<zwp_tablet_pad_v2::ZwpTabletPadV2, ()> for WaylandClientStatePtr {
    fn event(
        _: &mut Self,
        _: &zwp_tablet_pad_v2::ZwpTabletPadV2,
        _: zwp_tablet_pad_v2::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }

    event_created_child!(WaylandClientStatePtr, zwp_tablet_pad_v2::ZwpTabletPadV2, [
        zwp_tablet_pad_v2::EVT_GROUP_OPCODE => (zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2, ()),
    ]);
}

impl Dispatch<zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2, ()> for WaylandClientStatePtr {
    fn event(
        _: &mut Self,
        _: &zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2,
        _: zwp_tablet_pad_group_v2::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }

    event_created_child!(WaylandClientStatePtr, zwp_tablet_pad_group_v2::ZwpTabletPadGroupV2, [
        zwp_tablet_pad_group_v2::EVT_RING_OPCODE => (zwp_tablet_pad_ring_v2::ZwpTabletPadRingV2, ()),
        zwp_tablet_pad_group_v2::EVT_STRIP_OPCODE => (zwp_tablet_pad_strip_v2::ZwpTabletPadStripV2, ()),
        zwp_tablet_pad_group_v2::EVT_DIAL_OPCODE => (zwp_tablet_pad_dial_v2::ZwpTabletPadDialV2, ()),
    ]);
}

impl Dispatch<zwp_tablet_tool_v2::ZwpTabletToolV2, ()> for WaylandClientStatePtr {
    fn event(
        this: &mut Self,
        tool: &zwp_tablet_tool_v2::ZwpTabletToolV2,
        event: zwp_tablet_tool_v2::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use zwp_tablet_tool_v2::Event;
        let client = this.get_client();
        let inputs = {
            let mut guard = client.borrow_mut();
            let state = &mut *guard;
            if let Event::Removed = event {
                state.tablet.tools.remove(&tool.id());
                tool.destroy();
                return;
            }
            let Some(t) = state.tablet.tools.get_mut(&tool.id()) else { return };
            match event {
                Event::Type { tool_type: WEnum::Value(kind) } => {
                    use zwp_tablet_tool_v2::Type;
                    t.kind = match kind {
                        Type::Finger => ToolKind::Finger,
                        Type::Mouse | Type::Lens => ToolKind::Mouse,
                        _ => ToolKind::Pen,
                    };
                    return;
                }
                Event::ProximityIn { surface, .. } => {
                    t.frame.proximity_in = Some(surface);
                    return;
                }
                Event::ProximityOut => {
                    t.frame.proximity_out = true;
                    return;
                }
                Event::Down { .. } => {
                    t.frame.down = true;
                    return;
                }
                Event::Up => {
                    t.frame.up = true;
                    return;
                }
                Event::Motion { x, y } => {
                    t.frame.location = Some(point(px(x as f32), px(y as f32)));
                    return;
                }
                Event::Button { button, state: WEnum::Value(pressed), .. } => {
                    let button = match button {
                        BTN_STYLUS => MouseButton::Middle,
                        BTN_STYLUS2 => MouseButton::Right,
                        _ => return,
                    };
                    t.frame.buttons.push((button, pressed == zwp_tablet_tool_v2::ButtonState::Pressed));
                    return;
                }
                Event::Frame { .. } => {
                    let windows = &state.windows;
                    frame_inputs(t, &mut state.tablet.contacts, state.modifiers, &mut |surface| windows.get(&surface.id()).cloned())
                }
                _ => return,
            }
        };
        for (window, input) in inputs {
            window.handle_input(input);
        }
    }
}
