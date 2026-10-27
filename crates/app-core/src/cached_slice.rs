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
//!
//! Rules (or the slice is re-rendered on every keystroke and caret blink):
//! - No text field inside it: a field's notification re-renders the view it
//!   is in and every view around it. Put the field in the parent (or in a
//!   layer of its own) and the heavy parts beside it as slices.
//! - Inside another cached view, it is reused when that one re-renders only
//!   if both were drawn in the previous frame (gpui-pre view.rs NOSHAPE
//!   PATCH; upstream re-rendered every cached view nested in one it
//!   re-renders).
//! - A definite size: a cached view is laid out from its style alone. A
//!   floating, content-sized panel goes in an absolute full-size slice; a
//!   row of content-sized height uses [`Slice::sized`].
//! - It must be told about everything it shows: reading an entity doesn't
//!   subscribe to it ([`Slice::observing`], [`Slice::follow`]).

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::*;

/// Renders a part of `V`.
type SliceRender<V> = Rc<dyn Fn(&mut V, &mut Window, &mut Context<V>) -> AnyElement>;

pub struct Slice<V: 'static> {
    parent: WeakEntity<V>,
    render: SliceRender<V>,
    subs: Vec<Subscription>,
    /// See [`Slice::follow`].
    followed: Option<(EntityId, Subscription)>,
    /// See [`Slice::sized`].
    measure: Rc<Measure>,
}

#[derive(Default)]
struct Measure {
    /// [`Slice::sized`] is used: measure the content.
    used: Cell<bool>,
    /// The content's height when it was last laid out.
    height: Cell<Option<Pixels>>,
    /// Notified since: lay it out again.
    stale: Cell<bool>,
}

impl<V: 'static> Slice<V> {
    /// A slice of `parent` drawn by `render`, invalidated by the parent's
    /// own notifications.
    pub fn new(parent: &Entity<V>, render: impl Fn(&mut V, &mut Window, &mut Context<V>) -> AnyElement + 'static, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let sub = cx.observe(parent, Self::invalidate);
            Self { parent: parent.downgrade(), render: Rc::new(render), subs: vec![sub], followed: None, measure: Rc::default() }
        })
    }

    /// Also invalidate the slice when `other` notifies (state the slice reads
    /// from another entity, e.g. the viewport's camera).
    pub fn observing<T: 'static>(this: &Entity<Self>, other: &Entity<T>, cx: &mut App) {
        this.update(cx, |s, cx| {
            let sub = cx.observe(other, Self::invalidate);
            s.subs.push(sub);
        });
    }

    /// [`Slice::observing`] an entity that comes and goes (the field being
    /// edited): replaces the one followed before. `None` follows nothing.
    pub fn follow<T: 'static>(this: &Entity<Self>, other: Option<&Entity<T>>, cx: &mut App) {
        let id = other.map(|o| o.entity_id());
        if this.read(cx).followed.as_ref().map(|(i, _)| *i) == id {
            return;
        }
        this.update(cx, |s, cx| {
            s.followed = other.map(|o| (o.entity_id(), cx.observe(o, Self::invalidate)));
        });
    }

    fn invalidate<T>(&mut self, _: Entity<T>, cx: &mut Context<Self>) {
        self.measure.stale.set(true);
        cx.notify();
    }

    /// The slice as a row whose height is its content's (`style` gives the
    /// rest, e.g. its width). A cached view needs a definite size, so after
    /// every change it is laid out like a plain view and measured, and
    /// reused at that height until the next one: a caret blink beside it
    /// doesn't lay it out again.
    pub fn sized(this: &Entity<Self>, style: StyleRefinement, cx: &mut App) -> AnyElement {
        let m = this.read(cx).measure.clone();
        m.used.set(true);
        match m.height.get() {
            Some(h) if !m.stale.get() => this.clone().cached(style.h(h)).into_any_element(),
            _ => {
                let mut row = div();
                *row.style() = style;
                row.child(this.clone()).into_any_element()
            }
        }
    }
}

impl<V: 'static> Render for Slice<V> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let render = self.render.clone();
        let el = self.parent.update(cx, |parent, cx| render(parent, window, cx)).unwrap_or_else(|_| div().into_any_element());
        if !self.measure.used.get() {
            return el;
        }
        let m = self.measure.clone();
        let probe = canvas(
            move |b, _, _| {
                m.height.set(Some(b.size.height));
                m.stale.set(false);
            },
            |_, _, _, _| {},
        );
        div().w_full().relative().child(el).child(probe.absolute().top_0().left_0().size_full()).into_any_element()
    }
}
