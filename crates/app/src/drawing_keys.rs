//! Drawing editor, Onshape shortcuts that need their own state: `w` Zoom
//! window (drag a box, the sheet zooms to it), Ctrl+S (Sheets panel open /
//! close), Shift+N (rename the current sheet or the selected view).

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::*;

impl DrawingEditor {
    /// `w` / Zoom to window: the next left drag draws the window.
    pub fn start_zoom_window(&mut self, cx: &mut Context<Self>) {
        self.zoom_window = Some(None);
        self.status = Some("Zoom window: drag a box around the area to zoom to".into());
        cx.notify();
    }

    /// Mouse down while Zoom window is armed. True when used.
    pub fn zoom_window_down(&mut self, lp: [f64; 2], cx: &mut Context<Self>) -> bool {
        if self.zoom_window.is_none() {
            return false;
        }
        self.zoom_window = Some(Some((lp, lp)));
        cx.notify();
        true
    }

    pub fn zoom_window_move(&mut self, lp: [f64; 2], cx: &mut Context<Self>) -> bool {
        let Some(Some((a, _))) = self.zoom_window else { return false };
        self.zoom_window = Some(Some((a, lp)));
        cx.notify();
        true
    }

    /// Mouse up: zoom so the window fills the drawing area (a click zooms in 2x there).
    pub fn zoom_window_up(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(Some((a, b))) = self.zoom_window else { return false };
        self.zoom_window = None;
        self.status = None;
        let (w, h) = ((b[0] - a[0]).abs(), (b[1] - a[1]).abs());
        let s = self.size_px();
        let mid = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5];
        let factor = if w < 4.0 || h < 4.0 { 2.0 } else { (s[0] / w).min(s[1] / h) };
        self.center = self.to_sheet(mid);
        self.zoom = (self.zoom * factor).clamp(0.05, 400.0);
        self.redraw(cx);
        true
    }

    /// The window being dragged (canvas-local pixels).
    pub fn render_zoom_window(&self) -> Option<AnyElement> {
        let Some(Some((a, b))) = self.zoom_window else { return None };
        let pal = p();
        Some(
            div()
                .absolute()
                .left(px(a[0].min(b[0]) as f32))
                .top(px(a[1].min(b[1]) as f32))
                .w(px((b[0] - a[0]).abs() as f32))
                .h(px((b[1] - a[1]).abs() as f32))
                .border_1()
                .border_color(pal.accent)
                .bg(crate::theme::with_alpha(pal.accent, 0.08))
                .into_any_element(),
        )
    }

    /// Ctrl+S: open / close the Sheets panel ("Display sheet menu").
    pub fn toggle_sheets_panel(&mut self, cx: &mut Context<Self>) {
        self.sheets_open = !self.sheets_open;
        cx.notify();
    }

    /// Shift+N: rename the selected view (View properties › Name), else the current sheet.
    pub fn rename_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = self.selection.iter().find_map(|s| if let Sel::View(v) = s { Some(*v) } else { None });
        match view {
            Some(v) => self.open_view_props(v, window, cx),
            None => self.open_sheet_name_panel(self.sheet, window, cx),
        }
    }
}
