//! Onshape interaction behaviors of the Part Studio that are not tied to a
//! single panel: hide/show (Y, Shift+Y), isolate / make transparent
//! (Shift+I, Shift+T), Select other (`), the S shortcut toolbar, previous
//! view and zoom to selection/window, unified picking with filter feedback,
//! cross-highlighting between the feature list and the graphics area,
//! feature-list reordering rules and the automatic Add/Remove choices of a
//! new Extrude.

use super::{PartStudioView, feature_icon, kind_matches};
use crate::scene_builder::{PickKind, PickTarget};
use crate::theme::{p, with_alpha};
use glam::DVec2;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::*;
use gpui_kit::prelude::FluentBuilder as _;
use noshape_model::display::{FeatureStatus, StudioDisplay};
use noshape_model::interaction;
use noshape_model::params::{ParamValue, QueryFilter};
use noshape_model::reference::{BodyId, FeatureId, Reference};
use noshape_render::Camera;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Feature tools on the S shortcut toolbar (Part Studio).
const MINI_FEATURES: &[&str] = &["sketch", "extrude", "revolve", "sweep", "loft", "fillet", "chamfer", "shell", "hole", "mirror", "linearPattern", "circularPattern", "cPlane", "booleanBodies"];

/// Isolate (Shift+I) keeps the chosen parts opaque; Make transparent
/// (Shift+T) makes them transparent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IsolateMode {
    Isolate,
    Transparent,
}

pub struct Isolation {
    pub mode: IsolateMode,
    pub bodies: Vec<BodyId>,
    /// Opacity of the transparent parts (Onshape's transparency slider).
    pub opacity: f32,
    /// "Select transparent geometry": picks may land on transparent parts.
    pub select_transparent: bool,
    /// "Expand: distance": this many nearest parts join the chosen ones.
    pub expand: usize,
}

/// Open "Select other" list: candidates nearest first, highlighted index.
pub struct SelectOther {
    pub hits: Vec<Reference>,
    pub index: usize,
    /// Viewport-local position the list opens at.
    pub at: DVec2,
}

/// View state for the behaviors above.
pub struct UiState {
    pub isolate: Option<Isolation>,
    /// Cameras before discrete view changes (Previous view).
    pub cam_history: Vec<Camera>,
    /// Last pointer position over the viewport (viewport-local).
    pub pointer: Option<DVec2>,
    pub select_other: Option<SelectOther>,
    /// S shortcut toolbar position (viewport-local).
    pub mini_toolbar: Option<DVec2>,
    /// Feature list search box.
    pub filter: Option<(Entity<InputState>, Subscription)>,
    pub filter_query: String,
    /// Feature row under the pointer (its geometry is highlighted).
    pub list_hover: Option<FeatureId>,
    /// Last clicked feature (anchor of Shift+click ranges).
    pub anchor: Option<FeatureId>,
    /// Last viewport click (double-click detection).
    pub last_click: Option<(Reference, Instant)>,
    /// Focus of the feature list (rollback bar arrow keys).
    pub list_focus: FocusHandle,
    /// "Show dependencies" panel for a feature.
    pub dependencies: Option<FeatureId>,
    pub cube_hover: Option<super::view_cube::CubeCell>,
    pub cube_origin: Option<Point<Pixels>>,
    /// Measure details pinned open with `[`.
    pub measure_pinned: bool,
    /// Toolbar groups whose face is the last tool picked from the dropdown.
    pub toolbar_faces: std::collections::HashMap<&'static str, &'static str>,
    /// Search tools: name and description of the hovered result.
    pub palette_hover: Option<(String, String)>,
    /// Variable hovered in the variable table.
    pub var_hover: Option<String>,
    /// Section view planes (Shift+X).
    pub section: Vec<super::section::SectionPlane>,
    /// Open Appearance editor, and faces of a hovered appearance row.
    pub appearance_edit: Option<super::appearance::AppearanceEdit>,
    pub appearance_hover: Vec<Reference>,
    /// Active analysis color map (Shift+C / Shift+D).
    pub analysis: Option<super::analysis::Analysis>,
    /// Per-vertex colors for the current display: (key, colors).
    pub vcolor_cache: Option<((usize, String), Option<super::appearance::VertexColors>)>,
    /// View in high quality (Shift+R).
    pub high_quality: bool,
    /// Create selection panel open.
    pub create_selection: bool,
    /// Last selection-badge position update (time, position).
    pub badge_at: Option<(Instant, DVec2)>,
    /// Paused regeneration: the studio when paused (for Discard).
    pub regen_paused: Option<noshape_model::partstudio::PartStudio>,
    /// Run one regeneration although paused.
    pub force_regen: bool,
    /// Regeneration times panel.
    pub show_regen_times: bool,
    /// Parts list: collapsed sections, height, part properties shown.
    pub collapsed_sections: HashSet<&'static str>,
    pub parts_height: f32,
    pub show_part_props: bool,
    /// The whole Features and parts list collapsed.
    pub list_collapsed: bool,
    /// Mass properties panel on its Face tab.
    pub mass_faces: bool,
    /// Measure type filter (None = Show all).
    pub measure_type: Option<&'static str>,
    /// Feature dialog position after the user moved it.
    pub dialog_pos: Option<Point<Pixels>>,
    /// Toolbar customization (removed tools, group order, S toolbar).
    pub toolbar: super::toolbar_prefs::ToolbarPrefs,
    pub customizing_toolbar: bool,
    /// Preview slider value and the model before the edited feature.
    pub preview_opacity: f32,
    pub preview_before: Option<Arc<StudioDisplay>>,
    /// Regeneration context before the edited feature.
    pub before_context: Option<Arc<noshape_model::regen::context::Context>>,
    pub preview_task: Option<Task<()>>,
    /// Measure graphics uploaded for a selection.
    pub measure_lines: Option<(Vec<Reference>, Option<noshape_render::Instance>)>,
    /// Dynamic suppression editor: feature, expression input.
    pub dyn_suppression: Option<(FeatureId, Entity<InputState>, Subscription)>,
    /// Shortcut map search box.
    pub shortcut_filter: Option<(Entity<InputState>, Subscription)>,
    pub shortcut_query: String,
    /// Recently started tools, newest first (Search tools with no query).
    pub recent_tools: Vec<&'static str>,
    /// Camera menu "Orient normal to sketch on edit" and its state: an
    /// orientation is pending until the sketch editor opens; `oriented`
    /// restores the previous view when the sketch closes.
    pub orient_on_edit: bool,
    pub orient_pending: bool,
    pub oriented: bool,
    /// Values of REMEMBER_PREVIOUS_VALUE parameters from the last accepted
    /// feature of each type (Onshape defaults new features to them).
    pub remembered: std::collections::HashMap<(String, &'static str), ParamValue>,
    /// Feature list: "Default geometry" and "Parts" folders collapsed.
    pub default_geometry_collapsed: bool,
}

impl UiState {
    pub fn new(cx: &mut App) -> Self {
        Self {
            isolate: None,
            cam_history: Vec::new(),
            pointer: None,
            select_other: None,
            mini_toolbar: None,
            filter: None,
            filter_query: String::new(),
            list_hover: None,
            anchor: None,
            last_click: None,
            list_focus: cx.focus_handle(),
            dependencies: None,
            cube_hover: None,
            cube_origin: None,
            measure_pinned: false,
            toolbar_faces: Default::default(),
            palette_hover: None,
            var_hover: None,
            section: Vec::new(),
            appearance_edit: None,
            appearance_hover: Vec::new(),
            analysis: None,
            vcolor_cache: None,
            high_quality: false,
            create_selection: false,
            badge_at: None,
            regen_paused: None,
            force_regen: false,
            show_regen_times: false,
            collapsed_sections: HashSet::new(),
            parts_height: 180.0,
            show_part_props: false,
            list_collapsed: false,
            mass_faces: false,
            measure_type: None,
            dialog_pos: None,
            toolbar: super::toolbar_prefs::ToolbarPrefs::load(),
            customizing_toolbar: false,
            preview_opacity: 1.0,
            preview_before: None,
            before_context: None,
            preview_task: None,
            measure_lines: None,
            dyn_suppression: None,
            shortcut_filter: None,
            shortcut_query: String::new(),
            recent_tools: Vec::new(),
            orient_on_edit: false,
            orient_pending: false,
            oriented: false,
            remembered: Default::default(),
            default_geometry_collapsed: false,
        }
    }
}

/// What a selection field accepts, for the rejection message.
pub fn filter_description(f: QueryFilter) -> String {
    let mut kinds: Vec<&str> = Vec::new();
    let mut add = |q: QueryFilter, s: &'static str| {
        if f.intersects(q) && !kinds.contains(&s) {
            kinds.push(s);
        }
    };
    add(QueryFilter::SKETCH_REGION, "sketch regions");
    if f.intersects(QueryFilter::FACE) {
        add(QueryFilter::FACE, "faces");
    } else {
        add(QueryFilter::PLANAR_FACE, "planar faces");
        add(QueryFilter::CYLINDRICAL_FACE, "cylindrical faces");
    }
    add(QueryFilter::PLANE, "planes");
    if f.intersects(QueryFilter::EDGE) {
        add(QueryFilter::EDGE, "edges");
    } else {
        add(QueryFilter(QueryFilter::LINEAR_EDGE.0 | QueryFilter::CIRCULAR_EDGE.0), "edges");
    }
    add(QueryFilter::SKETCH_EDGE, "sketch curves");
    add(QueryFilter(QueryFilter::VERTEX.0 | QueryFilter::SKETCH_POINT.0 | QueryFilter::POINT_BODY.0), "points");
    add(QueryFilter::BODY, "parts");
    add(QueryFilter::WIRE_BODY, "curves");
    add(QueryFilter::MATE_CONNECTOR, "mate connectors");
    add(QueryFilter::FEATURE, "features");
    add(QueryFilter::SKETCH, "sketches");
    match kinds.len() {
        0 => "That entity can't be used in the active field.".into(),
        1 => format!("Select {}.", kinds[0]),
        n => format!("Select {} or {}.", kinds[..n - 1].join(", "), kinds[n - 1]),
    }
}

impl PartStudioView {
    // ------------------------------------------------------------ helpers

    /// Body owning a reference (faces, edges, vertices, the body itself).
    pub(super) fn body_of(&self, r: &Reference) -> Option<BodyId> {
        match r {
            Reference::Body(b) => Some(b.clone()),
            _ => self.display.bodies.iter().find(|b| b.face_refs.contains(r) || b.edge_refs.contains(r) || b.vertex_refs.contains(r)).map(|b| b.id.clone()),
        }
    }

    /// Entities that light up when a feature is hovered or selected in the
    /// list: its sketch, plane, mate connectors and the faces it created.
    pub(super) fn feature_geometry(&self, id: FeatureId) -> Vec<Reference> {
        let mut out = Vec::new();
        if let Some(sk) = self.display.sketches.iter().find(|s| s.feature == id) {
            out.extend(sk.curves.iter().map(|c| Reference::SketchEntity { sketch: id, entity: c.entity }));
            out.extend(sk.points.iter().map(|c| Reference::SketchEntity { sketch: id, entity: c.entity }));
        }
        out.extend(self.display.planes.iter().filter(|p| p.reference == Reference::FeaturePlane(id)).map(|p| p.reference.clone()));
        out.extend(self.display.connectors.iter().filter(|c| c.reference.depends_on() == Some(id)).map(|c| c.reference.clone()));
        for b in self.display.bodies.iter().filter(|b| !b.hidden) {
            out.extend(b.face_refs.iter().filter(|r| r.depends_on() == Some(id)).cloned());
        }
        out
    }

    /// Faces standing for a whole part (parts list hover/selection).
    pub(super) fn body_geometry(&self, id: &BodyId) -> Vec<Reference> {
        self.display.body(id).map(|b| b.face_refs.clone()).unwrap_or_default()
    }

    /// Selection plus the geometry of selected features and parts (for the
    /// graphics-area highlight).
    pub(super) fn highlighted_selection(&self) -> Vec<Reference> {
        let mut out = Vec::new();
        for r in &self.selection {
            match r {
                Reference::Body(b) => out.extend(self.body_geometry(b)),
                Reference::Sketch(f) | Reference::Feature(f) => out.extend(self.feature_geometry(*f)),
                _ => out.push(r.clone()),
            }
        }
        for f in &self.selected_features {
            out.extend(self.feature_geometry(*f));
        }
        out
    }

    /// Hover target in the graphics area, expanded like the selection.
    pub(super) fn highlighted_hover(&self, cx: &App) -> Vec<Reference> {
        if let Some(f) = self.ui.list_hover {
            return self.feature_geometry(f);
        }
        if self.ui.var_hover.is_some() {
            return self.variable_users(cx).into_iter().flat_map(|f| self.feature_geometry(f)).collect();
        }
        match &self.hover {
            Some(Reference::Body(b)) => self.body_geometry(b),
            Some(r) => vec![r.clone()],
            None => Vec::new(),
        }
    }

    /// Features related to the current selection (Onshape cross-highlight:
    /// selecting a face or part highlights the features that created it).
    /// Features using the variable hovered in the variable table.
    fn variable_users(&self, cx: &App) -> HashSet<FeatureId> {
        let (Some(name), Some(studio)) = (self.ui.var_hover.as_ref(), self.studio(cx)) else { return HashSet::new() };
        interaction::filter_features(studio, &self.display, &format!(":variable {name}")).unwrap_or_default()
    }

    pub(super) fn related_features(&self, cx: &App) -> HashSet<FeatureId> {
        let mut out = self.variable_users(cx);
        for r in &self.selection {
            match r {
                Reference::Body(b) => {
                    out.insert(b.feature);
                    if let Some(bd) = self.display.body(b) {
                        out.extend(bd.face_refs.iter().filter_map(|f| f.depends_on()));
                    }
                }
                Reference::Edge(n) | Reference::Vertex(n) => {
                    if let Some(f) = super::edge_creator(n) {
                        out.insert(f);
                    }
                }
                r => {
                    if let Some(f) = r.depends_on() {
                        out.insert(f);
                    }
                }
            }
        }
        out
    }

    // ------------------------------------------------------------ picking

    /// One pick from the graphics area or a list: goes to the active dialog
    /// field (with filter feedback) or toggles the selection.
    pub(super) fn pick(&mut self, target: PickTarget, cx: &mut Context<Self>) {
        if let Some(e) = &self.edit {
            let dialog = e.dialog.clone();
            let Some((_, filter, _)) = dialog.read(cx).active_filter() else {
                self.flash_info("Click a selection field in the dialog first.".into(), cx);
                return;
            };
            if !kind_matches(target.kind, &target.reference, filter) {
                // Clicking a sketch curve or point while a region field is
                // active takes the sketch's regions (like clicking the
                // sketch in the feature list).
                if let (Reference::SketchEntity { sketch, .. }, true) = (&target.reference, filter.intersects(QueryFilter::SKETCH_REGION))
                    && self.pick_feature_into_dialog(*sketch, cx)
                {
                    return;
                }
                dialog.update(cx, |d, cx| d.flash_active_field(cx));
                self.flash_info(filter_description(filter), cx);
                return;
            }
            dialog.update(cx, |d, cx| {
                d.add_pick(target.reference.clone(), cx);
            });
        } else if let Some(pos) = self.selection.iter().position(|r| *r == target.reference) {
            self.selection.remove(pos);
        } else {
            self.selection.push(target.reference);
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Pick target for a reference shown in a list (default geometry, parts).
    pub(super) fn list_target(&self, r: &Reference) -> PickTarget {
        let kind = match r {
            Reference::Origin => PickKind::Origin,
            Reference::Plane(_) | Reference::FeaturePlane(_) => PickKind::Plane,
            _ => PickKind::Face,
        };
        PickTarget { reference: r.clone(), kind, anchor: None }
    }

    /// Parts list click: dialog field (parts only) or additive selection.
    pub(super) fn pick_body(&mut self, b: BodyId, cx: &mut Context<Self>) {
        let r = Reference::Body(b);
        if let Some(e) = &self.edit {
            let dialog = e.dialog.clone();
            let Some((_, filter, _)) = dialog.read(cx).active_filter() else { return };
            if !filter.intersects(QueryFilter::BODY) {
                dialog.update(cx, |d, cx| d.flash_active_field(cx));
                self.flash_info(filter_description(filter), cx);
                return;
            }
            dialog.update(cx, |d, cx| {
                d.add_pick(r, cx);
            });
        } else if let Some(pos) = self.selection.iter().position(|x| *x == r) {
            self.selection.remove(pos);
        } else {
            self.selection.push(r);
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Box selection result: into the active dialog field (filtered) or
    /// the selection; Ctrl+drag deselects (Onshape).
    pub(super) fn box_select(&mut self, ids: &[u32], deselect: bool, cx: &mut Context<Self>) {
        let Some(built) = &self.built else { return };
        let targets: Vec<PickTarget> = ids.iter().filter_map(|id| built.picks.get(*id).cloned()).filter(|t| !matches!(t.kind, PickKind::Plane | PickKind::Origin)).collect();
        if let Some(e) = &self.edit {
            let dialog = e.dialog.clone();
            if let Some((_, filter, _)) = dialog.read(cx).active_filter() {
                let current = dialog.read(cx).selected_refs();
                dialog.update(cx, |d, cx| {
                    for t in targets.iter().filter(|t| kind_matches(t.kind, &t.reference, filter)) {
                        if current.contains(&t.reference) == deselect {
                            d.add_pick(t.reference.clone(), cx);
                        }
                    }
                });
            }
        } else {
            for t in targets.iter().filter(|t| !matches!(t.kind, PickKind::Vertex | PickKind::SketchRegion)) {
                let pos = self.selection.iter().position(|r| *r == t.reference);
                match (pos, deselect) {
                    (Some(p), true) => {
                        self.selection.remove(p);
                    }
                    (None, false) => self.selection.push(t.reference.clone()),
                    _ => {}
                }
            }
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Double-click on a sketch in the graphics area opens it for editing.
    pub(super) fn is_double_click(&mut self, r: &Reference) -> bool {
        let now = Instant::now();
        let dbl = self.ui.last_click.as_ref().is_some_and(|(prev, t)| prev == r && now.duration_since(*t) < Duration::from_millis(450));
        self.ui.last_click = if dbl { None } else { Some((r.clone(), now)) };
        dbl
    }

    // ------------------------------------------------------------ preselection

    /// References standing for the selected features, for preselection:
    /// a selected sketch preselects its regions (or the sketch, or its
    /// curves) and other features themselves.
    pub(super) fn feature_preselection(&self, fields: &[QueryFilter]) -> Vec<Reference> {
        let wants = |q: QueryFilter| fields.iter().any(|f| f.intersects(q));
        let mut out = Vec::new();
        for &id in &self.selected_features {
            if let Some(sk) = self.display.sketches.iter().find(|s| s.feature == id) {
                if wants(QueryFilter::SKETCH_REGION) && !sk.regions.is_empty() {
                    out.extend(sk.regions.iter().map(|r| Reference::SketchRegion { sketch: id, key: r.key.clone() }));
                } else if wants(QueryFilter::SKETCH) {
                    out.push(Reference::Sketch(id));
                } else if wants(QueryFilter::SKETCH_EDGE) {
                    out.extend(sk.curves.iter().map(|c| Reference::SketchEntity { sketch: id, entity: c.entity }));
                }
            } else if let Some(pl) = self.display.planes.iter().find(|p| p.reference == Reference::FeaturePlane(id)) {
                out.push(pl.reference.clone());
            } else if wants(QueryFilter::FEATURE) {
                out.push(Reference::Feature(id));
            }
        }
        out
    }

    // ------------------------------------------------------------ sketches

    /// Context menu "Copy sketch" (pasted onto a plane with "Paste sketch").
    pub(super) fn copy_sketch(&mut self, id: FeatureId, cx: &mut Context<Self>) {
        let prev = std::mem::replace(&mut self.selected_features, vec![id]);
        self.copy_selected_features(cx);
        self.selected_features = prev;
    }

    /// "Paste sketch" on a plane or planar face: paste the copied sketch(es)
    /// and move them onto that plane.
    pub(super) fn paste_sketch_onto(&mut self, plane: Reference, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|c| c.text()) else { return };
        let Ok(mut v) = serde_json::from_str::<serde_json::Value>(&text) else { return };
        let Some(Ok(features)) = v.get_mut("noshape_features").map(|f| serde_json::from_value::<Vec<noshape_model::partstudio::Feature>>(f.take())) else { return };
        let sketches: Vec<_> = features.into_iter().filter(|f| f.type_id == "sketch").collect();
        if sketches.is_empty() {
            self.flash_info("Copy a sketch first (right-click a sketch > Copy sketch).".into(), cx);
            return;
        }
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Paste sketch", cx, |ps| {
                let at = ps.rollback_index();
                for id in ps.paste_features(&sketches, at) {
                    if let Some(f) = ps.feature_mut(id) {
                        f.params.set("sketchPlane", ParamValue::Query(vec![plane.clone()]));
                    }
                }
            });
        });
    }

    /// "Export as DXF/DWG" for a sketch (DXF, millimeters, sketch plane
    /// coordinates; construction curves as hidden lines).
    pub(super) fn export_sketch(&mut self, id: FeatureId, ext: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sk) = self.display.sketches.iter().find(|s| s.feature == id) else { return };
        let g = sketch_sheet(sk);
        let name = sk.name.clone();
        let view = cx.entity();
        crate::files::request_save_path("Export sketch", format!("{name}.{ext}"), ext, window, cx, move |path, cx| {
            let data = if ext == "svg" { noshape_model::drawing::export::to_svg(&g) } else { noshape_model::drawing::export::to_dxf(&g) };
            let msg = match std::fs::write(&path, data) {
                Ok(()) => format!("Exported {}", path.display()),
                Err(e) => format!("Export failed: {e}"),
            };
            view.update(cx, |this, cx| this.flash_info(msg, cx));
        });
    }

    // ------------------------------------------------------------ dynamic suppression

    pub(super) fn set_suppression_expr(&mut self, id: FeatureId, expr: Option<String>, cx: &mut Context<Self>) {
        let expr = expr.filter(|e| !e.trim().is_empty());
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, if expr.is_some() { "Dynamic suppression" } else { "Remove dynamic suppression" }, cx, |ps| {
                if let Some(f) = ps.feature_mut(id) {
                    f.suppression_expr = expr;
                }
            });
        });
    }

    /// Open the Dynamic suppression editor (an expression such as
    /// `#depth < 0.5 in`; the feature is suppressed while it is true).
    pub(super) fn edit_dynamic_suppression(&mut self, id: FeatureId, window: &mut Window, cx: &mut Context<Self>) {
        let current = self.studio(cx).and_then(|s| s.feature(id)).and_then(|f| f.suppression_expr.clone()).unwrap_or_default();
        let st = cx.new(|cx| InputState::new(window, cx).default_value(current).placeholder("e.g. #depth < 0.5 in"));
        let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, window, cx| {
            if let InputEvent::Change = ev {
                cx.notify();
            }
            if let InputEvent::PressEnter { .. } = ev {
                let text = st.read(cx).value().to_string();
                this.set_suppression_expr(id, Some(text), cx);
                this.ui.dyn_suppression = None;
                window.focus(&this.focus, cx);
                cx.notify();
            }
        });
        st.update(cx, |s, cx| s.focus(window, cx));
        self.ui.dyn_suppression = Some((id, st, sub));
        cx.notify();
    }

    pub(super) fn render_dynamic_suppression(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (id, st, _) = self.ui.dyn_suppression.as_ref()?;
        let (id, st) = (*id, st.clone());
        let pal = p();
        let name = self.studio(cx).and_then(|s| s.feature(id)).map(|f| f.name.clone()).unwrap_or_default();
        let text = st.read(cx).value().to_string();
        let scope = self.display.variables.clone();
        let preview = (!text.trim().is_empty()).then(|| match noshape_model::regen::eval_bool(&text, &scope) {
            Ok(true) => ("Currently true: suppressed".to_string(), pal.text_muted),
            Ok(false) => ("Currently false: not suppressed".to_string(), pal.text_muted),
            Err(e) => (e, pal.error),
        });
        Some(
            v_flex()
                .id("dyn-suppression")
                .occlude()
                .absolute()
                .top_3()
                .left(px(340.))
                .w(px(280.))
                .p_2()
                .gap_1()
                .rounded_md()
                .bg(pal.panel_raised)
                .border_1()
                .border_color(pal.border_strong)
                .shadow_lg()
                .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                    if ev.keystroke.key == "escape" {
                        this.ui.dyn_suppression = None;
                        window.focus(&this.focus, cx);
                        cx.notify();
                    }
                }))
                .child(
                    h_flex()
                        .gap_2()
                        .child(div().flex_1().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(format!("Dynamic suppression: {name}")))
                        .child(div().id("dyn-ok").px_1().cursor_pointer().text_color(pal.accent).child("✓").on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            let text = this.ui.dyn_suppression.as_ref().map(|(_, s, _)| s.read(cx).value().to_string()).unwrap_or_default();
                            this.set_suppression_expr(id, Some(text), cx);
                            this.ui.dyn_suppression = None;
                            cx.notify();
                        })))
                        .child(div().id("dyn-x").px_1().cursor_pointer().text_color(pal.text_faint).child("×").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.ui.dyn_suppression = None;
                            cx.notify();
                        }))),
                )
                .child(div().text_xs().text_color(pal.text_muted).child("Suppress the feature while this expression is true"))
                .child(gpui_kit::component::input::Input::new(&st))
                .children(preview.map(|(t, c)| div().text_xs().text_color(c).child(t)))
                .into_any_element(),
        )
    }

    // ------------------------------------------------------------ regeneration

    /// Pause regeneration on / off (off regenerates).
    pub(super) fn toggle_pause_regen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ui.regen_paused.take().is_some() {
            self.schedule_regen(window, cx);
        } else {
            if self.sketch.is_some() {
                self.flash_info("Sketches cannot be edited while regeneration is paused.".into(), cx);
                return;
            }
            self.ui.regen_paused = self.studio(cx).cloned();
        }
        cx.notify();
    }

    /// Paused banner buttons: regenerate (stay paused), regenerate and exit,
    /// discard unregenerated changes and exit.
    fn paused_action(&mut self, action: u8, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            0 => {
                self.ui.force_regen = true;
                self.ui.regen_paused = self.studio(cx).cloned();
                self.schedule_regen(window, cx);
            }
            1 => {
                self.ui.regen_paused = None;
                self.schedule_regen(window, cx);
            }
            _ => {
                if let Some(saved) = self.ui.regen_paused.take() {
                    self.doc.update(cx, |doc, cx| doc.edit_studio(self.tab, "Discard unregenerated changes", cx, |ps| *ps = saved));
                }
                self.schedule_regen(window, cx);
            }
        }
        cx.notify();
    }

    pub(super) fn render_regen_panels(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let pal = p();
        let mut out = Vec::new();
        if self.ui.regen_paused.is_some() {
            let btn = |id: &'static str, label: &'static str, action: u8, cx: &mut Context<Self>| {
                div()
                    .id(id)
                    .px_2()
                    .py_0p5()
                    .rounded_sm()
                    .text_xs()
                    .text_color(pal.accent)
                    .cursor_pointer()
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(label)
                    .on_click(cx.listener(move |this, _: &ClickEvent, w, cx| this.paused_action(action, w, cx)))
            };
            out.push(
                h_flex()
                    .id("paused-banner")
                    .occlude()
                    .absolute()
                    .bottom(px(46.))
                    .left(px(260.))
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(pal.panel_raised)
                    .border_1()
                    .border_color(pal.warning)
                    .child(div().text_xs().text_color(pal.text).child("Paused regeneration"))
                    .child(btn("pr-regen", "Regenerate Part Studio", 0, cx))
                    .child(btn("pr-exit", "Regenerate features and exit", 1, cx))
                    .child(btn("pr-discard", "Discard unregenerated changes and exit", 2, cx))
                    .into_any_element(),
            );
        }
        if self.ui.show_regen_times {
            let studio = self.studio(cx).cloned().unwrap_or_default();
            let total: f32 = self.display.features.iter().map(|f| f.time_ms).sum();
            let rows: Vec<AnyElement> = studio
                .features
                .iter()
                .map(|f| {
                    let t = self.display.feature_state(f.id).map(|s| s.time_ms).unwrap_or(0.0);
                    let id = f.id;
                    h_flex()
                        .id(SharedString::from(format!("rt-{id}")))
                        .gap_2()
                        .px_1()
                        .rounded_sm()
                        .hover(|d| d.bg(pal.panel_hover))
                        .child(div().flex_1().truncate().text_xs().text_color(pal.text).child(f.name.clone()))
                        .child(div().text_xs().text_color(if t > 500.0 { pal.warning } else { pal.text_muted }).child(format!("{t:.1} ms")))
                        .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                            // Hovering a row highlights the feature's geometry.
                            this.ui.list_hover = h.then_some(id);
                            this.refresh_scene(false, cx);
                        }))
                        .into_any_element()
                })
                .collect();
            out.push(
                v_flex()
                    .id("regen-times")
                    .occlude()
                    .absolute()
                    .top_3()
                    .left(px(340.))
                    .w(px(260.))
                    .max_h(px(420.))
                    .overflow_y_scroll()
                    .p_2()
                    .gap_0p5()
                    .rounded_md()
                    .bg(pal.panel_raised)
                    .border_1()
                    .border_color(pal.border_strong)
                    .shadow_lg()
                    .child(
                        h_flex()
                            .justify_between()
                            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Regeneration times"))
                            .child(div().id("rt-close").px_1().cursor_pointer().text_color(pal.text_faint).child("×").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.ui.show_regen_times = false;
                                cx.notify();
                            }))),
                    )
                    .child(div().text_xs().text_color(pal.text_muted).child(format!("Total regeneration time: {total:.1} ms")))
                    .children(rows)
                    .into_any_element(),
            );
        }
        out
    }

    // ------------------------------------------------------------ select ▸

    /// Select ▸ / Create selection: add entities related to `seed` to the
    /// active dialog field (filtered) or the selection.
    pub(super) fn select_related(&mut self, seed: &Reference, c: noshape_model::select::Criterion, cx: &mut Context<Self>) {
        let Some(ctx) = self.context.clone() else { return };
        let found = noshape_model::select::select_related(&ctx, seed, c);
        if let Some(e) = &self.edit {
            let dialog = e.dialog.clone();
            let Some((_, filter, _)) = dialog.read(cx).active_filter() else { return };
            let current = dialog.read(cx).selected_refs();
            dialog.update(cx, |d, cx| {
                for r in found {
                    let kind = if matches!(r, Reference::Edge(_)) { PickKind::Edge } else { PickKind::Face };
                    if kind_matches(kind, &r, filter) && !current.contains(&r) {
                        d.add_pick(r, cx);
                    }
                }
            });
        } else {
            for r in found {
                if !self.selection.contains(&r) {
                    self.selection.push(r);
                }
            }
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Create selection panel: grow every selected face/edge by a criterion.
    pub(super) fn render_create_selection(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.ui.create_selection {
            return None;
        }
        let pal = p();
        let seeds: Vec<Reference> = self.selection.iter().filter(|r| matches!(r, Reference::Face(_) | Reference::Edge(_))).cloned().collect();
        let has_face = seeds.iter().any(|r| matches!(r, Reference::Face(_)));
        let has_edge = seeds.iter().any(|r| matches!(r, Reference::Edge(_)));
        let button = |c: noshape_model::select::Criterion, cx: &mut Context<Self>| {
            let seeds = seeds.clone();
            div()
                .id(SharedString::from(format!("cs-{c:?}")))
                .px_2()
                .py_0p5()
                .rounded_sm()
                .text_xs()
                .text_color(pal.text)
                .cursor_pointer()
                .hover(|d| d.bg(pal.panel_hover))
                .child(c.label())
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    for s in &seeds {
                        let ok = match s {
                            Reference::Face(_) => noshape_model::select::Criterion::FACE.contains(&c),
                            _ => noshape_model::select::Criterion::EDGE.contains(&c),
                        };
                        if ok {
                            this.select_related(s, c, cx);
                        }
                    }
                }))
                .into_any_element()
        };
        let mut col = v_flex()
            .id("create-selection")
            .occlude()
            .absolute()
            .top_3()
            .left(px(340.))
            .w(px(240.))
            .p_2()
            .gap_1()
            .rounded_md()
            .bg(pal.panel_raised)
            .border_1()
            .border_color(pal.border_strong)
            .shadow_lg()
            .child(
                h_flex()
                    .justify_between()
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child("Create selection"))
                    .child(div().id("cs-close").px_1().cursor_pointer().text_color(pal.text_faint).child("×").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.ui.create_selection = false;
                        cx.notify();
                    }))),
            )
            .child(div().text_xs().text_color(pal.text_muted).child(if seeds.is_empty() { "Select seed faces or edges.".to_string() } else { format!("{} seed{}; click a criterion to add related entities", seeds.len(), if seeds.len() == 1 { "" } else { "s" }) }));
        if has_face {
            col = col.child(div().text_xs().text_color(pal.text_faint).child("Faces"));
            for c in noshape_model::select::Criterion::FACE {
                col = col.child(button(c, cx));
            }
        }
        if has_edge {
            col = col.child(div().text_xs().text_color(pal.text_faint).child("Edges"));
            for c in noshape_model::select::Criterion::EDGE {
                col = col.child(button(c, cx));
            }
        }
        Some(col.into_any_element())
    }

    // ------------------------------------------------------------ hide / show

    /// Y: hide what is under the cursor (or the selection if the cursor is
    /// over a selected entity or nothing).
    pub(super) fn hide_hovered_or_selected(&mut self, cx: &mut Context<Self>) {
        let targets: Vec<Reference> = match &self.hover {
            Some(h) if !self.selection.contains(h) => vec![h.clone()],
            _ => self.selection.clone(),
        };
        let mut targets = targets;
        targets.extend(self.selected_features.iter().map(|f| Reference::Feature(*f)));
        if targets.is_empty() {
            return;
        }
        self.set_visibility(&targets, true, cx);
        self.selection.clear();
        self.selected_features.clear();
        self.hover = None;
        cx.notify();
    }

    /// Hide or show parts, sketches, planes and default geometry.
    pub(super) fn set_visibility(&mut self, targets: &[Reference], hidden: bool, cx: &mut Context<Self>) {
        let mut bodies: Vec<BodyId> = Vec::new();
        let mut feats: Vec<FeatureId> = Vec::new();
        let mut defaults: Vec<Reference> = Vec::new();
        for r in targets {
            match r {
                Reference::Origin | Reference::Plane(_) => defaults.push(r.clone()),
                Reference::SketchEntity { sketch, .. } | Reference::SketchRegion { sketch, .. } | Reference::Sketch(sketch) => feats.push(*sketch),
                Reference::FeaturePlane(f) | Reference::MateConnector { feature: f, .. } => feats.push(*f),
                Reference::Feature(f) => {
                    // A feature that made parts hides its parts; sketches,
                    // planes and connectors hide themselves.
                    let made: Vec<BodyId> = self.display.bodies.iter().filter(|b| b.id.feature == *f).map(|b| b.id.clone()).collect();
                    if made.is_empty() {
                        feats.push(*f);
                    } else {
                        bodies.extend(made);
                    }
                }
                r => {
                    if let Some(b) = self.body_of(r) {
                        bodies.push(b);
                    }
                }
            }
        }
        if bodies.is_empty() && feats.is_empty() && defaults.is_empty() {
            return;
        }
        let verb = if hidden { "Hide" } else { "Show" };
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, verb, cx, |ps| {
                for b in &bodies {
                    ps.part_properties.entry(b.clone()).or_default().hidden = hidden;
                }
                for f in &feats {
                    if let Some(f) = ps.feature_mut(*f) {
                        f.hidden = hidden;
                    }
                }
                for d in &defaults {
                    ps.hidden_defaults.retain(|x| x != d);
                    if hidden {
                        ps.hidden_defaults.push(d.clone());
                    }
                }
            });
        });
    }

    /// Shift+Y: show hidden selected items, otherwise every hidden part.
    pub(super) fn show_hidden(&mut self, cx: &mut Context<Self>) {
        let mut targets: Vec<Reference> = self.selection.clone();
        targets.extend(self.selected_features.iter().map(|f| Reference::Feature(*f)));
        if targets.is_empty() {
            self.show_all_parts(cx);
        } else {
            self.set_visibility(&targets, false, cx);
        }
    }

    /// Context menu "Show all": parts, sketches, planes and default geometry.
    pub(super) fn show_everything(&mut self, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Show all", cx, |ps| {
                for p in ps.part_properties.values_mut() {
                    p.hidden = false;
                }
                for f in &mut ps.features {
                    f.hidden = false;
                }
                ps.hidden_defaults.clear();
            });
        });
        self.opts.show_planes = true;
        self.opts.show_sketches = true;
        cx.notify();
    }

    /// "Hide other parts": hide every part except `keep`.
    pub(super) fn hide_other_parts(&mut self, keep: &[BodyId], cx: &mut Context<Self>) {
        let others: Vec<Reference> = self.display.bodies.iter().filter(|b| !keep.contains(&b.id)).map(|b| Reference::Body(b.id.clone())).collect();
        self.set_visibility(&others, true, cx);
    }

    pub(super) fn hide_all_parts(&mut self, cx: &mut Context<Self>) {
        let all: Vec<Reference> = self.display.bodies.iter().map(|b| Reference::Body(b.id.clone())).collect();
        self.set_visibility(&all, true, cx);
    }

    /// Shift+P: hide all construction geometry (planes, sketches).
    pub(super) fn hide_construction(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let any_shown = self.opts.show_planes || self.opts.show_sketches;
        self.opts.show_planes = !any_shown;
        self.opts.show_sketches = !any_shown;
        let d = self.display.clone();
        self.apply_display(d, window, cx);
    }

    // ------------------------------------------------------------ isolate

    /// Parts targeted by Isolate / Make transparent: hovered or selected.
    fn isolate_targets(&self) -> Vec<BodyId> {
        let mut out: Vec<BodyId> = Vec::new();
        let hovered = self.hover.as_ref().and_then(|h| self.body_of(h));
        let from_sel = self.selection.iter().filter_map(|r| self.body_of(r));
        for b in hovered.into_iter().chain(from_sel) {
            if !out.contains(&b) {
                out.push(b);
            }
        }
        out
    }

    /// Shift+I / Shift+T (cumulative while active, like Onshape).
    pub(super) fn isolate(&mut self, mode: IsolateMode, window: &mut Window, cx: &mut Context<Self>) {
        let targets = self.isolate_targets();
        if targets.is_empty() {
            self.flash_info("Select or hover over a part first.".into(), cx);
            return;
        }
        match &mut self.ui.isolate {
            Some(iso) if iso.mode == mode => {
                for t in targets {
                    if !iso.bodies.contains(&t) {
                        iso.bodies.push(t);
                    }
                }
            }
            _ => self.ui.isolate = Some(Isolation { mode, bodies: targets, opacity: 0.2, select_transparent: false, expand: 0 }),
        }
        let d = self.display.clone();
        self.apply_display(d, window, cx);
    }

    pub(super) fn exit_isolate(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.ui.isolate.take().is_none() {
            return false;
        }
        let d = self.display.clone();
        self.apply_display(d, window, cx);
        true
    }

    /// Display to build the scene from: the regenerated display with
    /// isolate/transparency applied to part colors.
    /// Parts made transparent by Isolate / Make transparent, with the
    /// "Expand: distance" neighbors (nearest bounding-box centers first).
    pub(super) fn isolation_transparent(&self) -> HashSet<BodyId> {
        let Some(iso) = &self.ui.isolate else { return HashSet::new() };
        let center = |b: &BodyId| self.display.body(b).and_then(|b| b.bounds).map(|(lo, hi)| (lo + hi) * 0.5);
        let mut chosen: Vec<BodyId> = iso.bodies.clone();
        let mut others: Vec<(f64, BodyId)> = self
            .display
            .bodies
            .iter()
            .filter(|b| !chosen.contains(&b.id))
            .filter_map(|b| {
                let c = center(&b.id)?;
                let d = iso.bodies.iter().filter_map(center).map(|x| x.distance(c)).fold(f64::INFINITY, f64::min);
                Some((d, b.id.clone()))
            })
            .collect();
        others.sort_by(|a, b| a.0.total_cmp(&b.0));
        chosen.extend(others.into_iter().take(iso.expand).map(|(_, b)| b));
        self.display
            .bodies
            .iter()
            .filter(|b| match iso.mode {
                IsolateMode::Isolate => !chosen.contains(&b.id),
                IsolateMode::Transparent => chosen.contains(&b.id),
            })
            .map(|b| b.id.clone())
            .collect()
    }

    /// Regenerate the model before the edited feature (preview slider).
    pub(super) fn schedule_preview_before(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Needed for the preview slider and for drag handles on geometry the
        // feature consumes (fillet edges, shell faces).
        let consumes = self.edit.as_ref().is_some_and(|e| matches!(e.dialog.read(cx).feature.type_id.as_str(), "fillet" | "chamfer" | "shell"));
        if (self.ui.preview_opacity >= 0.999 && !consumes) || self.edit.is_none() {
            self.ui.preview_before = None;
            return;
        }
        let Some((studio, _)) = self.effective_studio(cx) else { return };
        let Some(index) = self.edit.as_ref().map(|e| e.index) else { return };
        let units = self.doc.read(cx).session.doc.settings.units;
        let config = self.current_config(&studio);
        let doc_vars = noshape_model::variables::document_variables(&self.doc.read(cx).session.doc);
        let derived: Arc<dyn noshape_model::regen::derived::DerivedProvider> = self.doc.read(cx).derived_provider();
        let custom: Arc<dyn noshape_model::custom::CustomFeatureProvider> = self.doc.read(cx).custom_features();
        let regen = self.regen.clone();
        self.ui.preview_task = Some(cx.spawn_in(window, async move |this, cx| {
            let display = cx
                .background_spawn(async move {
                    let opts = noshape_model::regen::RegenOptions { upto: Some(index), document_variables: doc_vars, preview_meshes: true, derived: Some(derived), custom_features: Some(custom), ..Default::default() };
                    let r = regen.lock().regenerate(&studio, &config, &units, &opts);
                    (r.display, r.context)
                })
                .await;
            let (display, context) = display;
            let _ = this.update_in(cx, |this: &mut Self, window, cx| {
                this.ui.before_context = Some(context);
                this.ui.preview_before = Some(Arc::new(display));
                let d = this.display.clone();
                this.apply_display(d, window, cx);
            });
        }));
    }

    pub(super) fn display_for_scene(&self) -> Arc<StudioDisplay> {
        // Preview slider: blend the model before the feature (fading out)
        // with the result (fading in).
        if self.edit.is_some()
            && self.ui.preview_opacity < 0.999
            && let Some(before) = &self.ui.preview_before
        {
            let s = self.ui.preview_opacity;
            let mut d = (*self.display).clone();
            for b in &mut d.bodies {
                b.color[3] *= s.max(0.02);
            }
            for b in &before.bodies {
                let mut b = b.clone();
                b.color[3] *= (1.0 - s).max(0.02);
                d.bodies.push(b);
            }
            return Arc::new(d);
        }
        let Some(iso) = &self.ui.isolate else { return self.display.clone() };
        let transparent = self.isolation_transparent();
        let mut d = (*self.display).clone();
        for b in &mut d.bodies {
            if transparent.contains(&b.id) {
                b.color[3] = b.color[3].min(iso.opacity);
            }
        }
        Arc::new(d)
    }

    /// A click on a transparent part goes through it (Alt+click, or always
    /// while isolating without "Select transparent geometry"): the first
    /// entity of an opaque part along the ray, if any.
    pub(super) fn pick_through_transparent(&self, t: PickTarget, position: DVec2, alt: bool, cx: &App) -> Option<PickTarget> {
        let Some(body) = self.body_of(&t.reference) else { return Some(t) };
        let iso_t = self.isolation_transparent();
        let appearance_t = |b: &BodyId| self.display.body(b).is_some_and(|d| d.color[3] < 0.99);
        let select_t = self.ui.isolate.as_ref().is_some_and(|i| i.select_transparent);
        let through = |b: &BodyId| (iso_t.contains(b) && (alt || !select_t)) || (alt && appearance_t(b));
        if !through(&body) {
            return Some(t);
        }
        let (origin, dir, tol) = {
            let vp = self.viewport.read(cx);
            let (o, d) = vp.ray(position);
            (o, d, vp.camera.world_per_pixel(vp.size().y) * 5.0)
        };
        let hit = interaction::entities_along_ray(&self.display, origin, dir, tol, &|b| through(b), 1).into_iter().next()?;
        let kind = if matches!(hit.reference, Reference::Edge(_)) { PickKind::Edge } else { PickKind::Face };
        Some(PickTarget { reference: hit.reference, kind, anchor: None })
    }

    pub(super) fn render_isolate_panel(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let iso = self.ui.isolate.as_ref()?;
        let pal = p();
        let title = match iso.mode {
            IsolateMode::Isolate => "Isolate",
            IsolateMode::Transparent => "Make transparent",
        };
        let opacity = iso.opacity;
        let select_t = iso.select_transparent;
        let expand = iso.expand;
        let expand_step = |id: &'static str, label: &'static str, delta: isize, cx: &mut Context<Self>| {
            div()
                .id(id)
                .px_1p5()
                .rounded_sm()
                .cursor_pointer()
                .hover(|d| d.bg(pal.panel_hover))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    let n = this.display.bodies.len();
                    if let Some(iso) = &mut this.ui.isolate {
                        iso.expand = (iso.expand as isize + delta).clamp(0, n as isize) as usize;
                    }
                    let d = this.display.clone();
                    this.apply_display(d, window, cx);
                }))
        };
        let step = |id: &'static str, label: &'static str, delta: f32, cx: &mut Context<Self>| {
            div()
                .id(id)
                .px_1p5()
                .rounded_sm()
                .cursor_pointer()
                .hover(|d| d.bg(pal.panel_hover))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    if let Some(iso) = &mut this.ui.isolate {
                        iso.opacity = (iso.opacity + delta).clamp(0.0, 0.9);
                    }
                    let d = this.display.clone();
                    this.apply_display(d, window, cx);
                }))
        };
        Some(
            h_flex()
                .id("isolate-panel")
                .occlude()
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
                .text_xs()
                .child(div().text_color(pal.accent_hover).child(title.to_uppercase()))
                .child(div().text_color(pal.text_muted).child("Transparency"))
                .child(step("iso-less", "−", -0.1, cx))
                .child(div().w(px(32.)).text_center().child(format!("{:.0}%", (1.0 - opacity) * 100.0)))
                .child(step("iso-more", "+", 0.1, cx))
                .child(
                    div()
                        .id("iso-select-t")
                        .px_1()
                        .cursor_pointer()
                        .text_color(if select_t { pal.accent } else { pal.text_muted })
                        .child(if select_t { "☑ Select transparent geometry" } else { "☐ Select transparent geometry" })
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            if let Some(iso) = &mut this.ui.isolate {
                                iso.select_transparent = !iso.select_transparent;
                            }
                            cx.notify();
                        })),
                )
                .child(div().text_color(pal.text_muted).child("Expand"))
                .child(expand_step("iso-exp-less", "−", -1, cx))
                .child(div().w(px(16.)).text_center().child(expand.to_string()))
                .child(expand_step("iso-exp-more", "+", 1, cx))
                .child(
                    div()
                        .id("iso-exit")
                        .px_1p5()
                        .cursor_pointer()
                        .text_color(pal.text_faint)
                        .hover(|d| d.text_color(pal.error))
                        .child("×")
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.exit_isolate(window, cx);
                        })),
                )
                .into_any_element(),
        )
    }

    // ------------------------------------------------------------ camera

    pub(super) fn push_camera_history(&mut self, cx: &App) {
        let cam = self.viewport.read(cx).camera;
        if self.ui.cam_history.last() != Some(&cam) {
            self.ui.cam_history.push(cam);
            if self.ui.cam_history.len() > 50 {
                self.ui.cam_history.remove(0);
            }
        }
    }

    /// Camera menu "Previous view".
    pub(super) fn previous_view(&mut self, cx: &mut Context<Self>) {
        if let Some(cam) = self.ui.cam_history.pop() {
            self.viewport.update(cx, |vp, cx| vp.animate_to(cam, cx));
        }
    }

    pub(super) fn zoom_fit(&mut self, cx: &mut Context<Self>) {
        self.push_camera_history(cx);
        self.viewport.update(cx, |vp, cx| vp.zoom_to_fit(true, cx));
    }

    /// What zoom to fit frames (Onshape): visible parts and sketches; the
    /// default planes only when there is nothing else (`None`).
    pub(super) fn fit_bounds(&self) -> Option<(glam::DVec3, glam::DVec3)> {
        let mut acc = self.display.bounds();
        let mut add = |q: glam::DVec3| acc = Some(acc.map_or((q, q), |(lo, hi)| (lo.min(q), hi.max(q))));
        let p3 = |a: [f32; 3]| glam::DVec3::new(a[0] as f64, a[1] as f64, a[2] as f64);
        for sk in &self.display.sketches {
            let editing = self.opts.editing_sketch == Some(sk.feature);
            if !editing && (sk.hidden || !self.opts.show_sketches) {
                continue;
            }
            sk.curves.iter().flat_map(|c| c.points.iter()).for_each(|q| add(p3(*q)));
            sk.points.iter().for_each(|q| add(p3(q.position)));
        }
        acc
    }

    /// Logical pixels of the viewport covered by the feature dialog (top left).
    pub(super) fn fit_inset_left(&self) -> f64 {
        if self.edit.is_some() { 220.0 + 2.0 * 12.0 } else { 0.0 }
    }

    /// W: the next box drag zooms to the box (press W again to cancel).
    pub(super) fn toggle_zoom_window(&mut self, cx: &mut Context<Self>) {
        self.push_camera_history(cx);
        self.viewport.update(cx, |vp, cx| {
            vp.zoom_window = !vp.zoom_window;
            cx.notify();
        });
        cx.notify();
    }

    /// Ctrl+Shift+arrows (Onshape pans 150 px per step).
    pub(super) fn pan_by(&mut self, dx: f64, dy: f64, cx: &mut Context<Self>) {
        self.viewport.update(cx, |vp, cx| {
            let mut to = vp.camera;
            let h = vp.size().y;
            to.pan(DVec2::new(dx, dy), h);
            vp.animate_to(to, cx);
        });
    }

    /// Bounding box of the selection (parts, faces, edges, sketches, planes).
    fn selection_bounds(&self) -> Option<(glam::DVec3, glam::DVec3)> {
        use glam::DVec3;
        let mut lo = DVec3::splat(f64::INFINITY);
        let mut hi = DVec3::splat(f64::NEG_INFINITY);
        let mut add = |q: DVec3| {
            lo = lo.min(q);
            hi = hi.max(q);
        };
        let p3 = |a: [f32; 3]| DVec3::new(a[0] as f64, a[1] as f64, a[2] as f64);
        let mut refs = self.selection.clone();
        for f in &self.selected_features {
            refs.extend(self.feature_geometry(*f));
        }
        for r in &refs {
            match r {
                Reference::Body(b) => {
                    if let Some((a, c)) = self.display.body(b).and_then(|b| b.bounds) {
                        add(a);
                        add(c);
                    }
                }
                Reference::SketchEntity { sketch, .. } | Reference::SketchRegion { sketch, .. } | Reference::Sketch(sketch) => {
                    if let Some(sk) = self.display.sketches.iter().find(|s| s.feature == *sketch) {
                        sk.curves.iter().flat_map(|c| c.points.iter()).for_each(|q| add(p3(*q)));
                        sk.points.iter().for_each(|q| add(p3(q.position)));
                    }
                }
                Reference::Plane(_) | Reference::FeaturePlane(_) => {
                    if let Some(pl) = self.display.planes.iter().find(|p| &p.reference == r) {
                        for (u, v) in [(-1.0, -1.0), (1.0, 1.0), (-1.0, 1.0), (1.0, -1.0)] {
                            add(pl.frame.origin + pl.frame.x * (u * pl.half_width) + pl.frame.y * (v * pl.half_height));
                        }
                    }
                }
                Reference::Origin => add(DVec3::ZERO),
                r => {
                    for b in &self.display.bodies {
                        if let Some(fi) = b.face_refs.iter().position(|x| x == r)
                            && let Some(&(first, count)) = b.mesh.face_ranges.get(fi)
                        {
                            for &i in &b.mesh.indices[first as usize..(first + count) as usize] {
                                if let Some(q) = b.mesh.positions.get(i as usize) {
                                    add(p3(*q));
                                }
                            }
                        } else if let Some(ei) = b.edge_refs.iter().position(|x| x == r) {
                            b.mesh.edges.get(ei).into_iter().flatten().for_each(|q| add(p3(*q)));
                        } else if let Some(vi) = b.vertex_refs.iter().position(|x| x == r)
                            && let Some(q) = b.mesh.vertices.get(vi)
                        {
                            add(p3(*q));
                        }
                    }
                }
            }
        }
        (lo.x <= hi.x).then_some((lo, hi))
    }

    /// Context / camera menu "Zoom to selection".
    pub(super) fn zoom_to_selection(&mut self, cx: &mut Context<Self>) {
        let Some((lo, hi)) = self.selection_bounds() else { return };
        self.push_camera_history(cx);
        let inset = self.fit_inset_left();
        self.viewport.update(cx, |vp, cx| {
            let mut to = vp.camera;
            crate::viewport::fit_camera_to_box(&mut to, lo, hi, vp.size(), inset);
            vp.animate_to(to, cx);
        });
    }

    // ------------------------------------------------------------ select other

    /// ` (grave): list the faces and edges under the cursor, nearest first;
    /// pressing it again moves down the list (Shift+` up).
    pub(super) fn select_other(&mut self, backwards: bool, cx: &mut Context<Self>) {
        if let Some(so) = &mut self.ui.select_other {
            let n = so.hits.len().max(1);
            so.index = if backwards { (so.index + n - 1) % n } else { (so.index + 1) % n };
            self.hover = so.hits.get(so.index).cloned();
            self.refresh_scene(false, cx);
            cx.notify();
            return;
        }
        let Some(at) = self.ui.pointer else { return };
        let (origin, dir, tol) = {
            let vp = self.viewport.read(cx);
            let (o, d) = vp.ray(at);
            (o, d, vp.camera.world_per_pixel(vp.size().y) * 8.0)
        };
        let iso = self.ui.isolate.as_ref().map(|i| (i.mode, i.bodies.clone()));
        let skip = move |b: &BodyId| match &iso {
            Some((IsolateMode::Isolate, keep)) => !keep.contains(b),
            Some((IsolateMode::Transparent, gone)) => gone.contains(b),
            None => false,
        };
        let hits: Vec<Reference> = interaction::entities_along_ray(&self.display, origin, dir, tol, &skip, 20).into_iter().map(|h| h.reference).collect();
        if hits.is_empty() {
            self.flash_info("Nothing under the cursor to select.".into(), cx);
            return;
        }
        // Start on the entry after the highlighted one (Onshape).
        let start = self.hover.as_ref().and_then(|h| hits.iter().position(|r| r == h)).map(|i| (i + 1) % hits.len()).unwrap_or(0);
        self.hover = hits.get(start).cloned();
        self.ui.select_other = Some(SelectOther { hits, index: start, at });
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Pick the highlighted (or clicked) Select other entry.
    pub(super) fn choose_select_other(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        let Some(so) = self.ui.select_other.take() else { return };
        let Some(r) = so.hits.get(index.unwrap_or(so.index)).cloned() else { return };
        let kind = if matches!(r, Reference::Edge(_)) { PickKind::Edge } else { PickKind::Face };
        self.hover = None;
        self.pick(PickTarget { reference: r, kind, anchor: None }, cx);
    }

    pub(super) fn render_select_other(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let so = self.ui.select_other.as_ref()?;
        let pal = p();
        let labeler = self.labeler(cx);
        let rows: Vec<AnyElement> = so
            .hits
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let active = i == so.index;
                let r2 = r.clone();
                let kind = if matches!(r, Reference::Edge(_)) { "Edge" } else { "Face" };
                h_flex()
                    .id(SharedString::from(format!("so-{i}")))
                    .gap_2()
                    .px_2()
                    .py_0p5()
                    .rounded_sm()
                    .text_xs()
                    .cursor_pointer()
                    .when(active, |d| d.bg(pal.accent_soft))
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(div().w(px(32.)).text_color(pal.text_faint).child(kind))
                    .child(div().text_color(pal.text).child(labeler(r)))
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        if *h {
                            if let Some(so) = &mut this.ui.select_other {
                                so.index = i;
                            }
                            this.hover = Some(r2.clone());
                            this.refresh_scene(false, cx);
                            cx.notify();
                        }
                    }))
                    .on_mouse_down(MouseButton::Left, cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        this.choose_select_other(Some(i), cx);
                    }))
                    .into_any_element()
            })
            .collect();
        Some(
            v_flex()
                .id("select-other")
                .occlude()
                .absolute()
                .left(px(so.at.x as f32 + 14.0))
                .top(px(so.at.y as f32 + 10.0))
                .min_w(px(180.))
                .p_1()
                .rounded_md()
                .bg(pal.panel_raised)
                .border_1()
                .border_color(pal.border_strong)
                .shadow_lg()
                .child(div().px_2().pb_1().text_xs().text_color(pal.text_muted).child("Select other"))
                .children(rows)
                .into_any_element(),
        )
    }

    /// Onshape's cursor badge: the number of selected entities (up to 5,
    /// then "5+") beside the pointer over the graphics area.
    pub(super) fn render_selection_badge(&self, cx: &App) -> Option<AnyElement> {
        let at = self.ui.badge_at.map(|(_, p)| p).filter(|_| self.viewport.read(cx).pointer_inside())?;
        let n = self.selection.len() + self.selected_features.len();
        if n == 0 || self.edit.is_some() || self.ui.select_other.is_some() || self.ui.mini_toolbar.is_some() || self.context_menu.is_some() {
            return None;
        }
        let pal = p();
        Some(
            div()
                .absolute()
                .left(px(at.x as f32 + 14.0))
                .top(px(at.y as f32 + 14.0))
                .px_1()
                .rounded_sm()
                .bg(pal.accent)
                .text_size(px(10.))
                .text_color(gpui_kit::rgb(0xffffff))
                .child(if n > 5 { "5+".to_string() } else { n.to_string() })
                .into_any_element(),
        )
    }

    /// Measure graphics (Onshape): a dotted line between the closest points
    /// of the measured entities, with its X (red), Y (green) and Z (blue)
    /// components.
    pub(super) fn measure_overlay(&mut self) -> Vec<noshape_render::Instance> {
        use noshape_render::{Drawable, Instance, LineData, LineSegment};
        if self.edit.is_some() || self.show_mass || self.selection.len() < 2 {
            return Vec::new();
        }
        let Some(ctx) = self.context.clone() else { return Vec::new() };
        let sel = self.selection.clone();
        // Uploaded once per selection (refresh_scene runs on every hover).
        if let Some((s, inst)) = &self.ui.measure_lines
            && *s == sel
        {
            return inst.iter().cloned().collect();
        }
        let results = match &self.measure_cache {
            Some((s, r)) if *s == sel => r.clone(),
            _ => {
                let r = noshape_model::regen::measure::measure(&ctx, &sel);
                self.measure_cache = Some((sel, r.clone()));
                r
            }
        };
        let mut l = LineData::default();
        for (_, m) in results {
            if let noshape_model::regen::measure::Measurement::Distance { a, b, .. } = m {
                let f = |v: glam::DVec3| -> [f32; 3] { v.as_vec3().into() };
                let style = LineSegment::DASHED | LineSegment::ON_TOP;
                l.segments.push(LineSegment { a: f(a), b: f(b), color: [20, 20, 20, 255], pick_id: 0, width: 2.0, style });
                let x = glam::DVec3::new(b.x, a.y, a.z);
                let y = glam::DVec3::new(b.x, b.y, a.z);
                for (p, q, c) in [(a, x, [220, 40, 40, 255]), (x, y, [40, 170, 60, 255]), (y, b, [40, 90, 220, 255])] {
                    if p.distance(q) > 1e-9 {
                        l.segments.push(LineSegment { a: f(p), b: f(q), color: c, pick_id: 0, width: 1.0, style });
                    }
                }
            }
        }
        let inst = (!l.segments.is_empty()).then(|| Instance::new(Drawable::Lines(self.gpu.uploader().upload_lines(&l))).per_primitive_color().unpickable().ignore_clip().layer(21));
        self.ui.measure_lines = Some((self.selection.clone(), inst.clone()));
        inst.into_iter().collect()
    }

    /// Arc-length midpoint of an edge polyline (display mesh).
    fn edge_midpoint(&self, r: &Reference) -> Option<glam::DVec3> {
        let p3 = |a: [f32; 3]| glam::DVec3::new(a[0] as f64, a[1] as f64, a[2] as f64);
        self.display.bodies.iter().find_map(|b| {
            let ei = b.edge_refs.iter().position(|x| x == r)?;
            let pl: Vec<glam::DVec3> = b.mesh.edges.get(ei)?.iter().map(|p| p3(*p)).collect();
            let total: f64 = pl.windows(2).map(|w| w[0].distance(w[1])).sum();
            let mut acc = 0.0;
            for w in pl.windows(2) {
                let l = w[0].distance(w[1]);
                if acc + l >= total * 0.5 && l > 0.0 {
                    return Some(w[0].lerp(w[1], (total * 0.5 - acc) / l));
                }
                acc += l;
            }
            pl.first().copied()
        })
    }

    /// Midpoint marker of the hovered edge (no dialog or sketch open, like
    /// Onshape).
    pub(super) fn midpoint_overlay(&self) -> Vec<noshape_render::Instance> {
        use noshape_render::{Drawable, Instance, LineSegment, PointData, PointSprite};
        let mut points = Vec::new();
        let sprite = |m: glam::DVec3, color: [u8; 4], size: f32| PointSprite { position: m.as_vec3().into(), color, pick_id: 0, size, style: LineSegment::ON_TOP, _pad: 0 };
        // Selected midpoints.
        for r in &self.selection {
            if let Reference::Implicit { on, inference: noshape_model::reference::Inference::Midpoint } = r
                && let Some(m) = self.edge_midpoint(on)
            {
                points.push(sprite(m, [255, 169, 10, 255], 11.0));
            }
        }
        if self.edit.is_none()
            && self.sketch.is_none()
            && let Some(r @ Reference::Edge(_)) = &self.hover
            && let Some(m) = self.edge_midpoint(r)
        {
            points.push(sprite(m, [255, 194, 14, 255], 9.0));
        }
        if points.is_empty() {
            return Vec::new();
        }
        let pts = PointData { points };
        vec![Instance::new(Drawable::Points(self.gpu.uploader().upload_points(&pts))).per_primitive_color().unpickable().ignore_clip().layer(22)]
    }

    /// An edge click within ~8 px of its midpoint selects the midpoint.
    pub(super) fn midpoint_pick(&self, t: PickTarget, position: DVec2, cx: &App) -> PickTarget {
        if self.sketch.is_some() {
            return t;
        }
        let Some(m) = self.edge_midpoint(&t.reference) else { return t };
        let Some(s) = self.viewport.read(cx).project(m) else { return t };
        if s.distance(position) > 8.0 {
            return t;
        }
        let r = Reference::Implicit { on: Box::new(t.reference.clone()), inference: noshape_model::reference::Inference::Midpoint };
        PickTarget { reference: r, kind: PickKind::Vertex, anchor: Some(m) }
    }

    // ------------------------------------------------------------ S toolbar

    /// S: the shortcut toolbar at the cursor (feature tools; sketch tools
    /// in a sketch). S again or Esc closes it.
    pub(super) fn toggle_mini_toolbar(&mut self, cx: &mut Context<Self>) {
        if self.ui.mini_toolbar.take().is_none() {
            let at = self.ui.pointer.unwrap_or_else(|| self.viewport.read(cx).size() * 0.5);
            self.ui.mini_toolbar = Some(at);
        }
        cx.notify();
    }

    pub(super) fn render_mini_toolbar(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let at = self.ui.mini_toolbar?;
        let pal = p();
        let in_sketch = self.sketch.is_some();
        let btn = |id: SharedString, icon: SharedString, tip: String| {
            div()
                .id(id)
                .size(px(30.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .cursor_pointer()
                .hover(|d| d.bg(pal.panel_hover))
                .child(svg().path(icon).size(px(18.)).text_color(pal.text))
                .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(window, cx))
        };
        let mut row = h_flex().gap_0p5();
        if in_sketch {
            use crate::actions::*;
            let tools: [(&str, &str, Box<dyn Action>); 12] = [
                ("Line", "line", Box::new(SketchLine)),
                ("Corner rectangle", "corner-rectangle", Box::new(SketchCornerRectangle)),
                ("Center point rectangle", "center-rectangle", Box::new(SketchCenterRectangle)),
                ("Center point circle", "circle", Box::new(SketchCircle)),
                ("3 point arc", "three-point-arc", Box::new(SketchArc3)),
                ("Point", "point-sketch", Box::new(SketchPoint)),
                ("Fillet", "sketch-fillet", Box::new(SketchFillet)),
                ("Trim", "trim", Box::new(SketchTrim)),
                ("Offset", "offset", Box::new(SketchOffset)),
                ("Use", "use", Box::new(SketchUse)),
                ("Dimension", "dimension", Box::new(SketchDimension)),
                ("Construction", "construction", Box::new(SketchConstruction)),
            ];
            for (name, icon, action) in tools {
                row = row.child(btn(SharedString::from(format!("mini-{icon}")), crate::assets::icon_path(icon), name.to_string()).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        this.ui.mini_toolbar = None;
                        window.focus(&this.focus, cx);
                        window.dispatch_action(action.boxed_clone(), cx);
                        cx.notify();
                    }),
                ));
            }
        } else {
            for id in self.ui.toolbar.shortcut_tools() {
                let Some(t) = noshape_model::features::get(&id) else { continue };
                let id: &'static str = t.id;
                let tip = match t.shortcut {
                    Some(s) => format!("{} ({})", t.name, s.replace("shift-", "Shift+").to_uppercase()),
                    None => t.name.to_string(),
                };
                row = row.child(btn(SharedString::from(format!("mini-{id}")), feature_icon(id), tip).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        this.ui.mini_toolbar = None;
                        this.start_feature(id, window, cx);
                    }),
                ));
            }
        }
        Some(
            div()
                .id("mini-toolbar")
                .occlude()
                .absolute()
                .left(px((at.x as f32 - 60.0).max(4.0)))
                .top(px((at.y as f32 - 44.0).max(4.0)))
                .p_1()
                .rounded_lg()
                .bg(pal.panel_raised)
                .border_1()
                .border_color(pal.border_strong)
                .shadow_lg()
                .child(row)
                .into_any_element(),
        )
    }

    // ------------------------------------------------------------ feature list

    /// Search box at the top of the feature list (created on first render).
    pub(super) fn ensure_filter_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ui.filter.is_some() {
            return;
        }
        let st = cx.new(|cx| InputState::new(window, cx).placeholder("Filter by name or type"));
        let sub = cx.subscribe_in(&st, window, |this: &mut Self, st, ev: &InputEvent, window, cx| match ev {
            InputEvent::Change => {
                this.ui.filter_query = st.read(cx).value().to_string();
                cx.notify();
            }
            InputEvent::PressEnter { .. } => window.focus(&this.focus, cx),
            _ => {}
        });
        self.ui.filter = Some((st, sub));
    }

    pub(super) fn set_filter(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.ui.filter_query = text.to_string();
        if let Some((st, _)) = &self.ui.filter {
            st.update(cx, |s, cx| s.set_value(text.to_string(), window, cx));
        }
        cx.notify();
    }

    /// Feature list click with Onshape's additive / Shift-range rules.
    pub(super) fn click_feature(&mut self, id: FeatureId, shift: bool, cx: &mut Context<Self>) {
        let order: Vec<FeatureId> = self.studio(cx).map(|s| s.features.iter().map(|f| f.id).collect()).unwrap_or_default();
        self.selected_features = interaction::list_click(&order, &self.selected_features, self.ui.anchor.as_ref(), &id, shift);
        self.ui.anchor = Some(id);
        self.refresh_scene(false, cx);
        cx.notify();
    }

    /// Drop of dragged features onto the row at `to`: moves every selected
    /// feature when the dragged one is selected; refuses moves above a
    /// feature's dependencies (Onshape) with an error message.
    pub(super) fn drop_features(&mut self, dragged: FeatureId, to: usize, cx: &mut Context<Self>) {
        let Some(studio) = self.studio(cx) else { return };
        let moving: Vec<FeatureId> = if self.selected_features.contains(&dragged) { self.selected_features.clone() } else { vec![dragged] };
        let Some(order) = interaction::reordered(&studio.features, &moving, to) else { return };
        if let Err(msg) = interaction::check_order(&studio.features, &order) {
            self.message = Some((FeatureStatus::Error, msg));
            cx.notify();
            return;
        }
        let n = moving.len();
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, if n == 1 { "Reorder feature" } else { "Reorder features" }, cx, |ps| interaction::apply_order(ps, &order));
        });
    }

    /// Delete key with parts selected and no features: a Delete part
    /// feature (Onshape deletes parts parametrically).
    pub(super) fn delete_selected_parts(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let parts: Vec<Reference> = self.selection.iter().filter(|r| matches!(r, Reference::Body(_))).cloned().collect();
        if parts.is_empty() {
            return false;
        }
        let units = self.doc.read(cx).session.doc.settings.units;
        let Some(studio) = self.studio(cx) else { return false };
        let Some(mut f) = studio.new_feature("deleteBodies", &units) else { return false };
        f.params.set("entities", ParamValue::Query(parts));
        self.selection.clear();
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, &format!("Insert feature : {}", f.name), cx, |ps| {
                ps.insert_at_rollback(f);
            });
        });
        let _ = window;
        true
    }

    /// "Show dependencies": parents above the arrow, children below.
    pub(super) fn render_dependencies(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let id = self.ui.dependencies?;
        let studio = self.studio(cx)?;
        let idx = studio.index_of(id)?;
        let name = studio.features[idx].name.clone();
        let names = |ids: Vec<FeatureId>| -> Vec<(FeatureId, String)> { ids.into_iter().filter_map(|i| studio.feature(i).map(|f| (i, f.name.clone()))).collect() };
        let parents = names(interaction::feature_dependencies(&studio.features, idx));
        let children = names(interaction::feature_dependents(&studio.features, idx));
        let pal = p();
        let row = |(fid, n): (FeatureId, String), cx: &mut Context<Self>| {
            div()
                .id(SharedString::from(format!("dep-{fid}")))
                .px_2()
                .py_0p5()
                .rounded_sm()
                .text_xs()
                .cursor_pointer()
                .hover(|d| d.bg(pal.panel_hover))
                .child(n)
                .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                    this.ui.list_hover = h.then_some(fid);
                    this.refresh_scene(false, cx);
                }))
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.selected_features = vec![fid];
                    this.refresh_scene(false, cx);
                    cx.notify();
                }))
                .into_any_element()
        };
        let none = |what: &str| div().px_2().text_xs().text_color(pal.text_faint).child(format!("No {what}")).into_any_element();
        let parent_rows: Vec<AnyElement> = if parents.is_empty() { vec![none("parents")] } else { parents.into_iter().map(|x| row(x, cx)).collect() };
        let child_rows: Vec<AnyElement> = if children.is_empty() { vec![none("children")] } else { children.into_iter().map(|x| row(x, cx)).collect() };
        Some(
            v_flex()
                .id("dependencies")
                .occlude()
                .absolute()
                .top_3()
                .left(px(340.))
                .w(px(240.))
                .p_2()
                .gap_1()
                .rounded_md()
                .bg(pal.panel_raised)
                .border_1()
                .border_color(pal.border_strong)
                .shadow_lg()
                .child(
                    h_flex()
                        .justify_between()
                        .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text_muted).child(format!("Dependencies of {name}")))
                        .child(div().id("dep-close").px_1().cursor_pointer().text_color(pal.text_faint).child("×").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.ui.dependencies = None;
                            this.ui.list_hover = None;
                            this.refresh_scene(false, cx);
                            cx.notify();
                        }))),
                )
                .children(parent_rows)
                .child(div().text_center().text_sm().text_color(pal.accent).child("↑"))
                .child(div().px_2().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(name.clone()))
                .child(div().text_center().text_sm().text_color(pal.accent).child("↓"))
                .children(child_rows)
                .into_any_element(),
        )
    }

    /// K: show or hide all mate connectors.
    pub(super) fn toggle_mate_connectors(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<(FeatureId, bool)> = self.studio(cx).map(|s| s.features.iter().filter(|f| f.type_id == "mateConnector").map(|f| (f.id, f.hidden)).collect()).unwrap_or_default();
        if ids.is_empty() {
            return;
        }
        let hide = ids.iter().any(|(_, h)| !h);
        let targets: Vec<Reference> = ids.into_iter().map(|(id, _)| Reference::Feature(id)).collect();
        self.set_visibility(&targets, hide, cx);
    }

    /// After the sketch editor opened: orient normal to it if requested.
    pub(super) fn orient_for_sketch(&mut self, cx: &mut Context<Self>) {
        if self.ui.orient_pending && self.sketch.is_some() {
            self.ui.orient_pending = false;
            self.ui.oriented = true;
            self.view_normal(cx);
        }
    }

    /// When a sketch closes: return to the view before orienting.
    pub(super) fn restore_after_sketch(&mut self, cx: &mut Context<Self>) {
        self.ui.orient_pending = false;
        if std::mem::take(&mut self.ui.oriented) {
            self.previous_view(cx);
        }
    }

    // ------------------------------------------------------------ remembered values

    /// Store the REMEMBER_PREVIOUS_VALUE parameters of an accepted feature.
    pub(super) fn remember_values(&mut self, f: &noshape_model::partstudio::Feature) {
        let Some(ty) = f.feature_type() else { return };
        for spec in ty.params.iter().filter(|s| s.hints.has(noshape_model::params::Hints::REMEMBER_PREVIOUS_VALUE)) {
            if let Some(v) = f.params.get(spec.id) {
                self.ui.remembered.insert((f.type_id.clone(), spec.id), v.clone());
            }
        }
    }

    /// Defaults of a new feature from the last accepted one of its type.
    pub(super) fn apply_remembered(&self, f: &mut noshape_model::partstudio::Feature) {
        for ((ty, id), v) in &self.ui.remembered {
            if *ty == f.type_id {
                f.params.set(id, v.clone());
            }
        }
    }

    // ------------------------------------------------------------ dialogs

    /// Onshape's automatic choices in a new Extrude/Revolve: the operation
    /// becomes Add when the profile sits on a part (with that part as the
    /// merge scope), and choosing Remove/Intersect points the feature into
    /// the part. Fields the user set are left alone.
    pub(super) fn auto_operation(&mut self, cx: &mut Context<Self>) {
        let Some(e) = &self.edit else { return };
        if !e.is_new {
            return;
        }
        let dialog = e.dialog.clone();
        let (ty, op_user, flip_user, profiles, op, scope_user) = {
            let d = dialog.read(cx);
            (d.feature.type_id.clone(), d.user_set.contains("operationType"), d.user_set.contains("oppositeDirection"), d.feature.params.query("entities").to_vec(), d.feature.params.enum_("operationType").to_string(), d.user_set.contains("booleanScope"))
        };
        if !matches!(ty.as_str(), "extrude" | "revolve" | "sweep") {
            return;
        }
        // Part the first profile lies on: a picked planar face, or the face a
        // sketch was drawn on.
        let host = profiles.iter().find_map(|r| match r {
            Reference::Face(_) => self.body_of(r),
            Reference::SketchRegion { sketch, .. } | Reference::SketchEntity { sketch, .. } | Reference::Sketch(sketch) => {
                let plane = self.studio(cx)?.feature(*sketch)?.params.query("sketchPlane").first().cloned()?;
                self.body_of(&plane)
            }
            _ => None,
        });
        let mut changes: Vec<(&'static str, ParamValue)> = Vec::new();
        if !op_user {
            let want = if host.is_some() { "ADD" } else { "NEW" };
            if op != want && matches!(op.as_str(), "NEW" | "ADD") {
                changes.push(("operationType", ParamValue::Enum(want.into())));
            }
            if let (Some(b), false, "extrude" | "revolve" | "sweep") = (&host, scope_user, ty.as_str())
                && dialog.read(cx).feature.params.query("booleanScope").is_empty()
            {
                changes.push(("booleanScope", ParamValue::Query(vec![Reference::Body(b.clone())])));
            }
        }
        if ty == "extrude" && !flip_user && host.is_some() {
            let into = matches!(op.as_str(), "REMOVE" | "INTERSECT");
            if dialog.read(cx).feature.params.bool("oppositeDirection") != into {
                changes.push(("oppositeDirection", ParamValue::Bool(into)));
            }
        }
        if changes.is_empty() {
            return;
        }
        dialog.update(cx, |d, cx| {
            for (k, v) in changes {
                d.set_value_auto(k, v, cx);
            }
        });
    }
}

/// A sketch as 2D sheet geometry in millimeters (plane coordinates).
pub fn sketch_sheet(sk: &noshape_model::display::SketchDisplay) -> noshape_model::drawing::sheet::SheetGeometry {
    use noshape_model::drawing::sheet::{LineStyle, Polyline2};
    let f = &sk.frame;
    let local = |p: [f32; 3]| {
        let d = glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64) - f.origin;
        [d.dot(f.x) * 1000.0, d.dot(f.y) * 1000.0]
    };
    let lines: Vec<Polyline2> = sk
        .curves
        .iter()
        .filter(|c| c.points.len() >= 2 && !c.external)
        .map(|c| Polyline2 { points: c.points.iter().map(|p| local(*p)).collect(), style: if c.construction { LineStyle::Hidden } else { LineStyle::Visible }, closed: false, pick: 0 })
        .collect();
    let (mut w, mut h) = (0.0f64, 0.0f64);
    for l in &lines {
        for p in &l.points {
            w = w.max(p[0].abs());
            h = h.max(p[1].abs());
        }
    }
    let mut g = noshape_model::drawing::sheet::SheetGeometry::new(w * 2.0, h * 2.0);
    g.lines = lines;
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    // The glob import brings GPUI's `test` attribute; use the built-in one.
    use std::prelude::v1::test;

    #[test]
    fn rejection_messages_name_what_the_field_takes() {
        assert_eq!(filter_description(QueryFilter::EDGE), "Select edges.");
        assert_eq!(filter_description(QueryFilter::ANY_EDGE), "Select edges or sketch curves.");
        let m = filter_description(QueryFilter::SKETCH_REGION | QueryFilter::ANY_FACE);
        assert_eq!(m, "Select sketch regions or faces.");
        let m = filter_description(QueryFilter::PLANE_LIKE);
        assert_eq!(m, "Select planar faces, planes or mate connectors.");
    }
}
