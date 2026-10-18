//! Drawing editor chrome: toolbar, status bar, inline property panels
//! (note text, title block, scales, labels) and export (PDF/SVG/DXF).

use crate::assets::icon_path;
use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::{Disableable as _, Icon, IconName, Selectable as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::document::TabContent;
use noshape_model::drawing::build::render_sheet;
use noshape_model::drawing::views::{self, ViewCache};
use noshape_model::drawing::*;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Pdf,
    Svg,
    Dxf,
}

impl ExportFormat {
    pub fn ext(self) -> &'static str {
        match self {
            ExportFormat::Pdf => "pdf",
            ExportFormat::Svg => "svg",
            ExportFormat::Dxf => "dxf",
        }
    }
}

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
    fn tool_btn(&self, id: &'static str, icon: &'static str, tip: &'static str, active: bool, tool: impl Fn() -> Tool + 'static, cx: &mut Context<Self>) -> AnyElement {
        Button::new(id)
            .icon(Icon::empty().path(icon_path(icon)))
            .ghost()
            .small()
            .selected(active)
            .tooltip(tip)
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                window.focus(&this.focus, cx);
                this.set_tool(tool(), cx)
            }))
            .into_any_element()
    }

    fn insert_view_entries(&self, cx: &App) -> Vec<MenuEntry> {
        let doc = self.doc.read(cx);
        let mut out = Vec::new();
        for t in doc.session.doc.ordered_tabs() {
            let TabContent::PartStudio(_) = &t.content else { continue };
            let tab = t.id;
            let mut orient = Vec::new();
            for o in [ViewOrientation::Front, ViewOrientation::Top, ViewOrientation::Right, ViewOrientation::Left, ViewOrientation::Back, ViewOrientation::Bottom, ViewOrientation::Isometric] {
                orient.push(MenuEntry::item(format!("{o:?}"), move |e, _, cx| e.set_tool(Tool::InsertView { source: tab, orientation: o, part: None }, cx)));
            }
            // Single parts (when the studio has been regenerated).
            if let Some(src) = self.sources.get(&tab) {
                let parts = views::view_bodies(&src.context, None);
                if parts.len() > 1 {
                    orient.push(MenuEntry::Separator);
                    for b in parts {
                        let id = b.id.clone();
                        orient.push(MenuEntry::item(format!("{} (Front)", b.name), move |e, _, cx| {
                            e.set_tool(Tool::InsertView { source: tab, orientation: ViewOrientation::Front, part: Some(id.clone()) }, cx)
                        }));
                    }
                }
            }
            out.push(MenuEntry::Submenu(t.name.clone().into(), orient));
        }
        if out.is_empty() {
            out.push(MenuEntry::item("No Part Studios in this document", |_, _, _| {}));
        }
        out
    }

    pub fn render_toolbar(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let ed = cx.entity();
        let sep = || div().w(px(1.)).h(px(22.)).mx_1().bg(pal.border);
        let t = self.tool.clone();
        let insert_entries = self.insert_view_entries(cx);
        let ed1 = ed.clone();
        let insert = Button::new("dr-insert")
            .icon(Icon::empty().path(icon_path("base-view")))
            .label("Insert view")
            .ghost()
            .small()
            .selected(matches!(t, Tool::InsertView { .. }))
            .dropdown_caret(true)
            .tooltip("Insert view of a Part Studio")
            .dropdown_menu(move |m, w, cx| fill_menu(m, &insert_entries, &ed1, w, cx));
        let dim_kind = if let Tool::Dimension { kind, .. } = t { Some(kind) } else { None };
        let ed2 = ed.clone();
        let dims_menu = vec![
            MenuEntry::item("Linear (smart)", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Linear, picks: vec![] }, cx)),
            MenuEntry::item("Aligned", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Aligned, picks: vec![] }, cx)),
            MenuEntry::item("Horizontal", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Horizontal, picks: vec![] }, cx)),
            MenuEntry::item("Vertical", |e, _, cx| e.set_tool(Tool::Dimension { kind: DimTool::Vertical, picks: vec![] }, cx)),
        ];
        let ed3 = ed.clone();
        let sheet_menu = {
            let mut v = vec![MenuEntry::Submenu("Sheet size".into(), self.sheet_size_entries())];
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
        let ed4 = ed.clone();
        let export_menu = vec![
            MenuEntry::item("PDF…", |e, w, cx| e.export(ExportFormat::Pdf, w, cx)),
            MenuEntry::item("SVG…", |e, w, cx| e.export(ExportFormat::Svg, w, cx)),
            MenuEntry::item("DXF…", |e, w, cx| e.export(ExportFormat::Dxf, w, cx)),
        ];
        h_flex()
            .h(px(40.))
            .px_2()
            .gap_1()
            .bg(pal.panel)
            .border_b_1()
            .border_color(pal.border)
            .overflow_hidden()
            .child(
                Button::new("dr-update")
                    .icon(IconName::RefreshCw)
                    .ghost()
                    .small()
                    .when(self.out_of_date, |b| b.primary())
                    .disabled(!self.out_of_date && self.sources.len() >= views::referenced_studios(&self.drawing).len())
                    .tooltip("Update drawing (Ctrl+Q)")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.update_drawing(cx))),
            )
            .child(sep())
            .child(insert)
            .child(self.tool_btn("dr-proj", "projected-view", "Projected view (P)", matches!(t, Tool::Projected { .. }), || Tool::Projected { parent: None }, cx))
            .child(self.tool_btn("dr-aux", "auxiliary-view", "Auxiliary view", matches!(t, Tool::Auxiliary { .. }), || Tool::Auxiliary { parent: None }, cx))
            .child(self.tool_btn("dr-sect", "section-view", "Section view", matches!(t, Tool::Section { .. }), || Tool::Section { parent: None, p0: None, line: None }, cx))
            .child(self.tool_btn("dr-detail", "detail-view", "Detail view", matches!(t, Tool::Detail { .. }), || Tool::Detail { parent: None, center: None, radius: None }, cx))
            .child(sep())
            .child(
                h_flex()
                    .child(self.tool_btn("dr-dim", "linear-dim", "Dimension (D)", matches!(dim_kind, Some(DimTool::Linear | DimTool::Aligned | DimTool::Horizontal | DimTool::Vertical)), || Tool::Dimension { kind: DimTool::Linear, picks: vec![] }, cx))
                    .child(Button::new("dr-dim-more").icon(Icon::new(IconName::ChevronDown).xsmall()).ghost().xsmall().w(px(14.)).dropdown_menu(move |m, w, cx| fill_menu(m, &dims_menu, &ed2, w, cx))),
            )
            .child(self.tool_btn("dr-rad", "radial-dim", "Radial dimension (Shift+R)", dim_kind == Some(DimTool::Radius), || Tool::Dimension { kind: DimTool::Radius, picks: vec![] }, cx))
            .child(self.tool_btn("dr-dia", "diameter-dim", "Diameter dimension (Shift+D)", dim_kind == Some(DimTool::Diameter), || Tool::Dimension { kind: DimTool::Diameter, picks: vec![] }, cx))
            .child(self.tool_btn("dr-ang", "angular-dim", "Angular dimension", dim_kind == Some(DimTool::Angle), || Tool::Dimension { kind: DimTool::Angle, picks: vec![] }, cx))
            .child(sep())
            .child(self.tool_btn("dr-note", "note", "Note (N)", matches!(t, Tool::Note), || Tool::Note, cx))
            .child(self.tool_btn("dr-balloon", "balloon", "Callout", matches!(t, Tool::Balloon { .. }), || Tool::Balloon { anchor: None }, cx))
            .child(self.tool_btn("dr-cmark", "centermark", "Centermark", matches!(t, Tool::Centermark), || Tool::Centermark, cx))
            .child(self.tool_btn("dr-cline", "centerline", "2 point centerline", matches!(t, Tool::Centerline { .. }), || Tool::Centerline { first: None }, cx))
            .child(sep())
            .child(
                Button::new("dr-title")
                    .icon(Icon::empty().path(icon_path("title-block")))
                    .ghost()
                    .small()
                    .tooltip("Edit title block")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.open_title_panel(window, cx))),
            )
            .child(
                Button::new("dr-sheet")
                    .icon(Icon::empty().path(icon_path("sheet")))
                    .label("Sheet")
                    .ghost()
                    .small()
                    .dropdown_caret(true)
                    .dropdown_menu(move |m, w, cx| fill_menu(m, &sheet_menu, &ed3, w, cx)),
            )
            .child(div().flex_1())
            .child(
                Button::new("dr-fit")
                    .icon(Icon::empty().path(icon_path("zoom-fit")))
                    .ghost()
                    .small()
                    .tooltip("Zoom to fit (F)")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.zoom_to_fit(cx))),
            )
            .child(
                Button::new("dr-export")
                    .icon(Icon::empty().path(icon_path("export")))
                    .label("Export")
                    .ghost()
                    .small()
                    .dropdown_caret(true)
                    .dropdown_menu(move |m, w, cx| fill_menu(m, &export_menu, &ed4, w, cx)),
            )
            .into_any_element()
    }

    pub fn render_status(&mut self, _cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let sheet = self.current_sheet();
        let mut parts: Vec<String> = vec![format!("{} · {:?} · {}", sheet.name, sheet.size, sheet::format_scale(sheet.scale))];
        if let Some(pr) = self.prompt() {
            parts.push(format!("{}: {pr}", self.tool.name()));
        }
        if let Some(s) = &self.status {
            parts.push(s.to_string());
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
        parts.push(format!("x {:.1}  y {:.1} mm", self.cursor[0], self.cursor[1]));
        div()
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .h(px(24.))
            .px_3()
            .flex()
            .items_center()
            .bg(pal.panel)
            .border_t_1()
            .border_color(pal.border)
            .text_xs()
            .text_color(pal.text_muted)
            .child(parts.join("    "))
            .into_any_element()
    }

    // ------------------------------------------------------------ panels

    pub fn render_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        let panel = self.panel.as_ref()?;
        let (title, rows): (&str, Vec<AnyElement>) = match panel {
            Panel::Note { input, edit, .. } => (
                if edit.is_some() { "Edit note" } else { "Note" },
                vec![
                    div().text_xs().text_color(pal.text_faint).child("Text (use \\n for a new line)").into_any_element(),
                    Input::new(input).small().w(px(320.)).into_any_element(),
                ],
            ),
            Panel::TitleBlock { inputs } => (
                "Title block",
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
            Panel::Label { input, .. } => ("View label", vec![Input::new(input).small().w(px(220.)).into_any_element()]),
        };
        Some(
            v_flex()
                .id("dr-panel")
                .absolute()
                .top_2()
                .left_2()
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

    fn input(&mut self, value: &str, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        let st = cx.new(|cx| InputState::new(window, cx).default_value(value.to_string()));
        let sub = cx.subscribe_in(&st, window, |this: &mut Self, _, ev: &InputEvent, _, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.apply_panel(cx);
            }
        });
        self._subs.push(sub);
        st
    }

    fn focus_input(st: &Entity<InputState>, window: &mut Window, cx: &mut Context<Self>) {
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
    }

    pub fn open_note_panel(&mut self, edit: Option<u64>, pos: [f64; 2], window: &mut Window, cx: &mut Context<Self>) {
        let text = edit
            .and_then(|id| self.drawing.sheets[self.sheet].annotations.iter().find(|a| a.id == id))
            .and_then(|a| if let AnnotationKind::Note { text, .. } = &a.kind { Some(text.replace('\n', "\\n")) } else { None })
            .unwrap_or_default();
        let input = self.input(&text, window, cx);
        Self::focus_input(&input, window, cx);
        self.panel = Some(Panel::Note { input, edit, pos });
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

    pub fn open_label_panel(&mut self, view: ViewId, window: &mut Window, cx: &mut Context<Self>) {
        let cur = self.drawing.sheets[self.sheet].view(view).and_then(|v| v.label.clone()).unwrap_or_default();
        let input = self.input(&cur, window, cx);
        Self::focus_input(&input, window, cx);
        self.panel = Some(Panel::Label { view, input });
        cx.notify();
    }

    pub fn apply_panel(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = self.panel.take() else { return };
        match panel {
            Panel::Note { input, edit, pos } => {
                let text = input.read(cx).value().to_string().replace("\\n", "\n");
                if text.trim().is_empty() {
                    if let Some(id) = edit {
                        self.edit_sheet("Delete note", cx, |s| s.annotations.retain(|a| a.id != id));
                    }
                } else if let Some(id) = edit {
                    self.edit_sheet("Edit note", cx, |s| {
                        if let Some(AnnotationKind::Note { text: t, .. }) = s.annotations.iter_mut().find(|a| a.id == id).map(|a| &mut a.kind) {
                            *t = text;
                        }
                    });
                } else {
                    let id = self.current_sheet().next_annotation_id();
                    self.edit_sheet("Note", cx, |s| s.annotations.push(Annotation { id, kind: AnnotationKind::Note { text, height_mm: 3.5 }, position: pos, refs: vec![] }));
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
            Panel::Label { view, input } => {
                let text = input.read(cx).value().to_string();
                self.edit_view("View label", view, cx, |v| v.label = (!text.trim().is_empty()).then_some(text));
            }
        }
        cx.notify();
    }

    // ------------------------------------------------------------ export

    /// Export the current sheet (exact hidden lines) with a native save
    /// dialog; without a usable dialog (no desktop portal) the file is
    /// written next to the user's documents.
    pub fn export(&mut self, format: ExportFormat, window: &mut Window, cx: &mut Context<Self>) {
        let drawing = self.drawing.clone();
        let si = self.sheet;
        let sources = self.sources.clone();
        let cache = self.cache.clone();
        let (doc_name, tab_name) = {
            let d = self.doc.read(cx);
            (d.name(), d.session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_else(|| "Drawing".into()))
        };
        let file_name = format!("{doc_name} - {tab_name}.{}", format.ext());
        self.status = Some(format!("Exporting {}…", format.ext().to_uppercase()).into());
        cx.notify();
        let _ = window;
        cx.spawn(async move |this, cx| {
            let bytes = cx
                .background_spawn(async move {
                    let built = render_sheet(&drawing, si, &sources, &cache as &Arc<ViewCache>, true)?;
                    Some(match format {
                        ExportFormat::Pdf => export::to_pdf(&built.geometry),
                        ExportFormat::Svg => export::to_svg(&built.geometry).into_bytes(),
                        ExportFormat::Dxf => export::to_dxf(&built.geometry).into_bytes(),
                    })
                })
                .await;
            let Some(bytes) = bytes else { return };
            let name = file_name.clone();
            let result: Result<Option<PathBuf>, String> = cx
                .background_spawn(async move {
                    let fallback = || crate::files::documents_dir().join(&name);
                    let no_dialog = std::env::var_os("NOSHAPE_NO_FILE_DIALOG").is_some() || (cfg!(target_os = "linux") && std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none());
                    let path = if no_dialog {
                        Some(fallback())
                    } else {
                        rfd::FileDialog::new().set_file_name(&name).add_filter(format.ext().to_uppercase(), &[format.ext()]).save_file()
                    };
                    match path {
                        Some(p) => std::fs::write(&p, &bytes).map(|_| Some(p)).map_err(|e| e.to_string()),
                        None => Ok(None),
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                this.status = Some(match result {
                    Ok(Some(p)) => format!("Exported {}", p.display()).into(),
                    Ok(None) => "Export cancelled".into(),
                    Err(e) => format!("Export failed: {e}").into(),
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn notify_status(&mut self, msg: &str, window: &mut Window, cx: &mut Context<Self>) {
        window.push_notification(msg.to_string(), cx);
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
