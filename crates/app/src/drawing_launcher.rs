//! Drawing editor: the dimension palette's launcher (live behaviours "ui2
//! (10-24 15:25) dimension launcher", drawing/ln1–ln14). A 32 px "±0.1"
//! button beside a selected dimension: it shows 0.8 s after the dimension
//! is selected (also right after placing one), never on hover alone, its
//! bottom-left 14 px right of and 14 px above the text's ink top-right.
//! Resting on it for 0.6 s, or a click, opens the dimension palette at
//! its top-left + (-15, +9); leaving that palette closes it and deselects.

use crate::drawing_view::*;
use gpui_kit::*;
use std::time::{Duration, Instant};

/// Live: present 776 ms after the placing click (absent at 13 ms).
const SHOW_AFTER: Duration = Duration::from_millis(800);
/// Live: the palette opens 0.6 s after the pointer comes onto the launcher.
const OPEN_AFTER: Duration = Duration::from_millis(600);
const SIZE: f32 = 32.;

/// The launcher's state: the dimension it is for, since when, and the
/// pending hover-to-open.
#[derive(Default)]
pub struct Launcher {
    target: Option<(u64, Instant)>,
    hover: Option<Task<()>>,
    /// The palette was opened from the launcher (leaving it closes it).
    pub opened: bool,
}

impl DrawingEditor {
    /// The one selected dimension (no palette open).
    fn launcher_dimension(&self) -> Option<u64> {
        if self.palette.is_some() {
            return None;
        }
        let [Sel::Annotation(id)] = self.selection.as_slice() else { return None };
        self.current_sheet().annotations.iter().find(|a| a.id == *id && a.kind.is_dimension()).map(|a| a.id)
    }

    /// The launcher's top-left (editor px): its bottom-left 14 px right of
    /// and 14 px above the top-right of the dimension text's ink.
    fn launcher_at(&self, id: u64) -> Option<[f32; 2]> {
        let b = self.built.as_ref()?;
        let picks = self.picks_of(Sel::Annotation(id));
        let (mut x, mut y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for t in b.geometry.texts.iter().filter(|t| picks.contains(&t.pick)) {
            // `text_box` pads the ink by 0.15 of the height each side.
            let m = t.height * 0.15;
            for c in noshape_model::drawing::font::text_box(t) {
                x = x.max(c[0] - m);
                y = y.max(c[1] - m);
            }
        }
        if !x.is_finite() {
            return None;
        }
        let s = self.size_px();
        let px_x = (x - self.center[0]) * self.zoom + s[0] / 2.0;
        let px_y = (self.center[1] - y) * self.zoom + s[1] / 2.0;
        Some([(px_x + 14.0).round() as f32, (px_y - 14.0).round() as f32 - SIZE])
    }

    pub fn render_dim_launcher(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let Some(id) = self.launcher_dimension() else {
            self.launcher.target = None;
            self.launcher.hover = None;
            return None;
        };
        let since = match self.launcher.target {
            Some((t, at)) if t == id => at,
            _ => {
                let now = Instant::now();
                self.launcher.target = Some((id, now));
                self.launcher.hover = None;
                // Show it once its delay is up.
                cx.spawn_in(window, async move |this, cx| {
                    cx.background_executor().timer(SHOW_AFTER).await;
                    let _ = this.update(cx, |_, cx| cx.notify());
                })
                .detach();
                now
            }
        };
        if since.elapsed() < SHOW_AFTER {
            return None;
        }
        let at = self.launcher_at(id)?;
        let open = move |this: &mut Self, window: &mut Window, cx: &mut Context<Self>| {
            this.open_dim_palette(id, window, cx);
            this.launcher.opened = true;
            if let Some(p) = &mut this.palette {
                p.set_at([at[0] - 15.0, at[1] + 9.0]);
            }
            cx.notify();
        };
        Some(
            div()
                .id("dr-dim-launcher")
                .absolute()
                .left(px(at[0]))
                .top(px(at[1]))
                .size(px(SIZE))
                .occlude()
                .child(crate::assets::icon("dim-palette-launcher", SIZE, cx))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.launcher.hover = None;
                    open(this, window, cx)
                }))
                .on_hover(cx.listener(move |this, hovered: &bool, window, cx| {
                    if !*hovered {
                        this.launcher.hover = None;
                        return;
                    }
                    this.launcher.hover = Some(cx.spawn_in(window, async move |this, cx| {
                        cx.background_executor().timer(OPEN_AFTER).await;
                        let _ = this.update_in(cx, |this, window, cx| {
                            if this.launcher.hover.is_some() {
                                open(this, window, cx);
                            }
                        });
                    }));
                }))
                .into_any_element(),
        )
    }

    /// The pointer left a palette the launcher opened: it closes (keeping
    /// its changes) and the dimension is deselected (live).
    pub fn launcher_palette_left(&mut self, cx: &mut Context<Self>) {
        if !std::mem::take(&mut self.launcher.opened) {
            return;
        }
        self.close_palette(true, cx);
        self.selection.clear();
        cx.notify();
    }
}
