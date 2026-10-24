//! Viewport context menu, measurement panel, mass properties and part
//! properties for the Part Studio view.

use super::PartStudioView;
use crate::os_menu::{OsMenu, OsMenuItem};
use crate::os_ui::OsDropupExt as _;
use crate::scene_builder::{PickKind, PickTarget};
use crate::theme::{p, with_alpha};
use crate::ui::{TipExt as _, TipSide};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use noshape_model::partstudio::PartProperties;
use noshape_model::reference::{BodyId, FeatureId, Reference};
use noshape_model::regen::measure::{self, Measurement};
use noshape_render::StandardView;
use noshape_units::{Dim, Quantity};

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
    pub(super) fn creating_feature(&self, r: &Reference) -> Option<FeatureId> {
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
        let sketch_swatch = creator.map(|id| self.sketch_swatch(id, cx));
        let source_sketch = creator.and_then(|id| studio.and_then(|s| s.feature(id).and_then(|f| noshape_model::interaction::source_sketch(&s.features, f)).and_then(|sid| s.feature(sid).map(|g| (sid, g.name.clone())))));
        let has_selection = !self.selection.is_empty() || !self.selected_features.is_empty();
        let isolate = self.ui.isolate.as_ref().map(|i| i.mode);
        let section = !self.ui.section.is_empty();
        let editing = self.edit.is_some();
        let editing_tool = self.edit.as_ref().and_then(|e| e.dialog.read(cx).feature.feature_type()).map(|t| t.name);
        let planar = target_ref.as_ref().is_some_and(|r| self.frame_of_ref(r).is_some());
        let linear_edge = target_ref.as_ref().is_some_and(|r| self.linear_edge_direction(r).is_some());
        let plane_name = target_ref.as_ref().map(|r| self.labeler(cx)(r)).unwrap_or_default();
        let paste_sketch = Self::clipboard_has_sketch(cx);
        // Shown sketches besides the clicked one's (sketch region menus).
        let other_sketches_shown = self.display.sketches.iter().filter(|s| !s.hidden && Some(s.feature) != creator).count() > 0;
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
            let plain_tool = |label: &str, tool_id: &'static str, r: &Reference| {
                let r = r.clone();
                item(label, Box::new(move |this, w, cx| {
                    this.selection = vec![r.clone()];
                    this.start_feature(tool_id, w, cx);
                }))
            };
            let select_menu = |m: OsMenu, r: &Reference, k: PickKind, window: &mut Window, cx: &mut Context<OsMenu>| -> OsMenu {
                // Select ▸ grows the entity into related ones; Select other… lists what is under the cursor.
                let criteria: &[noshape_model::select::Criterion] = match k {
                    PickKind::Face => &noshape_model::select::Criterion::FACE,
                    PickKind::Edge => &noshape_model::select::Criterion::SKETCH_EDGE,
                    // Live: a region gets the face set, a line or circle the sketch edge set.
                    PickKind::SketchRegion => &noshape_model::select::Criterion::FACE,
                    PickKind::SketchCurve => &noshape_model::select::Criterion::SKETCH_EDGE,
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
                        let v = vs.clone();
                        sm.item(OsMenuItem::new("Create selection…").on_click(move |_, _, cx| v.update(cx, |this, cx| {
                            // Live: the right-clicked entity is not pre-filled.
                            this.close_context_menu(cx);
                            this.open_create_selection_at(this.ui.menu_pos, cx);
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
                    if matches!(r, Reference::Plane(_)) {
                        let rz = r.clone();
                        m = m.item(item("Size to Part Studio", Box::new(move |this, _, cx| this.size_plane_to_studio(rz.clone(), cx))));
                    }
                    if paste_sketch {
                        let rp = r.clone();
                        m = m.item(item("Paste sketch", Box::new(move |this, _, cx| this.paste_sketch_onto(rp.clone(), cx))));
                    }
                    let (rh, ro, rs) = (r.clone(), r.clone(), r.clone());
                    m = m
                        .separator()
                        .item(icon_item(&format!("Hide {plane_name}"), "eye-off", Box::new(move |this, _, cx| {
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
                        m = m.item(item(&format!("Edit {name}…"), Box::new(move |this, w, cx| this.edit_feature(id, w, cx))));
                        // Curves: Copy sketch entities (live order).
                        if let (PickKind::SketchCurve, Reference::SketchEntity { entity, .. }) = (k, &r) {
                            let entity = *entity;
                            m = m.item(item("Copy sketch entities", Box::new(move |this, _, cx| this.copy_sketch_entities(id, entity, cx))));
                        }
                        m = m
                            .item(item("Show dimensions", Box::new(move |this, w, cx| this.show_dimensions(id, w, cx))))
                            .item(item(&format!("Show dependencies of {name}…"), Box::new(move |this, _, cx| {
                                this.ui.dependencies = Some(id);
                                cx.notify();
                            })))
                            .separator();
                    }
                    if k == PickKind::SketchRegion {
                        m = m
                            .item(tool("New sketch…", "sketch", "sketch", &r))
                            .item(tool("Offset plane…", "plane", "cPlane", &r))
                            .item(tool("Extrude…", "extrude", "extrude", &r))
                            .item(tool("Revolve…", "revolve", "revolve", &r))
                            .separator();
                    }
                    let rh = r.clone();
                    m = m.item(icon_item("Hide", "eye-off", Box::new(move |this, _, cx| {
                        this.set_visibility(std::slice::from_ref(&rh), true, cx);
                        this.hover = None;
                    })));
                    // Live: Hide other sketches, Hide all sketches, when other
                    // sketches are shown (a lone sketch only has Hide).
                    if other_sketches_shown {
                        m = m
                            .item(item("Hide other sketches", Box::new(move |this, _, cx| this.hide_sketches(creator, cx))))
                            .item(item("Hide all sketches", Box::new(|this, _, cx| this.hide_sketches(None, cx))));
                    }
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
                            // Live: Create Drawing of Sketch N…, Export as DXF/DWG… (a sketch view; asb).
                            let sk_name = creator_name.clone().unwrap_or_else(|| "Sketch".into());
                            m = m.separator().item(icon_item(&format!("Create Drawing of {sk_name}…"), "menu-create-drawing", Box::new(move |this, w, cx| crate::drawing_create::request_create_drawing_of_sketch(this.tab, id, &sk_name, w, cx))));
                            m = m.item(item("Export as DXF/DWG…", Box::new(move |this, w, cx| this.export_sketch(id, "dxf", w, cx))));
                        }
                    }
                    m = select_menu(m.separator(), &r, k, window, cx).separator().item(zoom_fit()).item(zoom_sel(&r));
                    if k == PickKind::SketchRegion {
                        m = m.item(normal_to(&r));
                    } else if k == PickKind::SketchCurve && linear_edge {
                        // Straight sketch lines (live curve menu).
                        let ra = r.clone();
                        m = m.item(item("Align view with vertical axis", Box::new(move |this, _, cx| this.align_view_vertical(&ra, cx))));
                    }
                    if let (Some(id), Some(name)) = (creator, creator_name.clone()) {
                        m = m
                            .separator()
                            .item(icon_item(&format!("Delete {name}"), "menu-delete", Box::new(move |this, _, cx| this.delete_features(vec![id], cx))))
                            .separator()
                            .item(item("Edit sketch appearance…", Box::new(move |this, w, cx| this.edit_appearance(super::appearance::EditTarget::Feature(id, None), w, cx))).swatch(sketch_swatch.unwrap_or_else(|| rgb(0x999999).into())));
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
                        // Live (menus/rmb-ps-edge): no icons on these two.
                        m = m.item(plain_tool("Fillet…", "fillet", &r)).item(plain_tool("Chamfer…", "chamfer", &r)).separator();
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
                        if k != PickKind::Edge {
                            let rs = r.clone();
                            m = m.item(icon_item("Section view…", "menu-section-view", Box::new(move |this, w, cx| {
                                if this.frame_of_ref(&rs).is_some() {
                                    this.section_through(rs.clone(), w, cx);
                                } else {
                                    this.section_part(&b4, w, cx);
                                }
                            })));
                        }
                        // Copy the part (paste derives it in another Part Studio).
                        let bc = b.clone();
                        m = m.separator().item(icon_item(&format!("Copy {body_name}"), "menu-copy", Box::new(move |this, _, cx| this.copy_part(bc.clone(), cx))));
                        // Drawings and export.
                        let (b7, b8, name) = (b.clone(), b.clone(), body_name.to_string());
                        m = m
                            .separator()
                            .item(icon_item(&format!("Create Drawing of {body_name}…"), "menu-create-drawing", Box::new(move |this, w, cx| {
                                let source = Some(noshape_model::drawing::create::DrawingSource { tab: this.tab, part: Some(b7.clone()), config: Default::default(), sketch: None });
                                crate::drawing_create::request_create_drawing(crate::drawing_create::CreateRequest { source, name: Some(name.clone()) }, w, cx);
                            })));
                        if k == PickKind::Face && planar && faces.len() <= 1 {
                            let rf = r.clone();
                            m = m.item(item("Export as DXF/DWG…", Box::new(move |this, w, cx| this.export_face(rf.clone(), w, cx))));
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
                            // Live (qa5 DEL2): the Delete part dialog, the part prefilled.
                            this.selection = vec![Reference::Body(bd.clone())];
                            this.start_feature("deleteBodies", w, cx);
                        })));
                        if let (Some(id), Some(name)) = (creator, creator_name.clone()) {
                            m = m.item(icon_item(&format!("Delete {name}"), "menu-delete", Box::new(move |this, _, cx| this.delete_features(vec![id], cx))));
                        }
                        let (b1, b2) = (b.clone(), b.clone());
                        m = m
                            .separator()
                            .item(item(&format!("Assign material for {body_name}…"), Box::new(move |this, w, cx| this.open_material_panel(vec![b1.clone()], w, cx))))
                            .item(item(&format!("Edit appearance for {body_name}…"), Box::new(move |this, w, cx| this.edit_appearance(super::appearance::EditTarget::Parts(vec![b2.clone()]), w, cx))));
                        if k == PickKind::Face {
                            let fs = faces.clone();
                            let label = if fs.len() > 1 { format!("Add appearance to {} faces…", fs.len()) } else { "Add appearance to face…".to_string() };
                            m = m.item(icon_item(&label, "add-appearance", Box::new(move |this, w, cx| this.edit_appearance(super::appearance::EditTarget::Faces(fs.clone(), None), w, cx))));
                        }
                    }
                }
                // Empty space.
                _ => {
                    m = m.item(item("Show all", Box::new(|this, _, cx| this.show_everything(cx)))).separator();
                    // Select ▸ with nothing selected: only Create selection…
                    // (live menus/rmb-sketch-empty-select-submenu.png).
                    let vs = v.clone();
                    m = m.submenu("Select", window, cx, move |sm, _, _| {
                        let v = vs.clone();
                        sm.item(OsMenuItem::new("Create selection…").on_click(move |_, _, cx| v.update(cx, |this, cx| {
                            this.close_context_menu(cx);
                            this.open_create_selection_at(this.ui.menu_pos, cx);
                        })))
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
        if let Some((pos, _, _)) = self.context_menu.take() {
            self.ui.menu_pos = pos;
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

    /// Parts list › Properties…: Onshape's metadata Properties modal
    /// (live dom/dialog-assembly-part-properties.txt, "Onshape Part").
    pub(super) fn open_part_meta(&mut self, body: BodyId, window: &mut Window, cx: &mut Context<Self>) {
        use crate::properties_dialog::{MetaSpec, MetaValues};
        let props = self.studio(cx).and_then(|s| s.part_properties.get(&body).cloned()).unwrap_or_default();
        let name = self.display.body(&body).map(|b| b.name.clone()).unwrap_or_default();
        let computed = self.context.as_ref().filter(|_| props.material.is_some()).and_then(|ctx| {
            let density = |_: &BodyId| props.density;
            noshape_model::regen::measure::mass_properties(ctx, std::slice::from_ref(&body), &density)?.mass
        });
        let spec = MetaSpec {
            category: "Onshape Part",
            material: Some(props.material.clone().unwrap_or_else(|| "Unassigned".into())),
            computed_mass: computed.map(|m| format!("{} kg", noshape_units::to_fixed(m, 4))),
            bom_options: None,
            values: MetaValues {
                name,
                description: props.description.clone().unwrap_or_default(),
                part_number: props.part_number.clone().unwrap_or_default(),
                // Workspace parts have no revision unless one was typed
                // (Release__typical_release_workflow).
                revision: props.revision.clone().unwrap_or_default(),
                meta: props.meta.clone(),
                mass_override: props.mass_override.map(|m| format!("{m} kg")),
                bom: 0,
            },
        };
        let (doc, view, tab) = (self.doc.clone(), cx.entity(), self.tab);
        let (doc_r, body_r, name_r) = (self.doc.clone(), body.clone(), spec.values.name.clone());
        // Releases the configured part shown (Help: Releasing a configuration).
        let config_r = self.config_values.clone();
        // Reset all: the regenerated name, no metadata.
        let default_name = self.context.as_ref().and_then(|c| c.bodies.iter().find(|x| x.id == body).map(|x| x.name.clone())).unwrap_or_else(|| spec.values.name.clone());
        let reset_to = MetaValues { name: default_name, ..MetaValues::default() };
        // Fields configured for the active configuration show that value,
        // disabled, and saving leaves the part's own value alone (live psb
        // 10-20 q, 645–646).
        const FIELDS: [&str; 10] = ["Name", "Description", "Part number", "Revision", "Vendor", "Project", "Product line", "Title 1", "Title 2", "Title 3"];
        let mut spec = spec;
        let mut configured: Vec<&'static str> = Vec::new();
        if let Some(ps) = self.studio(cx) {
            for f in FIELDS {
                let Some(v) = noshape_model::configured_properties::configured_value(ps, &body, f, &self.config_values) else { continue };
                configured.push(f);
                let m = &mut spec.values;
                match f {
                    "Name" => m.name = v,
                    "Description" => m.description = v,
                    "Part number" => m.part_number = v,
                    "Revision" => m.revision = v,
                    "Vendor" => m.meta.vendor = Some(v),
                    "Project" => m.meta.project = Some(v),
                    "Product line" => m.meta.product_line = Some(v),
                    "Title 1" => m.meta.title1 = Some(v),
                    "Title 2" => m.meta.title2 = Some(v),
                    _ => m.meta.title3 = Some(v),
                }
            }
        }
        let kept = configured.clone();
        crate::properties_dialog::open_part_properties_configured(
            spec,
            Some(reset_to),
            configured,
            move |v, _, cx| {
                let body = body.clone();
                let values = noshape_ops::studio::PartPropertyValues {
                    name: v.name.clone(),
                    description: v.description.clone(),
                    part_number: v.part_number.clone(),
                    revision: v.revision.clone(),
                    meta: v.meta.clone(),
                    mass_override: v.mass_override.as_deref().and_then(crate::properties_dialog::parse_mass),
                };
                doc.update(cx, |d, cx| {
                    d.edit_studio(tab, &noshape_ops::studio::properties_description(&v.name), cx, |ps| noshape_ops::studio::set_part_properties(ps, &body, &values, &kept));
                });
                view.update(cx, |this, cx| this.refresh_scene(true, cx));
            },
            move |window, cx| {
                let items = crate::release_dialog::part_items(doc_r.read(cx), tab, std::slice::from_ref(&body_r), std::slice::from_ref(&name_r), &config_r);
                crate::release_dialog::open_release_candidate(doc_r.clone(), items, window, cx)
            },
            window,
            cx,
        );
    }

    /// Onshape "Export…" on parts: the Export modal (see `export.rs`).
    pub(super) fn open_export_dialog(&mut self, bodies: Vec<BodyId>, window: &mut Window, cx: &mut Context<Self>) {
        self.open_part_export(bodies, window, cx);
    }

    pub(super) fn export_flat(&mut self, body: BodyId, ext: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ctx) = self.context.clone() else { return };
        let flat = match noshape_model::sheetmetal::flat_pattern_for(&ctx, &body) {
            Ok(f) => f,
            Err(e) => {
                self.message = Some((noshape_model::display::FeatureStatus::Error, e));
                cx.notify();
                return;
            }
        };
        let name = self.display.body(&body).map(|b| b.name.clone()).unwrap_or_else(|| "flat".into());
        let view = cx.entity();
        crate::files::request_save_path("Export flat pattern", format!("{name} flat.{ext}"), ext, window, cx, move |path, cx| {
            // DXF as live's "Export DXF/DWG of Flat pattern" writes it (SM14).
            let bytes = if ext == "dxf" {
                noshape_model::sheetmetal::flat_dxf::flat_dxf(&flat, &Default::default())
            } else {
                Ok(noshape_model::drawing::export::to_svg(&flat.to_sheet_geometry()).into_bytes())
            };
            let msg = match bytes.and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string())) {
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

    /// The Measure panel's (x) (live 10-20 o, part-studio/566–572): the
    /// panel closes and a new Variable feature opens at the rollback bar on
    /// its Measured tab, the measured picks filled in and the Name empty:
    /// a distance row → Distance (Minimum / Maximum as the row; an X/Y/Z
    /// sub-row records "Along X/Y/Z"), Length → Length, Diameter / Radius →
    /// Diameter.
    fn create_variable_from_measure(&mut self, var: MeasureVar, axis: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        use noshape_model::params::ParamValue;
        if self.edit.is_some() {
            return;
        }
        let units = self.doc.read(cx).session.doc.settings.units;
        let Some(studio) = self.studio(cx) else { return };
        let Some(mut feature) = studio.new_feature("assignVariable", &units) else { return };
        let index = studio.rollback_index();
        let picks = self.selection.clone();
        let (mode, max) = var;
        let p = &mut feature.params;
        p.set("mode", ParamValue::Enum("MEASURED".into()));
        match mode {
            "LENGTH" => {
                p.set("measurementMode", ParamValue::Enum("LENGTH".into()));
                p.set("lengthEntities", ParamValue::Query(picks));
            }
            "DIAMETER" | "RADIUS" => {
                p.set("measurementMode", ParamValue::Enum("DIAMETER".into()));
                p.set("radius", ParamValue::Bool(mode == "RADIUS"));
                p.set("diameterEntity", ParamValue::Query(picks.into_iter().take(1).collect()));
            }
            _ => {
                p.set("measurementMode", ParamValue::Enum("DISTANCE".into()));
                p.set("minmax", ParamValue::Enum(if max { "MAXIMUM" } else { "MINIMUM" }.into()));
                p.set("entityCouple", ParamValue::Query(picks));
                if let Some(i) = axis {
                    p.set("axisMeasurements", ParamValue::Bool(true));
                    p.set("componentSelector", ParamValue::Enum(["X", "Y", "Z"][i].into()));
                }
            }
        }
        // The Measure panel closes.
        self.set_show_mass(false, cx);
        self.ui.measure_pinned = false;
        self.selection.clear();
        self.open_dialog(feature, index, true, window, cx);
    }

    /// A measured value as the status-bar readout shows it (document units).
    fn measure_fmt(&self, q: Quantity, cx: &App) -> String {
        measure_fmt_in(q, &self.doc.read(cx).session.doc.settings.units)
    }

    /// The Measure panel's units: its own Length / Angle unit selects over
    /// the workspace units.
    fn measure_units(&self, cx: &App) -> noshape_units::UnitPrefs {
        let mut u = self.doc.read(cx).session.doc.settings.units;
        if let Some(l) = self.ui.measure_length_unit {
            u.length = l;
        }
        if let Some(a) = self.ui.measure_angle_unit {
            u.angle = a;
        }
        u
    }

    /// The live measurement of the selection left of the evaluation tools
    /// (live part-studio/81: "Perp planes angle: 90.000 deg", the number in
    /// monospace; a point reads "Point: X↘ … Y↗ … Z↑ …" with colored axes).
    fn selection_readout(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let sel = self.selection.clone();
        let sel_count = sel.len();
        let ctx = self.measure_context(&sel, cx)?;
        let results = match &self.measure_cache {
            Some((s, r)) if *s == sel => r.clone(),
            _ => {
                let r = studio_measure(&ctx, &sel);
                self.measure_cache = Some((sel, r.clone()));
                r
            }
        };
        // Live (psb 10-20 m): the first primary row, plus the distances of a
        // circular context on the same line; nothing for three edges.
        let line = measure::quick_line(&results, sel_count);
        if line.is_empty() {
            return None;
        }
        let pal = p();
        // "12.5 mm" -> number (monospace, underlined) and unit.
        let value = |text: String| {
            let (num, unit) = text.split_once(' ').map(|(n, u)| (n.to_string(), u.to_string())).unwrap_or((text, String::new()));
            h_flex()
                .gap(px(2.))
                .child(crate::ui::measure_value(num).text_color(pal.text))
                .child(div().child(unit))
        };
        let l = |x: f64| self.measure_fmt(Quantity::new(x, Dim::LENGTH), cx);
        let mut row = h_flex().gap(px(12.)).pr(px(6.)).text_size(px(13.)).text_color(pal.text_muted).whitespace_nowrap();
        for (label, m) in line {
            let item = h_flex().gap(px(4.)).child(format!("{label}:"));
            row = row.child(match m {
                // Live: "Point: 0.000 mm 5.000 mm 20.000 mm".
                Measurement::Point(pt) => item.child(value(l(pt.x))).child(value(l(pt.y))).child(value(l(pt.z))),
                m => {
                    let text = match m {
                        Measurement::Length(x) | Measurement::Radius(x) | Measurement::Diameter(x) => l(x),
                        Measurement::Area(x) => self.measure_fmt(Quantity::new(x, Dim { length: 2, angle: 0 }), cx),
                        Measurement::Volume(x) => self.measure_fmt(Quantity::new(x, Dim { length: 3, angle: 0 }), cx),
                        Measurement::Angle(x) => self.measure_fmt(Quantity::new(x, Dim::ANGLE), cx),
                        Measurement::Percent(x) => format!("{} %", noshape_units::to_fixed(x, 2)),
                        Measurement::Distance { distance, .. } => l(distance),
                        Measurement::Point(_) => String::new(),
                    };
                    item.child(value(text))
                }
            });
        }
        Some(row.into_any_element())
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
        let readout = (!measure_on && !mass_on && self.edit.is_none() && !self.selection.is_empty()).then(|| self.selection_readout(cx)).flatten();
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
            .children(readout)
            // Live (part-studio/22): the tooltips sit over the buttons.
            .child(
                div().id("eval-measure-tip").child(btn("eval-measure", "measure", measure_on, cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.ui.measure_pinned = !this.ui.measure_pinned;
                    this.set_show_mass(false, cx);
                    cx.notify();
                }))).tip("Show measure details ([)", TipSide::Above),
            )
            // Onshape's "Show analysis tools" menu (Help: Analysis tools), in its order.
            // Live (dom/menu-analysis-tools.txt): a 157 px context-menu list of
            // 22 px rows above the button. Thickness analysis is only offered
            // on paid plans (Help View__thickness_analysis: "Only available
            // for", in this menu; the free account's capture has 10 items).
            .child(div().id("eval-analysis-tip").child(btn("eval-analysis", "analysis-tools", self.ui.analysis.is_some() || self.ui.analysis_tool.is_some() || self.ui.zebra.is_some() || self.ui.reflection.is_some() || self.ui.thickness.is_some(), cx).os_dropup_menu("eval-analysis-menu", move |m, _, _| {
                use super::analysis_tools::ToolKind as K;
                let tool = |label: &'static str, kind: K| {
                    let v = view.clone();
                    crate::os_menu::OsMenuItem::new(label).on_click(move |_, w, cx| v.update(cx, |this, cx| this.toggle_analysis_tool(kind, w, cx)))
                };
                let v = view.clone();
                let (v9, v10, v11) = (view.clone(), view.clone(), view.clone());
                let m = m.min_width(157.)
                    .item(tool("Curve/surface analysis…", K::CurveSurface))
                    .item(tool("Deviation analysis…", K::Deviation))
                    .item(tool("Connection analysis…", K::Connection))
                    .item(tool("Dihedral analysis…", K::Dihedral))
                    .item(tool("Interference detection…", K::Interference))
                    .item(tool("Zebra stripes…", K::Zebra))
                    .item(tool("Reflection analysis…", K::Reflection))
                    .item(crate::os_menu::OsMenuItem::new("Curvature color map").on_click(move |_, w, cx| v.update(cx, |this, cx| this.toggle_analysis(super::analysis::Analysis::Curvature, w, cx))))
                    .item(tool("Draft analysis…", K::Draft));
                // Once on, the menu edits or turns it off (Help).
                let m = if thickness_on {
                    m.item(crate::os_menu::OsMenuItem::new("Edit thickness analysis").on_click(move |_, w, cx| v11.update(cx, |this, cx| this.toggle_analysis_tool(K::Thickness, w, cx))))
                        .item(crate::os_menu::OsMenuItem::new("Turn thickness analysis off").on_click(move |_, _, cx| v10.update(cx, |this, cx| this.thickness_off(cx))))
                } else {
                    m.item(crate::os_menu::OsMenuItem::new("Thickness analysis…").on_click(move |_, w, cx| v9.update(cx, |this, cx| this.toggle_analysis_tool(K::Thickness, w, cx))))
                };
                m.item(tool("Flatten surfaces…", K::Flatten))
            })).tip("Show analysis tools", TipSide::Above))
            .child(
                div().id("eval-mass-tip").child(btn("eval-mass", "mass-properties", mass_on, cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.ui.measure_pinned = false;
                    this.set_show_mass(!this.show_mass, cx);
                }))).tip("Display mass and section properties", TipSide::Above),
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
        // The Measure panel's rows: (label, value, X/Y/Z sub-row axis).
        let mut mrows: Vec<MeasureRow> = Vec::new();
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
            // Inertia about a selected mate connector (reference coordinate system).
            let reference = self.selection.iter().find_map(|r| self.display.connectors.iter().find(|c| &c.reference == r)).cloned();
            let studio = self.studio(cx).cloned().unwrap_or_default();
            if let Some(noshape_ops::analysis::StudioMass { props: mp, reference }) = noshape_ops::analysis::studio_mass_properties(&ctx, &self.display, &studio, &bodies, reference.as_ref()) {
                if let Some(name) = &reference {
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
                let v = |x: f64| noshape_units::to_fixed(x / f, 4);
                mass_view = Some(MassView {
                    volume: self.fmt(Quantity::new(mp.volume, len3), cx),
                    area: self.fmt(Quantity::new(mp.surface_area, len2), cx),
                    mass: mp.mass.map(|m| format!("{} kg", noshape_units::to_fixed(m, 4))),
                    // Without a material Onshape leaves the center of mass empty.
                    center: if mp.mass.is_some() { [l(c.x), l(c.y), l(c.z)] } else { Default::default() },
                    inertia_unit: format!("{sym}² kg"),
                    inertia: mp.mass.map(|_| [[v(i.x_axis.x), v(i.y_axis.x), v(i.z_axis.x)], [v(i.x_axis.y), v(i.y_axis.y), v(i.z_axis.y)], [v(i.x_axis.z), v(i.y_axis.z), v(i.z_axis.z)]]),
                    parts: if bodies.is_empty() { Vec::new() } else { bodies.iter().filter_map(|b| self.display.body(b).map(|d| d.name.clone())).collect() },
                    reference,
                });
            }
        } else {
            // Live (part-studio/81-82): a selection only gets the one-line
            // readout at the bottom right; the panel opens with `[` / the
            // Measure button.
            if !self.ui.measure_pinned {
                return Vec::new();
            }
            let sel = self.selection.clone();
            let ctx = self.measure_context(&sel, cx).unwrap_or(ctx);
            let results = match &self.measure_cache {
                Some((s, r)) if *s == sel => r.clone(),
                _ => {
                    let r = studio_measure(&ctx, &sel);
                    self.measure_cache = Some((sel, r.clone()));
                    r
                }
            };
            let filter = self.ui.measure_type;
            let units = self.measure_units(cx);
            let f = |q: Quantity| measure_fmt_in(q, &units);
            let l = |x: f64| f(Quantity::new(x, Dim::LENGTH));
            for (label, m) in results {
                // Measure type selector (bundle: each type and the rows it keeps).
                if let Some(t) = filter
                    && measure_filter_type(&label) != Some(t)
                {
                    continue;
                }
                let v = match m {
                    Measurement::Length(x) | Measurement::Radius(x) | Measurement::Diameter(x) => l(x),
                    Measurement::Area(x) => f(Quantity::new(x, Dim { length: 2, angle: 0 })),
                    Measurement::Volume(x) => f(Quantity::new(x, Dim { length: 3, angle: 0 })),
                    Measurement::Angle(x) => f(Quantity::new(x, Dim::ANGLE)),
                    Measurement::Percent(x) => format!("{} %", noshape_units::to_fixed(x, 2)),
                    // Live: "Point: 0.000 in 0.197 in 0.787 in".
                    Measurement::Point(p) => format!("{} {} {}", l(p.x), l(p.y), l(p.z)),
                    Measurement::Distance { distance, .. } => l(distance),
                };
                // (x) on distance-type rows only: angles, areas and
                // positions have none (live 566).
                let var: Option<MeasureVar> = match m {
                    Measurement::Distance { .. } => Some(("DISTANCE", label.starts_with("Max"))),
                    Measurement::Length(_) => Some(("LENGTH", false)),
                    Measurement::Diameter(_) => Some(("DIAMETER", false)),
                    Measurement::Radius(_) => Some(("RADIUS", false)),
                    _ => None,
                };
                mrows.push((label, v, None, var));
                // Live: signed X / Y / Z sub-rows under a distance.
                if let Measurement::Distance { dx, dy, dz, .. } = m {
                    for (i, d) in [dx, dy, dz].into_iter().enumerate() {
                        mrows.push((String::new(), l(d), Some(i), var));
                    }
                }
            }
        }
        let current_type = self.ui.measure_type.unwrap_or("Show all");
        let view = cx.entity();
        // `panel`: the Measure panel's thin grey × (live part-studio/82: 11 px
        // of `#999` ink) instead of the feature dialogs' red ✕.
        let close = move |id: &'static str, panel: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .size(px(28.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|d| d.bg(pal.panel_hover))
                .cursor(gpui_kit::CursorStyle::Arrow)
                .when(panel, |d| d.child(crate::ui::glyph("panel-close", 20., gpui_kit::rgb(0x999999))))
                .when(!panel, |d| d.child(crate::assets::icon("dialog-cancel", 24., cx)))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.set_show_mass(false, cx);
                    this.ui.measure_pinned = false;
                    this.selection.clear();
                    this.refresh_scene(false, cx);
                    cx.notify();
                }))
        };
        let help = || div().size(px(14.)).rounded_full().flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).child(crate::ui::glyph("help-dot", 15.6, crate::theme::help_dot(0xaaaaaa)));
        // Onshape's selection box: `#def1fb` with a 1 px `#1651b0` border when
        // active, the caption small on top once something is listed.
        // `invalid`: entries in red (live: parts without a material).
        // `removable`: the items are `self.selection` and a chip's × drops
        // its entry (the Measure field).
        let field_view = cx.entity();
        let field = |label: &str, items: Vec<String>, active: bool, invalid: bool, removable: bool| {
            let empty = items.is_empty();
            v_flex()
                .mx(px(3.))
                .my(px(2.))
                .rounded(px(2.))
                .border_1()
                .border_color(if active { pal.accent } else { pal.border })
                .when(active, |d| d.bg(pal.row_hover))
                // Live (dom/dialog-mass-properties.txt): 26 px empty, 42 with one
                // chip (16 px label strip + 24 px chip rows) with the border.
                .child(div().px(px(6.)).flex().items_center().when(empty, |d| d.h(px(25.)).pt(px(2.)).text_size(px(12.))).when(!empty, |d| d.h(px(16.)).pt(px(2.)).text_size(px(10.))).text_color(gpui_kit::rgb(0x555555)).child(label.to_string()))
                // Live (part-studio/82): a 20 x 24 × cell (5 px left pad, 7 px of
                // `#666` ink, red on hover) 5 px from the border; every second
                // row 35 % lighter (woolsthorpe.css, as in the feature dialogs).
                .children(items.into_iter().enumerate().map(|(i, t)| {
                    h_flex()
                        .id(SharedString::from(format!("{label}-chip-{i}")))
                        .h(px(24.))
                        .pl(px(6.))
                        .pr(px(5.))
                        .text_size(px(13.33))
                        .text_color(if invalid { pal.text_invalid } else { gpui_kit::rgb(0x555555) })
                        .when(i % 2 == 1, |d| d.bg(hsla(0., 0., 1., 0.35)))
                        .child(div().flex_1().truncate().child(t))
                        .child(div().id("x").group("measure-x").w(px(20.)).h(px(24.)).pl(px(5.)).flex().items_center().child(crate::ui::glyph("ui-clear", 9., pal.icon_tertiary).when(removable, |g| g.group_hover("measure-x", |s| s.text_color(pal.error)))).when(removable, |d| {
                            let view = field_view.clone();
                            d.cursor_pointer().on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                view.update(cx, |this, cx| {
                                    if i < this.selection.len() {
                                        this.selection.remove(i);
                                        this.refresh_scene(false, cx);
                                        cx.notify();
                                    }
                                })
                            })
                        }))
                }))
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
                    .when(on, |d| d.text_color(pal.emphasis).font_weight(FontWeight::MEDIUM).border_b_2().border_color(pal.accent))
                    .when(!on, |d| d.text_color(pal.text).border_b_1().border_color(pal.border))
                    .child(label)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.ui.mass_faces = faces;
                        cx.notify();
                    }))
            };
            // Read-only Override (live: a gray `#999` filled box).
            let disabled_check = |id: &'static str| h_flex().id(id).h(px(27.)).pl(px(4.)).gap(px(4.)).text_size(px(13.33)).text_color(pal.text_muted).child(div().size(px(12.)).rounded(px(3.)).bg(gpui_kit::rgb(0x999999))).child("Override");
            // Live (part-studio/121): the letter in #999, a coloured arrow glyph.
            let axis = |l: &'static str, c: u32, glyph: &'static str| h_flex().w(px(30.)).pl(px(6.)).text_size(px(15.)).text_color(gpui_kit::rgb(0x999999)).child(l).child(crate::ui::glyph(glyph, 14., gpui_kit::rgb(c)));
            let mut body = v_flex().w_full().child(h_flex().w_full().mb(px(2.)).child(tab("mass-tab-part", "Part", false, cx)).child(tab("mass-tab-face", "Face", true, cx)));
            match mass_view {
                Some(m) => {
                    body = body
                        // Live: 5 px between the tabs and the two fields, 8 under them.
                        .child(field(if self.ui.mass_faces { "Faces to measure" } else { "Parts to measure" }, m.parts.clone(), true, m.mass.is_none(), false).mt(px(5.)).mb(px(0.)))
                        .child(field("Mate connector for reference frame", m.reference.clone().into_iter().collect(), false, false, false).mt(px(5.)).mb(px(0.)))
                        // Live (dom/dialog-mass-properties.txt): the rows from here are
                        // 27 / 26 / 29 px tall, 2 px apart (3 px above X): X, Y and Z
                        // sit 31 px apart, the inertia rows 28.
                        .child(div().mt(px(9.)).h(px(27.)).child(crate::ui::os_checkbox("mass-variance", "Show calculation variance", false)))
                        .child(h_flex().mt(px(2.)).h(px(27.)).child(div().pl(px(5.)).text_size(px(12.)).text_color(pal.text_muted).child("Mass")).child(crate::ui::os_checkbox("mass-override", "Override", false)).child(div().flex_1().mr(px(5.)).h(px(24.)).flex().items_center().justify_end().border_b_1().border_color(pal.border).text_size(px(13.33)).text_color(pal.text).child(m.mass.clone().unwrap_or_default())))
                        .child(value_row("Volume".into(), m.volume.clone()).mt(px(2.)).h(px(26.)))
                        .child(value_row("Surface area".into(), m.area.clone()).mt(px(2.)).h(px(26.)))
                        .child(h_flex().mt(px(2.)).h(px(27.)).child(div().pl(px(5.)).text_size(px(12.)).text_color(pal.text_muted).child("Center of mass")).child(disabled_check("com-override")))
                        .children([("X", 0xbd3039u32, "axis-arrow-x"), ("Y", 0x009400, "axis-arrow-y"), ("Z", 0x1651b0, "axis-arrow-z")].into_iter().zip(m.center.iter()).enumerate().map(|(k, ((l, c, g), v))| {
                            h_flex().mt(px(if k == 0 { 3. } else { 2. })).h(px(29.)).child(axis(l, c, g)).child(div().flex_1().mr(px(5.)).h(px(24.)).flex().items_center().justify_end().border_b_1().border_color(pal.border).text_size(px(13.33)).text_color(pal.text).child(v.clone()))
                        }))
                        .child(h_flex().mt(px(2.)).h(px(27.)).child(div().pl(px(5.)).text_size(px(12.)).text_color(pal.text_muted).child(format!("Mass moments of inertia ({})", m.inertia_unit))).child(disabled_check("moi-override")));
                    let names = [["Lxx", "Lxy", "Lxz"], ["Lyx", "Lyy", "Lyz"], ["Lzx", "Lzy", "Lzz"]];
                    for (r, row_names) in names.iter().enumerate() {
                        let mut row = h_flex().mt(px(2.)).h(px(26.)).px(px(5.)).gap(px(8.));
                        for (c, n) in row_names.iter().enumerate() {
                            let v = m.inertia.as_ref().map(|i| i[r][c].clone()).unwrap_or_default();
                            row = row.child(h_flex().flex_1().gap(px(4.)).child(div().w(px(24.)).text_size(px(12.)).text_color(pal.text_muted).child(*n)).child(div().flex_1().h(px(24.)).flex().items_center().justify_end().border_b_1().border_color(pal.border).text_size(px(13.33)).text_color(pal.text).child(v)));
                        }
                        body = body.child(row);
                    }
                }
                None => {
                    body = body.child(field(if self.ui.mass_faces { "Faces to measure" } else { "Parts to measure" }, selected_names.clone(), true, false, false));
                    for (k, v) in rows {
                        body = body.child(value_row(k.into(), v));
                    }
                }
            }
            return vec![crate::ui::moved_panel(
                v_flex()
                    .id("mass-dialog")
                    .occlude()
                    .child(crate::ui::panel_probe("ps-mass"))
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
                            .child(crate::ui::panel_title("ps-mass", div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Mass and section properties")))
                            .child(div().size(px(28.)).flex().items_center().justify_center().bg(crate::theme::p().accept).when(true, crate::ui::disabled_accept).cursor(gpui_kit::CursorStyle::Arrow).child(crate::assets::icon("dialog-ok", 24., cx)))
                            .child(close("mass-close", false, cx)),
                    )
                    // Live: 46 px from the last inertia row to the dialog's
                    // bottom (the list's 2 px, a 22 px band, the 22 px ? row).
                    .child(body.px(px(0.)).pb(px(23.)))
                    .child(h_flex().h(px(22.)).px(px(4.)).justify_end().child(help())),
                self.ui.panels.get("ps-mass"),
            )];
        }
        // Onshape's Measure panel (live, part-studio/82): 220 px, docked at the
        // bottom right; "Measure" + ×; the selection field; Measure type,
        // Length unit, Angle unit; Reference coordinate system; then one row
        // per result with the value in monospace.
        // Live (part-studio/82, at 1×): a 12 px `#666` label, then the select
        // over the rest of the row: 14 px black text and a 7 px `#777` ▼ 7 px
        // in from the end of its `#d7d7d7` underline (5 px from the panel's
        // edge, 2 px under the row's middle).
        let select_row = |label: &'static str, menu: AnyElement| h_flex().h(px(26.)).px(px(5.)).gap(px(4.)).child(div().text_size(px(12.)).text_color(pal.text_muted).child(label)).child(div().flex_1().mt(px(2.)).child(menu));
        let select_face = |value: SharedString| h_flex().w_full().child(div().flex_1().text_size(px(14.)).text_color(gpui_kit::rgb(0x000000)).child(value)).child(crate::ui::glyph("ui-select-caret", 7., gpui_kit::rgb(0x777777)));
        let type_menu = Button::new("measure-type")
            .custom(gpui_kit::component::button::ButtonCustomVariant::new(cx).color(gpui_kit::transparent_white()).foreground(pal.text.into()).hover(pal.row_hover.into()).active(pal.accent_soft.into()).shadow(false))
            .child(select_face(current_type.into()))
            .w_full()
            .h(px(24.))
            .pl(px(4.))
            .pr(px(7.))
            .rounded(px(0.))
            .border_b_1()
            .border_color(pal.border)
            .dropdown_menu(move |mut m, _, _| {
                for t in MEASURE_TYPES {
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
        // Length unit / Angle unit: the panel's own (live: native selects;
        // the workspace unit preselected, part-studio/190).
        let munits = self.measure_units(cx);
        let unit_menu = |id: &'static str, value: String, items: Vec<UnitChoice>, cx: &mut Context<Self>| {
            let view = cx.entity();
            let items = std::rc::Rc::new(items);
            Button::new(id)
                .custom(gpui_kit::component::button::ButtonCustomVariant::new(cx).color(gpui_kit::transparent_white()).foreground(pal.text.into()).hover(pal.row_hover.into()).active(pal.accent_soft.into()).shadow(false))
                .child(select_face(value.clone().into()))
                .w_full()
                .h(px(24.))
                .pl(px(4.))
                .pr(px(7.))
                .rounded(px(0.))
                .border_b_1()
                .border_color(pal.border)
                .dropdown_menu(move |mut m, _, _| {
                    for (i, (name, _)) in items.iter().enumerate() {
                        let (v, items) = (view.clone(), items.clone());
                        m = m.item(PopupMenuItem::new(name.clone()).checked(*name == value).on_click(move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                (items[i].1)(this);
                                cx.notify();
                            })
                        }));
                    }
                    m
                })
                .into_any_element()
        };
        let length_items: Vec<UnitChoice> = MEASURE_LENGTH_UNITS
            .iter()
            .map(|&(name, u)| (name.to_string(), Box::new(move |this: &mut Self| this.ui.measure_length_unit = Some(u)) as UnitAction))
            .collect();
        let angle_items: Vec<UnitChoice> = [("Degree", noshape_units::AngleUnit::Degree), ("Radian", noshape_units::AngleUnit::Radian)]
            .into_iter()
            .map(|(name, u)| (name.to_string(), Box::new(move |this: &mut Self| this.ui.measure_angle_unit = Some(u)) as UnitAction))
            .collect();
        let length_name = MEASURE_LENGTH_UNITS.iter().find(|(_, u)| *u == munits.length).map(|(n, _)| n.to_string()).unwrap_or_else(|| format!("{:?}", munits.length));
        let angle_name = format!("{:?}", munits.angle);
        let length_menu = unit_menu("measure-length-unit", length_name, length_items, cx);
        let angle_menu = unit_menu("measure-angle-unit", angle_name, angle_items, cx);
        // Live (ui 10-23 a (e), a default plane alone): a selection with no
        // measurements says so.
        let nothing = mrows.is_empty() && !self.selection.is_empty();
        let view_rows = cx.entity();
        // The distance row an X/Y/Z row belongs to (picks its glyph).
        let mut parent = String::new();
        vec![
            v_flex()
                .id("measure-panel")
                .occlude()
                // Live (part-studio/82, at 1×): 220 px, flush with the
                // viewport's bottom-right corner, over the panel strip and
                // the measure buttons.
                .absolute()
                .bottom(px(2.))
                .right_0()
                .w(px(220.))
                .bg(pal.dialog_bg)
                .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
                .text_color(pal.text)
                .child(h_flex().h(px(29.)).pl(px(6.)).border_b_1().border_color(pal.border).child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).child(title)).child(close("measure-close", true, cx)))
                .child(field("Select entities to measure", selected_names, true, false, true).mt(px(5.)).mb(px(5.)))
                .child(select_row("Measure type:", type_menu))
                .child(select_row("Length unit:", length_menu))
                .child(select_row("Angle unit:", angle_menu))
                .child(crate::ui::os_checkbox("measure-ref", "Reference coordinate system", false))
                .child(div().h(px(5.)))
                // Live (part-studio/2244, at 1×): a 202 px info box (`#def1fb`,
                // 1 px `#b2ddf6`) 5 px in, 14 px `#094174` text on 21 px lines
                // wrapping after "for"; 15 px above the footer line.
                .when(nothing, |d| {
                    let (fill, outline, text) = crate::ui::alert_colors(crate::ui::AlertKind::Info);
                    d.child(
                        div()
                            .mt(px(6.))
                            .mb(px(9.))
                            .ml(px(5.))
                            .w(px(202.))
                            .rounded(px(2.))
                            .border_1()
                            .border_color(outline)
                            .bg(fill)
                            .pt(px(4.))
                            .pb(px(7.))
                            .px(px(11.))
                            .text_size(px(14.))
                            .line_height(px(21.))
                            .text_color(text)
                            .child("No measurements available for current selection."),
                    )
                })
                .children(mrows.into_iter().enumerate().map(|(n, (k, v, axis, var))| {
                    if axis.is_none() {
                        parent = k.to_string();
                    }
                    // "12.5 in" -> monospace number on an underline and the unit.
                    let (num, unit) = v.split_once(' ').map(|(n, u)| (n.to_string(), u.to_string())).unwrap_or((v, String::new()));
                    let value = h_flex()
                        .gap(px(2.))
                        .child(crate::ui::measure_value(num))
                        .child(div().text_size(px(12.)).child(unit));
                    // Live (part-studio/82): labels, values and units all `#666`.
                    // The (x) 9 px from the panel's edge.
                    let row = h_flex().min_h(px(26.)).pl(px(4.)).pr(px(9.)).gap(px(4.)).text_size(px(12.)).text_color(pal.text_muted);
                    let row = match axis {
                        // Live (part-studio/495): the letter, then a glyph in the axis
                        // colour: arrows under a distance, converging arrows under
                        // Min dist, an I-beam under Max dist.
                        Some(i) => {
                            let (name, color, arrow) = [("X", 0xbd3039, "axis-arrow-x"), ("Y", 0x009400, "axis-arrow-y"), ("Z", 0x1651b0, "axis-arrow-z")][i];
                            let glyph = if parent.starts_with("Min") { crate::ui::glyph("measure-min-dist", 16., gpui_kit::rgb(color)) } else if parent.starts_with("Max") { crate::ui::glyph("measure-max-dist", 16., gpui_kit::rgb(color)) } else { crate::ui::glyph(arrow, 14., gpui_kit::rgb(color)) };
                            row.pl(px(20.)).child(h_flex().child(div().text_size(px(15.)).text_color(gpui_kit::rgb(0x999999)).child(name)).child(glyph)).child(value)
                        }
                        None => row.child(div().child(format!("{k}:"))).child(value),
                    };
                    // A distance row (not its X/Y/Z sub-rows) shows its dashed
                    // line while hovered (live ui 10-23 a).
                    let distance = axis.is_none() && var.is_some_and(|v| v.0 == "DISTANCE");
                    let hover_view = view_rows.clone();
                    let label = k.to_string();
                    let row = row.id(SharedString::from(format!("measure-row-{n}"))).when(distance, |d| {
                        d.on_hover(move |h: &bool, _, cx| {
                            let label = label.clone();
                            let h = *h;
                            hover_view.update(cx, |this, cx| {
                                let next = if h { Some(label) } else if this.ui.measure_hover.as_ref() == Some(&label) { None } else { this.ui.measure_hover.clone() };
                                if next != this.ui.measure_hover {
                                    this.ui.measure_hover = next;
                                    this.refresh_scene(false, cx);
                                }
                            });
                        })
                    });
                    // Live (behaviours.md: each row has an (x) "create variable"
                    // icon at the right).
                    let view = view_rows.clone();
                    row.child(div().flex_1()).children(var.map(|var| {
                        div()
                            .id(SharedString::from(format!("measure-var-{n}")))
                            .cursor_pointer()
                            .child(crate::assets::icon("variable", 14., cx))
                            .tooltip(crate::ui::tooltip("Add variable for this measurement"))
                            .on_click(move |_, window, cx| view.update(cx, |this, cx| this.create_variable_from_measure(var, axis, window, cx)))
                    }))
                }))
                // Live: 24 px from the last row's text to a 1 px `#d7d7d7` line
                // over a 21 px `#eee` band with the ?.
                .child(div().h(px(6.)))
                .child(h_flex().h(px(22.)).px(px(4.)).justify_end().border_t_1().border_color(pal.border).bg(gpui_kit::rgb(0xeeeeee)).child(help()))
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

/// Kind of a measurement for the measure type selector.
/// A Measure panel row: label, value, X/Y/Z sub-row axis, and the (x)
/// variable it creates.
type MeasureRow = (String, String, Option<usize>, Option<MeasureVar>);

/// What a Measure row's (x) measures: `DISTANCE`, `LENGTH`, `DIAMETER` or
/// `RADIUS`, and whether it is a maximum distance.
type MeasureVar = (&'static str, bool);

/// A Measure panel unit choice: its name and what picking it does.
type UnitAction = Box<dyn Fn(&mut PartStudioView)>;
type UnitChoice = (String, UnitAction);

/// The Measure panel's types (bundle: the Measure type select).
const MEASURE_TYPES: [&str; 12] = ["Show all", "Position", "Center position", "Min distance", "Max distance", "Length", "Diameter", "Radius", "Angle", "Face tangent angle", "Curvature deviation", "Surface area"];

/// The panel's Length unit choices (live order).
const MEASURE_LENGTH_UNITS: [(&str, noshape_units::LengthUnit); 6] = [
    ("Centimeter", noshape_units::LengthUnit::Centimeter),
    ("Foot", noshape_units::LengthUnit::Foot),
    ("Inch", noshape_units::LengthUnit::Inch),
    ("Meter", noshape_units::LengthUnit::Meter),
    ("Millimeter", noshape_units::LengthUnit::Millimeter),
    ("Yard", noshape_units::LengthUnit::Yard),
];

/// The Measure type a row belongs to (bundle: the map from each type to
/// its measurement types; Angle keeps every angle; the other distances only
/// show with Show all).
fn measure_filter_type(label: &str) -> Option<&'static str> {
    Some(match label {
        "Point" => "Position",
        "Center" => "Center position",
        "Min dist" => "Min distance",
        "Max dist" => "Max distance",
        "Length" => "Length",
        "Diameter" => "Diameter",
        "Radius" => "Radius",
        "Angle" | "Perp planes angle" | "Perp lines angle" | "Perp axes angle" | "Perp line and plane angle" => "Angle",
        "Curvature deviation" => "Curvature deviation",
        "Area" => "Surface area",
        _ => return None,
    })
}

/// A measured value as Onshape's measure readout and panel write it (bundle
/// `formatValue`, live part-studio/81: "90.000 deg", "11.902 in²"): the
/// display precision with trailing zeros kept, "deg" for degrees.
fn measure_fmt_in(q: Quantity, units: &noshape_units::UnitPrefs) -> String {
    let angle = q.dim == Dim::ANGLE;
    let decimals = usize::from(if angle { units.angle_precision } else { units.length_precision });
    let mut num = noshape_units::to_fixed(q.value / units.si_factor(q.dim), decimals as u8);
    if num.starts_with('-') && num[1..].chars().all(|c| c == '0' || c == '.') {
        num.remove(0);
    }
    let unit = if angle {
        units.angle.symbol().to_string()
    } else {
        let power = match q.dim.length {
            2 => "²",
            3 => "³",
            _ => "",
        };
        format!("{}{power}", units.length.symbol())
    };
    format!("{num} {unit}")
}

/// The notice for parts without a material (live F19, behaviours.md "Mass
/// properties without material + the material-not-defined notice").
const NO_MATERIAL: &str = "One or more parts do not have a material defined.";

/// Live shows the notice once per page load: once per app session here.
static NO_MATERIAL_SHOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl PartStudioView {
    /// Open or close the Mass properties panel. A part without a material
    /// has a red chip and blank mass, centre of mass and inertia; Onshape's
    /// yellow top-centre toast "One or more parts do not have a material
    /// defined." follows ≈3.6 s after the panel opens (when its computation
    /// is done) and hides itself ≈4.6 s later, once per session, whether or
    /// not the panel is still open (qa4 F19).
    pub(super) fn set_show_mass(&mut self, show: bool, cx: &mut Context<Self>) {
        self.show_mass = show;
        if show && !NO_MATERIAL_SHOWN.load(std::sync::atomic::Ordering::Relaxed) {
            let props = self.studio(cx).map(|s| s.part_properties.clone()).unwrap_or_default();
            if self.display.bodies.iter().any(|b| props.get(&b.id).and_then(|p| p.density).is_none()) {
                NO_MATERIAL_SHOWN.store(true, std::sync::atomic::Ordering::Relaxed);
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(std::time::Duration::from_millis(3600)).await;
                    let _ = this.update(cx, |this, cx| this.flash_for(noshape_model::display::FeatureStatus::Warning, NO_MATERIAL.into(), std::time::Duration::from_millis(4600), cx));
                })
                .detach();
            }
        }
        cx.notify();
    }
}

/// The Part Studio's measurements of a selection: no part volume (live, qa5
/// RD1: the volume only shows in Mass properties; the readout's went stale).
pub(super) fn studio_measure(ctx: &noshape_model::regen::context::Context, sel: &[noshape_model::reference::Reference]) -> Vec<(String, measure::Measurement)> {
    measure::measure(ctx, sel).into_iter().filter(|(_, m)| !matches!(m, measure::Measurement::Volume(_))).collect()
}
