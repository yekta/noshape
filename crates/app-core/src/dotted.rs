//! A dotted outline as the browser paints CSS `border: <w>px dotted`: square dots `w` px across, `w` px apart, on all
//! four sides (live2 config.json, a configured dialog field: 2 px dots of #fab601 every 4 px). GPUI's borders are solid
//! or dashed only, so the dots are painted as quads over the element.

use gpui_kit::*;

/// The dots of a `w` px dotted outline just inside `b`: the top and bottom rows left to right, then the left and right
/// columns between them, each dot `w` square, `w` apart.
pub fn dot_rects(b: Bounds<Pixels>, w: Pixels) -> Vec<Bounds<Pixels>> {
    let mut out = Vec::new();
    if w <= px(0.) || b.size.width < w || b.size.height < w {
        return out;
    }
    let step = w * 2.;
    let dot = size(w, w);
    let mut x = b.left();
    while x + w <= b.right() + px(0.01) {
        out.push(Bounds::new(point(x, b.top()), dot));
        out.push(Bounds::new(point(x, b.bottom() - w), dot));
        x += step;
    }
    let mut y = b.top() + step;
    while y + w <= b.bottom() - w + px(0.01) {
        out.push(Bounds::new(point(b.left(), y), dot));
        out.push(Bounds::new(point(b.right() - w, y), dot));
        y += step;
    }
    out
}

/// The outline over its parent (which must be `relative()`): dots `w` px of `color`.
pub fn dotted_outline(color: impl Into<Hsla>, w: f32) -> impl IntoElement {
    let color = color.into();
    canvas(|_, _, _| {}, move |b, _, window, _| {
        for r in dot_rects(b, px(w)) {
            window.paint_quad(fill(r, color));
        }
    })
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live2's configured field (configured-field.png): 2 px dots every 4 px along each side, corners included.
    #[::core::prelude::v1::test]
    fn dots_are_their_width_apart_on_every_side() {
        let b = Bounds::new(point(px(0.), px(0.)), size(px(12.), px(10.)));
        let r = dot_rects(b, px(2.));
        let at = |x: f32, y: f32| r.iter().any(|d| d.origin == point(px(x), px(y)) && d.size == size(px(2.), px(2.)));
        for x in [0., 4., 8.] {
            assert!(at(x, 0.) && at(x, 8.), "top and bottom at x {x}");
        }
        assert!(!at(2., 0.) && !at(6., 0.), "gaps between");
        assert!(at(0., 4.) && at(10., 4.), "the sides between the rows");
        assert!(r.iter().all(|d| d.right() <= b.right() && d.bottom() <= b.bottom()), "inside the bounds");
        assert!(dot_rects(Bounds::new(point(px(0.), px(0.)), size(px(1.), px(1.))), px(2.)).is_empty());
    }
}
