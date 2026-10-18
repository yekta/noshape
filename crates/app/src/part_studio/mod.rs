//! Part Studio tab: toolbar, feature list (with rollback bar), parts list,
//! 3D viewport with selection, and feature dialogs with live preview.

mod config_panel;
mod fab_panel;
mod feature_ops;
mod manipulators;
mod menus;
mod sketch;
mod view_tools;

use crate::actions::*;
use crate::sketch_mode::SketchEditor;
use crate::assets::icon_path;
use crate::doc_model::{DocEvent, DocModel};
use crate::feature_dialog::{DialogEvent, FeatureDialog, RefLabeler};
use crate::gpu::Gpu;
use crate::scene_builder::{self, BuiltScene, GpuCache, PickKind, SceneOptions};
use crate::theme::{p, with_alpha};
use crate::viewport::{Viewport, ViewportEvent};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use glam::DVec3;
use noshape_model::display::{FeatureStatus, StudioDisplay};
use noshape_model::document::TabId;
use noshape_model::features::{self, FeatureType};
use noshape_model::params::{ParamKind, QueryFilter};
use noshape_model::partstudio::{Feature, PartStudio};
use noshape_model::reference::{DatumPlane, FeatureId, Reference, TopoName};
use noshape_model::regen::{RegenOptions, Regenerator};
use noshape_render::StandardView;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;

/// Toolbar layout: groups of feature ids; the first id of an inner list is
/// the face button, the rest live in its dropdown.
const TOOLBAR: &[&[&[&str]]] = &[
    &[&["sketch"]],
    &[&["extrude"], &["revolve"], &["sweep"], &["loft"], &["thicken", "enclose"]],
    &[&["fillet"], &["chamfer", "faceBlend", "modifyFillet"], &["draft", "bodyDraft"]],
    &[&["rib"], &["shell"], &["hole", "externalThread"]],
    &[&["booleanBodies"], &["splitPart"], &["transform", "deleteBodies"]],
    &[&["linearPattern", "circularPattern", "curvePattern"], &["mirror"]],
    &[&["deleteFace", "moveFace", "replaceFace"]],
    &[&["cPlane"], &["mateConnector", "cPoint"]],
    &[&["helix", "fitSpline", "projectCurves", "bridgingCurve", "compositeCurve", "intersectionCurve", "trimCurve", "isocline", "offsetCurveOnFace", "isoparametricCurve", "editCurve", "routingCurve"]],
    &[&["offsetSurface", "fill", "extendSurface", "boundarySurface", "ruledSurface", "mutualTrim", "constrainedSurface", "wrap"]],
    &[&["sheetMetalStart", "sheetMetalFlange", "sheetMetalHem", "sheetMetalTab", "sheetMetalBend", "sheetMetalJog", "sheetMetalFormed", "sheetMetalLoft", "sheetMetalMakeJoint", "sheetMetalJoint", "sheetMetalCorner", "sheetMetalBendRelief", "sheetMetalCornerBreak", "sheetMetalUnfold", "sheetMetalRefold", "sheetMetalEnd"]],
    &[&["frame", "frameTrim", "gusset", "endcap", "cutlist", "tag"]],
    &[&["importDerived"]],
    &[&["assignVariable", "queryVariable"]],
];

/// Icon file for a feature type id.
pub fn feature_icon(type_id: &str) -> SharedString {
    if noshape_model::custom::is_custom(type_id) {
        return icon_path(features::get(type_id).map(|t| t.icon).unwrap_or("custom-feature"));
    }
    let name = match type_id {
        "cPlane" => "plane",
        "cPoint" => "point",
        "assignVariable" | "queryVariable" => "variable",
        "booleanBodies" => "boolean",
        "splitPart" => "split",
        "deleteBodies" => "delete-part",
        "projectCurves" => "projected-curve",
        "offsetCurveOnFace" => "offset-curve",
        "isoparametricCurve" => "isoparametric",
        "extendSurface" => "move-boundary",
        "sheetMetalStart" => "sheet-metal",
        "sheetMetalFlange" => "flange",
        "sheetMetalHem" => "hem",
        "sheetMetalTab" => "tab",
        "sheetMetalBendRelief" => "bend-relief",
        "sheetMetalCornerBreak" => "corner-break",
        "sheetMetalCorner" => "corner-relief",
        "sheetMetalUnfold" => "unfold",
        "sheetMetalRefold" => "refold",
        "sheetMetalJoint" | "sheetMetalMakeJoint" => "joint",
        "sheetMetalJog" => "jog",
        "sheetMetalFormed" => "form",
        "endcap" => "end-cap",
        "cutlist" => "cut-list",
        "importDerived" | "importForeign" => "import",
        "externalThread" => "external-thread",
        "faceBlend" => "face-blend",
        "modifyFillet" => "modify-fillet",
        "bodyDraft" => "body-draft",
        other => {
            // camelCase -> kebab-case
            let mut s = String::new();
            for (i, ch) in other.chars().enumerate() {
                if ch.is_uppercase() && i > 0 {
                    s.push('-');
                }
                s.push(ch.to_ascii_lowercase());
            }
            return icon_path(&s);
        }
    };
    icon_path(name)
}

struct EditSession {
    dialog: Entity<FeatureDialog>,
    /// Index the feature occupies (new features are inserted at the rollback bar).
    index: usize,
    is_new: bool,
    _sub: Subscription,
}

pub struct PartStudioView {
    pub tab: TabId,
    doc: Entity<DocModel>,
    gpu: Gpu,
    viewport: Entity<Viewport>,
    display: Arc<StudioDisplay>,
    built: Option<BuiltScene>,
    cache: GpuCache,
    pub selection: Vec<Reference>,
    hover: Option<Reference>,
    opts: SceneOptions,
    edit: Option<EditSession>,
    selected_features: Vec<FeatureId>,
    regen: Arc<Mutex<Regenerator>>,
    regen_seq: u64,
    regen_task: Option<Task<()>>,
    regenerating: bool,
    focus: FocusHandle,
    fitted_once: bool,
    message: Option<(FeatureStatus, String)>,
    sketch: Option<SketchEditor>,
    context: Option<Arc<noshape_model::regen::context::Context>>,
    context_menu: Option<(Point<Pixels>, Entity<gpui_kit::component::menu::PopupMenu>, Subscription)>,
    measure_cache: Option<(Vec<Reference>, Vec<(String, noshape_model::regen::measure::Measurement)>)>,
    show_mass: bool,
    right_panel: Option<config_panel::RightPanel>,
    panel_inputs: std::collections::HashMap<String, (Entity<gpui_kit::component::input::InputState>, String)>,
    panel_subs: Vec<Subscription>,
    config_values: noshape_model::configuration::ConfigValues,
    needs_regen: bool,
    palette: Option<(Entity<gpui_kit::component::input::InputState>, Subscription)>,
    palette_query: String,
    show_shortcuts: bool,
    section_input: Option<(Entity<gpui_kit::component::input::InputState>, Subscription)>,
    renaming: Option<(FeatureId, Entity<gpui_kit::component::input::InputState>, Subscription)>,
    dim_input: Option<(noshape_sketch::ConstraintId, Entity<gpui_kit::component::input::InputState>, Subscription)>,
    manip_drag: Option<manipulators::ManipDrag>,
    folder_renaming: Option<(u32, Entity<gpui_kit::component::input::InputState>, Subscription)>,
    named_views: Option<(Entity<gpui_kit::component::input::InputState>, Subscription)>,
    flat_cache: Option<fab_panel::FlatCache>,
    flat_task: Option<(usize, Task<()>)>,
    _subs: Vec<Subscription>,
}

impl Focusable for PartStudioView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl PartStudioView {
    pub fn new(doc: Entity<DocModel>, tab: TabId, gpu: Gpu, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let viewport = cx.new(|cx| Viewport::new(gpu.clone(), window, cx));
        let mut subs = Vec::new();
        subs.push(cx.subscribe_in(&viewport, window, Self::on_viewport_event));
        subs.push(cx.subscribe_in(&doc, window, |this: &mut Self, _, ev: &DocEvent, window, cx| match ev {
            DocEvent::TabChanged(t) if *t == this.tab => this.schedule_regen(window, cx),
            DocEvent::TabChanged(t) => {
                // Variable Studio edits affect every Part Studio.
                let doc = &this.doc.read(cx).session.doc;
                let is_vars = matches!(doc.tab(*t).map(|t| &t.content), Some(noshape_model::TabContent::VariableStudio(_)));
                // A changed Part Studio may be the source of our Derived features.
                let is_source = matches!(doc.tab(*t).map(|t| &t.content), Some(noshape_model::TabContent::PartStudio(_)))
                    && this.studio(cx).is_some_and(|s| s.features.iter().any(|f| f.type_id == "importDerived" && f.params.text("partStudio") == t.to_string()));
                // A Feature Studio defining custom features used here.
                let prefix = noshape_model::custom::type_id(*t, "");
                let is_fs = matches!(doc.tab(*t).map(|t| &t.content), Some(noshape_model::TabContent::FeatureStudio(_)));
                let uses_fs = is_fs && this.studio(cx).is_some_and(|s| s.features.iter().any(|f| f.type_id.starts_with(&prefix)));
                if is_vars || is_source || uses_fs {
                    this.schedule_regen(window, cx);
                } else if is_fs {
                    cx.notify(); // toolbar lists the studio's features
                }
            }
            DocEvent::Reloaded => {
                this.cancel_edit(window, cx);
                this.schedule_regen(window, cx)
            }
            DocEvent::Error(msg) => {
                this.message = Some((FeatureStatus::Error, msg.clone()));
                cx.notify();
            }
            _ => {}
        }));
        let mut this = Self {
            tab,
            doc,
            gpu,
            viewport,
            display: Arc::new(StudioDisplay::defaults()),
            built: None,
            cache: GpuCache::default(),
            selection: Vec::new(),
            hover: None,
            opts: SceneOptions { show_planes: true, show_sketches: true, show_origin: true, ..Default::default() },
            edit: None,
            selected_features: Vec::new(),
            regen: Arc::new(Mutex::new(Regenerator::new())),
            regen_seq: 0,
            regen_task: None,
            regenerating: false,
            focus: cx.focus_handle(),
            fitted_once: false,
            message: None,
            sketch: None,
            context: None,
            context_menu: None,
            measure_cache: None,
            show_mass: false,
            right_panel: None,
            panel_inputs: Default::default(),
            panel_subs: Vec::new(),
            config_values: Default::default(),
            needs_regen: false,
            palette: None,
            palette_query: String::new(),
            show_shortcuts: false,
            section_input: None,
            renaming: None,
            dim_input: None,
            manip_drag: None,
            folder_renaming: None,
            named_views: None,
            flat_cache: None,
            flat_task: None,
            _subs: subs,
        };
        this.schedule_regen(window, cx);
        this
    }

    fn studio<'a>(&self, cx: &'a App) -> Option<&'a PartStudio> {
        self.doc.read(cx).studio(self.tab)
    }

    // ------------------------------------------------------------ regeneration

    /// Studio to evaluate: the document's, with the feature being edited
    /// applied as a draft (live preview).
    fn effective_studio(&self, cx: &App) -> Option<(PartStudio, Option<usize>)> {
        let mut ps = self.studio(cx)?.clone();
        let mut upto = None;
        if let Some(e) = &self.edit {
            let draft = e.dialog.read(cx).feature.clone();
            if e.is_new {
                let at = e.index.min(ps.features.len());
                ps.features.insert(at, draft);
            } else if let Some(slot) = ps.features.get_mut(e.index) {
                *slot = draft;
            }
            // While editing, show the model rolled to just after this feature
            // (or the finished model with "Final").
            if !e.dialog.read(cx).show_final {
                upto = Some(e.index + 1);
            }
        }
        Some((ps, upto))
    }

    pub fn schedule_regen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((studio, upto)) = self.effective_studio(cx) else { return };
        let units = self.doc.read(cx).session.doc.settings.units.clone();
        let config = self.current_config(&studio);
        let doc_vars = noshape_model::variables::document_variables(&self.doc.read(cx).session.doc);
        let derived: Arc<dyn noshape_model::regen::derived::DerivedProvider> = self.doc.read(cx).derived_provider();
        let derived2 = derived.clone();
        let custom: Arc<dyn noshape_model::custom::CustomFeatureProvider> = self.doc.read(cx).custom_features();
        let custom2 = custom.clone();
        let regen = self.regen.clone();
        let regen2 = self.regen.clone();
        self.regen_seq += 1;
        let seq = self.regen_seq;
        self.regenerating = true;
        let editing_sketch = self.sketch.as_ref().map(|s| s.feature).or_else(|| self.editing_sketch_id(cx));
        self.regen_task = Some(cx.spawn_in(window, async move |this, cx| {
            // Coalesce bursts of edits (typing in a field).
            cx.background_executor().timer(Duration::from_millis(12)).await;
            let result = cx
                .background_spawn(async move {
                    let opts = RegenOptions { upto, editing_sketch, document_variables: doc_vars.clone(), preview_meshes: true, derived: Some(derived), custom_features: Some(custom), ..Default::default() };
                    let r = regen.lock().regenerate(&studio, &config, &units, &opts);
                    (r, studio, config, units, upto, editing_sketch, doc_vars)
                })
                .await;
            let (result, studio, config, units, upto, editing_sketch, doc_vars) = result;
            let refine = result.needs_refine;
            let _ = this.update_in(cx, |this: &mut Self, window, cx| {
                if seq != this.regen_seq {
                    return;
                }
                this.regenerating = false;
                this.context = Some(result.context.clone());
                this.apply_display(Arc::new(result.display), window, cx);
            });
            // Progressive meshing: refine preview meshes in the background.
            if refine {
                let regen2 = regen2;
                let fine = cx
                    .background_spawn(async move {
                        let opts = RegenOptions { upto, editing_sketch, document_variables: doc_vars, preview_meshes: false, derived: Some(derived2), custom_features: Some(custom2), ..Default::default() };
                        regen2.lock().regenerate(&studio, &config, &units, &opts)
                    })
                    .await;
                let _ = this.update_in(cx, |this: &mut Self, window, cx| {
                    if seq != this.regen_seq {
                        return;
                    }
                    this.context = Some(fine.context.clone());
                    this.apply_display(Arc::new(fine.display), window, cx);
                });
            }
        }));
        cx.notify();
    }

    /// Configuration values to evaluate: defaults overridden by the user's choice.
    pub(super) fn current_config(&self, studio: &PartStudio) -> noshape_model::configuration::ConfigValues {
        let mut v = studio.configuration.defaults();
        for (k, val) in &self.config_values.0 {
            if v.0.contains_key(k) {
                v.0.insert(k.clone(), val.clone());
            }
        }
        v
    }

    fn apply_display(&mut self, display: Arc<StudioDisplay>, window: &mut Window, cx: &mut Context<Self>) {
        self.display = display;
        let built = scene_builder::build(&self.display, &self.opts, &mut self.cache, self.gpu.uploader());
        let prio = built.picks.priority_fn();
        self.built = Some(built);
        self.viewport.update(cx, |vp, _| vp.pick_priority = prio);
        // Update dialog status from the edited feature's state.
        if let Some(e) = &self.edit {
            let id = e.dialog.read(cx).feature.id;
            let st = self.display.feature_state(id).and_then(|s| match s.status {
                FeatureStatus::Error | FeatureStatus::Warning | FeatureStatus::Info => Some((s.status, s.message.clone().unwrap_or_else(|| "Feature failed".into()))),
                _ => None,
            });
            e.dialog.update(cx, |d, cx| d.set_status(st, cx));
        }
        self.sync_sketch_editor(cx);
        self.refresh_scene(true, cx);
        if !self.fitted_once {
            self.fitted_once = true;
            if self.display.bounds().is_some() {
                self.viewport.update(cx, |vp, cx| vp.zoom_to_fit(false, cx));
            } else {
                // Empty studio: frame the default planes.
                self.viewport.update(cx, |vp, cx| {
                    let size = vp.size();
                    let aspect = if size.y > 0.0 { size.x / size.y } else { 1.6 };
                    vp.camera.fit_sphere(glam::DVec3::ZERO, 0.11, aspect);
                    vp.request_redraw(cx);
                });
            }
        }
        let _ = window;
        cx.notify();
    }

    /// Rebuild highlight state and push the scene to the viewport.
    fn refresh_scene(&mut self, geometry_changed: bool, cx: &mut Context<Self>) {
        if self.built.is_none() {
            return;
        }
        let (mut selection, mut accent) = match &self.edit {
            Some(e) => {
                let d = e.dialog.read(cx);
                (d.active_refs(), d.selected_refs())
            }
            None => (self.selection.clone(), Vec::new()),
        };
        let mut extra = self.sketch_overlay(&mut selection, &mut accent, cx);
        extra.extend(self.manipulator_overlay(cx));
        self.sync_manipulator(cx);
        let built = self.built.as_ref().expect("built");
        let scene = scene_builder::with_highlights(built, &self.display, &selection, self.hover.as_ref(), &accent, extra, self.gpu.uploader());
        self.viewport.update(cx, |vp, cx| vp.set_scene(Arc::new(scene), geometry_changed, cx));
    }

    // ------------------------------------------------------------ labels

    fn labeler(&self, cx: &App) -> RefLabeler {
        let names: Vec<(FeatureId, String)> = self.studio(cx).map(|s| s.features.iter().map(|f| (f.id, f.name.clone())).collect()).unwrap_or_default();
        let bodies: Vec<(noshape_model::BodyId, String)> = self.display.bodies.iter().map(|b| (b.id.clone(), b.name.clone())).collect();
        Arc::new(move |r: &Reference| {
            let fname = |id: FeatureId| names.iter().find(|(i, _)| *i == id).map(|(_, n)| n.clone()).unwrap_or_else(|| "deleted feature".into());
            match r {
                Reference::Origin => "Origin".into(),
                Reference::Plane(DatumPlane::Top) => "Top plane".into(),
                Reference::Plane(DatumPlane::Front) => "Front plane".into(),
                Reference::Plane(DatumPlane::Right) => "Right plane".into(),
                Reference::Face(n) => format!("Face of {}", n.creator().map(fname).unwrap_or_else(|| "part".into())),
                Reference::Edge(n) => format!("Edge of {}", edge_creator(n).map(fname).unwrap_or_else(|| "part".into())),
                Reference::Vertex(_) => "Vertex".into(),
                Reference::Body(b) => bodies.iter().find(|(i, _)| i == b).map(|(_, n)| n.clone()).unwrap_or_else(|| "Part".into()),
                Reference::SketchEntity { sketch, .. } => format!("{} entity", fname(*sketch)),
                Reference::SketchRegion { sketch, .. } => format!("{} region", fname(*sketch)),
                Reference::Sketch(s) => fname(*s),
                Reference::FeaturePlane(f) => fname(*f),
                Reference::MateConnector { feature, .. } => fname(*feature),
                Reference::Feature(f) => fname(*f),
                Reference::Implicit { .. } => "Mate connector".into(),
                Reference::QueryVariable(name) => format!("#{name}"),
            }
        })
    }

    // ------------------------------------------------------------ viewport events

    fn on_viewport_event(&mut self, vp: &Entity<Viewport>, ev: &ViewportEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.sketch.is_some() && self.sketch_viewport_event(vp, ev, window, cx) {
            return;
        }
        if self.sketch.is_none() && self.manipulator_event(ev, window, cx) {
            return;
        }
        match ev {
            ViewportEvent::Hover(id) => {
                let r = id.and_then(|id| self.built.as_ref()?.picks.get(id).map(|t| t.reference.clone()));
                if r != self.hover {
                    self.hover = r;
                    self.refresh_scene(false, cx);
                }
            }
            ViewportEvent::Click { pick, .. } => {
                let target = pick.and_then(|id| self.built.as_ref()?.picks.get(id).cloned());
                if let Some(e) = &self.edit {
                    if let Some(t) = target {
                        let accepted = e.dialog.update(cx, |d, cx| {
                            let Some((_, filter, _)) = d.active_filter() else { return false };
                            if !kind_matches(t.kind, &t.reference, filter) {
                                return false;
                            }
                            d.add_pick(t.reference.clone(), cx)
                        });
                        if !accepted {
                            self.message = Some((FeatureStatus::Info, "That entity can't be used in the active field.".into()));
                        }
                    }
                } else {
                    match target {
                        Some(t) => {
                            if let Some(pos) = self.selection.iter().position(|r| *r == t.reference) {
                                self.selection.remove(pos);
                            } else {
                                self.selection.push(t.reference);
                            }
                        }
                        None => self.selection.clear(),
                    }
                }
                self.refresh_scene(false, cx);
                cx.notify();
            }
            ViewportEvent::BoxSelect { ids, .. } => {
                if let Some(built) = &self.built {
                    for id in ids {
                        if let Some(t) = built.picks.get(*id)
                            && matches!(t.kind, PickKind::Face | PickKind::Edge | PickKind::SketchCurve | PickKind::SketchPoint)
                            && !self.selection.contains(&t.reference)
                        {
                            self.selection.push(t.reference.clone());
                        }
                    }
                }
                self.refresh_scene(false, cx);
            }
            ViewportEvent::ContextMenu { pick, position } => {
                let target = pick.and_then(|id| self.built.as_ref()?.picks.get(id).cloned());
                self.open_context_menu(target, *position, window, cx);
            }
            _ => {}
        }
        let _ = window;
    }

    // ------------------------------------------------------------ editing

    pub fn start_feature(&mut self, type_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        // Starting Extrude/Revolve from an open sketch finishes the sketch and
        // preselects all of its regions (Onshape behavior).
        if self.sketch.is_some() && type_id != "sketch" {
            let regions = self.finish_sketch(window, cx);
            self.selection = regions;
        } else if self.edit.is_some() {
            self.cancel_edit(window, cx);
        }
        let units = self.doc.read(cx).session.doc.settings.units.clone();
        let Some(studio) = self.studio(cx) else { return };
        let Some(mut feature) = studio.new_feature(type_id, &units) else { return };
        let index = studio.rollback_index();
        // Preselection fills the first compatible selection field (Onshape behavior).
        if let Some(ty) = features::get(type_id)
            && !self.selection.is_empty()
        {
            preselect(ty, &mut feature, &self.selection, self.built.as_ref());
        }
        self.selection.clear();
        self.open_dialog(feature, index, true, window, cx);
    }

    pub fn edit_feature(&mut self, id: FeatureId, window: &mut Window, cx: &mut Context<Self>) {
        if self.edit.is_some() {
            self.cancel_edit(window, cx);
        }
        let Some(studio) = self.studio(cx) else { return };
        let Some(index) = studio.index_of(id) else { return };
        let feature = studio.features[index].clone();
        if feature.feature_type().is_none() {
            self.message = Some((FeatureStatus::Error, format!("\"{}\" can't be edited: its feature type is not defined (was its Feature Studio deleted?)", feature.name)));
            cx.notify();
            return;
        }
        self.open_dialog(feature, index, false, window, cx);
    }

    fn open_dialog(&mut self, feature: Feature, index: usize, is_new: bool, window: &mut Window, cx: &mut Context<Self>) {
        let units = self.doc.read(cx).session.doc.settings.units.clone();
        let labeler = self.labeler(cx);
        let is_derived = feature.type_id == "importDerived";
        let dialog = cx.new(|cx| FeatureDialog::new(feature, is_new, units, labeler, cx));
        if is_derived {
            let provider = self.doc.read(cx).derived_provider();
            let doc = self.doc.read(cx).session.doc.clone();
            let me = self.tab.to_string();
            let sources: Vec<crate::feature_dialog::DerivedSource> = crate::derived::DocStudios::sources(&doc)
                .into_iter()
                .filter(|(id, _)| *id != me)
                .map(|(id, name)| {
                    use noshape_model::regen::derived::DerivedProvider as _;
                    let parts = provider.context(&id).map(|c| c.bodies.iter().map(|b| (b.id.clone(), b.name.clone())).collect()).unwrap_or_default();
                    crate::feature_dialog::DerivedSource { tab: id, name, parts }
                })
                .collect();
            dialog.update(cx, |d, _| d.derived_sources = sources);
        }
        let sub = cx.subscribe_in(&dialog, window, |this: &mut Self, _, ev: &DialogEvent, window, cx| match ev {
            DialogEvent::Changed => {
                this.schedule_regen(window, cx);
                this.refresh_scene(false, cx);
            }
            DialogEvent::Accept { repeat } => this.accept_edit(*repeat, window, cx),
            DialogEvent::Cancel => this.cancel_edit(window, cx),
            DialogEvent::QueryFocused { .. } => this.refresh_scene(false, cx),
        });
        dialog.update(cx, |d, cx| window.focus(&d.focus_handle(cx), cx));
        self.edit = Some(EditSession { dialog, index, is_new, _sub: sub });
        self.schedule_regen(window, cx);
        cx.notify();
    }

    fn accept_edit(&mut self, repeat: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.leave_sketch(cx);
        let Some(e) = self.edit.take() else { return };
        self.sync_manipulator(cx);
        let feature = e.dialog.read(cx).feature.clone();
        let type_id = feature.type_id.clone();
        let name = feature.name.clone();
        let (index, is_new) = (e.index, e.is_new);
        self.doc.update(cx, |doc, cx| {
            let desc = if is_new { format!("Insert feature : {name}") } else { format!("Edit : {name}") };
            doc.edit_studio(self.tab, &desc, cx, |ps| {
                if is_new {
                    let at = index.min(ps.features.len());
                    ps.features.insert(at, feature);
                    if let Some(r) = ps.rollback {
                        ps.rollback = Some(r + 1);
                    }
                } else if let Some(slot) = ps.features.get_mut(index) {
                    *slot = feature;
                }
            });
        });
        window.focus(&self.focus, cx);
        if repeat {
            self.start_feature(&type_id, window, cx);
        }
        cx.notify();
    }

    fn cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.leave_sketch(cx);
        if self.edit.take().is_some() {
            self.sync_manipulator(cx);
            window.focus(&self.focus, cx);
            self.schedule_regen(window, cx);
            cx.notify();
        }
    }

    fn delete_features(&mut self, ids: Vec<FeatureId>, cx: &mut Context<Self>) {
        if ids.is_empty() {
            return;
        }
        let names: Vec<String> = self.studio(cx).map(|s| ids.iter().filter_map(|id| s.feature(*id).map(|f| f.name.clone())).collect()).unwrap_or_default();
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, &format!("Delete : {}", names.join(", ")), cx, |ps| {
                for id in &ids {
                    ps.remove(*id);
                }
            });
        });
        self.selected_features.clear();
    }

    fn set_suppressed(&mut self, ids: Vec<FeatureId>, suppressed: bool, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            let verb = if suppressed { "Suppress" } else { "Unsuppress" };
            doc.edit_studio(self.tab, verb, cx, |ps| {
                for id in &ids {
                    if let Some(f) = ps.feature_mut(*id) {
                        f.suppressed = suppressed;
                    }
                }
            });
        });
    }

    fn set_rollback(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Move rollback bar", cx, |ps| {
                ps.rollback = index.filter(|i| *i < ps.features.len());
            });
        });
    }

    fn move_feature(&mut self, id: FeatureId, to: usize, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Reorder features", cx, |ps| {
                ps.reorder(id, to);
            });
        });
    }

    // ------------------------------------------------------------ actions

    fn view(&mut self, v: StandardView, cx: &mut Context<Self>) {
        self.viewport.update(cx, |vp, cx| vp.animate_to_view(v, cx));
    }

    fn on_delete(&mut self, _: &DeleteSelection, window: &mut Window, cx: &mut Context<Self>) {
        if self.sketch.is_some() {
            self.sketch_delete(window, cx);
            return;
        }
        let ids = std::mem::take(&mut self.selected_features);
        self.delete_features(ids, cx);
    }

    fn on_clear_selection(&mut self, _: &ClearSelection, _: &mut Window, cx: &mut Context<Self>) {
        self.selection.clear();
        self.selected_features.clear();
        self.refresh_scene(false, cx);
        cx.notify();
    }

    fn view_normal(&mut self, cx: &mut Context<Self>) {
        // Normal to the hovered or first selected planar face / plane / sketch.
        let r = self.hover.clone().or_else(|| self.selection.first().cloned());
        let Some(r) = r else { return };
        let frame = match &r {
            Reference::Plane(_) | Reference::FeaturePlane(_) => self.display.planes.iter().find(|p| p.reference == r).map(|p| p.frame),
            Reference::SketchRegion { sketch, .. } | Reference::SketchEntity { sketch, .. } => self.display.sketches.iter().find(|s| s.feature == *sketch).map(|s| s.frame),
            _ => None,
        };
        if let Some(f) = frame {
            let cur = self.viewport.read(cx).camera.forward();
            let fwd = if cur.dot(-f.normal) > 0.999 { f.normal } else { -f.normal };
            self.viewport.update(cx, |vp, cx| vp.animate_to_direction(fwd, f.y, cx));
        }
    }

    // ------------------------------------------------------------ render pieces

    fn render_toolbar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let mut bar = h_flex().h(px(40.)).px_2().gap_1().bg(pal.panel).border_b_1().border_color(pal.border).overflow_hidden();
        for (gi, group) in TOOLBAR.iter().enumerate() {
            if gi > 0 {
                bar = bar.child(div().w(px(1.)).h(px(22.)).mx_1().bg(pal.border));
            }
            for items in group.iter() {
                let Some(primary) = items.iter().find_map(|id| features::get(id)) else { continue };
                bar = bar.child(self.tool_button(primary, &items[1..], cx));
            }
        }
        // Custom features of the document's Feature Studios.
        let custom = self.doc.read(cx).custom_features().feature_types();
        if !custom.is_empty() {
            bar = bar.child(div().w(px(1.)).h(px(22.)).mx_1().bg(pal.border));
            bar = bar.child(self.custom_tools(custom, cx));
        }
        bar
        .into_any_element()
    }

    fn tool_button(&mut self, ty: &'static FeatureType, extra: &'static [&'static str], cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let tooltip: SharedString = match ty.shortcut {
            Some(s) => format!("{}  ({})", ty.name, s.replace("shift-", "Shift+").to_uppercase()).into(),
            None => ty.name.into(),
        };
        let id = ty.id;
        let main = Button::new(SharedString::from(format!("tb-{id}")))
            .icon(Icon::empty().path(feature_icon(id)).text_color(pal.text).with_size(px(18.)))
            .ghost()
            .with_size(px(30.))
            .tooltip(tooltip)
            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.start_feature(id, window, cx)));
        if extra.is_empty() {
            return main.into_any_element();
        }
        let view = cx.entity();
        h_flex()
            .child(main)
            .child(
                Button::new(SharedString::from(format!("tbm-{id}")))
                    .icon(Icon::new(IconName::ChevronDown).xsmall())
                    .ghost()
                    .xsmall()
                    .w(px(14.))
                    .dropdown_menu(move |mut menu, _, _| {
                        for other in extra.iter().chain(std::iter::once(&id)) {
                            let Some(t) = features::get(other) else { continue };
                            let v = view.clone();
                            let oid: &'static str = t.id;
                            menu = menu.item(PopupMenuItem::new(t.name).icon(Icon::empty().path(feature_icon(oid))).on_click(move |_, window, cx| {
                                v.update(cx, |this, cx| this.start_feature(oid, window, cx));
                            }));
                        }
                        menu
                    }),
            )
            .into_any_element()
    }

    /// Toolbar group of custom features: the first few as buttons, all of
    /// them in a "Custom features" dropdown.
    fn custom_tools(&mut self, custom: Vec<&'static FeatureType>, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let mut group = h_flex();
        for ty in custom.iter().take(4) {
            group = group.child(self.tool_button(ty, &[], cx));
        }
        let view = cx.entity();
        group
            .child(
                Button::new("tb-custom-features")
                    .icon(Icon::empty().path(icon_path("custom-feature")).text_color(pal.text).with_size(px(18.)))
                    .ghost()
                    .with_size(px(30.))
                    .tooltip("Custom features (Feature Studios)")
                    .dropdown_caret(true)
                    .dropdown_menu(move |mut menu, _, _| {
                        menu = menu.label("Custom features");
                        for t in &custom {
                            let v = view.clone();
                            let id: &'static str = t.id;
                            menu = menu.item(PopupMenuItem::new(t.name).icon(Icon::empty().path(feature_icon(id))).on_click(move |_, window, cx| {
                                v.update(cx, |this, cx| this.start_feature(id, window, cx));
                            }));
                        }
                        menu
                    }),
            )
            .into_any_element()
    }

    fn render_feature_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(studio) = self.studio(cx).cloned() else { return div().into_any_element() };
        let rollback = studio.rollback_index();
        let mut rows: Vec<AnyElement> = Vec::new();
        // Default geometry.
        for (label, r) in [
            ("Origin", Reference::Origin),
            ("Top", Reference::Plane(DatumPlane::Top)),
            ("Front", Reference::Plane(DatumPlane::Front)),
            ("Right", Reference::Plane(DatumPlane::Right)),
        ] {
            let selected = self.selection.contains(&r);
            let r2 = r.clone();
            let r3 = r.clone();
            let icon = if label == "Origin" { "point" } else { "plane" };
            rows.push(
                h_flex()
                    .id(SharedString::from(format!("dg-{label}")))
                    .h(px(24.))
                    .px_2()
                    .gap_2()
                    .rounded_sm()
                    .text_sm()
                    .text_color(pal.text_muted)
                    .when(selected, |d| d.bg(pal.accent_soft).text_color(pal.text))
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(svg().path(icon_path(icon)).size_3p5().text_color(pal.text_faint))
                    .child(label)
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        let new = if *h { Some(r2.clone()) } else { None };
                        if this.hover != new {
                            this.hover = new;
                            this.refresh_scene(false, cx);
                        }
                    }))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(e) = &this.edit {
                            e.dialog.update(cx, |d, cx| {
                                d.add_pick(r3.clone(), cx);
                            });
                        } else if let Some(pos) = this.selection.iter().position(|x| *x == r3) {
                            this.selection.remove(pos);
                        } else {
                            this.selection.push(r3.clone());
                        }
                        this.refresh_scene(false, cx);
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        rows.push(div().h(px(6.)).into_any_element());

        let editing_id = self.edit.as_ref().map(|e| e.dialog.read(cx).feature.id);
        let editing_new_at = self.edit.as_ref().filter(|e| e.is_new).map(|e| e.index);
        let mut open_folder: Option<u32> = None;
        for (i, f) in studio.features.iter().enumerate() {
            if editing_new_at == Some(i) {
                rows.push(self.render_editing_placeholder(cx));
            }
            if i == rollback {
                rows.push(self.render_rollback_bar(i, cx));
            }
            let folder = f.folder.and_then(|id| studio.folder(id));
            if f.folder != open_folder {
                open_folder = f.folder;
                if let Some(fd) = folder {
                    rows.push(self.render_folder_row(fd, cx));
                }
            }
            if folder.is_some_and(|fd| fd.collapsed) && editing_id != Some(f.id) {
                continue;
            }
            let row = self.render_feature_row(i, f, i >= rollback, editing_id == Some(f.id), cx);
            rows.push(if folder.is_some() { div().pl_3().child(row).into_any_element() } else { row });
        }
        if editing_new_at.is_some_and(|i| i >= studio.features.len()) {
            rows.push(self.render_editing_placeholder(cx));
        }
        if rollback >= studio.features.len() {
            rows.push(self.render_rollback_bar(studio.features.len(), cx));
        }

        // Parts list.
        let parts: Vec<AnyElement> = self
            .display
            .bodies
            .iter()
            .map(|b| {
                let r = Reference::Body(b.id.clone());
                let selected = self.selection.contains(&r);
                let c = b.color;
                let swatch = gpui_kit::Rgba { r: c[0].powf(1.0 / 2.2), g: c[1].powf(1.0 / 2.2), b: c[2].powf(1.0 / 2.2), a: 1.0 };
                let r2 = r.clone();
                h_flex()
                    .id(SharedString::from(format!("part-{}", b.id)))
                    .h(px(24.))
                    .px_2()
                    .gap_2()
                    .rounded_sm()
                    .text_sm()
                    .when(selected, |d| d.bg(pal.accent_soft))
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(div().size(px(10.)).rounded_sm().bg(swatch))
                    .child(div().flex_1().truncate().child(b.name.clone()))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some(e) = &this.edit {
                            e.dialog.update(cx, |d, cx| {
                                d.add_pick(r2.clone(), cx);
                            });
                        } else if let Some(pos) = this.selection.iter().position(|x| *x == r2) {
                            this.selection.remove(pos);
                        } else {
                            this.selection.push(r2.clone());
                        }
                        this.refresh_scene(false, cx);
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect();

        v_flex()
            .size_full()
            .bg(pal.panel)
            .border_r_1()
            .border_color(pal.border)
            .child(
                h_flex()
                    .h(px(32.))
                    .px_3()
                    .justify_between()
                    .border_b_1()
                    .border_color(pal.border)
                    .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text_muted).child(format!("FEATURES ({})", studio.features.len())))
                    .when(self.regenerating, |d| d.child(div().text_xs().text_color(pal.text_faint).child("regenerating…"))),
            )
            .child(v_flex().id("feature-list").key_context("FeatureList").flex_1().p_1().gap_px().overflow_y_scroll().children(rows))
            .child(
                v_flex()
                    .max_h(px(220.))
                    .border_t_1()
                    .border_color(pal.border)
                    .child(h_flex().h(px(28.)).px_3().child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text_muted).child(format!("PARTS ({})", self.display.bodies.len()))))
                    .child(v_flex().id("parts-list").p_1().overflow_y_scroll().children(parts)),
            )
            .into_any_element()
    }

    fn render_folder_row(&self, fd: &noshape_model::partstudio::Folder, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let id = fd.id;
        let collapsed = fd.collapsed;
        let row = h_flex()
            .id(SharedString::from(format!("folder-{id}")))
            .h(px(26.))
            .px_2()
            .gap_2()
            .rounded_sm()
            .text_sm()
            .text_color(pal.text_muted)
            .hover(|d| d.bg(pal.panel_hover))
            .child(Icon::new(if collapsed { IconName::ChevronRight } else { IconName::ChevronDown }).size_3().text_color(pal.text_faint))
            .child(svg().path(icon_path("folder")).size_4().text_color(pal.text_faint))
            .child(match self.folder_renaming.as_ref().filter(|(fid, _, _)| *fid == id) {
                Some((_, st, _)) => div().flex_1().child(gpui_kit::component::input::Input::new(st).xsmall()).into_any_element(),
                None => div().flex_1().truncate().child(fd.name.clone()).into_any_element(),
            })
            .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                if ev.click_count() >= 2 {
                    this.start_folder_rename(id, window, cx);
                } else {
                    this.toggle_folder(id, cx);
                }
            }));
        let view = cx.entity();
        row.context_menu(move |menu, _, _| {
            let v = view.clone();
            let v2 = view.clone();
            let v3 = view.clone();
            menu.item(PopupMenuItem::new(if collapsed { "Expand" } else { "Collapse" }).on_click(move |_, _, cx| v.update(cx, |this, cx| this.toggle_folder(id, cx))))
                .item(PopupMenuItem::new("Rename folder").on_click(move |_, window, cx| v2.update(cx, |this, cx| this.start_folder_rename(id, window, cx))))
                .separator()
                .item(PopupMenuItem::new("Remove folder").on_click(move |_, _, cx| v3.update(cx, |this, cx| this.dissolve_folder(id, cx))))
        })
        .into_any_element()
    }

    fn render_editing_placeholder(&self, cx: &App) -> AnyElement {
        let pal = p();
        let name = self.edit.as_ref().map(|e| e.dialog.read(cx).feature.name.clone()).unwrap_or_default();
        h_flex()
            .h(px(26.))
            .px_2()
            .gap_2()
            .rounded_sm()
            .bg(pal.accent_soft)
            .border_1()
            .border_color(pal.accent)
            .text_sm()
            .child(div().size(px(6.)).rounded_full().bg(pal.accent))
            .child(name)
            .into_any_element()
    }

    fn render_rollback_bar(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        div()
            .id(SharedString::from(format!("rollback-{index}")))
            .h(px(8.))
            .mx_1()
            .flex()
            .items_center()
            .cursor(CursorStyle::ResizeUpDown)
            .child(div().w_full().h(px(3.)).rounded_full().bg(pal.accent))
            .on_drag(RollbackDrag, |_, _, _, cx| cx.new(|_| RollbackDrag))
            .on_click(cx.listener(|_, _: &ClickEvent, window, cx| {
                window.focus(&cx.focus_handle(), cx);
            }))
            .into_any_element()
    }

    fn render_feature_row(&self, i: usize, f: &Feature, rolled_back: bool, editing: bool, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let state = self.display.feature_state(f.id);
        let status = state.map(|s| s.status).unwrap_or(FeatureStatus::Ok);
        let selected = self.selected_features.contains(&f.id);
        let color = if f.suppressed || rolled_back {
            pal.text_faint
        } else {
            match status {
                FeatureStatus::Error => pal.error,
                FeatureStatus::Warning => pal.warning,
                _ if state.is_some_and(|s| s.underdefined) => pal.info,
                _ => pal.text,
            }
        };
        let id = f.id;
        let msg = state.and_then(|s| s.message.clone());
        let suppressed = f.suppressed;
        let row = h_flex()
            .id(SharedString::from(format!("feat-{}", f.id)))
            .h(px(26.))
            .px_2()
            .gap_2()
            .rounded_sm()
            .text_sm()
            .text_color(color)
            .when(selected, |d| d.bg(pal.accent_soft))
            .when(editing, |d| d.bg(pal.accent_soft).border_1().border_color(pal.accent))
            .hover(|d| d.bg(pal.panel_hover))
            .child(svg().path(feature_icon(&f.type_id)).size_4().text_color(if rolled_back || suppressed { pal.text_faint } else { pal.text_muted }))
            .child(match self.renaming.as_ref().filter(|(rid, _, _)| *rid == f.id) {
                Some((_, st, _)) => div().flex_1().child(gpui_kit::component::input::Input::new(st).xsmall()).into_any_element(),
                None => div().flex_1().truncate().when(suppressed, |d| d.line_through()).child(f.name.clone()).into_any_element(),
            })
            .when(status == FeatureStatus::Error && !rolled_back, |d| d.child(div().size(px(6.)).rounded_full().bg(pal.error)))
            .when_some(msg, |d, m| d.tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(m.clone()).build(window, cx)))
            .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                if ev.click_count() >= 2 {
                    this.edit_feature(id, window, cx);
                    return;
                }
                if ev.modifiers().shift || ev.modifiers().secondary() {
                    if let Some(pos) = this.selected_features.iter().position(|x| *x == id) {
                        this.selected_features.remove(pos);
                    } else {
                        this.selected_features.push(id);
                    }
                } else {
                    this.selected_features = vec![id];
                }
                window.focus(&this.focus, cx);
                cx.notify();
            }))
            .drag_over::<RollbackDrag>(|s, _, _, _| s.border_t_2().border_color(rgb(0x8b7cff)))
            .on_drop(cx.listener(move |this, _: &RollbackDrag, _, cx| this.set_rollback(Some(i), cx)))
            .on_drag(FeatureDrag { id, name: f.name.clone().into() }, |d, _, _, cx| cx.new(|_| d.clone()))
            .drag_over::<FeatureDrag>(|s, _, _, _| s.border_t_2().border_color(rgb(0x5aa9ff)))
            .on_drop(cx.listener(move |this, d: &FeatureDrag, _, cx| this.move_feature(d.id, i, cx)));
        let view = cx.entity();
        row.context_menu(move |menu, _, _| {
            let v = view.clone();
            let v2 = view.clone();
            let v3 = view.clone();
            let v4 = view.clone();
            let v5 = view.clone();
            let v6 = view.clone();
            let v7 = view.clone();
            let v8 = view.clone();
            menu.item(PopupMenuItem::new("Edit…").on_click(move |_, window, cx| v.update(cx, |this, cx| this.edit_feature(id, window, cx))))
                .item(PopupMenuItem::new("Rename").on_click(move |_, window, cx| v5.update(cx, |this, cx| this.start_rename(id, window, cx))))
                .item(PopupMenuItem::new(if suppressed { "Unsuppress" } else { "Suppress" }).on_click(move |_, _, cx| v2.update(cx, |this, cx| this.set_suppressed(vec![id], !suppressed, cx))))
                .item(PopupMenuItem::new("Roll to here").on_click(move |_, _, cx| v3.update(cx, |this, cx| this.set_rollback(Some(i + 1), cx))))
                .separator()
                .item(PopupMenuItem::new("Copy").on_click(move |_, _, cx| {
                    v6.update(cx, |this, cx| {
                        if !this.selected_features.contains(&id) {
                            this.selected_features = vec![id];
                        }
                        this.copy_selected_features(cx);
                    })
                }))
                .item(PopupMenuItem::new("Paste").on_click(move |_, _, cx| v7.update(cx, |this, cx| this.paste_features(cx))))
                .item(PopupMenuItem::new("Add to new folder").on_click(move |_, _, cx| {
                    v8.update(cx, |this, cx| {
                        let ids = if this.selected_features.contains(&id) { this.selected_features.clone() } else { vec![id] };
                        this.add_to_new_folder(ids, cx);
                    })
                }))
                .separator()
                .item(PopupMenuItem::new("Delete").on_click(move |_, _, cx| v4.update(cx, |this, cx| this.delete_features(vec![id], cx))))
        })
        .into_any_element()
    }

    fn render_viewport_overlay(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let pal = p();
        let mut out = Vec::new();
        // Plane labels.
        if self.opts.show_planes {
            let vp = self.viewport.read(cx);
            for pl in self.display.planes.iter().filter(|p| !p.hidden) {
                let corner = pl.frame.origin + pl.frame.x * (-pl.half_width) + pl.frame.y * pl.half_height;
                if let Some(s) = vp.project(corner) {
                    out.push(
                        div()
                            .absolute()
                            .left(px(s.x as f32 + 4.0))
                            .top(px(s.y as f32 + 2.0))
                            .text_xs()
                            .text_color(with_alpha(pal.accent_hover, 0.9))
                            .child(pl.name.clone())
                            .into_any_element(),
                    );
                }
            }
        }
        // Feature dialog.
        if let Some(e) = &self.edit {
            out.push(div().absolute().top_3().left_3().child(e.dialog.clone()).into_any_element());
        }
        // View cube / quick views.
        out.push(self.render_view_controls(cx));
        if let Some(nv) = self.render_named_views(cx) {
            out.push(nv);
        }
        // Message toast.
        if let Some((st, msg)) = self.message.clone() {
            let c = match st {
                FeatureStatus::Error => pal.error,
                FeatureStatus::Warning => pal.warning,
                _ => pal.info,
            };
            out.push(
                h_flex()
                    .id("toast")
                    .absolute()
                    .bottom_3()
                    .left_3()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(pal.panel_raised)
                    .border_1()
                    .border_color(with_alpha(c, 0.5))
                    .text_xs()
                    .text_color(c)
                    .child(msg)
                    .child(Button::new("toast-x").icon(IconName::Close).ghost().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.message = None;
                        cx.notify();
                    })))
                    .into_any_element(),
            );
        }
        out
    }

    fn render_view_controls(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let btn = |id: &'static str, icon: &'static str, tip: &'static str, v: StandardView, cx: &mut Context<Self>| {
            Button::new(id).icon(Icon::empty().path(icon_path(icon))).ghost().xsmall().tooltip(tip).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.view(v, cx)))
        };
        v_flex()
            .absolute()
            .top_3()
            .right_3()
            .gap_0p5()
            .p_1()
            .rounded_md()
            .bg(with_alpha(pal.panel_raised, 0.85))
            .border_1()
            .border_color(pal.border)
            .child(self.render_display_menu(cx))
            .child(btn("v-iso", "iso-view", "Isometric (Shift+7)", StandardView::Isometric, cx))
            .child(btn("v-front", "front-view", "Front (Shift+1)", StandardView::Front, cx))
            .child(btn("v-top", "top-view", "Top (Shift+5)", StandardView::Top, cx))
            .child(btn("v-right", "right-view", "Right (Shift+4)", StandardView::Right, cx))
            .child(Button::new("v-named").icon(Icon::empty().path(icon_path("camera"))).ghost().xsmall().tooltip("Named views (Shift+V)").on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_named_views(window, cx))))
            .child(
                Button::new("v-fit").icon(Icon::empty().path(icon_path("zoom-fit"))).ghost().xsmall().tooltip("Zoom to fit (F)").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.viewport.update(cx, |vp, cx| vp.zoom_to_fit(true, cx));
                })),
            )
            .child(
                Button::new("v-planes").icon(Icon::empty().path(icon_path("plane"))).ghost().xsmall().tooltip("Show/hide planes (P)").on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.opts.show_planes = !this.opts.show_planes;
                    let d = this.display.clone();
                    this.apply_display(d, window, cx);
                })),
            )
        .into_any_element()
    }
}

#[derive(Clone)]
struct RollbackDrag;

impl Render for RollbackDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(200.)).h(px(3.)).rounded_full().bg(rgb(0x8b7cff))
    }
}

#[derive(Clone)]
struct FeatureDrag {
    id: FeatureId,
    name: SharedString,
}

impl Render for FeatureDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        div().px_2().py_1().rounded_md().bg(pal.panel_raised).border_1().border_color(pal.accent).text_sm().text_color(pal.text).child(self.name.clone())
    }
}

fn edge_creator(n: &TopoName) -> Option<FeatureId> {
    match n {
        TopoName::Edge { faces, .. } => faces[1].creator().or_else(|| faces[0].creator()),
        TopoName::Boundary { face, .. } => face.creator(),
        other => other.creator(),
    }
}

/// Whether a picked target fits a query filter.
pub fn kind_matches(kind: PickKind, r: &Reference, f: QueryFilter) -> bool {
    let want = |q: QueryFilter| f.intersects(q);
    match kind {
        PickKind::Face => want(QueryFilter::ANY_FACE) || want(QueryFilter::PLANE_LIKE),
        PickKind::Edge => want(QueryFilter::ANY_EDGE) || want(QueryFilter::AXIS),
        PickKind::Vertex => want(QueryFilter::POINT_LIKE),
        PickKind::Plane => want(QueryFilter::PLANE),
        PickKind::Origin => want(QueryFilter::POINT_LIKE),
        PickKind::SketchCurve => want(QueryFilter::SKETCH_EDGE) || want(QueryFilter::ANY_EDGE) || want(QueryFilter::AXIS),
        PickKind::SketchPoint => want(QueryFilter::SKETCH_POINT) || want(QueryFilter::POINT_LIKE),
        PickKind::SketchRegion => want(QueryFilter::SKETCH_REGION),
        PickKind::Connector => want(QueryFilter::MATE_CONNECTOR),
    }
    .then_some(())
    .is_some()
        && !matches!(r, Reference::Body(_) if !want(QueryFilter::BODY))
}

/// Fill the first compatible query parameter(s) with preselected references.
fn preselect(ty: &FeatureType, feature: &mut Feature, selection: &[Reference], built: Option<&BuiltScene>) {
    for s in &ty.params {
        if !s.is_visible(&feature.params) {
            continue;
        }
        if let ParamKind::Query { filter, max_picks } = s.kind {
            let fits: Vec<Reference> = selection
                .iter()
                .filter(|r| {
                    let kind = built.and_then(|b| b.picks.ids_for(r).first().and_then(|id| b.picks.get(*id)).map(|t| t.kind));
                    match kind {
                        Some(k) => kind_matches(k, r, filter),
                        None => matches!(r, Reference::Body(_)) && filter.intersects(QueryFilter::BODY),
                    }
                })
                .cloned()
                .collect();
            if !fits.is_empty() {
                let q = feature.params.query_mut(s.id);
                q.extend(fits.into_iter().take(max_picks.unwrap_or(usize::MAX)));
                return;
            }
        }
    }
}

#[allow(dead_code)]
fn world(p: [f32; 3]) -> DVec3 {
    DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)
}

impl Render for PartStudioView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let toolbar = if self.sketch.is_some() { self.render_sketch_toolbar(cx) } else { self.render_toolbar(cx) };
        let list = self.render_feature_list(cx);
        if self.needs_regen {
            self.needs_regen = false;
            self.schedule_regen(window, cx);
        }
        let right = self.render_right_panel(window, cx);
        let mut overlay = self.render_viewport_overlay(cx);
        overlay.push(div().absolute().top_3().right(px(52.)).child(self.render_panel_toggles(cx)).into_any_element());
        overlay.extend(self.render_sketch_labels(window, cx));
        overlay.extend(self.render_measure_panel(cx));
        overlay.extend(self.render_context_menu());
        overlay.extend(self.render_section_panel(cx));
        overlay.extend(self.render_palette(cx));
        overlay.extend(self.render_shortcut_sheet(cx));
        let in_sketch = self.sketch.is_some();
        let root = v_flex()
            .size_full()
            .key_context(if in_sketch { "PartStudio Sketch" } else { "PartStudio" })
            .track_focus(&self.focus)
            .capture_any_mouse_down(cx.listener(|this, _: &MouseDownEvent, window, cx| {
                // Keep keyboard shortcuts working after clicking the viewport or lists,
                // without stealing focus from an input that just took it.
                if this.palette.take().is_some() {
                    // Clicking anywhere closes the tool search (items still get the click).
                    window.focus(&this.focus, cx);
                    cx.notify();
                } else if !this.focus.contains_focused(window, cx) {
                    window.focus(&this.focus, cx);
                }
            }))
            .on_action(cx.listener(Self::on_delete))
            .on_action(cx.listener(|this, _: &Copy, _, cx| {
                if this.sketch.is_none() {
                    this.copy_selected_features(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Paste, _, cx| {
                if this.sketch.is_none() && this.edit.is_none() {
                    this.paste_features(cx);
                }
            }))
            .on_action(cx.listener(Self::on_clear_selection))
            .on_action(cx.listener(|this, _: &Cancel, window, cx| {
                if this.sketch.is_some() {
                    this.sketch_cancel(cx);
                    return;
                }
                if this.edit.is_some() {
                    this.cancel_edit(window, cx);
                } else {
                    this.selection.clear();
                    this.refresh_scene(false, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &NewSketch, window, cx| this.start_feature("sketch", window, cx)))
            .on_action(cx.listener(|this, _: &SearchTools, window, cx| this.open_palette(window, cx)))
            .on_action(cx.listener(|this, _: &ToggleShortcutHelp, _, cx| {
                this.show_shortcuts = !this.show_shortcuts;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &SectionView, window, cx| this.toggle_section(window, cx)))
            .on_action(cx.listener(|this, _: &NamedViews, window, cx| this.toggle_named_views(window, cx)))
            .on_action(cx.listener(|this, _: &RenameSelected, window, cx| {
                if let Some(id) = this.selected_features.first().copied() {
                    this.start_rename(id, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Extrude, window, cx| this.start_feature("extrude", window, cx)))
            .on_action(cx.listener(|this, _: &Revolve, window, cx| this.start_feature("revolve", window, cx)))
            .on_action(cx.listener(|this, _: &Fillet, window, cx| this.start_feature("fillet", window, cx)))
            .on_action(cx.listener(|this, _: &ViewFront, _, cx| this.view(StandardView::Front, cx)))
            .on_action(cx.listener(|this, _: &ViewBack, _, cx| this.view(StandardView::Back, cx)))
            .on_action(cx.listener(|this, _: &ViewLeft, _, cx| this.view(StandardView::Left, cx)))
            .on_action(cx.listener(|this, _: &ViewRight, _, cx| this.view(StandardView::Right, cx)))
            .on_action(cx.listener(|this, _: &ViewTop, _, cx| this.view(StandardView::Top, cx)))
            .on_action(cx.listener(|this, _: &ViewBottom, _, cx| this.view(StandardView::Bottom, cx)))
            .on_action(cx.listener(|this, _: &ViewIsometric, _, cx| this.view(StandardView::Isometric, cx)))
            .on_action(cx.listener(|this, _: &ViewNormal, _, cx| this.view_normal(cx)))
            .on_action(cx.listener(|this, _: &ZoomToFit, _, cx| this.viewport.update(cx, |vp, cx| vp.zoom_to_fit(true, cx))))
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| {
                this.viewport.update(cx, |vp, cx| {
                    let s = vp.size();
                    vp.camera.zoom_at(0.8, s * 0.5, s);
                    vp.request_redraw(cx);
                })
            }))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| {
                this.viewport.update(cx, |vp, cx| {
                    let s = vp.size();
                    vp.camera.zoom_at(1.25, s * 0.5, s);
                    vp.request_redraw(cx);
                })
            }))
            .on_action(cx.listener(|this, _: &TogglePlanes, window, cx| {
                this.opts.show_planes = !this.opts.show_planes;
                let d = this.display.clone();
                this.apply_display(d, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSketches, window, cx| {
                this.opts.show_sketches = !this.opts.show_sketches;
                let d = this.display.clone();
                this.apply_display(d, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleProjection, _, cx| {
                this.viewport.update(cx, |vp, cx| {
                    let p = match vp.camera.projection {
                        noshape_render::Projection::Orthographic => noshape_render::Projection::Perspective,
                        noshape_render::Projection::Perspective => noshape_render::Projection::Orthographic,
                    };
                    vp.set_projection(p, cx);
                })
            }))
            .on_action(cx.listener(|this, a: &RotateLeft, _, cx| this.rotate_by(a, -15.0, 0.0, cx)))
            .on_action(cx.listener(|this, a: &RotateRight, _, cx| this.rotate_by(a, 15.0, 0.0, cx)))
            .on_action(cx.listener(|this, a: &RotateUp, _, cx| this.rotate_by(a, 0.0, -15.0, cx)))
            .on_action(cx.listener(|this, a: &RotateDown, _, cx| this.rotate_by(a, 0.0, 15.0, cx)))
            .on_action(cx.listener(|this, a: &RotateLeftFine, _, cx| this.rotate_by(a, -5.0, 0.0, cx)))
            .on_action(cx.listener(|this, a: &RotateRightFine, _, cx| this.rotate_by(a, 5.0, 0.0, cx)))
            .on_action(cx.listener(|this, a: &RotateUpFine, _, cx| this.rotate_by(a, 0.0, -5.0, cx)))
            .on_action(cx.listener(|this, a: &RotateDownFine, _, cx| this.rotate_by(a, 0.0, 5.0, cx)))
            .on_action(cx.listener(|this, a: &RotateLeft90, _, cx| this.rotate_by(a, -90.0, 0.0, cx)))
            .on_action(cx.listener(|this, a: &RotateRight90, _, cx| this.rotate_by(a, 90.0, 0.0, cx)))
            .on_action(cx.listener(|this, a: &RotateUp90, _, cx| this.rotate_by(a, 0.0, -90.0, cx)))
            .on_action(cx.listener(|this, a: &RotateDown90, _, cx| this.rotate_by(a, 0.0, 90.0, cx)))
            .on_action(cx.listener(|this, _: &RollbackUp, _, cx| {
                let r = this.studio(cx).map(|s| s.rollback_index()).unwrap_or(0);
                if r > 0 {
                    this.set_rollback(Some(r - 1), cx);
                }
            }))
            .on_action(cx.listener(|this, _: &RollbackDown, _, cx| {
                let (r, n) = this.studio(cx).map(|s| (s.rollback_index(), s.features.len())).unwrap_or((0, 0));
                if r < n {
                    this.set_rollback(if r + 1 >= n { None } else { Some(r + 1) }, cx);
                }
            }))
            .child(toolbar)
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(div().w(px(250.)).h_full().flex_none().child(list))
                    .child(div().flex_1().h_full().relative().bg(pal.app_bg).child(self.viewport.clone()).children(overlay))
                    .children(right),
            );
        self.with_sketch_actions(root, cx)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Step,
    Stl,
    Obj,
    Iges,
    Brep,
}

impl ExportFormat {
    pub const ALL: [ExportFormat; 5] = [ExportFormat::Step, ExportFormat::Stl, ExportFormat::Obj, ExportFormat::Iges, ExportFormat::Brep];
    pub fn ext(self) -> &'static str {
        match self {
            ExportFormat::Step => "step",
            ExportFormat::Stl => "stl",
            ExportFormat::Obj => "obj",
            ExportFormat::Iges => "igs",
            ExportFormat::Brep => "brep",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            ExportFormat::Step => "STEP (.step)",
            ExportFormat::Stl => "STL (.stl)",
            ExportFormat::Obj => "OBJ (.obj)",
            ExportFormat::Iges => "IGES (.igs)",
            ExportFormat::Brep => "OpenCascade BREP (.brep)",
        }
    }
}

impl PartStudioView {
    /// Export the visible parts (or the selected parts, if any).
    pub fn export(&self, format: ExportFormat, path: &std::path::Path) -> anyhow::Result<()> {
        use noshape_kernel as k;
        let ctx = self.context.as_ref().ok_or_else(|| anyhow::anyhow!("Nothing to export yet"))?;
        let selected: Vec<_> = self.selection.iter().filter_map(|r| match r {
            Reference::Body(b) => Some(b.clone()),
            _ => None,
        }).collect();
        let bodies: Vec<_> = ctx.bodies.iter().filter(|b| selected.is_empty() || selected.contains(&b.id)).collect();
        anyhow::ensure!(!bodies.is_empty(), "There are no parts to export");
        let names: Vec<String> = bodies.iter().map(|b| self.display.body(&b.id).map(|d| d.name.clone()).unwrap_or_else(|| b.name.clone())).collect();
        match format {
            ExportFormat::Step => {
                let shapes: Vec<k::ExportShape> = bodies
                    .iter()
                    .zip(&names)
                    .map(|(b, n)| {
                        let c = self.display.body(&b.id).map(|d| d.color).unwrap_or([0.7, 0.7, 0.7, 1.0]);
                        k::ExportShape { shape: b.shape.clone(), name: Some(n.clone()), color: Some([c[0].powf(1.0 / 2.2), c[1].powf(1.0 / 2.2), c[2].powf(1.0 / 2.2)]) }
                    })
                    .collect();
                k::write_step(path, &shapes, k::LengthUnit::Meter)?;
            }
            ExportFormat::Iges => {
                let shapes: Vec<k::Shape> = bodies.iter().map(|b| b.shape.clone()).collect();
                k::write_iges(path, &shapes, k::LengthUnit::Meter)?;
            }
            ExportFormat::Brep => {
                let c = k::Shape::compound(&bodies.iter().map(|b| b.shape.clone()).collect::<Vec<_>>());
                k::write_brep(&c, path)?;
            }
            ExportFormat::Stl | ExportFormat::Obj => {
                // Mesh formats are unitless; write millimeters (the common convention).
                let c = k::Shape::compound(&bodies.iter().map(|b| b.shape.clone()).collect::<Vec<_>>());
                let mm = c.transformed(&k::Transform::Scale { center: glam::DVec3::ZERO, factor: 1000.0 })?;
                let diag = mm.bounding_box(false).map(|b| b.diagonal()).unwrap_or(100.0);
                let defl = (diag * 2e-4).max(1e-3);
                if format == ExportFormat::Stl {
                    k::write_stl(&mm, path, defl, k::StlFormat::Binary)?;
                } else {
                    k::write_obj(&mm, path, defl)?;
                }
            }
        }
        Ok(())
    }

    pub fn studio_name(&self, cx: &App) -> String {
        self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_else(|| "Part Studio".into())
    }

    fn rotate_by<A>(&mut self, _a: &A, dx_deg: f64, dy_deg: f64, cx: &mut Context<Self>) {
        self.viewport.update(cx, |vp, cx| {
            let mut to = vp.camera;
            let yaw = glam::DQuat::from_axis_angle(glam::DVec3::Z, dx_deg.to_radians());
            let pitch = glam::DQuat::from_axis_angle(to.right(), dy_deg.to_radians());
            to.rotation = (yaw * pitch * to.rotation).normalize();
            vp.animate_to(to, cx);
        });
    }
}
