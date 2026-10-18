//! Drawing editor, Onshape view behaviours: the view context menu in
//! Onshape's order (Show/hide, View orientation, Tangent edges | View
//! properties, Order, Align views, Rotate view | Switch to, Move to sheet |
//! Clear selection, Zoom to fit, Delete) and the View properties dialog
//! (double-click a view).

use crate::drawing_view::*;
use crate::theme::p;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use noshape_model::document::TabContent;
use noshape_model::drawing::views;
use noshape_model::drawing::*;

/// Tangent edges display (Onshape: Hidden / Solid / Phantom).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TangentMode {
    Hidden,
    Solid,
    Phantom,
}

impl TangentMode {
    pub fn of(v: &DrawingView) -> Self {
        match (v.show_tangent_edges, v.tangent_phantom) {
            (false, _) => TangentMode::Hidden,
            (true, false) => TangentMode::Solid,
            (true, true) => TangentMode::Phantom,
        }
    }
    fn label(self) -> &'static str {
        match self {
            TangentMode::Hidden => "Hidden",
            TangentMode::Solid => "Solid",
            TangentMode::Phantom => "Phantom",
        }
    }
    fn apply(self, v: &mut DrawingView) {
        v.show_tangent_edges = self != TangentMode::Hidden;
        v.tangent_phantom = self == TangentMode::Phantom;
    }
}

const ORIENTATIONS: [(ViewOrientation, &str); 7] = [
    (ViewOrientation::Top, "Top"),
    (ViewOrientation::Left, "Left"),
    (ViewOrientation::Right, "Right"),
    (ViewOrientation::Front, "Front"),
    (ViewOrientation::Back, "Back"),
    (ViewOrientation::Bottom, "Bottom"),
    (ViewOrientation::Isometric, "Isometric"),
];

fn orientation_name(o: ViewOrientation) -> &'static str {
    ORIENTATIONS.iter().find(|(x, _)| *x == o).map(|(_, n)| *n).unwrap_or("Front")
}

/// Default view name: the orientation of a base view, else the view type.
pub fn default_view_name(v: &DrawingView) -> String {
    match &v.kind {
        ViewKind::Base { orientation, .. } => orientation_name(*orientation).to_string(),
        ViewKind::Projected { .. } => "Projected view".into(),
        ViewKind::Auxiliary { .. } => "Auxiliary view".into(),
        ViewKind::Section { label, .. } | ViewKind::AlignedSection { label, .. } => format!("Section {label}-{label}"),
        ViewKind::Detail { label, .. } => format!("Detail {label}"),
    }
}

/// Views on the system clipboard (Copy in the view menu / Ctrl+C).
pub fn view_clipboard(cx: &App) -> Option<clipboard::ViewClipboard> {
    cx.read_from_clipboard().and_then(|c| c.text()).and_then(|t| clipboard::ViewClipboard::from_json(&t))
}

/// View properties dialog state.
pub struct ViewProps {
    pub view: ViewId,
    orientation: Option<ViewOrientation>,
    /// None: parent (or sheet) scale.
    scale: Option<f64>,
    scale_input: Entity<InputState>,
    rotation: Entity<InputState>,
    tangent: TangentMode,
    sheet: usize,
    name: Entity<InputState>,
    view_label: bool,
    label: Entity<InputState>,
    scale_label: bool,
}

impl DrawingEditor {
    // ------------------------------------------------------------ context menu

    /// The view context menu in Onshape's order.
    pub fn onshape_view_entries(&self, id: ViewId, cx: &App) -> Vec<MenuEntry> {
        let mut out = Vec::new();
        let sheet = self.current_sheet();
        let Some(v) = sheet.view(id).cloned() else { return out };
        // Show/hide ▸
        let hidden_lines = v.style == RenderStyle::HiddenLinesVisible;
        let shaded = v.style == RenderStyle::Shaded;
        let mut show = vec![
            MenuEntry::item(if hidden_lines { "Hide hidden lines" } else { "Show hidden lines" }, move |t, _, cx| {
                t.edit_view("Hidden lines", id, cx, |v| v.style = if hidden_lines { RenderStyle::HiddenLinesRemoved } else { RenderStyle::HiddenLinesVisible })
            }),
            MenuEntry::item(if shaded { "Hide shaded view" } else { "Show shaded view" }, move |t, _, cx| t.edit_view("Shaded view", id, cx, |v| v.style = if shaded { RenderStyle::HiddenLinesRemoved } else { RenderStyle::Shaded })),
        ];
        let cl = v.show_centerlines;
        show.push(MenuEntry::item(if cl { "Hide auto centermarks" } else { "Show auto centermarks" }, move |t, _, cx| t.edit_view("Centermarks", id, cx, |v| v.show_centerlines = !cl)));
        out.push(MenuEntry::Submenu("Show/hide".into(), show));
        // View orientation ▸ (base views; the current one is checked).
        if let ViewKind::Base { orientation, .. } = &v.kind {
            let cur = *orientation;
            let entries = ORIENTATIONS
                .iter()
                .map(|&(o, name)| {
                    MenuEntry::check(name, o == cur, move |t, _, cx| {
                        if o != cur {
                            t.edit_view("View orientation", id, cx, |v| {
                                if let ViewKind::Base { orientation, .. } = &mut v.kind {
                                    *orientation = o;
                                }
                            })
                        }
                    })
                })
                .collect();
            out.push(MenuEntry::Submenu("View orientation".into(), entries));
        }
        // Tangent edges ▸ Hidden / Solid / Phantom.
        let tm = TangentMode::of(&v);
        let tangent = [TangentMode::Hidden, TangentMode::Solid, TangentMode::Phantom].into_iter().map(|m| MenuEntry::check(m.label(), m == tm, move |t, _, cx| t.edit_view("Tangent edges", id, cx, |v| m.apply(v)))).collect();
        out.push(MenuEntry::Submenu("Tangent edges".into(), tangent));
        // View-type entries (Onshape: Edit / Flip direction / Remove crop ...).
        if matches!(v.kind, ViewKind::Section { .. } | ViewKind::AlignedSection { .. }) {
            out.push(MenuEntry::item("Flip direction", move |t, _, cx| t.flip_section(id, cx)));
            if sheet.section_points(id).is_some_and(|p| p.len() == 2) {
                out.push(MenuEntry::item("Edit section", move |t, w, cx| t.open_cut_angle_panel(id, w, cx)));
            }
        }
        if v.crop.is_some() {
            out.push(MenuEntry::item("Remove crop", move |t, _, cx| t.edit_view("Remove crop", id, cx, |v| v.crop = None)));
        }
        if !v.broken_out.is_empty() {
            out.push(MenuEntry::item("Remove section", move |t, _, cx| t.edit_view("Remove broken-out sections", id, cx, |v| v.broken_out.clear())));
        }
        // Configuration / Explode/Position of the source (help: view context menu).
        for e in self.view_extra_entries(id, cx) {
            if let MenuEntry::Submenu(label, children) = e {
                match label.as_ref() {
                    "Configuration" => out.push(MenuEntry::Submenu(label, children)),
                    "Exploded view" => out.push(MenuEntry::Submenu("Explode/Position".into(), children)),
                    _ => {}
                }
            }
        }
        out.push(MenuEntry::Separator);
        out.push(MenuEntry::item("View properties…", move |t, w, cx| t.open_view_props(id, w, cx)));
        out.push(MenuEntry::Submenu(
            "Order".into(),
            vec![
                MenuEntry::item("Bring to front", move |t, _, cx| t.order_view(id, true, cx)),
                MenuEntry::item("Send to back", move |t, _, cx| t.order_view(id, false, cx)),
            ],
        ));
        out.push(MenuEntry::Submenu(
            "Align views".into(),
            vec![
                MenuEntry::item("Vertically", move |t, _, cx| t.start_align_views(id, true, cx)),
                MenuEntry::item("Horizontally", move |t, _, cx| t.start_align_views(id, false, cx)),
            ],
        ));
        // Rotate view vertically / horizontally: pick a straight edge (views
        // without a parent or children, like the rotation angle).
        let free = sheet.parent_of(id).is_none() && !sheet.views.iter().any(|x| sheet.parent_of(x.id) == Some(id));
        if free {
            out.push(MenuEntry::item("Rotate view vertically", move |t, _, cx| t.start_rotate_to_edge(id, true, cx)));
            out.push(MenuEntry::item("Rotate view horizontally", move |t, _, cx| t.start_rotate_to_edge(id, false, cx)));
        }
        out.push(MenuEntry::Separator);
        // Switch to <source tab>.
        if let Some(root) = views::root_view(sheet, id).and_then(|r| sheet.view(r))
            && let ViewKind::Base { source, .. } = &root.kind
            && let Some(name) = self.doc.read(cx).session.doc.tab(*source).map(|t| t.name.clone())
        {
            let tab = *source;
            out.push(MenuEntry::item(format!("Switch to {name}"), move |_, w, cx| crate::workspace::request_activate_tab(tab, w, cx)));
        }
        // Move to sheet ▸ (from the existing entries).
        for e in self.view_more_entries(id) {
            if matches!(&e, MenuEntry::Submenu(l, _) if l.as_ref() == "Move to sheet") {
                out.push(e);
            }
        }
        out.push(MenuEntry::item("Copy", move |t, _, cx| t.copy_views(Some(id), cx)));
        if view_clipboard(cx).is_some() {
            out.push(MenuEntry::item("Paste", |t, _, cx| t.paste_views(cx)));
        }
        out.push(MenuEntry::Separator);
        out.push(MenuEntry::item("Clear selection", |t, _, cx| {
            t.selection.clear();
            t.redraw(cx);
        }));
        out.push(MenuEntry::item("Zoom to fit", |t, _, cx| t.zoom_to_fit(cx)));
        out.push(MenuEntry::item("Delete", move |t, _, cx| {
            t.selection.clear();
            t.edit_sheet("Delete view", cx, |s| s.remove_view(id));
        }));
        out
    }

    /// Copy (view menu, Ctrl+C): the selected views (or `id`) with their
    /// derived views, annotations and tables.
    pub fn copy_views(&mut self, id: Option<ViewId>, cx: &mut Context<Self>) {
        let mut ids: Vec<ViewId> = self.selection.iter().filter_map(|s| if let Sel::View(v) = s { Some(*v) } else { None }).collect();
        if let Some(id) = id.filter(|i| !ids.contains(i)) {
            ids = vec![id];
        }
        if ids.is_empty() {
            return;
        }
        match self.drawing.copy_views(self.sheet, &ids, Some(&self.sources)) {
            Ok(c) => {
                let n = c.sheet.views.len();
                cx.write_to_clipboard(ClipboardItem::new_string(c.to_json()));
                self.status = Some(if n == 1 { "Copied 1 view".into() } else { format!("Copied {n} views").into() });
            }
            Err(e) => self.status = Some(e.into()),
        }
        cx.notify();
    }

    /// Paste (view menu, Ctrl+V): copied views centered on the cursor.
    pub fn paste_views(&mut self, cx: &mut Context<Self>) {
        let Some(clip) = view_clipboard(cx) else { return };
        let at = self.cursor;
        let mut ids = Vec::new();
        self.edit_sheet("Paste views", cx, |s| ids = s.paste_views(&clip, Some(at)));
        self.selection = ids.into_iter().map(Sel::View).collect();
        self.redraw(cx);
    }

    /// Order ▸ Bring to front / Send to back (the view list is the layer stack).
    fn order_view(&mut self, id: ViewId, front: bool, cx: &mut Context<Self>) {
        self.edit_sheet(if front { "Bring to front" } else { "Send to back" }, cx, |s| {
            if let Some(i) = s.views.iter().position(|v| v.id == id) {
                let v = s.views.remove(i);
                if front {
                    s.views.push(v);
                } else {
                    s.views.insert(0, v);
                }
            }
        });
    }

    /// Align views ▸ Vertically / Horizontally: the next view clicked keeps
    /// its place and this view's center lines up with it.
    fn start_align_views(&mut self, id: ViewId, vertical: bool, cx: &mut Context<Self>) {
        self.align_views = Some((id, vertical));
        self.status = Some("Align views: select the view to align with".into());
        cx.notify();
    }

    /// Click while aligning views. True when the click was used.
    pub fn align_views_click(&mut self, cx: &mut Context<Self>) -> bool {
        let Some((id, vertical)) = self.align_views.take() else { return false };
        self.update_hover(cx);
        let target = self.sel_at(self.hover, self.cursor).and_then(|s| if let Sel::View(v) = s { Some(v) } else { None });
        match target.and_then(|t| self.current_sheet().view(t)).map(|t| t.position) {
            Some(to) if target != Some(id) => {
                self.edit_view("Align views", id, cx, |v| {
                    if vertical {
                        v.position[0] = to[0];
                    } else {
                        v.position[1] = to[1];
                    }
                });
                self.status = None;
            }
            _ => self.status = Some("Align views cancelled".into()),
        }
        cx.notify();
        true
    }

    fn start_rotate_to_edge(&mut self, id: ViewId, vertical: bool, cx: &mut Context<Self>) {
        self.rotate_to_edge = Some((id, vertical));
        self.status = Some("Select a straight edge to rotate the view to".into());
        cx.notify();
    }

    /// Click while rotating a view to an edge: the view turns (by the
    /// smallest angle) so the clicked straight edge is vertical / horizontal.
    pub fn rotate_to_edge_click(&mut self, cx: &mut Context<Self>) -> bool {
        let Some((id, vertical)) = self.rotate_to_edge.take() else { return false };
        self.update_hover(cx);
        let pick = self.line_pick().and_then(|p| if let Pick::Line { a, b } = p { Some((a, b)) } else { None });
        match pick.filter(|(a, _)| a.view == id).and_then(|(a, b)| self.placement(id).map(|pl| (pl.to_sheet(a.p), pl.to_sheet(b.p)))) {
            Some((pa, pb)) => {
                let angle = (pb[1] - pa[1]).atan2(pb[0] - pa[0]);
                let target = if vertical { std::f64::consts::FRAC_PI_2 } else { 0.0 };
                // Lines have no direction: turn by the smallest angle (mod 180°).
                let pi = std::f64::consts::PI;
                let mut delta = (target - angle).rem_euclid(pi);
                if delta > pi / 2.0 {
                    delta -= pi;
                }
                self.edit_view(if vertical { "Rotate view vertically" } else { "Rotate view horizontally" }, id, cx, |v| v.rotation += delta);
                self.status = None;
            }
            None => self.status = Some("Rotate view cancelled: select a straight edge of the view".into()),
        }
        cx.notify();
        true
    }

    // ------------------------------------------------------------ View properties

    pub fn open_view_props(&mut self, id: ViewId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(v) = self.current_sheet().view(id).cloned() else { return };
        self.panel = None;
        let deg = v.rotation.to_degrees();
        let scale_text = v.scale.map(noshape_model::drawing::sheet::format_scale).unwrap_or_default();
        let name = v.name.clone().unwrap_or_else(|| default_view_name(&v));
        let auto_label = matches!(v.kind, ViewKind::Section { .. } | ViewKind::Detail { .. } | ViewKind::AlignedSection { .. });
        let label = match &v.kind {
            ViewKind::Section { label, .. } | ViewKind::AlignedSection { label, .. } | ViewKind::Detail { label, .. } => label.clone(),
            _ => v.label.clone().unwrap_or_default(),
        };
        let input = |text: String, window: &mut Window, cx: &mut Context<Self>| cx.new(|cx| InputState::new(window, cx).default_value(text));
        self.view_props = Some(ViewProps {
            view: id,
            orientation: match &v.kind {
                ViewKind::Base { orientation, .. } => Some(*orientation),
                _ => None,
            },
            scale: v.scale,
            scale_input: input(scale_text, window, cx),
            rotation: input(format!("{}", (deg * 1000.0).round() / 1000.0), window, cx),
            tangent: TangentMode::of(&v),
            sheet: self.sheet,
            name: input(name, window, cx),
            view_label: auto_label || v.label.as_ref().is_some_and(|l| !l.is_empty()),
            label: input(label, window, cx),
            scale_label: v.scale_label.unwrap_or(v.label.is_some() && v.scale.is_some()),
        });
        cx.notify();
    }

    fn apply_view_props(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self.view_props.take() else { return };
        let id = p.view;
        let typed_scale = crate::drawing_ui::parse_scale(&p.scale_input.read(cx).value());
        let scale = if p.scale_input.read(cx).value().trim().is_empty() { p.scale } else { typed_scale.or(p.scale) };
        let rotation = p.rotation.read(cx).value().trim().trim_end_matches('°').parse::<f64>().ok();
        let name = p.name.read(cx).value().trim().to_string();
        let label = p.label.read(cx).value().trim().to_string();
        let (view_label, scale_label, tangent, orientation) = (p.view_label, p.scale_label, p.tangent, p.orientation);
        let Some(old) = self.current_sheet().view(id).cloned() else { return };
        let default_name = default_view_name(&old);
        let mut d = self.drawing.clone();
        if let Some(v) = d.sheets[self.sheet].view_mut(id) {
            v.scale = scale;
            if let Some(r) = rotation {
                v.rotation = r.to_radians();
            }
            tangent.apply(v);
            v.name = (!name.is_empty() && name != default_name).then_some(name);
            v.scale_label = Some(scale_label);
            match &mut v.kind {
                ViewKind::Section { label: l, .. } | ViewKind::AlignedSection { label: l, .. } | ViewKind::Detail { label: l, .. } => {
                    if !label.is_empty() {
                        *l = label;
                    }
                }
                _ => v.label = (view_label && !label.is_empty()).then_some(label).or_else(|| view_label.then(|| v.name.clone().unwrap_or(default_name.clone()).to_uppercase())),
            }
            if let (ViewKind::Base { orientation: o, .. }, Some(n)) = (&mut v.kind, orientation) {
                *o = n;
            }
        }
        if p.sheet != self.sheet {
            match d.move_views_to_sheet(self.sheet, &[id], p.sheet, Some(&self.sources)) {
                Ok(_) => {
                    self.commit("View properties", d, cx);
                    self.switch_sheet(p.sheet, cx);
                }
                Err(e) => self.status = Some(e.into()),
            }
        } else {
            self.commit("View properties", d, cx);
        }
        cx.notify();
    }

    pub fn render_view_props(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        let props = self.view_props.as_ref()?;
        let sheet = self.current_sheet();
        let v = sheet.view(props.view)?.clone();
        let root = views::root_view(sheet, props.view).and_then(|r| sheet.view(r)).cloned();
        let doc = &self.doc.read(cx).session.doc;
        let (source_name, is_asm) = match root.as_ref().map(|r| &r.kind) {
            Some(ViewKind::Base { source, .. }) => match doc.tab(*source) {
                Some(t) => (t.name.clone(), matches!(t.content, TabContent::Assembly(_))),
                None => (String::new(), false),
            },
            _ => (String::new(), false),
        };
        let doc_name = self.doc.read(cx).name();
        let has_parent = sheet.parent_of(props.view).is_some();
        let has_children = sheet.views.iter().any(|x| sheet.parent_of(x.id) == Some(props.view));
        let parent_scale = views::view_scale(sheet, props.view);
        let label_w = px(125.);
        let row = |label: &'static str, body: AnyElement| h_flex().min_h(px(28.)).gap_2().child(div().w(label_w).flex_none().text_color(pal.text).child(label)).child(div().flex_1().min_w_0().child(body)).into_any_element();
        let text = |s: String| div().truncate().child(s).into_any_element();
        let view = cx.entity();
        let dropdown = |id: &'static str, current: String, items: Vec<(String, bool, std::rc::Rc<dyn Fn(&mut ViewProps)>)>| {
            let view = view.clone();
            Button::new(id)
                .label(current)
                .outline()
                .xsmall()
                .w_full()
                .dropdown_caret(true)
                .dropdown_menu(move |mut menu, _, _| {
                    for (label, checked, f) in &items {
                        let (v, f) = (view.clone(), f.clone());
                        menu = menu.item(PopupMenuItem::new(label.clone()).checked(*checked).on_click(move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                if let Some(p) = &mut this.view_props {
                                    f(p);
                                }
                                cx.notify();
                            })
                        }));
                    }
                    menu
                })
                .into_any_element()
        };
        let mut rows: Vec<AnyElement> = vec![
            row("Document", text(doc_name)),
            row("Workspace", text("Main".into())),
            row("Type", text(if is_asm { "Assembly".into() } else { "Part".into() })),
            row("Reference", text(source_name)),
        ];
        // View orientation (base views).
        if let Some(cur) = props.orientation {
            let items = ORIENTATIONS
                .iter()
                .map(|&(o, n)| {
                    let f: std::rc::Rc<dyn Fn(&mut ViewProps)> = std::rc::Rc::new(move |p| p.orientation = Some(o));
                    (n.to_string(), o == cur, f)
                })
                .collect();
            rows.push(row("View orientation", dropdown("vp-orient", orientation_name(cur).into(), items)));
        } else {
            rows.push(row("View orientation", text(match &v.kind {
                ViewKind::Projected { .. } => "Projected".into(),
                _ => default_view_name(&v),
            })));
        }
        // Scale: "1:2 (Parent)" / "(Sheet)" or a standard scale; typing sets a custom one.
        let inherit = if has_parent { "Parent" } else { "Sheet" };
        let scale_label_text = match props.scale {
            None => format!("{} ({inherit})", noshape_model::drawing::sheet::format_scale(parent_scale)),
            Some(s) => noshape_model::drawing::sheet::format_scale(s),
        };
        let mut scale_items: Vec<(String, bool, std::rc::Rc<dyn Fn(&mut ViewProps)>)> = vec![(format!("{inherit} scale"), props.scale.is_none(), std::rc::Rc::new(|p: &mut ViewProps| p.scale = None))];
        for s in [10.0, 5.0, 4.0, 2.0, 1.0, 0.5, 0.25, 0.2, 0.1, 0.05] {
            scale_items.push((noshape_model::drawing::sheet::format_scale(s), props.scale == Some(s), std::rc::Rc::new(move |p: &mut ViewProps| p.scale = Some(s))));
        }
        rows.push(row("Scale", h_flex().gap_1().child(div().flex_1().child(dropdown("vp-scale", scale_label_text, scale_items))).child(Input::new(&props.scale_input).xsmall().w(px(64.))).into_any_element()));
        // Rotation: only views without a parent or children.
        let can_rotate = !has_parent && !has_children;
        rows.push(row(
            "Rotation angle",
            if can_rotate { Input::new(&props.rotation).xsmall().into_any_element() } else { div().text_color(pal.text_faint).child(format!("{}", (v.rotation.to_degrees() * 1000.0).round() / 1000.0)).into_any_element() },
        ));
        let tangent_items = [TangentMode::Hidden, TangentMode::Solid, TangentMode::Phantom]
            .into_iter()
            .map(|m| {
                let f: std::rc::Rc<dyn Fn(&mut ViewProps)> = std::rc::Rc::new(move |p| p.tangent = m);
                (m.label().to_string(), m == props.tangent, f)
            })
            .collect();
        rows.push(row("Tangent edges", dropdown("vp-tangent", props.tangent.label().into(), tangent_items)));
        let sheet_items = self
            .drawing
            .sheets
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let f: std::rc::Rc<dyn Fn(&mut ViewProps)> = std::rc::Rc::new(move |p| p.sheet = i);
                (s.name.clone(), i == props.sheet, f)
            })
            .collect();
        let sheet_name = self.drawing.sheets.get(props.sheet).map(|s| s.name.clone()).unwrap_or_default();
        rows.push(row("Sheet", dropdown("vp-sheet", sheet_name, sheet_items)));
        rows.push(row("Name", Input::new(&props.name).xsmall().into_any_element()));
        let (vl, sl) = (props.view_label, props.scale_label);
        rows.push(
            Checkbox::new("vp-view-label")
                .label("View label")
                .checked(vl)
                .on_click(cx.listener(|this, on: &bool, _, cx| {
                    if let Some(p) = &mut this.view_props {
                        p.view_label = *on;
                    }
                    cx.notify();
                }))
                .into_any_element(),
        );
        if vl {
            rows.push(row("Label", Input::new(&props.label).xsmall().into_any_element()));
        }
        rows.push(
            Checkbox::new("vp-scale-label")
                .label("Scale label")
                .checked(sl)
                .on_click(cx.listener(|this, on: &bool, _, cx| {
                    if let Some(p) = &mut this.view_props {
                        p.scale_label = *on;
                    }
                    cx.notify();
                }))
                .into_any_element(),
        );
        Some(
            v_flex()
                .id("dr-view-props")
                .absolute()
                .top_2()
                .left_2()
                .w(px(300.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_size(px(13.))
                .text_color(pal.text)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    // Onshape dialog header: title, green check, red X.
                    h_flex()
                        .h(px(28.))
                        .pl(px(6.))
                        .bg(pal.panel)
                        .border_b_1()
                        .border_color(pal.border)
                        .child(div().flex_1().font_weight(FontWeight::MEDIUM).child("View properties"))
                        .child(Button::new("vp-ok").ghost().child(crate::ui::glyph("check", 16., pal.accept)).w(px(28.)).h(px(28.)).p_0().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.apply_view_props(cx))))
                        .child(Button::new("vp-cancel").ghost().child(crate::ui::glyph("close", 14., pal.cancel)).w(px(28.)).h(px(28.)).p_0().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.view_props = None;
                            cx.notify();
                        }))),
                )
                .child(v_flex().p_2().gap_1().children(rows))
                .into_any_element(),
        )
    }
}
