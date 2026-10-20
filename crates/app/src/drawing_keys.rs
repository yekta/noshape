//! Drawing editor, Onshape shortcuts that need their own state: `w` Zoom
//! window (drag a box, the sheet zooms to it), Ctrl+S (Sheets panel open /
//! close), Shift+N (rename the current sheet or the selected view).

use crate::drawing_view::*;
use noshape_model::drawing::ViewId;
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
        // The panel pushes the canvas (live drawing/30): pan so the sheet
        // stays where it was on screen.
        let half = f64::from(crate::drawing_extra::SHEETS_W) / 2.0 / self.zoom.max(1e-9);
        self.center[0] += if self.sheets_open { half } else { -half };
        self.redraw(cx);
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

// ---------------------------------------------------------------- shortcut toolbar

/// Tools of the drawing shortcut toolbar (`s`): tooltip, icon, tool.
type ToolFn = fn() -> Tool;
const SHORTCUT_TOOLS: [(&str, &str, ToolFn); 11] = [
    ("Dimension (d)", "linear-dim", || Tool::Dimension { kind: DimTool::Linear, picks: vec![] }),
    ("Note (n)", "note", || Tool::Note),
    ("Callout", "balloon", || Tool::Balloon { anchor: None }),
    ("Projected view (p)", "projected-view", || Tool::Projected { parent: None }),
    ("Section view", "section-view", || Tool::Section { parent: None, p0: None, line: None, partial: false }),
    ("Detail view", "detail-view", || Tool::Detail { parent: None, center: None, radius: None }),
    ("Centermark", "centermark", || Tool::Centermark),
    ("2 point centerline", "centerline", || Tool::Centerline { first: None }),
    ("Geometric tolerance", "gdt", || Tool::Gdt),
    ("Datum", "datum", || Tool::Datum { anchor: None }),
    ("Surface finish symbol", "surface-finish", || Tool::SurfaceFinish),
];

impl DrawingEditor {
    /// `s`: the shortcut toolbar at the cursor (again or Esc closes it).
    pub fn toggle_shortcut_toolbar(&mut self, cx: &mut Context<Self>) {
        self.shortcut_toolbar = match self.shortcut_toolbar {
            Some(_) => None,
            None => {
                let s = self.size_px();
                let p = self.screen_of(self.cursor);
                Some([p[0].clamp(0.0, s[0]), p[1].clamp(0.0, s[1])])
            }
        };
        cx.notify();
    }

    /// Canvas pixels of a sheet point.
    fn screen_of(&self, q: [f64; 2]) -> [f64; 2] {
        let s = self.size_px();
        [(q[0] - self.center[0]) * self.zoom + s[0] / 2.0, -(q[1] - self.center[1]) * self.zoom + s[1] / 2.0]
    }

    pub fn render_shortcut_toolbar(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let at = self.shortcut_toolbar?;
        let pal = p();
        let mut row = gpui_kit::component::h_flex().gap_0p5();
        for (tip, icon, tool) in SHORTCUT_TOOLS {
            row = row.child(
                div()
                    .id(SharedString::from(format!("dr-mini-{icon}")))
                    .size(px(30.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(crate::assets::icon(&crate::assets::icon_path(icon), 20., cx))
                    .tooltip(crate::ui::tooltip(tip))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            this.shortcut_toolbar = None;
                            this.set_tool(tool(), cx);
                        }),
                    ),
            );
        }
        let w = SHORTCUT_TOOLS.len() as f32 * 32.0 + 8.0;
        let s = self.size_px();
        let left = (at[0] as f32 - w / 2.0).clamp(4.0, (s[0] as f32 - w - 4.0).max(4.0));
        Some(
            div()
                .id("dr-shortcut-toolbar")
                .occlude()
                .absolute()
                .left(px(left))
                .top(px((at[1] as f32 - 44.0).max(4.0)))
                .p_1()
                .rounded_lg()
                .bg(pal.panel_raised)
                .border_1()
                .border_color(pal.border_strong)
                .shadow_lg()
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(row)
                .into_any_element(),
        )
    }
}

// ---------------------------------------------------------------- measure

/// What the Measure tool (`[`) picked.
#[derive(Clone, Debug)]
pub enum MeasurePick {
    Point(PickedPoint),
    Line(PickedPoint, PickedPoint),
    Circle { view: ViewId, center: [f64; 2], radius: f64 },
}

impl DrawingEditor {
    /// `[`: Measure on / off. Click points, straight edges or circles; up to
    /// two picks are measured (model units, through the view scale).
    pub fn toggle_measure(&mut self, cx: &mut Context<Self>) {
        self.measure = if self.measure.is_some() { None } else { Some(Vec::new()) };
        self.status = self.measure.as_ref().map(|_| "Measure: select points, straight edges or circles".into());
        cx.notify();
    }

    /// Click while measuring. True when used.
    pub fn measure_click(&mut self, cx: &mut Context<Self>) -> bool {
        if self.measure.is_none() {
            return false;
        }
        self.update_hover(cx);
        let c = self.cursor;
        let pick = if let Some(p) = self.snap_at(c) {
            Some(MeasurePick::Point(p))
        } else if let Some(Pick::Line { a, b }) = self.line_pick() {
            Some(MeasurePick::Line(a, b))
        } else if let Some(Pick::Circle { view, center, radius, .. }) = self.circle_pick(c) {
            Some(MeasurePick::Circle { view, center, radius })
        } else {
            None
        };
        if let Some(m) = &mut self.measure {
            match pick {
                Some(p) => {
                    if m.len() >= 2 {
                        m.clear();
                    }
                    m.push(p);
                }
                None => m.clear(),
            }
        }
        cx.notify();
        true
    }

    /// Model length of a view-coordinate length (view coordinates are model meters).
    fn measure_text(&self, meters: f64) -> String {
        noshape_model::drawing::build::format_length(&self.drawing, meters) + if self.drawing.units_mm { " mm" } else { " in" }
    }

    /// The Measure results panel (bottom left of the drawing area).
    pub fn render_measure(&self) -> Option<AnyElement> {
        let picks = self.measure.as_ref()?;
        let pal = p();
        let mut rows: Vec<(String, String)> = Vec::new();
        let d2 = |a: [f64; 2], b: [f64; 2]| ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
        match picks.as_slice() {
            [] => {}
            [MeasurePick::Line(a, b)] => rows.push(("Length".into(), self.measure_text(d2(a.p, b.p)))),
            [MeasurePick::Circle { radius, .. }] => {
                rows.push(("Diameter".into(), self.measure_text(radius * 2.0)));
                rows.push(("Radius".into(), self.measure_text(*radius)));
            }
            [MeasurePick::Point(_)] => rows.push(("Point".into(), "select another entity".into())),
            [x, y] => {
                let point = |m: &MeasurePick| match m {
                    MeasurePick::Point(p) => Some((p.view, p.p)),
                    MeasurePick::Circle { view, center, .. } => Some((*view, *center)),
                    MeasurePick::Line(..) => None,
                };
                match (point(x), point(y), x, y) {
                    (Some((v1, p)), Some((v2, q)), _, _) if v1 == v2 => {
                        rows.push(("Distance".into(), self.measure_text(d2(p, q))));
                        rows.push(("Δx".into(), self.measure_text((q[0] - p[0]).abs())));
                        rows.push(("Δy".into(), self.measure_text((q[1] - p[1]).abs())));
                    }
                    (Some((v1, p)), None, _, MeasurePick::Line(a, b)) | (None, Some((v1, p)), MeasurePick::Line(a, b), _) if a.view == v1 => {
                        // Point to line: perpendicular distance.
                        let (u, w) = ([b.p[0] - a.p[0], b.p[1] - a.p[1]], [p[0] - a.p[0], p[1] - a.p[1]]);
                        let l = (u[0] * u[0] + u[1] * u[1]).sqrt().max(1e-12);
                        rows.push(("Distance".into(), self.measure_text((u[0] * w[1] - u[1] * w[0]).abs() / l)));
                    }
                    (None, None, MeasurePick::Line(a1, b1), MeasurePick::Line(a2, b2)) if a1.view == a2.view => {
                        let ang = |a: &PickedPoint, b: &PickedPoint| (b.p[1] - a.p[1]).atan2(b.p[0] - a.p[0]);
                        let mut t = (ang(a1, b1) - ang(a2, b2)).abs().to_degrees() % 180.0;
                        if t > 90.0 {
                            t = 180.0 - t;
                        }
                        rows.push(("Angle".into(), format!("{t:.2}°")));
                        if t < 1e-6 {
                            let (u, w) = ([b1.p[0] - a1.p[0], b1.p[1] - a1.p[1]], [a2.p[0] - a1.p[0], a2.p[1] - a1.p[1]]);
                            let l = (u[0] * u[0] + u[1] * u[1]).sqrt().max(1e-12);
                            rows.push(("Distance".into(), self.measure_text((u[0] * w[1] - u[1] * w[0]).abs() / l)));
                        }
                    }
                    _ => rows.push(("Measure".into(), "pick entities in the same view".into())),
                }
            }
            _ => {}
        }
        let mut col = gpui_kit::component::v_flex().gap_1();
        col = col.child(div().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Measure"));
        for (k, v) in rows {
            col = col.child(gpui_kit::component::h_flex().gap_3().text_size(px(12.)).child(div().w(px(70.)).text_color(pal.text_muted).child(k)).child(div().child(v)));
        }
        // Beside the Sheets panel when it is open.
        Some(div().absolute().left(px(self.dialog_left() + 4.)).bottom(px(34.)).min_w(px(200.)).p_2().bg(pal.dialog_bg).border_1().border_color(pal.border).shadow_md().text_color(pal.text).child(col).into_any_element())
    }
}
