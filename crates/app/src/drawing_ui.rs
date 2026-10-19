//! Drawing editor chrome: toolbar, status bar, inline property panels
//! (note text, title block, scales, labels). Export: `drawing_export`.

use crate::drawing_view::*;
use crate::theme::{p, with_alpha as theme_with_alpha};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState, TextareaState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::views;
use noshape_model::drawing::*;

/// Parse "1:2", "2:1", "1/2", "0.5" into a scale factor.
pub fn parse_scale(s: &str) -> Option<f64> {
    let s = s.trim();
    let v = if let Some((a, b)) = s.split_once([':', '/']) {
        a.trim().parse::<f64>().ok()? / b.trim().parse::<f64>().ok()?
    } else {
        s.parse::<f64>().ok()?
    };
    (v.is_finite() && v > 0.0).then_some(v)
}

impl DrawingEditor {
    pub(crate) fn tool_btn(&self, id: &'static str, icon: &'static str, tip: &'static str, active: bool, tool: impl Fn() -> Tool + 'static, cx: &mut Context<Self>) -> AnyElement {
        // Onshape toolbar button: 30 px cell, 20 px full-color icon (§3.2).
        crate::ui::tool_button(id, icon, active, cx)
            .tooltip(tip)
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                window.focus(&this.focus, cx);
                this.set_tool(tool(), cx)
            }))
            .into_any_element()
    }

    pub fn render_toolbar(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let ed = cx.entity();
        let sep = crate::ui::tool_separator;
        let t = self.tool.clone();
        // Insert view: Onshape opens "Select a part or assembly" (then the
        // Insert view panel with the view at the cursor).
        let insert = crate::ui::tool_button("dr-insert", "base-view", matches!(t, Tool::InsertView { .. }) || self.insert.chooser.is_some(), cx)
            .tooltip("Insert view")
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_insert_chooser(cx)));
        let dim_kind = if let Tool::Dimension { kind, .. } = t { Some(kind) } else { None };
        let ed2 = ed.clone();
        // Onshape's Dimension dropdown (live, in order). The 2 point, point to
        // line and line to line tools use the smart linear dimension's picks.
        let dims_menu = vec![
            MenuEntry::item("Dimension (d)", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Linear, picks: vec![] }, cx)).with_icon("linear-dim"),
            MenuEntry::item("Chamfer dimension", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Chamfer, picks: vec![] }, cx)).with_icon("chamfer-dim"),
            MenuEntry::item("Maximum or minimum dimension (ctrl+m)", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::MaxMin, picks: vec![] }, cx)).with_icon("driven-dimension"),
            MenuEntry::item("2 point linear dimension", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Linear, picks: vec![] }, cx)).with_icon("linear-dim"),
            MenuEntry::item("Point to line dimension", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Linear, picks: vec![] }, cx)).with_icon("linear-dim"),
            MenuEntry::item("Line to line dimension", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Linear, picks: vec![] }, cx)).with_icon("linear-dim"),
            MenuEntry::item("Line to line angular dimension", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Angle, picks: vec![] }, cx)).with_icon("angular-dim"),
            MenuEntry::item("3 point angular dimension", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Angle3, picks: vec![] }, cx)).with_icon("angular-dim"),
            MenuEntry::item("Arc length dimension", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::ArcLength, picks: vec![] }, cx)).with_icon("arc-length-dim"),
            MenuEntry::item("Radial dimension (shift+r)", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Radius, picks: vec![] }, cx)).with_icon("radial-dim"),
            MenuEntry::item("Diameter dimension (shift+d)", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Diameter, picks: vec![] }, cx)).with_icon("diameter-dim"),
            MenuEntry::item("Foreshorten dimension (shift+h)", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Foreshorten, picks: vec![] }, cx)).with_icon("linear-dim"),
        ];
        let ed5 = ed.clone();
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
        let ed3 = ed.clone();
        let sheet_menu = {
            let mut v = vec![
                MenuEntry::Submenu("Template".into(), self.template_entries()),
                MenuEntry::Submenu("Sheet size".into(), self.sheet_size_entries()),
                MenuEntry::Submenu("Title block".into(), self.title_style_entries()),
            ];
            let cur = self.current_sheet().scale;
            let mut scales = Vec::new();
            for s in [0.1, 0.2, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0] {
                scales.push(MenuEntry::check(sheet::format_scale(s), (cur - s).abs() < 1e-12, move |e, _, cx| e.edit_sheet("Sheet scale", cx, |sh| sh.scale = s)));
            }
            scales.push(MenuEntry::item("Custom…", |e, w, cx| e.open_scale_panel(None, w, cx)));
            v.push(MenuEntry::Submenu("Sheet scale".into(), scales));
            let proj = self.drawing.projection;
            v.push(MenuEntry::check("Third angle projection", proj == Projection::ThirdAngle, |e, _, cx| e.edit("Projection", cx, |d| d.projection = Projection::ThirdAngle)));
            v.push(MenuEntry::check("First angle projection", proj == Projection::FirstAngle, |e, _, cx| e.edit("Projection", cx, |d| d.projection = Projection::FirstAngle)));
            let mm = self.drawing.units_mm;
            v.push(MenuEntry::check("Millimeters", mm, |e, _, cx| e.edit("Units", cx, |d| d.units_mm = true)));
            v.push(MenuEntry::check("Inches", !mm, |e, _, cx| e.edit("Units", cx, |d| d.units_mm = false)));
            v.push(MenuEntry::Separator);
            v.push(MenuEntry::item("Edit title block…", |e, w, cx| e.open_title_panel(w, cx)));
            v
        };
        let up_to_date = !self.out_of_date && self.sources.len() >= views::referenced_sources(&self.drawing).len();
        // Onshape's drawing toolbar, in its order (live DOM, 1.222): update |
        // views | dimensions | symbols | notes and tables | centerlines.
        // Sheet setup and zoom to fit (elsewhere in Onshape) sit at
        // the right end.
        crate::ui::toolbar()
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
            )
            .child(sep())
            .child(insert)
            .child(self.tool_btn("dr-proj", "projected-view", "Projected view (p)", matches!(t, Tool::Projected { .. }), || Tool::Projected { parent: None }, cx))
            .child(self.tool_btn("dr-aux", "auxiliary-view", "Auxiliary view", matches!(t, Tool::Auxiliary { .. }), || Tool::Auxiliary { parent: None }, cx))
            .child(self.tool_btn("dr-sect", "section-view", "Section view", matches!(t, Tool::Section { .. } | Tool::JoggedSection { .. }), || Tool::Section { parent: None, p0: None, line: None, partial: false }, cx))
            .child(self.tool_btn("dr-aligned-sect", "section-view", "Aligned section view", matches!(t, Tool::AlignedSection { .. }), || Tool::AlignedSection { parent: None, center: None, a: None, b: None }, cx))
            .child(self.tool_btn("dr-brokenout", "broken-out-section", "Broken-out section", matches!(t, Tool::BrokenOut { .. }), || Tool::BrokenOut { parent: None, points: vec![], done: false }, cx))
            .child(self.tool_btn("dr-detail", "detail-view", "Detail view", matches!(t, Tool::Detail { .. }), || Tool::Detail { parent: None, center: None, radius: None }, cx))
            .child(self.tool_btn("dr-break", "broken-view", "Break view", matches!(t, Tool::Break { .. }), || Tool::Break { parent: None, first: None, vertical: true }, cx))
            .child(crate::ui::tool_group(
                "drg-crop",
                crate::ui::split_tool_button("dr-crop", "crop-view", matches!(t, Tool::Crop { .. } | Tool::CropPoly { .. }), cx).tooltip("Crop view").on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    window.focus(&this.focus, cx);
                    this.set_tool(Tool::Crop { parent: None, first: None, circle: false }, cx)
                })),
                crate::ui::tool_caret("dr-views-more", cx).tooltip("More view tools").on_click(move |e, w, cx| open_tool_menu(&views_menu, &ed5, e, w, cx)),
            ))
            .child(sep())
            .child(crate::ui::tool_group(
                "drg-dim",
                crate::ui::split_tool_button("dr-dim", "linear-dim", dim_kind.is_some(), cx).tooltip("Dimension (d)").on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    window.focus(&this.focus, cx);
                    this.set_tool(Tool::Dimension { kind: DimTool::Linear, picks: vec![] }, cx)
                })),
                crate::ui::tool_caret("dr-dim-more", cx).tooltip("Dimension (d)").on_click(move |e, w, cx| open_tool_menu(&dims_menu, &ed2, e, w, cx)),
            ))
            .child(self.tool_btn("dr-ord", "ordinate-dim", "Ordinate dimension", matches!(t, Tool::Ordinate { .. }), || Tool::Ordinate { origin: None, horizontal: None, group: None }, cx))
            .child(self.tool_btn("dr-holecallout", "hole-callout", "Hole/thread callout", matches!(t, Tool::HoleCallout { .. }), || Tool::HoleCallout { circle: None }, cx))
            .child(sep())
            .child(self.tool_btn("dr-datum", "datum", "Datum", matches!(t, Tool::Datum { .. } | Tool::DatumTarget { .. }), || Tool::Datum { anchor: None }, cx))
            .child(self.tool_btn("dr-gdt", "gdt", "Geometric tolerance", matches!(t, Tool::Gdt), || Tool::Gdt, cx))
            .child(self.tool_btn("dr-sf", "surface-finish", "Surface finish symbol", matches!(t, Tool::SurfaceFinish), || Tool::SurfaceFinish, cx))
            .child(sep())
            .child(self.tool_btn("dr-note", "note", "Note (n)", matches!(t, Tool::Note), || Tool::Note, cx))
            .child(self.tool_btn("dr-balloon", "callout", "Callout", matches!(t, Tool::Balloon { .. }), || Tool::Balloon { anchor: None }, cx))
            .child(self.tool_btn("dr-table", "table", "Table", matches!(t, Tool::Table { kind: TableTool::General, .. }), || Tool::Table { kind: TableTool::General, view: None, origin: None }, cx))
            .child(self.tool_btn("dr-bomtable", "drawing-bom", "BOM table", matches!(t, Tool::Table { kind: TableTool::Bom, .. }), || Tool::Table { kind: TableTool::Bom, view: None, origin: None }, cx))
            .child(self.tool_btn("dr-revtable", "revision-table", "Revision table", matches!(t, Tool::Table { kind: TableTool::Revision, .. }), || Tool::Table { kind: TableTool::Revision, view: None, origin: None }, cx))
            .child(self.tool_btn("dr-cutlist", "cut-list", "Cut list table", matches!(t, Tool::Table { kind: TableTool::CutList, .. }), || Tool::Table { kind: TableTool::CutList, view: None, origin: None }, cx))
            .child(self.tool_btn("dr-holetable", "hole-table", "Hole table", matches!(t, Tool::Table { kind: TableTool::Hole, .. }), || Tool::Table { kind: TableTool::Hole, view: None, origin: None }, cx))
            .child(sep())
            .child(self.tool_btn("dr-cline", "centerline", "2 point centerline", matches!(t, Tool::Centerline { .. }), || Tool::Centerline { first: None }, cx))
            .child(self.tool_btn("dr-ecline", "centerline-edge", "Edge to edge centerline", matches!(t, Tool::EdgeCenterline { .. }), || Tool::EdgeCenterline { first: None }, cx))
            .child(self.tool_btn("dr-ccline3", "centerline-circle3", "3 point circle centerline", matches!(t, Tool::CircleCenterline { three: true, .. }), || Tool::CircleCenterline { three: true, points: vec![] }, cx))
            .child(self.tool_btn("dr-ccline2", "centerline-circle2", "2 point circle centerline", matches!(t, Tool::CircleCenterline { three: false, .. }), || Tool::CircleCenterline { three: false, points: vec![] }, cx))
            .child(self.tool_btn("dr-cmark", "centermark", "Centermark", matches!(t, Tool::Centermark), || Tool::Centermark, cx))
            .child(self.tool_btn("dr-vsharp", "virtual-sharp", "Virtual sharp", matches!(t, Tool::VirtualSharp { .. }), || Tool::VirtualSharp { first: None }, cx))
            .children(self.sketch_toolbar(cx))
            .child(sep())
            .child(
                crate::ui::tool_button("dr-dwg-insert", "dwg-insert", false, cx)
                    .tooltip("Insert DXF or DWG")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.insert_cad_file(window, cx))),
            )
            .child(
                crate::ui::tool_button("dr-image", "image", matches!(t, Tool::PlaceImage { .. }), cx)
                    .tooltip("Insert image")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.insert_image(window, cx))),
            )
            .child(
                crate::ui::tool_button("dr-painter", "format-painter", self.painter.is_some(), cx)
                    .tooltip("Format painter")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        window.focus(&this.focus, cx);
                        this.toggle_format_painter(cx)
                    })),
            )
            .child(sep())
            .child(
                crate::ui::tool_button("dr-inspection", "inspection-item", self.inspection.is_some(), cx)
                    .tooltip("Inspection item")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        window.focus(&this.focus, cx);
                        this.open_inspection(cx)
                    })),
            )
            .child(
                crate::ui::tool_button("dr-inspection-hide", "inspection-hide", self.drawing.props.hide_inspection, cx)
                    .tooltip("Hide inspection items")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_hide_inspection(cx))),
            )
            .child(div().flex_1())
            .child(
                crate::ui::tool_button("dr-title", "title-block", false, cx)
                    .tooltip("Edit title block")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_title_panel(window, cx))),
            )
            .child(crate::ui::tool_group("drg-sheet", crate::ui::split_tool_button("dr-sheet", "sheet", false, cx).tooltip("Sheet properties"), crate::ui::tool_caret("dr-sheet-more", cx).tooltip("Sheet properties").on_click(move |e, w, cx| open_tool_menu(&sheet_menu, &ed3, e, w, cx))))
            .child(
                crate::ui::tool_button("dr-fit", "zoom-fit", false, cx)
                    .tooltip("Zoom to fit (f)")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.zoom_to_fit(cx))),
            )
            .into_any_element()
    }

    pub fn render_status(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let mut parts: Vec<String> = Vec::new();
        if let Some(pr) = self.prompt() {
            parts.push(format!("{}: {pr}", self.tool.name()));
        }
        if self.computing {
            parts.push("computing views…".into());
        }
        if self.out_of_date {
            parts.push("Out of date — Update (Ctrl+Q)".into());
        }
        if let Some(b) = &self.built
            && !b.dangling.is_empty()
        {
            parts.push(format!("{} dangling annotation(s)", b.dangling.len()));
        }
        // Messages ("Exported …", errors) are Onshape's message bubble at the
        // top center (os_ui::message_bubble), closed with its ×.
        let bubble = self.status.clone().map(|msg| {
            let kind = if msg.contains("failed") || msg.contains("Error") { crate::ui::AlertKind::Error } else { crate::ui::AlertKind::Info };
            let ed = cx.entity();
            div().absolute().top(px(8.)).left_0().right_0().flex().justify_center().child(crate::os_ui::message_bubble("dr-toast", kind, msg, move |_, _, cx| {
                ed.update(cx, |this, cx| {
                    this.status = None;
                    cx.notify();
                })
            }, cx))
        });
        // Onshape has no status bar: the tool prompt and drawing state show as
        // quiet text at the bottom-left of the drawing area.
        let quiet = (!parts.is_empty()).then(|| {
            div()
                .absolute()
                .bottom(px(6.))
                .left(px(10.))
                .px(px(6.))
                .py(px(2.))
                .rounded(px(3.))
                .bg(theme_with_alpha(pal.panel, 0.85))
                .text_size(px(12.))
                .text_color(pal.text_muted)
                .child(parts.join("    "))
        });
        div().absolute().inset_0().children(bubble).children(quiet).into_any_element()
    }

    // ------------------------------------------------------------ panels

    pub fn render_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        let panel = self.panel.as_ref()?;
        let (title, rows): (&str, Vec<AnyElement>) = match panel {
            Panel::Note { edit, .. } => {
                let title = if edit.is_some() { "Edit note" } else { "Note" };
                (title, self.note_panel_rows(cx))
            }
            Panel::Dimension { .. } | Panel::Gdt { .. } | Panel::Datum { .. } | Panel::SurfaceFinish { .. } | Panel::Cell { .. } | Panel::SheetName { .. } | Panel::Rotation { .. } | Panel::NewTable { .. } => self.extra_panel_rows(cx)?,
            Panel::BrokenOut { .. } | Panel::CutAngle { .. } | Panel::CellFormat { .. } | Panel::CellResize { .. } | Panel::DatumTarget { .. } => self.more_panel_rows(cx)?,
            Panel::BomCell { .. } => ("BOM cell", self.bom_cell_rows(cx)),
            Panel::TitleBlock { inputs } => (
                "Title block (fields like {part.name}, {sheet.scale} allowed)",
                inputs
                    .iter()
                    .map(|(k, st)| h_flex().gap_2().child(div().w(px(90.)).text_xs().text_color(pal.text_muted).child(k.clone())).child(Input::new(st).xsmall().w(px(220.))).into_any_element())
                    .collect(),
            ),
            Panel::Scale { view, input } => (
                if view.is_some() { "View scale" } else { "Sheet scale" },
                vec![
                    div().text_xs().text_color(pal.text_faint).child(if view.is_some() { "e.g. 1:2, 2:1 (empty = parent/sheet scale)" } else { "e.g. 1:2, 2:1" }).into_any_element(),
                    Input::new(input).small().w(px(160.)).into_any_element(),
                ],
            ),
        };
        Some(
            v_flex()
                .id("dr-panel")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .p_3()
                .gap_2()
                .bg(pal.panel_raised)
                .border_1()
                .border_color(pal.border_strong)
                .rounded_md()
                .shadow_md()
                .text_sm()
                .text_color(pal.text)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                .children(rows)
                .child(
                    h_flex()
                        .gap_2()
                        .justify_end()
                        .child(Button::new("dr-panel-cancel").label("Cancel").ghost().small().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.panel = None;
                            cx.notify();
                        })))
                        .child(Button::new("dr-panel-ok").label("OK").primary().small().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.apply_panel(cx)))),
                )
                .into_any_element(),
        )
    }

    pub fn textarea(&mut self, value: &str, rows: usize, window: &mut Window, cx: &mut Context<Self>) -> Entity<TextareaState> {
        let st = cx.new(|cx| TextareaState::new(window, cx).rows(rows).default_value(value.to_string()));
        st.update(cx, |s, cx| s.focus(window, cx));
        st
    }

    pub fn input(&mut self, value: &str, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        let st = cx.new(|cx| InputState::new(window, cx).default_value(value.to_string()));
        let sub = cx.subscribe_in(&st, window, |this: &mut Self, _, ev: &InputEvent, _, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.apply_panel(cx);
            }
        });
        self._subs.push(sub);
        st
    }

    pub fn focus_input(st: &Entity<InputState>, window: &mut Window, cx: &mut Context<Self>) {
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
    }

    pub fn open_note_panel(&mut self, edit: Option<u64>, pos: [f64; 2], window: &mut Window, cx: &mut Context<Self>) {
        let note = edit
            .and_then(|id| self.drawing.sheets[self.sheet].annotations.iter().find(|a| a.id == id))
            .and_then(|a| if let AnnotationKind::Note { text, height_mm, style } = &a.kind { Some((text.clone(), *height_mm, style.clone())) } else { None });
        // New notes: Drawing properties › Annotations › Notes text height, else All annotations'.
        let default_h = self.drawing.props.ann_type(noshape_model::drawing::sheet::AnnType::Notes).height.unwrap_or(self.drawing.props.ann_text_height);
        let (text, h, mut style) = note.unwrap_or((String::new(), default_h, Default::default()));
        if edit.is_none()
            && let Some(l) = self.note_leader.take()
        {
            style.leaders.push(l);
        }
        let input = self.textarea(&text, 5, window, cx);
        let height = self.input(&format!("{} mm", noshape_model::drawing::format::format_number(h, None, 2)), window, cx);
        self.panel = Some(Panel::Note { input, edit, pos, height, style });
        cx.notify();
    }

    pub fn open_title_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let fields = self.drawing.sheets[self.sheet].title_block.fields.clone();
        let inputs: Vec<(String, Entity<InputState>)> = fields.iter().map(|(k, v)| (k.clone(), self.input(v, window, cx))).collect();
        if let Some((_, first)) = inputs.first() {
            Self::focus_input(first, window, cx);
        }
        self.panel = Some(Panel::TitleBlock { inputs });
        cx.notify();
    }

    pub fn open_scale_panel(&mut self, view: Option<ViewId>, window: &mut Window, cx: &mut Context<Self>) {
        let sheet = &self.drawing.sheets[self.sheet];
        let cur = match view {
            Some(v) => sheet.view(v).and_then(|v| v.scale).map(sheet::format_scale).unwrap_or_default(),
            None => sheet::format_scale(sheet.scale),
        };
        let input = self.input(&cur, window, cx);
        Self::focus_input(&input, window, cx);
        self.panel = Some(Panel::Scale { view, input });
        cx.notify();
    }

    pub fn apply_panel(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = self.panel.take() else { return };
        match panel {
            Panel::Note { input, edit, pos, height, style } => {
                let text = input.read(cx).value().to_string();
                let h = height.read(cx).value().trim().trim_end_matches("mm").trim().parse::<f64>().ok().filter(|v| *v > 0.1);
                if text.trim().is_empty() {
                    if let Some(id) = edit {
                        self.edit_sheet("Delete note", cx, |s| s.annotations.retain(|a| a.id != id));
                    }
                } else if let Some(id) = edit {
                    self.edit_sheet("Edit note", cx, |s| {
                        if let Some(AnnotationKind::Note { text: t, height_mm, style: st }) = s.annotations.iter_mut().find(|a| a.id == id).map(|a| &mut a.kind) {
                            *t = text;
                            *st = style;
                            if let Some(h) = h {
                                *height_mm = h;
                            }
                        }
                    });
                } else {
                    let id = self.current_sheet().next_annotation_id();
                    // New notes take Drawing properties › Annotations › Text height.
                    // Notes › Text height, else All annotations'.
                    let default_h = self.drawing.props.ann_type(noshape_model::drawing::sheet::AnnType::Notes).height.unwrap_or(self.drawing.props.ann_text_height);
                    let height_mm = h.unwrap_or(default_h);
                    // The text height becomes the new Notes default.
                    if h.is_some_and(|h| (h - default_h).abs() > 1e-9) {
                        self.drawing.props.ann_type_mut(noshape_model::drawing::sheet::AnnType::Notes).height = Some(height_mm);
                    }
                    self.edit_sheet("Note", cx, |s| s.annotations.push(Annotation { id, kind: AnnotationKind::Note { text, height_mm, style }, position: pos, ..Default::default() }));
                    self.tool = Tool::Select;
                }
            }
            Panel::TitleBlock { inputs } => {
                let values: Vec<(String, String)> = inputs.iter().map(|(k, st)| (k.clone(), st.read(cx).value().to_string())).collect();
                self.edit_sheet("Edit title block", cx, |s| s.title_block.fields = values);
            }
            Panel::Scale { view, input } => {
                let text = input.read(cx).value().to_string();
                match (view, parse_scale(&text)) {
                    (Some(v), s) if s.is_some() || text.trim().is_empty() => self.edit_view("View scale", v, cx, |dv| dv.scale = s),
                    (None, Some(s)) => self.edit_sheet("Sheet scale", cx, |sh| sh.scale = s),
                    _ => {
                        self.status = Some(format!("Invalid scale \"{text}\"").into());
                    }
                }
            }
            other => self.apply_extra_panel(other, cx),
        }
        self.update_preview(cx);
        cx.notify();
    }

}

#[cfg(test)]
mod tests {
    use super::parse_scale;

    #[test]
    fn scales() {
        assert_eq!(parse_scale("1:2"), Some(0.5));
        assert_eq!(parse_scale("2:1"), Some(2.0));
        assert_eq!(parse_scale("0.25"), Some(0.25));
        assert_eq!(parse_scale("x"), None);
    }
}
