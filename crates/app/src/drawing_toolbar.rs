//! Drawing editor toolbar (live DOM toolbar-drawing.txt, in its order):
//! Undo, Redo | Update | views | dimensions | symbols | notes and tables |
//! centerlines | sketch | constraints | inserts | inspection. As in
//! Onshape's narrow window (behaviours.md "Narrow window: toolbar collapse",
//! part-studio/220–221): when the tools don't fit, every category collapses
//! into one split tool (its first tool's icon + ▾ listing the whole
//! category); there is no overflow button.

use crate::drawing_sketch::{SkCon, SkTool};
use crate::drawing_view::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::views;
use std::rc::Rc;

/// One toolbar tool: its button, tooltip / menu label, action and an
/// optional ▾ dropdown.
pub struct TbTool {
    pub id: &'static str,
    pub icon: &'static str,
    pub tip: &'static str,
    pub active: bool,
    pub run: EditorAction,
    pub more: Option<(&'static str, Vec<MenuEntry>)>,
}

fn tb(id: &'static str, icon: &'static str, tip: &'static str, active: bool, run: impl Fn(&mut DrawingEditor, &mut Window, &mut Context<DrawingEditor>) + 'static) -> TbTool {
    TbTool { id, icon, tip, active, run: Rc::new(run), more: None }
}

/// A tool that just switches to `tool`.
fn tool(id: &'static str, icon: &'static str, tip: &'static str, active: bool, t: impl Fn() -> Tool + 'static) -> TbTool {
    tb(id, icon, tip, active, move |e, w, cx| {
        w.focus(&e.focus, cx);
        e.set_tool(t(), cx)
    })
}

impl TbTool {
    fn with_more(mut self, caret: &'static str, items: Vec<MenuEntry>) -> Self {
        self.more = Some((caret, items));
        self
    }

    fn width(&self) -> f32 {
        crate::ui::TOOL_CELL + if self.more.is_some() { crate::ui::TOOL_CARET_W } else { 0. }
    }

    fn render(self, ed: &Entity<DrawingEditor>, cx: &mut Context<DrawingEditor>) -> AnyElement {
        let run = self.run.clone();
        let click = cx.listener(move |this, _: &ClickEvent, window, cx| run(this, window, cx));
        match self.more {
            None => crate::ui::tool_button(self.id, self.icon, self.active, cx).tooltip(self.tip).on_click(click).into_any_element(),
            Some((caret, items)) => {
                let ed = ed.clone();
                crate::ui::tool_group(SharedString::from(format!("drg-{}", self.id)), crate::ui::split_tool_button(self.id, self.icon, self.active, cx).tooltip(self.tip).on_click(click), crate::ui::tool_caret(caret, cx).tooltip(self.tip).on_click(move |e, w, cx| open_tool_menu(&items, &ed, e, w, cx))).into_any_element()
            }
        }
    }

    /// The tool's rows in a collapsed category's dropdown.
    fn menu_rows(&self) -> Vec<MenuEntry> {
        let run = self.run.clone();
        let mut out = vec![MenuEntry::item(self.tip, move |e, w, cx| run(e, w, cx)).with_icon(self.icon)];
        if let Some((_, items)) = &self.more {
            for m in items {
                match m {
                    MenuEntry::Item { label, .. } if label.as_ref() == self.tip => {}
                    MenuEntry::Separator => {}
                    m => out.push(m.clone()),
                }
            }
        }
        out
    }
}

/// A category collapsed to one split tool (`gi`: its index, for ids).
fn collapsed(gi: usize, group: Vec<TbTool>, ed: &Entity<DrawingEditor>, cx: &mut Context<DrawingEditor>) -> Option<AnyElement> {
    let first = group.first()?;
    let (icon, tip, run) = (first.icon, first.tip, first.run.clone());
    let active = group.iter().any(|t| t.active);
    // A tool also listed in another's ▾ (Aligned section view, Broken-out
    // section) shows once.
    let mut seen = std::collections::HashSet::new();
    let items: Vec<MenuEntry> = group.iter().flat_map(|t| t.menu_rows()).filter(|m| !matches!(m, MenuEntry::Item { label, .. } if !seen.insert(label.clone()))).collect();
    let ed = ed.clone();
    Some(
        crate::ui::tool_group(
            SharedString::from(format!("drg-c{gi}")),
            crate::ui::split_tool_button(SharedString::from(format!("dr-c{gi}")), icon, active, cx).tooltip(tip).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| run(this, window, cx))),
            crate::ui::tool_caret(SharedString::from(format!("dr-c{gi}-more")), cx).tooltip(tip).on_click(move |e, w, cx| open_tool_menu(&items, &ed, e, w, cx)),
        )
        .into_any_element(),
    )
}

impl DrawingEditor {
    /// The tool categories after Update, in live order.
    fn toolbar_groups(&self) -> Vec<Vec<TbTool>> {
        let t = self.tool.clone();
        let dim_kind = if let Tool::Dimension { kind, .. } = t { Some(kind) } else { None };
        // Onshape's Dimension dropdown (live, in order). The 2 point, point to
        // line and line to line tools use the smart linear dimension's picks.
        let dim = |kind: DimTool| move |e: &mut DrawingEditor, _: &mut Window, cx: &mut Context<DrawingEditor>| e.set_tool(Tool::Dimension { kind, picks: vec![] }, cx);
        let dims_menu = vec![
            MenuEntry::item("Dimension (d)", dim(DimTool::Linear)).with_icon("linear-dim"),
            MenuEntry::item("Chamfer dimension", dim(DimTool::Chamfer)).with_icon("chamfer-dim"),
            MenuEntry::item("Maximum or minimum dimension (ctrl+m)", dim(DimTool::MaxMin)).with_icon("driven-dimension"),
            MenuEntry::item("2 point linear dimension", dim(DimTool::Point2)).with_icon("linear-dim"),
            MenuEntry::item("Point to line dimension", dim(DimTool::PointLine)).with_icon("linear-dim"),
            MenuEntry::item("Line to line dimension", dim(DimTool::LineLine)).with_icon("linear-dim"),
            MenuEntry::item("Line to line angular dimension", dim(DimTool::Angle)).with_icon("angular-dim"),
            MenuEntry::item("3 point angular dimension", dim(DimTool::Angle3)).with_icon("angular-dim"),
            MenuEntry::item("Arc length dimension", dim(DimTool::ArcLength)).with_icon("arc-length-dim"),
            MenuEntry::item("Radial dimension (shift+r)", dim(DimTool::Radius)).with_icon("radial-dim"),
            MenuEntry::item("Diameter dimension (shift+d)", dim(DimTool::Diameter)).with_icon("diameter-dim"),
            MenuEntry::item("Foreshorten dimension (shift+h)", dim(DimTool::Foreshorten)).with_icon("linear-dim"),
        ];
        let views_menu = vec![
            MenuEntry::item("Jogged section view", |e, _, cx| e.set_tool(Tool::JoggedSection { parent: None, points: vec![], done: false }, cx)),
            MenuEntry::item("Aligned section view", |e, _, cx| e.set_tool(Tool::AlignedSection { parent: None, center: None, a: None, b: None }, cx)),
            MenuEntry::item("Partial section view", |e, _, cx| e.set_tool(Tool::Section { parent: None, p0: None, line: None, partial: true }, cx)),
            MenuEntry::item("Broken-out section", |e, _, cx| e.set_tool(Tool::BrokenOut { parent: None, points: vec![], done: false }, cx)),
            MenuEntry::Separator,
            MenuEntry::item("Crop view (rectangle)", |e, _, cx| e.set_tool(Tool::Crop { parent: None, first: None, circle: false }, cx)),
            MenuEntry::item("Crop view (circle)", |e, _, cx| e.set_tool(Tool::Crop { parent: None, first: None, circle: true }, cx)),
            MenuEntry::item("Crop view (polygon)", |e, _, cx| e.set_tool(Tool::CropPoly { parent: None, points: vec![], spline: false }, cx)),
            MenuEntry::item("Crop view (spline)", |e, _, cx| e.set_tool(Tool::CropPoly { parent: None, points: vec![], spline: true }, cx)),
            MenuEntry::Separator,
            MenuEntry::item("Datum target", |e, _, cx| e.set_tool(Tool::DatumTarget { anchor: None }, cx)),
        ];
        let table = |kind: TableTool| move || Tool::Table { kind, view: None, origin: None };
        let is_table = |k: TableTool| matches!(t, Tool::Table { kind, .. } if kind == k);
        let mut groups = vec![
            vec![
                // Insert view: Onshape opens "Select a part or assembly" (then
                // the Insert view panel with the view at the cursor).
                tb("dr-insert", "base-view", "Insert view", matches!(t, Tool::InsertView { .. }) || self.insert.chooser.is_some(), |e, _, cx| e.open_insert_chooser(cx)),
                tool("dr-proj", "projected-view", "Projected view (p)", matches!(t, Tool::Projected { .. }), || Tool::Projected { parent: None }),
                tool("dr-aux", "auxiliary-view", "Auxiliary view", matches!(t, Tool::Auxiliary { .. }), || Tool::Auxiliary { parent: None }),
                tool("dr-sect", "section-view", "Section view", matches!(t, Tool::Section { .. } | Tool::JoggedSection { .. }), || Tool::Section { parent: None, p0: None, line: None, partial: false }),
                tool("dr-aligned-sect", "section-view", "Aligned section view", matches!(t, Tool::AlignedSection { .. }), || Tool::AlignedSection { parent: None, center: None, a: None, b: None }),
                tool("dr-brokenout", "broken-out-section", "Broken-out section", matches!(t, Tool::BrokenOut { .. }), || Tool::BrokenOut { parent: None, points: vec![], done: false }),
                tool("dr-detail", "detail-view", "Detail view", matches!(t, Tool::Detail { .. }), || Tool::Detail { parent: None, center: None, radius: None }),
                tool("dr-break", "broken-view", "Break view", matches!(t, Tool::Break { .. }), || Tool::Break { parent: None, first: None, vertical: true }),
                tool("dr-crop", "crop-view", "Crop view", matches!(t, Tool::Crop { .. } | Tool::CropPoly { .. }), || Tool::Crop { parent: None, first: None, circle: false }).with_more("dr-views-more", views_menu),
            ],
            vec![
                tool("dr-dim", "linear-dim", "Dimension (d)", dim_kind.is_some(), || Tool::Dimension { kind: DimTool::Linear, picks: vec![] }).with_more("dr-dim-more", dims_menu),
                tool("dr-ord", "ordinate-dim", "Ordinate dimension", matches!(t, Tool::Ordinate { .. }), || Tool::Ordinate { origin: None, horizontal: None, group: None }),
                tool("dr-holecallout", "hole-callout", "Hole/thread callout", matches!(t, Tool::HoleCallout { .. }), || Tool::HoleCallout { circle: None }),
            ],
            vec![
                tool("dr-datum", "datum", "Datum", matches!(t, Tool::Datum { .. } | Tool::DatumTarget { .. }), || Tool::Datum { anchor: None }),
                tool("dr-gdt", "gdt", "Geometric tolerance", matches!(t, Tool::Gdt), || Tool::Gdt),
                tool("dr-sf", "surface-finish", "Surface finish symbol", matches!(t, Tool::SurfaceFinish), || Tool::SurfaceFinish),
                tool("dr-weld", "weld-symbol", "Weld symbol", matches!(t, Tool::Weld), || Tool::Weld),
            ],
            vec![
                tool("dr-note", "note", "Note (n)", matches!(t, Tool::Note), || Tool::Note),
                tool("dr-balloon", "callout", "Callout", matches!(t, Tool::Balloon { .. }), || Tool::Balloon { anchor: None }),
                tool("dr-table", "table", "Table", is_table(TableTool::General), table(TableTool::General)),
                tool("dr-bomtable", "drawing-bom", "BOM table", is_table(TableTool::Bom), table(TableTool::Bom)),
                tool("dr-revtable", "revision-table", "Revision table", is_table(TableTool::Revision), table(TableTool::Revision)),
                tool("dr-cutlist", "cut-list", "Cut list table", is_table(TableTool::CutList), table(TableTool::CutList)),
                tool("dr-holetable", "hole-table", "Hole table", is_table(TableTool::Hole), table(TableTool::Hole)),
                tool("dr-customtable", "featurescript-table", "Custom table", is_table(TableTool::Custom), table(TableTool::Custom)),
            ],
            vec![
                tool("dr-cline", "centerline", "2 point centerline", matches!(t, Tool::Centerline { .. }), || Tool::Centerline { first: None }),
                tool("dr-ecline", "centerline-edge", "Edge to edge centerline", matches!(t, Tool::EdgeCenterline { .. }), || Tool::EdgeCenterline { first: None }),
                tool("dr-ccline3", "centerline-circle3", "3 point circle centerline", matches!(t, Tool::CircleCenterline { three: true, .. }), || Tool::CircleCenterline { three: true, points: vec![] }),
                tool("dr-ccline2", "centerline-circle2", "2 point circle centerline", matches!(t, Tool::CircleCenterline { three: false, .. }), || Tool::CircleCenterline { three: false, points: vec![] }),
                tool("dr-cmark", "centermark", "Centermark", matches!(t, Tool::Centermark), || Tool::Centermark),
                tool("dr-vsharp", "virtual-sharp", "Virtual sharp", matches!(t, Tool::VirtualSharp { .. }), || Tool::VirtualSharp { first: None }),
            ],
        ];
        groups.extend(self.sketch_groups());
        groups.push(vec![
            tb("dr-dwg-insert", "dwg-insert", "Insert DXF or DWG", false, |e, w, cx| e.insert_cad_file(w, cx)),
            tb("dr-image", "image", "Insert image", matches!(t, Tool::PlaceImage { .. }), |e, w, cx| e.insert_image(w, cx)),
            tb("dr-painter", "format-painter", "Format painter", self.painter.is_some(), |e, w, cx| {
                w.focus(&e.focus, cx);
                e.toggle_format_painter(cx)
            }),
        ]);
        groups.push(vec![
            tb("dr-inspection", "inspection-item", "Inspection item", self.inspection.is_some(), |e, w, cx| {
                w.focus(&e.focus, cx);
                e.open_inspection(cx)
            }),
            tb("dr-inspection-hide", "inspection-hide", "Hide inspection items", self.drawing.props.hide_inspection, |e, _, cx| e.toggle_hide_inspection(cx)),
        ]);
        groups
    }

    /// The sketch tools (live order) and the constraints.
    fn sketch_groups(&self) -> Vec<Vec<TbTool>> {
        let cur = match &self.tool {
            Tool::Sketch { tool, .. } => Some(*tool),
            _ => None,
        };
        let con = match &self.tool {
            Tool::Constrain { kind, .. } => Some(*kind),
            _ => None,
        };
        let sk = |t: SkTool| move || Tool::Sketch { tool: t, pts: vec![], from: None };
        let menu = |items: &[(SkTool, &'static str, &'static str)]| -> Vec<MenuEntry> {
            items.iter().map(|&(t, label, icon)| MenuEntry::item(label, move |e, _, cx| e.set_tool(Tool::Sketch { tool: t, pts: vec![], from: None }, cx)).with_icon(icon)).collect()
        };
        let cons: Vec<MenuEntry> = [
            (SkCon::Horizontal, "Horizontal (h)", "horizontal"),
            (SkCon::Vertical, "Vertical (v)", "vertical"),
            (SkCon::Coincident, "Coincident (i)", "coincident"),
            (SkCon::Perpendicular, "Perpendicular (shift+l)", "perpendicular"),
            (SkCon::Parallel, "Parallel (b)", "parallel"),
            (SkCon::Tangent, "Tangent (t)", "tangent"),
        ]
        .iter()
        .map(|&(k, label, icon)| MenuEntry::item(label, move |e, _, cx| e.set_tool(Tool::Constrain { kind: k, picks: vec![] }, cx)).with_icon(icon))
        .collect();
        vec![
            vec![
                tool("dr-sk-line", "line", "Line (l)", cur == Some(SkTool::Line), sk(SkTool::Line)),
                tool("drg-sk-circle-main", "circle", "Center point circle (c)", matches!(cur, Some(SkTool::Circle | SkTool::Circle3)), sk(SkTool::Circle)).with_more("dr-sk-circle-more", menu(&[(SkTool::Circle, "Center point circle (c)", "circle"), (SkTool::Circle3, "3 point circle", "three-point-circle")])),
                tool("dr-sk-tarc", "tangent-arc", "Tangent arc", cur == Some(SkTool::TangentArc), sk(SkTool::TangentArc)),
                tool("drg-sk-rect-main", "corner-rectangle", "Corner rectangle (g)", matches!(cur, Some(SkTool::CornerRect | SkTool::CenterRect)), sk(SkTool::CornerRect)).with_more("dr-sk-rect-more", menu(&[(SkTool::CornerRect, "Corner rectangle (g)", "corner-rectangle"), (SkTool::CenterRect, "Center point rectangle (r)", "center-rectangle")])),
                tool("drg-sk-spline-main", "spline", "Spline", matches!(cur, Some(SkTool::Spline | SkTool::SplinePoint)), sk(SkTool::Spline)).with_more("dr-sk-spline-more", menu(&[(SkTool::Spline, "Spline", "spline"), (SkTool::SplinePoint, "Spline point", "spline-point")])),
                tool("dr-sk-point", "point-sketch", "Sketch point", cur == Some(SkTool::Point), sk(SkTool::Point)),
                tb("dr-sk-hatch", "hatch-region", "Hatch region", self.hatch_dlg.is_some(), |e, w, cx| {
                    w.focus(&e.focus, cx);
                    e.open_hatch_dialog(None, w, cx)
                }),
                tool("dr-sk-trim", "trim", "Trim (m)", cur == Some(SkTool::Trim), sk(SkTool::Trim)),
            ],
            vec![tool("dr-sk-con", "horizontal", "Horizontal (h)", con.is_some(), || Tool::Constrain { kind: SkCon::Horizontal, picks: vec![] }).with_more("dr-sk-con-more", cons)],
        ]
    }

    pub fn render_toolbar(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let ed = cx.entity();
        let sep = crate::ui::tool_separator;
        let up_to_date = !self.out_of_date && self.sources.len() >= views::referenced_sources(&self.drawing).len();
        let groups = self.toolbar_groups();
        // Fit: undo / redo, update and every category within the window
        // minus the left rail (live 1920 px: all tools shown, ending near
        // x 1745; the categories collapse only when they don't fit).
        let (cell, gap) = (crate::ui::TOOL_CELL, 2.);
        let full: f32 = 2. * cell + gap + (gap + 2. + gap + cell) + groups.iter().map(|g| gap + 2. + g.iter().map(|t| gap + t.width()).sum::<f32>()).sum::<f32>();
        let room = f32::from(window.viewport_size().width) - crate::ui::RAIL_W - 8.;
        let collapse = full > room;
        let mut bar = crate::ui::toolbar()
            .child(crate::ui::undo_redo(&self.doc, cx))
            .child(sep())
            .child(
                // Onshape: the update button is highlighted (#fcda7f, #fab601 border) when out of date.
                crate::ui::tool_button("dr-update", "drawing-update", false, cx)
                    .when(self.out_of_date, |b| b.bg(gpui_kit::rgb(0xfcda7f)).border_2().border_color(gpui_kit::rgb(0xfab601)))
                    .opacity(if up_to_date { 0.4 } else { 1.0 })
                    .tooltip("Update from this workspace (ctrl+q)")
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if !up_to_date {
                            this.update_drawing(cx)
                        }
                    })),
            );
        for (gi, g) in groups.into_iter().enumerate() {
            bar = bar.child(sep());
            if collapse {
                bar = bar.children(collapsed(gi, g, &ed, cx));
            } else {
                for t in g {
                    bar = bar.child(t.render(&ed, cx));
                }
            }
        }
        bar
            .into_any_element()
    }
}
