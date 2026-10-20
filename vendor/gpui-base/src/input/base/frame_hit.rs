//! NOSHAPE PATCH: the whole input box is the hit target, as in browsers.
//!
//! A browser text field takes a press anywhere inside its border, padding
//! included: it focuses, places the caret at the nearest position, and shows
//! the I-beam over all of it. Here the frame ([`super::base::InputBase`])
//! holds the border and the padding while the text element inside it handles
//! the mouse, so the padding was dead.
//!
//! Frames publish their box while their children prepaint ([`InputFrame`]
//! around the kit's frame, and around an app's own box around an input). The
//! text element's [`FrameHitArea`] claims every frame around it, giving its
//! own bounds and its press handler (an outer frame skips a press an inner
//! one already took). Each claimed frame
//! then inserts a hitbox over itself after its children (in its own content
//! mask: the text element clips its overflow, and hitboxes are clipped to
//! the mask they are made in), takes presses there outside the text element,
//! and asks for the I-beam before its children paint, so a control on it (a
//! clear button) keeps its own cursor. Drags and the release are already
//! window-wide listeners of the text element.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, CursorStyle, DispatchPhase, Element, ElementId, Entity,
    GlobalElementId, Hitbox, HitboxBehavior, IntoElement, LayoutId, MouseButton, MouseDownEvent,
    MouseUpEvent, Pixels, Position, Style, Window, px,
};

use super::{kind::InputModeKind, state::InputBaseState};

/// What the text element hands its frame.
#[derive(Clone)]
struct Claim {
    /// The text element's bounds: presses there are its own.
    inner: Bounds<Pixels>,
    press: Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App)>,
    /// The right button's release (the input's context menu opens on it).
    release: Rc<dyn Fn(&MouseUpEvent, &mut Window, &mut App)>,
    /// The input has the focus.
    focused: Rc<dyn Fn(&Window, &App) -> bool>,
}

struct Frame {
    bounds: Bounds<Pixels>,
    claim: Option<Claim>,
}

thread_local! {
    /// The input frames being prepainted, outermost first.
    static FRAMES: RefCell<Vec<Frame>> = const { RefCell::new(Vec::new()) };
}

fn push_frame(bounds: Bounds<Pixels>) {
    FRAMES.with(|frames| frames.borrow_mut().push(Frame { bounds, claim: None }));
}

/// Pops the frame and, when an input claimed it, covers it with a hitbox.
fn pop_frame(window: &mut Window) -> Option<(Hitbox, Claim)> {
    let frame = FRAMES.with(|frames| frames.borrow_mut().pop())?;
    let claim = frame.claim?;
    Some((window.insert_hitbox(frame.bounds, HitboxBehavior::Normal), claim))
}

/// The I-beam over the frame and its presses outside the text element.
fn paint_frame(hit: &(Hitbox, Claim), window: &mut Window) {
    let (hitbox, claim) = hit.clone();
    window.set_cursor_style(CursorStyle::IBeam, &hitbox);
    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
        if phase != DispatchPhase::Bubble
            || !matches!(event.button, MouseButton::Left | MouseButton::Right)
            || claim.inner.contains(&event.position)
            || !hitbox.is_hovered(window)
        {
            return;
        }
        // A frame inside this one (the kit's frame in an app's box) already
        // took the press: it marked it and gave the input the focus.
        if window.default_prevented() && (claim.focused)(window, cx) {
            return;
        }
        // Only the frame's own content ran before (bubbling goes inside
        // out): the kit frame focusing its own handle, a control on it. The
        // press is the input's: it takes the focus back and places the caret.
        (claim.press)(event, window, cx);
        // As the text element's own press does: the frame and the views
        // around it (which focus their own handles on a press) keep off.
        window.prevent_default();
    });
    let release = hit.1.release.clone();
    let (hitbox, inner) = (hit.0.clone(), hit.1.inner);
    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
        if phase == DispatchPhase::Bubble && event.button == MouseButton::Right && !inner.contains(&event.position) && hitbox.is_hovered(window) {
            release(event, window, cx);
        }
    });
}

/// A box pinned to all sides of the parent (the text element's bounds).
fn overlay_style() -> Style {
    let mut style = Style::default();
    style.position = Position::Absolute;
    style.inset.left = px(0.).into();
    style.inset.top = px(0.).into();
    style.inset.right = px(0.).into();
    style.inset.bottom = px(0.).into();
    style
}

/// Child of the input state element: claims the frames around it for this
/// input.
pub(crate) struct FrameHitArea<M: InputModeKind> {
    state: Entity<InputBaseState<M>>,
    disabled: bool,
}

impl<M: InputModeKind> FrameHitArea<M> {
    pub(crate) fn new(state: Entity<InputBaseState<M>>, disabled: bool) -> Self {
        Self { state, disabled }
    }
}

impl<M: InputModeKind> IntoElement for FrameHitArea<M> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl<M: InputModeKind> Element for FrameHitArea<M> {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        (window.request_layout(overlay_style(), [], cx), ())
    }

    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), _: &mut Window, _: &mut App) {
        if self.disabled {
            return;
        }
        let state = self.state.clone();
        let press: Rc<dyn Fn(&MouseDownEvent, &mut Window, &mut App)> = Rc::new(move |event, window, cx| {
            state.update(cx, |state, cx| {
                state.focus(window, cx);
                state.press(event, window, cx);
            });
        });
        let state = self.state.clone();
        let release: Rc<dyn Fn(&MouseUpEvent, &mut Window, &mut App)> = Rc::new(move |event, window, cx| {
            state.update(cx, |state, cx| state.on_mouse_up(event, window, cx));
        });
        let state = self.state.clone();
        let focused: Rc<dyn Fn(&Window, &App) -> bool> = Rc::new(move |window, cx| state.read(cx).focus_handle.is_focused(window));
        FRAMES.with(|frames| {
            // Every frame around the text element: an app's box and the kit's
            // frame inside it. The kit frame's own focus tracking takes a
            // press in its padding (focusing the frame, not the input) before
            // an outer frame's handler runs; its own claim takes it back.
            for frame in frames.borrow_mut().iter_mut().filter(|f| f.bounds.contains(&bounds.origin)) {
                frame.claim = Some(Claim { inner: bounds, press: press.clone(), release: release.clone(), focused: focused.clone() });
            }
        });
    }

    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>, _: Bounds<Pixels>, _: &mut (), _: &mut (), _: &mut Window, _: &mut App) {}
}

/// An app's own box around one input (its border and padding drawn outside
/// the kit's frame, e.g. an `Input` with `appearance(false)`): the whole box
/// becomes the input's hit target and shows the I-beam.
pub struct InputFrame {
    child: AnyElement,
}

/// Wraps the box drawn around one input; see [`InputFrame`].
pub fn input_frame(child: impl IntoElement) -> InputFrame {
    InputFrame { child: child.into_any_element() }
}

impl IntoElement for InputFrame {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// An [`InputFrame`]'s hitbox and claim between prepaint and paint.
pub struct InputFrameHit(Option<(Hitbox, Claim)>);

impl Element for InputFrame {
    type RequestLayoutState = ();
    type PrepaintState = InputFrameHit;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) -> Self::PrepaintState {
        push_frame(bounds);
        self.child.prepaint(window, cx);
        InputFrameHit(pop_frame(window))
    }

    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>, _: Bounds<Pixels>, _: &mut (), prepaint: &mut Self::PrepaintState, window: &mut Window, cx: &mut App) {
        if let Some(hit) = prepaint.0.take() {
            paint_frame(&hit, window);
        }
        self.child.paint(window, cx);
    }
}
