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
use gpui_kit::component::{IconName, Sizable as _, h_flex, v_flex};
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
                            })))
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
                        m = m.separator().item(icon_item(&format!("Copy {body_name}"), "copy", Box::new(move |this, _, cx| this.copy_part(bc.clone(), cx))));
                        // Drawings and export.
                        let (b7, b8, name) = (b.clone(), b.clone(), body_name.to_string());
                        m = m
                            .separator()
                            .item(icon_item(&format!("Create Drawing of {body_name}…"), "drawing", Box::new(move |this, w, cx| {
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
                    m = m.item(item("Show all", Box::new(|this, _, cx| this.show_everything(cx))));
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
        let chosen = std::sync::Arc::new(parking_lot::Mutex::new(super::ExportFormat::Step));
        let base = match bodies.as_slice() {
            [b] => self.display.body(b).map(|d| d.name.clone()).unwrap_or_else(|| "Part".into()),
            _ => self.studio_name(cx),
        };
        let pal = p();
        window.open_dialog(cx, move |dialog, _, _| {
            let current = *chosen.lock();
            let formats = super::ExportFormat::ALL.iter().map(|f| {
                let (f, c) = (*f, chosen.clone());
                Button::new(SharedString::from(format!("exp-{}", f.ext())))
                    .label(f.label())
                    .xsmall()
                    .when_selected(f == current)
                    .on_click(move |_, window, _| {
                        *c.lock() = f;
                        window.refresh();
                    })
            });
            let (v, c, bodies, base) = (view.clone(), chosen.clone(), bodies.clone(), base.clone());
            dialog
                .title("Export")
                .w(px(420.))
                .child(v_flex().gap_1().child(div().text_xs().text_color(pal.text_muted).child("Format")).child(h_flex().gap_1().flex_wrap().children(formats)))
                .footer(
                    DialogFooter::new()
                        .gap_2()
                        .child(DialogClose::new().child(Button::new("cancel").label("Cancel").outline()))
                        .child(DialogAction::new().child(Button::new("ok").label("Export").primary())),
                )
                .on_ok(move |_, window, cx| {
                    let format = *c.lock();
                    let (v, bodies) = (v.clone(), bodies.clone());
                    crate::files::request_save_path("Export", format!("{base}.{}", format.ext()), format.ext(), window, cx, move |path, cx| {
                        v.update(cx, |this, cx| {
                            let msg = match this.export_bodies(format, &path, &bodies) {
                                Ok(()) => format!("Exported {}", path.display()),
                                Err(e) => format!("Export failed: {e:#}"),
                            };
                            this.flash_info(msg, cx);
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
            .child(btn("eval-analysis", "analysis-tools", self.ui.analysis.is_some(), cx).tooltip("Analysis tools").dropdown_menu(move |m, _, _| {
                let (v1, v2) = (view.clone(), view.clone());
                m.item(PopupMenuItem::new("Curvature analysis").on_click(move |_, w, cx| v1.update(cx, |this, cx| this.toggle_analysis(super::analysis::Analysis::Curvature, w, cx))))
                    .item(PopupMenuItem::new("Draft analysis").on_click(move |_, w, cx| {
                        v2.update(cx, |this, cx| {
                            let a = this.draft_analysis();
                            this.toggle_analysis(a, w, cx)
                        })
                    }))
            }))
            .child(
                btn("eval-mass", "mass-properties", mass_on, cx).tooltip("Mass properties").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.show_mass = !this.show_mass;
                    this.ui.measure_pinned = false;
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
                rows.push(("Volume".into(), self.fmt(Quantity::new(mp.volume, len3), cx)));
                rows.push(("Surface area".into(), self.fmt(Quantity::new(mp.surface_area, len2), cx)));
                rows.push(("Mass".into(), mp.mass.map(|m| format!("{m:.4} kg{}", if overrides.is_some() { " (override)" } else { "" })).unwrap_or_else(|| "assign a material".into())));
                let c = mp.center_of_mass;
                let l = |x: f64| self.fmt(Quantity::new(x, Dim::LENGTH), cx);
                rows.push(("Center of mass".into(), format!("{}, {}, {}", l(c.x), l(c.y), l(c.z))));
                // Onshape lists the moments of inertia about the center of mass
                // (needs a material, like the mass).
                if mp.mass.is_some() {
                    let units = self.doc.read(cx).session.doc.settings.units;
                    let f = units.si_factor(Dim::LENGTH).powi(2);
                    let sym = units.length.symbol();
                    let i = mp.inertia;
                    let v = |x: f64| format!("{:.4} kg·{sym}²", x / f);
                    rows.push(("Ixx / Iyy / Izz".into(), format!("{} / {} / {}", v(i.x_axis.x), v(i.y_axis.y), v(i.z_axis.z))));
                    rows.push(("Ixy / Ixz / Iyz".into(), format!("{} / {} / {}", v(i.y_axis.x), v(i.z_axis.x), v(i.z_axis.y))));
                }
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
        let type_menu = (!self.show_mass).then(|| {
            Button::new("measure-type").label(current_type).ghost().xsmall().dropdown_caret(true).dropdown_menu(move |mut m, _, _| {
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
        });
        let close = Button::new("measure-close").icon(IconName::Close).ghost().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
            this.show_mass = false;
            this.ui.measure_pinned = false;
            this.selection.clear();
            this.refresh_scene(false, cx);
            cx.notify();
        }));
        vec![
            v_flex()
                .id("measure-panel")
                .occlude()
                .absolute()
                .bottom(px(46.))
                .right_3()
                .w(px(280.))
                .p_2()
                .gap_1()
                .rounded_lg()
                .bg(with_alpha(pal.panel_raised, 0.95))
                .border_1()
                .border_color(pal.border_strong)
                .shadow_lg()
                .child(h_flex().justify_between().child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text_muted).child(title.to_uppercase())).child(h_flex().children(type_menu).child(close)))
                .when(self.show_mass, |d| {
                    // Part / Face tabs (Onshape mass properties dialog).
                    let tab = |id: &'static str, label: &'static str, faces: bool, cx: &mut Context<Self>| {
                        let on = self.ui.mass_faces == faces;
                        div()
                            .id(id)
                            .px_2()
                            .text_xs()
                            .cursor_pointer()
                            .text_color(if on { pal.accent } else { pal.text_muted })
                            .when(on, |d| d.border_b_1().border_color(pal.accent))
                            .child(label)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.ui.mass_faces = faces;
                                cx.notify();
                            }))
                    };
                    d.child(h_flex().gap_1().child(tab("mass-tab-part", "Part", false, cx)).child(tab("mass-tab-face", "Face", true, cx)))
                })
                .children(rows.into_iter().map(|(k, v)| {
                    h_flex().justify_between().gap_2().child(div().text_xs().text_color(pal.text_muted).child(k)).child(div().text_xs().text_color(pal.text).child(v))
                }))
                .into_any_element(),
        ]
    }
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

