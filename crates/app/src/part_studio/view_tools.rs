//! Part Studio view tools: tool search palette (Alt+C), keyboard shortcut
//! sheet (Shift+?), display modes, section view (Shift+X) and feature rename.

use super::PartStudioView;
use super::feature_icon;
use crate::scene_builder::DisplayMode;
use crate::theme::{p, with_alpha};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use noshape_model::features;
use noshape_model::reference::FeatureId;

/// Shortcut sheet content (Onshape-compatible defaults).
const SHORTCUTS: &[(&str, &[(&str, &str)])] = &[
    ("General", &[("Ctrl/⌘+Z", "Undo"), ("Ctrl+Y / ⇧⌘Z", "Redo"), ("Enter", "Accept dialog / edit selected feature"), ("Shift+Enter", "Accept and repeat"), ("Esc", "Cancel / exit tool"), ("Space", "Clear selection"), ("Delete", "Delete selection"), ("Ctrl/⌘+C / V", "Copy / paste features"), ("Shift+N", "Rename feature"), ("Alt+C", "Search tools"), ("S", "Shortcut toolbar at the cursor"), ("` / Shift+`", "Select other (next / previous)"), ("[", "Measure"), ("Ctrl+M", "Mate connector"), ("K", "Show/hide mate connectors"), ("Alt+H", "Versions & history"), ("Shift+?", "This sheet")]),
    ("Part Studio", &[("Shift+S", "New sketch"), ("Shift+E", "Extrude"), ("Shift+W", "Revolve"), ("Shift+F", "Fillet"), ("Shift+H", "Show/hide sketches"), ("↑ / ↓", "Move rollback bar (bar selected)"), ("Double-click", "Edit feature / sketch")]),
    ("3D view", &[("Shift+1…7", "Front, Back, Left, Right, Top, Bottom, Isometric"), ("N", "View normal to (again: reverse)"), ("F", "Zoom to fit"), ("W", "Zoom to window"), ("Z / Shift+Z", "Zoom out / in"), ("Arrows", "Rotate 15° (Ctrl: 5°, Shift: 90°)"), ("Ctrl+Shift+Arrows", "Pan"), ("P", "Show/hide planes"), ("Shift+P", "Hide all construction geometry"), ("Y / Shift+Y", "Hide / show"), ("Shift+I", "Isolate"), ("Shift+T", "Make transparent"), ("O", "Orthographic / perspective"), ("Shift+X", "Section view"), ("Shift+V", "Named views"), ("Right drag", "Rotate"), ("Middle / Ctrl+right drag", "Pan"), ("Scroll", "Zoom at cursor"), ("Double middle click", "Zoom to fit")]),
    ("Sketch", &[("L", "Line"), ("G / R", "Corner / center rectangle"), ("C", "Circle"), ("A", "3-point arc"), ("Shift+S", "Point"), ("D", "Dimension"), ("Q", "Construction"), ("M", "Trim"), ("X", "Extend"), ("O", "Offset"), ("U", "Use (project)"), ("Shift+F", "Fillet"), ("I H V B T E", "Coincident, Horizontal, Vertical, Parallel, Tangent, Equal"), ("Shift+L / Shift+M / Shift+Q", "Perpendicular / Midpoint / Symmetric"), ("Shift", "Hold to suppress inference")]),
];

/// Shortcut rows matching a search (key or description, case-insensitive).
pub(super) fn shortcut_matches(key: &str, what: &str, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty() || key.to_lowercase().contains(&q) || what.to_lowercase().contains(&q)
}

#[derive(Clone)]
pub(super) enum PaletteItem {
    Feature(&'static str),
    View(noshape_render::StandardView, &'static str),
    Display(DisplayMode),
    /// A sketch tool (only while a sketch is open).
    SketchTool(crate::sketch_mode::Tool),
}

impl PartStudioView {
    pub(crate) fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let st = cx.new(|cx| InputState::new(window, cx).placeholder("Search tools…"));
        let sub = cx.subscribe_in(&st, window, |this: &mut Self, st, ev: &InputEvent, window, cx| match ev {
            InputEvent::Change => {
                this.palette_query = st.read(cx).value().to_string();
                cx.notify();
            }
            InputEvent::PressEnter { .. } => {
                if let Some(item) = this.palette_items(cx).into_iter().next() {
                    this.run_palette(item, window, cx);
                }
            }
            InputEvent::Blur => {
                this.palette = None;
                cx.notify();
            }
            _ => {}
        });
        st.update(cx, |s, cx| s.focus(window, cx));
        self.palette_query.clear();
        self.palette = Some((st, sub));
        cx.notify();
    }

    fn palette_items(&self, _cx: &App) -> Vec<PaletteItem> {
        let q = self.palette_query.to_lowercase();
        let mut out: Vec<PaletteItem> = Vec::new();
        // In a sketch, its tools come first (Onshape searches sketch tools there).
        if self.sketch.is_some() && !q.is_empty() {
            for t in super::sketch::all_sketch_tools() {
                if sketch_tool_name(t).to_lowercase().contains(&q) {
                    out.push(PaletteItem::SketchTool(t));
                }
            }
        }
        // Custom features of this document first (they are few and specific).
        for t in self.doc.read(_cx).custom_features().feature_types() {
            if q.is_empty() || t.name.to_lowercase().contains(&q) {
                out.push(PaletteItem::Feature(t.id));
            }
        }
        // With no query, recently used tools come first (Onshape).
        if q.is_empty() {
            out.extend(self.ui.recent_tools.iter().map(|id| PaletteItem::Feature(id)));
        }
        for t in features::all() {
            if q.is_empty() && self.ui.recent_tools.contains(&t.id) {
                continue;
            }
            if q.is_empty() || t.name.to_lowercase().contains(&q) || t.id.to_lowercase().contains(&q) {
                out.push(PaletteItem::Feature(t.id));
            }
        }
        use noshape_render::StandardView as V;
        for (v, n) in [(V::Isometric, "Isometric view"), (V::Front, "Front view"), (V::Top, "Top view"), (V::Right, "Right view"), (V::Back, "Back view"), (V::Left, "Left view"), (V::Bottom, "Bottom view")] {
            if !q.is_empty() && n.to_lowercase().contains(&q) {
                out.push(PaletteItem::View(v, n));
            }
        }
        for m in DisplayMode::ALL {
            if !q.is_empty() && m.label().to_lowercase().contains(&q) {
                out.push(PaletteItem::Display(m));
            }
        }
        out.truncate(14);
        out
    }

    fn run_palette(&mut self, item: PaletteItem, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = None;
        match item {
            PaletteItem::Feature(id) => self.start_feature(id, window, cx),
            PaletteItem::View(v, _) => self.view(v, cx),
            PaletteItem::Display(m) => self.set_display_mode(m, window, cx),
            PaletteItem::SketchTool(t) => self.activate_tool(t, window, cx),
        }
        cx.notify();
    }

    pub(super) fn render_palette(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (st, _) = self.palette.as_ref()?;
        let st = st.clone();
        let pal = p();
        let items = self.palette_items(cx);
        let rows = items.into_iter().map(|item| {
            let (icon, label, hint): (SharedString, String, String) = match &item {
                PaletteItem::Feature(id) => {
                    let t = features::get(id);
                    (feature_icon(id), t.map(|t| t.name.to_string()).unwrap_or_default(), t.and_then(|t| t.shortcut).map(|s| s.replace("shift-", "Shift+").to_uppercase()).unwrap_or_default())
                }
                PaletteItem::View(_, n) => (crate::assets::icon_path("iso-view"), n.to_string(), String::new()),
                PaletteItem::Display(m) => (crate::assets::icon_path(m.icon()), m.label().to_string(), String::new()),
                PaletteItem::SketchTool(t) => {
                    let hint = t.label().rsplit_once(" (").map(|(_, k)| k.trim_end_matches(')').to_string()).unwrap_or_default();
                    (crate::assets::icon_path(t.icon()), sketch_tool_name(*t).to_string(), hint)
                }
            };
            let it = item.clone();
            let desc = match &item {
                PaletteItem::Feature(id) => features::get(id).map(|t| t.description.to_string()).unwrap_or_default(),
                _ => String::new(),
            };
            let hover_info = (label.clone(), desc);
            h_flex()
                .id(SharedString::from(format!("pal-{label}")))
                // Onshape shows the hovered tool's description beside the list.
                .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                    if *h && this.ui.palette_hover.as_ref() != Some(&hover_info) {
                        this.ui.palette_hover = Some(hover_info.clone());
                        cx.notify();
                    }
                }))
                .gap_2()
                .px_2()
                .py_1()
                .rounded_sm()
                .cursor_pointer()
                .hover(|d| d.bg(pal.panel_hover))
                .child(svg().path(icon).size_4().text_color(pal.text_muted))
                .child(div().flex_1().text_sm().text_color(pal.text).child(label))
                .child(div().text_xs().text_color(pal.text_faint).child(hint))
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, window, cx| this.run_palette(it.clone(), window, cx)))
        });
        Some(
            div()
                .absolute()
                .top(px(60.))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                    if ev.keystroke.key == "escape" {
                        this.palette = None;
                        window.focus(&this.focus, cx);
                        cx.notify();
                    }
                }))
                .child(
                    v_flex()
                        .w(px(620.))
                        .p_2()
                        .gap_1()
                        .rounded_lg()
                        .bg(pal.panel_raised)
                        .border_1()
                        .border_color(pal.border_strong)
                        .shadow_lg()
                        .child(Input::new(&st).prefix(Icon::new(IconName::Search).small()))
                        .child(
                            h_flex()
                                .items_start()
                                .gap_2()
                                .child(v_flex().flex_1().gap_1().children(rows))
                                .children(self.ui.palette_hover.clone().filter(|(_, d)| !d.is_empty()).map(|(name, d)| {
                                    v_flex()
                                        .w(px(180.))
                                        .p_2()
                                        .gap_1()
                                        .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(name))
                                        .child(div().text_xs().text_color(pal.text_muted).child(d))
                                })),
                        ),
                )
                .into_any_element(),
        )
    }

    pub(super) fn render_shortcut_sheet(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.show_shortcuts {
            return None;
        }
        let pal = p();
        let query = self.ui.shortcut_query.clone();
        let search = self.ui.shortcut_filter.as_ref().map(|(st, _)| st.clone());
        let cols = SHORTCUTS.iter().filter(|(_, rows)| rows.iter().any(|(k, v)| shortcut_matches(k, v, &query))).map(|(title, rows)| {
            let query = query.clone();
            v_flex()
                .w(px(262.))
                .gap_1()
                .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.accent_hover).mb_1().child(title.to_uppercase()))
                .children(rows.iter().filter(move |(k, v)| shortcut_matches(k, v, &query)).map(|(k, v)| {
                    h_flex()
                        .gap_2()
                        .child(div().w(px(112.)).flex_none().child(div().px_1p5().rounded_sm().bg(pal.app_bg).border_1().border_color(pal.border).text_xs().text_color(pal.text).child(*k)))
                        .child(div().text_xs().text_color(pal.text_muted).child(*v))
                }))
        });
        Some(
            div()
                .id("shortcut-sheet")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(with_alpha(pal.overlay_shadow, 0.5))
                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                    if ev.keystroke.key == "escape" {
                        this.toggle_shortcut_sheet(window, cx);
                    }
                }))
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    if this.show_shortcuts {
                        this.toggle_shortcut_sheet(window, cx);
                    }
                }))
                .child(
                    v_flex()
                        .id("shortcut-panel")
                        .p_4()
                        .gap_3()
                        .rounded_lg()
                        .bg(pal.panel_raised)
                        .border_1()
                        .border_color(pal.border_strong)
                        // Clicks inside the sheet (search box) keep it open.
                        .on_click(|_, _, cx| cx.stop_propagation())
                        .child(
                            h_flex()
                                .gap_4()
                                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Keyboard shortcuts"))
                                .children(search.map(|st| div().w(px(220.)).child(Input::new(&st).small().prefix(Icon::new(IconName::Search).small())))),
                        )
                        .child(h_flex().items_start().gap_6().children(cols)),
                )
                .into_any_element(),
        )
    }

    /// Shift+/ toggles the shortcut map; it opens with a search box.
    pub(super) fn toggle_shortcut_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_shortcuts = !self.show_shortcuts;
        self.ui.shortcut_query.clear();
        if self.show_shortcuts {
            let st = cx.new(|cx| InputState::new(window, cx).placeholder("Search shortcuts"));
            let sub = cx.subscribe_in(&st, window, |this: &mut Self, st, ev: &InputEvent, _, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.ui.shortcut_query = st.read(cx).value().to_string();
                    cx.notify();
                }
            });
            // Typing goes to the search box right away (and bare-letter
            // shortcuts stay quiet while the map is open).
            st.update(cx, |s, cx| s.focus(window, cx));
            self.ui.shortcut_filter = Some((st, sub));
        } else {
            self.ui.shortcut_filter = None;
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }

    pub(super) fn set_display_mode(&mut self, m: DisplayMode, window: &mut Window, cx: &mut Context<Self>) {
        self.opts.display_mode = m;
        let d = self.display.clone();
        self.apply_display(d, window, cx);
    }

    // ------------------------------------------------------------ rename

    pub(super) fn start_rename(&mut self, id: FeatureId, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.studio(cx).and_then(|s| s.feature(id)).map(|f| f.name.clone()).unwrap_or_default();
        let st = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                let v = st.read(cx).value().to_string();
                this.renaming = None;
                if !v.trim().is_empty() {
                    this.doc.update(cx, |doc, cx| {
                        doc.edit_studio(this.tab, &format!("Rename : {v}"), cx, |ps| {
                            if let Some(f) = ps.feature_mut(id) {
                                f.name = v.trim().to_string();
                                f.custom_name = true;
                            }
                        });
                    });
                }
                cx.notify();
            }
        });
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        self.renaming = Some((id, st, sub));
        cx.notify();
    }
}

// ------------------------------------------------------------ named views

impl PartStudioView {
    pub(super) fn toggle_named_views(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.named_views.take().is_some() {
            window.focus(&self.focus, cx);
        } else {
            let st = cx.new(|cx| InputState::new(window, cx).placeholder("New view name"));
            let sub = cx.subscribe_in(&st, window, |this: &mut Self, st, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::PressEnter { .. }) {
                    let name = st.read(cx).value().trim().to_string();
                    this.save_named_view(name, cx);
                    st.update(cx, |s, cx| s.set_value("", window, cx));
                    window.focus(&this.focus, cx);
                }
            });
            st.update(cx, |s, cx| s.focus(window, cx));
            self.named_views = Some((st, sub));
        }
        cx.notify();
    }

    fn save_named_view(&mut self, name: String, cx: &mut Context<Self>) {
        let cam = self.viewport.read(cx).camera;
        let existing: Vec<String> = self.studio(cx).map(|s| s.named_views.iter().map(|v| v.name.clone()).collect()).unwrap_or_default();
        let name = if name.is_empty() {
            (1..).map(|n| format!("View {n}")).find(|n| !existing.contains(n)).unwrap_or_default()
        } else {
            name
        };
        let view = noshape_model::partstudio::NamedView {
            name: name.clone(),
            target: cam.target.to_array(),
            rotation: cam.rotation.to_array(),
            distance: cam.distance,
            perspective: cam.projection == noshape_render::Projection::Perspective,
            section: super::section::encode(&self.ui.section),
        };
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, &format!("Save view : {name}"), cx, |ps| {
                ps.named_views.retain(|v| v.name != view.name);
                ps.named_views.push(view);
            });
        });
    }

    fn apply_named_view(&mut self, v: &noshape_model::partstudio::NamedView, window: &mut Window, cx: &mut Context<Self>) {
        // Named views restore the section state too (Onshape).
        if !v.section.is_empty() || !self.ui.section.is_empty() {
            self.restore_section(&v.section, window, cx);
        }
        self.push_camera_history(cx);
        self.viewport.update(cx, |vp, cx| {
            let mut to = vp.camera;
            to.target = glam::DVec3::from_array(v.target);
            to.rotation = glam::DQuat::from_array(v.rotation).normalize();
            to.distance = v.distance;
            to.projection = if v.perspective { noshape_render::Projection::Perspective } else { noshape_render::Projection::Orthographic };
            vp.animate_to(to, cx);
        });
    }

    pub(super) fn render_named_views(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (st, _) = self.named_views.as_ref()?;
        let st = st.clone();
        let pal = p();
        let views = self.studio(cx).map(|s| s.named_views.clone()).unwrap_or_default();
        let rows: Vec<AnyElement> = views
            .into_iter()
            .enumerate()
            .map(|(i, v)| {
                let name = v.name.clone();
                h_flex()
                    .id(SharedString::from(format!("nv-{i}")))
                    .h(px(24.))
                    .px_2()
                    .gap_2()
                    .rounded_sm()
                    .text_sm()
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(div().flex_1().truncate().child(v.name.clone()))
                    .child(Button::new(SharedString::from(format!("nv-del-{i}"))).icon(IconName::Close).ghost().xsmall().tooltip("Delete view").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        let name = name.clone();
                        this.doc.update(cx, |doc, cx| {
                            doc.edit_studio(this.tab, &format!("Delete view : {name}"), cx, |ps| ps.named_views.retain(|v| v.name != name));
                        });
                    })))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.apply_named_view(&v, window, cx)))
                    .into_any_element()
            })
            .collect();
        let empty = rows.is_empty();
        Some(
            v_flex()
                .id("named-views")
                .occlude()
                .absolute()
                .top(px(12.))
                .right(px(170.))
                .w(px(220.))
                .p_2()
                .gap_1()
                .rounded_md()
                .bg(with_alpha(pal.panel_raised, 0.97))
                .border_1()
                .border_color(pal.border_strong)
                .shadow_lg()
                .child(
                    h_flex()
                        .justify_between()
                        .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text_muted).child("NAMED VIEWS"))
                        .child(Button::new("nv-close").icon(IconName::Close).ghost().xsmall().on_click(cx.listener(|this, _: &ClickEvent, w, cx| this.toggle_named_views(w, cx)))),
                )
                .when(empty, |d| d.child(div().px_2().py_1().text_xs().text_color(pal.text_faint).child("No saved views yet.")))
                .children(rows)
                .child(
                    h_flex()
                        .gap_1()
                        .pt_1()
                        .child(div().flex_1().child(Input::new(&st).xsmall()))
                        .child(Button::new("nv-save").label("Save").xsmall().on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            let name = st.read(cx).value().trim().to_string();
                            this.save_named_view(name, cx);
                            st.update(cx, |s, cx| s.set_value("", window, cx));
                        }))),
                )
                .into_any_element(),
        )
    }
}


/// A sketch tool's name without its shortcut ("Line (L)" -> "Line").
fn sketch_tool_name(t: crate::sketch_mode::Tool) -> &'static str {
    let l = t.label();
    l.rsplit_once(" (").map_or(l, |(name, _)| name)
}

#[cfg(test)]
mod tests {
    use super::*;
    // The glob import brings GPUI's `test` attribute; use the built-in one.
    use std::prelude::v1::test;

    #[test]
    fn shortcut_search_matches_keys_and_descriptions() {
        assert!(shortcut_matches("Shift+I", "Isolate", "isol"));
        assert!(shortcut_matches("Shift+I", "Isolate", "shift+i"));
        assert!(shortcut_matches("W", "Zoom to window", ""));
        assert!(!shortcut_matches("W", "Zoom to window", "fillet"));
        // Every shortcut the Part Studio binds is listed.
        let all: Vec<&str> = SHORTCUTS.iter().flat_map(|(_, r)| r.iter().map(|(k, _)| *k)).collect();
        for k in ["Y / Shift+Y", "Shift+I", "Shift+T", "W", "S", "` / Shift+`", "[", "Shift+P"] {
            assert!(all.contains(&k), "{k} missing from the shortcut sheet");
        }
    }
}
