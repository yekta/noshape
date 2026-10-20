//! A part of a view rendered as its own cached child view.
//!
//! GPUI re-renders, re-lays-out and re-paints a view and all its ancestors
//! when anything inside notifies: a keystroke or a caret blink in a text field
//! redrew the whole Part Studio (toolbar, overlays, view cube...). A
//! [`Slice`] renders one part of its parent through a plain function and is
//! inserted with [`Entity::cached`], so GPUI reuses its last layout and paint
//! unless the slice is notified. It is notified whenever its parent calls
//! `cx.notify()` (and whenever any other entity it reads notifies, see
//! [`Slice::observing`]); a child's notification that only marks the parent
//! dirty leaves it cached.

use std::rc::Rc;

use gpui_kit::*;

/// Renders a part of `V`.
pub type SliceRender<V> = fn(&mut V, &mut Window, &mut Context<V>) -> AnyElement;

/// [`SliceRender`] as a shared closure (it may capture, e.g. a row's key).
type SliceRenderFn<V> = Rc<dyn Fn(&mut V, &mut Window, &mut Context<V>) -> AnyElement>;

pub struct Slice<V: 'static> {
    parent: WeakEntity<V>,
    render: SliceRenderFn<V>,
    subs: Vec<Subscription>,
}

impl<V: 'static> Slice<V> {
    /// A slice of `parent` drawn by `render`, invalidated by the parent's
    /// own notifications.
    pub fn new(parent: &Entity<V>, render: SliceRender<V>, cx: &mut App) -> Entity<Self> {
        Self::with(parent, render, cx)
    }

    /// Like [`Slice::new`] with a closure, e.g. one row of a list keyed by
    /// what it captures.
    pub fn with(parent: &Entity<V>, render: impl Fn(&mut V, &mut Window, &mut Context<V>) -> AnyElement + 'static, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let sub = cx.observe(parent, |_, _, cx| cx.notify());
            Self { parent: parent.downgrade(), render: Rc::new(render), subs: vec![sub] }
        })
    }

    /// Also invalidate the slice when `other` notifies (state the slice reads
    /// from another entity, e.g. the viewport's camera).
    pub fn observing<T: 'static>(this: &Entity<Self>, other: &Entity<T>, cx: &mut App) {
        this.update(cx, |s, cx| {
            let sub = cx.observe(other, |_, _, cx| cx.notify());
            s.subs.push(sub);
        });
    }
}

impl<V: 'static> Render for Slice<V> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let render = self.render.clone();
        self.parent.update(cx, |parent, cx| render(parent, window, cx)).unwrap_or_else(|_| div().into_any_element())
    }
}
