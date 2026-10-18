//! Onshape's Create Drawing dialog: drawing name (editable in the title),
//! Existing templates (standard filter All / ANSI / ISO / JIS, template
//! list) or a Custom template (standard, size, units, projection, title
//! block), and the Suggested views / No views option.
//!
//! Any view asks for it with [`request_create_drawing`] (part, assembly or
//! "+" menu); the workspace shows the dialog, creates the Drawing tab and,
//! for No views, starts placing a view of the source at the cursor.

use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::create::DrawingSource;
use noshape_model::drawing::templates::DrawingTemplate;
use noshape_model::drawing::{Projection, SheetSize, TitleBlockStyle};
use parking_lot::Mutex;

/// A pending "create drawing" request (set by a view, taken by the workspace).
#[derive(Clone, Debug)]
pub struct CreateRequest {
    pub source: Option<DrawingSource>,
    /// Prefilled drawing name (the part / assembly name).
    pub name: Option<String>,
}

static REQUEST: Mutex<Option<CreateRequest>> = Mutex::new(None);

/// Ask the workspace to open the Create Drawing dialog.
pub fn request_create_drawing(req: CreateRequest, window: &mut Window, cx: &mut App) {
    *REQUEST.lock() = Some(req);
    window.dispatch_action(Box::new(crate::actions::CreateDrawing), cx);
}

pub fn take_request() -> Option<CreateRequest> {
    REQUEST.lock().take()
}

/// One row of the Existing templates list: Onshape's file name, its
/// document and the sheet setup.
#[derive(Clone, Copy, Debug)]
pub struct TemplateRow {
    pub file: &'static str,
    pub document: &'static str,
    pub standard: &'static str,
    pub template: DrawingTemplate,
}

/// Onshape's standard templates (ISO in millimeters, first angle; ANSI in
/// inches or millimeters, third angle): (file, sheet setup name, size, inches).
const ANSI: [(&str, &str, SheetSize, bool); 12] = [
    ("ANSI_A_INCH.dwt", "ANSI A", SheetSize::AnsiA, true),
    ("ANSI_A_MM.dwt", "ANSI A", SheetSize::AnsiA, false),
    ("ANSI_A_Portrait_INCH.dwt", "ANSI A Portrait", SheetSize::AnsiAPortrait, true),
    ("ANSI_A_Portrait_MM.dwt", "ANSI A Portrait", SheetSize::AnsiAPortrait, false),
    ("ANSI_B_INCH.dwt", "ANSI B", SheetSize::AnsiB, true),
    ("ANSI_B_MM.dwt", "ANSI B", SheetSize::AnsiB, false),
    ("ANSI_C_INCH.dwt", "ANSI C", SheetSize::AnsiC, true),
    ("ANSI_C_MM.dwt", "ANSI C", SheetSize::AnsiC, false),
    ("ANSI_D_INCH.dwt", "ANSI D", SheetSize::AnsiD, true),
    ("ANSI_D_MM.dwt", "ANSI D", SheetSize::AnsiD, false),
    ("ANSI_E_INCH.dwt", "ANSI E", SheetSize::AnsiE, true),
    ("ANSI_E_MM.dwt", "ANSI E", SheetSize::AnsiE, false),
];
const ISO: [(&str, &str, SheetSize); 5] = [("ISO_A0.dwt", "ISO A0", SheetSize::A0), ("ISO_A1.dwt", "ISO A1", SheetSize::A1), ("ISO_A2.dwt", "ISO A2", SheetSize::A2), ("ISO_A3.dwt", "ISO A3", SheetSize::A3), ("ISO_A4.dwt", "ISO A4", SheetSize::A4)];

pub fn template_rows() -> Vec<TemplateRow> {
    let ansi = ANSI.iter().map(|&(file, name, size, inches)| TemplateRow {
        file,
        document: "Onshape ANSI Drawing Templates",
        standard: "ANSI",
        template: DrawingTemplate { name, size, style: TitleBlockStyle::Ansi, inches, projection: Projection::ThirdAngle },
    });
    let iso = ISO.iter().map(|&(file, name, size)| TemplateRow {
        file,
        document: "Onshape ISO Drawing Templates",
        standard: "ISO",
        template: DrawingTemplate { name, size, style: TitleBlockStyle::Iso, inches: false, projection: Projection::FirstAngle },
    });
    ansi.chain(iso).collect()
}

/// Template sources of the Existing templates tab (label, icon).
const SOURCES: [(&str, &str); 6] = [("Onshape", "noshape-logo"), ("Owned by me", "src-owned"), ("Recently opened", "src-recent"), ("Created by me", "src-created"), ("Shared with me", "src-shared"), ("Public", "src-public")];

pub enum CreateDrawingEvent {
    Create { name: String, template: DrawingTemplate, source: Option<DrawingSource>, suggested: bool },
    Cancel,
}

pub struct CreateDrawingDialog {
    source: Option<DrawingSource>,
    name: Entity<InputState>,
    rows: Vec<TemplateRow>,
    /// Template source (index into [`SOURCES`]; 0 = Onshape).
    source_filter: usize,
    /// Standard filter: "All", "ANSI", "ISO", "JIS".
    standard: &'static str,
    selected: usize,
    /// Custom template tab and its choices.
    custom: bool,
    custom_standard: &'static str,
    custom_size: usize,
    custom_inches: bool,
    custom_first_angle: bool,
    custom_title_block: bool,
    suggested: bool,
    pub focus: FocusHandle,
    /// Template list scrolling (the preselected template is scrolled into view).
    list_scroll: ScrollHandle,
}

impl EventEmitter<CreateDrawingEvent> for CreateDrawingDialog {}

impl Focusable for CreateDrawingDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// Sheet sizes of the Custom template tab per standard.
/// (sheet setup name, size); the size label is the name's last word.
fn custom_sizes(standard: &str) -> Vec<(&'static str, SheetSize)> {
    if standard == "ISO" {
        ISO.iter().map(|&(_, n, s)| (n, s)).collect()
    } else {
        ANSI.iter().filter(|a| a.3 && a.2 != SheetSize::AnsiAPortrait).map(|&(_, n, s, _)| (n, s)).collect()
    }
}

impl CreateDrawingDialog {
    /// `metric`: the document works in millimeters (ISO_A3 preselected,
    /// else ANSI_A_INCH, Onshape's first row). `default_name`: the name
    /// Onshape suggests ("Part 1 Drawing 1", or "Drawing 1" without a source).
    pub fn new(req: CreateRequest, default_name: String, metric: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).default_value(default_name));
        let rows = template_rows();
        let selected = rows.iter().position(|r| r.file == if metric { "ISO_A3.dwt" } else { "ANSI_A_INCH.dwt" }).unwrap_or(0);
        Self {
            source: req.source,
            name,
            rows,
            source_filter: 0,
            standard: "All",
            selected,
            custom: false,
            custom_standard: if metric { "ISO" } else { "ANSI" },
            custom_size: if metric { 3 } else { 1 },
            custom_inches: !metric,
            custom_first_angle: metric,
            custom_title_block: true,
            // Onshape preselects "No views" (the first view follows the cursor).
            suggested: false,
            focus: cx.focus_handle(),
            list_scroll: {
                // Item 0 is the header row.
                let h = ScrollHandle::new();
                h.scroll_to_item(selected + 1);
                h
            },
        }
    }

    fn template(&self) -> Option<DrawingTemplate> {
        if self.custom {
            let (name, size) = *custom_sizes(self.custom_standard).get(self.custom_size)?;
            let style = if !self.custom_title_block {
                TitleBlockStyle::None
            } else if self.custom_standard == "ISO" {
                TitleBlockStyle::Iso
            } else {
                TitleBlockStyle::Ansi
            };
            return Some(DrawingTemplate { name, size, style, inches: self.custom_inches, projection: if self.custom_first_angle { Projection::FirstAngle } else { Projection::ThirdAngle } });
        }
        self.rows.get(self.selected).map(|r| r.template)
    }

    fn accept(&mut self, cx: &mut Context<Self>) {
        let Some(template) = self.template() else { return };
        let name = self.name.read(cx).value().trim().to_string();
        // No source: there are no views to suggest.
        let suggested = self.suggested && self.source.is_some();
        cx.emit(CreateDrawingEvent::Create { name, template, source: self.source.clone(), suggested });
    }

    fn visible_rows(&self) -> Vec<usize> {
        if self.source_filter != 0 {
            return Vec::new();
        }
        (0..self.rows.len()).filter(|i| self.standard == "All" || self.rows[*i].standard == self.standard).collect()
    }

    fn render_existing(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let std_tab = |label: &'static str, cur: &'static str, cx: &mut Context<Self>| {
            let on = label == cur;
            div()
                .id(SharedString::from(format!("cd-std-{label}")))
                .w(px(88.))
                .py(px(6.))
                .text_center()
                .text_size(px(13.))
                .cursor_pointer()
                .text_color(if on { pal.accent } else { pal.text })
                .when(on, |d| d.font_weight(FontWeight::SEMIBOLD).border_b_2().border_color(pal.accent))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.standard = label;
                    if let Some(&first) = this.visible_rows().first()
                        && !this.visible_rows().contains(&this.selected)
                    {
                        this.selected = first;
                    }
                    cx.notify();
                }))
        };
        let cur = self.standard;
        let mut list = v_flex().id("cd-templates").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.list_scroll);
        list = list.child(
            h_flex()
                .flex_none()
                .h(px(30.))
                .px(px(10.))
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(div().w(px(209.)).child("Template"))
                .child(div().w(px(209.)).child("Document"))
                .child(div().flex_1().child("Owner")),
        );
        let visible = self.visible_rows();
        if visible.is_empty() {
            list = list.child(div().px(px(10.)).py_2().text_size(px(13.)).text_color(pal.text_muted).child("No templates for this standard."));
        }
        for i in visible {
            let r = self.rows[i];
            let sel = i == self.selected;
            list = list.child(
                h_flex()
                    .id(SharedString::from(format!("cd-row-{i}")))
                    .flex_none()
                    .h(px(30.))
                    .px(px(10.))
                    .text_size(px(13.))
                    .cursor_pointer()
                    .when(sel, |d| d.bg(pal.accent_soft).font_weight(FontWeight::SEMIBOLD))
                    .when(!sel, |d| d.hover(|d| d.bg(pal.row_hover)))
                    .child(div().w(px(209.)).truncate().child(r.file))
                    .child(div().w(px(209.)).pr_2().truncate().child(r.document))
                    .child(div().flex_1().truncate().child("Onshape"))
                    .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                        this.selected = i;
                        if ev.click_count() >= 2 {
                            this.accept(cx);
                        }
                        cx.notify();
                    })),
            );
        }
        h_flex()
            .size_full()
            .items_start()
            .child(
                // Template sources (Onshape's document filters); only
                // Onshape's own templates exist locally.
                {
                    let mut col = v_flex().w(px(196.)).h_full().pr_2().border_r_1().border_color(gpui_kit::rgb(0xd0dbe5));
                    for (i, (label, icon)) in SOURCES.iter().enumerate() {
                        let on = i == self.source_filter;
                        col = col.child(
                            h_flex()
                                .id(SharedString::from(format!("cd-src-{i}")))
                                .h(px(32.))
                                .gap(px(6.))
                                .pl(px(6.))
                                .cursor_pointer()
                                .text_size(px(14.67))
                                .when(on, |d| d.border_l_4().border_color(pal.accent).font_weight(FontWeight::SEMIBOLD).text_color(pal.accent))
                                .when(!on, |d| d.pl(px(10.)).text_color(pal.text).hover(|d| d.bg(pal.row_hover)))
                                .child(crate::assets::icon(&crate::assets::icon_path(icon), 20., cx))
                                .child(*label)
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.source_filter = i;
                                    cx.notify();
                                })),
                        );
                    }
                    col
                },
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .min_h_0()
                    .pl_3()
                    .child(h_flex().border_b_1().border_color(pal.border).child(std_tab("All", cur, cx)).child(std_tab("ANSI", cur, cx)).child(std_tab("ISO", cur, cx)).child(std_tab("JIS", cur, cx)))
                    .child(list),
            )
            .into_any_element()
    }

    fn render_custom(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let row = |label: &'static str, body: AnyElement| h_flex().gap_3().h(px(32.)).child(div().w(px(150.)).text_size(px(13.)).child(label)).child(body);
        let seg = |id: &'static str, label: &'static str, on: bool, cx: &mut Context<Self>, f: fn(&mut Self)| {
            Button::new(id).label(label).small().outline().selected(on).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                f(this);
                cx.notify();
            }))
        };
        let std = self.custom_standard;
        let mut sizes = h_flex().gap_1();
        for (i, (name, _)) in custom_sizes(std).into_iter().enumerate() {
            let label = name.rsplit(' ').next().unwrap_or(name);
            let on = i == self.custom_size;
            sizes = sizes.child(Button::new(SharedString::from(format!("cd-size-{i}"))).label(label).small().outline().selected(on).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.custom_size = i;
                cx.notify();
            })));
        }
        v_flex()
            .flex_1()
            .p_3()
            .gap_1()
            .child(row("Standard", h_flex().gap_1().child(seg("cd-cs-ansi", "ANSI", std == "ANSI", cx, |t| {
                // The standard sets the default units and projection.
                t.custom_standard = "ANSI";
                t.custom_inches = true;
                t.custom_first_angle = false;
            })).child(seg("cd-cs-iso", "ISO", std == "ISO", cx, |t| {
                t.custom_standard = "ISO";
                t.custom_inches = false;
                t.custom_first_angle = true;
            })).into_any_element()))
            .child(row("Size", sizes.into_any_element()))
            .child(row("Units", h_flex().gap_1().child(seg("cd-cu-mm", "Millimeters", !self.custom_inches, cx, |t| t.custom_inches = false)).child(seg("cd-cu-in", "Inches", self.custom_inches, cx, |t| t.custom_inches = true)).into_any_element()))
            .child(row("Projection", h_flex().gap_1().child(seg("cd-cp-3", "Third angle", !self.custom_first_angle, cx, |t| t.custom_first_angle = false)).child(seg("cd-cp-1", "First angle", self.custom_first_angle, cx, |t| t.custom_first_angle = true)).into_any_element()))
            .child(row("Titleblock", h_flex().gap_1().child(seg("cd-ct-on", "Include", self.custom_title_block, cx, |t| t.custom_title_block = true)).child(seg("cd-ct-off", "Omit", !self.custom_title_block, cx, |t| t.custom_title_block = false)).into_any_element()))
            .child(div().pt_2().text_size(px(12.)).text_color(pal.text_muted).child("Decimal separator, language, border and zones follow the standard."))
            .into_any_element()
    }
}

impl Render for CreateDrawingDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let custom = self.custom;
        let page_tab = |label: &'static str, on: bool, cx: &mut Context<Self>, value: bool| {
            div()
                .id(SharedString::from(format!("cd-page-{label}")))
                .mr(px(20.))
                .pb(px(2.))
                .text_size(px(15.))
                .cursor_pointer()
                .text_color(if on { pal.accent } else { pal.text })
                .when(on, |d| d.font_weight(FontWeight::SEMIBOLD).border_b_2().border_color(pal.accent))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.custom = value;
                    cx.notify();
                }))
        };
        let has_source = self.source.is_some();
        let option = |id: &'static str, label: &'static str, on: bool, enabled: bool, icon: &'static str, cx: &mut Context<Self>, value: bool| {
            v_flex()
                .id(id)
                .w(px(140.))
                .h(px(90.))
                .items_center()
                .justify_center()
                .gap_1()
                .rounded(px(4.))
                .text_size(px(12.))
                .bg(if on { gpui_kit::rgb(0x9ccef0) } else { pal.panel_hover })
                .when(on, |d| d.border_b_2().border_color(pal.accent).font_weight(FontWeight::SEMIBOLD).text_color(pal.accent))
                .when(!enabled, |d| d.opacity(0.5))
                .child(crate::assets::icon(&crate::assets::icon_path(icon), 40., cx))
                .child(label)
                .when(enabled, |d| {
                    d.cursor_pointer().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.suggested = value;
                        cx.notify();
                    }))
                })
        };
        let suggested = self.suggested && has_source;
        let body = if custom { self.render_custom(cx) } else { self.render_existing(cx) };
        // Modal backdrop with the dialog centered.
        div()
            .id("create-drawing-backdrop")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(hsla(0., 0., 0., 0.25))
            .occlude()
            .key_context("CreateDrawing")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &crate::actions::Cancel, _, cx| cx.emit(CreateDrawingEvent::Cancel)))
            .on_action(cx.listener(|this, _: &crate::actions::Accept, _, cx| this.accept(cx)))
            .child(
                v_flex()
                    .w(px(908.))
                    .h(px(597.))
                    .bg(pal.dialog_bg)
                    .rounded(px(4.))
                    .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.25), offset: point(px(0.), px(6.)), blur_radius: px(18.), spread_radius: px(0.), inset: false }])
                    .text_color(pal.text)
                    .child(
                        h_flex()
                            .h(px(54.))
                            .px(px(10.))
                            .gap_2()
                            .border_b_1()
                            .border_color(pal.border)
                            .child(div().text_size(px(18.)).child("Create Drawing:"))
                            .child(div().w(px(320.)).child(Input::new(&self.name).appearance(false).text_size(px(18.)).font_weight(FontWeight::SEMIBOLD)))
                            .child(div().flex_1())
                            .child(Button::new("cd-close").ghost().child(crate::ui::glyph("close", 16., pal.text_muted)).on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CreateDrawingEvent::Cancel)))),
                    )
                    .child(h_flex().px(px(15.)).pt(px(14.)).child(page_tab("Existing templates", !custom, cx, false)).child(page_tab("Custom template", custom, cx, true)))
                    .child(div().mx(px(15.)).border_b_1().border_color(pal.border))
                    .child(div().flex().flex_1().min_h_0().px(px(15.)).pt_2().child(body))
                    .child(div().px(px(15.)).pt_2().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child("Options"))
                    .child(
                        h_flex()
                            .mx(px(15.))
                            .py_2()
                            .gap_2()
                            .justify_center()
                            .border_t_1()
                            .border_color(pal.border)
                            .child(option("cd-suggested", "Suggested views", suggested, has_source, "drawing-suggested-views", cx, true))
                            .child(option("cd-noviews", "No views", !suggested, true, "drawing-no-views", cx, false)),
                    )
                    .child(
                        h_flex()
                            .h(px(56.))
                            .px(px(15.))
                            .gap_2()
                            .justify_end()
                            .border_t_1()
                            .border_color(pal.border)
                            .child(Button::new("cd-ok").label("OK").primary().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.accept(cx))))
                            .child(Button::new("cd-cancel").label("Cancel").on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CreateDrawingEvent::Cancel)))),
                    ),
            )
    }
}
