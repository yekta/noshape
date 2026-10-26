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
use gpui_kit::component::Sizable as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::component::input::{Enter, Escape, IndentInline, OutdentInline, Textarea};
use gpui_kit::*;
use noshape_model::drawing::note;
use noshape_model::drawing::{AnnotationKind, TableKind};

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

/// The xsmall Textarea's own left / right padding (`Size::input_px`; no
/// vertical padding).
const TEXT_PX: f64 = 4.0;

impl DrawingEditor {
    /// Close the in-place note / cell editor without applying it (Esc, ✕);
    /// the hidden sheet text shows again at once.
    pub fn discard_inplace(&mut self, cx: &mut Context<Self>) {
        self.panel = None;
        self.inplace_hidden = None;
        self.inplace_cell = None;
        self.refresh(cx);
        cx.notify();
    }

    /// Sheet millimeters to canvas pixels (inverse of `to_sheet`).
    pub fn sheet_to_local(&self, p: [f64; 2]) -> [f64; 2] {
        let s = self.size_px();
        [(p[0] - self.center[0]) * self.zoom + s[0] / 2.0, -(p[1] - self.center[1]) * self.zoom + s[1] / 2.0]
    }

    /// The open note editor's place on the canvas.
    pub fn note_inplace_box(&self, cx: &App) -> Option<InplaceBox> {
        let Some(Panel::Note { input, pos, height, style, .. }) = &self.panel else { return None };
        let h = crate::drawing_note::parse_note_height(&height.read(cx).value(), self.drawing.units_mm).unwrap_or(3.5);
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
            None => (noshape_model::drawing::font::DEFAULT_FAMILY.to_string(), false, false),
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
        // The ruler at the sheet's scale over the note's box (live 414).
        self.ruler.sheet_k = Some(self.zoom as f32);
        let ruler = self.note_ruler(&style, b.width / self.zoom, cx);
        let ink = match style.color {
            Some([r, g, bl]) => gpui_kit::rgb(((r as u32) << 16) | ((g as u32) << 8) | bl as u32),
            None => gpui_kit::rgb(0x000000),
        };
        let mut text = Textarea::new(&input)
            .appearance(false)
            .xsmall()
            .w(px((b.width + b.font_px * 2.0 + 2.0 * TEXT_PX) as f32))
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
        if style.underline {
            text = text.underline();
        }
        if style.strikethrough {
            text = text.line_through();
        }
        let top = b.text_top.min(b.top);
        Some(
            div()
                .id("dr-note-inplace")
                .absolute()
                // The text's first glyph on the note's: the box border (1 px),
                // its padding and the Textarea's own padding go to the left.
                .left(px((b.left - pad - 1.0 - TEXT_PX) as f32))
                .top(px((top - pad - 1.0 - RULER_H) as f32))
                // Live: Esc ends editing and discards the change.
                .capture_key_down(cx.listener(|this, e: &KeyDownEvent, _, cx| {
                    if e.keystroke.key == "escape" {
                        cx.stop_propagation();
                        this.discard_inplace(cx);
                    }
                }))
                .flex()
                .flex_col()
                // Clicks in the editor stay in it (a click away commits).
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                // The ruler's zero on the text's left edge (the tab stop
                // selector, 22 px + 3 px gap, to the left of it), and the text
                // box not stretched to the ruler's width.
                .items_start()
                .child(div().h(px(RULER_H as f32)).ml(px((pad + 1.0 + TEXT_PX - 25.0) as f32)).child(ruler))
                .child(div().p(px(pad as f32)).border_1().border_dashed().border_color(gpui_kit::rgb(0x8a8a8a)).child(text))
                .into_any_element(),
        )
    }

    /// While a free note's rotate grip is dragged: its angle in a box by the
    /// pointer (live 235 / 236: "335deg", 2 px orange border).
    pub fn render_rotate_label(&self) -> Option<AnyElement> {
        let Drag::NoteGrip { id, grip: 2, .. } = &self.drag else { return None };
        let a = self.display().sheets.get(self.sheet)?.annotations.iter().find(|a| a.id == *id)?;
        let AnnotationKind::Note { style, .. } = &a.kind else { return None };
        let deg = style.angle.to_degrees().round().rem_euclid(360.0) as i64;
        let p = self.sheet_to_local(self.cursor);
        Some(
            div()
                .absolute()
                .left(px(p[0] as f32 + 18.0))
                .top(px(p[1] as f32 - 34.0))
                .px(px(2.))
                .bg(crate::theme::light().panel)
                .border_2()
                .border_color(gpui_kit::rgb(0xf5a623))
                // Live 235: 14 px, no space before "deg".
                .text_size(px(14.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(gpui_kit::rgb(0x000000))
                .child(format!("{deg}deg"))
                .into_any_element(),
        )
    }

    /// Byte offset in a general table cell's `text` under the pointer
    /// (live 756: a double-click puts the caret there, nothing selected);
    /// the cell's lines laid out as `tables::render` draws them.
    pub fn cell_click_offset(&self, table: u64, row: usize, col: usize, text: &str) -> usize {
        use noshape_model::drawing::font::text_width;
        use noshape_model::drawing::{CellAlign, CellVAlign};
        let (Some(t), Some(l)) = (self.current_sheet().table(table), self.built.as_ref().and_then(|b| b.table_layout(table))) else { return text.len() };
        let r = row + l.title as usize;
        let (Some(&xa), Some(&xb), Some(&ya), Some(&yb)) = (l.xs.get(col), l.xs.get(col + 1), l.ys.get(r), l.ys.get(r + 1)) else { return text.len() };
        let f = t.shown_cell_format(row, col);
        let h = f.height.filter(|v| *v > 0.0).unwrap_or(self.drawing.table_text_height(t));
        let lines: Vec<&str> = text.split('\n').collect();
        let n = lines.len().max(1) as f64;
        let line_h = h * 1.5;
        let y0 = match f.valign {
            CellVAlign::Top => ya - h,
            CellVAlign::Middle => (ya + yb) / 2.0 + (n - 1.0) * line_h / 2.0,
            CellVAlign::Bottom => yb + h + (n - 1.0) * line_h,
        };
        let [cx_, cy] = self.cursor;
        let k = (((y0 - cy) / line_h).round().max(0.0) as usize).min(lines.len() - 1);
        let line = lines[k];
        let tw = text_width(line, h);
        let x0 = match f.align {
            CellAlign::Left => xa,
            CellAlign::Center => (xa + xb - tw) / 2.0,
            CellAlign::Right => xb - tw,
        };
        let before: usize = lines[..k].iter().map(|l| l.len() + 1).sum();
        let mut at = line.len();
        for (i, c) in line.char_indices() {
            let mid = x0 + text_width(&line[..i], h) + text_width(&c.to_string(), h) / 2.0;
            if cx_ < mid {
                at = i;
                break;
            }
        }
        before + at
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
        let f = t.shown_cell_format(*row, *col);
        let h = f.height.filter(|v| *v > 0.0).unwrap_or(self.drawing.table_text_height(t));
        // The Note panel's B / I show at once.
        let (bold, italic) = match &self.panel {
            Some(Panel::Cell { note: Some(n), .. }) => n.style.font.as_ref().map(|f| (f.bold, f.italic)).unwrap_or((false, false)),
            _ => (f.bold, f.italic),
        };
        let family = match &self.panel {
            Some(Panel::Cell { note: Some(n), .. }) => n.style.font.as_ref().map(|f| f.family.clone()),
            _ => None,
        }
        .or_else(|| f.font.clone())
        .unwrap_or_else(|| noshape_model::drawing::font::DEFAULT_FAMILY.to_string());
        let (cap, asc, desc) = metrics(&family);
        let font_px = h * self.zoom / cap;
        let pitch_px = h * 1.6 * self.zoom;
        let tl = self.sheet_to_local([x0, ya]);
        let br = self.sheet_to_local([x1, yb]);
        // One line centred in the cell.
        let mid = (tl[1] + br[1]) / 2.0;
        let base = mid + h * self.zoom / 2.0;
        let text_top = base - ((pitch_px - (asc + desc) * font_px) / 2.0 + asc * font_px);
        Some(InplaceBox { left: tl[0], top: tl[1], width: br[0] - tl[0], height: br[1] - tl[1], font_px, pitch_px, family, bold, italic, text_top })
    }

    /// The cell's text editor: the caret in the cell, a blue cell outline
    /// with a dashed text box (live 740–757).
    pub fn render_cell_inplace(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let b = self.cell_inplace_box()?;
        let Some(Panel::Cell { input, note, .. }) = &self.panel else { return None };
        let align = note.as_ref().map(|n| n.style.align).unwrap_or_default();
        let (underline, strike) = note.as_ref().map(|n| (n.style.underline, n.style.strikethrough)).unwrap_or_default();
        let pad = 2.0;
        // Centred / right-aligned cells: the editor is as wide as its text
        // and placed by the alignment (a multi-line Textarea aligns left).
        let value = input.read(cx).value().to_string();
        let text_px = value.split('\n').map(|l| noshape_model::drawing::font::text_width(l, b.font_px * metrics(&b.family).0)).fold(0.0, f64::max) * if b.bold { 1.08 } else { 1.0 };
        let inner = (b.width - 2.0 * pad).max(b.font_px);
        // One character's slack so GPUI's shaping never wraps the line.
        let fit = (text_px + 2.0 * TEXT_PX + b.font_px).min(inner);
        // Alt+Enter lines make the editor taller (live 750: the cell grows).
        let lines = value.split('\n').count().max(1) as f64;
        let (w, justify) = match align {
            note::NoteAlign::Center => (fit, 1),
            note::NoteAlign::Right => (fit, 2),
            _ => (inner, 0),
        };
        let mut text = Textarea::new(input)
            .appearance(false)
            .xsmall()
            .w(px(w as f32))
            .h(px((b.pitch_px * lines).max(b.height - 2.0 * pad) as f32))
            .text_size(px(b.font_px as f32))
            .line_height(px(b.pitch_px as f32))
            .font_family(SharedString::from(b.family.clone()))
            .text_color(gpui_kit::rgb(0x000000));
        if b.bold {
            text = text.font_weight(FontWeight::BOLD);
        }
        if b.italic {
            text = text.italic();
        }
        if underline {
            text = text.underline();
        }
        if strike {
            text = text.line_through();
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
                // Enter → the cell below, Alt+Enter → a new line in the cell,
                // Esc discards. Tab / Enter / Esc are the Textarea's key
                // bindings, so they are taken as actions before it sees them.
                .capture_action(cx.listener(|this, _: &IndentInline, window, cx| {
                    cx.stop_propagation();
                    this.cell_step(1, window, cx);
                }))
                .capture_action(cx.listener(|this, _: &OutdentInline, window, cx| {
                    cx.stop_propagation();
                    this.cell_step(-1, window, cx);
                }))
                .capture_action(cx.listener(|this, e: &Enter, window, cx| {
                    if !e.shift && !e.secondary {
                        cx.stop_propagation();
                        this.cell_step(2, window, cx);
                    }
                }))
                .capture_action(cx.listener(|this, _: &Escape, _, cx| {
                    cx.stop_propagation();
                    this.discard_inplace(cx);
                }))
                .capture_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                    let k = &e.keystroke;
                    if k.key == "enter" && k.modifiers.alt {
                        if let Some(Panel::Cell { input, .. }) = &this.panel {
                            input.update(cx, |s, cx| s.insert("\n", window, cx));
                        }
                        cx.stop_propagation();
                    }
                }))
                .child(
                    div()
                        .flex()
                        .when(justify == 1, |d| d.justify_center())
                        .when(justify == 2, |d| d.justify_end())
                        .border_1()
                        .border_dashed()
                        .border_color(gpui_kit::rgb(0x8a8a8a))
                        .pt(px(((b.text_top - b.top - pad).max(0.0)) as f32))
                        .child(text),
                )
                .into_any_element(),
        )
    }
}
