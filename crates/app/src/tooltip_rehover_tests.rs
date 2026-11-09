//! A tooltip under a pointer that doesn't move, as a browser's (GPUI patch in `div.rs`, `tooltip_painted_at`): a row
//! that slides up under it shows its tooltip (live docs-trash-restore), a row an overlay uncovers doesn't (live: a
//! toolbar button under a closed dialog shows none).

use gpui_kit::{AnyWindowHandle, AppContext as _, Bounds, Context, Entity, InteractiveElement as _, IntoElement, Modifiers, ParentElement as _, Point, Render, StatefulInteractiveElement as _, Styled as _, TestAppContext, VisualTestContext, Window, WindowBounds, WindowOptions, div, px, size};
use gpui_kit::prelude::FluentBuilder as _;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

struct Tip;

impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child("tip")
    }
}

/// Rows "a" and "b", 40 px each from the top, and a menu-like overlay over the first.
struct Rows {
    first: bool,
    overlay: bool,
    shown: Rc<RefCell<Vec<&'static str>>>,
}

impl Render for Rows {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let row = |name: &'static str, shown: Rc<RefCell<Vec<&'static str>>>| {
            div().id(name).h(px(40.)).w_full().tooltip(move |_, cx| {
                shown.borrow_mut().push(name);
                cx.new(|_| Tip).into()
            })
        };
        let rows = [("a", self.first), ("b", true)].into_iter().filter(|(_, on)| *on).map(|(name, _)| row(name, self.shown.clone()));
        div().relative().size_full().children(rows).when(self.overlay, |d| d.child(div().id("overlay").absolute().top_0().left_0().w(px(200.)).h(px(40.)).occlude()))
    }
}

fn host(cx: &mut TestAppContext) -> (AnyWindowHandle, Entity<Rows>, Rc<RefCell<Vec<&'static str>>>) {
    cx.update(gpui_kit::init);
    let shown: Rc<RefCell<Vec<&'static str>>> = Default::default();
    let bounds = Bounds { origin: Point::default(), size: size(px(400.), px(200.)) };
    let mut view = None;
    let s = shown.clone();
    let w: AnyWindowHandle = cx
        .update(|cx| {
            cx.open_window(WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() }, |_, cx| {
                let v = cx.new(|_| Rows { first: true, overlay: true, shown: s });
                view = Some(v.clone());
                v
            })
        })
        .unwrap()
        .into();
    cx.update_window(w, |_, window, cx| window.draw(cx).clear(cx)).unwrap();
    cx.run_until_parked();
    // The pointer comes to rest on the overlay, over the first row.
    VisualTestContext::from_window(w, cx).simulate_mouse_move(Point::new(px(20.), px(20.)), None, Modifiers::default());
    cx.executor().advance_clock(Duration::from_millis(600));
    cx.run_until_parked();
    (w, view.unwrap(), shown)
}

fn change(cx: &mut TestAppContext, w: AnyWindowHandle, view: &Entity<Rows>, f: impl FnOnce(&mut Rows)) {
    cx.update(|cx| view.update(cx, |r, cx| {
        f(r);
        cx.notify();
    }));
    cx.update_window(w, |_, window, cx| window.draw(cx).clear(cx)).unwrap();
    cx.executor().advance_clock(Duration::from_millis(600));
    cx.run_until_parked();
}

#[gpui::test]
fn a_row_sliding_under_the_resting_pointer_shows_its_tooltip(cx: &mut TestAppContext) {
    let (w, view, shown) = host(cx);
    assert!(shown.borrow().is_empty(), "nothing under the overlay shows a tooltip");
    change(cx, w, &view, |r| (r.first, r.overlay) = (false, false));
    assert_eq!(*shown.borrow(), vec!["b"], "row b moved up under the pointer: its tooltip shows without a move");
}

#[gpui::test]
fn a_row_an_overlay_uncovers_shows_no_tooltip_until_the_pointer_moves(cx: &mut TestAppContext) {
    let (w, view, shown) = host(cx);
    change(cx, w, &view, |r| r.overlay = false);
    assert!(shown.borrow().is_empty(), "row a stayed where it was: no tooltip");
    VisualTestContext::from_window(w, cx).simulate_mouse_move(Point::new(px(21.), px(20.)), None, Modifiers::default());
    cx.executor().advance_clock(Duration::from_millis(600));
    cx.run_until_parked();
    assert_eq!(*shown.borrow(), vec!["a"], "a move over it shows it");
}
