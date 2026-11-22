//! Drawing editor, Onshape shortcuts that need their own state: `w` Zoom
//! window (drag a box, the sheet zooms to it), Ctrl+S (Sheets panel open /
//! close), Shift+N (rename the current sheet or, in the Sheets panel, a view), Space, `[` Measure.

use crate::drawing_sketch::{SkCon, SkTool};
use crate::drawing_view::*;
use noshape_scene::drawing_tools::{MEASURE_UNITS, Measured, measure_pick_names, measure_rows};
use crate::theme::p;
use gpui_kit::*;
use crate::os_ui::OsDropdownExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};

/// A drawing's Control-only shortcuts (live's key list, behaviours "Keys on macOS"): the Control key on every OS,
/// a Mac's too; Cmd (Super / Windows key elsewhere) never.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlKey {
    /// Ctrl+M: Max/min dimension.
    MaxMin,
    /// Ctrl+S: Display sheet menu (the Sheets panel).
    Sheets,
}

pub fn control_key(m: &Modifiers, key: &str) -> Option<ControlKey> {
    if !m.control || m.platform || m.shift || m.alt || m.function {
        return None;
    }
    match key {
        "m" => Some(ControlKey::MaxMin),
        "s" => Some(ControlKey::Sheets),
        _ => None,
    }
}

impl DrawingEditor {
    /// `w` / Zoom to window: the next left drag draws the window.
    pub fn start_zoom_window(&mut self, cx: &mut Context<Self>) {
        // No bubble (live2 shortcuts-1118).
        self.zoom_window = Some(None);
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
        (self.zoom, self.center) = noshape_scene::camera_ops::drawing_zoom_window(self.zoom, self.center, self.size_px(), a, b);
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

    /// Shift+/: the Keyboard shortcuts sheet (app-core's, shared by the
    /// studios), on its Drawing tab (live, qa DRAWING-SHIFT-SLASH).
    pub fn toggle_shortcut_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.shortcut_sheet.take().is_some() {
            window.focus(&self.focus, cx);
        } else {
            let sheet = crate::shortcut_sheet::ShortcutSheet::open(crate::shortcuts::Tab::Drawing, window, cx);
            let sub = cx.subscribe_in(&sheet, window, |this: &mut Self, _, _: &DismissEvent, window, cx| this.toggle_shortcut_sheet(window, cx));
            self.shortcut_sheet = Some((sheet, sub));
        }
        cx.notify();
    }

    /// Shift+N: with the Sheets panel open (live K20), one selected view or sheet row gets a "Rename" popup beside
    /// its row; any other selection, nothing. Closed: the current sheet's name, nothing with a view selected (live2
    /// shortcuts-1118).
    pub fn rename_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = self.selection.iter().find_map(|s| s.as_view());
        if self.sheets_open {
            let row = match (view, self.selection.len(), self.sheet_rows.as_slice()) {
                (Some(v), 1, []) => crate::drawing_extra::RowRename::View(v),
                (None, 0, [i]) => crate::drawing_extra::RowRename::Sheet(*i),
                _ => return,
            };
            return self.open_row_rename(row, window, cx);
        }
        if view.is_none() {
            self.open_sheet_name_panel(self.sheet, window, cx);
        }
    }

    /// Space: the selection cleared (live2 shortcuts-1118), not while a dialog is open.
    pub fn clear_selection_key(&mut self, cx: &mut Context<Self>) {
        if self.panel.is_some() || self.selection.is_empty() {
            return;
        }
        self.selection.clear();
        self.redraw(cx);
    }
}

// ---------------------------------------------------------------- shortcut toolbar

/// What a shortcut toolbar button does.
type Act = fn(&mut DrawingEditor, &mut Window, &mut Context<DrawingEditor>);

/// Every tool the drawing S toolbar can hold (Preferences › Shortcut
/// toolbars › Drawing, live REQUESTS z3: 64 tools in this order, and 1724's Edge condition symbol after Weld symbol): name,
/// icon, action.
pub const SHORTCUT_CATALOGUE: [(&str, &str, Act); 65] = [
    ("Dimension", "linear-dim", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Linear, picks: vec![] }, cx)),
    ("Maximum/minimum dimension", "driven-dimension", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::MaxMin, picks: vec![] }, cx)),
    ("2 point linear dimension", "dim-point-to-point", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Point2, picks: vec![] }, cx)),
    ("Point to line linear dimension", "dim-point-to-line", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::PointLine, picks: vec![] }, cx)),
    ("Line to line linear dimension", "dim-line-to-line", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::LineLine, picks: vec![] }, cx)),
    ("Line to line angular dimension", "angular-dim", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Angle, picks: vec![] }, cx)),
    ("Chamfer dimension", "chamfer-dim", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Chamfer, picks: vec![] }, cx)),
    ("3 point angular dimension", "dim-angular-3pt", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Angle3, picks: vec![] }, cx)),
    ("Radial dimension", "radial-dim", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Radius, picks: vec![] }, cx)),
    ("Diameter dimension", "diameter-dim", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Diameter, picks: vec![] }, cx)),
    ("Arc length dimension", "arc-length-dim", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::ArcLength, picks: vec![] }, cx)),
    ("Ordinate dimension", "ordinate-dim", |e, _, cx| e.set_tool(Tool::Ordinate { origin: None, horizontal: None, group: None }, cx)),
    ("Foreshorten dimension", "dim-foreshorten", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Foreshorten, picks: vec![] }, cx)),
    ("Hole/thread callout", "hole-callout", |e, _, cx| e.set_tool(Tool::HoleCallout { circle: None }, cx)),
    ("Datum", "datum", |e, _, cx| e.set_tool(Tool::Datum { anchor: None }, cx)),
    ("Geometric tolerance", "gdt", |e, _, cx| e.set_tool(Tool::Gdt, cx)),
    ("Surface finish symbol", "surface-finish", |e, _, cx| e.set_tool(Tool::SurfaceFinish, cx)),
    ("Weld symbol", "weld-symbol", |e, _, cx| e.set_tool(Tool::Weld, cx)),
    ("Edge condition symbol", "drawing-edge-condition", |e, _, cx| e.set_tool(Tool::EdgeCondition, cx)),
    ("Note", "note", |e, _, cx| e.set_tool(Tool::Note, cx)),
    ("Callout", "callout", |e, _, cx| e.set_tool(Tool::Balloon { anchor: None }, cx)),
    ("Table", "table", |e, _, cx| e.set_tool(Tool::Table { kind: TableTool::General, view: None, origin: None }, cx)),
    ("Insert BOM", "drawing-bom", |e, _, cx| e.set_tool(Tool::Table { kind: TableTool::Bom, view: None, origin: None }, cx)),
    ("Hole table", "drawing-hole-table", |e, _, cx| e.set_tool(Tool::Table { kind: TableTool::Hole, view: None, origin: None }, cx)),
    ("Revision table", "revision-table", |e, _, cx| e.set_tool(Tool::Table { kind: TableTool::Revision, view: None, origin: None }, cx)),
    ("Custom table", "drawing-custom-table", |e, _, cx| e.set_tool(Tool::Table { kind: TableTool::Custom, view: None, origin: None }, cx)),
    ("2 point linear centerline", "centerline", |e, _, cx| e.set_tool(Tool::Centerline { first: None }, cx)),
    ("Edge to edge centerline", "centerline-edge", |e, _, cx| e.set_tool(Tool::EdgeCenterline { first: None }, cx)),
    ("3 point circle centerline", "centerline-circle3", |e, _, cx| e.set_tool(Tool::CircleCenterline { three: true, points: vec![] }, cx)),
    ("2 point circle centerline", "centerline-circle2", |e, _, cx| e.set_tool(Tool::CircleCenterline { three: false, points: vec![] }, cx)),
    ("Centermark", "centermark", |e, _, cx| e.set_tool(Tool::Centermark, cx)),
    ("Virtual sharp", "virtual-sharp", |e, _, cx| e.set_tool(Tool::VirtualSharp { first: None }, cx)),
    ("Line", "drawing-line", |e, _, cx| e.set_tool(Tool::Sketch { tool: SkTool::Line, pts: vec![], from: None }, cx)),
    ("Center point circle", "circle", |e, _, cx| e.set_tool(Tool::Sketch { tool: SkTool::Circle, pts: vec![], from: None }, cx)),
    ("3 point circle", "three-point-circle", |e, _, cx| e.set_tool(Tool::Sketch { tool: SkTool::Circle3, pts: vec![], from: None }, cx)),
    ("Corner rectangle", "corner-rectangle", |e, _, cx| e.set_tool(Tool::Sketch { tool: SkTool::CornerRect, pts: vec![], from: None }, cx)),
    ("Center point rectangle", "center-rectangle", |e, _, cx| e.set_tool(Tool::Sketch { tool: SkTool::CenterRect, pts: vec![], from: None }, cx)),
    ("Spline", "spline", |e, _, cx| e.set_tool(Tool::Sketch { tool: SkTool::Spline, pts: vec![], from: None }, cx)),
    ("Spline control point", "spline-point", |e, _, cx| e.set_tool(Tool::Sketch { tool: SkTool::SplinePoint, pts: vec![], from: None }, cx)),
    ("Sketch point", "point-sketch", |e, _, cx| e.set_tool(Tool::Sketch { tool: SkTool::Point, pts: vec![], from: None }, cx)),
    ("Tangent arc", "tangent-arc", |e, _, cx| e.set_tool(Tool::Sketch { tool: SkTool::TangentArc, pts: vec![], from: None }, cx)),
    ("Hatch region", "hatch-region", |e, w, cx| e.open_hatch_dialog(None, w, cx)),
    ("Insert DXF or DWG", "dwg-insert", |e, w, cx| e.insert_cad_file(w, cx)),
    ("Insert image", "image", |e, w, cx| e.insert_image(w, cx)),
    ("Format painter", "format-painter", |e, _, cx| e.toggle_format_painter(cx)),
    ("Insert view", "base-view", |e, _, cx| e.open_insert_panel(cx)),
    ("Projected view", "projected-view", |e, _, cx| e.set_tool(Tool::Projected { parent: None }, cx)),
    ("Auxiliary view", "auxiliary-view", |e, _, cx| e.set_tool(Tool::Auxiliary { parent: None }, cx)),
    ("Detail view", "detail-view", |e, _, cx| e.set_tool(Tool::Detail { parent: None, center: None, radius: None }, cx)),
    ("Section view", "section-view", |e, _, cx| e.set_tool(Tool::Section { parent: None, p0: None, line: None, partial: false, jogs: vec![] }, cx)),
    ("Broken-out section", "broken-out-section", |e, _, cx| {
        e.bo_start_blind = false;
        e.set_tool(Tool::BrokenOut { parent: None, points: vec![], done: false }, cx)
    }),
    ("Break view", "broken-view", |e, _, cx| e.set_tool(Tool::Break { parent: None, first: None, vertical: true }, cx)),
    ("Crop view", "crop-view", |e, _, cx| e.start_crop(cx)),
    ("Inspection item", "inspection-item", |e, w, cx| e.open_inspection(w, cx)),
    ("Hide inspection item", "inspection-hide", |e, _, cx| e.toggle_hide_inspection(cx)),
    ("Aligned section view", "drawing-aligned-section", |e, _, cx| e.set_tool(Tool::AlignedSection { parent: None, center: None, a: None, b: None }, cx)),
    ("Cut list table", "drawing-cutlist-table", |e, _, cx| e.set_tool(Tool::Table { kind: TableTool::CutList, view: None, origin: None }, cx)),
    ("Update", "drawing-update", |e, _, cx| e.update_drawing(cx)),
    ("Horizontal", "horizontal", |e, _, cx| e.set_tool(Tool::Constrain { kind: SkCon::Horizontal, picks: vec![] }, cx)),
    ("Vertical", "vertical", |e, _, cx| e.set_tool(Tool::Constrain { kind: SkCon::Vertical, picks: vec![] }, cx)),
    ("Coincident", "coincident", |e, _, cx| e.set_tool(Tool::Constrain { kind: SkCon::Coincident, picks: vec![] }, cx)),
    ("Perpendicular", "perpendicular", |e, _, cx| e.set_tool(Tool::Constrain { kind: SkCon::Perpendicular, picks: vec![] }, cx)),
    ("Parallel", "parallel", |e, _, cx| e.set_tool(Tool::Constrain { kind: SkCon::Parallel, picks: vec![] }, cx)),
    ("Tangent", "tangent", |e, _, cx| e.set_tool(Tool::Constrain { kind: SkCon::Tangent, picks: vec![] }, cx)),
    ("Measure", "measure", |e, _, cx| e.toggle_measure(cx)),
];

/// The 15 tools ticked by default (live z3).
pub const DEFAULT_SHORTCUT_TOOLS: [&str; 15] = ["Dimension", "Ordinate dimension", "Hole/thread callout", "Datum", "Geometric tolerance", "Surface finish symbol", "Weld symbol", "Note", "Callout", "Table", "Insert view", "Projected view", "Detail view", "Section view", "Break view"];

impl DrawingEditor {
    /// `s`: the shortcut toolbar at the cursor (live z3: its corner 10 px
    /// right of and 10 px below the cursor). Pressing `s` again keeps it;
    /// Esc, a click elsewhere or picking a tool closes it.
    pub fn toggle_shortcut_toolbar(&mut self, cx: &mut Context<Self>) {
        if self.shortcut_toolbar.is_some() {
            return;
        }
        let s = self.size_px();
        let p = self.screen_of(self.cursor);
        self.shortcut_toolbar = Some([(p[0] + 10.0).clamp(0.0, (s[0] - 179.0).max(0.0)), (p[1] + 10.0).clamp(0.0, (s[1] - 96.0).max(0.0))]);
        cx.notify();
    }

    /// Canvas pixels of a sheet point.
    fn screen_of(&self, q: [f64; 2]) -> [f64; 2] {
        let s = self.size_px();
        [(q[0] - self.center[0]) * self.zoom + s[0] / 2.0, -(q[1] - self.center[1]) * self.zoom + s[1] / 2.0]
    }

    /// Onshape's drawing S toolbar (live z3, drawing/1121): the ticked
    /// tools in list order, five per row at a 34 px pitch (177 px wide), on
    /// white with a 1 px `#ccc` border and shadow 0 5 10 black 25 %; no
    /// tooltips.
    pub fn render_shortcut_toolbar(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let at = self.shortcut_toolbar?;
        let shown = noshape_ops::prefs::ToolbarPrefs::load().drawing_shortcut_tools(&DEFAULT_SHORTCUT_TOOLS);
        let mut row = gpui_kit::component::h_flex().w(px(175.)).flex_wrap().px(px(2.));
        // Live drawing-shortcut-toolbar: an emptied toolbar says so.
        if !SHORTCUT_CATALOGUE.iter().any(|t| shown.iter().any(|n| n == t.0)) {
            row = row.child(div().p(px(8.)).text_size(px(12.)).text_color(rgb(0x888888)).child("No tools added"));
        }
        for &(name, icon, act) in SHORTCUT_CATALOGUE.iter().filter(|t| shown.iter().any(|n| n == t.0)) {
            row = row.child(
                div()
                    .id(SharedString::from(format!("dr-mini-{name}")))
                    .size(px(30.))
                    .m(px(2.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(3.))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgb(0xeaeaea)))
                    .child(crate::assets::icon(&crate::assets::icon_path(icon), 20., cx))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            this.shortcut_toolbar = None;
                            act(this, window, cx);
                        }),
                    ),
            );
        }
        Some(
            div()
                .id("dr-shortcut-toolbar")
                .occlude()
                .absolute()
                .left(px(at[0] as f32))
                .top(px(at[1] as f32))
                .bg(rgb(0xffffff))
                .border_1()
                .border_color(rgb(0xcccccc))
                .shadow(vec![gpui_kit::BoxShadow { color: gpui_kit::hsla(0., 0., 0., 0.25), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(row)
                .into_any_element(),
        )
    }
}

// ---------------------------------------------------------------- measure

pub(crate) fn measure_unit_labels(units_mm: bool) -> Vec<String> {
    let own = if units_mm { 1 } else { 0 };
    MEASURE_UNITS.iter().enumerate().map(|(i, u)| if i == own { format!("{} (Drawing)", u.0) } else { u.0.to_string() }).collect()
}

impl DrawingEditor {
    /// `[`: Measure on / off. Click points, straight edges or circles; up to
    /// two picks are measured (model units, through the view scale).
    /// Live2 shortcuts-1118: the Measure panel at the top left, no bubble; opening it clears the selection.
    pub fn toggle_measure(&mut self, cx: &mut Context<Self>) {
        self.measure = if self.measure.is_some() { None } else { Some(Vec::new()) };
        if self.measure.is_some() {
            self.selection.clear();
        }
        self.redraw(cx);
    }

    /// Click while measuring. True when used.
    pub fn measure_click(&mut self, cx: &mut Context<Self>) -> bool {
        if self.measure.is_none() {
            return false;
        }
        self.update_hover(cx);
        let c = self.cursor;
        let pick = self.pk().measure_pick(c);
        // Live measure-1119: a click on empty sheet keeps the picks (only a row's × removes one), a pick in another
        // view than the first pick's is ignored, and a third pick is added.
        if let (Some(m), Some(p)) = (&mut self.measure, pick)
            && m.first().is_none_or(|f| f.view() == p.view())
        {
            m.push(p);
        }
        cx.notify();
        true
    }

    /// Model length of a view-coordinate length (view coordinates are model meters); with Show dual unit, the other
    /// unit after it in brackets.
    pub(crate) fn measure_text(&self, meters: f64) -> String {
        // Live2 wake-1118 (mu-mm-edge-*.png): two decimals in the chosen unit ("40.00 mm", "1.57 in"); Show dual unit
        // adds the drawing's other system with three ("[1.575 in]"), whatever unit is chosen.
        let own = if self.drawing.units_mm { 1 } else { 0 };
        let (_, per, abbr) = MEASURE_UNITS[self.measure_unit.unwrap_or(own)];
        let main = format!("{:.2} {abbr}", meters / per);
        if !self.measure_dual {
            return main;
        }
        let other = if self.drawing.units_mm { format!("{:.3} in", meters / 0.0254) } else { format!("{:.3} mm", meters * 1000.0) };
        format!("{main} [{other}]")
    }

    /// The Measure panel's picks as its selection box lists them.
    pub(crate) fn measure_pick_names(&self) -> Vec<String> {
        measure_pick_names(self.measure.as_deref().unwrap_or_default())
    }

    /// The Measure button at the drawing's bottom right (live keys-1118 hov-measure: "Measure ([)"), pressed while
    /// Measure is on.
    pub fn render_measure_button(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("dr-measure")
            .debug_selector(|| "dr-measure".into())
            .absolute()
            .right_0()
            .bottom_0()
            .occlude()
            .tooltip(crate::ui::tooltip("Measure ([)"))
            .child(crate::ui::icon_button("dr-measure-btn", "measure", 30., self.measure.is_some(), cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_measure(cx))))
            .into_any_element()
    }

    /// The Measure panel with what the picks measure.
    pub fn render_measure(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let picks = self.measure.as_ref()?;
        let pal = p();
        let (lengths, relation) = measure_rows(picks);
        let rows: Vec<(&str, String)> = lengths.into_iter().map(|(k, m)| (k, match m {
            Measured::Length(m) => self.measure_text(m),
            Measured::Angle(deg) => format!("{deg:.1} deg"),
        })).collect();
        // Live2 shortcuts-1118 (measure.png): a drawing panel at the top left: "Measure" with the red ✕, the blue
        // selection box "Select entities to measure", the unit "Inches (Drawing)" and Show dual unit.
        // The selection box: its caption, then a row per pick with its ✕ (live2 mu-mm-edge-mm-dual.png).
        let field = self.measure_pick_names().into_iter().enumerate().fold(
            v_flex().mx(px(4.)).my(px(3.)).px(px(6.)).py(px(4.)).border_1().border_color(pal.accent).bg(gpui_kit::rgb(0xe8f1fb)).child(div().text_size(px(11.)).text_color(pal.text_muted).child("Select entities to measure")),
            |f, (i, name)| {
                f.child(h_flex().h(px(22.)).text_size(px(13.)).child(div().flex_1().child(name)).child(div().id(("measure-pick-x", i)).px(px(4.)).cursor_pointer().text_color(pal.text_muted).child("×").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(m) = &mut this.measure
                        && i < m.len()
                    {
                        m.remove(i);
                    }
                    cx.notify();
                }))))
            },
        );
        // Under Show dual unit: "Measurements (Projected geometry)", the labels right-aligned in one column with the
        // values beside them, then after a blank line "Relation: …" (live measure-1119).
        let results = (!rows.is_empty()).then(|| {
            let labels = rows.iter().fold(v_flex().items_end().pl(px(10.)), |c, (k, _)| c.child(format!("{k}:")));
            let values = rows.iter().fold(v_flex(), |c, (_, v)| c.child(v.clone()));
            v_flex().px(px(8.)).pt(px(4.)).pb(px(6.)).text_size(px(13.)).child(div().child("Measurements (Projected geometry)")).child(h_flex().gap(px(8.)).child(labels).child(values))
                .when_some(relation, |c, r| c.child(div().pt(px(18.)).child(format!("Relation: {r}"))))
        });
        // Live2 wake-1118 (mu1c.png, mu-mm-units.png): live's fixed order, the drawing's own unit suffixed "(Drawing)"
        // in its place; the box's tooltip "Units".
        let own = if self.drawing.units_mm { 1 } else { 0 };
        let labels = measure_unit_labels(self.drawing.units_mm);
        let current = self.measure_unit.unwrap_or(own);
        let unit = labels[current].clone();
        let ed = cx.entity();
        let units = Button::new("measure-unit").label(unit.clone()).ghost().small().w_full().dropdown_caret(true).tooltip("Units").os_dropdown_menu("measure-unit-dd", true, move |mut m, _, _| {
            for (i, l) in labels.iter().enumerate() {
                let e = ed.clone();
                m = m.item(crate::os_menu::OsMenuItem::new(l.clone()).checked(i == current).on_click(move |_, _, cx| {
                    e.update(cx, |t, cx| {
                        t.measure_unit = (i != own).then_some(i);
                        cx.notify();
                    })
                }));
            }
            m
        });
        let dual = self.measure_dual;
        let dual_box = div().h(px(26.)).px(px(4.)).flex().items_center().child(crate::drawing_chrome::DrCheckbox::new("measure-dual").label("Show dual unit").checked(dual).on_click(cx.listener(|this, v: &bool, _, cx| {
            this.measure_dual = *v;
            cx.notify();
        })));
        crate::ui_script::note_dialog(|| serde_json::json!({ "kind": "drawing-panel", "title": "Measure", "text": ["Measure", "Select entities to measure", unit.clone(), "Show dual unit".to_string()] }));
        Some(
            v_flex()
                .id("dr-measure-panel")
                .debug_selector(|| "dr-measure-panel".into())
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(220.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(pal.text)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_move(cx.listener(Self::pointer_off_sheet))
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Measure"))
                        .child(div().id("measure-close").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_measure(cx)))),
                )
                .child(v_flex().p(px(3.)).child(field).child(div().h(px(26.)).flex().items_center().border_b_1().border_color(gpui_kit::rgb(0xdddddd)).text_size(px(12.)).child(units)).child(dual_box).children(results))
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod control_key_tests {
    use super::{ControlKey, control_key, measure_unit_labels};

    /// Live2 wake-1118: the list keeps its order on a millimetre drawing; "(Drawing)" marks the drawing's unit.
    #[test]
    fn measure_units_keep_lives_order_with_the_drawings_unit_marked() {
        assert_eq!(measure_unit_labels(false), ["Inches (Drawing)", "Millimeters", "Centimeters", "Feet", "Meters", "Yards"]);
        assert_eq!(measure_unit_labels(true), ["Inches", "Millimeters (Drawing)", "Centimeters", "Feet", "Meters", "Yards"]);
    }
    use gpui_kit::Modifiers;

    /// Live: Control+S / Control+M only; Cmd+S / Cmd+M (Super / Windows elsewhere) and Shift or Alt chords do nothing.
    #[test]
    fn drawing_control_keys_take_the_control_key_only() {
        let ctrl = Modifiers { control: true, ..Default::default() };
        assert_eq!(control_key(&ctrl, "s"), Some(ControlKey::Sheets));
        assert_eq!(control_key(&ctrl, "m"), Some(ControlKey::MaxMin));
        assert_eq!(control_key(&ctrl, "q"), None);
        for m in [Modifiers { platform: true, ..Default::default() }, Modifiers { control: true, platform: true, ..Default::default() }, Modifiers { control: true, shift: true, ..Default::default() }, Modifiers { control: true, alt: true, ..Default::default() }, Modifiers::default()] {
            assert_eq!(control_key(&m, "s"), None, "{m:?}");
            assert_eq!(control_key(&m, "m"), None, "{m:?}");
        }
    }
}

