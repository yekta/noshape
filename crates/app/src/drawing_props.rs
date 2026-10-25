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
use crate::os_ui::OsDropdownExt as _;
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::component::Disableable as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::document::TabContent;
use noshape_model::drawing::views;
use noshape_model::drawing::*;

pub use noshape_scene::drawing_tools::view_edit::{ORIENTATIONS, TangentMode, default_view_name, orientation_name, set_standard_orientation};

/// Views on the system clipboard (Copy in the view menu / Ctrl+C).
pub fn view_clipboard(cx: &App) -> Option<clipboard::ViewClipboard> {
    cx.read_from_clipboard().and_then(|c| c.text()).and_then(|t| clipboard::ViewClipboard::from_json(&t))
}

/// Paste, in the drawing's menus when the clipboard holds drawing content
/// (live: u-906, u-944, 1074, 1108 have it; drawing/290, with nothing
/// copied, doesn't).
pub fn paste_entry(cx: &App) -> Option<MenuEntry> {
    view_clipboard(cx).map(|_| MenuEntry::item("Paste", |t, _, cx| t.paste_views(cx)))
}

/// A View properties dropdown choice.
type PropsAct = std::rc::Rc<dyn Fn(&mut ViewProps)>;

/// View properties dialog state.
pub struct ViewProps {
    pub view: ViewId,
    orientation: Option<ViewOrientation>,
    /// View orientation › a named view of the source.
    named: Option<String>,
    /// None: parent (or sheet) scale.
    scale: Option<f64>,
    /// Child views: Sheet scale (not Parent scale) while `scale` is None.
    sheet_scale: bool,
    scale_input: Entity<InputState>,
    rotation: Entity<InputState>,
    /// Rotation angle › reverse direction (the angle turns clockwise).
    rot_reverse: bool,
    /// Suppress alignment with parent (child views).
    unaligned: bool,
    /// Apply parent view section cut (projected views of section views).
    parent_cut: bool,
    /// Show sketch appearances, Show annotations from model.
    sketch_appearances: bool,
    model_annotations: bool,
    tangent: TangentMode,
    /// View render mode › Best quality.
    best_quality: bool,
    /// Assemblies: Explode/Position and Display state (children: None =
    /// Follow parent; base views: None = Show all for the display state).
    asm_position: Option<AsmPosition>,
    display: Option<DisplayChoice>,
    sheet: usize,
    name: Entity<InputState>,
    view_label: bool,
    label: Entity<InputState>,
    scale_label: bool,
    /// View simplification (None: the parent's) and its value field.
    simplification: Option<Simplification>,
    simp_value: Entity<InputState>,
    /// The other selected views (live "View properties with several views
    /// selected"): the panel shows their common values and ✓ applies the
    /// fields changed to every one.
    others: Vec<ViewId>,
    multi: Option<MultiInit>,
    /// Tangent edges / View simplification chosen in the panel.
    tangent_pick: bool,
    simp_pick: bool,
}

/// Several views' values as the panel opened (None: they differ).
#[derive(Clone)]
struct MultiInit {
    scale_text: String,
    tangent: Option<TangentMode>,
    best_quality: Option<bool>,
    sheet: usize,
    view_label: bool,
    scale_label: bool,
    sketch_appearances: bool,
    model_annotations: bool,
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
        ];
        // Flat pattern views: bend lines and bend notes (shown by default).
        if v.flat {
            let (bl, bn) = (!v.hide_bend_lines, !v.hide_bend_notes);
            show.push(MenuEntry::item(if bl { "Hide bend lines" } else { "Show bend lines" }, move |t, _, cx| t.edit_view("Bend lines", id, cx, |v| v.hide_bend_lines = bl)));
            show.push(MenuEntry::item(if bn { "Hide bend notes" } else { "Show bend notes" }, move |t, _, cx| t.edit_view("Bend notes", id, cx, |v| v.hide_bend_notes = bn)));
        }
        // Live order (dom/drawing-replicate-menu-before.html): hidden lines,
        // shaded view, threads, faulty parts, sketches…, edges…, auto
        // centermarks, center of mass.
        show.push(MenuEntry::item(if shaded { "Hide shaded view" } else { "Show shaded view" }, move |t, _, cx| t.edit_view("Shaded view", id, cx, |v| v.style = if shaded { RenderStyle::HiddenLinesRemoved } else { RenderStyle::Shaded })));
        let threads = !v.hide_threads;
        show.push(MenuEntry::item(if threads { "Hide threads" } else { "Show threads" }, move |t, _, cx| t.edit_view("Threads", id, cx, |v| v.hide_threads = threads)));
        let faulty = v.show_faulty;
        show.push(MenuEntry::item(if faulty { "Hide faulty parts" } else { "Show faulty parts" }, move |t, _, cx| t.edit_view("Faulty parts", id, cx, |v| v.show_faulty = !faulty)));
        // Part views (not sketch views): the source's sketches and their points.
        if v.sketch.is_none() && !self.view_sketch_list(id).is_empty() {
            show.push(MenuEntry::item("Show/hide sketches...", move |t, _, cx| t.open_view_sketches(id, cx)));
            if !v.shown_sketches.is_empty() {
                let hp = v.hide_sketch_points;
                show.push(MenuEntry::item(if hp { "Show sketch points" } else { "Hide sketch points" }, move |t, _, cx| t.edit_view("Sketch points", id, cx, |v| v.hide_sketch_points = !hp)));
            }
        }
        show.push(MenuEntry::item("Show/hide edges...", move |t, _, cx| t.open_hide_edges(id, cx)));
        let cl = v.show_centerlines;
        show.push(MenuEntry::item(if cl { "Hide auto centermarks" } else { "Show auto centermarks" }, move |t, _, cx| t.edit_view("Centermarks", id, cx, |v| {
            v.show_centerlines = !cl;
            v.deleted_centermarks.clear();
        })));
        // Automatic centerlines (help: holes, cylinders and spheres seen
        // from the side): only views that have such axes.
        if self.geoms.get(&id).is_some_and(|g| !g.axes.is_empty()) {
            let al = v.auto_centerlines.unwrap_or(cl);
            show.push(MenuEntry::item(if al { "Hide centerlines" } else { "Show centerlines" }, move |t, _, cx| t.edit_view("Centerlines", id, cx, |v| v.auto_centerlines = Some(!al))));
        }
        if v.sketch.is_none() {
            // Several parts (a whole Part Studio or an assembly; a one-part
            // view has no Show/hide parts, live).
            if noshape_model::drawing::views::view_source(sheet, id).is_some_and(|(_, part)| part.is_none()) {
                show.push(MenuEntry::item("Show/hide parts...", move |t, _, cx| t.open_hide_parts(id, cx)));
            }
            let com = v.show_com;
            // Flat patterns have no center of mass (live drawing/176).
            if !v.flat {
                show.push(MenuEntry::item(if com { "Hide center of mass" } else { "Show center of mass" }, move |t, _, cx| t.edit_view("Center of mass", id, cx, |v| v.show_com = !com)));
            }
        }
        out.push(MenuEntry::Submenu("Show/hide".into(), show));
        // View orientation ▸ (base views; the current one is checked).
        if let ViewKind::Base { orientation, source, .. } = &v.kind {
            let (cur, source) = (*orientation, *source);
            let cur_named = v.named_view.clone();
            let mut entries: Vec<MenuEntry> = ORIENTATIONS
                .iter()
                .map(|&(o, name)| {
                    let same = o == cur && cur_named.is_none();
                    MenuEntry::check(name, same, move |t, _, cx| {
                        if !same {
                            t.edit_view("View orientation", id, cx, |v| set_standard_orientation(v, o))
                        }
                    })
                })
                .collect();
            // The source's named views; a deleted one stays listed, grayed (help H:views).
            let doc = &self.doc.read(cx).session.doc;
            let named = crate::drawing_insert::named_views(doc, source);
            for nv in named {
                let (name, frame) = (nv.name.clone(), views::named_view_frame(nv));
                let same = cur_named.as_ref() == Some(&nv.name);
                entries.push(MenuEntry::check(nv.name.clone(), same, move |t, _, cx| {
                    if !same {
                        let name = name.clone();
                        t.edit_view("View orientation", id, cx, move |v| {
                            v.custom_frame = Some(frame);
                            v.named_view = Some(name);
                        })
                    }
                }));
            }
            if let Some(n) = cur_named.filter(|n| !named.iter().any(|v| &v.name == n)) {
                entries.push(MenuEntry::check(n, true, |_, _, _| {}).disabled(true));
            }
            out.push(MenuEntry::Submenu("View orientation".into(), entries));
        }
        // Configuration / Explode/Position / Display state of the source,
        // right after View orientation (live drawing/1108).
        for e in self.view_extra_entries(id, cx) {
            if let MenuEntry::Submenu(label, children) = e {
                match label.as_ref() {
                    "Configuration" => out.push(MenuEntry::Submenu(label, children)),
                    "Exploded view" => out.push(MenuEntry::Submenu("Explode/Position".into(), children)),
                    "Display state" => out.push(MenuEntry::Submenu(label, children)),
                    _ => {}
                }
            }
        }
        // Tangent edges ▸ Hidden / Solid / Phantom.
        let tm = TangentMode::of(&v);
        let tangent = [TangentMode::Hidden, TangentMode::Solid, TangentMode::Phantom].into_iter().map(|m| MenuEntry::check(m.label(), m == tm, move |t, _, cx| t.edit_view("Tangent edges", id, cx, |v| m.apply(v)))).collect();
        out.push(MenuEntry::Submenu("Tangent edges".into(), tangent));
        out.push(MenuEntry::item("Adjust linestyle...", move |t, w, cx| t.open_linestyle(id, w, cx)));
        // View-type entries (Onshape: Edit / Flip direction / Remove crop ...),
        // after Paste as table (live 5664-5670 crop, 2853 auxiliary).
        let mut edits: Vec<MenuEntry> = Vec::new();
        if matches!(v.kind, ViewKind::Section { .. } | ViewKind::AlignedSection { .. }) {
            edits.push(MenuEntry::item("Flip direction", move |t, _, cx| t.flip_section(id, cx)));
            if matches!(v.kind, ViewKind::Section { .. }) {
                edits.push(MenuEntry::item("Edit section...", move |t, _, cx| t.open_section_edit(id, cx)));
            } else if sheet.section_points(id).is_some_and(|p| p.len() == 2) {
                edits.push(MenuEntry::item("Edit section", move |t, w, cx| t.open_cut_angle_panel(id, w, cx)));
            }
        }
        if matches!(v.kind, ViewKind::Auxiliary { .. }) {
            edits.push(MenuEntry::item("Edit auxiliary view...", move |t, w, cx| t.edit_aux_view(id, w, cx)));
        }
        if v.crop.is_some() {
            // Live 5671-5677: "Edit crop..." reopens the Crop view dialog.
            edits.push(MenuEntry::item("Edit crop...", move |t, _, cx| t.edit_crop(id, cx)));
            edits.push(MenuEntry::item("Remove crop", move |t, _, cx| t.edit_view("Remove crop", id, cx, |v| v.crop = None)));
        }
        if !v.broken_out.is_empty() {
            // Help broken_out_section_view: "Edit section" and "Remove section".
            let last = v.broken_out.len() - 1;
            edits.push(MenuEntry::item("Edit section", move |t, w, cx| t.open_broken_out_panel(id, last, w, cx)));
            edits.push(MenuEntry::item("Remove section", move |t, _, cx| t.edit_view("Remove broken-out sections", id, cx, |v| v.broken_out.clear())));
        }
        // Live (drawing context menu): "Insert custom table of Part Studio 1…";
        // assemblies: "Insert BOM for Assembly 1 ..." (live drawing/1108).
        if let Some(root) = views::root_view(sheet, id).and_then(|r| sheet.view(r))
            && let ViewKind::Base { source, .. } = &root.kind
            && let Some(t) = self.doc.read(cx).session.doc.tab(*source)
        {
            match t.content {
                noshape_model::TabContent::PartStudio(_) => out.push(MenuEntry::item(format!("Insert custom table of {}...", t.name), move |t, _, cx| t.set_tool(Tool::Table { kind: TableTool::Custom, view: Some(id), origin: None }, cx))),
                noshape_model::TabContent::Assembly(_) => out.push(MenuEntry::item(format!("Insert BOM for {} ...", t.name), move |t, _, cx| t.set_tool(Tool::Table { kind: TableTool::Bom, view: Some(id), origin: None }, cx))),
                _ => {}
            }
        }
        out.push(MenuEntry::Separator);
        out.push(MenuEntry::item("View properties...", move |t, w, cx| t.open_view_props(id, w, cx)));
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
        // A view linked to a parent on another sheet doesn't align with it (live x (4)).
        let free = sheet.parent_of(id).is_none_or(|p| p.is_ghost()) && !sheet.views.iter().any(|x| sheet.parent_of(x.id) == Some(id));
        // Live (behaviours "Replicate annotations with real annotations" (5)):
        // a "Rotate view" submenu "Vertical to… | Horizontal to…".
        if free {
            out.push(MenuEntry::Submenu(
                "Rotate view".into(),
                vec![
                    MenuEntry::item("Vertical to...", move |t, _, cx| t.start_rotate_to_edge(id, true, cx)),
                    MenuEntry::item("Horizontal to...", move |t, _, cx| t.start_rotate_to_edge(id, false, cx)),
                ],
            ));
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
        // Live drawing/290: Move to sheet…, Copy, Paste as table (always).
        out.push(MenuEntry::item("Move to sheet...", move |t, _, cx| t.open_move_sheet(crate::drawing_move_sheet::MoveTarget::View(id), cx)));
        // Suppress alignment with parent (derived views that align with it;
        // live drawing/447: after Move to sheet…).
        if matches!(v.kind, ViewKind::Projected { .. } | ViewKind::Auxiliary { .. } | ViewKind::Section { .. } | ViewKind::AlignedSection { .. }) {
            let un = v.unaligned;
            // Help H:views: a rotated view can't be re-aligned.
            let rotated = v.rotation.abs() > 1e-9;
            out.push(
                MenuEntry::item(if un { "Unsuppress alignment with parent" } else { "Suppress alignment with parent" }, move |t, _, cx| {
                    t.edit_view(if un { "Unsuppress alignment with parent" } else { "Suppress alignment with parent" }, id, cx, |v| v.unaligned = !un)
                })
                .disabled(un && rotated),
            );
        }
        // Live drawing/1108, u-944: Paste (with something to paste), Copy.
        out.extend(paste_entry(cx));
        out.push(MenuEntry::item("Copy", move |t, _, cx| t.copy_views(Some(id), cx)));
        out.push(MenuEntry::item("Paste as table", |t, _, cx| {
            t.start_paste_table(cx);
        }));
        out.extend(edits);
        out.extend(self.replication_entries(id));
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
            self.copy_entities(cx);
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

    /// Copy the selected annotations and tables (Ctrl+C, context menu Copy).
    pub fn copy_entities(&mut self, cx: &mut Context<Self>) {
        let anns: Vec<u64> = self.selection.iter().filter_map(|s| if let Sel::Annotation(a) = s { Some(*a) } else { None }).collect();
        let tables: Vec<u64> = self.selection.iter().filter_map(|s| if let Sel::Table(t) = s { Some(*t) } else { None }).collect();
        if let Ok(c) = self.drawing.copy_entities(self.sheet, &anns, &tables) {
            cx.write_to_clipboard(ClipboardItem::new_string(c.to_json()));
            cx.notify();
        }
    }

    /// Paste (view menu, Ctrl+V): copied views centered on the cursor.
    pub fn paste_views(&mut self, cx: &mut Context<Self>) {
        let Some(clip) = view_clipboard(cx) else { return };
        let at = self.cursor;
        let mut ids = Vec::new();
        let first = self.current_sheet().next_annotation_id();
        self.edit_sheet("Paste views", cx, |s| ids = s.paste_views(&clip, Some(at)));
        // The pasted views and annotations come out selected (live u-948).
        let anns: Vec<u64> = self.current_sheet().annotations.iter().map(|a| a.id).filter(|id| *id >= first).collect();
        self.selection = ids.into_iter().map(Sel::View).chain(anns.into_iter().map(Sel::Annotation)).collect();
        self.redraw(cx);
    }

    /// Order ▸ Bring to front / Send to back (the view list is the layer stack).
    fn order_view(&mut self, id: ViewId, front: bool, cx: &mut Context<Self>) {
        self.edit_sheet(if front { "Bring to front" } else { "Send to back" }, cx, |s| noshape_scene::drawing_tools::view_edit::order_view(s, id, front));
    }

    /// Align views ▸ Vertically / Horizontally: the next view clicked keeps
    /// its place and this view's center lines up with it.
    pub(crate) fn start_align_views(&mut self, id: ViewId, vertical: bool, cx: &mut Context<Self>) {
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
                self.edit_view("Align views", id, cx, |v| noshape_scene::drawing_tools::view_edit::align_view(v, to, vertical));
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
                self.edit_view(if vertical { "Rotate view vertically" } else { "Rotate view horizontally" }, id, cx, |v| noshape_scene::drawing_tools::view_edit::rotate_to_line(v, pa, pb, vertical));
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
        // Rotation angle 0–360 (counter-clockwise).
        let deg = v.rotation.to_degrees().rem_euclid(360.0);
        // The Scale combo shows "1:1 (Sheet)" / "1:2 (Parent)" or the view's own scale.
        let inherit = if self.current_sheet().parent_of(id).is_some() && !v.sheet_scale { "Parent" } else { "Sheet" };
        let scale_text = match v.scale {
            Some(s) => noshape_model::drawing::sheet::format_scale(s),
            None => format!("{} ({inherit})", noshape_model::drawing::sheet::format_scale(views::view_scale(self.current_sheet(), id))),
        };
        let name = v.name.clone().unwrap_or_else(|| default_view_name(&self.drawing, self.current_sheet(), &v));
        let auto_label = matches!(v.kind, ViewKind::Section { .. } | ViewKind::Detail { .. } | ViewKind::AlignedSection { .. });
        let label = match &v.kind {
            ViewKind::Section { label, .. } | ViewKind::AlignedSection { label, .. } | ViewKind::Detail { label, .. } => label.clone(),
            _ => v.label.clone().unwrap_or_default(),
        };
        let input = |text: String, window: &mut Window, cx: &mut Context<Self>| cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).default_value(text));
        self.view_props = Some(ViewProps {
            view: id,
            orientation: match &v.kind {
                ViewKind::Base { orientation, .. } => Some(*orientation),
                _ => None,
            },
            named: v.named_view.clone(),
            scale: v.scale,
            sheet_scale: v.sheet_scale,
            scale_input: input(scale_text, window, cx),
            rotation: input(format!("{}", (deg * 1000.0).round() / 1000.0), window, cx),
            rot_reverse: false,
            unaligned: v.unaligned,
            parent_cut: v.cut_from_section,
            sketch_appearances: v.sketch_appearances,
            model_annotations: v.model_annotations,
            tangent: TangentMode::of(&v),
            best_quality: v.best_quality,
            asm_position: if matches!(v.kind, ViewKind::Base { .. }) { Some(v.asm_position()) } else { v.own_position },
            display: if matches!(v.kind, ViewKind::Base { .. }) { Some(v.display_state.clone().map_or(DisplayChoice::ShowAll, DisplayChoice::State)) } else { v.own_display.clone() },
            sheet: self.sheet,
            name: input(name, window, cx),
            view_label: auto_label || v.label.as_ref().is_some_and(|l| !l.is_empty()),
            label: input(label, window, cx),
            scale_label: v.scale_label.unwrap_or(v.label.is_some() && v.scale.is_some()),
            simplification: v.simplification,
            simp_value: input(crate::drawing_insert::simplification_text(views::view_simplification(self.current_sheet(), id), self.drawing.units_mm), window, cx),
            others: Vec::new(),
            multi: None,
            tangent_pick: false,
            simp_pick: false,
        });
        cx.notify();
    }

    /// View properties of several selected views (live drw2 10-21 j): the
    /// common values, "Multiple values" where they differ.
    pub fn open_view_props_multi(&mut self, ids: Vec<ViewId>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(&first) = ids.first() else { return };
        if ids.len() == 1 {
            return self.open_view_props(first, window, cx);
        }
        self.open_view_props(first, window, cx);
        let sheet = self.current_sheet();
        let vs: Vec<DrawingView> = ids.iter().filter_map(|i| sheet.view(*i).cloned()).collect();
        let common = |f: &dyn Fn(&DrawingView) -> bool| vs.iter().all(|v| f(v) == f(&vs[0]));
        let tangent = vs.iter().all(|v| TangentMode::of(v) == TangentMode::of(&vs[0])).then(|| TangentMode::of(&vs[0]));
        let best_quality = common(&|v| v.best_quality).then_some(vs[0].best_quality);
        let s0 = views::view_scale(sheet, first);
        let scales_differ = ids.iter().any(|i| (views::view_scale(sheet, *i) - s0).abs() > 1e-9);
        let Some(p) = &mut self.view_props else { return };
        if scales_differ {
            p.scale_input = cx.new(|cx| InputState::new(window, cx).select_on_focus(false).revert_on_escape(false).placeholder("Multiple values"));
        }
        p.others = ids[1..].to_vec();
        p.multi = Some(MultiInit {
            scale_text: p.scale_input.read(cx).value().to_string(),
            tangent,
            best_quality,
            sheet: p.sheet,
            view_label: p.view_label,
            scale_label: p.scale_label,
            sketch_appearances: p.sketch_appearances,
            model_annotations: p.model_annotations,
        });
        cx.notify();
    }

    /// ✓ with several views: the fields changed, on every view.
    fn apply_view_props_multi(&mut self, p: ViewProps, m: MultiInit, cx: &mut Context<Self>) {
        let ids: Vec<ViewId> = std::iter::once(p.view).chain(p.others.iter().copied()).collect();
        let typed = p.scale_input.read(cx).value().trim().to_string();
        // None: unchanged; Some(None): the inherited scale.
        let scale: Option<Option<f64>> = if typed.is_empty() || typed == m.scale_text.trim() {
            None
        } else if typed.ends_with("(Sheet)") || typed.ends_with("(Parent)") {
            Some(None)
        } else {
            crate::drawing_ui::parse_scale(&typed).map(Some)
        };
        let simplification = p.simplification.map(|k| crate::drawing_insert::parse_simplification(k, &p.simp_value.read(cx).value(), self.drawing.units_mm).unwrap_or(k));
        let mut d = self.drawing.clone();
        for id in &ids {
            let default_name = self.current_sheet().view(*id).map(|v| default_view_name(&self.drawing, self.current_sheet(), v)).unwrap_or_default();
            let Some(v) = d.sheets[self.sheet].view_mut(*id) else { continue };
            if let Some(s) = scale {
                v.scale = s;
                v.sheet_scale = s.is_none() && typed.ends_with("(Sheet)");
            }
            if p.tangent_pick {
                p.tangent.apply(v);
            }
            if p.simp_pick {
                v.simplification = simplification;
            }
            if p.sketch_appearances != m.sketch_appearances {
                v.sketch_appearances = p.sketch_appearances;
            }
            if p.model_annotations != m.model_annotations {
                v.model_annotations = p.model_annotations;
            }
            if p.scale_label != m.scale_label {
                v.scale_label = Some(p.scale_label);
            }
            // Section / detail views always keep their labels.
            if p.view_label != m.view_label && !matches!(v.kind, ViewKind::Section { .. } | ViewKind::Detail { .. } | ViewKind::AlignedSection { .. }) {
                v.label = p.view_label.then(|| v.name.clone().unwrap_or(default_name).to_uppercase());
            }
        }
        if p.sheet != m.sheet {
            match d.move_views_to_sheet_linked(self.sheet, &ids, p.sheet) {
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

    fn apply_view_props(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self.view_props.take() else { return };
        if let Some(m) = p.multi.clone() {
            return self.apply_view_props_multi(p, m, cx);
        }
        let id = p.view;
        // Scale: "(Sheet)" / "(Parent)" keep the inherited scale; a typed N:N
        // or N/N sets the view's own.
        let typed = p.scale_input.read(cx).value().trim().to_string();
        let scale = if typed.is_empty() || typed.ends_with("(Sheet)") || typed.ends_with("(Parent)") { p.scale } else { crate::drawing_ui::parse_scale(&typed).or(p.scale) };
        // Child views: "(Sheet)" links to the sheet's scale, "(Parent)" to the parent's.
        let sheet_scale = if typed.ends_with("(Sheet)") { true } else if typed.ends_with("(Parent)") { false } else { p.sheet_scale && scale.is_none() };
        // Rotation angle: 0–360; the reverse button turns it clockwise.
        let rotation = p.rotation.read(cx).value().trim().trim_end_matches('°').parse::<f64>().ok().filter(|a| (0.0..=360.0).contains(a)).map(|a| if p.rot_reverse { -a } else { a });
        let simplification = p.simplification.map(|k| crate::drawing_insert::parse_simplification(k, &p.simp_value.read(cx).value(), self.drawing.units_mm).unwrap_or(k));
        let values = noshape_ops::drawing::ViewPropValues {
            orientation: p.orientation,
            named: p.named.clone(),
            scale,
            sheet_scale,
            rotation_deg: rotation,
            tangent: p.tangent,
            best_quality: p.best_quality,
            asm_position: p.asm_position,
            display: p.display.clone(),
            unaligned: p.unaligned,
            cut_from_section: p.parent_cut,
            sketch_appearances: p.sketch_appearances,
            model_annotations: p.model_annotations,
            simplification,
            name: p.name.read(cx).value().trim().to_string(),
            view_label: p.view_label,
            label: p.label.read(cx).value().trim().to_string(),
            scale_label: p.scale_label,
        };
        let mut d = self.drawing.clone();
        noshape_ops::drawing::apply_view_props(&self.doc.read(cx).session.doc, &mut d, self.sheet, id, &values);
        if p.sheet != self.sheet {
            // Live (behaviours "Views linked across sheets"): the view moves alone, linked to its parent.
            match d.move_views_to_sheet_linked(self.sheet, &[id], p.sheet) {
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
        // Live drawing/177 + dom/drawing-flat-view-properties.txt (help images
        // viewproperties-new-01 for child views): a 295 px dialog; a 28 px
        // header (13 px / 500 title, ✓ on #009500, ✕); the reference block
        // (75 px labels: Document, Workspace, Type, Reference with the part
        // icon and a link, the release badge), then 134 px labels with
        // 120 x 25 controls (inputs 13.33 px #555 on a #ddd bottom line),
        // checkboxes, and the BOM / cut list table references. Labels 12 px #333.
        let pal = p();
        let props = self.view_props.as_ref()?;
        // Several views (live drw2 10-21 j): no reference block, orientation,
        // name or alignment rows; rotation, render mode and the table
        // references disabled.
        let multi = props.multi.clone();
        let sheet = self.current_sheet();
        let v = sheet.view(props.view)?.clone();
        let root = views::root_view(sheet, props.view).and_then(|r| sheet.view(r)).cloned();
        let doc = &self.doc.read(cx).session.doc;
        let (source_tab, source_name, is_asm) = match root.as_ref().map(|r| &r.kind) {
            Some(ViewKind::Base { source, part, .. }) => match doc.tab(*source) {
                Some(t) => {
                    // Live: the reference is the part ("Part 1"), or the Part Studio / assembly shown whole.
                    let part_name = part.as_ref().and_then(|b| self.sources.get(&views::SourceKey::studio(*source)).and_then(|s| s.context.bodies.iter().find(|x| &x.id == b).map(|x| x.name.clone())));
                    (Some(*source), part_name.unwrap_or_else(|| t.name.clone()), matches!(t.content, TabContent::Assembly(_)))
                }
                None => (None, String::new(), false),
            },
            _ => (None, String::new(), false),
        };
        let pinned = root.as_ref().and_then(|r| r.pinned.clone());
        let doc_name = self.doc.read(cx).name();
        let parent = sheet.parent_of(props.view);
        let has_parent = parent.is_some();
        let parent_is_section = parent.and_then(|p| sheet.view(p)).is_some_and(|p| matches!(p.kind, ViewKind::Section { .. }));
        let has_children = sheet.views.iter().any(|x| sheet.parent_of(x.id) == Some(props.view));
        let ink = gpui_kit::rgb(0x333333);
        let field_ink = gpui_kit::rgb(0x555555);
        let line = gpui_kit::rgb(0xdddddd);
        let label = |w: f32, t: &'static str| div().w(px(w)).flex_none().pl(px(5.)).pt(px(7.)).text_size(px(12.)).text_color(ink).child(t);
        let info_row = |t: &'static str, body: AnyElement| h_flex().items_start().min_h(px(25.)).child(label(76., t)).child(div().flex_1().min_w_0().pl(px(5.)).pt(px(7.)).text_size(px(12.)).text_color(ink).child(body)).into_any_element();
        let row = |t: &'static str, body: AnyElement| h_flex().items_start().min_h(px(26.)).child(label(136., t)).child(div().w(px(120.)).flex_none().mt(px(1.)).child(body)).into_any_element();
        let text = |s: String| div().truncate().child(s).into_any_element();
        let value = |s: String| div().pl(px(5.)).pt(px(6.)).text_size(px(12.)).text_color(ink).child(s).into_any_element();
        let view = cx.entity();
        // Onshape's select: white, 120 x 25, a #ddd bottom line, ▾ at the right.
        let select = |id: &'static str, current: String, items: Vec<(String, bool, PropsAct)>, enabled: bool| {
            let view = view.clone();
            let b = Button::new(id).label(current).ghost().xsmall().w_full().h(px(25.)).rounded(px(0.)).border_b_1().border_color(line).text_color(field_ink).text_size(px(13.33)).dropdown_caret(true);
            if !enabled {
                return div().opacity(0.5).child(b).into_any_element();
            }
            b.os_dropdown_menu(SharedString::from(format!("{id}-dd")), true, move |mut menu, _, _| {
                for (label, checked, f) in &items {
                    let (v, f) = (view.clone(), f.clone());
                    menu = menu.item(crate::os_menu::OsMenuItem::new(label.clone()).checked(*checked).on_click(move |_, _, cx| {
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
        let field = |input: &Entity<InputState>| div().h(px(25.)).border_b_1().border_color(line).text_color(field_ink).child(Input::new(input).appearance(false).xsmall().text_size(px(13.33))).into_any_element();
        // Reference block.
        let reference = h_flex()
            .gap(px(3.))
            .child(crate::assets::icon(if is_asm { "assembly" } else { "part" }, 20., cx))
            .child(div().id("vp-ref-link").text_color(gpui_kit::rgb(0x0088cc)).cursor_pointer().child(source_name).on_click(move |_, w, cx| {
                if let Some(tab) = source_tab {
                    crate::workspace::request_activate_tab(tab, w, cx);
                }
            }))
            .into_any_element();
        // The reference's state: "In progress" for the workspace, else the version.
        let badge = div().ml(px(82.)).mt(px(4.)).mb(px(2.)).px(px(3.)).py(px(1.)).rounded(px(3.)).bg(gpui_kit::rgb(0x90cef1)).text_size(px(12.)).text_color(gpui_kit::rgb(0x000000)).child(pinned.map(|p| p.name).unwrap_or_else(|| "In progress".into()));
        let mut rows: Vec<AnyElement> = if multi.is_some() {
            vec![div().pl(px(5.)).pt(px(6.)).pb(px(2.)).text_size(px(12.)).text_color(gpui_kit::rgb(0x777777)).child("Multiple views selected.").into_any_element()]
        } else {
            vec![
                info_row("Document", text(doc_name)),
                info_row("Workspace", text("Main".into())),
                info_row("Type", text(if is_asm { "Assembly".into() } else { "Part".into() })),
                info_row("Reference", reference),
                h_flex().child(badge).into_any_element(),
            ]
        };
        // A named view's base view: its name, read-only (live drawing/1104).
        if let Some(n) = v.named_view.as_ref().filter(|_| multi.is_none()) {
            rows.push(div().h(px(4.)).into_any_element());
            rows.push(info_row("Named view", text(n.clone())));
        }
        rows.push(div().h(px(6.)).into_any_element());
        // View orientation (base views choose it; derived views show theirs).
        if let Some(cur) = props.orientation.filter(|_| multi.is_none()) {
            let cur_named = props.named.clone();
            let mut items: Vec<(String, bool, PropsAct)> = ORIENTATIONS
                .iter()
                .map(|&(o, n)| {
                    let f: PropsAct = std::rc::Rc::new(move |p| {
                        p.orientation = Some(o);
                        p.named = None;
                    });
                    (n.to_string(), cur_named.is_none() && o == cur, f)
                })
                .collect();
            if let ViewKind::Base { source, .. } = &v.kind {
                let named = crate::drawing_insert::named_views(&self.doc.read(cx).session.doc, *source);
                for nv in named {
                    let name = nv.name.clone();
                    let f: PropsAct = std::rc::Rc::new(move |p| p.named = Some(name.clone()));
                    items.push((nv.name.clone(), cur_named.as_ref() == Some(&nv.name), f));
                }
                // Deleted from the source: still listed (stored in the drawing; live drawing/1117).
                if let Some(n) = v.named_view.clone().filter(|n| !named.iter().any(|x| &x.name == n)) {
                    let name = n.clone();
                    let f: PropsAct = std::rc::Rc::new(move |p| p.named = Some(name.clone()));
                    items.push((n.clone(), cur_named.as_ref() == Some(&n), f));
                }
            }
            let label = cur_named.unwrap_or_else(|| orientation_name(cur).into());
            rows.push(row("View orientation", select("vp-orient", label, items, true)));
        } else if multi.is_none() {
            rows.push(row("View orientation", value(default_view_name(&self.drawing, sheet, &v))));
        }
        // Assemblies: Explode/Position and Display state (live drawing/1110,
        // 1111, 1115). Child views add "Follow parent" (sections and details
        // always follow); a deleted display state shows an empty pink field.
        if is_asm && multi.is_none() && let Some(src) = source_tab {
            let doc = &self.doc.read(cx).session.doc;
            if let Some(TabContent::Assembly(asm)) = doc.tab(src).map(|t| &t.content) {
                let is_base = matches!(v.kind, ViewKind::Base { .. });
                let follows = matches!(v.kind, ViewKind::Section { .. } | ViewKind::AlignedSection { .. } | ViewKind::Detail { .. });
                let cur_p = props.asm_position;
                let mut pos_items: Vec<(String, bool, PropsAct)> = Vec::new();
                for (name, p) in crate::drawing_insert::asm_positions(asm) {
                    let f: PropsAct = std::rc::Rc::new(move |pr| pr.asm_position = Some(p));
                    pos_items.push((name, cur_p == Some(p), f));
                    if !is_base && p == AsmPosition::Default {
                        let f: PropsAct = std::rc::Rc::new(|pr| pr.asm_position = None);
                        pos_items.push(("Follow parent".into(), cur_p.is_none(), f));
                    }
                }
                let pos_label = match cur_p {
                    Some(p) => crate::drawing_insert::asm_position_name(asm, p),
                    None => "Follow parent".into(),
                };
                rows.push(row("Explode/Position", select("vp-explode", pos_label, pos_items, !follows)));
                let cur_d = props.display.clone();
                let missing = matches!(&cur_d, Some(DisplayChoice::State(n)) if !asm.display_states.iter().any(|d| &d.name == n));
                let mut ds_items: Vec<(String, bool, PropsAct)> = Vec::new();
                let f: PropsAct = std::rc::Rc::new(|pr| pr.display = Some(DisplayChoice::ShowAll));
                ds_items.push(("Show all".into(), cur_d == Some(DisplayChoice::ShowAll), f));
                if !is_base {
                    let f: PropsAct = std::rc::Rc::new(|pr| pr.display = None);
                    ds_items.push(("Follow parent".into(), cur_d.is_none(), f));
                }
                for d in &asm.display_states {
                    let n = d.name.clone();
                    let f: PropsAct = std::rc::Rc::new(move |pr| pr.display = Some(DisplayChoice::State(n.clone())));
                    ds_items.push((d.name.clone(), cur_d == Some(DisplayChoice::State(d.name.clone())), f));
                }
                let ds_label = match &cur_d {
                    _ if missing => String::new(),
                    Some(DisplayChoice::ShowAll) => "Show all".into(),
                    Some(DisplayChoice::State(n)) => n.clone(),
                    None => "Follow parent".into(),
                };
                let field = select("vp-display", ds_label, ds_items, !follows);
                rows.push(row("Display state", if missing { div().bg(gpui_kit::rgb(0xfde8e1)).child(field).into_any_element() } else { field }));
            }
        }
        // Scale: one combo ("1:1 (Sheet)" / "(Parent)" or a scale): type N:N
        // or N/N, or pick one from its menu.
        // Child views: Parent scale or Sheet scale (help H:views); base views: the sheet's.
        let sheet_scale_value = if sheet.scale > 0.0 { sheet.scale } else { 1.0 };
        let parent_scale_value = parent.map(|p| views::view_scale(sheet, p)).unwrap_or(sheet_scale_value);
        let fmt = noshape_model::drawing::sheet::format_scale;
        let text_for = move |s: Option<f64>, on_sheet: bool| match s {
            None if has_parent && !on_sheet => format!("{} (Parent)", fmt(parent_scale_value)),
            None => format!("{} (Sheet)", fmt(sheet_scale_value)),
            Some(s) => fmt(s),
        };
        let mut scale_items: Vec<(String, bool, PropsAct)> = vec![(text_for(None, false), props.scale.is_none() && !props.sheet_scale, std::rc::Rc::new(|p: &mut ViewProps| {
            p.scale = None;
            p.sheet_scale = false;
        }))];
        if has_parent {
            scale_items.push((text_for(None, true), props.scale.is_none() && props.sheet_scale, std::rc::Rc::new(|p: &mut ViewProps| {
                p.scale = None;
                p.sheet_scale = true;
            })));
        }
        for s in [10.0, 5.0, 4.0, 2.0, 1.0, 0.5, 0.25, 0.2, 0.1, 0.05] {
            scale_items.push((fmt(s), props.scale == Some(s), std::rc::Rc::new(move |p: &mut ViewProps| {
                p.scale = Some(s);
                p.sheet_scale = false;
            })));
        }
        let scale_menu = {
            let view = view.clone();
            Button::new("vp-scale-dd").ghost().xsmall().w(px(20.)).h(px(24.)).dropdown_caret(true).os_dropdown_menu("vp-scale-menu", true, move |mut menu, _, _| {
                for (label, checked, f) in &scale_items {
                    let (v, f) = (view.clone(), f.clone());
                    menu = menu.item(crate::os_menu::OsMenuItem::new(label.clone()).checked(*checked).on_click(move |_, w, cx| {
                        v.update(cx, |this, cx| {
                            if let Some(p) = &mut this.view_props {
                                f(p);
                                let t = text_for(p.scale, p.sheet_scale);
                                p.scale_input.update(cx, |s, cx| s.set_value(t, w, cx));
                            }
                            cx.notify();
                        })
                    }));
                }
                menu
            })
        };
        rows.push(row("Scale", h_flex().h(px(26.)).border_b_1().border_color(line).text_color(field_ink).child(div().flex_1().child(Input::new(&props.scale_input).appearance(false).xsmall().text_size(px(13.33)))).child(scale_menu).into_any_element()));
        let check = |id: &'static str, t: &'static str, on: bool, f: fn(&mut ViewProps, bool), cx: &mut Context<Self>| {
            // A rotated view can't be re-aligned with its parent (help H:views).
            let locked = id == "vp-unaligned" && on && v.rotation.abs() > 1e-9;
            div()
                .pl(px(5.))
                .py(px(4.))
                .child(Checkbox::new(id).label(t).small().checked(on).disabled(locked).on_click(cx.listener(move |this, on: &bool, _, cx| {
                    if let Some(p) = &mut this.view_props {
                        f(p, *on);
                    }
                    cx.notify();
                })))
                .into_any_element()
        };
        // Child views: Suppress alignment with parent; projected views of a
        // section view: Apply parent view section cut (help H:views).
        if has_parent && multi.is_none() && !matches!(v.kind, ViewKind::Detail { .. }) {
            rows.push(check("vp-unaligned", "Suppress alignment with parent", props.unaligned, |p, on| p.unaligned = on, cx));
        }
        if parent_is_section && multi.is_none() && matches!(v.kind, ViewKind::Projected { .. }) {
            rows.push(check("vp-parent-cut", "Apply parent view section cut", props.parent_cut, |p, on| p.parent_cut = on, cx));
        }
        // Rotation angle (0–360) with the reverse-direction button: editable
        // only without parent and children, or with the parent alignment
        // suppressed (help H:views).
        let can_rotate = (!has_parent || props.unaligned) && !has_children && multi.is_none();
        let rev = props.rot_reverse;
        let angle = if can_rotate {
            field(&props.rotation)
        } else {
            div().h(px(25.)).pl(px(3.)).pt(px(5.)).border_b_1().border_color(line).text_size(px(13.33)).text_color(field_ink).child(format!("{}", (v.rotation.to_degrees().rem_euclid(360.0) * 1000.0).round() / 1000.0)).into_any_element()
        };
        let rotation = h_flex()
            .gap(px(6.))
            .child(div().w(px(120.)).flex_none().when(!can_rotate, |d| d.opacity(0.5)).child(angle))
            .child(
                div()
                    .id("vp-rot-reverse")
                    .size(px(26.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_1()
                    .border_color(gpui_kit::rgb(0xd3d7d9))
                    .when(rev, |d| d.bg(gpui_kit::rgb(0xd4e8f8)))
                    .when(!can_rotate, |d| d.opacity(0.5))
                    .child(crate::assets::icon("rotation-reverse", 20., cx))
                    .when(can_rotate, |d| {
                        d.cursor_pointer().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(p) = &mut this.view_props {
                                p.rot_reverse = !p.rot_reverse;
                            }
                            cx.notify();
                        }))
                    }),
            )
            .into_any_element();
        rows.push(h_flex().items_start().min_h(px(28.)).child(label(136., "Rotation angle")).child(rotation).into_any_element());
        let tangent_items = [TangentMode::Hidden, TangentMode::Solid, TangentMode::Phantom]
            .into_iter()
            .map(|m| {
                let f: PropsAct = std::rc::Rc::new(move |p| {
                    p.tangent = m;
                    p.tangent_pick = true;
                });
                (m.label().to_string(), m == props.tangent, f)
            })
            .collect();
        let tangent_label = match &multi {
            Some(mi) if mi.tangent.is_none() && !props.tangent_pick => "Multiple values".to_string(),
            _ => props.tangent.label().to_string(),
        };
        rows.push(row("Tangent edges", select("vp-tangent", tangent_label, tangent_items, true)));
        // View render mode (live drawing/1104): Best quality / Best performance (default).
        let bq = props.best_quality;
        let mode_items: Vec<(String, bool, PropsAct)> = [("Best quality", true), ("Best performance", false)]
            .into_iter()
            .map(|(n, q)| {
                let f: PropsAct = std::rc::Rc::new(move |p| p.best_quality = q);
                (n.to_string(), q == bq, f)
            })
            .collect();
        let mode_label = match multi.as_ref().map(|m| m.best_quality) {
            Some(None) => "Multiple values",
            _ if bq => "Best quality",
            _ => "Best performance",
        };
        rows.push(row("View render mode", select("vp-render-mode", mode_label.into(), mode_items, multi.is_none())));
        // View simplification (child views follow the parent unless set).
        let eff = props.simplification.unwrap_or_else(|| sheet.parent_of(props.view).map(|p| views::view_simplification(sheet, p)).unwrap_or(Simplification::Auto));
        let simp_items = crate::drawing_insert::simplification_choices(eff)
            .into_iter()
            .map(|c| {
                let f: PropsAct = std::rc::Rc::new(move |p| {
                    p.simplification = Some(c);
                    p.simp_pick = true;
                });
                (c.name().to_string(), std::mem::discriminant(&c) == std::mem::discriminant(&eff), f)
            })
            .collect();
        rows.push(row("View simplification", select("vp-simp", eff.name().into(), simp_items, true)));
        if !crate::drawing_insert::simplification_text(eff, true).is_empty() && props.simplification.is_some() {
            rows.push(row("", field(&props.simp_value)));
        }
        // Sheet: the sheet's name; with several sheets a list that moves the view.
        let sheet_name = self.drawing.sheets.get(props.sheet).map(|s| s.name.clone()).unwrap_or_default();
        if self.drawing.sheets.len() > 1 {
            let sheet_items = self
                .drawing
                .sheets
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let f: PropsAct = std::rc::Rc::new(move |p| p.sheet = i);
                    (s.name.clone(), i == props.sheet, f)
                })
                .collect();
            rows.push(row("Sheet", select("vp-sheet", sheet_name, sheet_items, true)));
        } else {
            rows.push(row("Sheet", value(sheet_name)));
        }
        if multi.is_none() {
            rows.push(row("Name", field(&props.name)));
        }
        let (vl, sl) = (props.view_label, props.scale_label);
        rows.push(check("vp-view-label", "View label", vl, |p, on| p.view_label = on, cx));
        if vl && multi.is_none() {
            rows.push(row("Label", field(&props.label)));
        }
        rows.push(check("vp-scale-label", "Scale label", sl, |p, on| p.scale_label = on, cx));
        // BOM / cut list table references: the table listing this view's parts.
        let root_id = root.as_ref().map(|r| r.id);
        let table_of = |bom: bool| {
            sheet.tables.iter().any(|t| match &t.kind {
                TableKind::Bom { view, .. } => bom && views::root_view(sheet, *view) == root_id,
                TableKind::CutList { view, .. } => !bom && views::root_view(sheet, *view) == root_id,
                _ => false,
            })
        };
        let bom = if table_of(true) { "Bill of materials" } else { "Unassigned" };
        let cut = if table_of(false) { "Cut list" } else { "Unassigned" };
        rows.push(check("vp-sketch-app", "Show sketch appearances", props.sketch_appearances, |p, on| p.sketch_appearances = on, cx));
        rows.push(check("vp-model-ann", "Show annotations from model", props.model_annotations, |p, on| p.model_annotations = on, cx));
        rows.push(row("BOM table reference", select("vp-bom", bom.into(), Vec::new(), false)));
        rows.push(row("Cut list table reference", select("vp-cut", cut.into(), Vec::new(), false)));
        Some(
            v_flex()
                .id("dr-view-props")
                .absolute()
                .top(px(DIALOG_TOP))
                // Beside the Sheets panel when it is open (it opened under it).
                .left(px(self.dialog_left()))
                .w(px(295.))
                .bg(pal.dialog_bg)
                .shadow(crate::drawing_chrome::dialog_shadow())
                .text_color(ink)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .h(px(28.))
                        .pl(px(11.))
                        .border_b_1()
                        .border_color(gpui_kit::rgb(0xd3d7d9))
                        .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("View properties"))
                        .child(div().id("vp-ok").w(px(28.)).h(px(27.)).flex().items_center().justify_center().bg(gpui_kit::rgb(0x009500)).cursor_pointer().child(crate::ui::glyph("check", 18., gpui_kit::white())).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.apply_view_props(cx))))
                        .child(div().id("vp-cancel").w(px(28.)).h(px(27.)).flex().items_center().justify_center().cursor_pointer().child(crate::drawing_chrome::cancel_x(cx)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.view_props = None;
                            cx.notify();
                        }))),
                )
                .child(v_flex().p(px(3.)).pb(px(6.)).children(rows))
                .into_any_element(),
        )
    }
}
