//! Viewport context menu, measurement panel, mass properties and part
//! properties for the Part Studio view.

use super::PartStudioView;
use crate::os_menu::{OsMenu, OsMenuItem};
use crate::scene_builder::{PickKind, PickTarget};
use crate::theme::{p, with_alpha};
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::dialog::{DialogAction, DialogClose, DialogFooter};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use noshape_model::partstudio::{Appearance, PartProperties};
use noshape_model::reference::{BodyId, FeatureId, Reference};
use noshape_model::regen::measure::{self, Measurement};
use noshape_render::StandardView;
use noshape_units::{Dim, Quantity};

/// Common materials with densities (kg/m³).
pub const MATERIALS: &[(&str, f64)] = &[
    ("Steel", 7850.0),
    ("Stainless steel", 8000.0),
    ("Aluminum 6061", 2700.0),
    ("Titanium Ti-6Al-4V", 4430.0),
    ("Brass", 8500.0),
    ("Copper", 8960.0),
    ("ABS", 1040.0),
    ("PLA", 1240.0),
    ("PETG", 1270.0),
    ("Nylon", 1150.0),
    ("Acrylic", 1180.0),
    ("Oak", 750.0),
    ("Plywood", 600.0),
];

const SWATCHES: &[u32] = &[0xb8bec9, 0x9fb7d6, 0xd6b89f, 0xa7cfae, 0xcfa7c7, 0xd6d29f, 0x9fd3d6, 0xc9a9a9, 0x5d6470, 0x2f3540, 0xe0e3e8, 0xd9534f];

/// A context menu entry's action on the view.
type MenuAction = Box<dyn Fn(&mut PartStudioView, &mut Window, &mut Context<PartStudioView>)>;

impl PartStudioView {
    /// Body owning a referenced entity.
    fn owning_body(&self, r: &Reference) -> Option<BodyId> {
        match r {
            Reference::Body(b) => Some(b.clone()),
            _ => self.display.bodies.iter().find(|b| b.face_refs.contains(r) || b.edge_refs.contains(r) || b.vertex_refs.contains(r)).map(|b| b.id.clone()),
        }
    }

    /// Feature that created a referenced entity.
    fn creating_feature(&self, r: &Reference) -> Option<FeatureId> {
        match r {
            Reference::SketchEntity { sketch, .. } | Reference::SketchRegion { sketch, .. } => Some(*sketch),
            Reference::Face(n) => n.creator(),
            Reference::Edge(n) | Reference::Vertex(n) => super::edge_creator(n),
            Reference::Body(b) => Some(b.feature),
            Reference::FeaturePlane(f) => Some(*f),
            Reference::MateConnector { feature, .. } => Some(*feature),
            _ => None,
        }
    }

    pub(super) fn open_context_menu(&mut self, target: Option<PickTarget>, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        // Onshape (live capture): the menu lists, in groups, the feature
        // commands of the clicked entity, creation tools, visibility, export,
        // selection, view and delete commands, then materials/appearances.
        let view = cx.entity();
        let in_sketch = self.sketch.is_some();
        let target_ref = target.as_ref().map(|t| t.reference.clone());
        let target_ref_for_highlight = target_ref.clone();
        let kind = target.as_ref().map(|t| t.kind);
        let body = target_ref.as_ref().and_then(|r| self.owning_body(r));
        let body_name = body.as_ref().and_then(|b| self.display.body(b)).map(|b| b.name.clone()).unwrap_or_else(|| "part".into());
        let creator = target_ref.as_ref().and_then(|r| self.creating_feature(r));
        let studio = self.studio(cx);
        let creator_name = creator.and_then(|id| studio.and_then(|s| s.feature(id)).map(|f| f.name.clone()));
        let source_sketch = creator.and_then(|id| studio.and_then(|s| s.feature(id).and_then(|f| noshape_model::interaction::source_sketch(&s.features, f)).and_then(|sid| s.feature(sid).map(|g| (sid, g.name.clone())))));
        let sheet_metal = body.as_ref().is_some_and(|b| self.context.as_ref().is_some_and(|c| c.sheet_metal.contains_key(b)));
        let has_selection = !self.selection.is_empty() || !self.selected_features.is_empty();
        let isolate = self.ui.isolate.as_ref().map(|i| i.mode);
        let section = !self.ui.section.is_empty();
        let editing = self.edit.is_some();
        let editing_tool = self.edit.as_ref().and_then(|e| e.dialog.read(cx).feature.feature_type()).map(|t| t.name);
        let planar = target_ref.as_ref().is_some_and(|r| self.frame_of_ref(r).is_some());
        let linear_edge = target_ref.as_ref().is_some_and(|r| self.linear_edge_direction(r).is_some());
        let plane_name = target_ref.as_ref().map(|r| self.labeler(cx)(r)).unwrap_or_default();
        let paste_sketch = Self::clipboard_has_sketch(cx);
        // Faces the appearance entry applies to: the selected faces plus this one.
        let mut faces = self.selected_faces();
        if let Some(r @ Reference::Face(_)) = &target_ref
            && !faces.contains(r)
        {
            faces.push(r.clone());
        }
        let menu = OsMenu::build(window, cx, move |mut m, window, cx| {
            let v = view.clone();
            let item = |label: &str, f: MenuAction| {
                let v = v.clone();
                OsMenuItem::new(label.to_string()).on_click(move |_, window, cx| v.update(cx, |this, cx| {
                    this.close_context_menu(cx);
                    f(this, window, cx)
                }))
            };
            let icon_item = |label: &str, icon: &str, f: MenuAction| item(label, f).icon_name(icon);
            // Start a tool with `r` preselected.
            let tool = |label: &str, icon: &str, tool_id: &'static str, r: &Reference| {
                let r = r.clone();
                icon_item(label, icon, Box::new(move |this, w, cx| {
                    this.selection = vec![r.clone()];
                    this.start_feature(tool_id, w, cx);
                }))
            };
            let select_menu = |m: OsMenu, r: &Reference, k: PickKind, window: &mut Window, cx: &mut Context<OsMenu>| -> OsMenu {
                // Select ▸ grows the entity into related ones; Select other… lists what is under the cursor.
                let criteria: &[noshape_model::select::Criterion] = match k {
                    PickKind::Face => &noshape_model::select::Criterion::FACE,
                    PickKind::Edge => &noshape_model::select::Criterion::EDGE,
                    _ => &[],
                };
                let mut m = m;
                if !criteria.is_empty() {
                    let (vs, rs) = (v.clone(), r.clone());
                    m = m.submenu("Select", window, cx, move |mut sm, _, _| {
                        for c in criteria {
                            let (v, r, c) = (vs.clone(), rs.clone(), *c);
                            sm = sm.item(OsMenuItem::new(c.label()).on_click(move |_, _, cx| v.update(cx, |this, cx| {
                                this.close_context_menu(cx);
                                this.select_related(&r, c, cx);
                            })));
                        }
                        let (v, r9) = (vs.clone(), rs.clone());
                        sm.separator().item(OsMenuItem::new("Create selection…").on_click(move |_, _, cx| v.update(cx, |this, cx| {
                            this.close_context_menu(cx);
                            if !this.selection.contains(&r9) {
                                this.selection.push(r9.clone());
                            }
                            this.ui.create_selection = true;
                            this.refresh_scene(false, cx);
                            cx.notify();
                        })))
                    });
                }
                if !matches!(k, PickKind::Origin | PickKind::Connector) {
                    m = m.item(item("Select other…", Box::new(|this, _, cx| this.select_other(false, cx))));
                }
                m
            };
            let zoom_fit = || item("Zoom to fit", Box::new(|this, _, cx| this.zoom_fit(cx)));
            let zoom_sel = |r: &Reference| {
                let r = r.clone();
                item("Zoom to selection", Box::new(move |this, _, cx| {
                    if !this.selection.contains(&r) {
                        this.selection.push(r.clone());
                    }
                    this.zoom_to_selection(cx);
                }))
            };
            let normal_to = |r: &Reference| {
                let r = r.clone();
                item("View normal to", Box::new(move |this, _, cx| {
                    let prev = std::mem::replace(&mut this.selection, vec![r.clone()]);
                    this.view_normal(cx);
                    this.selection = prev;
                }))
            };

            match (target_ref.clone(), kind) {
                // A dialog is open: only what doesn't start another command.
                (Some(r), Some(k)) if editing || in_sketch => {
                    m = select_menu(m, &r, k, window, cx).separator().item(zoom_fit()).item(zoom_sel(&r));
                }
                // Default and feature planes.
                (Some(r), Some(PickKind::Plane)) => {
                    if let (Some(id), Some(name)) = (creator, creator_name.clone()) {
                        m = m.item(item(&format!("Edit {name}…"), Box::new(move |this, w, cx| this.edit_feature(id, w, cx)))).separator();
                    }
                    m = m.item(tool("New sketch…", "sketch", "sketch", &r)).item(tool("Offset plane…", "plane", "cPlane", &r));
                    if paste_sketch {
                        let rp = r.clone();
                        m = m.item(item("Paste sketch", Box::new(move |this, _, cx| this.paste_sketch_onto(rp.clone(), cx))));
                    }
                    let (rh, ro, rs) = (r.clone(), r.clone(), r.clone());
                    m = m
                        .separator()
                        .item(item(&format!("Hide {plane_name}"), Box::new(move |this, _, cx| {
                            this.set_visibility(std::slice::from_ref(&rh), true, cx);
                            this.hover = None;
                        })))
                        .item(item("Hide other planes", Box::new(move |this, _, cx| this.hide_planes(Some(&ro), cx))))
                        .item(item("Hide all planes", Box::new(|this, _, cx| this.hide_planes(None, cx))))
                        .item(icon_item("Section view…", "menu-section-view", Box::new(move |this, w, cx| this.section_through(rs.clone(), w, cx))))
                        .separator();
                    m = select_menu(m, &r, PickKind::Plane, window, cx).separator().item(zoom_fit()).item(zoom_sel(&r)).item(normal_to(&r));
                }
                // Sketch regions, curves and points (live: region and curve menus).
                (Some(r), Some(k @ (PickKind::SketchRegion | PickKind::SketchCurve | PickKind::SketchPoint))) => {
                    if let (Some(id), Some(name)) = (creator, creator_name.clone()) {
                        m = m
                            .item(icon_item(&format!("Edit {name}…"), "sketch", Box::new(move |this, w, cx| this.edit_feature(id, w, cx))))
                            .item(item("Show dimensions", Box::new(move |this, w, cx| this.show_dimensions(id, w, cx))))
                            .item(item(&format!("Show dependencies of {name}…"), Box::new(move |this, _, cx| {
                                this.ui.dependencies = Some(id);
                                cx.notify();
                            })))
                            .separator();
                    }
                    if k == PickKind::SketchRegion {
                        m = m.item(tool("Extrude…", "extrude", "extrude", &r)).item(tool("Revolve…", "revolve", "revolve", &r)).separator();
                    }
                    let rh = r.clone();
                    m = m.item(icon_item("Hide", "eye-off", Box::new(move |this, _, cx| {
                        this.set_visibility(std::slice::from_ref(&rh), true, cx);
                        this.hover = None;
                    })));
                    if k == PickKind::SketchCurve {
                        let rc = r.clone();
                        m = m.item(item("Curve/surface analysis…", Box::new(move |this, w, cx| {
                            this.selection = vec![rc.clone()];
                            this.ui.analysis_tool = None;
                            this.toggle_analysis_tool(super::analysis_tools::ToolKind::CurveSurface, w, cx);
                        })));
                    }
                    if k == PickKind::SketchRegion {
                        let rs = r.clone();
                        m = m.item(icon_item("Section view…", "menu-section-view", Box::new(move |this, w, cx| this.section_through(rs.clone(), w, cx))));
                        if let Some(id) = creator {
                            m = m.separator().item(item("Export as DXF/DWG…", Box::new(move |this, w, cx| this.export_sketch(id, "dxf", w, cx))));
                        }
                    }
                    m = select_menu(m.separator(), &r, k, window, cx).separator().item(zoom_fit()).item(zoom_sel(&r));
                    if k == PickKind::SketchRegion {
                        m = m.item(normal_to(&r));
                    }
                    if let (Some(id), Some(name)) = (creator, creator_name.clone()) {
                        m = m
                            .separator()
                            .item(icon_item(&format!("Delete {name}"), "menu-delete", Box::new(move |this, _, cx| this.delete_features(vec![id], cx))))
                            .separator()
                            .item(item("Edit sketch appearance…", Box::new(move |this, w, cx| this.edit_appearance(super::appearance::EditTarget::Feature(id, None), w, cx))));
                    }
                }
                // Part faces, edges and vertices.
                (Some(r), Some(k)) => {
                    if let (Some(id), Some(name)) = (creator, creator_name.clone()) {
                        m = m.item(item(&format!("Edit {name}…"), Box::new(move |this, w, cx| this.edit_feature(id, w, cx))));
                        if let Some((sid, sname)) = source_sketch.clone() {
                            m = m
                                .item(item(&format!("Edit {sname}…"), Box::new(move |this, w, cx| this.edit_feature(sid, w, cx))))
                                .item(item("Show dimensions", Box::new(move |this, w, cx| this.show_dimensions(sid, w, cx))));
                        }
                        m = m
                            .item(item(&format!("Show dependencies of {name}…"), Box::new(move |this, _, cx| {
                                this.ui.dependencies = Some(id);
                                cx.notify();
                            })))
                            .separator();
                    }
                    // Creation tools for this entity.
                    if k == PickKind::Face && planar && faces.len() <= 1 {
                        m = m
                            .item(tool("New sketch…", "sketch", "sketch", &r))
                            .item(tool("Offset plane…", "plane", "cPlane", &r))
                            .item(tool("Extrude…", "extrude", "extrude", &r))
                            .item(tool("Revolve…", "revolve", "revolve", &r));
                        if paste_sketch {
                            let rp = r.clone();
                            m = m.item(item("Paste sketch", Box::new(move |this, _, cx| this.paste_sketch_onto(rp.clone(), cx))));
                        }
                        m = m.separator();
                    } else if k == PickKind::Edge {
                        m = m.item(tool("Fillet…", "fillet", "fillet", &r)).item(tool("Chamfer…", "chamfer", "chamfer", &r)).separator();
                    }
                    if let Some(b) = body.clone() {
                        let (b1, b2, b3, b4) = (b.clone(), b.clone(), b.clone(), b.clone());
                        m = m
                            .item(icon_item(&format!("Hide {body_name}"), "eye-off", Box::new(move |this, _, cx| {
                                this.set_part_hidden(&b1, true, cx);
                                this.hover = None;
                            })));
                        if k == PickKind::Edge {
                            let re = r.clone();
                            m = m.item(item("Curve/surface analysis…", Box::new(move |this, w, cx| {
                                this.selection = vec![re.clone()];
                                this.ui.analysis_tool = None;
                                this.toggle_analysis_tool(super::analysis_tools::ToolKind::CurveSurface, w, cx);
                            })));
                        }
                        m = m
                            .item(item("Isolate…", Box::new(move |this, w, cx| {
                                this.selection = vec![Reference::Body(b2.clone())];
                                this.hover = None;
                                this.isolate(super::behaviors::IsolateMode::Isolate, w, cx);
                            })))
                            .item(item("Make transparent…", Box::new(move |this, w, cx| {
                                this.selection = vec![Reference::Body(b3.clone())];
                                this.hover = None;
                                this.isolate(super::behaviors::IsolateMode::Transparent, w, cx);
                            })));
                        let rs = r.clone();
                        m = m.item(icon_item("Section view…", "menu-section-view", Box::new(move |this, w, cx| {
                            if this.frame_of_ref(&rs).is_some() {
                                this.section_through(rs.clone(), w, cx);
                            } else {
                                this.section_part(&b4, w, cx);
                            }
                        })));
                        // Copy the part (paste derives it in another Part Studio).
                        let bc = b.clone();
                        m = m.separator().item(icon_item(&format!("Copy {body_name}"), "menu-copy", Box::new(move |this, _, cx| this.copy_part(bc.clone(), cx))));
                        // Drawings and export.
                        let (b7, b8, name) = (b.clone(), b.clone(), body_name.to_string());
                        m = m
                            .separator()
                            .item(icon_item(&format!("Create Drawing of {body_name}…"), "menu-create-drawing", Box::new(move |this, w, cx| {
                                let source = Some(noshape_model::drawing::create::DrawingSource { tab: this.tab, part: Some(b7.clone()), config: Default::default() });
                                crate::drawing_create::request_create_drawing(crate::drawing_create::CreateRequest { source, name: Some(name.clone()) }, w, cx);
                            })));
                        if k == PickKind::Face && planar {
                            let rf = r.clone();
                            m = m.item(item("Export as DXF/DWG…", Box::new(move |this, w, cx| this.export_face(rf.clone(), w, cx))));
                        }
                        if sheet_metal {
                            let (b5, b6) = (b.clone(), b.clone());
                            m = m
                                .item(item("Export flat pattern (DXF)…", Box::new(move |this, w, cx| this.export_flat(b5.clone(), "dxf", w, cx))))
                                .item(item("Export flat pattern (SVG)…", Box::new(move |this, w, cx| this.export_flat(b6.clone(), "svg", w, cx))));
                        }
                        m = m.item(item("Export…", Box::new(move |this, w, cx| this.open_export_dialog(vec![b8.clone()], w, cx))));
                    }
                    m = m.separator();
                    if has_selection {
                        m = m.item(item("Clear selection", Box::new(|this, _, cx| {
                            this.selection.clear();
                            this.selected_features.clear();
                            this.refresh_scene(false, cx);
                            cx.notify();
                        })));
                    }
                    m = select_menu(m, &r, k, window, cx).separator().item(zoom_fit()).item(zoom_sel(&r));
                    if planar {
                        m = m.item(normal_to(&r));
                    } else if linear_edge {
                        let ra = r.clone();
                        m = m.item(item("Align view with vertical axis", Box::new(move |this, _, cx| this.align_view_vertical(&ra, cx))));
                    }
                    if let Some(b) = body.clone() {
                        let bd = b.clone();
                        m = m.separator().item(icon_item(&format!("Delete {body_name}…"), "menu-delete", Box::new(move |this, w, cx| {
                            this.selection = vec![Reference::Body(bd.clone())];
                            this.delete_selected_parts(w, cx);
                        })));
                        if let (Some(id), Some(name)) = (creator, creator_name.clone()) {
                            m = m.item(icon_item(&format!("Delete {name}"), "menu-delete", Box::new(move |this, _, cx| this.delete_features(vec![id], cx))));
                        }
                        let (b1, b2) = (b.clone(), b.clone());
                        m = m
                            .separator()
                            .item(item(&format!("Assign material for {body_name}…"), Box::new(move |this, w, cx| this.open_part_properties(b1.clone(), w, cx))))
                            .item(item(&format!("Edit appearance for {body_name}…"), Box::new(move |this, w, cx| this.edit_appearance(super::appearance::EditTarget::Parts(vec![b2.clone()]), w, cx))));
                        if k == PickKind::Face {
                            let fs = faces.clone();
                            let label = if fs.len() > 1 { format!("Add appearance to {} faces…", fs.len()) } else { "Add appearance to face…".to_string() };
                            m = m.item(icon_item(&label, "appearance", Box::new(move |this, w, cx| this.edit_appearance(super::appearance::EditTarget::Faces(fs.clone(), None), w, cx))));
                        }
                    }
                }
                // Empty space.
                _ => {
                    m = m.item(item("Show all", Box::new(|this, _, cx| this.show_everything(cx)))).separator();
                    // Select ▸ everything of a kind (the live capture didn't open this submenu).
                    let vs = v.clone();
                    m = m.submenu("Select", window, cx, move |mut sm, _, _| {
                        for (label, kind) in [("All parts", 0u8), ("All surfaces", 1), ("All sketches", 2), ("All planes", 3), ("All mate connectors", 4)] {
                            let v = vs.clone();
                            sm = sm.item(OsMenuItem::new(label).on_click(move |_, _, cx| v.update(cx, |this, cx| {
                                this.close_context_menu(cx);
                                this.select_all_of(kind, cx);
                            })));
                        }
                        sm
                    });
                    if has_selection {
                        m = m.separator().item(item("Clear selection", Box::new(|this, _, cx| {
                            this.selection.clear();
                            this.selected_features.clear();
                            this.refresh_scene(false, cx);
                            cx.notify();
                        })));
                    }
                    m = m.separator().item(zoom_fit()).item(item("Isometric", Box::new(|this, _, cx| this.view(StandardView::Isometric, cx))));
                }
            }
            // Modes that are on: a way out of each.
            let mut exits: Vec<OsMenuItem> = Vec::new();
            if let Some(tool) = editing_tool {
                exits.push(item(&format!("Exit {tool}"), Box::new(|this, w, cx| this.cancel_edit(w, cx))));
            }
            if let Some(mode) = isolate {
                let label = match mode {
                    super::behaviors::IsolateMode::Isolate => "Exit isolate",
                    super::behaviors::IsolateMode::Transparent => "Exit make transparent",
                };
                exits.push(item(label, Box::new(|this, w, cx| {
                    this.exit_isolate(w, cx);
                })));
            }
            if section && target_ref.is_none() {
                exits.push(item("Edit section view…", Box::new(|this, w, cx| this.open_section_dialog(w, cx))));
            }
            if section {
                exits.push(item("Turn off section view", Box::new(|this, w, cx| this.toggle_section(w, cx))));
            }
            if !exits.is_empty() {
                m = m.separator();
                for e in exits {
                    m = m.item(e);
                }
            }
            m
        });
        let sub = cx.subscribe(&menu, |this: &mut Self, _, _: &DismissEvent, cx| this.close_context_menu(cx));
        menu.update(cx, |m, cx| window.focus(&m.focus_handle(cx), cx));
        self.context_menu = Some((position, menu, sub));
        // Onshape keeps the right-clicked entity highlighted while its menu is open.
        self.ui.menu_target = target_ref_for_highlight;
        self.refresh_scene(false, cx);
        cx.notify();
    }

    pub(super) fn close_context_menu(&mut self, cx: &mut Context<Self>) {
        if self.context_menu.take().is_some() {
            if self.ui.menu_target.take().is_some() {
                self.refresh_scene(false, cx);
            }
            cx.notify();
        }
    }

    pub(super) fn render_context_menu(&self) -> Vec<AnyElement> {
        match &self.context_menu {
            Some((pos, menu, _)) => vec![deferred(anchored().position(*pos).snap_to_window_with_margin(px(8.)).child(menu.clone())).with_priority(2).into_any_element()],
            None => Vec::new(),
        }
    }

    // ------------------------------------------------------------ parts

    fn edit_part_props(&mut self, body: &BodyId, desc: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut PartProperties)) {
        let body = body.clone();
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, desc, cx, |ps| f(ps.part_properties.entry(body).or_default()));
        });
    }

    pub(super) fn set_part_hidden(&mut self, body: &BodyId, hidden: bool, cx: &mut Context<Self>) {
        self.edit_part_props(body, if hidden { "Hide part" } else { "Show part" }, cx, |p| p.hidden = hidden);
    }

    pub(super) fn show_all_parts(&mut self, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Show all parts", cx, |ps| {
                for p in ps.part_properties.values_mut() {
                    p.hidden = false;
                }
            });
        });
    }

    pub(super) fn open_part_properties(&mut self, body: BodyId, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.display.body(&body).map(|b| b.name.clone()).unwrap_or_default();
        let name = cx.new(|cx| InputState::new(window, cx).default_value(current));
        let view = cx.entity();
        let props = self.studio(cx).and_then(|s| s.part_properties.get(&body).cloned()).unwrap_or_default();
        // Part number / revision (parts list properties) and mass override.
        let part_no = cx.new(|cx| InputState::new(window, cx).default_value(props.part_number.clone().unwrap_or_default()));
        let revision = cx.new(|cx| InputState::new(window, cx).default_value(props.revision.clone().unwrap_or_default()));
        let mass = cx.new(|cx| InputState::new(window, cx).placeholder("computed from material").default_value(props.mass_override.map(|m| format!("{m}")).unwrap_or_default()));
        let pal = p();
        window.open_dialog(cx, move |dialog, _, _| {
            let v = view.clone();
            let b = body.clone();
            let name2 = name.clone();
            let (pn2, rv2, ms2) = (part_no.clone(), revision.clone(), mass.clone());
            let swatches = SWATCHES.iter().map(|hex| {
                let v = view.clone();
                let b = body.clone();
                let hex = *hex;
                div()
                    .id(SharedString::from(format!("sw-{hex:x}")))
                    .size(px(22.))
                    .rounded_md()
                    .bg(rgb(hex))
                    .border_1()
                    .border_color(pal.border_strong)
                    .cursor_pointer()
                    .on_click(move |_, _, cx| {
                        v.update(cx, |this, cx| this.edit_part_props(&b, "Change appearance", cx, |p| p.appearance = Some(Appearance { color: crate::theme::lin(hex) })));
                    })
            });
            let materials = MATERIALS.iter().map(|(m, rho)| {
                let v = view.clone();
                let b = body.clone();
                let selected = props.material.as_deref() == Some(*m);
                let m2 = m.to_string();
                let rho = *rho;
                Button::new(SharedString::from(format!("mat-{m}")))
                    .label(*m)
                    .xsmall()
                    .when_selected(selected)
                    .on_click(move |_, _, cx| {
                        let m3 = m2.clone();
                        v.update(cx, |this, cx| this.edit_part_props(&b, "Assign material", cx, |p| {
                            p.material = Some(m3);
                            p.density = Some(rho);
                        }));
                    })
            });
            dialog
                .title("Part properties")
                .w(px(460.))
                .child(
                    v_flex()
                        .gap_3()
                        .child(v_flex().gap_1().child(div().text_xs().text_color(pal.text_muted).child("Name")).child(Input::new(&name)))
                        .child(
                            h_flex()
                                .gap_2()
                                .child(v_flex().flex_1().gap_1().child(div().text_xs().text_color(pal.text_muted).child("Part number")).child(Input::new(&part_no)))
                                .child(v_flex().flex_1().gap_1().child(div().text_xs().text_color(pal.text_muted).child("Revision")).child(Input::new(&revision)))
                                .child(v_flex().flex_1().gap_1().child(div().text_xs().text_color(pal.text_muted).child("Mass override (kg)")).child(Input::new(&mass))),
                        )
                        .child(v_flex().gap_1().child(div().text_xs().text_color(pal.text_muted).child("Appearance")).child(h_flex().gap_1p5().flex_wrap().children(swatches)))
                        .child(v_flex().gap_1().child(div().text_xs().text_color(pal.text_muted).child("Material")).child(h_flex().gap_1().flex_wrap().children(materials))),
                )
                .footer(
                    DialogFooter::new()
                        .gap_2()
                        .child(DialogClose::new().child(Button::new("cancel").label("Close").outline()))
                        .child(DialogAction::new().child(Button::new("ok").label("Apply").primary())),
                )
                .on_ok(move |_, _, cx| {
                    let n = name2.read(cx).value().to_string();
                    let b2 = b.clone();
                    let pn = pn2.read(cx).value().trim().to_string();
                    let rv = rv2.read(cx).value().trim().to_string();
                    let ms = ms2.read(cx).value().trim().parse::<f64>().ok().filter(|m| *m > 0.0);
                    v.update(cx, |this, cx| {
                        this.edit_part_props(&b2, "Part properties", cx, |p| {
                            p.name = Some(n.clone()).filter(|s| !s.trim().is_empty());
                            p.part_number = Some(pn.clone()).filter(|s| !s.is_empty());
                            p.revision = Some(rv.clone()).filter(|s| !s.is_empty());
                            p.mass_override = ms;
                        })
                    });
                    true
                })
        });
    }

    /// Onshape "Export…" on parts: choose a format, then where to save.
    pub(super) fn open_export_dialog(&mut self, bodies: Vec<BodyId>, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity();
        let base = match bodies.as_slice() {
            [b] => self.display.body(b).map(|d| d.name.clone()).unwrap_or_else(|| "Part".into()),
            _ => self.studio_name(cx),
        };
        let file = cx.new(|cx| gpui_kit::component::input::InputState::new(window, cx).default_value(base));
        let chosen = std::rc::Rc::new(std::cell::Cell::new(0usize));
        // Onshape's Export modal (shared with the tab menu's Export…).
        window.open_dialog(cx, move |dialog, _, cx| {
            let formats: Vec<String> = super::ExportFormat::ALL.iter().map(|f| f.label().to_string()).collect();
            let (v, c, bodies, file2) = (view.clone(), chosen.clone(), bodies.clone(), file.clone());
            crate::os_ui::export_modal(dialog, "Export", &file, formats, chosen.clone(), cx).on_ok(move |_, window, cx| {
                let format = super::ExportFormat::ALL[c.get()];
                let base = file2.read(cx).value().trim().to_string();
                let (v, bodies) = (v.clone(), bodies.clone());
                window.defer(cx, move |window, cx| {
                    crate::files::request_save_path("Export", format!("{base}.{}", format.ext()), format.ext(), window, cx, move |path, cx| {
                        v.update(cx, |this, cx| {
                            let msg = match this.export_bodies(format, &path, &bodies) {
                                Ok(()) => format!("Exported {}", path.display()),
                                Err(e) => format!("Export failed: {e:#}"),
                            };
                            this.flash_info(msg, cx);
                        });
                    });
                });
                true
            })
        });
    }

    pub(super) fn export_flat(&mut self, body: BodyId, ext: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ctx) = self.context.clone() else { return };
        let geometry = match noshape_model::sheetmetal::flat_pattern_for(&ctx, &body) {
            Ok(f) => f.to_sheet_geometry(),
            Err(e) => {
                self.message = Some((noshape_model::display::FeatureStatus::Error, e));
                cx.notify();
                return;
            }
        };
        let name = self.display.body(&body).map(|b| b.name.clone()).unwrap_or_else(|| "flat".into());
        let view = cx.entity();
        crate::files::request_save_path("Export flat pattern", format!("{name} flat.{ext}"), ext, window, cx, move |path, cx| {
            let bytes = if ext == "dxf" { noshape_model::drawing::export::to_dxf(&geometry).into_bytes() } else { noshape_model::drawing::export::to_svg(&geometry).into_bytes() };
            let msg = match std::fs::write(&path, bytes) {
                Ok(()) => (noshape_model::display::FeatureStatus::Info, format!("Exported {}", path.display())),
                Err(e) => (noshape_model::display::FeatureStatus::Error, format!("Export failed: {e}")),
            };
            view.update(cx, |this, cx| {
                this.message = Some(msg);
                cx.notify();
            });
        });
    }

    // ------------------------------------------------------------ measurement

    fn fmt(&self, q: Quantity, cx: &App) -> String {
        let units = self.doc.read(cx).session.doc.settings.units;
        noshape_units::format_quantity(q, &units)
    }

    /// Model evaluation tools at the bottom right (Onshape): Measure and
    /// Mass properties.
    pub(super) fn render_evaluation_tools(&mut self, cx: &mut Context<Self>) -> AnyElement {
        // Onshape (live): a 110x30 area flush with the viewport's bottom-right
        // corner, white at 75%, holding three 30x30 buttons 5 px apart:
        // Measure, Analysis tools and Mass properties (20 px icons).
        let pal = p();
        let measure_on = self.ui.measure_pinned;
        let mass_on = self.show_mass;
        let btn = |id: &'static str, icon: &'static str, on: bool, cx: &App| crate::ui::icon_button(id, icon, 30., on, cx);
        let view = cx.entity();
        let thickness_on = self.ui.thickness.is_some();
        h_flex()
            .id("eval-tools")
            .occlude()
            .absolute()
            .bottom_0()
            .right_0()
            .h(px(30.))
            .pl(px(5.))
            .pr(px(5.))
            .gap(px(5.))
            .bg(with_alpha(pal.panel, 0.75))
            .child(
                btn("eval-measure", "measure", measure_on, cx).tooltip("Show measure details ([)").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.ui.measure_pinned = !this.ui.measure_pinned;
                    this.show_mass = false;
                    cx.notify();
                })),
            )
            // Onshape's "Show analysis tools" menu (Help: Analysis tools), in its order.
            .child(btn("eval-analysis", "analysis-tools", self.ui.analysis.is_some() || self.ui.analysis_tool.is_some() || self.ui.zebra.is_some() || self.ui.reflection.is_some() || self.ui.thickness.is_some(), cx).tooltip("Show analysis tools").dropdown_menu(move |m, _, _| {
                let (v1, v2, v3, v4, v5, v6, v7, v8, v9, v10) = (view.clone(), view.clone(), view.clone(), view.clone(), view.clone(), view.clone(), view.clone(), view.clone(), view.clone(), view.clone());
                let v11 = view.clone();
                m.item(PopupMenuItem::new("Curve/surface analysis").on_click(move |_, w, cx| v1.update(cx, |this, cx| this.toggle_analysis_tool(super::analysis_tools::ToolKind::CurveSurface, w, cx))))
                    .item(PopupMenuItem::new("Deviation analysis").on_click(move |_, w, cx| v5.update(cx, |this, cx| this.toggle_analysis_tool(super::analysis_tools::ToolKind::Deviation, w, cx))))
                    .item(PopupMenuItem::new("Connection analysis").on_click(move |_, w, cx| v6.update(cx, |this, cx| this.toggle_analysis_tool(super::analysis_tools::ToolKind::Connection, w, cx))))
                    .item(PopupMenuItem::new("Dihedral analysis").on_click(move |_, w, cx| v2.update(cx, |this, cx| this.toggle_analysis_tool(super::analysis_tools::ToolKind::Dihedral, w, cx))))
                    .item(PopupMenuItem::new("Zebra stripes").on_click(move |_, w, cx| v7.update(cx, |this, cx| this.toggle_analysis_tool(super::analysis_tools::ToolKind::Zebra, w, cx))))
                    .item(PopupMenuItem::new("Reflection analysis").on_click(move |_, w, cx| v8.update(cx, |this, cx| this.toggle_analysis_tool(super::analysis_tools::ToolKind::Reflection, w, cx))))
                    .item(PopupMenuItem::new("Curvature color map").on_click(move |_, w, cx| v3.update(cx, |this, cx| this.toggle_analysis(super::analysis::Analysis::Curvature, w, cx))))
                    .item(PopupMenuItem::new("Draft analysis").on_click(move |_, w, cx| {
                        v4.update(cx, |this, cx| {
                            let a = this.draft_analysis();
                            this.toggle_analysis(a, w, cx)
                        })
                    }))
                    // Thickness analysis: once on, the menu edits or turns it off.
                    .when(!thickness_on, |m| m.item(PopupMenuItem::new("Thickness analysis").on_click(move |_, w, cx| v9.update(cx, |this, cx| this.toggle_analysis_tool(super::analysis_tools::ToolKind::Thickness, w, cx)))))
                    .when(thickness_on, |m| {
                        m.item(PopupMenuItem::new("Edit thickness analysis").on_click(move |_, w, cx| v11.update(cx, |this, cx| this.toggle_analysis_tool(super::analysis_tools::ToolKind::Thickness, w, cx))))
                            .item(PopupMenuItem::new("Turn thickness analysis off").on_click(move |_, _, cx| v10.update(cx, |this, cx| this.thickness_off(cx))))
                    })
            }))
            .child(
                btn("eval-mass", "mass-properties", mass_on, cx).tooltip("Mass properties").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.show_mass = !this.show_mass;
                    this.ui.measure_pinned = false;
                    // Onshape: a yellow warning under the toolbar when a part
                    // has no material (live part-studio/80).
                    if this.show_mass {
                        let props = this.studio(cx).map(|s| s.part_properties.clone()).unwrap_or_default();
                        if this.display.bodies.iter().any(|b| props.get(&b.id).and_then(|p| p.density).is_none()) {
                            this.message = Some((noshape_model::display::FeatureStatus::Warning, "One or more parts do not have a material defined.".into()));
                        }
                    }
                    cx.notify();
                })),
            )
            .into_any_element()
    }

    pub(super) fn render_measure_panel(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let pal = p();
        if self.edit.is_some() || self.context.is_none() {
            return Vec::new();
        }
        let ctx = self.context.clone().unwrap();
        let mut rows: Vec<(String, String)> = Vec::new();
        let mut title = "Measure";
        // Part tab of Mass and section properties, laid out like Onshape's dialog.
        let mut mass_view: Option<MassView> = None;
        if self.show_mass && self.ui.mass_faces {
            // Face tab: area, perimeter and centroid of the selected faces.
            title = "Mass properties (faces)";
            let faces: Vec<Reference> = self.selection.iter().filter(|r| matches!(r, Reference::Face(_))).cloned().collect();
            let (mut area, mut perimeter, mut moment) = (0.0f64, 0.0f64, glam::DVec3::ZERO);
            for f in &faces {
                for sh in measure::resolve_shapes(&ctx, f) {
                    let a = sh.area().unwrap_or(0.0);
                    area += a;
                    moment += sh.centroid().unwrap_or_default() * a;
                    perimeter += sh.edges().iter().map(|e| e.length().unwrap_or(0.0)).sum::<f64>();
                }
            }
            if faces.is_empty() {
                rows.push(("Select faces".into(), String::new()));
            } else {
                let c = if area > 0.0 { moment / area } else { glam::DVec3::ZERO };
                let l = |x: f64| self.fmt(Quantity::new(x, Dim::LENGTH), cx);
                rows.push(("Faces".into(), faces.len().to_string()));
                rows.push(("Surface area".into(), self.fmt(Quantity::new(area, Dim { length: 2, angle: 0 }), cx)));
                rows.push(("Perimeter".into(), l(perimeter)));
                rows.push(("Centroid".into(), format!("{}, {}, {}", l(c.x), l(c.y), l(c.z))));
            }
        } else if self.show_mass {
            title = "Mass properties";
            let bodies: Vec<BodyId> = self.selection.iter().filter_map(|r| self.owning_body(r)).collect();
            let props = self.studio(cx).map(|s| s.part_properties.clone()).unwrap_or_default();
            // Mass override (Onshape): replaces the computed mass when every
            // part in the computation has one.
            let all: Vec<BodyId> = if bodies.is_empty() { self.display.bodies.iter().map(|b| b.id.clone()).collect() } else { bodies.clone() };
            let overrides: Option<f64> = all.iter().map(|b| props.get(b).and_then(|p| p.mass_override)).sum();
            // Inertia about a selected mate connector (reference coordinate system).
            let reference = self.selection.iter().find_map(|r| self.display.connectors.iter().find(|c| &c.reference == r)).map(|c| (c.origin, c.x, c.z.cross(c.x), c.z, c.name.clone()));
            if let Some(mut mp) = measure::mass_properties(&ctx, &bodies, &|b| props.get(b).and_then(|p| p.density)) {
                if let Some(m) = overrides {
                    if let Some(old) = mp.mass.filter(|o| *o > 0.0) {
                        mp.inertia *= m / old;
                    }
                    mp.mass = Some(m);
                }
                if let (Some((o, x, y, z, name)), Some(m)) = (&reference, mp.mass) {
                    // Parallel axis to the connector origin, then its axes.
                    let d = mp.center_of_mass - *o;
                    let shift = glam::DMat3::from_diagonal(glam::DVec3::splat(d.length_squared())) - glam::DMat3::from_cols(d * d.x, d * d.y, d * d.z);
                    let r = glam::DMat3::from_cols(*x, *y, *z);
                    mp.inertia = r.transpose() * (mp.inertia + shift * m) * r;
                    rows.push(("Reference".into(), name.clone()));
                }
                let len3 = Dim { length: 3, angle: 0 };
                let len2 = Dim { length: 2, angle: 0 };
                let c = mp.center_of_mass;
                let l = |x: f64| self.fmt(Quantity::new(x, Dim::LENGTH), cx);
                // Onshape lists the moments of inertia about the center of mass
                // (needs a material, like the mass).
                let units = self.doc.read(cx).session.doc.settings.units;
                let f = units.si_factor(Dim::LENGTH).powi(2);
                let sym = units.length.symbol();
                let i = mp.inertia;
                let v = |x: f64| format!("{:.4}", x / f);
                mass_view = Some(MassView {
                    volume: self.fmt(Quantity::new(mp.volume, len3), cx),
                    area: self.fmt(Quantity::new(mp.surface_area, len2), cx),
                    mass: mp.mass.map(|m| format!("{m:.4} kg")),
                    // Without a material Onshape leaves the center of mass empty.
                    center: if mp.mass.is_some() { [l(c.x), l(c.y), l(c.z)] } else { Default::default() },
                    inertia_unit: format!("{sym}² kg"),
                    inertia: mp.mass.map(|_| [[v(i.x_axis.x), v(i.y_axis.x), v(i.z_axis.x)], [v(i.x_axis.y), v(i.y_axis.y), v(i.z_axis.y)], [v(i.x_axis.z), v(i.y_axis.z), v(i.z_axis.z)]]),
                    parts: if bodies.is_empty() { Vec::new() } else { bodies.iter().filter_map(|b| self.display.body(b).map(|d| d.name.clone())).collect() },
                    reference: reference.as_ref().map(|r| r.4.clone()),
                });
            }
        } else {
            if self.selection.is_empty() && !self.ui.measure_pinned {
                return Vec::new();
            }
            let sel = self.selection.clone();
            let results = match &self.measure_cache {
                Some((s, r)) if *s == sel => r.clone(),
                _ => {
                    let r = measure::measure(&ctx, &sel);
                    self.measure_cache = Some((sel, r.clone()));
                    r
                }
            };
            let filter = self.ui.measure_type;
            for (label, m) in results {
                // Measure type selector (Onshape): show only one kind.
                if let Some(f) = filter
                    && !measurement_kind(&m).eq_ignore_ascii_case(f)
                {
                    continue;
                }
                let v = match m {
                    Measurement::Length(x) | Measurement::Radius(x) | Measurement::Diameter(x) => self.fmt(Quantity::new(x, Dim::LENGTH), cx),
                    Measurement::Area(x) => self.fmt(Quantity::new(x, Dim { length: 2, angle: 0 }), cx),
                    Measurement::Volume(x) => self.fmt(Quantity::new(x, Dim { length: 3, angle: 0 }), cx),
                    Measurement::Angle(x) => self.fmt(Quantity::new(x, Dim::ANGLE), cx),
                    Measurement::Point(p) => {
                        let l = |x: f64| self.fmt(Quantity::new(x, Dim::LENGTH), cx);
                        format!("{}, {}, {}", l(p.x), l(p.y), l(p.z))
                    }
                    Measurement::Distance { distance, dx, dy, dz, .. } => {
                        let l = |x: f64| self.fmt(Quantity::new(x, Dim::LENGTH), cx);
                        rows.push(("ΔX / ΔY / ΔZ".into(), format!("{} / {} / {}", l(dx), l(dy), l(dz))));
                        l(distance)
                    }
                };
                rows.push((label, v));
            }
            if rows.is_empty() {
                if !self.ui.measure_pinned {
                    return Vec::new();
                }
                // Measure opened with `[`: prompt for a selection.
                rows.push(("Select entities to measure".into(), String::new()));
            }
        }
        let current_type = self.ui.measure_type.unwrap_or("Show all");
        let view = cx.entity();
        let units = self.doc.read(cx).session.doc.settings.units;
        let close = move |id: &'static str, cx: &mut Context<Self>| {
            div()
                .id(id)
                .size(px(28.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|d| d.bg(pal.panel_hover))
                .child(crate::assets::icon("dialog-cancel", 18., cx))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.show_mass = false;
                    this.ui.measure_pinned = false;
                    this.selection.clear();
                    this.refresh_scene(false, cx);
                    cx.notify();
                }))
        };
        let help = || div().size(px(14.)).rounded_full().bg(gpui_kit::rgb(0xaaaaaa)).flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb(0xffffff)).child("?");
        // Onshape's selection box: `#def1fb` with a 1 px `#1651b0` border when
        // active, the caption small on top once something is listed.
        // `invalid`: entries in red (live: parts without a material).
        let field = |label: &str, items: Vec<String>, active: bool, invalid: bool| {
            let empty = items.is_empty();
            v_flex()
                .mx(px(3.))
                .my(px(2.))
                .rounded(px(2.))
                .border_1()
                .border_color(if active { pal.accent } else { pal.border })
                .when(active, |d| d.bg(pal.row_hover))
                .child(div().px(px(6.)).when(empty, |d| d.py(px(5.)).text_size(px(12.))).when(!empty, |d| d.pt(px(2.)).text_size(px(10.))).text_color(gpui_kit::rgb(0x555555)).child(label.to_string()))
                .children(items.into_iter().map(|t| h_flex().h(px(24.)).px(px(6.)).text_size(px(13.33)).text_color(if invalid { pal.text_invalid } else { gpui_kit::rgb(0x555555) }).child(div().flex_1().truncate().child(t)).child(div().text_color(pal.text_muted).child("×"))))
        };
        // Label and right-aligned read-only value on an underline (12 px `#666` label).
        let value_row = |label: SharedString, value: String| {
            h_flex()
                .h(px(28.))
                .child(div().pl(px(5.)).pr(px(5.)).text_size(px(12.)).text_color(pal.text_muted).child(label))
                .child(div().flex_1().mr(px(5.)).h(px(24.)).flex().items_center().justify_end().border_b_1().border_color(pal.border).text_size(px(13.33)).text_color(pal.text).child(value))
        };
        let selected_names: Vec<String> = {
            let label = self.labeler(cx);
            self.selection.iter().map(|r| label(r)).collect()
        };
        if self.show_mass {
            // Onshape (live, dom/dialog-mass-properties.txt): a 440 px dialog in the
            // feature-dialog place; tabs Part | Face; Parts to measure; Mate
            // connector for reference frame; Show calculation variance; Mass,
            // Volume, Surface area, Center of mass, Mass moments of inertia.
            let tab = |id: &'static str, label: &'static str, faces: bool, cx: &mut Context<Self>| {
                let on = self.ui.mass_faces == faces;
                div()
                    .id(id)
                    .flex_1()
                    .h(px(26.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.))
                    .cursor_pointer()
                    .when(on, |d| d.text_color(pal.accent).font_weight(FontWeight::MEDIUM).border_b_2().border_color(pal.accent))
                    .when(!on, |d| d.text_color(pal.text).border_b_1().border_color(pal.border))
                    .child(label)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.ui.mass_faces = faces;
                        cx.notify();
                    }))
            };
            // Read-only Override (live: a gray `#999` filled box).
            let disabled_check = |id: &'static str| h_flex().id(id).h(px(27.)).pl(px(4.)).gap(px(4.)).text_size(px(13.33)).text_color(pal.text_muted).child(div().size(px(12.)).rounded(px(3.)).bg(gpui_kit::rgb(0x999999))).child("Override");
            let axis = |l: &'static str, c: u32, glyph: &'static str| h_flex().w(px(30.)).pl(px(5.)).gap(px(1.)).text_size(px(12.)).text_color(gpui_kit::rgb(c)).child(l).child(glyph);
            let mut body = v_flex().w_full().child(h_flex().w_full().mb(px(2.)).child(tab("mass-tab-part", "Part", false, cx)).child(tab("mass-tab-face", "Face", true, cx)));
            match mass_view {
                Some(m) => {
                    body = body
                        .child(field(if self.ui.mass_faces { "Faces to measure" } else { "Parts to measure" }, m.parts.clone(), true, m.mass.is_none()))
                        .child(field("Mate connector for reference frame", m.reference.clone().into_iter().collect(), false, false))
                        .child(crate::ui::os_checkbox("mass-variance", "Show calculation variance", false))
                        .child(h_flex().h(px(28.)).child(div().pl(px(5.)).text_size(px(12.)).text_color(pal.text_muted).child("Mass")).child(crate::ui::os_checkbox("mass-override", "Override", false)).child(div().flex_1().mr(px(5.)).h(px(24.)).flex().items_center().justify_end().border_b_1().border_color(pal.border).text_size(px(13.33)).text_color(pal.text).child(m.mass.clone().unwrap_or_default())))
                        .child(value_row("Volume".into(), m.volume.clone()))
                        .child(value_row("Surface area".into(), m.area.clone()))
                        .child(h_flex().h(px(28.)).child(div().pl(px(5.)).text_size(px(12.)).text_color(pal.text_muted).child("Center of mass")).child(disabled_check("com-override")))
                        .children([("X", 0xcd0000u32, "↘"), ("Y", 0x009400, "↗"), ("Z", 0x0000d0, "↑")].into_iter().zip(m.center.iter()).map(|((l, c, g), v)| {
                            h_flex().h(px(26.)).child(axis(l, c, g)).child(div().flex_1().mr(px(5.)).h(px(24.)).flex().items_center().justify_end().border_b_1().border_color(pal.border).text_size(px(13.33)).text_color(pal.text).child(v.clone()))
                        }))
                        .child(h_flex().h(px(28.)).child(div().pl(px(5.)).text_size(px(12.)).text_color(pal.text_muted).child(format!("Mass moments of inertia ({})", m.inertia_unit))).child(disabled_check("moi-override")));
                    let names = [["Lxx", "Lxy", "Lxz"], ["Lyx", "Lyy", "Lyz"], ["Lzx", "Lzy", "Lzz"]];
                    for (r, row_names) in names.iter().enumerate() {
                        let mut row = h_flex().h(px(26.)).px(px(5.)).gap(px(8.));
                        for (c, n) in row_names.iter().enumerate() {
                            let v = m.inertia.as_ref().map(|i| i[r][c].clone()).unwrap_or_default();
                            row = row.child(h_flex().flex_1().gap(px(4.)).child(div().w(px(24.)).text_size(px(12.)).text_color(pal.text_muted).child(*n)).child(div().flex_1().h(px(22.)).flex().items_center().justify_end().border_b_1().border_color(pal.border).text_size(px(13.33)).text_color(pal.text).child(v)));
                        }
                        body = body.child(row);
                    }
                }
                None => {
                    body = body.child(field(if self.ui.mass_faces { "Faces to measure" } else { "Parts to measure" }, selected_names.clone(), true, false));
                    for (k, v) in rows {
                        body = body.child(value_row(k.into(), v));
                    }
                }
            }
            return vec![
                v_flex()
                    .id("mass-dialog")
                    .occlude()
                    .absolute()
                    .top(px(3.))
                    .left(px(0.))
                    .w(px(440.))
                    .bg(pal.dialog_bg)
                    .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                    .text_color(pal.text)
                    .child(
                        h_flex()
                            .h(px(29.))
                            .pl(px(6.))
                            .bg(pal.panel)
                            .border_b_1()
                            .border_color(pal.border)
                            .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Mass and section properties"))
                            .child(div().size(px(28.)).flex().items_center().justify_center().bg(gpui_kit::rgb(0xcce9cc)).child(crate::assets::icon("dialog-ok", 20., cx)))
                            .child(close("mass-close", cx)),
                    )
                    .child(body.px(px(0.)).pb(px(4.)))
                    .child(h_flex().h(px(22.)).px(px(4.)).justify_end().child(help()))
                    .into_any_element(),
            ];
        }
        // Onshape's Measure panel (live, part-studio/82): 220 px, docked at the
        // bottom right; "Measure" + ×; the selection field; Measure type,
        // Length unit, Angle unit; Reference coordinate system; then one row
        // per result with the value in monospace.
        let select_row = |label: &'static str, value: SharedString, menu: Option<AnyElement>| {
            h_flex()
                .h(px(26.))
                .px(px(5.))
                .gap(px(4.))
                .child(div().text_size(px(12.)).text_color(pal.text).child(label))
                .child(match menu {
                    Some(m) => m,
                    None => h_flex().flex_1().h(px(22.)).px(px(4.)).border_b_1().border_color(pal.border).text_size(px(13.33)).child(div().flex_1().child(value)).child(crate::ui::glyph("ui-caret-down", 10., pal.text)).into_any_element(),
                })
        };
        let type_menu = Button::new("measure-type")
            .custom(gpui_kit::component::button::ButtonCustomVariant::new(cx).color(gpui_kit::transparent_white()).foreground(pal.text.into()).hover(pal.row_hover.into()).active(pal.accent_soft.into()).shadow(false))
            .child(h_flex().w_full().child(div().flex_1().text_size(px(13.33)).child(current_type)).child(crate::ui::glyph("ui-caret-down", 10., pal.text)))
            .flex_1()
            .h(px(22.))
            .px(px(4.))
            .rounded(px(0.))
            .border_b_1()
            .border_color(pal.border)
            .dropdown_menu(move |mut m, _, _| {
                for t in ["Show all", "Distance", "Length", "Area", "Angle", "Radius", "Diameter", "Position"] {
                    let v = view.clone();
                    m = m.item(PopupMenuItem::new(t).checked(t == current_type).on_click(move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            this.ui.measure_type = (t != "Show all").then_some(t);
                            cx.notify();
                        })
                    }));
                }
                m
            })
            .into_any_element();
        let length_unit: SharedString = format!("{:?}", units.length).into();
        let angle_unit: SharedString = format!("{:?}", units.angle).into();
        vec![
            v_flex()
                .id("measure-panel")
                .occlude()
                .absolute()
                .bottom(px(36.))
                .right(px(40.))
                .w(px(220.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(pal.text)
                .child(h_flex().h(px(29.)).pl(px(6.)).border_b_1().border_color(pal.border).child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(title)).child(close("measure-close", cx)))
                .child(field("Select entities to measure", selected_names, true, false))
                .child(select_row("Measure type:", current_type.into(), Some(type_menu)))
                .child(select_row("Length unit:", length_unit, None))
                .child(select_row("Angle unit:", angle_unit, None))
                .child(crate::ui::os_checkbox("measure-ref", "Reference coordinate system", false))
                .children(rows.into_iter().filter(|(k, _)| !k.starts_with("Select")).map(|(k, v)| {
                    h_flex()
                        .min_h(px(26.))
                        .px(px(5.))
                        .gap(px(4.))
                        .text_size(px(13.33))
                        .child(div().text_color(pal.text).child(format!("{k}:")))
                        .child(div().px(px(2.)).bg(gpui_kit::rgb(0xf5f5f5)).border_b_1().border_color(pal.border).font_family("monospace").text_size(px(12.)).child(v))
                }))
                .child(h_flex().h(px(22.)).px(px(4.)).justify_end().bg(gpui_kit::rgb(0xeeeeee)).child(help()))
                .into_any_element(),
        ]
    }
}

/// The Part tab of Mass and section properties, formatted.
struct MassView {
    volume: String,
    area: String,
    mass: Option<String>,
    center: [String; 3],
    inertia_unit: String,
    inertia: Option<[[String; 3]; 3]>,
    parts: Vec<String>,
    reference: Option<String>,
}

trait WhenSelected {
    fn when_selected(self, on: bool) -> Self;
}

impl WhenSelected for Button {
    fn when_selected(self, on: bool) -> Self {
        use gpui_kit::component::Selectable as _;
        self.selected(on)
    }
}

/// Kind of a measurement for the measure type selector.
fn measurement_kind(m: &Measurement) -> &'static str {
    match m {
        Measurement::Length(_) => "Length",
        Measurement::Area(_) | Measurement::Volume(_) => "Area",
        Measurement::Radius(_) => "Radius",
        Measurement::Diameter(_) => "Diameter",
        Measurement::Distance { .. } => "Distance",
        Measurement::Angle(_) => "Angle",
        Measurement::Point(_) => "Position",
    }
}

