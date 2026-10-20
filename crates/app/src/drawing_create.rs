//! Onshape's Create Drawing dialog: drawing name (editable in the title),
//! Existing templates (standard filter All / ANSI / ISO / JIS, template
//! list) or a Custom template (standard, size, units, projection, title
//! block), and the Suggested views / No views option.
//!
//! Any view asks for it with [`request_create_drawing`] (part, assembly or
//! "+" menu); the workspace shows the dialog, creates the Drawing tab and,
//! for No views, starts placing a view of the source at the cursor.

use crate::os_ui as os;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::create::DrawingSource;
use noshape_model::drawing::tb_lang::TitleLanguage;
use noshape_model::drawing::templates::DrawingTemplate;
use noshape_model::drawing::{DimUnits, Projection, SheetFormat, SheetSize, TitleBlockStyle, ZoneStart};
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

/// Part Studio › "Create Drawing of Sketch N…": the Create Drawing dialog
/// for a sketch view of feature `sketch` (named after the sketch).
pub fn request_create_drawing_of_sketch(tab: noshape_model::document::TabId, sketch: noshape_model::FeatureId, sketch_name: &str, window: &mut Window, cx: &mut App) {
    let source = noshape_model::drawing::create::DrawingSource { tab, part: None, config: Default::default(), sketch: Some(sketch) };
    request_create_drawing(CreateRequest { source: Some(source), name: Some(sketch_name.to_string()) }, window, cx);
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
        template: DrawingTemplate::standard(name, size, TitleBlockStyle::Ansi, inches, Projection::ThirdAngle),
    });
    let iso = ISO.iter().map(|&(file, name, size)| TemplateRow {
        file,
        document: "Onshape ISO Drawing Templates",
        standard: "ISO",
        template: DrawingTemplate::standard(name, size, TitleBlockStyle::Iso, false, Projection::FirstAngle),
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
    ct: CustomTemplate,
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
/// Live drawing/800–825: ANSI A (default), A Portrait, B, C, D; ISO and
/// JIS A4 (default), A4 Portrait, A3, A2, A1, A0.
fn custom_sizes(standard: &str) -> Vec<(&'static str, SheetSize)> {
    let names: &[&'static str] = match standard {
        "ISO" => &["ISO A4", "ISO A4 Portrait", "ISO A3", "ISO A2", "ISO A1", "ISO A0"],
        "JIS" => &["JIS A4", "JIS A4 Portrait", "JIS A3", "JIS A2", "JIS A1", "JIS A0"],
        _ => &["ANSI A", "ANSI A Portrait", "ANSI B", "ANSI C", "ANSI D"],
    };
    names.iter().copied().zip(SheetSize::standard_list(standard == "ANSI").iter().copied()).collect()
}

/// The Custom template's default zones per size (live: ANSI A and A Portrait
/// 2 x 2, B and C 4 x 4, D 8 x 4; ISO / JIS A4 0 x 0, A3 6 x 4, A2 8 x 6, A1
/// 12 x 8, A0 16 x 12).
fn custom_zones(size: SheetSize) -> [u32; 2] {
    match size {
        SheetSize::AnsiB | SheetSize::AnsiC => [4, 4],
        s => s.standard_zones(),
    }
}

/// Custom template › Units (help H:custom_drawing_templates; "Feet and
/// inches" exists only in drawings).
const CUSTOM_UNITS: [(DimUnits, &str); 7] = [
    (DimUnits::Millimeter, "Millimeters"),
    (DimUnits::Centimeter, "Centimeter"),
    (DimUnits::Meter, "Meter"),
    (DimUnits::Inch, "Inches"),
    (DimUnits::FeetInches, "Feet and inches"),
    (DimUnits::Foot, "Foot"),
    (DimUnits::Yard, "Yard"),
];


/// Zone counts offered by Horizontal / Vertical zones: even numbers 0–20 (live).
const ZONE_COUNTS: [u32; 11] = [0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20];

/// The Custom template tab's choices (live drawing/02).
#[derive(Clone, Copy, Debug)]
struct CustomTemplate {
    standard: &'static str,
    language: usize,
    size: usize,
    units: DimUnits,
    comma: bool,
    first_angle: bool,
    border: bool,
    zones: [u32; 2],
    top_left: bool,
    title_block: bool,
}

impl CustomTemplate {
    fn new(metric: bool) -> Self {
        let mut t = Self { standard: "ANSI", language: 2, size: 0, units: DimUnits::Inch, comma: false, first_angle: false, border: true, zones: [2, 2], top_left: false, title_block: true };
        t.set_standard(if metric { "ISO" } else { "ANSI" });
        t
    }

    /// Choosing a standard resets the size (A / A4) and its zones, the
    /// units, the decimal separator and the projection (live: ANSI Inches,
    /// Period, Third angle; ISO Millimeters, Comma, First angle; JIS
    /// Millimeters, Comma, Third angle). Border, start zones and title block
    /// stay.
    fn set_standard(&mut self, standard: &'static str) {
        self.standard = standard;
        let ansi = standard == "ANSI";
        self.units = if ansi { DimUnits::Inch } else { DimUnits::Millimeter };
        self.comma = !ansi;
        self.first_angle = standard == "ISO";
        self.set_size(0);
    }

    fn set_size(&mut self, i: usize) {
        self.size = i;
        if let Some((_, size)) = custom_sizes(self.standard).get(i) {
            self.zones = custom_zones(*size);
        }
    }

    fn template(&self) -> Option<DrawingTemplate> {
        let (name, size) = *custom_sizes(self.standard).get(self.size)?;
        let style = if !self.title_block {
            TitleBlockStyle::None
        } else if self.standard == "ANSI" {
            TitleBlockStyle::Ansi
        } else {
            TitleBlockStyle::Iso
        };
        let projection = if self.first_angle { Projection::FirstAngle } else { Projection::ThirdAngle };
        let mut t = DrawingTemplate::standard(name, size, style, self.units.imperial(), projection);
        if !matches!(self.units, DimUnits::Millimeter | DimUnits::Inch) {
            t.units = Some(self.units);
        }
        t.decimal_comma = self.comma;
        t.language = TitleLanguage::ALL.get(self.language).copied().unwrap_or_default();
        let zones = (self.zones != size.standard_zones()).then_some(self.zones);
        t.format = SheetFormat { border: self.border, zones, start: if self.top_left { ZoneStart::TopLeft } else { ZoneStart::BottomRight } };
        Some(t)
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
            ct: CustomTemplate::new(metric),
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
            return self.ct.template();
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
                // Live drawing/01: the current standard is blue, semibold and underlined.
                .when(on, |d| d.font_weight(FontWeight::SEMIBOLD).underline())
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
                .text_size(px(14.))
                .font_weight(FontWeight::SEMIBOLD)
                // Live drawing/01: the list is too narrow for Owner, which shows as "…".
                .child(div().w(px(209.)).child("Template"))
                .child(div().w(px(200.)).child("Document"))
                .child(div().flex_1().child("…")),
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
                    .text_size(px(14.))
                    .cursor_pointer()
                    // Live (drawing/01): the chosen row is `#b2ddf6` and bold.
                    .when(sel, |d| d.bg(gpui_kit::rgb(0xb2ddf6)).font_weight(FontWeight::SEMIBOLD))
                    .when(!sel, |d| d.hover(|d| d.bg(pal.row_hover)))
                    .child(div().w(px(209.)).truncate().child(r.file))
                    .child(div().w(px(200.)).truncate().child(r.document))
                    .child(div().flex_1().child("…"))
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
                    .child(h_flex().h(px(42.)).flex_none().border_b_1().border_color(pal.border).child(std_tab("All", cur, cx)).child(std_tab("ANSI", cur, cx)).child(std_tab("ISO", cur, cx)).child(std_tab("JIS", cur, cx)))
                    .child(list),
            )
            .into_any_element()
    }

    fn render_custom(&mut self, cx: &mut Context<Self>) -> AnyElement {
        // Live drawing/02: two columns split by a 1 px `#d0dbe5` rule; labels
        // right-aligned (14 px, `#333`), 35 px rows; choices are blue
        // semibold with a 3 px blue underline when chosen, `#666` otherwise;
        // dropdowns 120 x 24 with blue semibold text.
        let pal = p();
        let ct = self.ct;
        let label = |w: f32, text: &'static str| div().w(px(w)).flex_none().text_right().text_size(px(14.)).line_height(px(22.)).text_color(gpui_kit::rgb(0x333333)).child(text);
        let row = |w: f32, text: &'static str, body: AnyElement| h_flex().min_h(px(35.)).gap(px(16.)).items_center().child(label(w, text)).child(body);
        let choice = |id: String, text: &'static str, on: bool, cx: &mut Context<Self>, f: Box<dyn Fn(&mut CustomTemplate)>| {
            div()
                .id(SharedString::from(id))
                .mr(px(15.))
                .pt(px(2.))
                .text_size(px(14.))
                .cursor_pointer()
                .when(on, |d| d.font_weight(FontWeight::SEMIBOLD).text_color(pal.accent).pb(px(1.)).border_b(px(3.)).border_color(pal.accent))
                .when(!on, |d| d.text_color(gpui_kit::rgb(0x666666)).pb(px(4.)))
                .child(text)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    f(&mut this.ct);
                    cx.notify();
                }))
        };
        let entity = cx.entity().downgrade();
        let drop = |id: &'static str, options: Vec<String>, current: usize, f: fn(&mut CustomTemplate, usize)| {
            let entity = entity.clone();
            let label = options.get(current).cloned().unwrap_or_default();
            let bounds: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
            let probe = bounds.clone();
            div()
                .id(id)
                .relative()
                .w(px(120.))
                .h(px(24.))
                .flex()
                .items_center()
                .pl(px(10.))
                .pr(px(6.))
                .border_1()
                .border_color(gpui_kit::rgb(0x888888))
                .bg(gpui_kit::rgb(0xffffff))
                .cursor_pointer()
                .text_size(px(14.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(pal.accent)
                .child(canvas(move |b, _, _| probe.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
                .child(div().flex_1().truncate().child(label))
                .child(os::glyph(os::GLYPH_CARET, 8., gpui_kit::rgb(0x777777)))
                .on_click(move |_, window, cx| {
                    let (options, b, entity) = (options.clone(), bounds.get(), entity.clone());
                    let width = f32::from(b.size.width);
                    let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                        m = m.select_list(width);
                        for (i, o) in options.iter().enumerate() {
                            let entity = entity.clone();
                            m = m.item(crate::os_menu::OsMenuItem::new(o.clone()).checked(i == current).on_click(move |_, window, cx| {
                                let _ = entity.update(cx, |this, cx| {
                                    f(&mut this.ct, i);
                                    cx.notify();
                                });
                                window.refresh();
                            }));
                        }
                        m
                    });
                    crate::os_menu::open_anchored(point(b.origin.x, b.origin.y + b.size.height), Anchor::TopLeft, menu, window, cx);
                })
        };
        let tile = |id: &'static str, text: &'static str, icon: &'static str, on: bool, cx: &mut Context<Self>, first: bool| {
            v_flex()
                .id(id)
                .w(px(140.))
                .h(px(90.))
                .items_center()
                .justify_center()
                .gap_1()
                .rounded(px(4.))
                .text_size(px(12.))
                .bg(if on { gpui_kit::rgb(0x90cef1) } else { pal.panel })
                .when(on, |d| d.border_b(px(3.)).border_color(pal.accent).font_weight(FontWeight::SEMIBOLD).text_color(pal.accent))
                .when(!on, |d| d.border_1().border_color(gpui_kit::rgb(0xaaaaaa)).text_color(gpui_kit::rgb(0x333333)))
                .cursor_pointer()
                .child(div().when(!on, |d| d.opacity(0.6)).child(crate::assets::icon(&crate::assets::icon_path(icon), 64., cx)))
                .child(text)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.ct.first_angle = first;
                    cx.notify();
                }))
        };
        let standards = ["ANSI", "ISO", "JIS"].into_iter().fold(h_flex(), |r, s| r.child(choice(format!("cd-cs-{s}"), s, ct.standard == s, cx, Box::new(move |t| t.set_standard(s)))));
        let sizes: Vec<String> = custom_sizes(ct.standard).iter().map(|(_, s)| s.list_name().to_string()).collect();
        let units = CUSTOM_UNITS.iter().position(|(u, _)| *u == ct.units).unwrap_or(0);
        let zones: Vec<String> = ZONE_COUNTS.iter().map(|n| n.to_string()).collect();
        let zone_at = |n: u32| ZONE_COUNTS.iter().position(|z| *z == n).unwrap_or(0);
        let left = v_flex()
            .w(px(440.))
            .flex_none()
            .pr(px(6.))
            .child(row(133., "Standard", standards.into_any_element()))
            .child(row(133., "Language", drop("cd-lang", TitleLanguage::ALL.iter().map(|l| l.name().to_string()).collect(), ct.language, |t, i| t.language = i).into_any_element()))
            .child(row(133., "Size", drop("cd-size", sizes, ct.size, |t, i| t.set_size(i)).into_any_element()))
            .child(row(133., "Units", drop("cd-units", CUSTOM_UNITS.iter().map(|(_, n)| n.to_string()).collect(), units, |t, i| t.units = CUSTOM_UNITS[i].0).into_any_element()))
            .child(
                h_flex()
                    .min_h(px(45.))
                    .gap(px(16.))
                    .items_center()
                    .child(div().w(px(133.)).flex_none().text_right().text_size(px(14.)).line_height(px(22.)).text_color(gpui_kit::rgb(0x333333)).child("Decimal").child(div().child("separator")))
                    .child(h_flex().child(choice("cd-dec-period".into(), "Period (12.34)", !ct.comma, cx, Box::new(|t| t.comma = false))).child(choice("cd-dec-comma".into(), "Comma (12,34)", ct.comma, cx, Box::new(|t| t.comma = true)))),
            )
            .child(
                h_flex()
                    .mt(px(4.))
                    .gap(px(16.))
                    .items_start()
                    .child(label(133., "Projection").mt(px(10.)))
                    .child(h_flex().gap(px(5.)).child(tile("cd-proj-3", "Third angle (ANSI)", "projection-third-angle", !ct.first_angle, cx, false)).child(tile("cd-proj-1", "First angle (ISO)", "projection-first-angle", ct.first_angle, cx, true))),
            );
        let right = v_flex()
            .flex_1()
            .pt(px(11.))
            .child(row(122., "Border", h_flex().child(choice("cd-border-on".into(), "Include", ct.border, cx, Box::new(|t| t.border = true))).child(choice("cd-border-off".into(), "Do not include", !ct.border, cx, Box::new(|t| t.border = false))).into_any_element()))
            // Live 815-a: "Do not include" hides the zone rows.
            .when(ct.border, |c| {
                c.child(row(122., "Horizontal zones", drop("cd-hzones", zones.clone(), zone_at(ct.zones[0]), |t, i| t.zones[0] = ZONE_COUNTS[i]).into_any_element()))
                    .child(row(122., "Vertical zones", drop("cd-vzones", zones, zone_at(ct.zones[1]), |t, i| t.zones[1] = ZONE_COUNTS[i]).into_any_element()))
                    .child(row(122., "Start zones", h_flex().child(choice("cd-start-br".into(), "Bottom right", !ct.top_left, cx, Box::new(|t| t.top_left = false))).child(choice("cd-start-tl".into(), "Top left", ct.top_left, cx, Box::new(|t| t.top_left = true))).into_any_element()))
            })
            .child(row(122., "Titleblock", h_flex().child(choice("cd-tb-on".into(), "Include", ct.title_block, cx, Box::new(|t| t.title_block = true))).child(choice("cd-tb-off".into(), "Do not include", !ct.title_block, cx, Box::new(|t| t.title_block = false))).into_any_element()));
        h_flex()
            .flex_1()
            .items_start()
            .pt(px(4.))
            .child(left)
            .child(div().w(px(1.)).h(px(262.)).flex_none().bg(gpui_kit::rgb(0xd0dbe5)))
            .child(right)
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
                .mr(px(19.))
                .pb(px(6.))
                .text_size(px(16.))
                .cursor_pointer()
                // Live drawing/01: blue, semibold and underlined; the other tab `#555`.
                .text_color(if on { pal.accent } else { gpui_kit::rgb(0x555555) })
                .when(on, |d| d.font_weight(FontWeight::SEMIBOLD).underline())
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
                // Live drawing/01: the chosen tile `#9ccef0` with a 2 px blue bottom edge,
                // the other white with a 1 px `#ccc` outline.
                .bg(if on { gpui_kit::rgb(0x9ccef0) } else { pal.panel })
                .when(on, |d| d.border_b_2().border_color(pal.accent).font_weight(FontWeight::SEMIBOLD).text_color(pal.accent))
                .when(!on, |d| d.border_1().border_color(gpui_kit::rgb(0xcccccc)))
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
            .items_start()
            .justify_center()
            // Onshape's modals hang 11 px under the window top (live drawing/01).
            .pt(px(11.))
            .bg(pal.modal_backdrop)
            .occlude()
            .key_context("CreateDrawing")
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &crate::actions::Cancel, _, cx| cx.emit(CreateDrawingEvent::Cancel)))
            .on_action(cx.listener(|this, _: &crate::actions::Accept, _, cx| this.accept(cx)))
            .child(
                v_flex()
                    .w(px(908.))
                    .h(px(597.))
                    .bg(pal.modal_bg)
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
                            .child(div().w(px(320.)).ml(px(-6.)).child(Input::new(&self.name).appearance(false).text_size(px(18.)).font_weight(FontWeight::SEMIBOLD)))
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
                            .child(os::primary_button("cd-ok", "OK").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.accept(cx))))
                            .child(os::cancel_button("cd-cancel", "Cancel", cx).on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(CreateDrawingEvent::Cancel)))),
                    ),
            )
    }
}
