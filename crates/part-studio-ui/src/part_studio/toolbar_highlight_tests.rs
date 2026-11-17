//! Search tools' "Highlight tool in toolbar" never moves the toolbar (user 11-17, P1: a highlighted Sketch button
//! grew 4 px and pushed every button right of it): a highlighted Sketch, split, plain button and ⌄ keep their bounds
//! and their content where it was (live psui/toolbar-highlight: an inset ring, no border).

use super::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{Pixels, canvas};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Per button (Sketch, split, plain, ⌄): its box and its content's end (a probe after the icon / label), drawn.
type Seen = Rc<RefCell<HashMap<(usize, bool), Bounds<Pixels>>>>;

struct Row {
    lit: bool,
    seen: Seen,
}

impl Render for Row {
    fn render(&mut self, _: &mut Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        let probe = |slot: usize, content: bool, seen: &Seen| {
            let seen = seen.clone();
            canvas(|b, _, _| b, move |b, _, _, _| {
                seen.borrow_mut().insert((slot, content), b);
            })
        };
        let buttons = [
            crate::ui::text_tool_button("hl-sketch", "sketch", "Sketch", false, cx),
            crate::ui::split_tool_button("hl-fillet", "fillet", false, cx),
            crate::ui::tool_button("hl-extrude", "extrude", false, cx),
            crate::ui::tool_caret("hl-caret", cx),
        ];
        let lit = self.lit;
        gpui_kit::component::h_flex().children(buttons.into_iter().enumerate().map(|(i, b)| {
            let b = b.child(probe(i, true, &self.seen).w(px(0.)).h(px(1.))).when(lit, crate::ui::search_highlight);
            div().relative().flex_none().child(b).child(probe(i, false, &self.seen).absolute().size_full())
        }))
    }
}

#[gpui::test]
fn a_highlighted_toolbar_button_keeps_its_size_and_content(cx: &mut TestAppContext) {
    cx.update(gpui_kit::init);
    let seen: Seen = Default::default();
    let options = WindowOptions { window_bounds: Some(WindowBounds::Windowed(Bounds { origin: Point::default(), size: size(px(400.), px(100.)) })), ..Default::default() };
    let s = seen.clone();
    let (window, row) = cx.update(|cx| crate::os_menu::open_window(options, cx, |_, cx| cx.new(|_| Row { lit: false, seen: s }))).unwrap();
    draw(cx, window);
    let plain = seen.borrow().clone();
    cx.update(|cx| row.update(cx, |r, cx| {
        r.lit = true;
        cx.notify();
    }));
    draw(cx, window);
    let lit = seen.borrow().clone();
    for (i, name) in ["Sketch", "split", "plain", "⌄"].iter().enumerate() {
        assert_eq!(lit[&(i, false)], plain[&(i, false)], "{name}: the button's bounds");
        assert_eq!(lit[&(i, true)].origin, plain[&(i, true)].origin, "{name}: its content's place");
    }
}
