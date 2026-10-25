//! Drawing editor: Onshape's Hatch region dialog (live drawing/468–472,
//! dom/drawing-hatch-dialog.txt, drawing-hatch-lists.json). Profile tools
//! (Center point circle ▾, Corner rectangle ▾, Spline, Polygon); the
//! "Closed profiles" list (a click inside a closed sketch profile or a
//! region of a view adds "Hatch N" ×); tabs ANSI | ISO | General; Pattern
//! (icon list); Scale and Angle (editable, Auto / 1.0–10.0 / 0.0°–180.0°;
//! not on General); Color (swatch + ☑ Drawing properties); a preview box.
//! While the dialog is open the hatch shows tan with its profile in
//! reference blue. ✓ adds a region per profile (or, from "Edit hatch",
//! updates the region).

use crate::drawing_sketch::SkTool;
use crate::drawing_view::*;
use crate::os_ui::OsDropdownExt as _;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::drawing::ViewId;
use noshape_model::drawing::hatch_pat::{self, GROUPS};
use noshape_model::drawing::sheet_sketch::HatchRegion;

/// A closed profile: outer boundary and holes (sheet mm).
pub type Profile = (Vec<[f64; 2]>, Vec<Vec<[f64; 2]>>);
/// The hatch preview on the sheet while the dialog is open (live: tan).
const PREVIEW_TAN: [u8; 3] = [0xcc, 0xb2, 0x92];
/// Dialog ink and the field rules.
const INK: u32 = 0x333333;
const RULE: u32 = 0xc9c9c9;

/// Hatch region dialog state.
pub struct HatchDlg {
    /// Editing this region (None: new ones).
    pub edit: Option<u64>,
    /// Pattern group (tab) and pattern ([`hatch_pat::pattern`]).
    pub standard: usize,
    pub pattern: usize,
    /// "Auto" or a number.
    pub scale: Entity<InputState>,
    /// "Auto" or degrees.
    pub angle: Entity<InputState>,
    /// Use the drawing's hatch color, else `color`.
    pub drawing_color: bool,
    pub color: Option<[u8; 3]>,
    /// The picked closed profiles, each in a view (view coordinates) or on
    /// the sheet.
    pub profiles: Vec<(Profile, Option<ViewId>)>,
    /// Edit hatch section... (live asb 10-23 views (4)): the picked hatch
    /// regions of views' cuts (a point inside each, view coordinates);
    /// None: the Hatch region dialog.
    pub sections: Option<Vec<(ViewId, [f64; 2])>>,
    _subs: [Subscription; 2],
}

/// General › None in Edit hatch section... (the region unhatched).
const NO_PATTERN: usize = usize::MAX;

impl HatchDlg {
    /// The dialog's options as typed (see `noshape_scene::drawing_tools::HatchOpts`).
    fn opts(&self, cx: &App) -> noshape_scene::drawing_tools::HatchOpts {
        noshape_scene::drawing_tools::HatchOpts {
            standard: self.standard,
            pattern: self.pattern,
            scale: self.scale.read(cx).value().to_string(),
            angle: self.angle.read(cx).value().to_string(),
            drawing_color: self.drawing_color,
            color: self.color,
            profiles: self.profiles.clone(),
        }
    }

    fn scale_value(&self, cx: &App) -> f64 {
        self.opts(cx).scale_value()
    }

    /// The angle added to the pattern's own.
    fn angle_value(&self, cx: &App) -> f64 {
        self.opts(cx).angle_value()
    }

    fn regions(&self, first_id: u64, cx: &App) -> Vec<HatchRegion> {
        self.opts(cx).regions(first_id)
    }
}

impl DrawingEditor {
    /// Toolbar Hatch region / context menu "Edit hatch".
    pub fn open_hatch_dialog(&mut self, edit: Option<u64>, window: &mut Window, cx: &mut Context<Self>) {
        if self.hatch_dlg.is_some() && edit.is_none() {
            self.hatch_dlg = None;
            self.set_preview(None, cx);
            return;
        }
        let existing = edit.and_then(|id| self.sketch().hatches.iter().find(|h| h.id == id).cloned());
        let (scale_text, angle_text) = match &existing {
            Some(h) => (
                if (h.scale - 1.0).abs() < 1e-9 { "Auto".to_string() } else { format!("{:.1}", h.scale) },
                if h.angle.abs() < 1e-9 { "Auto".to_string() } else { format!("{:.1}°", (h.angle + hatch_pat::base_angle(h.standard, h.pattern)).rem_euclid(360.0)) },
            ),
            None => ("Auto".into(), "Auto".into()),
        };
        let scale = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(scale_text));
        let angle = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(angle_text));
        let sub = |st: &Entity<InputState>, cx: &mut Context<Self>| {
            cx.subscribe(st, |this: &mut Self, _, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.hatch_preview(cx);
                }
            })
        };
        let subs = [sub(&scale, cx), sub(&angle, cx)];
        self.hatch_dlg = Some(match existing {
            Some(h) => HatchDlg { edit, standard: h.standard, pattern: h.pattern, scale, angle, drawing_color: h.color.is_none(), color: h.color, profiles: vec![((h.boundary, h.holes), h.view)], sections: None, _subs: subs },
            None => HatchDlg { edit: None, standard: 0, pattern: hatch_pat::default_pattern(0), scale, angle, drawing_color: true, color: None, profiles: Vec::new(), sections: None, _subs: subs },
        });
        self.tool = Tool::Select;
        self.status = None;
        self.hatch_preview(cx);
    }

    /// The view menu's Edit hatch section...: the Edit hatch panel with an
    /// empty Hatches box (live asbv-36).
    pub fn open_section_hatch_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_hatch_dialog(None, window, cx);
        if self.hatch_dlg.is_none() {
            self.open_hatch_dialog(None, window, cx);
        }
        if let Some(d) = &mut self.hatch_dlg {
            d.sections = Some(Vec::new());
        }
        self.hatch_preview(cx);
    }

    /// The hatch regions of view `v`'s cut (view coordinates).
    fn section_regions_of(&self, v: ViewId) -> Option<Vec<noshape_model::drawing::views::HatchArea>> {
        let g = self.geoms.get(&v)?;
        noshape_model::drawing::views::section_regions(&g.section_segments)
    }

    /// Edit hatch: a click on a hatch region of a view's cut adds it (or,
    /// picked already, removes it); the first one sets the dialog to its
    /// current style.
    fn section_hatch_click(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let c = self.cursor;
        let Some(view) = self.view_at(c) else { return };
        let (Some(pl), Some(regions)) = (self.placement(view), self.section_regions_of(view)) else { return };
        let at = pl.to_view(c);
        let region_at = noshape_model::drawing::views::section_region_at;
        let Some(k) = region_at(&regions, at) else { return };
        let current = self.current_sheet().view(view).and_then(|v| noshape_model::drawing::views::section_region_styles(&regions, &v.section_hatches)[k].cloned());
        let Some(d) = &mut self.hatch_dlg else { return };
        let Some(list) = &mut d.sections else { return };
        let mut texts = None;
        if let Some(i) = list.iter().position(|(v, p)| *v == view && region_at(&regions, *p) == Some(k)) {
            list.remove(i);
        } else {
            let first = list.is_empty();
            list.push((view, at));
            if first {
                let h = current.unwrap_or(noshape_model::drawing::SectionHatch { at, standard: 0, pattern: Some(hatch_pat::default_pattern(0)), scale: 1.0, angle: 0.0, color: None });
                d.standard = h.standard;
                d.pattern = h.pattern.unwrap_or(NO_PATTERN);
                d.drawing_color = h.color.is_none();
                d.color = h.color;
                // Scale / Angle as the Hatch region dialog shows them.
                let scale = if (h.scale - 1.0).abs() < 1e-9 { "Auto".to_string() } else { format!("{:.1}", h.scale) };
                let base = h.pattern.map(|p| hatch_pat::base_angle(h.standard, p)).unwrap_or(0.0);
                let angle = if h.angle.abs() < 1e-9 { "Auto".to_string() } else { format!("{:.1}°", (h.angle + base).rem_euclid(360.0)) };
                texts = Some((d.scale.clone(), scale, d.angle.clone(), angle));
            }
        }
        if let Some((s_in, s, a_in, a)) = texts {
            s_in.update(cx, |x, cx| x.set_value(s, window, cx));
            a_in.update(cx, |x, cx| x.set_value(a, window, cx));
        }
        self.hatch_preview(cx);
    }

    /// The dialog's style for the picked regions (None: the drawing's hatch,
    /// "ANSI31 general (drawing)" at Auto in the drawing's color).
    fn section_style(&self, cx: &App) -> Option<noshape_model::drawing::SectionHatch> {
        let d = self.hatch_dlg.as_ref()?;
        let (scale, angle) = (d.scale_value(cx), d.angle_value(cx));
        let default = d.standard == 0 && d.pattern == hatch_pat::default_pattern(0) && (scale - 1.0).abs() < 1e-9 && angle.abs() < 1e-9 && d.drawing_color;
        (!default).then(|| noshape_model::drawing::SectionHatch {
            at: [0.0, 0.0],
            standard: d.standard,
            pattern: (d.pattern != NO_PATTERN).then_some(d.pattern),
            scale,
            angle,
            color: if d.drawing_color { None } else { d.color },
        })
    }

    /// The drawing with the dialog's style on the picked regions (`tint`:
    /// the preview's tan).
    fn section_hatched(&self, tint: bool, cx: &App) -> Option<noshape_model::drawing::Drawing> {
        let list = self.hatch_dlg.as_ref()?.sections.clone()?;
        let style = self.section_style(cx);
        let mut dr = self.drawing.clone();
        for (view, at) in list {
            let Some(geo) = self.geoms.get(&view).cloned() else { continue };
            let Some(v) = dr.sheets.get_mut(self.sheet).and_then(|s| s.view_mut(view)) else { continue };
            // The preview shows the default hatch tan too.
            let st = match (&style, tint) {
                (Some(h), true) => Some(noshape_model::drawing::SectionHatch { color: Some(PREVIEW_TAN), ..h.clone() }),
                (None, true) => Some(noshape_model::drawing::SectionHatch { at, standard: 0, pattern: Some(hatch_pat::default_pattern(0)), scale: 1.0, angle: 0.0, color: Some(PREVIEW_TAN) }),
                (s, false) => s.clone(),
            };
            noshape_scene::drawing_tools::set_section_hatch(v, &geo, at, st);
        }
        Some(dr)
    }

    /// A click while the dialog is open (no sketch tool): add the closed
    /// sketch profile (else the region of a view) under the cursor. True
    /// when used.
    pub fn hatch_click(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.hatch_dlg.is_none() || !matches!(self.tool, Tool::Select) {
            return false;
        }
        if self.hatch_dlg.as_ref().is_some_and(|d| d.sections.is_some()) {
            self.section_hatch_click(window, cx);
            return true;
        }
        let Some(picked) = self.pk().hatch_profile_at(self.cursor) else { return true };
        if let Some(d) = &mut self.hatch_dlg {
            noshape_scene::drawing_tools::hatch_toggle(&mut d.profiles, picked, d.edit.is_some());
        }
        self.hatch_preview(cx);
        true
    }

    /// Ids for the dialog's regions: the edited one, else new ones.
    fn hatch_first_id(&self) -> u64 {
        self.hatch_dlg.as_ref().and_then(|d| d.edit).unwrap_or_else(|| self.sketch().next_hatch_id())
    }

    /// Show the hatches (tan) in the preview while the dialog is open.
    fn hatch_preview(&mut self, cx: &mut Context<Self>) {
        if let Some(dr) = self.section_hatched(true, cx) {
            self.set_preview(Some(dr), cx);
            cx.notify();
            return;
        }
        let Some(d) = &self.hatch_dlg else { return };
        let first = self.hatch_first_id();
        let mut regions = d.regions(first, cx);
        for r in &mut regions {
            r.color = Some(PREVIEW_TAN);
        }
        let mut dr = self.drawing.clone();
        let hs = &mut dr.sheets[self.sheet].sketch.hatches;
        if let Some(id) = d.edit {
            hs.retain(|x| x.id != id);
        }
        hs.extend(regions);
        self.set_preview(Some(dr), cx);
        cx.notify();
    }

    /// The dialog's profiles on the sheet (for the reference-blue outline).
    pub fn hatch_profile_outlines(&self) -> Vec<Vec<[f64; 2]>> {
        let Some(d) = &self.hatch_dlg else { return vec![] };
        let mut out = Vec::new();
        for ((outer, holes), view) in &d.profiles {
            let pl = view.and_then(|v| self.placement(v));
            let map = |l: &Vec<[f64; 2]>| -> Vec<[f64; 2]> {
                let mut v: Vec<[f64; 2]> = l.iter().map(|p| pl.as_ref().map_or(*p, |pl| pl.to_sheet(*p))).collect();
                if let Some(f) = v.first().copied() {
                    v.push(f);
                }
                v
            };
            out.push(map(outer));
            out.extend(holes.iter().map(map));
        }
        out
    }

    fn accept_hatch(&mut self, cx: &mut Context<Self>) {
        // Edit hatch section...: the picked regions take the style.
        if let Some(dr) = self.section_hatched(false, cx) {
            let picked = self.hatch_dlg.take().and_then(|d| d.sections).is_some_and(|l| !l.is_empty());
            self.preview = None;
            if picked {
                let s = dr.sheets[self.sheet].clone();
                self.edit_sheet("Edit hatch", cx, |x| *x = s);
            } else {
                self.refresh(cx);
            }
            cx.notify();
            return;
        }
        let first = self.hatch_first_id();
        let Some(d) = self.hatch_dlg.take() else { return };
        self.preview = None;
        let regions = d.regions(first, cx);
        if regions.is_empty() {
            self.refresh(cx);
        } else {
            let edit = d.edit;
            self.edit_sheet(if edit.is_some() { "Edit hatch" } else { "Hatch region" }, cx, |s| noshape_scene::drawing_tools::apply_hatches(&mut s.sketch, edit, regions));
        }
        self.status = None;
        cx.notify();
    }

    pub fn cancel_hatch(&mut self, cx: &mut Context<Self>) -> bool {
        if self.hatch_dlg.take().is_none() {
            return false;
        }
        self.set_preview(None, cx);
        true
    }

    fn set_hatch(&mut self, f: impl FnOnce(&mut HatchDlg), cx: &mut Context<Self>) {
        if let Some(d) = &mut self.hatch_dlg {
            f(d);
        }
        self.hatch_preview(cx);
    }

    pub fn render_hatch_dlg(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let d = self.hatch_dlg.as_ref()?;
        let pal = p();
        let ed = cx.entity();
        let ink = gpui_kit::rgb(INK);
        let (standard, pattern, drawing_color) = (d.standard, d.pattern, d.drawing_color);
        let general = standard == 2;
        // Edit hatch section... (live asbv-36/37): "Edit hatch", no profile
        // tools, a Hatches box (chips named after the view); the rest once a
        // region is picked; General › None hides Scale, Angle and Color.
        let section_mode = d.sections.is_some();
        let picked = d.sections.as_ref().is_some_and(|l| !l.is_empty());
        let none = pattern == NO_PATTERN;
        let row = |label: &'static str, body: AnyElement| h_flex().h(px(34.)).pl(px(5.)).child(div().w(px(56.)).text_size(px(11.5)).child(label)).child(body).into_any_element();
        // Profile tools (live: circle ▾, rectangle ▾, spline, polygon).
        let cur = match &self.tool {
            Tool::Sketch { tool, .. } => Some(*tool),
            _ => None,
        };
        let menu = |items: &[(SkTool, &'static str, &'static str)]| -> Vec<MenuEntry> {
            items.iter().map(|&(t, label, icon)| MenuEntry::item(label, move |e, _, cx| e.set_tool(Tool::Sketch { tool: t, pts: vec![], from: None }, cx)).with_icon(icon)).collect()
        };
        let group = |id: &'static str, main: (SkTool, &'static str, &'static str), items: Vec<MenuEntry>, on: bool, cx: &mut Context<Self>| {
            let ed = ed.clone();
            crate::ui::tool_group(
                id,
                crate::ui::split_tool_button(SharedString::from(format!("{id}-main")), main.2, on, cx).tooltip(main.1).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_tool(Tool::Sketch { tool: main.0, pts: vec![], from: None }, cx))),
                crate::ui::tool_caret(SharedString::from(format!("{id}-more")), cx).tooltip(main.1).on_click(move |e, w, cx| open_tool_menu(&items, &ed, e, w, cx)),
            )
            .into_any_element()
        };
        let tool = |id: &'static str, t: SkTool, icon: &'static str, tip: &'static str, cx: &mut Context<Self>| {
            crate::ui::tool_button(id, icon, cur == Some(t), cx).tooltip(tip).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_tool(Tool::Sketch { tool: t, pts: vec![], from: None }, cx))).into_any_element()
        };
        let tools = h_flex()
            .pl(px(4.))
            .pb(px(3.))
            .gap(px(2.))
            .child(group("hd-circle", (SkTool::Circle, "Center point circle", "circle"), menu(&[(SkTool::Circle, "Center point circle (c)", "circle"), (SkTool::Circle3, "3 point circle", "three-point-circle")]), matches!(cur, Some(SkTool::Circle | SkTool::Circle3)), cx))
            .child(group("hd-rect", (SkTool::CornerRect, "Corner rectangle", "corner-rectangle"), menu(&[(SkTool::CornerRect, "Corner rectangle (g)", "corner-rectangle"), (SkTool::CenterRect, "Center point rectangle (r)", "center-rectangle")]), matches!(cur, Some(SkTool::CornerRect | SkTool::CenterRect)), cx))
            .child(tool("hd-spline", SkTool::Spline, "spline", "Spline", cx))
            .child(tool("hd-polygon", SkTool::Line, "drawing-polygon", "Polygon", cx));
        // Closed profiles (live: a light blue list box, "Hatch N" ×).
        let rows: Vec<AnyElement> = d
            .profiles
            .iter()
            .enumerate()
            .map(|(i, _)| {
                h_flex()
                    .id(("hd-profile", i))
                    .h(px(20.))
                    .text_size(px(13.))
                    .child(div().flex_1().child(format!("Hatch {}", i + 1)))
                    .child(div().id(("hd-profile-x", i)).px(px(4.)).text_color(gpui_kit::rgb(0x9a9a9a)).cursor_pointer().child("×").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.set_hatch(
                            |d| {
                                if i < d.profiles.len() {
                                    d.profiles.remove(i);
                                }
                            },
                            cx,
                        )
                    })))
                    .into_any_element()
            })
            .collect();
        let rows = match &d.sections {
            Some(list) => list
                .iter()
                .enumerate()
                .map(|(i, (v, _))| {
                    let name = self.current_sheet().view(*v).map(|x| match &x.kind {
                        noshape_model::drawing::ViewKind::Section { label, .. } | noshape_model::drawing::ViewKind::AlignedSection { label, .. } => format!("Section {label} - {label}"),
                        _ => crate::drawing_props::default_view_name(&self.drawing, self.current_sheet(), x),
                    });
                    h_flex()
                        .id(("hd-section", i))
                        .h(px(20.))
                        .text_size(px(13.))
                        .child(div().flex_1().child(name.unwrap_or_default()))
                        .child(div().id(("hd-section-x", i)).px(px(4.)).text_color(gpui_kit::rgb(0x9a9a9a)).cursor_pointer().child("×").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.set_hatch(
                                |d| {
                                    if let Some(l) = &mut d.sections
                                        && i < l.len()
                                    {
                                        l.remove(i);
                                    }
                                },
                                cx,
                            )
                        })))
                        .into_any_element()
                })
                .collect(),
            None => rows,
        };
        let profiles = v_flex()
            .mx(px(9.))
            .my(px(3.))
            .px(px(6.))
            .py(px(4.))
            .min_h(px(46.))
            .rounded(px(2.))
            .border_1()
            .border_color(gpui_kit::rgb(0x5da7dd))
            .bg(gpui_kit::rgb(0xd9eef9))
            .child(div().text_size(px(10.)).text_color(gpui_kit::rgb(0x5c6670)).child(if section_mode { "Hatches" } else { "Closed profiles" }))
            .children(rows);
        // Tabs ANSI | ISO | General.
        let tabs = h_flex().mx(px(9.)).my(px(3.)).border_b_1().border_color(gpui_kit::rgb(0xe0e0e0)).children(GROUPS.iter().enumerate().map(|(g, (name, _))| {
            let on = g == standard;
            let tip = format!("{name} patterns");
            div()
                .id(("hd-tab", g))
                .flex_1()
                .h(px(26.))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(13.))
                .cursor_pointer()
                .when(on, |e| e.text_color(gpui_kit::rgb(0x1651b0)).font_weight(FontWeight::SEMIBOLD).border_b_2().border_color(gpui_kit::rgb(0x1651b0)))
                .child(*name)
                .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.set_hatch(
                        |d| {
                            if d.standard != g {
                                d.standard = g;
                                d.pattern = hatch_pat::default_pattern(g);
                            }
                        },
                        cx,
                    )
                }))
                .into_any_element()
        }));
        // Pattern: icon list.
        let pat = if none { "None" } else { hatch_pat::pattern(standard, pattern).map(|p| p.name).unwrap_or("") };
        let pattern_btn = {
            let ed = ed.clone();
            Button::new("hd-pattern")
                .ghost()
                .xsmall()
                .w(px(188.))
                .rounded(px(0.))
                .border_b_1()
                .border_color(gpui_kit::rgb(RULE))
                .dropdown_caret(true)
                .child(h_flex().gap(px(4.)).when(!none, |h| h.child(crate::ui::glyph(&hatch_pat::icon_name(standard, pattern), 16., ink))).child(div().text_size(px(11.5)).text_color(ink).child(pat)))
                .os_dropdown_menu("hd-pattern-dd", true, move |mut m, _, _| {
                    m = m.select_list(188.);
                    // Edit hatch section...: General starts with None (live (4)).
                    if section_mode && standard == 2 {
                        let e = ed.clone();
                        m = m.item(crate::os_menu::OsMenuItem::new("None").checked(none).on_click(move |_, _, cx| e.update(cx, |t, cx| t.set_hatch(|d| d.pattern = NO_PATTERN, cx))));
                    }
                    for i in hatch_pat::display_order(standard) {
                        let Some(p) = hatch_pat::pattern(standard, i) else { continue };
                        let e = ed.clone();
                        m = m.item(crate::os_menu::OsMenuItem::new(p.name).icon_name(&hatch_pat::icon_name(standard, i)).checked(i == pattern).on_click(move |_, _, cx| e.update(cx, |t, cx| t.set_hatch(|d| d.pattern = i, cx))));
                    }
                    m
                })
                .into_any_element()
        };
        // Scale / Angle: editable boxes with a list.
        let combo = |id: &'static str, input: &Entity<InputState>, items: Vec<String>| -> AnyElement {
            let current = input.read(cx).value().to_string();
            let (ed, input2) = (ed.clone(), input.clone());
            h_flex()
                .w(px(188.))
                .border_b_1()
                .border_color(gpui_kit::rgb(RULE))
                .child(div().flex_1().child(Input::new(input).text_size(px(13.33)).text_color(gpui_kit::rgb(0x555555)).appearance(false).xsmall()))
                .child(Button::new(id).ghost().xsmall().dropdown_caret(true).os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut m, _, _| {
                    for v in &items {
                        let (e, st, v) = (ed.clone(), input2.clone(), v.clone());
                        m = m.item(crate::os_menu::OsMenuItem::new(v.clone()).checked(current == v).on_click(move |_, window, cx| {
                            st.update(cx, |s, cx| s.set_value(v.clone(), window, cx));
                            e.update(cx, |t, cx| t.hatch_preview(cx));
                        }));
                    }
                    m
                }))
                .into_any_element()
        };
        let scales: Vec<String> = std::iter::once("Auto".to_string()).chain((1..=10).map(|k| format!("{k}.0"))).collect();
        let angles: Vec<String> = std::iter::once("Auto".to_string()).chain((0..=12).map(|k| format!("{}.0°", k * 15))).collect();
        // Color: the swatch (presets) and ☑ Drawing properties.
        let c = d.color.unwrap_or([0, 0, 0]);
        let swatch_color = gpui_kit::rgb(((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32);
        let swatch = {
            let ed = ed.clone();
            let current = d.color;
            div().id("hd-color").size(px(22.)).bg(if drawing_color { gpui_kit::rgb(0x000000) } else { swatch_color }).when(drawing_color, |e| e.cursor_not_allowed()).when(!drawing_color, |e| {
                e.cursor_pointer().on_click(move |ev, window, cx| {
                    let ed = ed.clone();
                    let entries: Vec<MenuEntry> = crate::drawing_drawprops::COLORS
                        .iter()
                        .map(|&(n, c)| MenuEntry::check(n, current == c, move |t, _, cx| t.set_hatch(|d| d.color = c, cx)))
                        .collect();
                    open_tool_menu(&entries, &ed, ev, window, cx);
                })
            })
        };
        let color = h_flex()
            .h(px(34.))
            .pl(px(5.))
            .gap(px(8.))
            .child(div().w(px(48.)).text_size(px(11.5)).child("Color"))
            .child(swatch)
            .child(crate::drawing_chrome::DrCheckbox::new("hd-dcolor").label("Drawing properties").checked(drawing_color).on_click(cx.listener(|this, v: &bool, _, cx| {
                let v = *v;
                this.set_hatch(|d| d.drawing_color = v, cx)
            })));
        // The preview box (live: 150 × 100 canvas, the pattern in a framed box).
        let preview = {
            let (scale, angle) = (d.scale_value(cx), d.angle_value(cx));
            let col = if drawing_color { Hsla::black() } else { swatch_color.into() };
            canvas(
                |_, _, _| (),
                move |b, _, window, _| {
                    let (w, h) = (126.0f64, 62.0f64);
                    let o = b.origin + point(px(12.), px(19.));
                    // Pattern units: 3.2 px per mm.
                    let k = 3.2;
                    let rect = [[0.0, 0.0], [w / k, 0.0], [w / k, h / k], [0.0, h / k]];
                    let to = |q: [f64; 2]| o + point(px((q[0] * k) as f32), px((h - q[1] * k) as f32));
                    if let Some(p) = hatch_pat::pattern(standard, pattern) {
                        if p.families.is_empty() {
                            window.paint_quad(fill(Bounds::new(o, size(px(w as f32), px(h as f32))), col));
                        }
                        for l in hatch_pat::lines(p, &rect, &[], scale, angle) {
                            let mut pb = PathBuilder::stroke(px(1.));
                            pb.move_to(to(l[0]));
                            pb.line_to(to(l[1]));
                            if let Ok(path) = pb.build() {
                                window.paint_path(path, col);
                            }
                        }
                    }
                    window.paint_quad(outline(Bounds::new(o, size(px(w as f32), px(h as f32))), Hsla::black(), BorderStyle::default()));
                },
            )
            .w(px(150.))
            .h(px(100.))
            .ml(px(45.))
        };
        let ok_disabled = if section_mode { !picked } else { d.profiles.is_empty() };
        let show_style = !section_mode || picked;
        Some(
            v_flex()
                .id("dr-hatch-dlg")
                .absolute()
                .top(px(DIALOG_TOP))
                .left(px(self.dialog_left()))
                .w(px(268.))
                .pb(px(9.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(ink)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(30.))
                        .pl(px(11.))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(if section_mode { "Edit hatch" } else { "Hatch region" }))
                        .child(
                            div()
                                .id("hd-ok")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .bg(gpui_kit::rgb(0x009500))
                                .when(ok_disabled, |e| e.bg(gpui_kit::rgb(crate::drawing_chrome::OK_DISABLED)))
                                .cursor_pointer()
                                .child(crate::ui::glyph("check", 18., gpui_kit::white()))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.accept_hatch(cx))),
                        )
                        .child(
                            div()
                                .id("hd-cancel")
                                .w(px(28.))
                                .h(px(27.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .child(crate::drawing_chrome::cancel_x(cx))
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.cancel_hatch(cx);
                                    cx.notify();
                                })),
                        ),
                )
                .when(!section_mode, |e| e.child(div().pt(px(6.)).child(tools)))
                .when(section_mode, |e| e.pt(px(6.)))
                .child(profiles)
                .when(show_style, |e| {
                    e.child(tabs)
                        .child(div().px(px(9.)).child(row("Pattern", pattern_btn)))
                        .when(!general && !none, |e| e.child(div().px(px(9.)).child(row("Scale", combo("hd-scale", &d.scale, scales)))).child(div().px(px(9.)).child(row("Angle", combo("hd-angle", &d.angle, angles)))))
                        .when(!none, |e| e.child(div().px(px(9.)).border_b_1().border_color(gpui_kit::rgb(0xe0e0e0)).child(color)))
                        .child(div().pt(px(4.)).child(preview))
                })
                .into_any_element(),
        )
    }
}
