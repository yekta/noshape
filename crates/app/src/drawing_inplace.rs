//! Drawing editor: notes edited in place (live drawing/224–227, behaviours.md
//! "Notes: alignment, rotation, menus"): the text is typed on the sheet at the
//! note, in its font, size and line spacing, inside a dashed box with the
//! formatting ruler above it; the floating Note panel sits just above that.
//! The note's own sheet text is hidden meanwhile (`DrawingEditor.inplace_hidden`).
//! General table cells are edited the same way (live "General table text
//! entry"): the caret in the cell, Tab / Shift+Tab to the next / previous
//! cell, Enter to the cell below, Alt+Enter a new line, Enter in the last
//! cell ends.

use crate::drawing_view::*;
use gpui_kit::component::input::Textarea;
use gpui_kit::*;
use noshape_model::drawing::note;
use noshape_model::drawing::TableKind;

/// Em metrics of the drawing fonts: cap height, ascent, descent (their hhea
/// tables), so the editor draws the same cap height on the same baseline.
fn metrics(family: &str) -> (f64, f64, f64) {
    let f = family.to_ascii_lowercase();
    if f.contains("noto") {
        (0.714, 1.069, 0.293)
    } else if f.contains("serif") && !f.contains("sans") {
        (0.655, 0.891, 0.216)
    } else if f.contains("mono") {
        (0.659, 0.833, 0.300)
    } else {
        (0.688, 0.905, 0.212)
    }
}

/// Where the in-place editor goes (canvas pixels) and its type metrics.
pub struct InplaceBox {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
    pub font_px: f64,
    pub pitch_px: f64,
    pub family: String,
    pub bold: bool,
    pub italic: bool,
    /// Top of the editor's first line box (its baseline on the note's).
    pub text_top: f64,
}

/// Height (px) of the ruler above the edited text.
const RULER_H: f64 = 24.0;

impl DrawingEditor {
    /// Sheet millimeters to canvas pixels (inverse of `to_sheet`).
    pub fn sheet_to_local(&self, p: [f64; 2]) -> [f64; 2] {
        let s = self.size_px();
        [(p[0] - self.center[0]) * self.zoom + s[0] / 2.0, -(p[1] - self.center[1]) * self.zoom + s[1] / 2.0]
    }

    /// The open note editor's place on the canvas.
    pub fn note_inplace_box(&self, cx: &App) -> Option<InplaceBox> {
        let Some(Panel::Note { input, pos, height, style, .. }) = &self.panel else { return None };
        let h = height.read(cx).value().trim().trim_end_matches("mm").trim().parse::<f64>().ok().filter(|v| *v > 0.1).unwrap_or(3.5);
        let text = input.read(cx).value().to_string();
        let shown = if text.trim().is_empty() { "H".to_string() } else { note::fractions(&text) };
        let l = note::layout(&shown, h, style, *pos);
        // Baseline to baseline (line spacing, paragraph spacing aside).
        let two = note::layout("H\nH", h, style, *pos);
        let pitch_mm = match (two.runs.first(), two.runs.get(1)) {
            (Some(a), Some(b)) if (a.0[1] - b.0[1]).abs() > 1e-9 => (a.0[1] - b.0[1]).abs(),
            _ => h * 1.6,
        };
        let (family, bold, italic) = match &style.font {
            Some(f) => (f.family.clone(), f.bold, f.italic),
            None => (noshape_sketch::text::DEFAULT_FONT.to_string(), false, false),
        };
        let (cap, asc, desc) = metrics(&family);
        let font_px = h * self.zoom / cap;
        let pitch_px = pitch_mm * self.zoom;
        let tl = self.sheet_to_local([l.bbox[0], l.bbox[3]]);
        let br = self.sheet_to_local([l.bbox[2], l.bbox[1]]);
        let base = l.runs.first().map(|r| self.sheet_to_local(r.0)[1]).unwrap_or(br[1]);
        // A text line centres the font's ascent + descent in its line height.
        let text_top = base - ((pitch_px - (asc + desc) * font_px) / 2.0 + asc * font_px);
        Some(InplaceBox { left: tl[0], top: tl[1], width: (br[0] - tl[0]).max(font_px), height: (br[1] - tl[1]).max(pitch_px), font_px, pitch_px, family, bold, italic, text_top })
    }

    /// The note's text editor on the sheet: the ruler, then the text in a
    /// dashed box.
    pub fn render_note_inplace(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let b = self.note_inplace_box(cx)?;
        let Some(Panel::Note { input, style, .. }) = &self.panel else { return None };
        let (input, style) = (input.clone(), style.clone());
        let lines = input.read(cx).value().split('\n').count().max(1);
        let pad = 3.0;
        let ruler = self.note_ruler(&style, b.width / self.zoom, cx);
        let ink = match style.color {
            Some([r, g, bl]) => gpui_kit::rgb(((r as u32) << 16) | ((g as u32) << 8) | bl as u32),
            None => gpui_kit::rgb(0x000000),
        };
        let mut text = Textarea::new(&input)
            .appearance(false)
            .p_0()
            .w(px((b.width + b.font_px * 2.0) as f32))
            .h(px((b.pitch_px * lines as f64 + 2.0) as f32))
            .text_size(px(b.font_px as f32))
            .line_height(px(b.pitch_px as f32))
            .font_family(SharedString::from(b.family.clone()))
            .text_color(ink);
        if b.bold {
            text = text.font_weight(FontWeight::BOLD);
        }
        if b.italic {
            text = text.italic();
        }
        let top = b.text_top.min(b.top);
        Some(
            div()
                .id("dr-note-inplace")
                .absolute()
                .left(px((b.left - pad) as f32))
                .top(px((top - pad - RULER_H) as f32))
                .flex()
                .flex_col()
                // Clicks in the editor stay in it (a click away commits).
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(div().h(px(RULER_H as f32)).child(ruler))
                .child(div().p(px(pad as f32)).border_1().border_dashed().border_color(gpui_kit::rgb(0x8a8a8a)).child(text))
                .into_any_element(),
        )
    }

    /// The general table cell being edited (`Panel::Cell`) on the canvas.
    pub fn cell_inplace_box(&self) -> Option<InplaceBox> {
        let Some(Panel::Cell { table, row, col, .. }) = &self.panel else { return None };
        let t = self.current_sheet().table(*table)?;
        if !matches!(t.kind, TableKind::General { .. }) {
            return None;
        }
        let l = self.built.as_ref()?.table_layout(*table)?;
        let r = row + l.title as usize;
        let (x0, x1) = (*l.xs.get(*col)?, *l.xs.get(col + 1)?);
        let (ya, yb) = (*l.ys.get(r)?, *l.ys.get(r + 1)?);
        let f = t.cell_format(*row, *col);
        let h = f.height.filter(|v| *v > 0.0).unwrap_or(t.text_height);
        let family = noshape_sketch::text::DEFAULT_FONT.to_string();
        let (cap, asc, desc) = metrics(&family);
        let font_px = h * self.zoom / cap;
        let pitch_px = h * 1.6 * self.zoom;
        let tl = self.sheet_to_local([x0, ya]);
        let br = self.sheet_to_local([x1, yb]);
        // One line centred in the cell.
        let mid = (tl[1] + br[1]) / 2.0;
        let base = mid + h * self.zoom / 2.0;
        let text_top = base - ((pitch_px - (asc + desc) * font_px) / 2.0 + asc * font_px);
        Some(InplaceBox { left: tl[0], top: tl[1], width: br[0] - tl[0], height: br[1] - tl[1], font_px, pitch_px, family, bold: f.bold, italic: false, text_top })
    }

    /// The cell's text editor: the caret in the cell, a blue cell outline
    /// with a dashed text box (live 740–757).
    pub fn render_cell_inplace(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let b = self.cell_inplace_box()?;
        let Some(Panel::Cell { input, .. }) = &self.panel else { return None };
        let pad = 2.0;
        let mut text = Textarea::new(input)
            .appearance(false)
            .p_0()
            .w(px((b.width - 2.0 * pad).max(b.font_px) as f32))
            .h(px(b.pitch_px.max(b.height - 2.0 * pad) as f32))
            .text_size(px(b.font_px as f32))
            .line_height(px(b.pitch_px as f32))
            .font_family(SharedString::from(b.family.clone()))
            .text_color(gpui_kit::rgb(0x000000));
        if b.bold {
            text = text.font_weight(FontWeight::BOLD);
        }
        Some(
            div()
                .id("dr-cell-inplace")
                .absolute()
                .left(px(b.left as f32))
                .top(px(b.top as f32))
                .w(px(b.width as f32))
                .min_h(px(b.height as f32))
                .border_2()
                .border_color(gpui_kit::rgb(0x1651b0))
                .bg(gpui_kit::rgb(0xffffff))
                .p(px(pad as f32 - 1.0))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                // Live: Tab → the next cell, Shift+Tab → the previous one,
                // Enter → the cell below, Alt+Enter → a new line in the cell.
                .capture_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                    let k = &e.keystroke;
                    let step = match (k.key.as_str(), k.modifiers.shift, k.modifiers.alt) {
                        ("tab", false, _) => Some(1),
                        ("tab", true, _) => Some(-1),
                        ("enter", _, true) => {
                            if let Some(Panel::Cell { input, .. }) = &this.panel {
                                input.update(cx, |s, cx| s.insert("\n", window, cx));
                            }
                            cx.stop_propagation();
                            None
                        }
                        ("enter", _, false) => Some(2),
                        _ => None,
                    };
                    if let Some(step) = step {
                        cx.stop_propagation();
                        this.cell_step(step, window, cx);
                    }
                }))
                .child(div().border_1().border_dashed().border_color(gpui_kit::rgb(0x8a8a8a)).pt(px(((b.text_top - b.top - pad).max(0.0)) as f32)).child(text))
                .into_any_element(),
        )
    }
}
