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
/// Keys, action, locked.
type ShortcutRow = (&'static str, &'static str, bool);

/// Onshape's keyboard shortcut sheet (live capture, dom/keyboard-shortcuts.txt):
/// tab, then (keys, action, locked = not customizable in Onshape).
pub(super) const SHORTCUT_TABS: &[(&str, &[ShortcutRow])] = &[
    ("General", &[
        ("shift enter", "Accept & repeat command", true),
        ("enter", "Accept command", true),
        ("escape", "Cancel", true),
        ("space", "Clear selection", true),
        ("ctrl/cmd c", "Copy", true),
        ("shift c", "Curve/surface analysis", false),
        ("delete / backspace", "Delete selection", true),
        ("shift d", "Dihedral analysis", false),
        ("ctrl/cmd shift f", "FeatureScript search", true),
        ("ctrl u", "Feedback/Report a bug", true),
        ("a", "Flip primary axis", false),
        ("k", "Hide/show mate connectors", false),
        ("shift /", "Keyboard shortcuts", false),
        ("shift", "Lock mate inference", true),
        ("ctrl m", "Mate connector", false),
        ("[", "Measure", false),
        ("ctrl/cmd click", "Open in new tab", true),
        ("shift click", "Open in new window", true),
        ("ctrl/cmd v", "Paste", true),
        ("ctrl space", "Quick tab switching", true),
        ("ctrl y / shift cmd z", "Redo", true),
        ("shift n", "Rename selection", false),
        ("q", "Reorient secondary axis", false),
        ("alt/⌥ c", "Search tools", false),
        ("`", "Select other", false),
        ("s", "Shortcut toolbars", true),
        ("alt/⌥ t", "Tab manager", false),
        ("ctrl/cmd z", "Undo", true),
    ]),
    ("Part Studio", &[
        ("shift e", "Extrude", false),
        ("shift f", "Fillet", false),
        ("shift h", "Hide/show sketches", false),
        ("select rollback bar ↑ ↓", "Move rollback bar", true),
        ("shift w", "Revolve", false),
        ("shift s", "Sketch", false),
    ]),
    ("Assembly", &[
        ("j", "Hide/show mates", false),
        ("i", "Insert dialog", false),
        ("m", "Mate", false),
        ("h", "Show mates mode", false),
        ("shift s", "Snap mode", false),
    ]),
    ("3D view", &[
        ("shift 2", "Back view", false),
        ("shift 6", "Bottom view", false),
        ("shift ← → ↑ ↓", "Coarse rotate", true),
        ("shift 1", "Front view", false),
        ("shift p", "Hide all construction", false),
        ("y", "Hide selected parts", false),
        ("p", "Hide/show planes", false),
        ("shift 7", "Isometric view", false),
        ("shift 3", "Left view", false),
        ("shift i", "Make isolate", false),
        ("shift t", "Make transparent", false),
        ("shift v", "Named view", false),
        ("ctrl shift ← → ↑ ↓", "Pan", true),
        ("ctrl ← → ↑ ↓", "Precision rotate", true),
        ("shift 4", "Right view", false),
        ("← → ↑ ↓", "Rotate", true),
        ("shift x", "Section view", false),
        ("alt/⌥ click", "Select through transparent", true),
        ("shift y", "Show selected parts", false),
        ("shift 5", "Top view", false),
        ("shift r", "View in high quality", false),
        ("n", "View normal to", false),
        ("shift z", "Zoom in", false),
        ("z", "Zoom out", false),
        ("f", "Zoom to fit", false),
        ("w", "Zoom to window", false),
    ]),
    ("Sketch", &[
        ("a", "3 point arc", false),
        ("c", "Center point circle*", false),
        ("r", "Center point rectangle*", false),
        ("i", "Coincident*", false),
        ("shift o", "Concentric", false),
        ("g", "Corner rectangle*", false),
        ("shift u", "Curvature", false),
        ("d", "Dimension*", false),
        ("e", "Equal", false),
        ("x", "Extend", false),
        ("shift j", "Fix", false),
        ("h", "Horizontal*", false),
        ("l", "Line*", false),
        ("shift a", "Line/tangent arc switch", false),
        ("shift m", "Midpoint", false),
        ("shift k", "Normal", false),
        ("o", "Offset", false),
        ("b", "Parallel*", false),
        ("shift l", "Perpendicular*", false),
        ("shift g", "Pierce", false),
        ("shift s", "Point", false),
        ("shift f", "Sketch fillet", false),
        ("shift", "Suppress inferencing", true),
        ("shift q", "Symmetric", false),
        ("t", "Tangent*", false),
        ("q", "Toggle construction", false),
        ("m", "Trim*", false),
        ("u", "Use", false),
        ("v", "Vertical*", false),
    ]),
    ("Feature Studio", &[
        ("ctrl/cmd [", "Back reference", true),
        ("ctrl/cmd s", "Commit", true),
        ("ctrl/cmd shift s", "Commit all", true),
        ("escape", "Dismiss autocomplete", true),
        ("ctrl/cmd ]", "Forward reference", true),
        ("ctrl/cmd shift o", "Top level symbols dropdown", true),
    ]),
    ("Drawing", &[
        ("alt/⌥ lmb drag", "Copy annotation", true),
        ("shift d", "Diameter dimension", false),
        ("ctrl s", "Display sheet menu", false),
        ("home", "First sheet", false),
        ("shift h", "Foreshorten dimension", false),
        ("end", "Last sheet", false),
        ("ctrl m", "Max/min dimension", false),
        ("page down", "Next sheet", false),
        ("n", "Note", false),
        ("page up", "Previous sheet", false),
        ("p", "Projected view", false),
        ("shift q", "Quad & mid snap point", false),
        ("shift r", "Radial dimension", false),
        ("ctrl q", "Update drawing", false),
    ]),
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
        // Onshape (live): a white modal over a dimmed page: title +
        // "Customize keyboard shortcuts", a search box and ×; tabs General,
        // Part Studio, Assembly, 3D view, Sketch, Feature Studio, Drawing;
        // rows of right-aligned key chips and the action (🔒 = Onshape
        // controlled); a legend at the bottom. Searching lists matches of
        // every tab.
        let pal = p();
        let query = self.ui.shortcut_query.clone();
        let search = self.ui.shortcut_filter.as_ref().map(|(st, _)| st.clone());
        let tab = self.ui.shortcut_tab.min(SHORTCUT_TABS.len() - 1);
        let chip = |k: &str| div().h(px(24.)).px(px(5.)).flex().items_center().border_1().border_color(gpui_kit::rgb(0x999999)).rounded(px(2.)).text_size(px(13.)).text_color(pal.text).child(k.to_string());
        let keys_el = |keys: &str| {
            let mut row = h_flex().gap(px(4.)).justify_end();
            let rest = match keys.strip_prefix("select rollback bar ") {
                Some(r) => {
                    row = row.child(div().text_size(px(13.)).text_color(pal.text).child("select rollback bar"));
                    r
                }
                None => keys,
            };
            let tokens: Vec<&str> = rest.split(' ').filter(|k| !k.is_empty()).collect();
            for (i, k) in tokens.iter().enumerate() {
                // "/" between two key groups separates alternatives; at the end it is the key.
                let separator = *k == "/" && i > 0 && i + 1 < tokens.len();
                row = if separator { row.child(div().px(px(2.)).text_size(px(16.)).text_color(pal.text).child("/")) } else { row.child(chip(k)) };
            }
            row
        };
        let rows: Vec<ShortcutRow> = if query.trim().is_empty() {
            let mut r: Vec<_> = SHORTCUT_TABS[tab].1.to_vec();
            r.sort_by_key(|(_, a, _)| a.to_lowercase());
            r
        } else {
            SHORTCUT_TABS.iter().flat_map(|(_, r)| r.iter().copied()).filter(|(k, a, _)| shortcut_matches(k, a, &query)).collect()
        };
        let body = rows.into_iter().map(|(k, a, locked)| {
            h_flex()
                .w(px(560.))
                .h(px(39.))
                .flex_none()
                .gap(px(14.))
                .child(div().w(px(273.)).flex_none().child(keys_el(k)))
                .child(h_flex().flex_1().gap_1().text_size(px(14.)).text_color(pal.text).child(a.to_string()).when(locked, |d| d.child(crate::assets::icon("lock", 14., cx))))
        });
        let tabs = SHORTCUT_TABS.iter().enumerate().map(|(i, (name, _))| {
            let on = i == tab && query.trim().is_empty();
            div()
                .id(SharedString::from(format!("sc-tab-{i}")))
                .px(px(6.))
                .pb(px(4.))
                .cursor_pointer()
                .text_size(px(14.))
                .text_color(if on { pal.accent } else { pal.text })
                .when(on, |d| d.border_b_2().border_color(pal.accent))
                .child(*name)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.ui.shortcut_tab = i;
                    cx.notify();
                }))
        });
        let legend_chip = |t: &str, color: Rgba, strike: bool| div().px(px(4.)).border_1().border_color(color).text_size(px(13.)).text_color(pal.text).when(strike, |d| d.line_through().text_color(pal.text_faint)).child(t.to_string());
        let legend = h_flex()
            .gap(px(8.))
            .px(px(8.))
            .py(px(6.))
            .border_1()
            .border_color(pal.border_strong)
            .text_size(px(13.))
            .text_color(pal.text)
            .child(legend_chip("example", pal.accent, false))
            .child("Customized")
            .child(legend_chip("example", gpui_kit::rgb(0x999999), false))
            .child("Onshape default")
            .child(legend_chip("example", gpui_kit::rgb(0x999999), true))
            .child("Disabled")
            .child(crate::assets::icon("lock", 14., cx))
            .child("Onshape controlled")
            .child("* applies in drawing sketches");
        Some(
            div()
                .id("shortcut-sheet")
                .occlude()
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
                        .w(px(1000.))
                        .max_w(relative(0.95))
                        .h(relative(0.92))
                        .max_h(px(928.))
                        .bg(gpui_kit::rgb(0xffffff))
                        .shadow_lg()
                        // Clicks inside the sheet (search box, tabs) keep it open.
                        .on_click(|_, _, cx| cx.stop_propagation())
                        .child(
                            h_flex()
                                .flex_none()
                                .h(px(54.))
                                .px(px(10.))
                                .gap(px(6.))
                                .border_b_1()
                                .border_color(pal.border)
                                .child(div().text_size(px(19.)).text_color(pal.text).child("Keyboard shortcuts"))
                                .child(
                                    div()
                                        .id("sc-customize")
                                        .text_size(px(12.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(pal.accent)
                                        .cursor_pointer()
                                        .child("Customize keyboard shortcuts")
                                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                            this.toggle_shortcut_sheet(window, cx);
                                            this.open_sketch_shortcuts(window, cx);
                                        })),
                                )
                                .child(div().flex_1())
                                .children(search.map(|st| div().w(px(198.)).child(Input::new(&st).small())))
                                .child(
                                    div()
                                        .id("sc-close")
                                        .px_1()
                                        .cursor_pointer()
                                        .child(Icon::new(IconName::Close).small().text_color(pal.text_muted))
                                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_shortcut_sheet(window, cx))),
                                ),
                        )
                        .child(h_flex().flex_none().justify_center().gap(px(20.)).pt(px(16.)).pb(px(6.)).children(tabs))
                        .child(div().id("sc-rows").flex_1().min_h_0().overflow_y_scroll().py(px(8.)).child(v_flex().items_center().children(body)))
                        .child(div().flex_none().p(px(10.)).border_t_1().border_color(pal.border).flex().justify_center().child(legend)),
                )
                .into_any_element(),
        )
    }

    /// Shift+/ toggles the shortcut map; it opens with a search box.
    pub(super) fn toggle_shortcut_sheet(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_shortcuts = !self.show_shortcuts;
        self.ui.shortcut_query.clear();
        if self.show_shortcuts {
            self.ui.shortcut_tab = 0;
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
        let all: Vec<&str> = SHORTCUT_TABS.iter().flat_map(|(_, r)| r.iter().map(|(k, _, _)| *k)).collect();
        for k in ["y", "shift y", "shift i", "shift t", "w", "s", "`", "[", "shift p"] {
            assert!(all.contains(&k), "{k} missing from the shortcut sheet");
        }
    }
}
