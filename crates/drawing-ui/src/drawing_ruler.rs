//! Drawing editor: the note editor's formatting ruler (Onshape's ruler at
//! the top of the note box): the tab stop alignment selector (Left /
//! Center / Right / Decimal; a click cycles), ticks along the box width,
//! tab stops (click the ruler to add one, drag to move it, drag it up or
//! down off the ruler to remove it), the first line indent (top marker),
//! the left and right indents (bottom markers) and the bounding box width
//! marker at the right end.

use crate::drawing_view::*;
use gpui_kit::component::h_flex;
use gpui_kit::*;
use noshape_model::drawing::note::{NoteStyle, TabKind, TabStop};
use std::cell::Cell;
use std::rc::Rc;

/// Ruler width (px) and how far a tab stop must be dragged off it to go.
const RULER_W: f32 = 390.;
/// The ruler bar's grey (live2 drw-tools-1119 rad-note 30: rgb(183,183,183)).
const BAR: Rgba = Rgba { r: 183. / 255., g: 183. / 255., b: 183. / 255., a: 1. };
const OFF: f32 = 18.;

/// What a ruler drag moves.
#[derive(Clone, Copy, PartialEq)]
enum Handle {
    Tab(usize),
    First,
    Left,
    Right,
    Width,
}

/// Ruler state: the selector's alignment, the drag in progress and the
/// ruler's bounds (for mouse positions).
pub struct RulerState {
    pub kind: TabKind,
    drag: Option<(Handle, bool)>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    /// px per mm when the ruler sits on the sheet over the note (the
    /// sheet's zoom; live 225 / 414), else the ruler fits [`RULER_W`].
    pub sheet_k: Option<f32>,
}

impl Default for RulerState {
    fn default() -> Self {
        Self { kind: TabKind::Left, drag: None, bounds: Rc::new(Cell::new(Bounds::default())), sheet_k: None }
    }
}

/// On the sheet: the grey part right of the box, with the ◁▷ width handle.
const TAIL: f32 = 40.;

/// The selector's glyph for an alignment.
fn glyph(k: TabKind) -> &'static str {
    match k {
        TabKind::Left => "⌊",
        TabKind::Center => "⊥",
        TabKind::Right => "⌋",
        TabKind::Decimal => "⫰",
    }
}

impl DrawingEditor {
    /// Millimeters shown on the ruler and px per mm for a note style
    /// (`text_w`: the laid-out box width).
    fn ruler_scale(st: &NoteStyle, text_w: f64, sheet_k: Option<f32>) -> (f64, f32) {
        if let Some(k) = sheet_k {
            return (st.width.unwrap_or(text_w).max(1.0), k);
        }
        let w = st.width.unwrap_or(text_w).max(10.0);
        let shown = w * 1.15 + 5.0;
        (w, RULER_W / shown as f32)
    }

    /// Apply a ruler drag at window x / y (px).
    fn ruler_drag_to(&mut self, x: Pixels, y: Pixels, text_w: f64, cx: &mut Context<Self>) {
        let Some((h, _)) = self.ruler.drag else { return };
        let b = self.ruler.bounds.get();
        let sheet_k = self.ruler.sheet_k;
        let Some(Panel::Note { style, .. }) = &mut self.panel else { return };
        let (w, k) = Self::ruler_scale(style, text_w, sheet_k);
        let mm = (f32::from(x - b.origin.x) / k).max(0.0) as f64;
        let off = f32::from(y - b.origin.y) < -OFF || f32::from(y - b.origin.y) > f32::from(b.size.height) + OFF;
        match h {
            Handle::Tab(i) => {
                if let Some(t) = style.tabs.get_mut(i) {
                    t.pos = mm.min(w);
                }
            }
            Handle::First => style.first_indent = mm.min(w - 1.0),
            Handle::Left => style.left_indent = mm.min(w - 1.0),
            Handle::Right => style.right_indent = (w - mm).clamp(0.0, w - 1.0),
            Handle::Width => style.width = Some(mm.max(5.0)),
        }
        self.ruler.drag = Some((h, off && matches!(h, Handle::Tab(_))));
        cx.notify();
    }

    fn ruler_release(&mut self, cx: &mut Context<Self>) {
        if let Some((Handle::Tab(i), true)) = self.ruler.drag
            && let Some(Panel::Note { style, .. }) = &mut self.panel
            && i < style.tabs.len()
        {
            style.tabs.remove(i);
        }
        self.ruler.drag = None;
        cx.notify();
    }

    /// Mouse down on the ruler: grab the nearest marker, or add a tab stop
    /// of the selector's alignment there.
    fn ruler_press(&mut self, x: Pixels, y: Pixels, text_w: f64, cx: &mut Context<Self>) {
        let b = self.ruler.bounds.get();
        let kind = self.ruler.kind;
        let sheet_k = self.ruler.sheet_k;
        let Some(Panel::Note { style, .. }) = &mut self.panel else { return };
        let (w, k) = Self::ruler_scale(style, text_w, sheet_k);
        let px_of = |mm: f64| mm as f32 * k;
        let (lx, ly) = (f32::from(x - b.origin.x), f32::from(y - b.origin.y));
        let top = ly < f32::from(b.size.height) / 2.0;
        let near = |mm: f64| (px_of(mm) - lx).abs() < 5.0;
        let handle = if near(w) {
            Some(Handle::Width)
        } else if let Some(i) = style.tabs.iter().position(|t| near(t.pos)) {
            Some(Handle::Tab(i))
        } else if top && near(style.first_indent) {
            Some(Handle::First)
        } else if !top && near(style.left_indent) {
            Some(Handle::Left)
        } else if !top && near(w - style.right_indent) {
            Some(Handle::Right)
        } else {
            None
        };
        let handle = handle.unwrap_or_else(|| {
            style.tabs.push(TabStop { pos: ((lx / k) as f64).clamp(0.0, w), kind });
            Handle::Tab(style.tabs.len() - 1)
        });
        self.ruler.drag = Some((handle, false));
        cx.notify();
    }

    /// The ruler row of the note editor.
    pub fn note_ruler(&mut self, st: &NoteStyle, text_w: f64, cx: &mut Context<Self>) -> AnyElement {
        let (w, k) = Self::ruler_scale(st, text_w, self.ruler.sheet_k);
        let ruler_w = if self.ruler.sheet_k.is_some() { w as f32 * k + TAIL } else { RULER_W };
        let ink = gpui_kit::rgb(0x333333);
        let px_of = |mm: f64| px(mm as f32 * k);
        let dragging_off = matches!(self.ruler.drag, Some((Handle::Tab(_), true)));
        let dragged = match self.ruler.drag {
            Some((Handle::Tab(i), _)) => Some(i),
            _ => None,
        };
        // Live2 drw-tools-1119 rad-note 30, recheck-1119: over the box's width a white band framed #5c5c5c then
        // #808080 inside the ruler's grey bar, the grey tail past it.
        let band_w = px_of(w).min(px(ruler_w));
        let mut ruler = div().id("note-ruler").relative().w(px(ruler_w)).h(px(20.)).bg(BAR).overflow_hidden();
        ruler = ruler.child(div().absolute().top_0().left_0().h(px(20.)).w(band_w).border_1().border_color(gpui_kit::rgb(0x5c5c5c)).child(div().size_full().border_1().border_color(gpui_kit::rgb(0x808080)).bg(gpui_kit::rgb(0xffffff))));
        // Ticks every 1/8 in, 4 px, and every 1/2 in, 10 px, centred, whatever the drawing's units (live2
        // recheck-1119: 13 / 53 px apart on a mm drawing).
        let step = 25.4 / 8.0;
        let mut n = 1;
        while px_of(n as f64 * step) < band_w - px(2.) {
            let tall = n % 4 == 0;
            if tall || step as f32 * k >= 4.0 {
                let h = if tall { 10. } else { 4. };
                ruler = ruler.child(div().absolute().left(px_of(n as f64 * step)).top(px((20. - h) / 2.)).w(px(1.)).h(px(h)).bg(gpui_kit::rgb(0x404040)));
            }
            n += 1;
        }
        // Indents: first line (top), left and right (bottom); the box width.
        let marker = |x: Pixels, top: bool, glyph: &'static str| {
            // Kept inside the ruler: at an indent of 0 the markers sit at the band's ends (live2 recheck-1119).
            let d = div().absolute().left((x - px(4.)).max(px(0.)).min(px(ruler_w - 9.))).w(px(9.)).h(px(10.)).flex().justify_center().text_size(px(10.)).text_color(gpui_kit::rgb(0x5c5c5c)).child(glyph);
            if top { d.top(px(-1.)) } else { d.bottom(px(-1.)) }
        };
        ruler = ruler
            .child(marker(px_of(st.first_indent), true, "▼"))
            .child(marker(px_of(st.left_indent), false, "▲"))
            .child(marker(px_of(w - st.right_indent), false, "▲"));
        // Live 414: the box-width handle in the grey tail, two small filled triangles ◁ ▷ (no blue bar: live2 rad-note
        // 30; drawn, not a font's outlined ◁▷: live2 recheck-1119).
        let handle = canvas(
            |_, _, _| {},
            |b, _, window, _| {
                let at = |x: f32, y: f32| point(b.origin.x + px(x), b.origin.y + px(y));
                for [tip, base] in [[0., 4.], [10., 6.]] {
                    let mut pb = PathBuilder::fill();
                    pb.move_to(at(base, 0.));
                    pb.line_to(at(tip, 3.5));
                    pb.line_to(at(base, 7.));
                    pb.close();
                    if let Ok(path) = pb.build() {
                        window.paint_path(path, gpui_kit::rgb(0x2e2e2e));
                    }
                }
            },
        )
        .w(px(10.))
        .h(px(7.));
        ruler = ruler.child(div().absolute().left(px_of(w) + px(4.)).top_0().bottom_0().flex().items_center().child(handle));
        for (i, t) in st.tabs.iter().enumerate() {
            if dragging_off && dragged == Some(i) {
                continue;
            }
            ruler = ruler.child(div().absolute().left(px_of(t.pos) - px(4.)).top(px(5.)).w(px(9.)).flex().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(ink).child(glyph(t.kind)));
        }
        // Bounds, and the drag's mouse moves / release anywhere in the window.
        let bounds = self.ruler.bounds.clone();
        let ed = cx.entity();
        let drag_on = self.ruler.drag.is_some();
        ruler = ruler.child(
            canvas(
                move |b, _, _| bounds.set(b),
                move |_, _, window, _| {
                    if !drag_on {
                        return;
                    }
                    let e1 = ed.clone();
                    window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                        if phase == DispatchPhase::Bubble {
                            e1.update(cx, |t, cx| t.ruler_drag_to(e.position.x, e.position.y, text_w, cx));
                        }
                    });
                    let e2 = ed.clone();
                    window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                        if phase == DispatchPhase::Bubble {
                            e2.update(cx, |t, cx| t.ruler_release(cx));
                        }
                    });
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        );
        let ruler = ruler.on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                this.ruler_press(e.position.x, e.position.y, text_w, cx);
            }),
        );
        let kind = self.ruler.kind;
        // One grey bar: the tab stop selector (white, black frame) inside its left end, then the ruler (live2
        // drw-tools-1119 rad-note 30).
        h_flex()
            .p(px(3.))
            .gap(px(3.))
            .bg(BAR)
            .border_1()
            .border_color(gpui_kit::rgb(0x5c5c5c))
            .child(
                div()
                    .id("note-tab-kind")
                    .size(px(20.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(gpui_kit::rgb(0xffffff))
                    .border_1()
                    .border_color(gpui_kit::rgb(0x000000))
                    .text_size(px(12.))
                    .font_weight(FontWeight::BOLD)
                    .cursor_pointer()
                    .tooltip(crate::ui::tooltip(format!("Tab stop alignment: {}", kind.name())))
                    .child(glyph(kind))
                    // Not a click on the sheet: the note stays in edit (qa
                    // DWG-RULER-SELECTOR-CLOSES; live cycles it in place).
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.ruler.kind = this.ruler.kind.next();
                        cx.notify();
                    })),
            )
            .child(ruler)
            .into_any_element()
    }
}
