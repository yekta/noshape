//! Drawing editor, second part: sheet tabs (add / rename / duplicate /
//! reorder / delete), jogged and aligned section tools, crop and break
//! views, view rotation, ordinate / chamfer / arc length dimensions, the
//! dimension properties panel (precision, units, tolerances), GD&T, datum
//! and surface finish symbols, tables (general, BOM, hole) and the drags
//! and context menus that go with them.

use crate::drawing_scene::Overlay;
use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::button::Button;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, Textarea};
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::{Sizable as _, h_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::document::TabContent;
use noshape_model::drawing::build::PickTarget;
use noshape_model::drawing::format;
use noshape_model::drawing::views;
use noshape_model::drawing::*;

fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
fn add(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] + b[0], a[1] + b[1]]
}
fn mul(a: [f64; 2], k: f64) -> [f64; 2] {
    [a[0] * k, a[1] * k]
}
fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn len(a: [f64; 2]) -> f64 {
    dot(a, a).sqrt()
}
fn norm(a: [f64; 2]) -> [f64; 2] {
    let l = len(a).max(1e-12);
    [a[0] / l, a[1] / l]
}

/// GD&T characteristic symbols (glyph, name).
pub const GDT_SYMBOLS: &[(&str, &str)] = &[
    ("⌖", "Position"),
    ("⏤", "Straightness"),
    ("▱", "Flatness"),
    ("○", "Circularity"),
    ("⌭", "Cylindricity"),
    ("⌒", "Profile of a line"),
    ("⌓", "Profile of a surface"),
    ("∠", "Angularity"),
    ("⟂", "Perpendicularity"),
    ("∥", "Parallelism"),
    ("◎", "Concentricity"),
    ("⌯", "Symmetry"),
    ("↗", "Circular runout"),
    ("⌰", "Total runout"),
];

/// Material condition modifiers (glyph, name).
pub const GDT_MODIFIERS: &[(&str, &str)] = &[("", "None"), ("Ⓜ", "Maximum material (MMC)"), ("Ⓛ", "Least material (LMC)"), ("Ⓢ", "Regardless of feature size"), ("Ⓟ", "Projected tolerance zone"), ("Ⓕ", "Free state"), ("Ⓣ", "Tangent plane")];

const TOLERANCES: [&str; 8] = ["None", "Symmetrical", "Deviation", "Limits", "Basic", "MIN", "MAX", "Fit"];

fn tol_index(t: &Tolerance) -> usize {
    match t {
        Tolerance::None => 0,
        Tolerance::Symmetric { .. } => 1,
        Tolerance::Deviation { .. } => 2,
        Tolerance::Limits { .. } => 3,
        Tolerance::Basic => 4,
        Tolerance::Min => 5,
        Tolerance::Max => 6,
        Tolerance::Fit { .. } => 7,
    }
}

impl DrawingEditor {
    // ------------------------------------------------------------ sheets

    pub fn switch_sheet(&mut self, i: usize, cx: &mut Context<Self>) {
        let i = i.min(self.drawing.sheets.len().saturating_sub(1));
        if i == self.sheet {
            return;
        }
        self.sheet = i;
        self.selection.clear();
        self.tool = Tool::Select;
        self.panel = None;
        self.preview = None;
        self.overlay = Overlay::default();
        self.geoms.clear();
        self.fitted = false;
        self.ensure_sources(false, cx);
        self.refresh(cx);
    }

    pub fn add_sheet_after(&mut self, i: usize, cx: &mut Context<Self>) {
        let mut d = self.drawing.clone();
        let at = d.add_sheet(i);
        self.commit("Add sheet", d, cx);
        self.switch_sheet(at, cx);
    }

    fn sheet_entries(&self, i: usize) -> Vec<MenuEntry> {
        let n = self.drawing.sheets.len();
        let mut out = vec![
            MenuEntry::item("Rename…", move |t, w, cx| t.open_sheet_name_panel(i, w, cx)),
            MenuEntry::item("Properties…", move |t, w, cx| t.open_sheet_props(i, w, cx)),
            MenuEntry::item("Duplicate", move |t, _, cx| {
                let mut d = t.drawing.clone();
                if let Some(j) = d.duplicate_sheet(i) {
                    t.commit("Duplicate sheet", d, cx);
                    t.switch_sheet(j, cx);
                }
            }),
            MenuEntry::item("Add sheet after", move |t, _, cx| t.add_sheet_after(i, cx)),
        ];
        if i > 0 {
            out.push(MenuEntry::item("Move left", move |t, _, cx| {
                let cur = t.sheet;
                t.edit("Move sheet", cx, |d| {
                    d.move_sheet(i, i - 1);
                });
                if cur == i {
                    t.sheet = i - 1;
                } else if cur == i - 1 {
                    t.sheet = i;
                }
                t.refresh(cx);
            }));
        }
        if i + 1 < n {
            out.push(MenuEntry::item("Move right", move |t, _, cx| {
                let cur = t.sheet;
                t.edit("Move sheet", cx, |d| {
                    d.move_sheet(i, i + 1);
                });
                if cur == i {
                    t.sheet = i + 1;
                } else if cur == i + 1 {
                    t.sheet = i;
                }
                t.refresh(cx);
            }));
        }
        if n > 1 {
            out.push(MenuEntry::Separator);
            out.push(MenuEntry::item("Delete sheet", move |t, _, cx| {
                let mut d = t.drawing.clone();
                if d.delete_sheet(i) {
                    let cur = t.sheet;
                    t.selection.clear();
                    t.sheet = if cur > i || cur >= d.sheets.len() { cur.saturating_sub(1) } else { cur };
                    t.commit("Delete sheet", d, cx);
                    t.fitted = false;
                }
            }));
        }
        out
    }

    fn open_sheet_menu(&mut self, i: usize, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let entries = self.sheet_entries(i);
        let ed = cx.entity();
        // Onshape's right-click menu (os_menu: 12 px, 22 px rows).
        let menu = crate::os_menu::OsMenu::build(window, cx, move |m, window, cx| fill_os_menu(m, &entries, &ed, window, cx));
        if !menu.read(cx).is_empty() {
            crate::os_menu::open(position, menu, window, cx);
        }
        cx.notify();
    }

    /// Name of the reference (part or source tab) a view's base view shows.
    fn view_reference(&self, sheet: &Sheet, id: ViewId, cx: &App) -> String {
        let mut root = id;
        while let Some(p) = sheet.parent_of(root) {
            root = p;
        }
        let Some(ViewKind::Base { source, part, .. }) = sheet.view(root).map(|v| &v.kind) else { return String::new() };
        let src = self.sources.iter().find(|(k, _)| k.tab == *source).map(|(_, m)| m);
        match (src, part) {
            (Some(m), Some(_)) => m.part(part.as_ref()).map(|p| p.name.clone()).unwrap_or_else(|| m.name.clone()),
            (Some(m), None) => m.name.clone(),
            (None, _) => self.doc.read(cx).session.doc.tab(*source).map(|t| t.name.clone()).unwrap_or_default(),
        }
    }

    fn sheet_view_row(&self, sheet: &Sheet, v: &DrawingView, indent: f32, cx: &mut Context<Self>) -> AnyElement {
        // Derived views nest under their parent (20 px per level).
        let mut depth = 0;
        let mut cur = v.id;
        while let Some(p) = sheet.parent_of(cur) {
            depth += 1;
            cur = p;
        }
        let named = |fallback: String| v.name.clone().unwrap_or(fallback);
        let (label, icon) = match &v.kind {
            // Onshape lists views by name: "Front", and projected views by the
            // orientation they show ("Right").
            ViewKind::Base { .. } => (named(crate::drawing_props::default_view_name(&self.drawing, sheet, v)), "base-view"),
            ViewKind::Projected { .. } => (named(crate::drawing_props::default_view_name(&self.drawing, sheet, v)), "projected-view"),
            ViewKind::Auxiliary { .. } => ("Auxiliary view".to_string(), "auxiliary-view"),
            ViewKind::Section { label, .. } => (format!("Section {label}"), "section-view"),
            ViewKind::Detail { label, .. } => (format!("Detail {label}"), "detail-view"),
            _ => ("Section view".to_string(), "section-view"),
        };
        let label = v.label.clone().filter(|l| !l.is_empty()).unwrap_or(label);
        let id = v.id;
        crate::ui::tree_row(SharedString::from(format!("dr-view-{}", v.id.0)), false)
            .pl(px(indent + 20. * depth as f32))
            .text_color(rgb(0x666666))
            .child(crate::ui::tree_icon(icon, false, cx))
            .child(div().flex_1().truncate().child(label))
            // Onshape's view row menu: Properties…, Delete.
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |_, e: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    let entries = vec![
                        MenuEntry::item("Properties…", move |t, w, cx| t.open_view_props(id, w, cx)),
                        MenuEntry::item("Delete", move |t, _, cx| {
                            t.selection.clear();
                            t.edit_sheet("Delete view", cx, |s| s.remove_view(id));
                        }),
                    ];
                    let ed = cx.entity();
                    let menu = crate::os_menu::OsMenu::build(window, cx, move |m, window, cx| fill_os_menu(m, &entries, &ed, window, cx));
                    crate::os_menu::open(e.position, menu, window, cx);
                }),
            )
            .into_any_element()
    }

    /// Onshape's sheet tabs at the top-left of the drawing area ("Sheet1",
    /// the active one on a `#c7c7c7` chip) and the 33x34 tab at the left
    /// edge that opens the Sheets panel.
    fn render_sheet_tabs(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let mut tabs = h_flex().absolute().top(px(5.)).left(px(4.)).gap(px(2.));
        // A dialog panel at the top left (Insert view, View properties)
        // covers the sheet tabs, as in Onshape.
        let covered = matches!(self.tool, Tool::InsertView { .. }) || self.view_props.is_some() || self.sheet_props.is_some() || self.section_dlg.is_some();
        for (i, s) in self.drawing.sheets.iter().enumerate().filter(|_| !covered) {
            let active = i == self.sheet;
            tabs = tabs.child(
                div()
                    .id(("dr-sheet-chip", i))
                    .occlude()
                    .h(px(20.))
                    .px(px(5.))
                    .flex()
                    .items_center()
                    .text_size(px(13.))
                    .text_color(pal.text)
                    .cursor_pointer()
                    .when(active, |d| d.bg(rgb(0xc7c7c7)))
                    .when(!active, |d| d.hover(|d| d.bg(rgb(0xd0d0d0))))
                    .child(s.name.clone())
                    .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                        if e.click_count() >= 2 {
                            this.open_sheet_name_panel(i, window, cx);
                        } else {
                            this.switch_sheet(i, cx);
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            this.open_sheet_menu(i, e.position, window, cx);
                        }),
                    ),
            );
        }
        div().absolute().top_0().left_0().size_full().child(tabs).child(self.sheets_toggle(0., cx)).into_any_element()
    }

    fn sheets_toggle(&self, left: f32, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let open = self.sheets_open;
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(left))
            .flex()
            .items_center()
            .child(
                div()
                    .id("dr-sheets-toggle")
                    .occlude()
                    .w(px(33.))
                    .h(px(34.))
                    .ml(px(-1.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(pal.panel)
                    .border_1()
                    .border_color(pal.border_strong)
                    .cursor_pointer()
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(crate::assets::icon("sheet", 20., cx))
                    .tooltip(crate::ui::tooltip(if open { "Collapse the Sheets panel" } else { "Show the Sheets panel" }))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.sheets_open = !this.sheets_open;
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    /// Onshape's Sheets panel: a 258 px `#fafafa` panel over the left of the
    /// drawing area. Blue "Sheets (n)" title with the Insert sheet button,
    /// a 2 px blue rule, "Sort by sheet | Sort by reference" tabs, then the
    /// tree (sheet › reference › views; the active sheet `#e9eef2` with a
    /// 4 px blue bar). Closed, only the sheet chips and the toggle show.
    pub fn render_sheet_bar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if !self.sheets_open {
            return self.render_sheet_tabs(cx);
        }
        const W: f32 = 258.;
        let blue = rgb(0x1b5faa);
        let pal = p();
        let si = self.sheet;
        let by_ref = self.sheets_by_ref;
        let mut list = gpui_kit::component::v_flex().id("dr-sheets").flex_1().min_h_0().pt(px(1.)).overflow_y_scroll();
        let sheet_row = |i: usize, s: &Sheet, active: bool, cx: &mut Context<Self>| {
            crate::ui::tree_row(("dr-sheet-tab", i), false)
                .relative()
                .cursor_pointer()
                .when(active, |d| d.bg(rgb(0xe9eef2)).text_color(blue).font_weight(FontWeight::SEMIBOLD).border_b_1().border_color(rgb(0x90cef1)))
                .child(crate::ui::caret(active))
                .child(crate::ui::tree_icon("sheet-page", false, cx))
                .child(div().flex_1().truncate().child(format!("{} ({})", s.name, s.views.len())))
                .when(active, |d| d.child(div().absolute().left(px(2.)).top_0().bottom_0().w(px(4.)).bg(blue)))
                .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| {
                    if e.click_count() >= 2 {
                        this.open_sheet_name_panel(i, window, cx);
                    } else {
                        this.switch_sheet(i, cx);
                    }
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        this.open_sheet_menu(i, e.position, window, cx);
                    }),
                )
        };
        let ref_row = |key: String, name: &str, indent: f32, cx: &mut Context<Self>| {
            crate::ui::tree_row(SharedString::from(key), false)
                .pl(px(indent))
                .text_color(rgb(0x666666))
                .child(crate::ui::caret(true))
                .child(crate::ui::tree_icon("part", false, cx))
                .child(div().min_w_0().truncate().child(name.to_string()))
                // The reference is the live workspace (not a version): Onshape's badge.
                .child(div().ml(px(8.)).px(px(4.)).rounded(px(3.)).bg(rgb(0x9ccef0)).text_size(px(11.)).text_color(rgb(0x333333)).child("In progress"))
                .child(div().flex_1())
        };
        // (reference, views) groups of a sheet, in first-view order.
        let groups = |this: &Self, s: &Sheet, cx: &App| {
            let mut g: Vec<(String, Vec<DrawingView>)> = Vec::new();
            for v in &s.views {
                let r = this.view_reference(s, v.id, cx);
                match g.iter_mut().find(|(n, _)| *n == r) {
                    Some((_, vs)) => vs.push(v.clone()),
                    None => g.push((r, vec![v.clone()])),
                }
            }
            g
        };
        if by_ref {
            let mut refs: Vec<(String, Vec<(usize, DrawingView)>)> = Vec::new();
            for (sheet_i, s) in self.drawing.sheets.iter().enumerate() {
                for (r, vs) in groups(self, s, cx) {
                    let entry = match refs.iter().position(|(n, _)| *n == r) {
                        Some(k) => &mut refs[k].1,
                        None => {
                            refs.push((r, Vec::new()));
                            &mut refs.last_mut().expect("just pushed").1
                        }
                    };
                    entry.extend(vs.into_iter().map(|v| (sheet_i, v)));
                }
            }
            for (k, (r, vs)) in refs.iter().enumerate() {
                list = list.child(ref_row(format!("dr-ref-{k}"), r, 6., cx));
                for (sheet_i, v) in vs {
                    list = list.child(self.sheet_view_row(&self.drawing.sheets[*sheet_i], v, 44., cx));
                }
            }
        } else {
            for (i, s) in self.drawing.sheets.iter().enumerate() {
                let active = i == self.sheet;
                list = list.child(sheet_row(i, s, active, cx));
                if active {
                    for (k, (r, vs)) in groups(self, s, cx).iter().enumerate() {
                        list = list.child(ref_row(format!("dr-sheet-ref-{k}"), r, 24., cx));
                        for v in vs {
                            list = list.child(self.sheet_view_row(s, v, 62., cx));
                        }
                    }
                }
            }
        }
        let tab = |id: &'static str, label: &'static str, selected: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .flex_1()
                .h_full()
                .relative()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .cursor_pointer()
                .when(selected, |d| d.text_color(blue).font_weight(FontWeight::MEDIUM).child(div().absolute().left_0().right_0().bottom_0().h(px(2.)).bg(blue)))
                .when(!selected, |d| d.text_color(pal.text))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.sheets_by_ref = id == "dr-sort-ref";
                    cx.notify();
                }))
        };
        let panel = gpui_kit::component::v_flex()
            .id("dr-sheets-panel")
            .occlude()
            .absolute()
            .top_0()
            .left_0()
            .bottom_0()
            .w(px(W))
            .bg(rgb(0xfafafa))
            .border_r_1()
            .border_color(rgb(0xd1d1d1))
            .text_size(px(13.))
            .child(
                h_flex()
                    .relative()
                    .h(px(37.))
                    .flex_none()
                    .justify_center()
                    .child(div().text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(blue).child(format!("Sheets ({})", self.drawing.sheets.len())))
                    .child(
                        div()
                            .id("dr-add-sheet")
                            .absolute()
                            .right(px(2.))
                            .top(px(4.))
                            .size(px(29.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .bg(rgb(0xffffff))
                            .shadow_sm()
                            .cursor_pointer()
                            .hover(|d| d.bg(rgb(0xeaeaea)))
                            .child(crate::assets::icon("sheet-insert", 20., cx))
                            .tooltip(crate::ui::tooltip("Insert sheet"))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.add_sheet_after(si, cx))),
                    ),
            )
            .child(div().h(px(2.)).flex_none().bg(blue))
            .child(h_flex().h(px(27.)).flex_none().bg(rgb(0xffffff)).border_b_1().border_color(rgb(0xdbdbdb)).child(tab("dr-sort-sheet", "Sort by sheet", !by_ref, cx)).child(tab("dr-sort-ref", "Sort by reference", by_ref, cx)))
            .child(list);
        div().absolute().top_0().left_0().size_full().child(panel).child(self.sheets_toggle(W, cx)).into_any_element()
    }

    // ------------------------------------------------------------ tools

    /// Enter / double-click: finish a jogged cutting line.
    pub fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.more_confirm(window, cx) {
            return;
        }
        if let Tool::JoggedSection { parent: Some(pid), points, done: false } = self.tool.clone()
            && points.len() >= 2
        {
            let pts = self.extend_line_ends(pid, &points);
            self.tool = Tool::JoggedSection { parent: Some(pid), points: pts, done: true };
            self.update_preview(cx);
        } else if let Tool::Ordinate { .. } = self.tool {
            self.set_tool(Tool::Select, cx);
        }
    }

    /// Extend the end legs of a cutting line past the view's geometry.
    fn extend_line_ends(&self, view: ViewId, points: &[[f64; 2]]) -> Vec<[f64; 2]> {
        let mut pts = points.to_vec();
        let Some(g) = self.geoms.get(&view) else { return pts };
        let n = pts.len();
        if n < 2 {
            return pts;
        }
        let margin = (g.max[0] - g.min[0]).max(g.max[1] - g.min[1]) * 0.08 + 1e-4;
        let corners = [[g.min[0] - margin, g.min[1] - margin], [g.max[0] + margin, g.min[1] - margin], [g.max[0] + margin, g.max[1] + margin], [g.min[0] - margin, g.max[1] + margin]];
        let reach = |from: [f64; 2], dir: [f64; 2]| corners.iter().map(|k| dot(sub(*k, from), dir)).fold(0.0f64, f64::max);
        let d0 = norm(sub(pts[0], pts[1]));
        let t0 = reach(pts[1], d0).max(len(sub(pts[0], pts[1])));
        pts[0] = add(pts[1], mul(d0, t0));
        let d1 = norm(sub(pts[n - 1], pts[n - 2]));
        let t1 = reach(pts[n - 2], d1).max(len(sub(pts[n - 1], pts[n - 2])));
        pts[n - 1] = add(pts[n - 2], mul(d1, t1));
        pts
    }

    /// A point in a view: snap, else the cursor position in the view.
    pub(crate) fn view_point(&self, view: Option<ViewId>) -> Option<PickedPoint> {
        let c = self.cursor;
        if let Some(p) = self.snap_at(c).filter(|p| view.is_none_or(|v| v == p.view)) {
            return Some(p);
        }
        let v = view.or_else(|| self.view_at(c))?;
        let pl = self.placement(v)?;
        Some(PickedPoint { view: v, p: pl.to_view(c), r: None })
    }

    /// Orthogonal continuation (in the view's frame) from `last` toward `target`.
    fn ortho_view(last: [f64; 2], target: [f64; 2]) -> [f64; 2] {
        let d = sub(target, last);
        if d[0].abs() >= d[1].abs() { [target[0], last[1]] } else { [last[0], target[1]] }
    }

    fn next_datum_label(&self) -> String {
        let used: Vec<String> = self.current_sheet().annotations.iter().filter_map(|a| if let AnnotationKind::Datum { label, .. } = &a.kind { Some(label.clone()) } else { None }).collect();
        "ABCDEFGHJKLMNPRTUVWY".chars().map(|c| c.to_string()).find(|l| !used.contains(l)).unwrap_or_else(|| "A".into())
    }

    /// Sheet direction perpendicular to the ordinate's measured axis.
    fn ordinate_normal(&self, view: ViewId, horizontal: bool) -> [f64; 2] {
        let rot = self.placement(view).map(|p| p.rotation).unwrap_or(0.0);
        views::rotate2(if horizontal { [0.0, 1.0] } else { [1.0, 0.0] }, rot)
    }

    pub fn extra_pending_annotation(&self, id: u64) -> Option<Annotation> {
        if let Some(a) = self.more_pending_annotation(id) {
            return Some(a);
        }
        let c = self.cursor;
        let refs = |v: Vec<(u8, Option<PointRef>)>| v.into_iter().filter_map(|(slot, r)| r.map(|point| AnnotationRef { slot, point })).collect::<Vec<_>>();
        match &self.tool {
            Tool::Dimension { kind: DimTool::Chamfer, picks } => {
                let [Pick::Line { a: a1, b: b1 }, Pick::Line { a: a2, b: b2 }] = picks.as_slice() else { return None };
                if a1.view != a2.view {
                    return None;
                }
                Some(Annotation {
                    id,
                    kind: AnnotationKind::ChamferDimension { view: a1.view, a: a1.p, b: b1.p, c: a2.p, d: b2.p },
                    position: c,
                    refs: refs(vec![(0, a1.r.clone()), (1, b1.r.clone()), (2, a2.r.clone()), (3, b2.r.clone())]),
                    ..Default::default()
                })
            }
            Tool::Dimension { kind: DimTool::ArcLength, picks } => {
                let [Pick::Arc { view, center, radius, a, b, mid, r }] = picks.as_slice() else { return None };
                let pl = self.placement(*view)?;
                let offset = (len(sub(c, pl.to_sheet(*center))) - pl.len_to_sheet(*radius)).max(2.0);
                Some(Annotation {
                    id,
                    kind: AnnotationKind::ArcLengthDimension { view: *view, center: *center, radius: *radius, a: a.p, b: b.p, mid: *mid, offset },
                    position: c,
                    refs: refs(vec![(0, r.clone()), (1, a.r.clone()), (2, b.r.clone())]),
                    ..Default::default()
                })
            }
            Tool::Ordinate { origin: Some(o), horizontal: None, .. } => {
                let os = self.pick_sheet(o)?;
                let rot = self.placement(o.view).map(|p| p.rotation).unwrap_or(0.0);
                let d = views::rotate2(sub(c, os), -rot);
                if len(d) < 3.0 {
                    return None;
                }
                let horizontal = d[1].abs() >= d[0].abs();
                let n = self.ordinate_normal(o.view, horizontal);
                let pos = add(os, mul(n, dot(sub(c, os), n)));
                // A new ordinate group holding the datum's own "0".
                Some(Annotation { id, kind: AnnotationKind::OrdinateGroup { view: o.view, origin: o.p, points: vec![o.p], horizontal }, position: pos, refs: refs(vec![(0, o.r.clone()), (1, o.r.clone())]), ..Default::default() })
            }
            Tool::Ordinate { origin: Some(o), horizontal: Some(_), group: Some(gid), .. } => {
                // The group with the point under the cursor added.
                let p = self.snap_at(c).filter(|p| p.view == o.view)?;
                let mut a = self.current_sheet().annotations.iter().find(|a| a.id == *gid)?.clone();
                let AnnotationKind::OrdinateGroup { points, .. } = &mut a.kind else { return None };
                if points.iter().any(|q| len(sub(*q, p.p)) < 1e-9) {
                    return None;
                }
                points.push(p.p);
                let slot = points.len() as u8;
                if let Some(r) = p.r.clone() {
                    a.refs.push(AnnotationRef { slot, point: r });
                }
                Some(a)
            }
            Tool::Datum { anchor: Some(a) } => {
                if len(sub(c, *a)) < 3.0 {
                    return None;
                }
                Some(Annotation { id, kind: AnnotationKind::Datum { anchor: *a, label: self.next_datum_label() }, position: c, ..Default::default() })
            }
            _ => None,
        }
    }

    /// Placement of a section view perpendicular to the first leg.
    fn section_position(&self, parent: &DrawingView, a: [f64; 2], b: [f64; 2]) -> Option<[f64; 2]> {
        let pl = self.placement(parent.id)?;
        let e = norm(sub(pl.to_sheet(b), pl.to_sheet(a)));
        let n = [-e[1], e[0]];
        let t = dot(sub(self.cursor, parent.position), n);
        Some(add(parent.position, mul(n, t)))
    }

    pub fn extra_pending_view(&self, id: ViewId) -> Option<DrawingView> {
        let sheet = self.drawing.sheets.get(self.sheet)?;
        match &self.tool {
            Tool::JoggedSection { parent: Some(pid), points, done: true } if points.len() >= 2 => {
                let parent = sheet.view(*pid)?;
                let pos = self.section_position(parent, points[0], points[1])?;
                Some(DrawingView { id, kind: ViewKind::Section { parent: *pid, line: points.clone(), label: sheet.next_label() }, position: pos, show_tangent_edges: parent.show_tangent_edges, ..Default::default() })
            }
            Tool::AlignedSection { parent: Some(pid), center: Some(c), a: Some(a), b: Some(b) } => {
                let parent = sheet.view(*pid)?;
                let pos = self.section_position(parent, *a, *c)?;
                Some(DrawingView { id, kind: ViewKind::AlignedSection { parent: *pid, center: *c, a: *a, b: *b, label: sheet.next_label() }, position: pos, show_tangent_edges: parent.show_tangent_edges, ..Default::default() })
            }
            _ => None,
        }
    }

    pub fn extra_overlay(&self, overlay: &mut Overlay) {
        let c = self.hover_snap.unwrap_or(self.cursor);
        match &self.tool {
            Tool::JoggedSection { parent: Some(pid), points, done } if !points.is_empty() => {
                let Some(pl) = self.placement(*pid) else { return };
                let mut pts: Vec<[f64; 2]> = points.iter().map(|p| pl.to_sheet(*p)).collect();
                if !done {
                    let last = *points.last().unwrap();
                    pts.push(pl.to_sheet(Self::ortho_view(last, pl.to_view(c))));
                }
                overlay.lines.push((pts, true));
            }
            Tool::AlignedSection { parent: Some(pid), center: Some(ctr), a, b: None } => {
                let Some(pl) = self.placement(*pid) else { return };
                let cs = pl.to_sheet(*ctr);
                let mut pts = vec![];
                if let Some(a) = a {
                    pts.push(pl.to_sheet(*a));
                }
                pts.push(cs);
                pts.push(c);
                overlay.lines.push((pts, true));
                overlay.points.push(cs);
            }
            Tool::Crop { parent: Some(_), first: Some(f), circle } => {
                if *circle {
                    overlay.circle(*f, len(sub(c, *f)), true);
                } else {
                    overlay.rect(*f, c, true);
                }
            }
            Tool::Break { parent: Some(pid), first, vertical } => {
                let Some(pl) = self.placement(*pid) else { return };
                let mut xs = vec![self.cursor];
                if let Some(f) = first {
                    xs.push(*f);
                }
                for x in xs {
                    if *vertical {
                        overlay.lines.push((vec![[x[0], pl.min[1] - 3.0], [x[0], pl.max[1] + 3.0]], true));
                    } else {
                        overlay.lines.push((vec![[pl.min[0] - 3.0, x[1]], [pl.max[0] + 3.0, x[1]]], true));
                    }
                }
            }
            Tool::Ordinate { origin: Some(o), .. } => {
                if let Some(s) = self.pick_sheet(o) {
                    overlay.points.push(s);
                }
            }
            Tool::Table { kind: TableTool::Hole, origin: Some(o), .. } => {
                if let Some(s) = self.pick_sheet(o) {
                    overlay.points.push(s);
                }
            }
            Tool::Table { kind, view, .. } if *kind == TableTool::General || view.is_some() => {
                let w = if *kind == TableTool::General { 60.0 } else { 90.0 };
                overlay.rect([self.cursor[0] - w, self.cursor[1] - 20.0], self.cursor, true);
            }
            Tool::Datum { anchor: Some(a) } => overlay.lines.push((vec![*a, self.cursor], true)),
            _ => self.more_overlay(overlay),
        }
    }

    /// Clicks of the tools defined here; false when the tool is not one of them.
    pub fn extra_tool_click(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.more_tool_click(window, cx) {
            return true;
        }
        let c = self.cursor;
        match self.tool.clone() {
            Tool::JoggedSection { parent, points, done: false } => {
                if points.is_empty() || parent.is_none() {
                    if let Some(pp) = self.view_point(parent) {
                        self.tool = Tool::JoggedSection { parent: Some(pp.view), points: vec![pp.p], done: false };
                    }
                } else if let Some(pid) = parent
                    && let Some(pp) = self.view_point(Some(pid))
                {
                    let last = *points.last().unwrap();
                    let q = Self::ortho_view(last, pp.p);
                    let mut pts = points.clone();
                    if len(sub(q, last)) > 1e-7 {
                        pts.push(q);
                    }
                    self.tool = Tool::JoggedSection { parent: Some(pid), points: pts, done: false };
                }
            }
            Tool::JoggedSection { done: true, .. } | Tool::AlignedSection { b: Some(_), .. } => {
                if let Some(v) = self.pending_view() {
                    let id = v.id;
                    let name = if matches!(v.kind, ViewKind::AlignedSection { .. }) { "Aligned section view" } else { "Section view" };
                    self.edit_sheet(name, cx, |s| s.views.push(v));
                    self.tool = Tool::Select;
                    self.selection = vec![Sel::View(id)];
                }
            }
            Tool::AlignedSection { parent, center: None, .. } => {
                if let Some(pp) = self.view_point(parent) {
                    self.tool = Tool::AlignedSection { parent: Some(pp.view), center: Some(pp.p), a: None, b: None };
                }
            }
            Tool::AlignedSection { parent: Some(pid), center: Some(ctr), a, b: None } => {
                if let Some(pp) = self.view_point(Some(pid))
                    && len(sub(pp.p, ctr)) > 1e-7
                {
                    let ext = self.extend_line_ends(pid, &[pp.p, ctr]);
                    match a {
                        None => self.tool = Tool::AlignedSection { parent: Some(pid), center: Some(ctr), a: Some(ext[0]), b: None },
                        Some(a) => self.tool = Tool::AlignedSection { parent: Some(pid), center: Some(ctr), a: Some(a), b: Some(ext[0]) },
                    }
                }
            }
            Tool::Crop { parent, first: None, circle } => {
                let v = parent.or_else(|| self.view_at(c));
                if v.is_some() {
                    let p = self.snap_at(c).and_then(|s| self.pick_sheet(&s)).unwrap_or(c);
                    self.tool = Tool::Crop { parent: v, first: Some(p), circle };
                }
            }
            Tool::Crop { parent: Some(v), first: Some(f), circle } => {
                let Some(pl) = self.placement(v) else { return true };
                let crop = if circle {
                    let r = len(sub(c, f)) / (pl.scale * 1000.0);
                    (r > 1e-7).then(|| Crop::Circle { center: pl.to_view(f), radius: r })
                } else {
                    let corners = [f, [c[0], f[1]], c, [f[0], c[1]]];
                    (len(sub(c, f)) > 1.0).then(|| Crop::Polygon(corners.iter().map(|k| pl.to_view(*k)).collect()))
                };
                if let Some(crop) = crop {
                    self.edit_view("Crop view", v, cx, |dv| dv.crop = Some(crop));
                }
                self.tool = Tool::Select;
            }
            Tool::Break { parent, first: None, vertical } => {
                let v = parent.or_else(|| self.view_at(c));
                if v.is_some() {
                    self.tool = Tool::Break { parent: v, first: Some(c), vertical };
                }
            }
            Tool::Break { parent: Some(v), first: Some(f), vertical } => {
                let Some(pl) = self.placement(v) else { return true };
                let (a, b) = (pl.to_view(f), pl.to_view(c));
                let k = if vertical { 0 } else { 1 };
                let (lo, hi) = (a[k].min(b[k]), a[k].max(b[k]));
                if hi - lo > 1e-6 {
                    self.edit_view("Break view", v, cx, |dv| dv.breaks.push(ViewBreak { vertical, a: lo, b: hi, gap_mm: 6.0, style: BreakStyle::ZigZag }));
                }
                self.tool = Tool::Select;
            }
            Tool::Ordinate { origin: None, .. } => {
                if let Some(p) = self.any_point(c) {
                    self.tool = Tool::Ordinate { origin: Some(p), horizontal: None, group: None };
                }
            }
            Tool::Ordinate { origin: Some(o), group, .. } => {
                if let Some(a) = self.pending_annotation() {
                    let AnnotationKind::OrdinateGroup { horizontal: h, .. } = a.kind.clone() else { return true };
                    let gid = a.id;
                    match group {
                        None => self.edit_sheet("Ordinate dimension", cx, |s| s.annotations.push(a)),
                        Some(_) => self.edit_sheet("Add to ordinate group", cx, |s| {
                            if let Some(x) = s.annotations.iter_mut().find(|x| x.id == gid) {
                                *x = a;
                            }
                        }),
                    }
                    self.tool = Tool::Ordinate { origin: Some(o), horizontal: Some(h), group: Some(gid) };
                }
            }
            Tool::Table { kind: TableTool::General, .. } => self.open_new_table_panel(c, window, cx),
            Tool::Table { kind, view: None, origin } => {
                if let Some(v) = self.view_at(c) {
                    self.tool = Tool::Table { kind, view: Some(v), origin };
                }
            }
            Tool::Table { kind: TableTool::Hole, view: Some(v), origin: None } => {
                if let Some(p) = self.snap_at(c).filter(|p| p.view == v) {
                    self.tool = Tool::Table { kind: TableTool::Hole, view: Some(v), origin: Some(p) };
                }
            }
            Tool::Table { kind, view: Some(v), origin } => {
                let id = self.current_sheet().next_table_id();
                let tk = match kind {
                    TableTool::Bom => TableKind::Bom { view: v },
                    _ => {
                        let o = origin.clone();
                        TableKind::Hole { view: v, origin: o.as_ref().map(|o| o.p).unwrap_or([0.0; 2]), origin_ref: o.and_then(|o| o.r), precision: None }
                    }
                };
                let name = if kind == TableTool::Bom { "BOM table" } else { "Hole table" };
                self.edit_sheet(name, cx, |s| s.tables.push(Table::new(id, tk, c)));
                self.tool = Tool::Select;
                self.selection = vec![Sel::Table(id)];
            }
            Tool::Gdt => self.open_gdt_panel(None, c, &[], window, cx),
            Tool::Datum { anchor: None } => {
                let p = self.snap_at(c).and_then(|s| self.pick_sheet(&s)).unwrap_or(c);
                self.tool = Tool::Datum { anchor: Some(p) };
            }
            Tool::Datum { anchor: Some(_) } => {
                if let Some(a) = self.pending_annotation() {
                    let id = a.id;
                    self.edit_sheet("Datum", cx, |s| s.annotations.push(a));
                    self.tool = Tool::Select;
                    self.selection = vec![Sel::Annotation(id)];
                }
            }
            Tool::SurfaceFinish => self.open_surface_finish_panel(None, c, "Ra 3.2", window, cx),
            _ => return false,
        }
        true
    }

    // ------------------------------------------------------------ drags

    pub fn extra_start_drag(&mut self, pick: Option<u32>, start: [f64; 2]) -> Option<Drag> {
        let target = self.built.as_ref()?.picks.get(pick? as usize)?.clone();
        let sheet = self.drawing.sheets.get(self.sheet)?;
        match target {
            PickTarget::Table(id) => {
                let t = sheet.table(id)?.clone();
                let l = self.built.as_ref()?.table_layout(id)?.clone();
                self.selection = vec![Sel::Table(id)];
                let grip = l.free_corner(t.corner);
                if len(sub(start, grip)) < self.tol_mm(9.0) {
                    let widths: Vec<f64> = l.xs.windows(2).map(|w| w[1] - w[0]).collect();
                    let size = [l.max()[0] - l.min()[0], l.max()[1] - l.min()[1]];
                    Some(Drag::ResizeTable { id, start, orig: t, size, widths })
                } else {
                    Some(Drag::MoveTable { id, start, orig: t.position })
                }
            }
            PickTarget::SectionLine(v) => {
                self.selection = vec![Sel::View(v)];
                // Near a point of the line: move that point only.
                if let Some(index) = self.section_point_at(v, start) {
                    return Some(Drag::MoveSectionPoint { view: v, index, orig: sheet.section_points(v)? });
                }
                Some(Drag::MoveSectionLine { view: v, start, orig: sheet.view(v)?.kind.clone() })
            }
            PickTarget::DetailCircle(v) => {
                self.selection = vec![Sel::View(v)];
                match sheet.view(v)?.kind {
                    ViewKind::Detail { center, .. } => Some(Drag::MoveDetailCircle { view: v, start, orig: center }),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub fn extra_drag_move(&mut self, drag: &Drag, cx: &mut Context<Self>) {
        let c = self.cursor;
        let mut dr = self.drawing.clone();
        let si = self.sheet;
        let Some(sheet) = dr.sheets.get_mut(si) else { return };
        match drag {
            Drag::MoveTable { id, start, orig } => {
                if let Some(t) = sheet.table_mut(*id) {
                    t.position = add(*orig, sub(c, *start));
                }
            }
            Drag::ResizeTable { id, start, orig, size, widths } => {
                let d = sub(c, *start);
                let sx = if matches!(orig.corner, TableCorner::TopLeft | TableCorner::BottomLeft) { 1.0 } else { -1.0 };
                let sy = if matches!(orig.corner, TableCorner::TopLeft | TableCorner::TopRight) { -1.0 } else { 1.0 };
                let kx = ((size[0] + d[0] * sx) / size[0]).max(0.3);
                let ky = ((size[1] + d[1] * sy) / size[1]).max(0.5);
                if let Some(t) = sheet.table_mut(*id) {
                    t.col_widths = widths.iter().map(|w| w * kx).collect();
                    t.row_height = (orig.row_height * ky).max(orig.text_height * 1.4);
                }
            }
            Drag::MoveSectionLine { view, start, orig } => {
                let Some(parent) = sheet.parent_of(*view) else { return };
                let Some(ppl) = self.placement(parent) else { return };
                let dv = sub(ppl.to_view(c), ppl.to_view(*start));
                let kind = match orig.clone() {
                    ViewKind::Section { parent, line, label } if line.len() >= 2 => {
                        let e = norm(sub(line[1], line[0]));
                        let n = [-e[1], e[0]];
                        let t = dot(dv, n);
                        ViewKind::Section { parent, line: line.iter().map(|p| add(*p, mul(n, t))).collect(), label }
                    }
                    ViewKind::AlignedSection { parent, center, a, b, label } => ViewKind::AlignedSection { parent, center: add(center, dv), a: add(a, dv), b: add(b, dv), label },
                    k => k,
                };
                if let Some(v) = sheet.view_mut(*view) {
                    v.kind = kind;
                }
            }
            Drag::MoveSectionPoint { view, index, orig } => {
                let Some(parent) = sheet.parent_of(*view) else { return };
                let Some(ppl) = self.placement(parent) else { return };
                let p = self.snap_at(c).filter(|s| s.view == parent).map(|s| s.p).unwrap_or_else(|| ppl.to_view(c));
                let ortho = matches!(sheet.view(*view).map(|v| &v.kind), Some(ViewKind::Section { .. }));
                sheet.set_section_points(*view, views::move_cut_point(orig, *index, p, ortho));
            }
            Drag::MoveDetailCircle { view, start, orig } => {
                let Some(parent) = sheet.parent_of(*view) else { return };
                let Some(ppl) = self.placement(parent) else { return };
                let dv = sub(ppl.to_view(c), ppl.to_view(*start));
                if let Some(ViewKind::Detail { center, .. }) = sheet.view_mut(*view).map(|v| &mut v.kind) {
                    *center = add(*orig, dv);
                }
            }
            _ => return,
        }
        self.set_preview(Some(dr), cx);
    }

    // ------------------------------------------------------------ menus

    pub fn view_extra_entries(&self, id: ViewId, cx: &App) -> Vec<MenuEntry> {
        let mut out = Vec::new();
        let sheet = &self.drawing.sheets[self.sheet];
        let Some(v) = sheet.view(id).cloned() else { return out };
        let deg = std::f64::consts::PI / 180.0;
        let rot = v.rotation;
        let mut rotate = vec![
            MenuEntry::item("Rotate 90° counter-clockwise", move |t, _, cx| t.edit_view("Rotate view", id, cx, |v| v.rotation += 90.0 * deg)),
            MenuEntry::item("Rotate 90° clockwise", move |t, _, cx| t.edit_view("Rotate view", id, cx, |v| v.rotation -= 90.0 * deg)),
            MenuEntry::item("Rotate 180°", move |t, _, cx| t.edit_view("Rotate view", id, cx, |v| v.rotation += 180.0 * deg)),
            MenuEntry::item("Custom angle…", move |t, w, cx| t.open_rotation_panel(id, w, cx)),
        ];
        if rot != 0.0 {
            rotate.push(MenuEntry::item("Reset rotation", move |t, _, cx| t.edit_view("Rotate view", id, cx, |v| v.rotation = 0.0)));
        }
        out.push(MenuEntry::Submenu("Rotate".into(), rotate));
        let mut crop = vec![
            MenuEntry::item("Rectangle", move |t, _, cx| t.set_tool(Tool::Crop { parent: Some(id), first: None, circle: false }, cx)),
            MenuEntry::item("Circle", move |t, _, cx| t.set_tool(Tool::Crop { parent: Some(id), first: None, circle: true }, cx)),
            MenuEntry::item("Polygon", move |t, _, cx| t.set_tool(Tool::CropPoly { parent: Some(id), points: vec![], spline: false }, cx)),
            MenuEntry::item("Spline", move |t, _, cx| t.set_tool(Tool::CropPoly { parent: Some(id), points: vec![], spline: true }, cx)),
        ];
        if v.crop.is_some() {
            crop.push(MenuEntry::item("Remove crop", move |t, _, cx| t.edit_view("Remove crop", id, cx, |v| v.crop = None)));
        }
        out.push(MenuEntry::Submenu("Crop".into(), crop));
        let mut brk = vec![
            MenuEntry::item("Vertical break lines", move |t, _, cx| t.set_tool(Tool::Break { parent: Some(id), first: None, vertical: true }, cx)),
            MenuEntry::item("Horizontal break lines", move |t, _, cx| t.set_tool(Tool::Break { parent: Some(id), first: None, vertical: false }, cx)),
        ];
        if !v.breaks.is_empty() {
            brk.push(MenuEntry::item("Remove all breaks", move |t, _, cx| t.edit_view("Remove breaks", id, cx, |v| v.breaks.clear())));
        }
        out.push(MenuEntry::Submenu("Break".into(), brk));
        out.push(MenuEntry::Submenu(
            "More section views".into(),
            vec![
                MenuEntry::item("Jogged section view", move |t, _, cx| t.set_tool(Tool::JoggedSection { parent: Some(id), points: vec![], done: false }, cx)),
                MenuEntry::item("Aligned section view", move |t, _, cx| t.set_tool(Tool::AlignedSection { parent: Some(id), center: None, a: None, b: None }, cx)),
                MenuEntry::item("Partial section view", move |t, _, cx| t.set_tool(Tool::Section { parent: Some(id), p0: None, line: None, partial: true }, cx)),
                MenuEntry::item("Broken-out section", move |t, _, cx| t.set_tool(Tool::BrokenOut { parent: Some(id), points: vec![], done: false }, cx)),
            ],
        ));
        if matches!(v.kind, ViewKind::Section { .. } | ViewKind::AlignedSection { .. }) {
            out.push(MenuEntry::item("Flip section direction", move |t, _, cx| t.flip_section(id, cx)));
        }
        // Source-dependent entries (root base view).
        if let Some(root) = views::root_view(sheet, id).and_then(|r| sheet.view(r)).cloned()
            && let ViewKind::Base { source, config, .. } = &root.kind
        {
            let rid = root.id;
            let doc = &self.doc.read(cx).session.doc;
            match doc.tab(*source).map(|t| &t.content) {
                Some(TabContent::PartStudio(ps)) if !ps.configuration.inputs.is_empty() => {
                    let mut cfg_entries = Vec::new();
                    for input in &ps.configuration.inputs {
                        let iid = input.id.clone();
                        match &input.kind {
                            noshape_model::configuration::ConfigInputKind::List { options, default } => {
                                let cur = config.0.get(&iid).cloned().unwrap_or(default.clone());
                                let opts: Vec<MenuEntry> = options
                                    .iter()
                                    .map(|o| {
                                        let (iid, oid) = (iid.clone(), o.id.clone());
                                        MenuEntry::check(o.name.clone(), cur == o.id, move |t, _, cx| t.set_view_config(rid, &iid, &oid, cx))
                                    })
                                    .collect();
                                cfg_entries.push(MenuEntry::Submenu(input.name.clone().into(), opts));
                            }
                            noshape_model::configuration::ConfigInputKind::Checkbox { default } => {
                                let on = config.0.get(&iid).map(|v| v == "true").unwrap_or(*default);
                                cfg_entries.push(MenuEntry::check(input.name.clone(), on, move |t, _, cx| t.set_view_config(rid, &iid, if on { "false" } else { "true" }, cx)));
                            }
                            _ => {}
                        }
                    }
                    if !cfg_entries.is_empty() {
                        out.push(MenuEntry::Submenu("Configuration".into(), cfg_entries));
                    }
                }
                Some(TabContent::Assembly(asm)) => {
                    let cur = root.explode;
                    let mut ex = vec![MenuEntry::check("Not exploded", cur.is_none(), move |t, _, cx| t.edit_view("Exploded view", rid, cx, |v| v.explode = None))];
                    for (i, e) in asm.exploded_views.iter().enumerate() {
                        ex.push(MenuEntry::check(e.name.clone(), cur == Some(i), move |t, _, cx| t.edit_view("Exploded view", rid, cx, |v| v.explode = Some(i))));
                    }
                    out.push(MenuEntry::Submenu("Exploded view".into(), ex));
                    out.push(MenuEntry::item("Insert BOM table", move |t, _, cx| t.set_tool(Tool::Table { kind: TableTool::Bom, view: Some(id), origin: None }, cx)));
                }
                _ => {}
            }
        }
        out.push(MenuEntry::item("Insert hole table", move |t, _, cx| t.set_tool(Tool::Table { kind: TableTool::Hole, view: Some(id), origin: None }, cx)));
        out
    }

    pub(crate) fn set_view_config(&mut self, view: ViewId, input: &str, value: &str, cx: &mut Context<Self>) {
        let (input, value) = (input.to_string(), value.to_string());
        self.edit_view("View configuration", view, cx, |v| {
            if let ViewKind::Base { config, .. } = &mut v.kind {
                config.0.insert(input, value);
            }
        });
        self.ensure_sources(false, cx);
    }

    /// Move a section view to the other side of its cutting line.
    pub(crate) fn flip_section(&mut self, id: ViewId, cx: &mut Context<Self>) {
        let sheet = &self.drawing.sheets[self.sheet];
        let (Some(parent), Some(cut)) = (sheet.parent_of(id), views::section_cut(sheet, id)) else { return };
        let (Some(pv), Some(v)) = (sheet.view(parent), sheet.view(id)) else { return };
        let prot = views::view_rotation(sheet, parent);
        let n = views::rotate2(cut.toward_view, prot);
        let d = sub(v.position, pv.position);
        let t = dot(d, n);
        let pos = sub(v.position, mul(n, 2.0 * t));
        self.edit_view("Flip section", id, cx, |v| v.position = pos);
    }

    pub fn annotation_extra_entries(&self, a: &Annotation) -> Vec<MenuEntry> {
        let mut out = Vec::new();
        let a2 = a.clone();
        if a.kind.is_dimension() {
            out.push(MenuEntry::item("Dimension properties…", move |t, w, cx| t.open_annotation_panel(&a2, w, cx)));
        } else if matches!(a.kind, AnnotationKind::Gdt { .. } | AnnotationKind::Datum { .. } | AnnotationKind::SurfaceFinish { .. } | AnnotationKind::Balloon { .. }) {
            out.push(MenuEntry::item("Edit…", move |t, w, cx| t.open_annotation_panel(&a2, w, cx)));
        }
        out
    }

    pub fn table_entries(&self, id: u64) -> Vec<MenuEntry> {
        let mut out = Vec::new();
        let sheet = &self.drawing.sheets[self.sheet];
        let Some(t) = sheet.table(id).cloned() else { return out };
        let cell = self.built.as_ref().and_then(|b| b.table_layout(id)).and_then(|l| l.cell_at(self.cursor));
        if let TableKind::General { .. } = t.kind {
            out.push(MenuEntry::item("Edit cell…", move |t, w, cx| t.open_cell_panel(id, w, cx)));
            let (r, c) = cell.map(|(r, c)| t.anchor_cell(r, c)).unwrap_or((0, 0));
            out.push(MenuEntry::item("Insert row above", move |t, _, cx| t.edit_table(id, "Insert row", cx, |t| t.insert_row(r))));
            out.push(MenuEntry::item("Insert row below", move |t, _, cx| t.edit_table(id, "Insert row", cx, |t| t.insert_row(r + 1))));
            out.push(MenuEntry::item("Insert column left", move |t, _, cx| t.edit_table(id, "Insert column", cx, |t| t.insert_col(c))));
            out.push(MenuEntry::item("Insert column right", move |t, _, cx| t.edit_table(id, "Insert column", cx, |t| t.insert_col(c + 1))));
            out.push(MenuEntry::item("Delete row", move |t, _, cx| t.edit_table(id, "Delete row", cx, |t| {
                t.delete_row(r);
            })));
            out.push(MenuEntry::item("Delete column", move |t, _, cx| t.edit_table(id, "Delete column", cx, |t| {
                t.delete_col(c);
            })));
            out.push(MenuEntry::Separator);
        }
        out.extend(self.table_more_entries(&t, cell));
        if let TableKind::Hole { precision, .. } = t.kind {
            let mut prec = vec![MenuEntry::check("Drawing default", precision.is_none(), move |t, _, cx| t.edit_table(id, "Hole table precision", cx, |t| if let TableKind::Hole { precision, .. } = &mut t.kind { *precision = None }))];
            for p in 0..=4u8 {
                prec.push(MenuEntry::check(format!("{p} decimals"), precision == Some(p), move |t, _, cx| t.edit_table(id, "Hole table precision", cx, |t| if let TableKind::Hole { precision, .. } = &mut t.kind { *precision = Some(p) })));
            }
            out.push(MenuEntry::Submenu("Precision".into(), prec));
        }
        let corner = t.corner;
        let corners = [("Top right", TableCorner::TopRight), ("Top left", TableCorner::TopLeft), ("Bottom right", TableCorner::BottomRight), ("Bottom left", TableCorner::BottomLeft)]
            .into_iter()
            .map(|(name, k)| {
                MenuEntry::check(name, corner == k, move |t, _, cx| {
                    // Keep the table where it is: move the anchor to the new corner.
                    let l = t.built.as_ref().and_then(|b| b.table_layout(id)).cloned();
                    t.edit_table(id, "Table fixed corner", cx, |t| {
                        if let Some(l) = &l {
                            let (lo, hi) = (l.min(), l.max());
                            t.position = match k {
                                TableCorner::TopLeft => [lo[0], hi[1]],
                                TableCorner::TopRight => [hi[0], hi[1]],
                                TableCorner::BottomLeft => [lo[0], lo[1]],
                                TableCorner::BottomRight => [hi[0], lo[1]],
                            };
                        }
                        t.corner = k;
                    })
                })
            })
            .collect();
        out.push(MenuEntry::Submenu("Fixed corner".into(), corners));
        let mut text = Vec::new();
        for h in [2.5, 3.0, 3.5, 5.0] {
            text.push(MenuEntry::check(format!("{h} mm"), (t.text_height - h).abs() < 1e-9, move |t, _, cx| t.edit_table(id, "Table text height", cx, |t| {
                t.text_height = h;
                t.col_widths.clear();
                t.row_height = (h * 2.2).max(7.0);
            })));
        }
        out.push(MenuEntry::Submenu("Text height".into(), text));
        out.push(MenuEntry::item("Fit to content", move |t, _, cx| t.edit_table(id, "Fit table", cx, |t| {
            t.col_widths.clear();
            t.row_height = (t.text_height * 2.2).max(7.0);
        })));
        out.push(MenuEntry::Separator);
        out.push(MenuEntry::item("Delete table", move |t, _, cx| {
            t.selection.clear();
            t.edit_sheet("Delete table", cx, |s| s.tables.retain(|x| x.id != id));
        }));
        out
    }

    pub(crate) fn edit_table(&mut self, id: u64, name: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut Table)) {
        self.edit_sheet(name, cx, |s| {
            if let Some(t) = s.table_mut(id) {
                f(t);
            }
        });
    }

    pub fn break_entries(&self, view: ViewId, index: usize) -> Vec<MenuEntry> {
        let mut out = Vec::new();
        let Some(b) = self.drawing.sheets[self.sheet].view(view).and_then(|v| v.breaks.get(index)).cloned() else { return out };
        let styles = [("Zig zag", BreakStyle::ZigZag), ("Straight", BreakStyle::Straight), ("Curve", BreakStyle::Curve)]
            .into_iter()
            .map(|(n, st)| MenuEntry::check(n, b.style == st, move |t, _, cx| t.edit_view("Break style", view, cx, |v| if let Some(b) = v.breaks.get_mut(index) { b.style = st })))
            .collect();
        out.push(MenuEntry::Submenu("Break line style".into(), styles));
        let gaps = [3.0, 6.0, 10.0, 15.0]
            .into_iter()
            .map(|g| MenuEntry::check(format!("{g} mm"), (b.gap_mm - g).abs() < 1e-9, move |t, _, cx| t.edit_view("Break gap", view, cx, |v| if let Some(b) = v.breaks.get_mut(index) { b.gap_mm = g })))
            .collect();
        out.push(MenuEntry::Submenu("Gap".into(), gaps));
        out.push(MenuEntry::Separator);
        out.push(MenuEntry::item("Delete break", move |t, _, cx| {
            t.selection.clear();
            t.edit_view("Delete break", view, cx, |v| {
                if index < v.breaks.len() {
                    v.breaks.remove(index);
                }
            })
        }));
        out
    }

    // ------------------------------------------------------------ panels

    pub fn open_sheet_name_panel(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.drawing.sheets.get(index).map(|s| s.name.clone()).unwrap_or_default();
        let input = self.input(&name, window, cx);
        Self::focus_input(&input, window, cx);
        self.panel = Some(Panel::SheetName { index, input });
        cx.notify();
    }

    pub fn open_rotation_panel(&mut self, view: ViewId, window: &mut Window, cx: &mut Context<Self>) {
        let cur = self.current_sheet().view(view).map(|v| v.rotation.to_degrees()).unwrap_or(0.0);
        let input = self.input(&format::format_number(cur, None, 3), window, cx);
        Self::focus_input(&input, window, cx);
        self.panel = Some(Panel::Rotation { view, input });
        cx.notify();
    }

    fn open_new_table_panel(&mut self, pos: [f64; 2], window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.input("4", window, cx);
        let cols = self.input("3", window, cx);
        let title = self.input("", window, cx);
        Self::focus_input(&rows, window, cx);
        self.panel = Some(Panel::NewTable { pos, rows, cols, title, header: true });
        cx.notify();
    }

    /// Double-click / "Edit cell…" on a general table: edit the cell under the cursor.
    pub fn open_cell_panel(&mut self, table: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(t) = self.current_sheet().table(table).cloned() else { return };
        let TableKind::General { cells, .. } = &t.kind else {
            self.status = Some(format!("{} content comes from the model", t.type_name()).into());
            cx.notify();
            return;
        };
        let (row, col) = self.built.as_ref().and_then(|b| b.table_layout(table)).and_then(|l| l.cell_at(self.cursor)).map(|(r, c)| t.anchor_cell(r, c)).unwrap_or((0, 0));
        let text = cells.get(row).and_then(|r| r.get(col)).cloned().unwrap_or_default();
        let input = self.textarea(&text, 3, window, cx);
        self.panel = Some(Panel::Cell { table, row, col, input });
        cx.notify();
    }

    fn open_gdt_panel(&mut self, edit: Option<u64>, pos: [f64; 2], frame: &[String], window: &mut Window, cx: &mut Context<Self>) {
        let symbol = frame.first().and_then(|s| GDT_SYMBOLS.iter().position(|(g, _)| g == s)).unwrap_or(0);
        let mut tol = frame.get(1).cloned().unwrap_or_else(|| "0.1".into());
        let diameter = tol.starts_with('Ø') || tol.starts_with('⌀');
        if diameter {
            tol = tol.chars().skip(1).collect();
        }
        let modifier = GDT_MODIFIERS.iter().skip(1).position(|(g, _)| tol.ends_with(g)).map(|i| i + 1).unwrap_or(0);
        if modifier > 0 {
            tol = tol.trim_end_matches(GDT_MODIFIERS[modifier].0).to_string();
        }
        let tolerance = self.input(&tol, window, cx);
        let datums = [0, 1, 2].map(|i| frame.get(i + 2).cloned().unwrap_or_default()).map(|d| self.input(&d, window, cx));
        Self::focus_input(&tolerance, window, cx);
        self.panel = Some(Panel::Gdt { edit, pos, symbol, diameter, modifier, tolerance, datums });
        cx.notify();
    }

    fn open_surface_finish_panel(&mut self, edit: Option<u64>, pos: [f64; 2], value: &str, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.input(value, window, cx);
        Self::focus_input(&input, window, cx);
        self.panel = Some(Panel::SurfaceFinish { edit, pos, input });
        cx.notify();
    }

    /// Properties / edit panel of an annotation.
    pub fn open_annotation_panel(&mut self, a: &Annotation, window: &mut Window, cx: &mut Context<Self>) {
        let Some(a) = self.drawing.sheets[self.sheet].annotations.iter().find(|x| x.id == a.id).cloned() else { return };
        match &a.kind {
            AnnotationKind::Gdt { anchor, frame, .. } => self.open_gdt_panel(Some(a.id), *anchor, frame, window, cx),
            AnnotationKind::SurfaceFinish { anchor, value } => self.open_surface_finish_panel(Some(a.id), *anchor, value, window, cx),
            AnnotationKind::Datum { anchor, label } => {
                let input = self.input(label, window, cx);
                Self::focus_input(&input, window, cx);
                self.panel = Some(Panel::Datum { edit: Some(a.id), anchor: *anchor, input });
                cx.notify();
            }
            AnnotationKind::Balloon { text, .. } => {
                // Reuse the datum panel layout for the callout text.
                let input = self.input(text, window, cx);
                Self::focus_input(&input, window, cx);
                self.panel = Some(Panel::Datum { edit: Some(a.id), anchor: [f64::NAN; 2], input });
                cx.notify();
            }
            k if k.is_dimension() => {
                let f = a.format.clone();
                let angular = matches!(k, AnnotationKind::AngularDimension { .. });
                let units = format::length_units(&self.drawing, &f);
                let show = |v: f64| if angular { format::format_number(v.to_degrees(), None, 4) } else { format::format_number(v / units.meters(), None, 6) };
                let (up, lo, hole, shaft) = match &f.tolerance {
                    Tolerance::Symmetric { value } => (show(*value), String::new(), String::new(), String::new()),
                    Tolerance::Deviation { upper, lower } | Tolerance::Limits { upper, lower } => (show(*upper), show(*lower), String::new(), String::new()),
                    Tolerance::Fit { hole, shaft } => (String::new(), String::new(), hole.clone(), shaft.clone()),
                    _ => (String::new(), String::new(), String::new(), String::new()),
                };
                let precision = self.input(&f.precision.map(|p| p.to_string()).unwrap_or_default(), window, cx);
                let upper = self.input(&up, window, cx);
                let lower = self.input(&lo, window, cx);
                let hole = self.input(&hole, window, cx);
                let shaft = self.input(&shaft, window, cx);
                let prefix = self.input(&f.prefix, window, cx);
                let suffix = self.input(&f.suffix, window, cx);
                let text = match k {
                    AnnotationKind::LinearDimension { override_text, .. } => Some(self.input(override_text.as_deref().unwrap_or(""), window, cx)),
                    _ => None,
                };
                Self::focus_input(&precision, window, cx);
                self.panel = Some(Panel::Dimension { id: a.id, angular, units: f.units, show_units: f.show_units, tol: tol_index(&f.tolerance), precision, upper, lower, hole, shaft, prefix, suffix, text });
                cx.notify();
            }
            _ => self.open_more_panel(&a, window, cx),
        }
    }

    pub(crate) fn label_row(label: &str, el: AnyElement) -> AnyElement {
        let pal = p();
        h_flex().gap_2().items_center().child(div().w(px(84.)).text_xs().text_color(pal.text_muted).child(label.to_string())).child(el).into_any_element()
    }

    /// Title and rows of the panels defined here.
    pub fn extra_panel_rows(&mut self, cx: &mut Context<Self>) -> Option<(&'static str, Vec<AnyElement>)> {
        let ed = cx.entity();
        let pal = p();
        let panel = self.panel.as_ref()?;
        Some(match panel {
            Panel::SheetName { input, .. } => ("Rename sheet", vec![Input::new(input).small().w(px(220.)).into_any_element()]),
            Panel::Rotation { input, .. } => ("View rotation", vec![Self::label_row("Angle (°)", Input::new(input).small().w(px(120.)).into_any_element())]),
            Panel::NewTable { rows, cols, title, header, .. } => {
                let header = *header;
                (
                    "Insert table",
                    vec![
                        Self::label_row("Rows", Input::new(rows).xsmall().w(px(80.)).into_any_element()),
                        Self::label_row("Columns", Input::new(cols).xsmall().w(px(80.)).into_any_element()),
                        Self::label_row("Title", Input::new(title).xsmall().w(px(200.)).into_any_element()),
                        Checkbox::new("dr-tbl-header")
                            .label("Header row")
                            .checked(header)
                            .on_change(cx.listener(|this, v: &bool, _, cx| {
                                if let Some(Panel::NewTable { header, .. }) = &mut this.panel {
                                    *header = *v;
                                }
                                cx.notify();
                            }))
                            .into_any_element(),
                    ],
                )
            }
            Panel::Cell { input, row, col, .. } => (
                "Edit cell",
                vec![
                    div().text_xs().text_color(pal.text_faint).child(format!("Row {}, column {} — fields like {{part.name}} allowed", row + 1, col + 1)).into_any_element(),
                    Textarea::new(input).w(px(300.)).into_any_element(),
                ],
            ),
            Panel::SurfaceFinish { input, .. } => ("Surface finish", vec![Self::label_row("Roughness", Input::new(input).small().w(px(160.)).into_any_element())]),
            Panel::Datum { input, anchor, .. } => (if anchor[0].is_nan() { "Callout text (empty = BOM item)" } else { "Datum" }, vec![Self::label_row("Label", Input::new(input).small().w(px(120.)).into_any_element())]),
            Panel::Gdt { symbol, diameter, modifier, tolerance, datums, .. } => {
                let (symbol, diameter, modifier) = (*symbol, *diameter, *modifier);
                let sym_entries: Vec<MenuEntry> = GDT_SYMBOLS
                    .iter()
                    .enumerate()
                    .map(|(i, (g, n))| {
                        MenuEntry::check(format!("{g}  {n}"), i == symbol, move |t, _, cx| {
                            if let Some(Panel::Gdt { symbol, .. }) = &mut t.panel {
                                *symbol = i;
                            }
                            cx.notify();
                        })
                    })
                    .collect();
                let mod_entries: Vec<MenuEntry> = GDT_MODIFIERS
                    .iter()
                    .enumerate()
                    .map(|(i, (g, n))| {
                        MenuEntry::check(format!("{g}  {n}"), i == modifier, move |t, _, cx| {
                            if let Some(Panel::Gdt { modifier, .. }) = &mut t.panel {
                                *modifier = i;
                            }
                            cx.notify();
                        })
                    })
                    .collect();
                let (e1, e2) = (ed.clone(), ed.clone());
                (
                    "Geometric tolerance",
                    vec![
                        Self::label_row(
                            "Characteristic",
                            Button::new("dr-gdt-sym").label(format!("{}  {}", GDT_SYMBOLS[symbol].0, GDT_SYMBOLS[symbol].1)).small().outline().dropdown_caret(true).dropdown_menu(move |m, w, cx| fill_menu(m, &sym_entries, &e1, w, cx)).into_any_element(),
                        ),
                        Self::label_row(
                            "Tolerance",
                            h_flex()
                                .gap_2()
                                .child(Checkbox::new("dr-gdt-dia").label("Ø").checked(diameter).on_change(cx.listener(|this, v: &bool, _, cx| {
                                    if let Some(Panel::Gdt { diameter, .. }) = &mut this.panel {
                                        *diameter = *v;
                                    }
                                    cx.notify();
                                })))
                                .child(Input::new(tolerance).xsmall().w(px(80.)))
                                .into_any_element(),
                        ),
                        Self::label_row(
                            "Modifier",
                            Button::new("dr-gdt-mod").label(if modifier == 0 { "None".to_string() } else { format!("{}  {}", GDT_MODIFIERS[modifier].0, GDT_MODIFIERS[modifier].1) }).small().outline().dropdown_caret(true).dropdown_menu(move |m, w, cx| fill_menu(m, &mod_entries, &e2, w, cx)).into_any_element(),
                        ),
                        Self::label_row(
                            "Datums",
                            h_flex().gap_1().children(datums.iter().map(|d| Input::new(d).xsmall().w(px(44.)))).into_any_element(),
                        ),
                    ],
                )
            }
            Panel::Dimension { angular, units, show_units, tol, precision, upper, lower, hole, shaft, prefix, suffix, text, .. } => {
                let (angular, units, show_units, tol) = (*angular, *units, *show_units, *tol);
                let mut unit_entries = vec![MenuEntry::check("Drawing units", units.is_none(), |t, _, cx| {
                    if let Some(Panel::Dimension { units, .. }) = &mut t.panel {
                        *units = None;
                    }
                    cx.notify();
                })];
                for u in DimUnits::ALL {
                    unit_entries.push(MenuEntry::check(u.name(), units == Some(u), move |t, _, cx| {
                        if let Some(Panel::Dimension { units, .. }) = &mut t.panel {
                            *units = Some(u);
                        }
                        cx.notify();
                    }));
                }
                let tol_entries: Vec<MenuEntry> = TOLERANCES
                    .iter()
                    .enumerate()
                    .map(|(i, n)| {
                        MenuEntry::check(*n, i == tol, move |t, _, cx| {
                            if let Some(Panel::Dimension { tol, .. }) = &mut t.panel {
                                *tol = i;
                            }
                            cx.notify();
                        })
                    })
                    .collect();
                let (e1, e2) = (ed.clone(), ed.clone());
                let mut rows = vec![
                    Self::label_row("Prefix / suffix", h_flex().gap_1().child(Input::new(prefix).xsmall().w(px(90.))).child(Input::new(suffix).xsmall().w(px(90.))).into_any_element()),
                    Self::label_row("Precision", h_flex().gap_2().child(Input::new(precision).xsmall().w(px(60.))).child(div().text_xs().text_color(pal.text_faint).child("decimals (empty = drawing)")).into_any_element()),
                ];
                if !angular {
                    rows.push(Self::label_row(
                        "Units",
                        h_flex()
                            .gap_2()
                            .child(Button::new("dr-dim-units").label(units.map(|u| u.name()).unwrap_or("Drawing units")).small().outline().dropdown_caret(true).dropdown_menu(move |m, w, cx| fill_menu(m, &unit_entries, &e1, w, cx)))
                            .child(Checkbox::new("dr-dim-showunits").label("Show units").checked(show_units).on_change(cx.listener(|this, v: &bool, _, cx| {
                                if let Some(Panel::Dimension { show_units, .. }) = &mut this.panel {
                                    *show_units = *v;
                                }
                                cx.notify();
                            })))
                            .into_any_element(),
                    ));
                }
                rows.push(Self::label_row("Tolerance", Button::new("dr-dim-tol").label(TOLERANCES[tol]).small().outline().dropdown_caret(true).dropdown_menu(move |m, w, cx| fill_menu(m, &tol_entries, &e2, w, cx)).into_any_element()));
                let unit_hint = if angular { "°" } else { "in dimension units" };
                match tol {
                    1 => rows.push(Self::label_row("±", h_flex().gap_2().child(Input::new(upper).xsmall().w(px(80.))).child(div().text_xs().text_color(pal.text_faint).child(unit_hint)).into_any_element())),
                    2 | 3 => {
                        rows.push(Self::label_row("Upper", Input::new(upper).xsmall().w(px(80.)).into_any_element()));
                        rows.push(Self::label_row("Lower", h_flex().gap_2().child(Input::new(lower).xsmall().w(px(80.))).child(div().text_xs().text_color(pal.text_faint).child(unit_hint)).into_any_element()));
                    }
                    7 => rows.push(Self::label_row("Hole / shaft", h_flex().gap_1().child(Input::new(hole).xsmall().w(px(60.))).child(Input::new(shaft).xsmall().w(px(60.))).into_any_element())),
                    _ => {}
                }
                if let Some(t) = text {
                    rows.push(Self::label_row("Override text", Input::new(t).xsmall().w(px(180.)).into_any_element()));
                }
                ("Dimension properties", rows)
            }
            _ => return None,
        })
    }

    pub fn apply_extra_panel(&mut self, panel: Panel, cx: &mut Context<Self>) {
        let val = |e: &Entity<gpui_kit::component::input::InputState>, cx: &App| e.read(cx).value().to_string();
        match panel {
            Panel::SheetName { index, input } => {
                let name = val(&input, cx);
                self.edit("Rename sheet", cx, |d| {
                    d.rename_sheet(index, &name);
                });
            }
            Panel::Rotation { view, input } => match val(&input, cx).trim().trim_end_matches('°').parse::<f64>() {
                Ok(deg) => self.edit_view("Rotate view", view, cx, |v| v.rotation = deg.to_radians()),
                Err(_) => self.status = Some("Invalid angle".into()),
            },
            Panel::NewTable { pos, rows, cols, title, header } => {
                let r = val(&rows, cx).trim().parse::<usize>().unwrap_or(3).clamp(1, 60);
                let c = val(&cols, cx).trim().parse::<usize>().unwrap_or(3).clamp(1, 20);
                let title = val(&title, cx);
                let mut cells = vec![vec![String::new(); c]; r];
                if header {
                    for (i, cell) in cells[0].iter_mut().enumerate() {
                        *cell = format!("HEADER {}", i + 1);
                    }
                }
                let id = self.current_sheet().next_table_id();
                let mut t = Table::new(id, TableKind::General { cells, title: (!title.trim().is_empty()).then_some(title), header }, pos);
                t.col_widths = vec![28.0; c];
                self.edit_sheet("Table", cx, |s| s.tables.push(t));
                self.tool = Tool::Select;
                self.selection = vec![Sel::Table(id)];
            }
            Panel::Cell { table, row, col, input } => {
                let text = input.read(cx).value().to_string();
                self.edit_table(table, "Edit cell", cx, |t| {
                    if let TableKind::General { cells, .. } = &mut t.kind {
                        while cells.len() <= row {
                            cells.push(vec![]);
                        }
                        let n = cells.iter().map(|r| r.len()).max().unwrap_or(0).max(col + 1);
                        for r in cells.iter_mut() {
                            r.resize(n, String::new());
                        }
                        cells[row][col] = text.clone();
                    }
                    // Grow the column to fit when needed.
                    let th = t.text_height;
                    let need = text.lines().map(|l| noshape_model::drawing::font::text_width(l, th)).fold(0.0, f64::max) + th * 1.6;
                    let ncols = match &t.kind {
                        TableKind::General { cells, .. } => cells.first().map(|r| r.len()).unwrap_or(0),
                        _ => 0,
                    };
                    if t.col_widths.len() == ncols && let Some(w) = t.col_widths.get_mut(col) {
                        *w = w.max(need);
                    } else {
                        t.col_widths.clear();
                    }
                });
            }
            Panel::SurfaceFinish { edit, pos, input } => {
                let value = val(&input, cx);
                match edit {
                    Some(id) => self.edit_sheet("Edit surface finish", cx, |s| {
                        if let Some(AnnotationKind::SurfaceFinish { value: v, .. }) = s.annotations.iter_mut().find(|a| a.id == id).map(|a| &mut a.kind) {
                            *v = value;
                        }
                    }),
                    None => {
                        let id = self.current_sheet().next_annotation_id();
                        self.edit_sheet("Surface finish", cx, |s| s.annotations.push(Annotation { id, kind: AnnotationKind::SurfaceFinish { anchor: pos, value }, position: pos, ..Default::default() }));
                        self.tool = Tool::Select;
                    }
                }
            }
            Panel::Datum { edit: Some(id), input, .. } => {
                let label = val(&input, cx);
                self.edit_sheet("Edit annotation", cx, |s| match s.annotations.iter_mut().find(|a| a.id == id).map(|a| &mut a.kind) {
                    Some(AnnotationKind::Datum { label: l, .. }) => *l = label,
                    Some(AnnotationKind::Balloon { text, .. }) => *text = label,
                    _ => {}
                });
            }
            Panel::Datum { edit: None, .. } => {}
            p @ (Panel::BrokenOut { .. } | Panel::CutAngle { .. } | Panel::CellFormat { .. } | Panel::DatumTarget { .. }) => self.apply_more_panel(p, cx),
            Panel::Gdt { edit, pos, symbol, diameter, modifier, tolerance, datums } => {
                let tol = val(&tolerance, cx);
                let mut frame = vec![GDT_SYMBOLS[symbol].0.to_string(), format!("{}{}{}", if diameter { "Ø" } else { "" }, tol.trim(), GDT_MODIFIERS[modifier].0)];
                for d in &datums {
                    let d = val(d, cx);
                    if !d.trim().is_empty() {
                        frame.push(d.trim().to_string());
                    }
                }
                match edit {
                    Some(id) => self.edit_sheet("Edit geometric tolerance", cx, |s| {
                        if let Some(AnnotationKind::Gdt { frame: f, .. }) = s.annotations.iter_mut().find(|a| a.id == id).map(|a| &mut a.kind) {
                            *f = frame;
                        }
                    }),
                    None => {
                        let id = self.current_sheet().next_annotation_id();
                        self.edit_sheet("Geometric tolerance", cx, |s| s.annotations.push(Annotation { id, kind: AnnotationKind::Gdt { anchor: pos, frame, leaders: vec![] }, position: pos, ..Default::default() }));
                        self.tool = Tool::Select;
                        self.selection = vec![Sel::Annotation(id)];
                    }
                }
            }
            Panel::Dimension { id, angular, units, show_units, tol, precision, upper, lower, hole, shaft, prefix, suffix, text } => {
                let mut f = DimFormat { precision: val(&precision, cx).trim().parse::<u8>().ok().map(|p| p.min(8)), units, show_units, prefix: val(&prefix, cx), suffix: val(&suffix, cx), tolerance: Tolerance::None };
                let u = format::length_units(&self.drawing, &f);
                let num = |s: String| -> f64 {
                    let v = s.trim().trim_start_matches('+').parse::<f64>().unwrap_or(0.0);
                    if angular { v.to_radians() } else { v * u.meters() }
                };
                f.tolerance = match tol {
                    1 => Tolerance::Symmetric { value: num(val(&upper, cx)).abs() },
                    2 => Tolerance::Deviation { upper: num(val(&upper, cx)), lower: num(val(&lower, cx)) },
                    3 => Tolerance::Limits { upper: num(val(&upper, cx)), lower: num(val(&lower, cx)) },
                    4 => Tolerance::Basic,
                    5 => Tolerance::Min,
                    6 => Tolerance::Max,
                    7 => Tolerance::Fit { hole: val(&hole, cx).trim().to_string(), shaft: val(&shaft, cx).trim().to_string() },
                    _ => Tolerance::None,
                };
                let over = text.map(|t| val(&t, cx));
                self.edit_sheet("Dimension properties", cx, |s| {
                    if let Some(a) = s.annotations.iter_mut().find(|a| a.id == id) {
                        a.format = f;
                        if let (Some(o), AnnotationKind::LinearDimension { override_text, .. }) = (over, &mut a.kind) {
                            *override_text = (!o.trim().is_empty()).then_some(o);
                        }
                    }
                });
            }
            _ => {}
        }
        cx.notify();
    }
}
