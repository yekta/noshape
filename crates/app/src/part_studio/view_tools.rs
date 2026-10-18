//! Part Studio view tools: tool search palette (Alt+C), keyboard shortcut
//! sheet (Shift+?), display modes, section view (Shift+X) and feature rename.

use super::PartStudioView;
use super::feature_icon;
use crate::scene_builder::DisplayMode;
use crate::theme::{p, with_alpha};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use noshape_model::features;
use noshape_model::reference::{FeatureId, Reference};

/// Shortcut sheet content (Onshape-compatible defaults).
const SHORTCUTS: &[(&str, &[(&str, &str)])] = &[
    ("General", &[("Ctrl/⌘+Z", "Undo"), ("Ctrl+Y / ⇧⌘Z", "Redo"), ("Enter", "Accept dialog"), ("Shift+Enter", "Accept and repeat"), ("Esc", "Cancel / exit tool"), ("Space", "Clear selection"), ("Delete", "Delete selection"), ("Shift+N", "Rename feature"), ("Alt+C", "Search tools"), ("Alt+H", "Versions & history"), ("Shift+?", "This sheet")]),
    ("Part Studio", &[("Shift+S", "New sketch"), ("Shift+E", "Extrude"), ("Shift+W", "Revolve"), ("Shift+F", "Fillet"), ("Shift+H", "Show/hide sketches"), ("↑ / ↓", "Move rollback bar (feature list)")]),
    ("3D view", &[("Shift+1…7", "Front, Back, Left, Right, Top, Bottom, Isometric"), ("N", "View normal to"), ("F", "Zoom to fit"), ("Z / Shift+Z", "Zoom out / in"), ("Arrows", "Rotate 15° (Ctrl: 5°, Shift: 90°)"), ("Ctrl+Shift+Arrows", "Pan"), ("P", "Show/hide planes"), ("O", "Orthographic / perspective"), ("Shift+X", "Section view"), ("Right drag", "Rotate"), ("Middle / Ctrl+right drag", "Pan"), ("Scroll", "Zoom at cursor")]),
    ("Sketch", &[("L", "Line"), ("G / R", "Corner / center rectangle"), ("C", "Circle"), ("A", "3-point arc"), ("Shift+S", "Point"), ("D", "Dimension"), ("Q", "Construction"), ("M", "Trim"), ("X", "Extend"), ("O", "Offset"), ("U", "Use (project)"), ("Shift+F", "Fillet"), ("I H V B T E", "Coincident, Horizontal, Vertical, Parallel, Tangent, Equal"), ("Shift+L / Shift+M / Shift+Q", "Perpendicular / Midpoint / Symmetric"), ("Shift", "Hold to suppress inference")]),
];

#[derive(Clone)]
pub(super) enum PaletteItem {
    Feature(&'static str),
    View(noshape_render::StandardView, &'static str),
    Display(DisplayMode),
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
        // Custom features of this document first (they are few and specific).
        for t in self.doc.read(_cx).custom_features().feature_types() {
            if q.is_empty() || t.name.to_lowercase().contains(&q) {
                out.push(PaletteItem::Feature(t.id));
            }
        }
        for t in features::all() {
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
            };
            let it = item.clone();
            h_flex()
                .id(SharedString::from(format!("pal-{label}")))
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
                        .w(px(420.))
                        .p_2()
                        .gap_1()
                        .rounded_lg()
                        .bg(pal.panel_raised)
                        .border_1()
                        .border_color(pal.border_strong)
                        .shadow_lg()
                        .child(Input::new(&st).prefix(Icon::new(IconName::Search).small()))
                        .children(rows),
                )
                .into_any_element(),
        )
    }

    pub(super) fn render_shortcut_sheet(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.show_shortcuts {
            return None;
        }
        let pal = p();
        let cols = SHORTCUTS.iter().map(|(title, rows)| {
            v_flex()
                .w(px(262.))
                .gap_1()
                .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.accent_hover).mb_1().child(title.to_uppercase()))
                .children(rows.iter().map(|(k, v)| {
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
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.show_shortcuts = false;
                    cx.notify();
                }))
                .child(
                    v_flex()
                        .p_4()
                        .gap_3()
                        .rounded_lg()
                        .bg(pal.panel_raised)
                        .border_1()
                        .border_color(pal.border_strong)
                        .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Keyboard shortcuts"))
                        .child(h_flex().items_start().gap_6().children(cols)),
                )
                .into_any_element(),
        )
    }

    pub(super) fn set_display_mode(&mut self, m: DisplayMode, window: &mut Window, cx: &mut Context<Self>) {
        self.opts.display_mode = m;
        let d = self.display.clone();
        self.apply_display(d, window, cx);
    }

    pub(super) fn render_display_menu(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity();
        let current = self.opts.display_mode;
        Button::new("display-mode")
            .icon(Icon::empty().path(crate::assets::icon_path(current.icon())))
            .ghost()
            .xsmall()
            .tooltip("Display mode")
            .dropdown_menu(move |mut m, _, _| {
                for mode in DisplayMode::ALL {
                    let v = view.clone();
                    m = m.item(PopupMenuItem::new(mode.label()).icon(Icon::empty().path(crate::assets::icon_path(mode.icon()))).checked(mode == current).on_click(move |_, w, cx| v.update(cx, |t, cx| t.set_display_mode(mode, w, cx))));
                }
                m
            })
            .into_any_element()
    }

    // ------------------------------------------------------------ section view

    pub(super) fn toggle_section(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.opts.section.take().is_some() {
            self.section_input = None;
        } else {
            // Section through the selected plane/planar face, else the Front plane.
            let frame = self
                .selection
                .iter()
                .find_map(|r| match r {
                    Reference::Plane(_) | Reference::FeaturePlane(_) => self.display.planes.iter().find(|p| &p.reference == r).map(|p| p.frame),
                    _ => None,
                })
                .unwrap_or(noshape_model::display::PlaneFrame::FRONT);
            let n = frame.normal;
            self.opts.section = Some((n, n.dot(frame.origin)));
            let st = cx.new(|cx| InputState::new(window, cx).default_value("0 mm"));
            let base = n.dot(frame.origin);
            let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Change) {
                    let text = st.read(cx).value().to_string();
                    let units = this.doc.read(cx).session.doc.settings.units.clone();
                    if let Ok(q) = noshape_units::evaluate_as(&text, noshape_units::Dim::LENGTH, &units, &noshape_units::EmptyScope)
                        && let Some((n, _)) = this.opts.section
                    {
                        this.opts.section = Some((n, base + q.value));
                        let d = this.display.clone();
                        this.apply_display(d, window, cx);
                    }
                }
            });
            self.section_input = Some((st, sub));
        }
        let d = self.display.clone();
        self.apply_display(d, window, cx);
    }

    pub(super) fn render_section_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (st, _) = self.section_input.as_ref()?;
        let pal = p();
        Some(
            h_flex()
                .absolute()
                .top_3()
                .left(px(340.))
                .gap_2()
                .px_2()
                .py_1()
                .rounded_md()
                .bg(with_alpha(pal.panel_raised, 0.95))
                .border_1()
                .border_color(pal.accent)
                .child(div().text_xs().text_color(pal.accent_hover).child("SECTION VIEW"))
                .child(div().text_xs().text_color(pal.text_muted).child("Offset"))
                .child(div().w(px(90.)).child(Input::new(st).xsmall()))
                .child(Button::new("section-close").icon(IconName::Close).ghost().xsmall().on_click(cx.listener(|this, _: &ClickEvent, w, cx| this.toggle_section(w, cx))))
                .into_any_element(),
        )
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
        };
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, &format!("Save view : {name}"), cx, |ps| {
                ps.named_views.retain(|v| v.name != view.name);
                ps.named_views.push(view);
            });
        });
    }

    fn apply_named_view(&mut self, v: &noshape_model::partstudio::NamedView, cx: &mut Context<Self>) {
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
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.apply_named_view(&v, cx)))
                    .into_any_element()
            })
            .collect();
        let empty = rows.is_empty();
        Some(
            v_flex()
                .id("named-views")
                .absolute()
                .top(px(44.))
                .right(px(52.))
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
