//! Part Studio tab: toolbar, feature list (with rollback bar), parts list,
//! 3D viewport with selection, and feature dialogs with live preview.

mod analysis;
mod appearance;
mod behaviors;
mod create_selection;
mod hole_callout;
mod triad;
mod material_data;
mod material_panel;
mod config_panel;
mod custom_tables;
pub(crate) mod input_propagation;
mod config_ids;
mod fab_panel;
mod feature_ops;
mod manipulators;
mod menus;
mod section;
mod sketch;
pub(crate) mod shortcut_cmds;
mod analysis_tools;
pub(crate) mod environment_maps;
pub(crate) mod reflection;
mod thickness;
mod flatten_tool;
mod draft_tool;
pub(crate) mod export;
mod shown_dims;
pub(crate) mod toolbar_prefs;
pub mod view_cube;
mod view_tools;
mod in_context;
pub use in_context::{AssemblyContext, ContextOccurrence, PartStudioEvent, context_arrow_icon};

use crate::actions::*;
use crate::sketch_mode::SketchEditor;
use crate::assets::icon_path;
use crate::doc_model::{DocEvent, DocModel};
use crate::feature_dialog::{DialogEvent, FeatureDialog, RefLabeler};
use crate::gpu::Gpu;
use crate::scene_builder::{self, BuiltScene, GpuCache, PickKind, SceneOptions};
use crate::theme::p;
use crate::viewport::{Viewport, ViewportEvent};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use crate::os_menu::OsContextMenuExt as _;
use gpui_kit::*;
use crate::ui::TipExt as _;
use noshape_model::display::{FeatureStatus, StudioDisplay};
use noshape_model::document::TabId;
use noshape_model::features::{self, FeatureType};
use noshape_model::params::{ParamKind, QueryFilter};
use noshape_model::partstudio::{Feature, PartStudio};
use noshape_model::reference::{BodyId, DatumPlane, FeatureId, Reference, TopoName};
use noshape_model::regen::{RegenOptions, Regenerator};
use noshape_render::StandardView;
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;

/// Toolbar layout: groups of feature ids; the first id of an inner list is
/// the face button, the rest live in its dropdown.
/// The Part Studio toolbar as live (dom/toolbar-partstudio.txt,
/// dropdowns-partstudio.json): groups between separators, each a list of
/// buttons; a button with several tools is a split tool with a dropdown.
const TOOLBAR: &[&[&[&str]]] = &[
    &[&["sketch"]],
    &[&["extrude"], &["revolve"], &["sweep"], &["loft"], &["thicken", "enclose"]],
    &[&["fillet", "faceBlend"], &["chamfer"], &["draft", "bodyDraft"], &["rib"], &["shell"], &["hole"], &["externalThread"]],
    &[&["linearPattern", "circularPattern", "curvePattern"], &["mirror"]],
    &[&["booleanBodies"], &["splitPart"], &["transform", "wrap", "decal"], &["deleteBodies"]],
    &[&["modifyFillet"], &["deleteFace"], &["moveFace"], &["replaceFace"]],
    &[
        &["cPlane"],
        &["offsetSurface", "boundarySurface", "fill", "extendSurface", "ruledSurface", "mutualTrim", "constrainedSurface"],
        &["helix", "fitSpline", "projectCurves", "bridgingCurve", "compositeCurve", "intersectionCurve", "trimCurve", "isocline", "offsetCurveOnFace", "isoparametricCurve", "editCurve", "routingCurve"],
        &["mateConnector"],
        &["importDerived"],
        &["assignVariable", "queryVariable"],
        &["compositePart"],
        &["tag"],
    ],
    &[&["frame"], &["frameTrim"], &["gusset"], &["endcap"], &["cutlist"]],
    &[&["sheetMetalStart", "sheetMetalFlange", "sheetMetalHem", "sheetMetalTab", "sheetMetalBend", "sheetMetalJog", "sheetMetalFormed", "sheetMetalLoft", "sheetMetalMakeJoint", "sheetMetalCorner", "sheetMetalBendRelief", "sheetMetalCornerBreak", "sheetMetalEnd"]],
];

/// A narrow window (live part-studio/220-221, dom/toolbar-narrow-900*.txt):
/// each group after Sketch collapses into one split tool with this face and
/// every tool of the group in its dropdown.
const COLLAPSED_FACE: [&str; 9] = ["sketch", "extrude", "hole", "linearPattern", "booleanBodies", "modifyFillet", "cPlane", "frame", "sheetMetalStart"];
/// `toolbar_faces` keys of the collapsed groups (apart from the full ones).
const COLLAPSED_KEY: [&str; 9] = ["~0", "~1", "~2", "~3", "~4", "~5", "~6", "~7", "~8"];

/// Every tool of toolbar group `gi`, in order (a collapsed group's dropdown).
fn collapsed_group(gi: usize) -> &'static [&'static str] {
    static LISTS: std::sync::OnceLock<Vec<Vec<&'static str>>> = std::sync::OnceLock::new();
    LISTS.get_or_init(|| TOOLBAR.iter().map(|g| g.iter().flat_map(|items| items.iter().copied()).collect()).collect())[gi].as_slice()
}

/// Icon file for a feature type id.
pub fn feature_icon(type_id: &str) -> SharedString {
    if noshape_model::custom::is_custom(type_id) {
        return icon_path(features::get(type_id).map(|t| t.icon).unwrap_or("custom-feature"));
    }
    let name = match type_id {
        "cPlane" => "plane",
        "cPoint" => "point",
        "assignVariable" => "variable",
        "queryVariable" => "query-variable",
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
        "sheetMetalJoint" => "joint",
        "sheetMetalMakeJoint" => "sheet-metal-make-joint",
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

/// A menu entry's action on the Part Studio view.
type ViewAction = Box<dyn Fn(&mut PartStudioView, &mut Window, &mut Context<PartStudioView>)>;

struct EditSession {
    dialog: Entity<FeatureDialog>,
    /// Index the feature occupies (new features are inserted at the rollback bar).
    index: usize,
    is_new: bool,
    _sub: Subscription,
    /// Dialog-local undo (Onshape: Ctrl+Z inside an open dialog or sketch
    /// undoes the last change to the draft, not the document's last edit).
    history: DraftHistory,
}

/// Snapshots of the draft feature while its dialog is open.
pub(crate) struct DraftHistory {
    undo: Vec<Feature>,
    redo: Vec<Feature>,
    /// The draft as of the last checkpoint.
    last: Feature,
    /// A viewport gesture is in progress: its steps become one undo entry.
    dragging: bool,
}

impl DraftHistory {
    const LIMIT: usize = 200;

    pub(crate) fn new(f: &Feature) -> Self {
        Self { undo: Vec::new(), redo: Vec::new(), last: f.clone(), dragging: false }
    }

    /// Record `current` if it differs from the last checkpoint.
    pub(crate) fn checkpoint(&mut self, current: &Feature) -> bool {
        if *current == self.last {
            return false;
        }
        let prev = std::mem::replace(&mut self.last, current.clone());
        self.undo.push(prev);
        if self.undo.len() > Self::LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        true
    }

    /// The draft to restore for Undo (`current` is the live draft).
    pub(crate) fn undo(&mut self, current: &Feature) -> Option<Feature> {
        // Unrecorded changes (e.g. mid-drag) are undone first.
        self.checkpoint(current);
        let prev = self.undo.pop()?;
        self.redo.push(std::mem::replace(&mut self.last, prev.clone()));
        Some(prev)
    }

    pub(crate) fn redo(&mut self) -> Option<Feature> {
        let next = self.redo.pop()?;
        self.undo.push(std::mem::replace(&mut self.last, next.clone()));
        Some(next)
    }
}

/// Measurements of the current selection: (selection, labelled results).
type MeasureCache = (Vec<Reference>, Vec<(String, noshape_model::regen::measure::Measurement)>);

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
    /// Custom features shown in the toolbar (for its width).
    custom_feature_count: usize,
    focus: FocusHandle,
    fitted_once: bool,
    message: Option<(FeatureStatus, String)>,
    /// A sketch cancelled with ✕: its draft, index and whether it was new,
    /// for the toast's Restore link (live sketch/68), with the toast text.
    canceled_edit: Option<(Feature, usize, bool, String)>,
    sketch: Option<SketchEditor>,
    context: Option<Arc<noshape_model::regen::context::Context>>,
    context_menu: Option<(Point<Pixels>, Entity<crate::os_menu::OsMenu>, Subscription)>,
    measure_cache: Option<MeasureCache>,
    show_mass: bool,
    right_panel: Option<config_panel::RightPanel>,
    panel_inputs: std::collections::HashMap<String, (Entity<gpui_kit::component::input::InputState>, String)>,
    panel_subs: Vec<Subscription>,
    /// Panel cell that had the keyboard focus at the last render, and the
    /// commit actions of all cells: a cell is committed when the focus
    /// leaves it (GPUI does not deliver blur events for these inputs).
    panel_focused: Option<String>,
    panel_commits: std::collections::HashMap<String, config_panel::CellCommit>,
    config_values: noshape_model::configuration::ConfigValues,
    needs_regen: bool,
    palette: Option<(Entity<gpui_kit::component::input::InputState>, Subscription)>,
    palette_query: String,
    show_shortcuts: bool,
    renaming: Option<(FeatureId, Entity<gpui_kit::component::input::InputState>, Subscription)>,
    dim_input: Option<(noshape_sketch::ConstraintId, Entity<gpui_kit::component::input::InputState>, Subscription)>,
    manip_drag: Option<manipulators::ManipDrag>,
    folder_renaming: Option<(u32, Entity<gpui_kit::component::input::InputState>, Subscription)>,
    /// The header's "Folder name" popup: its input and the features it wraps.
    folder_prompt: Option<(Entity<gpui_kit::component::input::InputState>, Vec<FeatureId>, Subscription)>,
    named_views: Option<(Entity<gpui_kit::component::input::InputState>, Subscription)>,
    /// The Named views dialog's "Select a view" list and the view picked in it.
    named_view_select: Option<(Entity<crate::os_select::OsSelect>, Subscription)>,
    named_view_picked: Option<String>,
    pending_named_view: Option<noshape_model::partstudio::NamedView>,
    flat_cache: Option<fab_panel::FlatCache>,
    flat_task: Option<(usize, Task<()>)>,
    /// Onshape interaction state (hide/isolate, select other, S toolbar...).
    ui: behaviors::UiState,
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
        subs.push(cx.subscribe_in(&viewport, window, |this, vp, ev, window, cx| {
            this.on_viewport_event(vp, ev, window, cx);
            this.push_tool_cursor(cx);
        }));
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
            DocEvent::Info(msg) => {
                this.message = Some((FeatureStatus::Info, msg.clone()));
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
            custom_feature_count: 0,
            focus: cx.focus_handle(),
            fitted_once: false,
            message: None,
            canceled_edit: None,
            sketch: None,
            context: None,
            context_menu: None,
            measure_cache: None,
            show_mass: false,
            right_panel: None,
            panel_inputs: Default::default(),
            panel_subs: Vec::new(),
            panel_focused: None,
            panel_commits: Default::default(),
            config_values: Default::default(),
            needs_regen: false,
            palette: None,
            palette_query: String::new(),
            show_shortcuts: false,
            renaming: None,
            dim_input: None,
            manip_drag: None,
            folder_renaming: None,
            folder_prompt: None,
            named_views: None,
            named_view_select: None,
            named_view_picked: None,
            pending_named_view: None,
            flat_cache: None,
            flat_task: None,
            ui: behaviors::UiState::new(cx),
            _subs: subs,
        };
        // Graphics preferences apply live (camera and render options dialog).
        this._subs.push(cx.observe_global::<crate::graphics_prefs::GraphicsPrefs>(|this, cx| this.apply_graphics_prefs(cx)));
        this.apply_graphics_prefs(cx);
        this.schedule_regen(window, cx);
        this
    }

    /// Apply the Graphics preferences: field of view, shading, line widths
    /// and colors, construction line pattern.
    pub(crate) fn apply_graphics_prefs(&mut self, cx: &mut Context<Self>) {
        let g = crate::graphics_prefs::get(cx);
        self.opts.graphics = g;
        self.viewport.update(cx, |vp, cx| {
            let s = crate::graphics_prefs::lin3(g.specular);
            vp.style.specular = [s[0], s[1], s[2], g.shininess];
            vp.style.construction_dash = g.construction;
            vp.style.ambient_occlusion = g.ambient_occlusion;
            vp.camera.fov_y = (g.fov_deg as f64).to_radians();
            cx.notify();
        });
        self.rebuild_scene(cx);
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
                // Configured parameters preview through the active configuration.
                let mut draft = draft;
                ps.configuration.store_edit(slot, &mut draft, &self.config_values);
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
        // Paused regeneration (Onshape): edits wait until "Regenerate".
        if self.ui.regen_paused.is_some() && !std::mem::take(&mut self.ui.force_regen) {
            cx.notify();
            return;
        }
        let Some((studio, upto)) = self.effective_studio(cx) else { return };
        let units = self.doc.read(cx).session.doc.settings.units;
        let config = self.current_config(&studio);
        let doc_vars = noshape_model::variables::element_variables(&self.doc.read(cx).session.doc, self.tab);
        let derived: Arc<dyn noshape_model::regen::derived::DerivedProvider> = self.doc.read(cx).derived_provider();
        let derived2 = derived.clone();
        let custom: Arc<dyn noshape_model::custom::CustomFeatureProvider> = self.doc.read(cx).custom_features();
        let custom2 = custom.clone();
        let regen = self.regen.clone();
        let regen2 = self.regen.clone();
        self.regen_seq += 1;
        let seq = self.regen_seq;
        let editing_sketch = self.sketch.as_ref().map(|s| s.feature).or_else(|| self.editing_sketch_id(cx));
        // View in high quality (Shift+R): a 4x finer chord tolerance.
        let deflection = RegenOptions::default().relative_deflection * if self.ui.high_quality { 0.25 } else { 1.0 };
        self.regen_task = Some(cx.spawn_in(window, async move |this, cx| {
            // Coalesce bursts of edits (typing in a field).
            cx.background_executor().timer(Duration::from_millis(12)).await;
            let result = cx
                .background_spawn(async move {
                    let opts = RegenOptions { upto, editing_sketch, document_variables: doc_vars.clone(), preview_meshes: true, relative_deflection: deflection, derived: Some(derived), custom_features: Some(custom), ..Default::default() };
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
                this.context = Some(result.context.clone());
                scene_builder::log_regen_perf(if refine { "preview" } else { "full" }, &result.display, &studio);
                this.apply_display(Arc::new(result.display), window, cx);
                // Remove / Intersect: the see-through tool body follows.
                this.schedule_tool_preview(window, cx);
            });
            // Progressive meshing: refine preview meshes in the background.
            if refine {
                let regen2 = regen2;
                let fine = cx
                    .background_spawn(async move {
                        let opts = RegenOptions { upto, editing_sketch, document_variables: doc_vars, preview_meshes: false, relative_deflection: deflection, derived: Some(derived2), custom_features: Some(custom2), ..Default::default() };
                        let r = regen2.lock().regenerate(&studio, &config, &units, &opts);
                        (r, studio)
                    })
                    .await;
                let (fine, studio) = fine;
                let _ = this.update_in(cx, |this: &mut Self, window, cx| {
                    if seq != this.regen_seq {
                        return;
                    }
                    this.context = Some(fine.context.clone());
                    scene_builder::log_regen_perf("refine", &fine.display, &studio);
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

    /// The feature's computed parameter values, formatted in the document's
    /// units ("12.7 mm", "Multiple").
    pub(super) fn computed_values(&self, id: FeatureId, cx: &App) -> Vec<(String, String)> {
        let units = self.doc.read(cx).session.doc.settings.units;
        let Some(st) = self.display.feature_state(id) else { return Vec::new() };
        st.computed
            .iter()
            .map(|(k, v)| {
                let text = match v {
                    noshape_model::display::ComputedValue::Length(m) => noshape_units::format_quantity(noshape_units::Quantity::new(*m, noshape_units::Dim::LENGTH), &units),
                    noshape_model::display::ComputedValue::Text(t) => t.clone(),
                };
                (k.clone(), text)
            })
            .collect()
    }

    /// Read-only values of the open dialog that the model measures (a
    /// Measured variable's Distance, Along X/Y/Z, Along custom).
    fn update_dialog_outputs(&mut self, cx: &mut Context<Self>) {
        let Some(e) = &self.edit else { return };
        let dialog = e.dialog.clone();
        let computed = {
            let d = dialog.read(cx);
            let p = &d.feature.params;
            if d.feature.type_id != "assignVariable" || !p.is("mode", "MEASURED") || !p.is("measurementMode", "DISTANCE") {
                // Values the feature computed (a Hole's Up to next depth).
                let out = self.computed_values(d.feature.id, cx).into_iter().collect();
                dialog.update(cx, |d, cx| d.set_computed(out, cx));
                return;
            }
            let refs = p.query("entityCouple").to_vec();
            let units = self.doc.read(cx).session.doc.settings.units;
            let fmt = |v: f64| noshape_units::format_quantity(noshape_units::Quantity::new(v, noshape_units::Dim::LENGTH), &units);
            let mut out = std::collections::HashMap::new();
            let measured = match (refs.as_slice(), &self.context) {
                ([a, b], Some(ctx)) => noshape_model::regen::measure::entity_distance(ctx, a, b, p.is("minmax", "MAXIMUM")),
                _ => None,
            };
            if let Some(m) = measured {
                out.insert("distance".to_string(), fmt(m.distance));
                out.insert("xOffset".to_string(), fmt(m.delta.x.abs()));
                out.insert("yOffset".to_string(), fmt(m.delta.y.abs()));
                out.insert("zOffset".to_string(), fmt(m.delta.z.abs()));
                if let Some(dir) = p.query("customDirection").first().and_then(|r| self.linear_edge_direction(r)) {
                    out.insert("customOffset".to_string(), fmt(m.delta.dot(dir).abs()));
                }
            }
            out
        };
        dialog.update(cx, |d, cx| d.set_computed(computed, cx));
    }

    fn apply_display(&mut self, display: Arc<StudioDisplay>, window: &mut Window, cx: &mut Context<Self>) {
        let _t = noshape_render::perf::scope("ps.apply_display");
        let geometry_changed = !Arc::ptr_eq(&self.display, &display);
        self.display = display;
        self.update_dialog_outputs(cx);
        // Before building: the scene draws the edited sketch in its editing
        // colors only once `opts.editing_sketch` is set (otherwise an
        // existing sketch opened for editing showed gray until the next edit).
        self.sync_sketch_editor(cx);
        self.ui.measure_lines = None;
        self.opts.vertex_colors = self.vertex_colors(cx);
        self.ui.transform_ghost = self.transform_ghost(cx);
        let shown = self.display_for_scene();
        let opts = self.scene_options(cx);
        let built = scene_builder::build(&shown, &opts, &mut self.cache, self.gpu.uploader());
        let prio = built.picks.priority_fn();
        self.built = Some(built);
        self.ui.pick_filter = None;
        // Thickness analysis follows every edit.
        if geometry_changed && self.ui.thickness.as_ref().is_some_and(|t| !t.parts.is_empty()) {
            self.recompute_thickness(cx);
        }
        // Interference detection doesn't: its Refresh row appears (live psb 10-19 i).
        if geometry_changed
            && let Some(t) = self.ui.analysis_tool.as_mut().filter(|t| t.kind == analysis_tools::ToolKind::Interference && t.result.is_some())
        {
            t.stale = true;
        }
        // Right-drag rotates about the geometry under the cursor.
        let disp = self.display.clone();
        let pivot: Arc<dyn Fn(glam::DVec3, glam::DVec3) -> Option<glam::DVec3> + Send + Sync> = Arc::new(move |o, d| {
            noshape_model::interaction::entities_along_ray(&disp, o, d, 0.0, &|_| false, 1).first().map(|h| o + d.normalize() * h.depth)
        });
        self.viewport.update(cx, |vp, _| {
            vp.pick_priority = prio;
            vp.orbit_pivot_fn = Some(pivot);
        });
        // Update dialog status from the edited feature's state.
        if let Some(e) = &self.edit {
            let id = e.dialog.read(cx).feature.id;
            let st = self.display.feature_state(id).and_then(|s| match s.status {
                FeatureStatus::Error | FeatureStatus::Warning | FeatureStatus::Info => Some((s.status, s.message.clone().unwrap_or_else(|| "Feature failed".into()))),
                _ => None,
            });
            let faulty = self.display.feature_state(id).map(|s| s.faulty.clone()).unwrap_or_default();
            // Live (sketch/00-sketch-dialog-empty.png): a sketch without a
            // plane asks for one in a blue bubble over the graphics.
            const NO_PLANE: &str = "Select a sketch plane";
            let no_plane = e.dialog.read(cx).feature.type_id == "sketch" && st.as_ref().is_some_and(|(k, m)| *k == FeatureStatus::Error && m == NO_PLANE);
            if no_plane {
                self.message = Some((FeatureStatus::Info, NO_PLANE.into()));
            } else if self.message.as_ref().is_some_and(|(_, m)| m == NO_PLANE) {
                self.message = None;
            }
            e.dialog.update(cx, |d, cx| {
                d.set_status(st, cx);
                d.set_faulty(faulty, cx);
            });
        }
        self.orient_for_sketch(cx);
        self.refresh_scene(true, cx);
        if !self.fitted_once {
            self.fitted_once = true;
            if self.display.bounds().is_some() {
                self.viewport.update(cx, |vp, cx| vp.zoom_to_fit(false, cx));
            } else {
                // Empty studio: Onshape's default planes are squares whose side
                // is 0.615 of the viewport height (measured live at 1920x1080).
                self.viewport.update(cx, |vp, cx| {
                    let half_height = self.display.planes.first().map(|p| p.half_height).unwrap_or(0.05);
                    vp.camera.target = glam::DVec3::ZERO;
                    vp.camera.distance = (half_height / 0.615) / (vp.camera.fov_y * 0.5).tan();
                    vp.request_redraw(cx);
                });
            }
        }
        let _ = window;
        cx.notify();
    }

    /// Rebuild highlight state and push the scene to the viewport.
    /// Scene options with the view state that isn't kept in `opts`: zebra
    /// stripes (which can hide the edges).
    fn scene_options(&mut self, cx: &App) -> SceneOptions {
        let mut o = self.opts.clone();
        o.replaced_meshes = self.thickness_meshes(cx);
        o.preview_bodies = self.preview_body_ids();
        o.display_only = self.ui.transform_ghost.clone();
        // Interference detection: the parts see-through around the red overlaps.
        if self.ui.analysis_tool.as_ref().is_some_and(|t| t.kind == analysis_tools::ToolKind::Interference && t.result.as_ref().is_some_and(|r| !r.interferences.is_empty())) {
            o.transparent_bodies = true;
            o.neutral_bodies = true;
        }
        let zebra = self.active_zebra();
        o.zebra = zebra.map(|z| noshape_render::Zebra { stripes: z.stripes, flip: z.flip });
        o.reflection = self.scene_reflection();
        let hide_edges = zebra.is_some_and(|z| !z.show_edges) || self.active_reflection().is_some_and(|r| !r.show_edges);
        if hide_edges && o.display_mode == scene_builder::DisplayMode::ShadedWithEdges {
            o.display_mode = scene_builder::DisplayMode::Shaded;
        }
        o
    }

    /// Rebuild the scene for changed view options (same display).
    pub(super) fn rebuild_scene(&mut self, cx: &mut Context<Self>) {
        if self.built.is_none() {
            return;
        }
        self.opts.vertex_colors = self.vertex_colors(cx);
        self.ui.transform_ghost = self.transform_ghost(cx);
        let shown = self.display_for_scene();
        let opts = self.scene_options(cx);
        self.built = Some(scene_builder::build(&shown, &opts, &mut self.cache, self.gpu.uploader()));
        self.refresh_scene(true, cx);
        cx.notify();
    }

    fn refresh_scene(&mut self, geometry_changed: bool, cx: &mut Context<Self>) {
        let _t = noshape_render::perf::scope("ps.refresh_scene");
        if self.built.is_none() {
            return;
        }
        let (mut selection, mut accent) = match &self.edit {
            Some(e) => {
                let d = e.dialog.read(cx);
                (d.active_refs(), d.selected_refs())
            }
            None => (self.highlighted_selection(), Vec::new()),
        };
        // Hovering a feature or part in the list lights up all of its
        // geometry; a single hovered entity uses the hover highlight.
        let hovered = self.highlighted_hover(cx);
        // A manipulator handle under the cursor hides the geometry hover.
        let on_handle = self.ui.manip_hover || self.ui.triad_hover.is_some();
        let single_hover = if hovered.len() == 1 && self.ui.list_hover.is_none() && self.ui.var_hover.is_none() && self.ui.ct.hover.is_none() && !on_handle { hovered.first().cloned() } else { None };
        // Live (sketch/08-09): while a drawing tool is active, regions under
        // the cursor don't light up.
        let drawing = self.sketch.as_ref().is_some_and(|s| s.tool.draws());
        let single_hover = single_hover.filter(|r| !(drawing && matches!(r, Reference::SketchRegion { .. })));
        if single_hover.is_none() {
            accent.extend(hovered);
        }
        accent.extend(self.ui.appearance_hover.iter().cloned());
        let cs = self.cs_preview();
        if let Some((seeds, _)) = &cs {
            selection.extend(seeds.iter().cloned());
        }
        let mut extra = self.sketch_overlay(&mut selection, &mut accent, cx);
        extra.extend(self.manipulator_overlay(cx));
        extra.extend(self.consumed_picks_overlay(&selection, &accent));
        extra.extend(self.measure_overlay(cx));
        extra.extend(self.midpoint_overlay(cx));
        extra.extend(self.shown_dims_overlay(cx));
        extra.extend(self.hole_callout_overlay(cx));
        extra.extend(self.analysis_tool_overlay());
        extra.extend(self.context_overlay());
        extra.extend(self.context_ref_overlay(&selection, cx));
        self.sync_manipulator(cx);
        self.sync_pick_filter(cx);
        let built = self.built.as_ref().expect("built");
        let mut scene = scene_builder::with_highlights(built, &self.display, &selection, single_hover.as_ref(), &accent, extra, self.gpu.uploader());
        // Parts picked in a dialog field: outlined, not filled (live part-studio/440).
        if self.edit.is_some() {
            for r in selection.iter().chain(&accent) {
                if let Reference::Body(b) = r {
                    for f in self.body_geometry(b) {
                        for &id in built.picks.ids_for(&f) {
                            scene.add_flag(id, noshape_render::state::OUTLINE);
                        }
                    }
                }
            }
        }
        // Interference detection's parts: outlined like dialog field picks
        // (live analysis/81–82; with results every part is).
        if let Some(t) = self.ui.analysis_tool.as_ref().filter(|t| t.kind == analysis_tools::ToolKind::Interference) {
            for r in &t.selection {
                if let Reference::Body(b) = r {
                    for f in self.body_geometry(b) {
                        for &id in built.picks.ids_for(&f) {
                            scene.add_flag(id, noshape_render::state::OUTLINE);
                        }
                    }
                }
            }
        }
        for r in cs.iter().flat_map(|(_, found)| found) {
            for &id in built.picks.ids_for(r) {
                scene.add_flag(id, noshape_render::state::QUERY);
            }
        }
        if let Some(q) = &self.ui.query_hover {
            let refs = match q {
                Reference::Body(b) => self.body_geometry(b),
                Reference::Sketch(f) | Reference::Feature(f) => self.feature_geometry(*f),
                r => vec![r.clone()],
            };
            for r in &refs {
                for &id in built.picks.ids_for(r) {
                    scene.add_flag(id, noshape_render::state::QUERY);
                }
            }
        }
        let (fit_bounds, fit_inset) = (self.zoom_fit_bounds(), self.fit_inset_left());
        // The selection-count cursor: sketch entities while sketching,
        // otherwise the Part Studio selection (not a dialog's picks).
        let count = match (&self.sketch, &self.edit) {
            (Some(ed), _) => ed.selection.len(),
            (None, None) => self.selection.len(),
            _ => 0,
        };
        let (tool_cursor, overlay_cursor) = (self.viewport_tool_cursor(), self.viewport_overlay_cursor());
        self.viewport.update(cx, |vp, cx| {
            (vp.fit_bounds, vp.fit_inset_left) = (fit_bounds, fit_inset);
            vp.selection_count = count;
            (vp.tool_cursor, vp.overlay_cursor) = (tool_cursor, overlay_cursor);
            vp.set_scene(Arc::new(scene), geometry_changed, cx)
        });
    }

    // ------------------------------------------------------------ labels

    fn labeler(&self, cx: &App) -> RefLabeler {
        let names: Vec<(FeatureId, String)> = self.studio(cx).map(|s| s.features.iter().map(|f| (f.id, f.name.clone())).collect()).unwrap_or_default();
        let bodies: Vec<(noshape_model::BodyId, String)> = self.display.bodies.iter().map(|b| (b.id.clone(), b.name.clone())).collect();
        let entity_kinds = sketch_entity_kinds(self.studio(cx), &self.display);
        let renamed: Vec<(Reference, String)> = self.studio(cx).map(|s| s.default_names.clone()).unwrap_or_default();
        // Assembly context geometry reads "Face of Context 1" (live 504).
        let contexts: Vec<(u32, String)> = self.studio(cx).map(|s| s.contexts.iter().map(|c| (c.id, c.name.clone())).collect()).unwrap_or_default();
        Arc::new(move |r: &Reference| {
            let fname = |id: FeatureId| match noshape_model::in_context::context_of(id) {
                Some(c) => contexts.iter().find(|(i, _)| *i == c).map(|(_, n)| n.clone()).unwrap_or_else(|| "context".into()),
                None => names.iter().find(|(i, _)| *i == id).map(|(_, n)| n.clone()).unwrap_or_else(|| "deleted feature".into()),
            };
            if let Some((_, n)) = renamed.iter().find(|(x, _)| x == r) {
                return n.clone();
            }
            match r {
                Reference::Origin => "Origin".into(),
                Reference::Plane(DatumPlane::Top) => "Top plane".into(),
                Reference::Plane(DatumPlane::Front) => "Front plane".into(),
                Reference::Plane(DatumPlane::Right) => "Right plane".into(),
                Reference::Face(n) => format!("Face of {}", n.creator().map(fname).unwrap_or_else(|| "part".into())),
                Reference::Edge(n) => format!("Edge of {}", edge_creator(n).map(fname).unwrap_or_else(|| "part".into())),
                Reference::Vertex(_) => "Vertex".into(),
                Reference::Body(b) => bodies.iter().find(|(i, _)| i == b).map(|(_, n)| n.clone()).unwrap_or_else(|| "Part".into()),
                Reference::SketchEntity { sketch, entity } => format!("{} {}", fname(*sketch), entity_kinds.get(&(*sketch, *entity)).copied().unwrap_or("entity")),
                Reference::SketchRegion { sketch, .. } => format!("Face of {}", fname(*sketch)),
                Reference::Sketch(s) => fname(*s),
                Reference::FeaturePlane(f) => fname(*f),
                Reference::MateConnector { feature, .. } => fname(*feature),
                Reference::Feature(f) => fname(*f),
                Reference::Implicit { on, inference: noshape_model::reference::Inference::Midpoint } if matches!(**on, Reference::Edge(_)) => "Midpoint of edge".into(),
                Reference::Implicit { .. } => "Mate connector".into(),
                Reference::QueryVariable(name) => format!("#{name}"),
            }
        })
    }

    // ------------------------------------------------------------ viewport events

    fn on_viewport_event(&mut self, vp: &Entity<Viewport>, ev: &ViewportEvent, window: &mut Window, cx: &mut Context<Self>) {
        // Clicking the graphics area takes the keyboard focus from a panel
        // field (which then commits), so shortcuts like F don't type into it.
        if self.panel_focused.is_some() && matches!(ev, ViewportEvent::Click { .. } | ViewportEvent::ContextMenu { .. } | ViewportEvent::ToolDown { .. } | ViewportEvent::BoxSelect { .. }) {
            window.focus(&self.focus, cx);
            cx.notify();
        }
        // A press-drag-release gesture (a sketch click that adds a line,
        // a drag, a manipulator drag) is one dialog-undo step.
        if matches!(ev, ViewportEvent::ToolDown { .. }) {
            self.draft_checkpoint(cx);
        }
        if let (ViewportEvent::ToolDown { .. } | ViewportEvent::ToolDrag { .. }, Some(e)) = (ev, &mut self.edit) {
            e.history.dragging = true;
        }
        self.dispatch_viewport_event(vp, ev, window, cx);
        if matches!(ev, ViewportEvent::ToolUp { .. }) {
            if let Some(e) = &mut self.edit {
                e.history.dragging = false;
            }
            self.draft_checkpoint(cx);
        }
    }

    /// Onshape: with a feature dialog open, clicking a feature in the list
    /// feeds the active selection field: feature fields (feature pattern)
    /// take the feature, region fields take every region of a sketch, so a
    /// sketch drawn on a part face can be extruded without picking regions
    /// in the viewport. Returns false when the click is a plain selection.
    fn pick_feature_into_dialog(&mut self, id: FeatureId, cx: &mut Context<Self>) -> bool {
        let Some(e) = &self.edit else { return false };
        let dialog = e.dialog.clone();
        let Some((_, filter, _)) = dialog.read(cx).active_filter() else { return false };
        if dialog.read(cx).feature.id == id {
            return false;
        }
        let sketch = self.display.sketches.iter().find(|s| s.feature == id);
        let refs: Vec<Reference> = if filter.intersects(QueryFilter::FEATURE) {
            vec![Reference::Feature(id)]
        } else if let Some(sk) = sketch.filter(|_| filter.intersects(QueryFilter::SKETCH_REGION)) {
            sk.regions.iter().map(|r| Reference::SketchRegion { sketch: id, key: r.key.clone() }).collect()
        } else if sketch.is_some() && filter.intersects(QueryFilter::SKETCH) {
            vec![Reference::Sketch(id)]
        } else if let Some(sk) = sketch.filter(|_| filter.intersects(QueryFilter::SKETCH_POINT)) {
            // Hole locations: every point of the sketch (Onshape).
            sketch_points(sk)
        } else {
            Vec::new()
        };
        if refs.is_empty() {
            return false;
        }
        // Add the missing ones; if all are already picked, toggle them off.
        let current = dialog.read(cx).selected_refs();
        let missing: Vec<Reference> = refs.iter().filter(|r| !current.contains(r)).cloned().collect();
        let toggle = if missing.is_empty() { refs } else { missing };
        dialog.update(cx, |d, cx| {
            for r in toggle {
                d.add_pick(r, cx);
            }
        });
        self.refresh_scene(false, cx);
        true
    }

    /// Onshape's green ✓ toast (live: "Size copied.", "Matching parameters
    /// successfully configured."), fading like [`Self::flash_info`].
    pub(super) fn flash_success(&mut self, text: String, cx: &mut Context<Self>) {
        self.flash(FeatureStatus::Ok, text, cx);
    }

    /// Show a transient info bubble that fades after a few seconds (errors
    /// and warnings stay until dismissed).
    pub(super) fn flash_info(&mut self, text: String, cx: &mut Context<Self>) {
        self.flash(FeatureStatus::Info, text, cx);
    }

    fn flash(&mut self, status: FeatureStatus, text: String, cx: &mut Context<Self>) {
        let shown = (status, text);
        self.message = Some(shown.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(4)).await;
            let _ = this.update(cx, |this, cx| {
                if this.message.as_ref() == Some(&shown) {
                    this.message = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Record the open dialog's draft for dialog-local undo.
    fn draft_checkpoint(&mut self, cx: &mut Context<Self>) {
        let Some(e) = &mut self.edit else { return };
        if e.history.dragging {
            return;
        }
        let dialog = e.dialog.clone();
        e.history.checkpoint(&dialog.read(cx).feature);
    }

    /// Ctrl+Z / Ctrl+Y while a feature dialog (or sketch) is open.
    fn draft_undo(&mut self, redo: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(e) = &mut self.edit else { return };
        e.history.dragging = false;
        let dialog = e.dialog.clone();
        let restored = if redo { e.history.redo() } else { e.history.undo(&dialog.read(cx).feature) };
        let Some(f) = restored else { return };
        dialog.update(cx, |d, cx| d.restore_draft(f, cx));
        if let Some(ed) = &mut self.sketch {
            use crate::sketch_mode::Tool;
            // Drop in-progress clicks and picks that may name removed entities.
            let tool = ed.tool;
            ed.set_tool(Tool::Select);
            if tool != Tool::Select {
                ed.set_tool(tool);
            }
            ed.selection.clear();
            ed.selected_constraints.clear();
            ed.dim_edit = None;
            ed.hover = None;
            ed.hover_constraint = None;
            self.dim_input = None;
            self.regen_sync(window, cx);
        }
        cx.notify();
    }

    fn dispatch_viewport_event(&mut self, vp: &Entity<Viewport>, ev: &ViewportEvent, window: &mut Window, cx: &mut Context<Self>) {
        match ev {
            ViewportEvent::PointerMove { position, .. } => {
                self.ui.pointer = Some(*position);
                // Hovering a manipulator arrow outlines it (Onshape).
                let on_manip = vp.read(cx).manipulator_at(*position);
                if on_manip != self.ui.manip_hover {
                    self.ui.manip_hover = on_manip;
                    self.refresh_scene(false, cx);
                }
                // The translate triad outlines the handle under the cursor.
                if let Some(t) = self.current_manip(cx).and_then(|m| m.triad()) {
                    let h = self.triad_handle_at(&t, *position, cx);
                    if h != self.ui.triad_hover {
                        self.ui.triad_hover = h;
                        self.refresh_scene(false, cx);
                    }
                }
            }
            ViewportEvent::Click { position, .. } | ViewportEvent::ToolDown { position, .. } => {
                self.ui.pointer = Some(*position);
                // A click elsewhere closes the S toolbar and Select other.
                if self.ui.mini_toolbar.take().is_some() || self.ui.select_other.take().is_some() {
                    cx.notify();
                }
            }
            _ => {}
        }
        // The Create selection dialog takes the picks while it is open,
        // also in sketch mode.
        if let (ViewportEvent::Click { pick, .. }, true) = (ev, self.ui.create_selection.is_some()) {
            if let Some(t) = pick.and_then(|id| self.built.as_ref()?.picks.get(id).cloned()) {
                self.cs_pick(&t.reference, t.kind, cx);
            }
            return;
        }
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
            // Shown dimensions: a click elsewhere hides them (Onshape).
            ViewportEvent::Click { .. } if self.ui.shown_dims.is_some() => {
                self.hide_dimensions(window, cx);
            }
            // An analysis tool's dialog takes the picks.
            ViewportEvent::Click { pick, .. } if self.ui.analysis_tool.is_some() && self.edit.is_none() => {
                if let Some(t) = pick.and_then(|id| self.built.as_ref()?.picks.get(id).cloned()) {
                    self.analysis_pick(&t.reference, cx);
                }
            }
            // Section view dialog: picks fill its fields.
            ViewportEvent::Click { pick, .. } if self.ui.section_dialog.is_some() => {
                if let Some(t) = pick.and_then(|id| self.built.as_ref()?.picks.get(id).cloned()) {
                    self.section_pick(&t.reference, window, cx);
                }
            }
            ViewportEvent::Click { pick, position, modifiers } => {
                let target = pick.and_then(|id| self.built.as_ref()?.picks.get(id).cloned());
                // Alt+click (or isolate without "Select transparent
                // geometry") picks through transparent parts.
                let target = match target {
                    Some(t) if matches!(t.kind, PickKind::Face | PickKind::Edge | PickKind::Vertex) => self.pick_through_transparent(t, *position, modifiers.alt, cx),
                    other => other,
                };
                // Clicking near a hovered edge's midpoint picks the midpoint,
                // unless the active field takes no points (Onshape: picks
                // follow the field's filter).
                let takes_points = self.active_field_filter(cx).is_none_or(|f| f.intersects(QueryFilter::POINT_LIKE));
                let target = match target {
                    Some(t) if matches!(t.kind, PickKind::Edge | PickKind::SketchCurve) && takes_points => Some(self.midpoint_pick(t, *position, cx)),
                    other => other,
                };
                match target {
                    // Double-clicking a sketch in the graphics area edits it.
                    Some(t) if self.edit.is_none() && self.is_double_click(&t.reference) && matches!(t.kind, PickKind::SketchCurve | PickKind::SketchPoint | PickKind::SketchRegion) => {
                        if let Reference::SketchEntity { sketch, .. } | Reference::SketchRegion { sketch, .. } = t.reference {
                            self.selection.clear();
                            self.edit_feature(sketch, window, cx);
                        }
                    }
                    Some(t) => self.pick(t, cx),
                    // Clicking empty space clears the selection (not dialog fields).
                    None if self.edit.is_none() => {
                        self.selection.clear();
                        self.selected_features.clear();
                        self.refresh_scene(false, cx);
                    }
                    None => {}
                }
                cx.notify();
            }
            ViewportEvent::BoxSelect { ids, modifiers } => {
                // Ctrl+drag deselects (Onshape).
                self.box_select(ids, modifiers.control || modifiers.platform, cx);
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
        let units = self.doc.read(cx).session.doc.settings.units;
        let Some(studio) = self.studio(cx) else { return };
        let Some(mut feature) = studio.new_feature(type_id, &units) else { return };
        if let Some(t) = features::get(type_id) {
            self.ui.recent_tools.retain(|x| *x != t.id);
            self.ui.recent_tools.insert(0, t.id);
            self.ui.recent_tools.truncate(8);
        }
        let index = studio.rollback_index();
        self.apply_remembered(&mut feature);
        // Preselection fills the first compatible selection field (Onshape behavior).
        if let Some(ty) = features::get(type_id) {
            // Features selected in the list preselect too (a selected
            // sketch fills Extrude's regions field, Onshape behavior).
            let filters: Vec<QueryFilter> = ty.params.iter().filter_map(|p| match p.kind {
                ParamKind::Query { filter, .. } => Some(filter),
                _ => None,
            }).collect();
            let mut sel = self.selection.clone();
            sel.extend(self.feature_preselection(&filters));
            if !sel.is_empty() {
                preselect(ty, &mut feature, &sel, self.built.as_ref());
                // Plane: the type follows the preselection (Onshape's cPlaneLogic).
                if type_id == "cPlane"
                    && let Some(ctx) = self.context.as_ref()
                {
                    noshape_model::regen::features::cplane::cplane_logic(ctx, &mut feature.params);
                }
            }
        }
        self.selection.clear();
        self.selected_features.clear();
        self.ui.select_other = None;
        self.ui.mini_toolbar = None;
        self.open_dialog(feature, index, true, window, cx);
    }

    pub fn edit_feature(&mut self, id: FeatureId, window: &mut Window, cx: &mut Context<Self>) {
        if self.edit.is_some() {
            self.cancel_edit(window, cx);
        }
        let Some(studio) = self.studio(cx) else { return };
        let Some(index) = studio.index_of(id) else { return };
        // Configured parameters show the active configuration's values.
        let feature = studio.configuration.for_editing(&studio.features[index], &self.config_values);
        if feature.feature_type().is_none() {
            self.message = Some((FeatureStatus::Error, format!("\"{}\" can't be edited: its feature type is not defined (was its Feature Studio deleted?)", feature.name)));
            cx.notify();
            return;
        }
        self.open_dialog(feature, index, false, window, cx);
    }

    /// Tabs a FeatureScript reference parameter can pick (files, other
    /// Part Studios with their parts) and the document name.
    fn set_reference_sources(&self, dialog: &Entity<FeatureDialog>, cx: &mut Context<Self>) {
        use noshape_model::regen::derived::DerivedProvider as _;
        let provider = self.doc.read(cx).derived_provider();
        let doc = self.doc.read(cx).session.doc.clone();
        let name = self.doc.read(cx).name();
        let sources = crate::fs_reference::sources(&doc, &self.tab.to_string(), |id| provider.context(id).map(|c| crate::fs_reference::parts_and_sketches(&c)).unwrap_or_default());
        let studios: Vec<TabId> = doc.tabs.iter().filter(|t| t.id != self.tab && matches!(t.content, noshape_model::document::TabContent::PartStudio(_))).map(|t| t.id).collect();
        let wants_pictures = dialog.read(cx).ty.id == "importDerived" || dialog.read(cx).ty.params.iter().any(|s| matches!(s.kind, noshape_model::params::ParamKind::Reference { kind: noshape_model::params::ReferenceKind::PartStudio }));
        let (path, except, gpu_for_dialog) = (self.doc.read(cx).session.path.clone(), self.tab.to_string(), self.gpu.clone());
        dialog.update(cx, |d, cx| {
            d.ref_sources = sources;
            d.ref_document_name = name;
            d.ref_document_path = Some(path);
            d.ref_except = except;
            d.ref_gpu = Some(gpu_for_dialog);
            cx.notify();
        });
        // Pictures of the other Part Studios and their parts, in the background.
        if wants_pictures && !studios.is_empty() {
            let (gpu, dialog) = (self.gpu.clone(), dialog.downgrade());
            cx.spawn(async move |_, cx| {
                let pictures = cx.background_spawn(crate::fs_reference::render_thumbnails(gpu, doc, studios, |t| t.to_string())).await;
                let _ = dialog.update(cx, |d, cx| {
                    d.ref_thumbs = pictures.into_iter().collect();
                    cx.notify();
                });
            })
            .detach();
        }
    }

    /// A reference parameter's "Import…": the file becomes a tab (Onshape
    /// uploads it into the document) and the parameter points at it.
    fn import_reference_file(&mut self, param: String, file: std::path::PathBuf, cx: &mut Context<Self>) {
        let Ok(bytes) = std::fs::read(&file) else { return };
        let name = file.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
        let tab = noshape_model::file_tab::FileTab::new(&name, &bytes, noshape_model::file_tab::now());
        let mut added = None;
        self.doc.update(cx, |d, cx| d.edit_document(&format!("Import {name}"), cx, |d| added = Some(crate::file_tab_view::add_file_tab(d, tab, None, None))));
        let Some(dialog) = self.edit.as_ref().map(|e| e.dialog.clone()) else { return };
        self.set_reference_sources(&dialog, cx);
        if let Some(id) = added {
            dialog.update(cx, |d, cx| d.set_value(&param, noshape_model::params::ParamValue::Text(id.to_string()), cx));
        }
    }

    fn open_dialog(&mut self, feature: Feature, index: usize, is_new: bool, window: &mut Window, cx: &mut Context<Self>) {
        // The edited row shows as being edited, not selected.
        self.selected_features.clear();
        let units = self.doc.read(cx).session.doc.settings.units;
        let labeler = self.labeler(cx);
        let is_derived = feature.type_id == "importDerived";
        let dialog = cx.new(|cx| FeatureDialog::new(feature, units, labeler, cx));
        if is_derived {
            let provider = self.doc.read(cx).derived_provider();
            let doc = self.doc.read(cx).session.doc.clone();
            let me = self.tab.to_string();
            let sources: Vec<crate::feature_dialog::DerivedSource> = crate::derived::DocStudios::sources(&doc)
                .into_iter()
                .filter(|(id, _)| *id != me)
                .map(|(id, _)| {
                    use noshape_model::regen::derived::DerivedProvider as _;
                    let parts = provider.context(&id).map(|c| c.bodies.iter().map(|b| (b.id.clone(), b.name.clone())).collect()).unwrap_or_default();
                    crate::feature_dialog::DerivedSource { tab: id, parts }
                })
                .collect();
            dialog.update(cx, |d, _| d.derived_sources = sources);
        }
        if dialog.read(cx).ty.params.iter().any(|s| matches!(s.kind, noshape_model::params::ParamKind::Reference { .. })) {
            self.set_reference_sources(&dialog, cx);
        }
        let sub = cx.subscribe_in(&dialog, window, |this: &mut Self, _, ev: &DialogEvent, window, cx| match ev {
            DialogEvent::Changed => {
                this.draft_checkpoint(cx);
                this.auto_operation(cx);
                this.schedule_regen(window, cx);
                this.refresh_scene(false, cx);
            }
            DialogEvent::Accept { repeat } => this.accept_edit(*repeat, window, cx),
            DialogEvent::Cancel => this.cancel_edit(window, cx),
            DialogEvent::QueryFocused => this.refresh_scene(false, cx),
            // Live (psb 10-19 h): Tab past the dialog's last control goes to
            // the navbar; Shift+Tab before its first one to <body> (here the
            // Part Studio itself, so shortcuts keep working).
            DialogEvent::LeaveFocus { forward: true } => window.dispatch_action(Box::new(crate::actions::FocusNavbar), cx),
            DialogEvent::LeaveFocus { forward: false } => window.focus(&this.focus, cx),
            DialogEvent::CreateSelection(field) => this.open_create_selection_for_field(field.clone(), cx),
            DialogEvent::CreateVariable { name, kind, value, description, configuration } => this.create_variable_from_field(name, kind, value, description, *configuration, cx),
            DialogEvent::ImportReference { param, file } => this.import_reference_file(param.clone(), file.clone(), cx),
            // Create version…: this document's dialog, or another document opened in its window.
            DialogEvent::CreateVersion(None) => crate::history_panel::open_create_version_in(this.doc.clone(), window, cx),
            DialogEvent::CreateVersion(Some(path)) => {
                let _ = crate::files::open_document_window(path.clone(), None, this.gpu.clone(), cx);
            }
            DialogEvent::PreviewOpacity(v) => {
                this.ui.preview_opacity = *v;
                this.schedule_preview_before(window, cx);
                let d = this.display.clone();
                this.apply_display(d, window, cx);
            }
            DialogEvent::ConfigureParam(param) => {
                if let Some(input) = this.ui.configure_pick.take() {
                    let fid = this.edit.as_ref().map(|e| e.dialog.read(cx).feature.id);
                    this.cancel_edit(window, cx);
                    if let Some(fid) = fid {
                        this.add_configured(fid, param, &input, cx);
                    }
                    this.right_panel = Some(config_panel::RightPanel::Configurations);
                    cx.notify();
                }
            }
            // Hovering a selection row lights its entity yellow-green (Onshape).
            DialogEvent::HoverRef(r) => {
                if this.ui.query_hover != *r {
                    this.ui.query_hover = r.clone();
                    this.refresh_scene(false, cx);
                }
            }
        });
        // Variables for evaluated-value hints; "Final" only when later
        // features exist (Onshape).
        let mut vars = noshape_model::variables::element_variables(&self.doc.read(cx).session.doc, self.tab);
        vars.extend(self.display.variables.iter().map(|(k, v)| (k.clone(), *v)));
        // Only variables that exist where this feature evaluates (Onshape).
        if let Some(s) = self.studio(cx) {
            for name in noshape_model::variables::variables_defined_later(s, index) {
                vars.remove(&name);
            }
        }
        let later = self.studio(cx).is_some_and(|s| !is_new && index + 1 < s.features.len());
        // Missing references: their creating feature is gone or suppressed.
        let live: std::collections::HashSet<FeatureId> = self.studio(cx).map(|s| s.features.iter().filter(|f| !f.suppressed).map(|f| f.id).collect()).unwrap_or_default();
        let missing: Arc<dyn Fn(&Reference) -> bool + Send + Sync> = Arc::new(move |r: &Reference| match r {
            Reference::Edge(n) | Reference::Vertex(n) => edge_creator(n).is_some_and(|f| !live.contains(&f) && !noshape_model::in_context::is_context_feature(f)),
            // Assembly context geometry is never missing (snapshot).
            r => r.depends_on().is_some_and(|f| !live.contains(&f) && !noshape_model::in_context::is_context_feature(f)),
        });
        let dialog_size = self.ui.dialog_size;
        let region_counts: std::collections::HashMap<FeatureId, usize> = self.display.sketches.iter().map(|s| (s.feature, s.regions.len())).collect();
        let studio_view = cx.entity().downgrade();
        dialog.update(cx, |d, cx| {
            if d.feature.type_id == "sketch" {
                d.extra = Some(Self::sketch_dialog_extra(studio_view));
            }
            d.missing = missing;
            d.sketch_region_count = Arc::new(move |id| region_counts.get(&id).copied().unwrap_or(0));
            d.size = dialog_size;
            d.variables = vars;
            d.final_available = later;
            window.focus(&d.focus_handle(cx), cx);
            d.focus_first_field(window, cx);
        });
        let history = DraftHistory::new(&dialog.read(cx).feature);
        self.ui.orient_pending = self.ui.orient_on_edit && dialog.read(cx).feature.type_id == "sketch";
        self.ui.preview_opacity = 1.0;
        self.ui.preview_before = None;
        self.ui.before_context = None;
        self.edit = Some(EditSession { dialog, index, is_new, _sub: sub, history });
        self.auto_operation(cx);
        self.schedule_regen(window, cx);
        self.schedule_preview_before(window, cx);
        cx.notify();
    }

    fn accept_edit(&mut self, repeat: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.close_create_selection_of_dialog(cx);
        // Onshape (live): a feature with an error can be committed (Enter;
        // the ✓ button is disabled); the model falls back to the state
        // before it and the row shows the error.
        if let Some(ed) = &self.sketch {
            ed.save_options_if_changed();
        }
        self.leave_sketch(cx);
        self.restore_after_sketch(cx);
        let Some(e) = self.edit.take() else { return };
        // Nothing stays selected after a feature is accepted (Onshape).
        self.selected_features.clear();
        self.ui.query_hover = None;
        self.sync_manipulator(cx);
        let feature = e.dialog.read(cx).feature.clone();
        let type_id = feature.type_id.clone();
        let name = feature.name.clone();
        let (index, is_new) = (e.index, e.is_new);
        self.remember_values(&feature);
        let config_values = self.config_values.clone();
        self.doc.update(cx, |doc, cx| {
            let desc = if is_new { format!("Insert feature : {name}") } else { format!("Edit : {name}") };
            doc.edit_studio(self.tab, &desc, cx, |ps| {
                if is_new {
                    // Onshape hides the sketch a new feature consumes.
                    if let Some(sid) = noshape_model::interaction::source_sketch(&ps.features, &feature)
                        && let Some(sk) = ps.feature_mut(sid)
                    {
                        sk.hidden = true;
                    }
                    let at = index.min(ps.features.len());
                    ps.features.insert(at, feature);
                    if let Some(r) = ps.rollback {
                        ps.rollback = Some(r + 1);
                    }
                } else if let Some(slot) = ps.features.get_mut(index) {
                    // Configured parameters go to the active configuration's row.
                    let mut feature = feature;
                    ps.configuration.store_edit(slot, &mut feature, &config_values);
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
        self.close_create_selection_of_dialog(cx);
        // Live Onshape: cancelling a sketch shows "Sketch N has been
        // canceled. Restore" (sketch/68); Restore reopens the draft.
        if self.sketch.is_some()
            && let Some(e) = &self.edit
        {
            let feature = e.dialog.read(cx).feature.clone();
            let text = format!("{} has been canceled.", feature.name);
            self.message = Some((FeatureStatus::Info, text.clone()));
            self.canceled_edit = Some((feature, e.index, e.is_new, text));
        }
        self.leave_sketch(cx);
        self.restore_after_sketch(cx);
        self.ui.query_hover = None;
        if self.edit.take().is_some() {
            self.sync_manipulator(cx);
            window.focus(&self.focus, cx);
            self.schedule_regen(window, cx);
            cx.notify();
        }
    }

    /// Restore link of the "has been canceled" toast: reopen the draft.
    fn restore_canceled_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((feature, index, is_new, _)) = self.canceled_edit.take() else { return };
        self.message = None;
        if self.edit.is_some() {
            self.cancel_edit(window, cx);
            self.message = None;
            self.canceled_edit = None;
        }
        let Some(studio) = self.studio(cx) else { return };
        let index = if is_new { index.min(studio.features.len()) } else {
            match studio.index_of(feature.id) {
                Some(i) => i,
                None => return,
            }
        };
        self.open_dialog(feature, index, is_new, window, cx);
        cx.notify();
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

    // ------------------------------------------------------------ actions

    fn view(&mut self, v: StandardView, cx: &mut Context<Self>) {
        self.push_camera_history(cx);
        self.viewport.update(cx, |vp, cx| vp.animate_to_view(v, cx));
    }

    fn on_delete(&mut self, _: &DeleteSelection, window: &mut Window, cx: &mut Context<Self>) {
        if self.sketch.is_some() {
            self.sketch_delete(window, cx);
            return;
        }
        if self.edit.is_some() {
            return;
        }
        let ids = std::mem::take(&mut self.selected_features);
        if ids.is_empty() {
            // Selected parts are deleted with a Delete part feature.
            self.delete_selected_parts(window, cx);
            return;
        }
        self.delete_features(ids, cx);
    }

    fn on_clear_selection(&mut self, _: &ClearSelection, _: &mut Window, cx: &mut Context<Self>) {
        // With a dialog open, Space belongs to the focused control (it
        // toggles a checkbox, Onshape); the selection lives in the fields.
        if self.edit.is_some() && self.sketch.is_none() {
            cx.propagate();
            return;
        }
        self.selection.clear();
        self.selected_features.clear();
        self.refresh_scene(false, cx);
        cx.notify();
    }

    fn view_normal(&mut self, cx: &mut Context<Self>) {
        // Onshape: normal to the first selected planar face / plane / sketch
        // entity; in a sketch with nothing usable selected, normal to the
        // sketch plane. Only outside a sketch does the hovered entity count
        // (otherwise N in a Top sketch could turn to whatever plane happened
        // to be under the cursor, leaving the sketch plane edge-on).
        let frame_of = |this: &Self, r: &Reference| -> Option<noshape_model::display::PlaneFrame> {
            match r {
                Reference::Plane(_) | Reference::FeaturePlane(_) => this.display.planes.iter().find(|p| &p.reference == r).map(|p| p.frame),
                Reference::SketchRegion { sketch, .. } | Reference::SketchEntity { sketch, .. } => this.display.sketches.iter().find(|s| s.feature == *sketch).map(|s| s.frame),
                Reference::Face(n) => this.context.as_ref()?.bodies.iter().find_map(|b| {
                    let fi = *b.find_faces(n).first()?;
                    let pl = b.shape.sub_shape(noshape_kernel::ShapeType::Face, fi).ok()?.face_plane().ok()??;
                    let (normal, x) = (pl.unit_normal(), pl.unit_x());
                    Some(noshape_model::display::PlaneFrame { origin: pl.origin, x, y: normal.cross(x), normal })
                }),
                _ => None,
            }
        };
        let frame = self
            .selection
            .iter()
            .find_map(|r| frame_of(self, r))
            .or_else(|| self.sketch.as_ref().map(|s| s.frame))
            .or_else(|| self.hover.as_ref().and_then(|r| frame_of(self, r)));
        if let Some(f) = frame {
            self.push_camera_history(cx);
            let cur = self.viewport.read(cx).camera.forward();
            let fwd = if cur.dot(-f.normal) > 0.999 { f.normal } else { -f.normal };
            self.viewport.update(cx, |vp, cx| vp.animate_to_direction(fwd, f.y, cx));
        }
    }

    // ------------------------------------------------------------ render pieces

    /// Width of the full toolbar (tools, carets, separators, gaps).
    fn toolbar_full_width(&self) -> f32 {
        let removed = &self.ui.toolbar.removed;
        let (cell, caret, gap, sep) = (crate::ui::TOOL_CELL, crate::ui::TOOL_CARET_W, 2., 2.);
        // Undo + Redo, then per group a separator and its buttons.
        let mut w = 2. * cell + gap;
        for group in TOOLBAR {
            w += gap + sep;
            for items in group.iter().filter(|items| !items.iter().all(|id| removed.iter().any(|r| r == id))) {
                w += gap + if items[0] == "sketch" { 83. } else { cell } + if items.len() > 1 { caret } else { 0. };
            }
        }
        // The document's custom features (up to 4 + their menu), ⊞.
        let custom = self.custom_feature_count.min(4);
        if self.custom_feature_count > 0 {
            w += gap + sep + (custom as f32 + 1.) * cell;
        }
        w + 2. * gap + sep + cell
    }

    fn render_toolbar(&mut self, viewport_w: f32, cx: &mut Context<Self>) -> AnyElement {
        // Live (navigation/301): a version replaces the toolbar with its
        // view-only bar.
        if let Some(version) = self.doc.read(cx).session.viewing_version() {
            return self.render_version_bar(&version, cx);
        }
        // Onshape toolbar (§3.2): undo/redo first, 2 px group separators,
        // "Search tools…" at the right end.
        // Live: "Search tools…" follows the last tool (it is not pinned to
        // the right edge).
        let mut bar = h_flex().flex_initial().min_w_0().overflow_hidden().gap(px(2.)).child(crate::ui::undo_redo(&self.doc, cx)).child(crate::ui::tool_separator());
        let customizing = self.ui.customizing_toolbar;
        let removed = self.ui.toolbar.removed.clone();
        // Live (part-studio/220): when the tools don't fit, every group after
        // Sketch collapses to one split tool. Room: the window minus the left
        // rail, the toolbar padding and "Search tools…" (172 px with its margin).
        let custom = self.doc.read(cx).custom_features().feature_types();
        self.custom_feature_count = custom.len();
        let collapsed = !customizing && self.toolbar_full_width() > viewport_w - crate::ui::RAIL_W - 4. - 172.;
        for (pos, gi) in self.ui.toolbar.ordered_groups(TOOLBAR.len()).into_iter().enumerate() {
            let group = TOOLBAR[gi];
            if pos > 0 {
                bar = bar.child(crate::ui::tool_separator());
            }
            if collapsed && gi > 0 {
                let items = collapsed_group(gi);
                let live: Vec<&'static str> = items.iter().copied().filter(|id| !removed.iter().any(|r| r == id)).collect();
                let Some(rep) = live.iter().find(|id| **id == COLLAPSED_FACE[gi]).or(live.first()).and_then(|id| features::get(id)) else { continue };
                let key = COLLAPSED_KEY[gi];
                let face = self.ui.toolbar_faces.get(key).and_then(|id| features::get(id)).unwrap_or(rep);
                bar = bar.child(self.tool_button(face, items, key, cx));
                continue;
            }
            if customizing {
                // Customize toolbar: move tool sets left/right.
                bar = bar.child(self.group_mover(gi, -1, cx));
            }
            for items in group.iter() {
                if items.iter().all(|id| removed.iter().any(|r| r == id)) {
                    continue;
                }
                let Some(primary) = items.iter().find(|id| !removed.iter().any(|r| r == *id)).and_then(|id| features::get(id)) else { continue };
                // Onshape: the tool last picked from a dropdown becomes the
                // group's face for the rest of the session.
                let key = items[0];
                let face = self.ui.toolbar_faces.get(key).and_then(|id| features::get(id)).unwrap_or(primary);
                bar = bar.child(self.tool_button(face, items, key, cx));
                if customizing {
                    // Remove the tool (its whole dropdown) from the toolbar.
                    let ids: Vec<String> = items.iter().map(|s| s.to_string()).collect();
                    bar = bar.child(
                        div()
                            .id(SharedString::from(format!("tb-rm-{key}")))
                            .text_xs()
                            .text_color(p().error)
                            .cursor_pointer()
                            .child("×")
                            .tooltip(crate::ui::tooltip("Remove from toolbar"))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.ui.toolbar.removed.extend(ids.iter().cloned());
                                this.ui.toolbar.save();
                                cx.notify();
                            })),
                    );
                }
            }
            if customizing {
                bar = bar.child(self.group_mover(gi, 1, cx));
            }
        }
        if customizing {
            // Removed tools: click to put them back.
            let view = cx.entity();
            let removed_ids = removed.clone();
            bar = bar
                .child(crate::ui::tool_separator())
                .child(
                    Button::new("tb-removed")
                        .label(format!("Removed ({})", removed.len()))
                        .ghost()
                        .xsmall()
                        .dropdown_caret(true)
                        .dropdown_menu(move |mut m, _, _| {
                            for id in &removed_ids {
                                let Some(t) = features::get(id) else { continue };
                                let (v, id) = (view.clone(), id.clone());
                                m = m.item(PopupMenuItem::new(t.name).icon(Icon::empty().path(feature_icon(t.id))).on_click(move |_, _, cx| {
                                    v.update(cx, |this, cx| {
                                        this.ui.toolbar.removed.retain(|r| *r != id);
                                        this.ui.toolbar.save();
                                        cx.notify();
                                    })
                                }));
                            }
                            m
                        }),
                )
                .child({
                    // Tools on the S shortcut toolbar (checked = shown).
                    let v = cx.entity();
                    let on = self.ui.toolbar.shortcut_tools();
                    Button::new("tb-shortcuts").label("Shortcut toolbar (S)").ghost().xsmall().dropdown_caret(true).dropdown_menu(move |mut m, _, _| {
                        for group in TOOLBAR.iter() {
                            for items in group.iter() {
                                for id in items.iter() {
                                    let Some(t) = features::get(id) else { continue };
                                    let (v, id) = (v.clone(), id.to_string());
                                    let checked = on.contains(&id);
                                    m = m.item(PopupMenuItem::new(t.name).checked(checked).on_click(move |_, _, cx| {
                                        v.update(cx, |this, cx| {
                                            this.ui.toolbar.toggle_shortcut(&id);
                                            this.ui.toolbar.save();
                                            cx.notify();
                                        })
                                    }));
                                }
                            }
                        }
                        m.scrollable(true).max_h(px(420.))
                    })
                })
                .child(Button::new("tb-reset").label("Reset to default").ghost().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.ui.toolbar.removed.clear();
                    this.ui.toolbar.group_order.clear();
                    this.ui.toolbar.shortcut_tools = None;
                    this.ui.toolbar.save();
                    cx.notify();
                })))
                .child(Button::new("tb-done").label("Done").primary().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.ui.customizing_toolbar = false;
                    cx.notify();
                })));
        }
        // Custom features of the document's Feature Studios.
        if !custom.is_empty() {
            bar = bar.child(crate::ui::tool_separator());
            bar = bar.child(self.custom_tools(custom.clone(), cx));
        }
        // Live: ⊞ "Add custom features" ends the toolbar.
        bar = bar.child(crate::ui::tool_separator()).child(self.add_custom_features_button(custom, cx));
        let view = cx.entity();
        crate::ui::toolbar()
            .id("part-studio-toolbar")
            // Right-click the toolbar: Customize toolbar… (Onshape).
            .os_context_menu(move |m, _, _| {
                let (v1, v2) = (view.clone(), view.clone());
                m.item(crate::os_menu::OsMenuItem::new("Customize toolbar…").on_click(move |_, _, cx| {
                    v1.update(cx, |this, cx| {
                        this.ui.customizing_toolbar = true;
                        cx.notify();
                    })
                }))
                .item(crate::os_menu::OsMenuItem::new("Reset toolbar to default").on_click(move |_, _, cx| {
                    v2.update(cx, |this, cx| {
                        this.ui.toolbar.removed.clear();
                        this.ui.toolbar.group_order.clear();
                        this.ui.toolbar.save();
                        cx.notify();
                    })
                }))
            })
            .child(bar)
            .child(crate::ui::search_tools_toggle("tb-search", self.palette.is_some()).ml(px(10.)).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_palette(window, cx))))
            .into_any_element()
    }

    /// Customize toolbar: arrow moving a tool set left (-1) or right (+1).
    fn group_mover(&self, group: usize, delta: isize, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id(SharedString::from(format!("tb-mv-{group}-{delta}")))
            .text_xs()
            .text_color(p().accent)
            .cursor_pointer()
            .child(if delta < 0 { "◂" } else { "▸" })
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.ui.toolbar.move_group(TOOLBAR.len(), group, delta);
                this.ui.toolbar.save();
                cx.notify();
            }))
            .into_any_element()
    }

    fn tool_button(&mut self, ty: &'static FeatureType, group: &'static [&'static str], key: &'static str, cx: &mut Context<Self>) -> AnyElement {
        // Onshape: "Extrude (shift+e)", "Mate connector (ctrl+m)".
        let tooltip: SharedString = match ty.shortcut {
            Some(s) => format!("{} ({})", ty.name, s.replace('-', "+").to_lowercase()).into(),
            None => ty.name.into(),
        };
        let id = ty.id;
        // Onshape: a tool whose dialog is open shows the "running" fill.
        let active = self.edit.as_ref().is_some_and(|e| e.dialog.read(cx).feature.type_id == id);
        let icon_name = feature_icon(id);
        let main = if id == "sketch" {
            crate::ui::text_tool_button(SharedString::from(format!("tb-{id}")), &icon_name, "Sketch", active, cx)
        } else if group.len() > 1 {
            crate::ui::split_tool_button(SharedString::from(format!("tb-{id}")), &icon_name, active, cx)
        } else {
            crate::ui::tool_button(SharedString::from(format!("tb-{id}")), &icon_name, active, cx)
        };
        // Tool search with "Highlight tool in toolbar": the selected result's
        // button (or its group) turns yellow (live: `#fcda7f`, border `#fab601`).
        let found = self.palette_found_tool(cx).is_some_and(|f| f == id || group.contains(&f));
        let main = main.when(found, |b| b.bg(gpui_kit::rgb(0xfcda7f)).border_1().border_color(gpui_kit::rgb(0xfab601)));
        let main = main.on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                // Clicking the active tool again closes it (Onshape).
                let active_new = this.edit.as_ref().is_some_and(|e| e.is_new && e.dialog.read(cx).feature.type_id == id);
                if active_new && this.sketch.is_none() {
                    this.cancel_edit(window, cx);
                } else {
                    this.start_feature(id, window, cx);
                }
            }));
        let main = crate::ui::with_rich_tooltip(SharedString::from(format!("tt-{id}")), tooltip, crate::tool_text::description(id).unwrap_or(ty.description), main);
        if group.len() <= 1 {
            return main;
        }
        let view = cx.entity();
        crate::ui::tool_group(
            SharedString::from(format!("tg-{id}")),
            main,
            crate::ui::tool_caret(SharedString::from(format!("tbm-{id}")), cx)
                    .dropdown_menu(move |mut menu, window, _| {
                        // Onshape: the dropdown lists every tool of the group
                        // (the face included), 180 px wide, with shortcut chips.
                        menu = crate::ui::fit_tool_menu(menu.min_w(px(180.)), window);
                        for other in group.iter() {
                            let Some(t) = features::get(other) else { continue };
                            let v = view.clone();
                            let oid: &'static str = t.id;
                            menu = menu.item(crate::ui::menu_tool_item(t.name, &feature_icon(oid), t.shortcut).on_click(move |_, window, cx| {
                                v.update(cx, |this, cx| {
                                    this.ui.toolbar_faces.insert(key, oid);
                                    this.start_feature(oid, window, cx)
                                });
                            }));
                        }
                        menu
                    }),
        )
        .into_any_element()
    }

    /// Viewing a version (live navigation/301, dom/version-view-only-bar.txt):
    /// the toolbar row holds only a white 32 px bar at the top centre,
    /// rounded at the bottom: "Versions are view only. Viewing V1." (14 px /
    /// 600, #333), an outline [⊞ Custom features], a small blue [Return to
    /// Main] and (?).
    fn render_version_bar(&mut self, version: &str, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let branch = {
            let s = &self.doc.read(cx).session;
            s.store.branch(s.branch).map(|b| b.name).unwrap_or_else(|_| "Main".into())
        };
        let custom = self.doc.read(cx).custom_features().feature_types();
        let doc = self.doc.clone();
        let custom_btn = div()
            .id("vb-custom-features")
            .h(px(26.))
            .px(px(8.))
            .flex()
            .items_center()
            .gap(px(4.))
            .rounded(px(3.))
            .border_1()
            .border_color(gpui_kit::rgb(0xaaaaaa))
            .bg(gpui_kit::rgb(0xffffff))
            .cursor_pointer()
            .hover(|d| d.bg(pal.row_hover))
            .text_size(px(13.))
            .child(crate::assets::icon("add-custom-features", 18., cx))
            .child("Custom features")
            // The version's custom features (adding them to the toolbar from a
            // version needs Onshape's Select Feature Studio dialog; listed only).
            .on_click(move |e: &ClickEvent, window, cx| {
                let names: Vec<&'static str> = custom.iter().map(|t| t.name).collect();
                let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                    if names.is_empty() {
                        return m.item(crate::os_menu::OsMenuItem::new("No custom features in this document").disabled(true));
                    }
                    for n in &names {
                        m = m.item(crate::os_menu::OsMenuItem::new(*n).disabled(true));
                    }
                    m
                });
                crate::os_menu::open(e.position(), menu, window, cx);
            });
        let custom_btn = crate::ui::with_tooltip("tt-vb-custom", "Add custom features in this version to your toolbar.", custom_btn);
        let back = div()
            .id("vb-return")
            .h(px(24.))
            .px(px(8.))
            .flex()
            .items_center()
            .rounded(px(3.))
            .bg(pal.accent)
            .cursor_pointer()
            .text_size(px(12.))
            .text_color(gpui_kit::rgb(0xffffff))
            .child(format!("Return to {branch}"))
            .on_click(move |_, _, cx| {
                doc.update(cx, |d, cx| {
                    if d.session.view_microversion(None).is_ok() {
                        d.reloaded(cx);
                    }
                })
            });
        let help = div().size(px(16.)).rounded_full().bg(gpui_kit::rgb(0x999999)).flex().items_center().justify_center().text_size(px(12.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb(0xffffff)).child("?");
        let text = format!("Versions are view only. Viewing {version}.");
        crate::ui::toolbar()
            .id("part-studio-version-bar")
            .justify_center()
            .child(
                h_flex()
                    .h(px(32.))
                    .mt(px(-4.))
                    .px(px(20.))
                    .gap(px(10.))
                    .bg(gpui_kit::rgb(0xffffff))
                    .rounded_b(px(10.))
                    .child(crate::ui::with_tooltip("tt-vb-text", text.clone(), div().id("vb-text").text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(gpui_kit::rgb(0x333333)).child(text)))
                    .child(custom_btn)
                    .child(back)
                    .child(help),
            )
            .into_any_element()
    }

    /// ⊞ "Add custom features": the document's Feature Studio features.
    fn add_custom_features_button(&mut self, custom: Vec<&'static FeatureType>, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity();
        let btn = crate::ui::tool_button("tb-add-custom", "add-custom-features", false, cx).on_click(move |e: &ClickEvent, window, cx| {
            let (v, custom) = (view.clone(), custom.clone());
            let menu = crate::os_menu::OsMenu::build(window, cx, move |mut m, _, _| {
                if custom.is_empty() {
                    return m.item(crate::os_menu::OsMenuItem::new("No custom features in this document").disabled(true));
                }
                for t in &custom {
                    let (v, id): (_, &'static str) = (v.clone(), t.id);
                    m = m.item(crate::os_menu::OsMenuItem::new(t.name).on_click(move |_, window, cx| v.update(cx, |this, cx| this.start_feature(id, window, cx))));
                }
                m
            });
            crate::os_menu::open(e.position(), menu, window, cx);
        });
        crate::ui::with_tooltip("tt-add-custom", "Add custom features", btn)
    }

    /// Toolbar group of custom features: the first few as buttons, all of
    /// them in a "Custom features" dropdown.
    fn custom_tools(&mut self, custom: Vec<&'static FeatureType>, cx: &mut Context<Self>) -> AnyElement {
        let mut group = h_flex().flex_none();
        for ty in custom.iter().take(4) {
            group = group.child(self.tool_button(ty, &[], ty.id, cx));
        }
        let view = cx.entity();
        group
            .child(
                crate::ui::tool_button("tb-custom-features", "custom-feature", false, cx)
                    .w(px(crate::ui::TOOL_CELL + 15.))
                    .child(crate::ui::glyph("tool-caret", 12., p().text))
                    .tooltip("Custom features (Feature Studios)")
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

    fn render_feature_list(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(studio) = self.studio(cx).cloned() else { return div().into_any_element() };
        let rollback = studio.rollback_index();
        // Editing a feature rolls the model to just after it: the rows
        // below show rolled back (Onshape), unless "Final" is on.
        let rollback = match &self.edit {
            Some(e) if !e.is_new && !e.dialog.read(cx).show_final => rollback.min(e.index + 1),
            _ => rollback,
        };
        let filtered = noshape_model::interaction::filter_features(&studio, &self.display, &self.ui.filter_query);
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut dg_rows: Vec<AnyElement> = Vec::new();
        // Default geometry.
        for (label, r) in [
            ("Origin", Reference::Origin),
            ("Top", Reference::Plane(DatumPlane::Top)),
            ("Front", Reference::Plane(DatumPlane::Front)),
            ("Right", Reference::Plane(DatumPlane::Right)),
        ] {
            if filtered.is_some() {
                break;
            }
            let selected = self.selection.contains(&r);
            let hidden = studio.hidden_defaults.contains(&r);
            let r2 = r.clone();
            let r3 = r.clone();
            let icon = if label == "Origin" { "origin" } else { "plane" };
            let view = cx.entity();
            let name = self.default_name(&r, cx);
            let renaming = self.ui.dg_renaming.as_ref().filter(|(x, ..)| *x == r).map(|(_, st, _)| st.clone());
            dg_rows.push(
                // Live (part-studio/16): the hover band spans the list (x 40
                // to 239); the indent is inside the row.
                crate::ui::with_side_tooltip(crate::ui::tree_row(SharedString::from(format!("dg-{label}")), selected), name.clone())
                    .mr(px(5.))
                    .pl(px(23.))
                    // Live: the eye's center 19 px from the row's end.
                    .pr(px(11.))
                    .group("dg-row")
                    // Onshape: hidden default geometry is listed in gray.
                    .when(hidden, |d| d.text_color(pal.text_faint))
                    .child(crate::ui::tree_icon(icon, hidden, cx))
                    .child(match renaming {
                        Some(st) => div().flex_1().child(gpui_kit::component::input::Input::new(&st).xsmall()).into_any_element(),
                        None => div().flex_1().truncate().child(name).into_any_element(),
                    })
                    // The eye shows / hides the item (it had no click handler).
                    .child({
                        let (v_eye, r_eye) = (view.clone(), r.clone());
                        let eye = div()
                            .id(SharedString::from(format!("dg-eye-{label}")))
                            .cursor_pointer()
                            .child(crate::assets::icon(if hidden { "eye-off" } else { "eye" }, 16., cx))
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                v_eye.update(cx, |this, cx| this.set_visibility(std::slice::from_ref(&r_eye), !hidden, cx));
                            });
                        if hidden { eye } else { crate::ui::hover_reveal(eye, "dg-row") }
                    })
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        let new = if *h { Some(r2.clone()) } else { None };
                        if this.hover != new {
                            this.hover = new;
                            this.refresh_scene(false, cx);
                        }
                    }))
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        // The section view dialog takes planes from the list too.
                        if this.ui.section_dialog.is_some() {
                            this.section_pick(&r3, window, cx);
                            return;
                        }
                        let t = this.list_target(&r3);
                        this.pick(t, cx);
                    }))
                    .os_context_menu(move |menu, _, cx| {
                        // Onshape (live): plane rows: Rename | New sketch…,
                        // Offset plane…, Size to Part Studio | Hide, Hide other
                        // planes, Hide all planes | Zoom to selection, View
                        // normal to | Show dependencies…; the origin: Rename |
                        // Hide | Show dependencies….
                        let r = r.clone();
                        let is_plane = matches!(r, Reference::Plane(_));
                        let (v1, v2, v3, v4) = (view.clone(), view.clone(), view.clone(), view.clone());
                        let (r1, r2, r3, r4) = (r.clone(), r.clone(), r.clone(), r.clone());
                        let (vn, rn) = (view.clone(), r.clone());
                        let mut m = menu.item(crate::os_menu::OsMenuItem::new("Rename").on_click(move |_, w, cx| vn.update(cx, |this, cx| this.start_default_rename(rn.clone(), w, cx)))).separator();
                        if is_plane {
                            let (vs, rs) = (view.clone(), r.clone());
                            m = m
                                .item(crate::os_menu::OsMenuItem::new("New sketch…").icon(Icon::empty().path(icon_path("sketch"))).on_click(move |_, w, cx| {
                                    v1.update(cx, |this, cx| {
                                        this.selection = vec![r1.clone()];
                                        this.start_feature("sketch", w, cx);
                                    })
                                }))
                                .item(crate::os_menu::OsMenuItem::new("Offset plane…").icon(Icon::empty().path(icon_path("plane"))).on_click(move |_, w, cx| {
                                    v4.update(cx, |this, cx| {
                                        this.selection = vec![r4.clone()];
                                        this.start_feature("cPlane", w, cx);
                                    })
                                }))
                                .item(crate::os_menu::OsMenuItem::new("Size to Part Studio").on_click(move |_, _, cx| vs.update(cx, |this, cx| this.size_plane_to_studio(rs.clone(), cx))));
                            if PartStudioView::clipboard_has_sketch(cx) {
                                let (vp, rp) = (view.clone(), r.clone());
                                m = m.item(crate::os_menu::OsMenuItem::new("Paste sketch").on_click(move |_, _, cx| vp.update(cx, |this, cx| this.paste_sketch_onto(rp.clone(), cx))));
                            }
                            m = m.separator();
                        }
                        m = m.item(crate::os_menu::OsMenuItem::new(if hidden { "Show" } else { "Hide" }).on_click(move |_, _, cx| v2.update(cx, |this, cx| this.set_visibility(std::slice::from_ref(&r2), !hidden, cx))));
                        if is_plane {
                            let (vo, ro, va) = (view.clone(), r.clone(), view.clone());
                            let (vz, rz) = (view.clone(), r.clone());
                            m = m
                                .item(crate::os_menu::OsMenuItem::new("Hide other planes").on_click(move |_, _, cx| vo.update(cx, |this, cx| this.hide_planes(Some(&ro), cx))))
                                .item(crate::os_menu::OsMenuItem::new("Hide all planes").on_click(move |_, _, cx| va.update(cx, |this, cx| this.hide_planes(None, cx))))
                                .separator()
                                .item(crate::os_menu::OsMenuItem::new("Zoom to selection").on_click(move |_, _, cx| {
                                    vz.update(cx, |this, cx| {
                                        this.selection = vec![rz.clone()];
                                        this.zoom_to_selection(cx);
                                    })
                                }))
                                .item(crate::os_menu::OsMenuItem::new("View normal to").on_click(move |_, _, cx| {
                                    v3.update(cx, |this, cx| {
                                        this.selection = vec![r3.clone()];
                                        this.view_normal(cx);
                                    })
                                }));
                        }
                        let (vd, rd) = (view.clone(), r.clone());
                        m = m.separator().item(crate::os_menu::OsMenuItem::new("Show dependencies…").on_click(move |_, _, cx| {
                            vd.update(cx, |this, cx| {
                                this.ui.dependencies = None;
                                this.ui.plane_dependencies = Some(rd.clone());
                                cx.notify();
                            })
                        }));
                        m
                    })
                    .into_any_element(),
            );
        }
        if !dg_rows.is_empty() {
            // Onshape: a collapsible "Default geometry" folder; children are
            // indented 22 px under a 1 px guide line.
            let open = !self.ui.default_geometry_collapsed;
            rows.push(
                crate::ui::tree_row("dg-folder", false)
                    .mr(px(5.))
                    .child(crate::ui::caret(open))
                    .child("Default geometry")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.ui.default_geometry_collapsed = !this.ui.default_geometry_collapsed;
                        cx.notify();
                    }))
                    .into_any_element(),
            );
            if open {
                rows.push(
                    v_flex()
                        .relative()
                        .children(dg_rows)
                        // Live: the line ends in a 9 px tick; drawn over the rows.
                        .child(div().absolute().left(px(9.)).top(px(2.)).bottom(px(1.)).w(px(1.)).bg(pal.border))
                        .child(div().absolute().left(px(5.)).bottom(px(1.)).w(px(9.)).h(px(1.)).bg(pal.border))
                        .into_any_element(),
                );
            }
        }

        let editing_id = self.edit.as_ref().map(|e| e.dialog.read(cx).feature.id);
        let editing_new_at = self.edit.as_ref().filter(|e| e.is_new).map(|e| e.index);
        let related = self.related_features(cx);
        let mut open_folder: Vec<u32> = Vec::new();
        // The Folder name popup sits beside the first selected feature, or
        // the rollback bar (live 586, 589).
        let prompt_anchor = self.folder_prompt.as_ref().map(|(_, ids, _)| ids.first().copied());
        if filtered.is_none() {
            for fd in studio.empty_folders_after(None) {
                rows.push(self.render_folder_row(fd, cx));
            }
        }
        for (i, f) in studio.features.iter().enumerate() {
            if editing_new_at == Some(i) {
                rows.push(self.render_editing_placeholder(cx));
            }
            if i == rollback && filtered.is_none() {
                rows.push(self.render_rollback_bar(i, window, cx));
            }
            if filtered.as_ref().is_some_and(|set| !set.contains(&f.id)) {
                continue;
            }
            // Nested folders: emit headers for folders newly entered, indent
            // by depth, and hide rows under a collapsed ancestor.
            let chain = studio.folder_chain(f.folder);
            let common = open_folder.iter().zip(&chain).take_while(|(a, b)| a == b).count();
            for (depth, fid) in chain.iter().enumerate().skip(common) {
                let ancestors_open = chain[..depth].iter().all(|a| studio.folder(*a).is_some_and(|fd| !fd.collapsed));
                if let (Some(fd), true) = (studio.folder(*fid), ancestors_open || filtered.is_some()) {
                    let row = self.render_folder_row(fd, cx);
                    rows.push(div().pl(px(12.0 * depth as f32)).child(row).into_any_element());
                }
            }
            open_folder = chain.clone();
            let collapsed = chain.iter().any(|c| studio.folder(*c).is_some_and(|fd| fd.collapsed));
            if collapsed && editing_id != Some(f.id) && filtered.is_none() {
                continue;
            }
            let row = self.render_feature_row(i, f, i >= rollback, editing_id == Some(f.id), related.contains(&f.id), cx);
            rows.push(if chain.is_empty() { row } else { div().pl(px(12.0 * chain.len() as f32)).child(row).into_any_element() });
            if prompt_anchor == Some(Some(f.id)) {
                rows.push(self.render_folder_prompt(cx));
            }
            if filtered.is_none() {
                for fd in studio.empty_folders_after(Some(f.id)) {
                    rows.push(self.render_folder_row(fd, cx));
                }
            }
        }
        if editing_new_at.is_some_and(|i| i >= studio.features.len()) {
            rows.push(self.render_editing_placeholder(cx));
        }
        if rollback >= studio.features.len() && filtered.is_none() {
            rows.push(self.render_rollback_bar(studio.features.len(), window, cx));
        }
        if prompt_anchor == Some(None) {
            rows.push(self.render_folder_prompt(cx));
        }
        if filtered.as_ref().is_some_and(|s| s.is_empty()) {
            rows.push(div().px_2().py_1().text_xs().text_color(pal.text_faint).child("No matching features").into_any_element());
        }

        // Parts list.
        // Parts list sections by body kind (Onshape: Parts, Surfaces, Curves).
        use noshape_model::display::BodyKind;
        // Composite parts have their own list (Help PartStudio__composite_part:
        // "the composite part in the Composite parts list"); an open one's
        // members stay in Parts.
        let sections: Vec<(&'static str, &'static str, Vec<AnyElement>, Vec<BodyId>)> = [("Parts", "parts"), ("Composite parts", "composites"), ("Surfaces", "surfaces"), ("Curves", "curves")]
            .into_iter()
            .map(|(title, key)| {
                let of_kind = |b: &noshape_model::display::BodyDisplay| match key {
                    "composites" => b.composite.is_some(),
                    _ if b.composite.is_some() => false,
                    "parts" => b.kind == BodyKind::Solid,
                    "surfaces" => b.kind == BodyKind::Surface,
                    _ => matches!(b.kind, BodyKind::Wire | BodyKind::Point),
                };
                let bodies: Vec<&noshape_model::display::BodyDisplay> = self.display.bodies.iter().filter(|b| of_kind(b)).collect();
                let rows: Vec<AnyElement> = bodies.iter().map(|b| self.render_part_row(b, cx)).collect();
                (title, key, rows, bodies.iter().map(|b| b.id.clone()).collect())
            })
            .filter(|(key, _, rows, _)| *key == "Parts" || !rows.is_empty())
            .collect();
        let failed = |id: noshape_model::FeatureId| self.display.feature_state(id).is_some_and(|s| s.status == FeatureStatus::Error);
        // The open dialog counts when it can't regenerate (a new feature
        // isn't in the list yet).
        let errors = studio.features.iter().filter(|f| failed(f.id)).count()
            + usize::from(self.edit_invalid(cx) && !editing_id.is_some_and(|id| studio.features.iter().any(|f| f.id == id && failed(f.id))));
        let dialog_open = self.edit.is_some();
        let viewing_version = self.doc.read(cx).session.viewing_version().is_some();
        let filter_input = self.ui.filter.as_ref().map(|(st, _)| st.clone());
        let config_box = self.render_config_box(cx);
        let contexts_section = self.render_contexts_section(cx);

        // Onshape (live): a 30 px filter row (funnel, then the "Filter by
        // name or type" box), then the 30 px "Features (N)" row with the
        // new-folder, pause-regeneration and regeneration-timer buttons.
        let header_btn = |id: &'static str, icon: &'static str, on: bool, cx: &App| {
            div()
                .id(id)
                .w(px(30.))
                .h(px(26.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .cursor_pointer()
                .when(on, |d| d.bg(pal.panel_active))
                .hover(|d| d.bg(pal.panel_hover))
                .child(crate::assets::icon(icon, 20., cx))
        };
        v_flex()
            .size_full()
            .bg(pal.panel)
            .border_r_1()
            .border_t_1()
            .border_b_1()
            .border_color(pal.border)
            .children(config_box)
            .children(contexts_section)
            .children(filter_input.map(|st| {
                h_flex()
                    .h(px(30.))
                    .mt(px(7.))
                    .flex_none()
                    .pl(px(5.))
                    .pr(px(8.))
                    .gap(px(14.))
                    // Live: the funnel is a button (`os-button os-clear`): pointer.
                    .child(div().flex_none().cursor_pointer().child(crate::assets::icon("filter", 20., cx)))
                    // Live (part-studio/00): a 30 px box, 1 px #aaa, radius 3; the
                    // placeholder italic gray.
                    .child({
                        let empty = st.read(cx).value().is_empty();
                        // The whole box takes clicks and shows the I-beam.
                        gpui_kit::base::input::input_frame(
                            div()
                                .flex_1()
                                .relative()
                                .h(px(30.))
                                .px(px(1.))
                                .flex()
                                .items_center()
                                .border_1()
                                .border_color(gpui_kit::rgb(0xaaaaaa))
                                .rounded(px(3.))
                                .child(gpui_kit::component::input::Input::new(&st).appearance(false).text_size(px(14.)))
                                .when(empty, |d| d.child(div().absolute().left(px(6.)).top_0().bottom_0().flex().items_center().text_size(px(14.)).italic().text_color(pal.text_faint).child("Filter by name or type"))),
                        )
                    })
            }))
            .child(
                h_flex()
                    .h(px(30.))
                    .flex_none()
                    .pl(px(4.))
                    .pr(px(15.))
                    // Live (part-studio/94, 570): with failed features a red
                    // icon comes first (click: show only them) and the title
                    // truncates to "Features …".
                    .when(errors > 0, |d| {
                        d.child(
                            div()
                                .id("feature-errors")
                                .size(px(20.))
                                .mr(px(8.))
                                .flex_none()
                                .cursor_pointer()
                                .child(crate::assets::icon("regen-error", 20., cx))
                                .tip(format!("{errors} feature{} failed to regenerate", if errors == 1 { "" } else { "s" }), crate::ui::TipSide::Right)
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                    let next = if this.ui.filter_query.trim() == ":errors" { "" } else { ":errors" };
                                    this.set_filter(next, window, cx);
                                })),
                        )
                    })
                    // (Chrome keeps the space before the ellipsis.)
                    .child(div().text_size(px(crate::ui::ROW_TEXT)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(if errors > 0 { "Features …".to_string() } else { format!("Features ({})", studio.features.len() + 4) }))
                    .child(div().flex_1())
                    // Live (navigation/301): a version keeps only the timer.
                    .when(!viewing_version, |d| d.child(
                        // Live (570): new folder and pause are disabled while a
                        // feature dialog is open.
                        header_btn("new-folder-btn", "new-folder", false, cx)
                            .when(dialog_open, |d| d.opacity(0.4).cursor_default())
                            // Live (585–590): "New folder", always enabled; the
                            // "Folder name" popup comes first.
                            .tooltip(crate::ui::tooltip("New folder"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                if this.edit.is_some() {
                                    return;
                                }
                                let ids = this.selected_features.clone();
                                this.open_folder_prompt(ids, window, cx)
                            })),
                    )
                    .child(
                        header_btn("pause-regen-btn", "pause-regen", self.ui.regen_paused.is_some(), cx)
                            .when(dialog_open, |d| d.opacity(0.4).cursor_default())
                            .tooltip(crate::ui::tooltip("Pause regeneration"))
                            .on_click(cx.listener(|this, _: &ClickEvent, w, cx| {
                                if this.edit.is_none() {
                                    this.toggle_pause_regen(w, cx)
                                }
                            })),
                    ))
                    .child(
                        header_btn("regen-times-btn", "regen-timer", self.ui.show_regen_times, cx)
                            .tooltip(crate::ui::tooltip("Show regeneration times"))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.ui.show_regen_times = !this.ui.show_regen_times;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                v_flex().id("feature-list").key_context("FeatureList").track_focus(&self.ui.list_focus).flex_1().overflow_y_scroll().children(rows).child(
                    // Space below the last feature: dropping the rollback bar
                    // here rolls it to the end (otherwise it could only be
                    // dragged up, and back down to above the last feature).
                    div()
                        .id("feature-list-end")
                        .flex_1()
                        .min_h(px(32.))
                        .drag_over::<RollbackDrag>(|s, _, _, _| s.border_t_2().border_color(p().accent))
                        .on_drop(cx.listener(|this, _: &RollbackDrag, _, cx| this.set_rollback(None, cx)))
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            // Clicking empty list space clears the feature selection.
                            if !this.selected_features.is_empty() {
                                this.selected_features.clear();
                                this.refresh_scene(false, cx);
                                cx.notify();
                            }
                        })),
                ),
            )
            // Drag the separator to share space between features and parts.
            .child(
                div()
                    .id("parts-separator")
                    .h(px(5.))
                    .flex_none()
                    // Live: `div.os-splitter` is row-resize.
                    .cursor(CursorStyle::ResizeRow)
                    .border_t_1()
                    .border_color(pal.border)
                    .on_drag(SeparatorDrag, |_, _, _, cx| cx.new(|_| SeparatorDrag)),
            )
            .child({
                // Live (part-studio/00, 220): the parts take the lower 35% of the
                // panel until the separator is dragged. A share of the panel, so
                // both sections keep room when the panel gets shorter (the
                // FeatureScript notices panel under the tab bar).
                let parts_share = self.ui.parts_height.unwrap_or(0.35);
                let mut col = v_flex().h(relative(parts_share)).min_h(px(30.)).flex_none().id("parts-area").overflow_y_scroll();
                let show_props = self.ui.show_part_props;
                for (title, key, rows, ids) in sections {
                    let collapsed = self.ui.collapsed_sections.contains(key);
                    let n = rows.len();
                    let all_hidden = !ids.is_empty() && ids.iter().all(|b| self.display.body(b).is_some_and(|d| d.hidden));
                    let view = cx.entity();
                    let ids2 = ids.clone();
                    col = col
                        .child(
                            crate::ui::tree_row(SharedString::from(format!("{key}-folder")), false)
                                .group("section-head")
                                .child(crate::ui::caret(!collapsed))
                                .child(div().flex_1().child(format!("{title} ({n})")))
                                // Eye on hover: hide or show every item of the section.
                                .child(
                                    div()
                                        .id(SharedString::from(format!("{key}-eye")))
                                        .opacity(0.)
                                        .group_hover("section-head", |d| d.opacity(1.))
                                        .cursor_pointer()
                                        .child(svg().path(icon_path(if all_hidden { "eye-off" } else { "eye" })).size_3p5().text_color(pal.text_faint))
                                        .tooltip(crate::ui::tooltip(format!("{} all {}", if all_hidden { "Show" } else { "Hide" }, title.to_lowercase())))
                                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                            cx.stop_propagation();
                                            let t: Vec<Reference> = ids.iter().cloned().map(Reference::Body).collect();
                                            this.set_visibility(&t, !all_hidden, cx);
                                        })),
                                )
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    if !this.ui.collapsed_sections.remove(key) {
                                        this.ui.collapsed_sections.insert(key);
                                    }
                                    cx.notify();
                                }))
                                .os_context_menu(move |m, _, _| {
                                    let (v1, v2) = (view.clone(), view.clone());
                                    let ids = ids2.clone();
                                    m.item(crate::os_menu::OsMenuItem::new(if show_props { "Hide parts list properties" } else { "Show parts list properties" }).on_click(move |_, _, cx| {
                                        v1.update(cx, |this, cx| {
                                            this.ui.show_part_props = !this.ui.show_part_props;
                                            // The same setting as Preferences › Display parts list properties.
                                            crate::preferences_window::set_parts_list_properties(this.ui.show_part_props);
                                            cx.notify();
                                        })
                                    }))
                                    .item(crate::os_menu::OsMenuItem::new(format!("Hide all {}", title.to_lowercase())).on_click(move |_, _, cx| {
                                        let t: Vec<Reference> = ids.iter().cloned().map(Reference::Body).collect();
                                        v2.update(cx, |this, cx| this.set_visibility(&t, true, cx))
                                    }))
                                }),
                        )
                        .when(!collapsed, |d| d.child(v_flex().pb(px(4.)).children(rows)));
                }
                col
            })
            .into_any_element()
    }

    /// A row of the parts list: click selects (or feeds a dialog's part
    /// field), hover highlights the part, the eye hides or shows it, and the
    /// context menu has Onshape's part commands.
    fn render_part_row(&self, b: &noshape_model::display::BodyDisplay, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let r = Reference::Body(b.id.clone());
        // A part picked in the open dialog's fields is selected in the list too (live part-studio/440).
        let selected = match &self.edit {
            Some(e) => e.dialog.read(cx).selected_refs().contains(&r),
            None => self.selection.contains(&r),
        };
        let hidden = b.hidden;
        let (id1, id2, id3) = (b.id.clone(), b.id.clone(), b.id.clone());
        let r_hover = r.clone();
        let view = cx.entity();
        let sheet_metal = self.context.as_ref().is_some_and(|ctx| ctx.sheet_metal.contains_key(&b.id));
        let part_name = b.name.clone();
        // Release…: the part's number and whether it has released revisions.
        let part_number = self.studio(cx).and_then(|s| s.part_properties.get(&b.id)).and_then(|p| p.part_number.clone()).filter(|p| !p.is_empty());
        let has_revisions = part_number.as_ref().is_some_and(|pn| !crate::release_dialog::release_log(self.doc.read(cx)).revisions(pn).is_empty());
        let part_icon = match b.kind {
            _ if b.composite.is_some() => "composite-part",
            // Onshape's svg-icon-part-mesh (live G27).
            _ if b.is_mesh => "part-mesh",
            noshape_model::display::BodyKind::Surface => "surface-body",
            noshape_model::display::BodyKind::Wire => "wire-body",
            _ => "part",
        };
        let renaming = self.ui.part_renaming.as_ref().is_some_and(|(x, ..)| *x == b.id);
        // Live (selection/09, part-studio/450): the band spans the list with
        // the content indented 18 px, 24 px tall, 5 px short of the edge.
        crate::ui::tree_row(SharedString::from(format!("part-{}", b.id)), selected)
            .h(px(24.))
            .pl(px(23.))
            .mr(px(5.))
            .group("part-row")
            .when(renaming, |d| d.bg(gpui_kit::rgb(0xffffff)).hover(|d| d.bg(gpui_kit::rgb(0xffffff))))
            .when(hidden, |d| d.text_color(pal.text_faint))
            .child(crate::ui::tree_icon(part_icon, hidden, cx))
            .child(match self.ui.part_renaming.as_ref().filter(|(x, ..)| *x == b.id).map(|(_, st, _)| st.clone()) {
                // Live: a white input with a 1 px rgb(22,81,176) border, 13.33 px.
                Some(st) => div()
                    .flex_1()
                    .h(px(24.))
                    .mr(px(-3.))
                    .flex()
                    .items_center()
                    .border_1()
                    .border_color(gpui_kit::rgb(0x1651b0))
                    .bg(gpui_kit::rgb(0xffffff))
                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                        if ev.keystroke.key == "escape" {
                            this.ui.part_renaming = None;
                            cx.stop_propagation();
                            cx.notify();
                        }
                    }))
                    .child(gpui_kit::component::input::Input::new(&st).appearance(false).xsmall().text_size(px(13.33)))
                    .into_any_element(),
                // Live (dom/partslist-composite-open.txt): the name's tooltip;
                // a composite adds "(open|closed) (N parts)".
                None => div()
                    .id(SharedString::from(format!("part-name-{}", b.id)))
                    .flex_1()
                    .truncate()
                    .child(b.name.clone())
                    .tooltip(crate::ui::tooltip(match &b.composite {
                        Some(c) => format!("{} ({}) ({} parts)", b.name, if c.closed { "closed" } else { "open" }, c.members.len()),
                        None => b.name.clone(),
                    }))
                    .into_any_element(),
            })
            // Parts list properties (part number, revision).
            .when_some(self.ui.show_part_props.then(|| self.studio(cx).and_then(|s| s.part_properties.get(&b.id)).map(|p| [p.part_number.clone().unwrap_or_default(), p.revision.clone().unwrap_or_default()].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(" · "))).flatten().filter(|t| !t.is_empty()), |d, t| d.child(div().text_xs().text_color(pal.text_faint).child(t)))
            .child(
                div()
                    .id(SharedString::from(format!("part-eye-{}", b.id)))
                    .mr(px(9.))
                    .when(renaming, |d| d.hidden())
                    .when(!hidden, |d| crate::ui::hover_reveal(d, "part-row"))
                    .cursor_pointer()
                    .child(crate::assets::icon(if hidden { "eye-off" } else { "eye" }, 16., cx))
                    .tooltip(crate::ui::tooltip(format!("{} {}", if hidden { "Show" } else { "Hide" }, b.name)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.set_part_hidden(&id3, !hidden, cx);
                    })),
            )
            .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                let new = if *h { Some(r_hover.clone()) } else { None };
                if this.hover != new {
                    this.hover = new;
                    this.refresh_scene(false, cx);
                }
            }))
            // Live: double-clicking the name renames the part.
            .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                if ev.click_count() >= 2 {
                    this.start_part_rename(id1.clone(), window, cx);
                } else {
                    this.pick_body(id1.clone(), cx);
                }
            }))
            .os_context_menu(move |menu, _, _| {
                let b = id2.clone();
                let item = |label: &str, f: ViewAction| {
                    let v = view.clone();
                    crate::os_menu::OsMenuItem::new(label.to_string()).on_click(move |_, w, cx| v.update(cx, |this, cx| f(this, w, cx)))
                };
                // Commands apply to every selected part when the row is selected.
                let targets = move |this: &PartStudioView, b: &BodyId| -> Vec<BodyId> {
                    let sel: Vec<BodyId> = this.selection.iter().filter_map(|r| match r {
                        Reference::Body(x) => Some(x.clone()),
                        _ => None,
                    }).collect();
                    if sel.contains(b) { sel } else { vec![b.clone()] }
                };
                // Onshape (live) parts list menu order.
                let (b1, b2, b3, b4, b5, b7, b8, b9) = (b.clone(), b.clone(), b.clone(), b.clone(), b.clone(), b.clone(), b.clone(), b.clone());
                let (b10, b11, b12) = (b.clone(), b.clone(), b.clone());
                let mut m = menu
                    .item(item("Rename", Box::new(move |this, w, cx| this.start_part_rename(b1.clone(), w, cx))))
                    .item(item("Properties…", Box::new(move |this, w, cx| this.open_part_meta(b2.clone(), w, cx))))
                    .item(item("Assign material…", Box::new(move |this, w, cx| {
                        let t = targets(this, &b3);
                        this.open_material_panel(t, w, cx);
                    })))
                    .item(item("Edit appearance…", Box::new(move |this, w, cx| {
                        let t = targets(this, &b4);
                        this.edit_appearance(appearance::EditTarget::Parts(t), w, cx);
                    })))
                    .separator()
                    .item(item("Copy here…", Box::new({
                        let b = b.clone();
                        move |this, w, cx| {
                            let t = targets(this, &b);
                            this.copy_here(t, w, cx);
                        }
                    })))
                    // Copy Part N (live: right after Copy here…).
                    .item(item(&format!("Copy {part_name}"), Box::new({
                        let b = b.clone();
                        move |this, _, cx| this.copy_part(b.clone(), cx)
                    })).icon_name("menu-copy"))
                    .separator()
                    .item(item(&format!("Create Drawing of {part_name}…"), Box::new(move |this, w, cx| {
                        let source = Some(noshape_model::drawing::create::DrawingSource { tab: this.tab, part: Some(b10.clone()), config: Default::default(), sketch: None });
                        let name = this.display.body(&b10).map(|d| d.name.clone());
                        crate::drawing_create::request_create_drawing(crate::drawing_create::CreateRequest { source, name }, w, cx);
                    })).icon_name("menu-create-drawing"))
                    .item(item("Export…", Box::new(move |this, w, cx| {
                        let t = targets(this, &b12);
                        this.open_export_dialog(t, w, cx);
                    })))
                    // Live: Release… after Export… (the selected parts).
                    .item(item("Release…", Box::new({
                        let b = b.clone();
                        move |this, w, cx| {
                            let t = targets(this, &b);
                            let names: Vec<String> = t.iter().map(|id| this.display.body(id).map(|d| d.name.clone()).unwrap_or_default()).collect();
                            let items = crate::release_dialog::part_items(this.doc.read(cx), this.tab, &t, &names, &this.config_values);
                            crate::release_dialog::open_release_candidate(this.doc.clone(), items, w, cx);
                        }
                    })));
                if has_revisions && let Some(pn) = part_number.clone() {
                    m = m.item(item("Revision history", Box::new(move |this, w, cx| crate::release_dialog::open_revision_history(this.doc.clone(), pn.clone(), w, cx))));
                }
                if sheet_metal {
                    let b13 = b.clone();
                    m = m.item(item("Export flat pattern (DXF)…", Box::new(move |this, w, cx| this.export_flat(b13.clone(), "dxf", w, cx))));
                }
                m = m
                    .separator()
                    .item(item(if hidden { "Show" } else { "Hide" }, Box::new(move |this, _, cx| {
                        let t: Vec<Reference> = targets(this, &b5).into_iter().map(Reference::Body).collect();
                        this.set_visibility(&t, !hidden, cx);
                    })).icon_name(if hidden { "eye" } else { "eye-off" }))
                    .item(item("Isolate…", Box::new(move |this, w, cx| {
                        this.selection = targets(this, &b7).into_iter().map(Reference::Body).collect();
                        this.hover = None;
                        this.isolate(behaviors::IsolateMode::Isolate, w, cx);
                    })))
                    .item(item("Make transparent…", Box::new(move |this, w, cx| {
                        this.selection = targets(this, &b8).into_iter().map(Reference::Body).collect();
                        this.hover = None;
                        this.isolate(behaviors::IsolateMode::Transparent, w, cx);
                    })))
                    .separator()
                    .item(item("Zoom to selection", Box::new(move |this, _, cx| {
                        let t = targets(this, &b9);
                        this.selection = t.into_iter().map(Reference::Body).collect();
                        this.zoom_to_selection(cx);
                    })));
                m.separator().item(item("Delete…", Box::new(move |this, w, cx| {
                    this.selection = targets(this, &b11).into_iter().map(Reference::Body).collect();
                    this.delete_selected_parts(w, cx);
                })).icon_name("menu-delete"))
            })
            .into_any_element()
    }

    /// The header's New folder: the "Folder name" popup, "New folder"
    /// selected (live 586).
    fn open_folder_prompt(&mut self, ids: Vec<FeatureId>, window: &mut Window, cx: &mut Context<Self>) {
        let st = cx.new(|cx| gpui_kit::component::input::InputState::new(window, cx).default_value("New folder"));
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        let sub = cx.subscribe_in(&st, window, |this: &mut Self, _, ev: &gpui_kit::component::input::InputEvent, window, cx| {
            if matches!(ev, gpui_kit::component::input::InputEvent::PressEnter { .. }) {
                this.commit_folder_prompt(window, cx);
            }
        });
        self.folder_prompt = Some((st, ids, sub));
        cx.notify();
    }

    /// ✓ / Enter: an empty folder at the rollback bar, or the selection wrapped.
    fn commit_folder_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((st, ids, _)) = self.folder_prompt.take() else { return };
        let name = st.read(cx).value().trim().to_string();
        let name = if name.is_empty() { "New folder".to_string() } else { name };
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, "Add folder", cx, |ps| {
                ps.new_folder(&ids, Some(name));
            });
        });
        let fh = self.focus.clone();
        window.focus(&fh, cx);
        cx.notify();
    }

    /// The 220 px "Folder name" ✓ ✕ popup right of the list, level with the
    /// row above it (live 586, 589).
    fn render_folder_prompt(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some((st, _, _)) = &self.folder_prompt else { return div().into_any_element() };
        let popup = v_flex()
            .id("folder-prompt")
            .occlude()
            .w(px(220.))
            .bg(gpui_kit::rgb(0xffffff))
            .shadow_md()
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                if e.keystroke.key == "escape" {
                    cx.stop_propagation();
                    this.folder_prompt = None;
                    let fh = this.focus.clone();
                    window.focus(&fh, cx);
                    cx.notify();
                }
            }))
            .child(
                h_flex()
                    .h(px(29.))
                    .border_b_1()
                    .border_color(gpui_kit::rgb(0xd7d7d7))
                    .child(div().flex_1().pl(px(6.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child("Folder name"))
                    .child(div().id("folder-ok").size(px(28.)).flex().items_center().justify_center().bg(gpui_kit::rgb(0x009400)).cursor_pointer().child(crate::ui::glyph("check", 16., gpui_kit::rgb(0xffffff))).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.commit_folder_prompt(window, cx))))
                    .child(div().id("folder-cancel").size(px(28.)).flex().items_center().justify_center().cursor_pointer().child(crate::ui::glyph("close", 14., gpui_kit::rgb(0xd52b1e))).on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.folder_prompt = None;
                        let fh = this.focus.clone();
                        window.focus(&fh, cx);
                        cx.notify();
                    }))),
            )
            .child(div().m(px(6.)).border_b_1().border_color(pal.accent).child(gpui_kit::component::input::Input::new(st).small().appearance(false).text_size(px(13.33))));
        // A zero-height box under the anchor row; the popup rises beside it.
        div().relative().w_full().h_0().child(div().absolute().left_full().top(px(-26.)).size_0().child(deferred(anchored().snap_to_window().child(popup)).with_priority(5))).into_any_element()
    }

    fn render_folder_row(&self, fd: &noshape_model::partstudio::Folder, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let id = fd.id;
        let collapsed = fd.collapsed;
        let _ = pal;
        let row = crate::ui::tree_row(SharedString::from(format!("folder-{id}")), false)
            .mr(px(5.))
            .child(crate::ui::caret(!collapsed))
            .child(crate::ui::tree_icon("folder", false, cx))
            .child(match self.folder_renaming.as_ref().filter(|(fid, _, _)| *fid == id) {
                Some((_, st, _)) => div().flex_1().child(gpui_kit::component::input::Input::new(st).xsmall()).into_any_element(),
                // Live 587, 590: "New folder (0)" — the name and its feature count.
                None => div().flex_1().truncate().child(format!("{} ({})", fd.name, self.studio(cx).map(|s| s.folder_count(id)).unwrap_or(0))).into_any_element(),
            })
            .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                if ev.click_count() >= 2 {
                    this.start_folder_rename(id, window, cx);
                } else {
                    this.toggle_folder(id, cx);
                }
            }));
        let view = cx.entity();
        row.os_context_menu(move |menu, _, _| {
            let v = view.clone();
            let v2 = view.clone();
            let v3 = view.clone();
            let v4 = view.clone();
            let v5 = view.clone();
            let v6 = view.clone();
            menu.item(crate::os_menu::OsMenuItem::new(if collapsed { "Expand" } else { "Collapse" }).on_click(move |_, _, cx| v.update(cx, |this, cx| this.toggle_folder(id, cx))))
                .item(crate::os_menu::OsMenuItem::new("Rename").on_click(move |_, window, cx| v2.update(cx, |this, cx| this.start_folder_rename(id, window, cx))))
                .separator()
                // Onshape: suppressing a folder suppresses everything in it.
                .item(crate::os_menu::OsMenuItem::new("Suppress").on_click(move |_, _, cx| v4.update(cx, |this, cx| this.suppress_folder(id, true, cx))))
                .item(crate::os_menu::OsMenuItem::new("Unsuppress").on_click(move |_, _, cx| v5.update(cx, |this, cx| this.suppress_folder(id, false, cx))))
                .separator()
                .item(crate::os_menu::OsMenuItem::new("Expand all").on_click({
                    let v = v6.clone();
                    move |_, _, cx| v.update(cx, |this, cx| this.set_all_folders_collapsed(false, cx))
                }))
                .item(crate::os_menu::OsMenuItem::new("Collapse all").on_click(move |_, _, cx| v6.update(cx, |this, cx| this.set_all_folders_collapsed(true, cx))))
                .separator()
                .item(crate::os_menu::OsMenuItem::new("Unpack folder").on_click(move |_, _, cx| v3.update(cx, |this, cx| this.dissolve_folder(id, cx))))
        })
        .into_any_element()
    }

    /// The top of Onshape's cursor stack over the graphics area
    /// (docs/parity/cursors.md): `move` over a dimension value, the sketch
    /// tools' crosshair (the pattern tools keep the arrow), else none (the
    /// selection cursor shows).
    /// The number on the selection-count cursor (also shown over the navbar).
    pub fn selection_cursor_count(&self, cx: &App) -> usize {
        self.viewport.read(cx).selection_count
    }

    /// Sketch hover changes don't rebuild the scene; hand the viewport its
    /// tool cursor after every viewport event (it applies it on render).
    fn push_tool_cursor(&self, cx: &mut Context<Self>) {
        let (tool_cursor, overlay_cursor) = (self.viewport_tool_cursor(), self.viewport_overlay_cursor());
        self.viewport.update(cx, |vp, cx| {
            if (vp.tool_cursor, vp.overlay_cursor) != (tool_cursor, overlay_cursor) {
                (vp.tool_cursor, vp.overlay_cursor) = (tool_cursor, overlay_cursor);
                cx.notify();
            }
        });
    }

    /// Cursors of overlays over the graphics area: a sketch dimension value
    /// is draggable (CSS `move`); an analysis legend's color map shows the
    /// crosshair.
    fn viewport_overlay_cursor(&self) -> Option<crate::cursor::AppCursor> {
        use crate::cursor::AppCursor;
        if self.sketch.as_ref().is_some_and(|ed| ed.hover_constraint.is_some()) {
            return Some(AppCursor::Move);
        }
        let map = |b: Option<&thickness::BarUi>| b.is_some_and(|b| b.map_hover);
        (map(self.ui.thickness.as_ref().map(|t| &t.bar)) || map(self.ui.curvature_map.as_ref().map(|c| &c.bar))).then_some(AppCursor::Crosshair)
    }

    fn viewport_tool_cursor(&self) -> Option<crate::cursor::AppCursor> {
        use crate::cursor::AppCursor;
        use crate::sketch_mode::Tool;
        let ed = self.sketch.as_ref()?;
        if ed.box_dragging() && ed.box_remove {
            return Some(AppCursor::RemoveSelection);
        }
        // Pattern, transform and offset: once the operation can be created and
        // nothing is pre-highlighted, the "click to accept" cursor (Onshape's
        // showConfirmationCursor; not while a dimension box is open).
        let complete = match ed.tool {
            Tool::LinearPattern | Tool::CircularPattern | Tool::Transform => !ed.selection.is_empty(),
            Tool::Offset => ed.offset_pending.is_some(),
            _ => false,
        };
        if complete && ed.hover.is_none() && ed.hover_region.is_none() && ed.dim_edit.is_none() {
            return Some(AppCursor::Confirmation);
        }
        match ed.tool {
            Tool::Select | Tool::LinearPattern | Tool::CircularPattern | Tool::Transform => None,
            _ => Some(AppCursor::Crosshair),
        }
    }

    /// The open feature dialog can't regenerate (live: its row turns red and
    /// the Features header shows the error icon until it can).
    fn edit_invalid(&self, cx: &App) -> bool {
        self.edit.as_ref().is_some_and(|e| matches!(e.dialog.read(cx).status.as_ref().map(|s| s.0), Some(FeatureStatus::Error)))
    }

    fn render_editing_placeholder(&self, cx: &App) -> AnyElement {
        let pal = p();
        // Live: a new Hole's row reads "Ø 6.35 mm ↧ 12 mm" while it is drawn.
        let name = self
            .edit
            .as_ref()
            .map(|e| {
                let d = e.dialog.read(cx);
                let units = self.doc.read(cx).session.doc.settings.units;
                let computed = self.computed_values(d.feature.id, cx);
                noshape_model::interaction::display_name_computed(&d.feature, &|k| {
                    computed.iter().find(|(c, _)| c == k).map(|(_, v)| v.clone()).or_else(|| crate::feature_dialog::shown_length(&d.feature, k, &units, &self.display.variables))
                })
            })
            .unwrap_or_default();
        let icon = self.edit.as_ref().map(|e| feature_icon(&e.dialog.read(cx).feature.type_id)).unwrap_or_else(|| icon_path("part"));
        crate::ui::tree_row("editing-placeholder", true)
            .mr(px(5.))
            .bg(pal.accent_soft)
            .font_weight(FontWeight::SEMIBOLD)
            .when(self.edit_invalid(cx), |d| d.text_color(pal.text_invalid))
            .child(crate::ui::tree_icon(&icon, false, cx))
            .child(div().flex_1().truncate().child(name))
            .into_any_element()
    }

    fn render_rollback_bar(&self, index: usize, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        // Light blue once ↑/↓ (or a click) gave it the keyboard, until the
        // selection changes; selecting a row alone leaves it gray (live 31, 32).
        let focused = self.ui.list_focus.is_focused(window) && self.ui.rollback_keyboard.as_ref() == Some(&self.selected_features);
        div()
            .id(SharedString::from(format!("rollback-{index}")))
            .h(px(crate::ui::ROW_H))
            // Live (menus/rmb-ps-rollback-bar): x 45-234, `#c8d9e2` on hover,
            // tooltip "Rollback bar".
            .pl(px(5.))
            .pr(px(10.))
            .flex()
            .flex_none()
            .items_center()
            // Live (dom/cursors.txt): the default arrow, also while dragging.
            .group("rollback")
            .tip("Rollback bar", crate::ui::TipSide::Below)
            .child(div().w_full().h(px(7.)).bg(if focused { gpui_kit::rgb(0xa0c7dd) } else { pal.rollback }).group_hover("rollback", |d| d.bg(gpui_kit::rgb(0xc8d9e2))))
            .on_drag(RollbackDrag, |_, grab, _, cx| cx.new(|_| RollbackDragPill(grab)))
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                // Selecting the bar lets ↑/↓ step it (Onshape).
                window.focus(&this.ui.list_focus, cx);
                this.ui.rollback_keyboard = Some(this.selected_features.clone());
                cx.notify();
            }))
            .into_any_element()
    }

    fn render_feature_row(&self, i: usize, f: &Feature, rolled_back: bool, editing: bool, related: bool, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        // The feature being edited shows its draft's name (live: a Hole's
        // row reads "Ø 6.35 mm ↧ 12 mm" while its dialog is open).
        let draft = self.edit.as_ref().map(|e| e.dialog.read(cx).feature.clone()).filter(|d| d.id == f.id);
        let f = draft.as_ref().unwrap_or(f);
        let name = {
            let computed = self.computed_values(f.id, cx);
            let units = self.doc.read(cx).session.doc.settings.units;
            let vars = &self.display.variables;
            noshape_model::interaction::display_name_computed(f, &|k| {
                computed.iter().find(|(c, _)| c == k).map(|(_, v)| v.clone()).or_else(|| crate::feature_dialog::shown_length(f, k, &units, vars))
            })
        };
        let state = self.display.feature_state(f.id);
        let status = if editing && self.edit_invalid(cx) { FeatureStatus::Error } else { state.map(|s| s.status).unwrap_or(FeatureStatus::Ok) };
        let selected = self.selected_features.contains(&f.id);
        // Onshape (§3.4): rolled back = italic #ccc, suppressed = #999 struck
        // through, error = #aa252d; everything else (sketches included) #333.
        // Suppressed directly, or by a configuration / expression (the
        // regenerated state says so).
        let suppressed = f.suppressed || (!rolled_back && status == FeatureStatus::Suppressed);
        // Hidden features (a sketch consumed by a feature) read gray too (live part-studio/92).
        let color = if rolled_back {
            pal.text_rolled_back
        } else if suppressed || f.hidden {
            pal.text_faint
        } else {
            match status {
                FeatureStatus::Error => pal.text_invalid,
                _ => pal.text,
            }
        };
        let underdefined = !rolled_back && state.is_some_and(|s| s.underdefined);
        let id = f.id;
        let msg = state.and_then(|s| s.message.clone());
        // Onshape (live part-studio/16, 90, 95): hovering a row shows its
        // error, else what it depends on ("Extrude 1 <- Sketch 1"), else its
        // name, in a box beside the row.
        let msg = msg.unwrap_or_else(|| {
            let parents: Vec<String> = self.studio(cx).map(|studio| noshape_model::interaction::feature_dependencies(&studio.features, i).into_iter().filter_map(|p| studio.feature(p).map(|g| g.name.clone())).collect()).unwrap_or_default();
            if parents.is_empty() { name.clone() } else { format!("{} <- {}", f.name, parents.join(", ")) }
        });
        let manually_suppressed = f.suppressed;
        let hidden = f.hidden;
        // Sketches, planes and mate connectors can be hidden from the list.
        let hideable = matches!(f.type_id.as_str(), "sketch" | "cPlane" | "mateConnector") || self.display.sketches.iter().any(|s| s.feature == id);
        let first_config_input = self.studio(cx).and_then(|s| s.configuration.inputs.first().map(|i| i.id.clone()));
        let dyn_expr = f.suppression_expr.clone();
        let makes_faces = self.display.bodies.iter().any(|b| b.face_refs.iter().any(|r| r.depends_on() == Some(id)));
        let has_appearance = self.studio(cx).is_some_and(|s| s.appearances.iter().any(|a| a.target == noshape_model::partstudio::AppearanceTarget::Feature(id)));
        let is_sketch = self.display.sketches.iter().any(|s| s.feature == id);
        let has_plane = is_sketch || self.display.planes.iter().any(|p| p.reference == Reference::FeaturePlane(id));
        let row = crate::ui::tree_row(SharedString::from(format!("feat-{}", f.id)), selected || editing)
            .mr(px(5.))
            .pr(px(11.))
            .when(self.ui.list_menu == Some(id) && !(selected || editing), |d| d.bg(pal.row_hover))
            .group("feature-row")
            .relative()
            // Rollback drop line (live part-studio/30): a small hollow blue
            // circle at its left end, shown while the bar is dragged over.
            .child(
                div()
                    .absolute()
                    .left(px(0.))
                    .top(px(-2.5))
                    .size(px(7.))
                    .rounded_full()
                    .border_2()
                    .border_color(pal.accent)
                    .bg(gpui_kit::rgb(0xffffff))
                    .invisible()
                    .group_drag_over::<RollbackDrag>("feature-row", |s| s.visible()),
            )
            .text_color(color)
            // Onshape (part-studio/31, 92): rolled back and suppressed rows are italic.
            .when(rolled_back || suppressed, |d| d.italic())
            .when(related && !selected && !editing, |d| d.bg(gpui_kit::rgb(0xecf0f4)))
            .when(editing, |d| d.font_weight(FontWeight::SEMIBOLD))
            .child(
                crate::ui::tree_icon(&feature_icon(&f.type_id), rolled_back || suppressed || hidden, cx)
                    .relative()
                    // Onshape list badges: under-defined sketch (blue) at the icon's
                    // bottom-right. An error only turns the name red (part-studio/94).
                    // Missing selections / warnings: Onshape's yellow triangle.
                    .when(status == FeatureStatus::Warning && !rolled_back, |d| d.child(div().absolute().right(px(-3.)).bottom(px(-5.)).text_size(px(10.)).text_color(pal.warning).child("▲")))
                    .when(underdefined, |d| d.child(div().absolute().right(px(-1.)).bottom(px(-3.)).w(px(8.)).h(px(4.)).bg(pal.accent))),
            )
            .child(match self.renaming.as_ref().filter(|(rid, _, _)| *rid == f.id) {
                Some((_, st, _)) => div().flex_1().child(gpui_kit::component::input::Input::new(st).xsmall()).into_any_element(),
                None => div().flex_1().truncate().when(suppressed, |d| d.line_through()).child(name.clone()).into_any_element(),
            })
            // In-context arrow (features referencing an Assembly context).
            .children(self.context_arrow(f, cx))
            // Onshape's expression icon on dynamically suppressed features.
            // Live (assembly/141): "<name> (Suppressed with expression)".
            .when_some(dyn_expr.clone().map(|_| if suppressed { format!("{name} (Suppressed with expression)") } else { name.clone() }), |d, e| {
                d.child(
                    div()
                        .id(SharedString::from(format!("feat-dyn-{id}")))
                        .text_xs()
                        .italic()
                        .text_color(pal.accent)
                        .cursor_pointer()
                        .child("ƒx")
                        .tooltip(crate::ui::tooltip(e))
                        .on_click(cx.listener(move |this, _: &ClickEvent, w, cx| {
                            cx.stop_propagation();
                            this.edit_dynamic_suppression(id, w, cx);
                        })),
                )
            })
            .when(hideable, |d| {
                d.child(
                    div()
                        .id(SharedString::from(format!("feat-eye-{}", f.id)))
                        .when(!hidden, |d| crate::ui::hover_reveal(d, "feature-row"))
                        .cursor_pointer()
                        .child(crate::assets::icon(if hidden { "eye-off" } else { "eye" }, 16., cx))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.set_visibility(&[Reference::Feature(id)], !hidden, cx);
                        })),
                )
            })

            .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                // Hovering a feature highlights its geometry (Onshape).
                let new = if *h { Some(id) } else { None };
                if this.ui.list_hover != new && (*h || this.ui.list_hover == Some(id)) {
                    this.ui.list_hover = new;
                    this.refresh_scene(false, cx);
                }
            }))
            .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                // Configure features: the clicked feature opens with its
                // configurable parameters outlined.
                if this.ui.configure_pick.is_some() {
                    this.edit_feature(id, window, cx);
                    if let Some(e) = &this.edit {
                        e.dialog.update(cx, |d, cx| {
                            d.configure_mode = true;
                            cx.notify();
                        });
                    }
                    return;
                }
                if ev.click_count() >= 2 {
                    this.edit_feature(id, window, cx);
                    return;
                }
                if this.pick_feature_into_dialog(id, cx) {
                    return;
                }
                // A dialog field that can't take it: the row isn't selected.
                if let Some(f) = this.active_field_filter(cx) {
                    this.flash_info(behaviors::filter_description(f), cx);
                    return;
                }
                this.click_feature(id, ev.modifiers().shift, cx);
                // Onshape: after clicking a row, ↑/↓ move the rollback bar.
                window.focus(&this.ui.list_focus, cx);
            }))
            .drag_over::<RollbackDrag>(|s, _, _, _| s.border_t_2().border_color(p().accent))
            .on_drop(cx.listener(move |this, _: &RollbackDrag, _, cx| this.set_rollback(Some(i), cx)))
            .on_drag(FeatureDrag { id, name: f.name.clone().into() }, |d, _, _, cx| cx.new(|_| d.clone()))
            .drag_over::<FeatureDrag>(|s, _, _, _| s.border_t_2().border_color(p().accent))
            .on_drop(cx.listener(move |this, d: &FeatureDrag, _, cx| this.drop_features(d.id, i, cx)));
        let row = crate::ui::with_side_tooltip(row, msg);
        let view = cx.entity();
        let is_last = self.studio(cx).is_some_and(|s| i + 1 >= s.features.len());
        let source_sketch = self.studio(cx).and_then(|s| noshape_model::interaction::source_sketch(&s.features, f).and_then(|sid| s.feature(sid).map(|g| (sid, g.name.clone()))));
        let feature_name = f.name.clone();
        row.os_context_menu(move |menu, window, cx| {
            view.update(cx, |this, cx| {
                if this.ui.list_menu != Some(id) {
                    this.ui.list_menu = Some(id);
                    this.refresh_scene(false, cx);
                    cx.notify();
                }
            });
            // Onshape's feature context menu (live capture order); commands act
            // on every selected feature when the clicked one is selected.
            let item = |label: &str, f: ViewAction| {
                let v = view.clone();
                crate::os_menu::OsMenuItem::new(label.to_string()).on_click(move |_, w, cx| v.update(cx, |this, cx| f(this, w, cx)))
            };
            let targets = move |this: &PartStudioView| if this.selected_features.contains(&id) { this.selected_features.clone() } else { vec![id] };
            let multi = {
                let this = view.read(cx);
                this.selected_features.len() > 1 && this.selected_features.contains(&id)
            };
            // Live (assembly/142): once an expression is set the row offers
            // "Unsuppress" (drops the expression) and "Edit dynamic
            // suppression" in place of Suppress and the submenu.
            let has_expr = dyn_expr.is_some();
            let suppress = if has_expr {
                item("Unsuppress", Box::new(move |this, _, cx| this.clear_dynamic_suppression(id, cx)))
            } else {
                item(if manually_suppressed { "Unsuppress" } else { "Suppress" }, Box::new(move |this, _, cx| {
                    let t = targets(this);
                    this.set_suppressed(t, !manually_suppressed, cx);
                }))
            };
            // Dynamic suppression ▸ Add expression… ("Unsuppressed if").
            let vd = view.clone();
            let dynamic = move |m: crate::os_menu::OsMenu, window: &mut Window, cx: &mut Context<crate::os_menu::OsMenu>| {
                if has_expr {
                    let v1 = vd.clone();
                    return m.item(crate::os_menu::OsMenuItem::new("Edit dynamic suppression").on_click(move |_, w, cx| v1.update(cx, |this, cx| this.edit_dynamic_suppression(id, w, cx))));
                }
                m.submenu("Dynamic suppression", window, cx, move |sm, _, _| {
                    let v1 = vd.clone();
                    sm.item(crate::os_menu::OsMenuItem::new("Add expression…").on_click(move |_, w, cx| v1.update(cx, |this, cx| this.edit_dynamic_suppression(id, w, cx))))
                })
            };
            let appearance = |m: crate::os_menu::OsMenu| {
                if is_sketch {
                    m.item(item("Edit sketch appearance…", Box::new(move |this, w, cx| this.edit_appearance(appearance::EditTarget::Feature(id, None), w, cx))).icon_name("add-appearance"))
                } else if makes_faces {
                    // Live (menus/rmb-ps-featurelist-fillet): with its icon, as Delete.
                    m.item(item(if has_appearance { "Edit feature appearance…" } else { "Add appearance to feature…" }, Box::new(move |this, w, cx| this.edit_appearance(appearance::EditTarget::Feature(id, None), w, cx))).icon_name("add-appearance"))
                } else {
                    m
                }
            };
            let delete = item("Delete", Box::new(move |this, _, cx| {
                let t = targets(this);
                this.delete_features(t, cx);
            }))
            .icon_name("menu-delete");
            let folder = item("Add selection to folder…", Box::new(move |this, window, cx| {
                let t = targets(this);
                this.open_folder_prompt(t, window, cx);
            }));
            if multi {
                // Section view… through the selected features' planes (live order).
                let section = item("Section view…", Box::new(move |this, w, cx| {
                    let refs: Vec<Reference> = targets(this)
                        .into_iter()
                        .map(|f| {
                            if this.display.sketches.iter().any(|s| s.feature == f) {
                                Reference::Sketch(f)
                            } else if this.display.planes.iter().any(|p| p.reference == Reference::FeaturePlane(f)) {
                                Reference::FeaturePlane(f)
                            } else {
                                Reference::Feature(f)
                            }
                        })
                        .collect();
                    this.section_through_all(&refs, w, cx);
                }));
                let m = menu.item(folder).separator().item(section).item(suppress);
                let m = dynamic(m, window, cx).separator().item(item("Clear selection", Box::new(|this, _, cx| {
                    this.selection.clear();
                    this.selected_features.clear();
                    this.refresh_scene(false, cx);
                    cx.notify();
                })));
                return appearance(m.separator()).separator().item(delete);
            }
            let mut m = menu.item(item("Rename", Box::new(move |this, w, cx| this.start_rename(id, w, cx)))).separator().item(item("Edit…", Box::new(move |this, w, cx| this.edit_feature(id, w, cx))));
            if let Some((sid, sname)) = source_sketch.clone() {
                m = m.item(item(&format!("Edit {sname}…"), Box::new(move |this, w, cx| this.edit_feature(sid, w, cx))));
            }
            if is_sketch {
                m = m.item(item("Copy sketch", Box::new(move |this, _, cx| this.copy_sketch(id, cx))));
            }
            if is_sketch || source_sketch.is_some() {
                m = m.item(item("Show dimensions", Box::new(move |this, w, cx| {
                    if let Some(sk) = this.dimensions_sketch(id, cx) {
                        this.show_dimensions(sk, w, cx);
                    }
                })));
            }
            m = m.separator().item(folder);
            if hideable {
                m = m.separator().item(item(if hidden { "Show" } else { "Hide" }, Box::new(move |this, _, cx| {
                    let t: Vec<Reference> = targets(this).into_iter().map(Reference::Feature).collect();
                    this.set_visibility(&t, !hidden, cx);
                })));
            }
            if is_sketch {
                // Live order: Create Drawing of Sketch N…, Export as DXF/DWG… (a sketch view; asb).
                let sk_name = feature_name.clone();
                m = m.separator().item(item(&format!("Create Drawing of {sk_name}…"), Box::new(move |this, w, cx| crate::drawing_create::request_create_drawing_of_sketch(this.tab, id, &sk_name, w, cx))));
                m = m.item(item("Export as DXF/DWG…", Box::new(move |this, w, cx| this.export_sketch(id, "dxf", w, cx))));
            }
            m = m.separator().item(suppress);
            if let Some(input) = first_config_input.clone() {
                // Configure suppression adds the feature to the configuration table.
                m = m.item(item("Configure suppression", Box::new(move |this, _, cx| {
                    this.add_configured(id, noshape_model::configuration::SUPPRESSED_PARAM, &input, cx);
                    this.right_panel = Some(config_panel::RightPanel::Configurations);
                    cx.notify();
                })));
            }
            m = dynamic(m, window, cx).separator().item(item("Zoom to selection", Box::new(move |this, _, cx| {
                if !this.selected_features.contains(&id) {
                    this.selected_features = vec![id];
                }
                this.zoom_to_selection(cx);
            })));
            if has_plane {
                m = m.item(item("View normal to", Box::new(move |this, _, cx| {
                    let r = if is_sketch {
                        this.display.sketches.iter().find(|s| s.feature == id).and_then(|s| s.curves.first().map(|c| Reference::SketchEntity { sketch: id, entity: c.entity })).unwrap_or(Reference::FeaturePlane(id))
                    } else {
                        Reference::FeaturePlane(id)
                    };
                    let prev = std::mem::replace(&mut this.selection, vec![r]);
                    this.view_normal(cx);
                    this.selection = prev;
                })));
            }
            m = m.separator().item(item("Show dependencies…", Box::new(move |this, _, cx| {
                this.ui.dependencies = Some(id);
                cx.notify();
            })));
            if !is_last {
                m = m.separator().item(item("Roll to here", Box::new(move |this, _, cx| this.set_rollback(Some(i + 1), cx))));
            }
            if is_sketch || makes_faces {
                m = appearance(m.separator());
            }
            m.separator().item(delete)
        })
        .into_any_element()
    }

    fn render_viewport_overlay(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let pal = p();
        let mut out = Vec::new();
        // Plane labels: Onshape draws the name *in the plane* (blue, at the
        // top-left corner, inside), so it skews with the view. The glyph
        // outlines are laid out in the plane and painted as projected paths.
        if self.opts.show_planes {
            let vp = self.viewport.read(cx);
            let mut polys: Vec<Vec<Point<Pixels>>> = Vec::new();
            for pl in self.display.planes.iter().filter(|p| !p.hidden) {
                let f = &pl.frame;
                // Ascender height relative to the plane (measured live: about 20 px on a 700 px plane).
                let asc = 0.031 * 2.0 * pl.half_height;
                let top_left = f.origin + f.x * (-pl.half_width) + f.y * pl.half_height;
                in_plane_text(vp, &pl.name, top_left, f.x, f.y, asc, &mut polys);
            }
            if !polys.is_empty() {
                out.push(paint_paths(polys, pal.accent.into()));
            }
        }
        // Graphics preferences (Camera and render options), in the dialog place.
        if let Some((d, _)) = &self.ui.graphics_dialog {
            out.push(div().absolute().top(px(3.)).left(px(0.)).child(d.clone()).into_any_element());
        }
        // Feature dialog.
        if let Some(e) = &self.edit {
            // Moved dialogs keep their place (Onshape lets you drag them).
            let d = div().absolute().child(e.dialog.clone());
            // Onshape: at the viewport's left edge, 3 px under the toolbar.
            let d = match self.ui.dialog_pos {
                Some(p) => d.left(p.x).top(p.y),
                None => d.top(px(3.)).left(px(0.)),
            };
            out.push(d.into_any_element());
        }
        // View cube / quick views.
        out.push(self.render_view_controls(cx));
        if let Some(nv) = self.render_named_views(cx) {
            out.push(nv);
        }
        // Message toast.
        // The tool-miss toast belongs to the open Remove / Intersect dialog.
        if let Some((st, msg)) = self.message.clone().filter(|(_, m)| self.edit.is_some() || m != behaviors::NO_INTERSECTION) {
            // Onshape alert: top-center of the viewport, info/warn/error colors (§4.9).
            let kind = match st {
                FeatureStatus::Error => crate::os_ui::BubbleKind::Error,
                FeatureStatus::Warning => crate::os_ui::BubbleKind::Warning,
                // `flash_success`: Onshape's green ✓ toast.
                FeatureStatus::Ok => crate::os_ui::BubbleKind::Success,
                _ => crate::os_ui::BubbleKind::Info,
            };
            // Onshape's message bubble (os_ui; live dom/toast-*.txt), with the
            // "Restore" link after a canceled edit.
            let view = cx.entity();
            let close = {
                let view = view.clone();
                move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                    view.update(cx, |this, cx| {
                        this.message = None;
                        this.canceled_edit = None;
                        cx.notify();
                    })
                }
            };
            let bubble = if self.canceled_edit.as_ref().is_some_and(|c| c.3 == msg) {
                crate::os_ui::message_bubble_with_link("toast", kind, msg, "Restore", move |_, window, cx| view.update(cx, |this, cx| this.restore_canceled_edit(window, cx)), close, cx)
            } else {
                crate::os_ui::message_bubble("toast", kind, msg, close, cx)
            };
            // Live document/01: centered on the window, not the graphics area.
            let left = crate::ui::RAIL_W + if self.ui.list_collapsed { 0. } else { crate::ui::LEFT_PANEL_W };
            out.push(div().absolute().top(px(8.)).left_0().right_0().pr(px(left)).flex().justify_center().child(bubble).into_any_element());
        }
        out
    }

    /// Onshape's split toggle at the viewport's left edge, vertically
    /// centered: a 33x34 white button with a 1 px `#aaa` border that
    /// collapses or restores the Features and parts list.
    fn render_list_toggle(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let collapsed = self.ui.list_collapsed;
        div()
            .id("list-toggle")
            .occlude()
            .w(px(33.))
            .h(px(34.))
            .ml(px(-1.))
            .flex()
            .items_center()
            .justify_center()
            .bg(pal.panel)
            .border_1()
            .border_color(pal.border_strong)
            .cursor_pointer()
            .hover(|d| d.bg(pal.panel_hover))
            // Live (part-studio/00): the glyph sits 2.5 px right of center.
            .child(div().ml(px(5.)).child(crate::assets::icon("treelist", 20., cx)))
            .tooltip(crate::ui::tooltip(if collapsed { "Show the Features and parts list" } else { "Collapse the Features and parts list" }))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.ui.list_collapsed = !this.ui.list_collapsed;
                cx.notify();
            }))
            .into_any_element()
    }

    fn render_view_controls(&mut self, cx: &mut Context<Self>) -> AnyElement {
        // Onshape: the view cube sits 15 px from the viewport's top-right
        // corner; view and render options live in its dropdown menu (§3.7).
        let cube = self.render_view_cube(cx);
        div().absolute().top(px(15.)).right(px(15.)).child(cube).into_any_element()
    }
}

/// The separator between the feature and parts lists being dragged.
#[derive(Clone)]
struct SeparatorDrag;

impl Render for SeparatorDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

#[derive(Clone)]
struct RollbackDrag;

/// The rollback bar's drag preview; `0` is where the bar was grabbed (the
/// preview is drawn at the pointer minus it).
struct RollbackDragPill(Point<Pixels>);

impl Render for RollbackDragPill {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // Onshape (live, part-studio/30): a dark-blue pill "Rollback bar" just
        // right of the pointer, level with it.
        // (Margins on a drag preview's root are ignored; offset a child.)
        div().child(div()
            .relative()
            .left(self.0.x + px(12.))
            .top(self.0.y - px(12.))
            .px(px(10.))
            .py(px(3.))
            .rounded_full()
            .bg(p().accent)
            .text_size(px(13.))
            .font_weight(FontWeight::BOLD)
            .text_color(gpui_kit::rgb(0xffffff))
            .child("Rollback bar"))
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
/// The points a sketch offers a point field (Hole locations): its sketch
/// points, else every vertex (curve ends and centers).
pub(super) fn sketch_points(sk: &noshape_model::display::SketchDisplay) -> Vec<Reference> {
    let standalone: Vec<Reference> = sk.points.iter().filter(|p| p.standalone).map(|p| Reference::SketchEntity { sketch: sk.feature, entity: p.entity }).collect();
    if !standalone.is_empty() {
        return standalone;
    }
    sk.points.iter().filter(|p| !p.handle).map(|p| Reference::SketchEntity { sketch: sk.feature, entity: p.entity }).collect()
}

pub fn kind_matches(kind: PickKind, r: &Reference, f: QueryFilter) -> bool {
    // A parts field that also takes mate connectors or point bodies
    // (Transform, Pattern, Mirror entities) takes a clicked face, edge or
    // vertex as its part, not as an implicit mate connector (Onshape:
    // part-studio/445 reads "Part 1").
    let part_geometry = matches!(kind, PickKind::Face | PickKind::Edge | PickKind::Vertex) && f.intersects(QueryFilter::BODY);
    let f = if part_geometry { QueryFilter(f.0 & !(QueryFilter::MATE_CONNECTOR.0 | QueryFilter::POINT_BODY.0)) } else { f };
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
                        None => match r {
                            Reference::Body(_) => filter.intersects(QueryFilter::BODY),
                            Reference::Sketch(_) => filter.intersects(QueryFilter::SKETCH),
                            Reference::Feature(_) => filter.intersects(QueryFilter::FEATURE),
                            Reference::SketchRegion { .. } => filter.intersects(QueryFilter::SKETCH_REGION),
                            Reference::SketchEntity { .. } => filter.intersects(QueryFilter::SKETCH_EDGE),
                            Reference::Plane(_) | Reference::FeaturePlane(_) => filter.intersects(QueryFilter::PLANE),
                            _ => false,
                        },
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


/// Text drawn *in a plane* like Onshape's plane and sketch names: glyph
/// outlines laid out from the corner `top_left` (inside, along `x`, with
/// `y` up) with ascender height `asc`, projected to viewport polygons.
pub(super) fn in_plane_text(vp: &crate::viewport::Viewport, text: &str, top_left: glam::DVec3, x: glam::DVec3, y: glam::DVec3, asc: f64, polys: &mut Vec<Vec<Point<Pixels>>>) {
    let layout = noshape_sketch::text::layout_text(text, noshape_sketch::text::DEFAULT_FONT, false, false);
    let base = top_left + x * (0.25 * asc) - y * (1.3 * asc);
    for contour in layout.polylines(6) {
        let mut poly = Vec::with_capacity(contour.len());
        for q in contour {
            match vp.project(base + x * (q.x * asc) + y * (q.y * asc)) {
                Some(s) => poly.push(point(px(s.x as f32), px(s.y as f32))),
                None => {
                    poly.clear();
                    break;
                }
            }
        }
        if poly.len() >= 3 {
            polys.push(poly);
        }
    }
}

/// Fills projected glyph polygons (from [`in_plane_text`]) over the viewport.
pub(super) fn paint_paths(polys: Vec<Vec<Point<Pixels>>>, color: Hsla) -> AnyElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let o = bounds.origin;
            let mut b = PathBuilder::fill();
            for poly in &polys {
                b.move_to(point(o.x + poly[0].x, o.y + poly[0].y));
                for q in &poly[1..] {
                    b.line_to(point(o.x + q.x, o.y + q.y));
                }
                b.close();
            }
            if let Ok(path) = b.build() {
                window.paint_path(path, color);
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

impl Render for PartStudioView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        if let Some(v) = self.pending_named_view.take() {
            self.apply_named_view(&v, window, cx);
        }
        self.ensure_filter_input(window, cx);
        // A feature row's context menu closed: drop its highlight.
        if self.ui.list_menu.is_some() && !crate::os_menu::is_open(cx) {
            self.ui.list_menu = None;
            self.refresh_scene(false, cx);
        }
        let viewport_w = f32::from(window.viewport_size().width);
        let toolbar = if self.sketch.is_some() { self.render_sketch_toolbar(viewport_w, cx) } else { self.render_toolbar(viewport_w, cx) };
        let list = self.render_feature_list(window, cx);
        if self.needs_regen {
            self.needs_regen = false;
            self.schedule_regen(window, cx);
        }
        let right = self.render_right_panel(window, cx);
        let mut overlay = self.render_viewport_overlay(cx);
        // Live (part-studio/00, 77): the strip is centered 8 px above the
        // viewport's middle (4 or 5 buttons alike).
        overlay.push(div().absolute().top_0().bottom(px(16.)).right_0().flex().items_center().child(self.render_panel_toggles(cx)).into_any_element());
        // Under everything else, so a feature dialog covers them (live: the
        // dialog hides sketch names and the list toggle).
        let labels = self.render_sketch_labels(window, cx);
        overlay.splice(0..0, labels);
        // Live: centered 8 px above the viewport's middle, like the right strip.
        overlay.insert(0, div().absolute().top_0().bottom(px(16.)).left_0().flex().items_center().child(self.render_list_toggle(cx)).into_any_element());
        overlay.extend(self.render_measure_panel(cx));
        // Live (sketch/01, part-studio/230, 234): also while a sketch or a
        // feature dialog is open.
        overlay.push(self.render_evaluation_tools(cx));
        overlay.extend(self.render_context_menu());
        overlay.extend(self.render_section_panel(cx));
        overlay.extend(self.render_section_value(cx));
        overlay.extend(self.render_palette(window, cx));
        overlay.extend(self.render_shortcut_sheet(cx));
        overlay.extend(self.render_isolate_panel(cx));
        overlay.extend(self.render_appearance_editor(cx));
        overlay.extend(self.render_create_selection(cx));
        overlay.extend(self.render_material_panel(cx));
        overlay.extend(self.render_dynamic_suppression(cx));
        overlay.extend(self.render_insert_go(cx));
        overlay.extend(self.render_regen_panels(cx));
        overlay.extend(self.render_analysis_panel(cx));
        overlay.extend(self.render_dependencies(cx));
        overlay.extend(self.render_shown_dims(cx));
        overlay.extend(self.render_analysis_tool(cx));
        overlay.extend(self.render_color_bar(thickness::Bar::Thickness, cx));
        if matches!(self.ui.analysis, Some(analysis::Analysis::Curvature)) {
            overlay.extend(self.render_color_bar(thickness::Bar::Curvature, cx));
        }
        overlay.extend(self.render_draft_legend());
        overlay.extend(self.render_draft_hover(cx));
        overlay.extend(self.render_thickness_progress());
        overlay.extend(self.render_select_other(cx));
        overlay.extend(self.render_mini_toolbar(cx));
        overlay.extend(self.render_context_banner(cx));
        let in_sketch = self.sketch.is_some();
        let root = v_flex()
            .size_full()
            .key_context(if in_sketch { "PartStudio Sketch" } else { "PartStudio" })
            .track_focus(&self.focus)
            .capture_any_mouse_down(cx.listener(|this, _: &MouseDownEvent, window, cx| {
                // Keep keyboard shortcuts working after clicking the viewport or lists,
                // without stealing focus from an input that just took it.
                // (The tool search closes itself on clicks outside it.)
                if this.palette.is_none() && !this.focus.contains_focused(window, cx) {
                    window.focus(&this.focus, cx);
                }
            }))
            .on_action(cx.listener(Self::on_delete))
            // Shift+Tab on <body> stays there (live psb 10-19 h); elsewhere in
            // the Part Studio it moves back as usual.
            .on_action(cx.listener(|this, _: &crate::actions::StudioTabPrev, window, cx| {
                if !this.focus.is_focused(window) {
                    window.focus_prev(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Copy, _, cx| {
                if this.sketch.is_none() {
                    this.copy_selected_features(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Paste, window, cx| {
                if this.sketch.is_none() && this.edit.is_none() && !this.paste_part(window, cx) {
                    this.paste_features(cx);
                }
            }))
            .on_action(cx.listener(Self::on_clear_selection))
            .on_action(cx.listener(|this, _: &Undo, window, cx| {
                if this.edit.is_some() {
                    this.draft_undo(false, window, cx);
                } else {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|this, _: &Redo, window, cx| {
                if this.edit.is_some() {
                    this.draft_undo(true, window, cx);
                } else {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|this, _: &Cancel, window, cx| {
                // Esc closes transient UI first (S toolbar, Select other,
                // zoom to window, dependencies), then the tool or dialog,
                // then isolate / make transparent, then the selection.
                if this.show_shortcuts {
                    this.toggle_shortcut_sheet(window, cx);
                    return;
                }
                if this.close_insert_go(cx) {
                    return;
                }
                if this.hide_dimensions(window, cx) {
                    return;
                }
                if this.ui.section_dialog.is_some() {
                    this.close_section_dialog(false, window, cx);
                    return;
                }
                if this.ui.env_picker.take().is_some() {
                    cx.notify();
                    return;
                }
                if this.close_analysis_tool(cx) {
                    return;
                }
                if this.ui.create_selection.is_some() {
                    this.close_create_selection(cx);
                    return;
                }
                if this.ui.material_panel.take().is_some() {
                    cx.notify();
                    return;
                }
                if this.ui.mini_toolbar.take().is_some() || this.ui.select_other.take().is_some() || this.ui.dependencies.take().is_some() || this.ui.plane_dependencies.take().is_some() || this.ui.configure_pick.take().is_some() {
                    this.hover = None;
                    this.refresh_scene(false, cx);
                    cx.notify();
                    return;
                }
                if this.viewport.read(cx).zoom_window {
                    this.viewport.update(cx, |vp, cx| {
                        vp.zoom_window = false;
                        cx.notify();
                    });
                    return;
                }
                if this.sketch.is_some() {
                    this.sketch_cancel(cx);
                    return;
                }
                if this.edit.is_some() {
                    this.cancel_edit(window, cx);
                } else if !this.exit_isolate(window, cx) {
                    this.selection.clear();
                    this.selected_features.clear();
                    this.refresh_scene(false, cx);
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &Accept, window, cx| {
                if this.accept_view_tool(cx) {
                    return;
                }
                if this.ui.section_dialog.is_some() {
                    this.close_section_dialog(true, window, cx);
                } else if this.ui.select_other.is_some() {
                    this.choose_select_other(None, cx);
                } else if this.sketch.is_some() {
                    cx.propagate();
                } else if this.edit.is_some() {
                    this.accept_edit(false, window, cx);
                } else if let Some(id) = this.selected_features.first().copied() {
                    // Enter on a selected feature opens it for editing.
                    this.edit_feature(id, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &AcceptAndRepeat, window, cx| {
                if this.edit.is_some() && this.sketch.is_none() {
                    this.accept_edit(true, window, cx);
                } else {
                    cx.propagate();
                }
            }))
            .on_action(cx.listener(|this, _: &HideSelected, _, cx| this.hide_hovered_or_selected(cx)))
            .on_action(cx.listener(|this, _: &ShowAll, _, cx| this.show_hidden(cx)))
            .on_action(cx.listener(|this, _: &IsolateSelected, window, cx| this.isolate(behaviors::IsolateMode::Isolate, window, cx)))
            .on_action(cx.listener(|this, _: &TransparentSelected, window, cx| this.isolate(behaviors::IsolateMode::Transparent, window, cx)))
            .on_action(cx.listener(|this, _: &HideConstruction, window, cx| this.hide_construction(window, cx)))
            .on_action(cx.listener(|this, _: &ZoomWindow, _, cx| this.toggle_zoom_window(cx)))
            .on_action(cx.listener(|this, _: &ShortcutToolbar, _, cx| {
                if this.edit.is_none() || this.sketch.is_some() {
                    this.toggle_mini_toolbar(cx);
                }
            }))
            .on_action(cx.listener(|this, _: &SelectOther, _, cx| this.select_other(false, cx)))
            .on_action(cx.listener(|this, _: &SelectOtherPrevious, _, cx| this.select_other(true, cx)))
            .on_action(cx.listener(|this, _: &MeasureTool, _, cx| {
                this.ui.measure_pinned = !this.ui.measure_pinned;
                this.show_mass = false;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &MateConnectorTool, window, cx| this.start_feature("mateConnector", window, cx)))
            .on_action(cx.listener(|this, _: &ToggleMateConnectors, _, cx| this.toggle_mate_connectors(cx)))
            // Onshape: Shift+C Curve/surface analysis, Shift+D Dihedral analysis.
            .on_action(cx.listener(|this, _: &CurveSurfaceAnalysis, window, cx| this.toggle_analysis_tool(analysis_tools::ToolKind::CurveSurface, window, cx)))
            .on_action(cx.listener(|this, _: &DihedralAnalysis, window, cx| this.toggle_analysis_tool(analysis_tools::ToolKind::Dihedral, window, cx)))
            .on_action(cx.listener(|this, _: &ViewHighQuality, window, cx| this.toggle_high_quality(window, cx)))
            .on_action(cx.listener(|this, _: &PanLeft, _, cx| this.pan_by(-150.0, 0.0, cx)))
            .on_action(cx.listener(|this, _: &PanRight, _, cx| this.pan_by(150.0, 0.0, cx)))
            .on_action(cx.listener(|this, _: &PanUp, _, cx| this.pan_by(0.0, -150.0, cx)))
            .on_action(cx.listener(|this, _: &PanDown, _, cx| this.pan_by(0.0, 150.0, cx)))
            .on_action(cx.listener(|this, _: &NewSketch, window, cx| this.start_feature("sketch", window, cx)))
            .on_action(cx.listener(|this, _: &SearchTools, window, cx| this.open_palette(window, cx)))
            .on_action(cx.listener(|this, _: &ToggleShortcutHelp, window, cx| this.toggle_shortcut_sheet(window, cx)))
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
            .on_action(cx.listener(|this, _: &ZoomToFit, _, cx| this.zoom_fit(cx)))
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| this.viewport.update(cx, |vp, cx| vp.zoom_step(true, cx))))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| this.viewport.update(cx, |vp, cx| vp.zoom_step(false, cx))))
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
                this.ui.rollback_keyboard = Some(this.selected_features.clone());
                let r = this.studio(cx).map(|s| s.rollback_index()).unwrap_or(0);
                if r > 0 {
                    this.set_rollback(Some(r - 1), cx);
                }
            }))
            .on_action(cx.listener(|this, _: &RollbackDown, _, cx| {
                this.ui.rollback_keyboard = Some(this.selected_features.clone());
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
                    .child(if self.ui.list_collapsed {
                        // Collapsed Features and parts list: Onshape collapses it
                        // to nothing; the toggle tab at the viewport's left edge reopens it.
                        div().into_any_element()
                    } else {
                        div()
                            .id("list-panel")
                            .w(px(crate::ui::LEFT_PANEL_W))
                            .h_full()
                            .flex_none()
                            .on_drag_move(cx.listener(|this, e: &DragMoveEvent<SeparatorDrag>, _, cx| {
                                let panel = f32::from(e.bounds.size.height).max(1.0);
                                let h = f32::from(e.bounds.bottom() - e.event.position.y).clamp(60.0, (panel - 120.0).max(60.0));
                                let share = h / panel;
                                if this.ui.parts_height.is_none_or(|p| (share - p).abs() * panel > 0.5) {
                                    this.ui.parts_height = Some(share);
                                    cx.notify();
                                }
                            }))
                            .child(list)
                            .into_any_element()
                    })
                    .child(
                        div()
                            .id("viewport-area")
                            .flex_1()
                            .h_full()
                            .relative()
                            .overflow_hidden()
                            .bg(pal.app_bg)
                            // Resizing the dialog by its corner grip (min 220 x 120).
                            .on_drag_move(cx.listener(|this, e: &DragMoveEvent<crate::feature_dialog::DialogResize>, _, cx| {
                                let origin = this.ui.dialog_pos.unwrap_or(point(px(0.), px(3.)));
                                let p = e.event.position - e.bounds.origin - origin;
                                let size = size(p.x.max(px(220.)), p.y.max(px(120.)));
                                this.ui.dialog_size = Some(size);
                                if let Some(ed) = &this.edit {
                                    ed.dialog.update(cx, |d, cx| {
                                        d.size = Some(size);
                                        cx.notify();
                                    });
                                }
                                cx.notify();
                            }))
                            .on_drag_move(cx.listener(|this, e: &DragMoveEvent<create_selection::CsMove>, _, cx| this.cs_drag(e, cx)))
                            .on_drag_move(cx.listener(|this, e: &DragMoveEvent<crate::feature_dialog::DialogMove>, _, cx| {
                                let grab = *e.drag(cx).0.lock();
                                let p = e.event.position - e.bounds.origin - grab;
                                let max_x = (e.bounds.size.width - px(80.)).max(px(0.));
                                let max_y = (e.bounds.size.height - px(40.)).max(px(0.));
                                this.ui.dialog_pos = Some(point(p.x.clamp(px(0.), max_x), p.y.clamp(px(0.), max_y)));
                                cx.notify();
                            }))
                            .child(self.viewport.clone())
                            .children(overlay),
                    )
                    .children(right),
            );
        self.with_sketch_actions(root, cx)
    }
}

impl PartStudioView {
    /// The FeatureScript notices panel's "Result:" line for this Part Studio.
    pub fn regen_result(&self) -> String {
        match self.display.features.iter().find(|f| f.status == noshape_model::display::FeatureStatus::Error) {
            Some(f) => format!("Result: Regeneration failed: {}", f.message.clone().unwrap_or_default()),
            None => "Result: Regeneration complete".to_string(),
        }
    }

    /// Performance panel › Tab performance and Tab graphics.
    pub fn tab_perf(&self, cx: &App) -> crate::doc_panels::TabPerf {
        let mut features: Vec<(String, usize)> = Vec::new();
        for f in self.studio(cx).map(|s| s.features.as_slice()).unwrap_or_default() {
            let name = noshape_model::features::get(&f.type_id).map(|t| t.name.to_string()).unwrap_or_else(|| f.type_id.clone());
            match features.iter_mut().find(|(n, _)| *n == name) {
                Some((_, c)) => *c += 1,
                None => features.push((name, 1)),
            }
        }
        let d = &self.display;
        let sum = |f: fn(&noshape_model::display::BodyDisplay) -> usize| d.bodies.iter().map(f).sum::<usize>();
        let sketch: usize = d.sketches.iter().map(|s| s.curves.len() + s.points.len()).sum();
        let primitives = [("Faces", sum(|b| b.face_refs.len())), ("Edges", sum(|b| b.edge_refs.len())), ("Vertices", sum(|b| b.vertex_refs.len())), ("Sketch entities", sketch), ("Planes", d.planes.len())];
        crate::doc_panels::TabPerf { regen_ms: d.regen_ms, features, primitives: primitives.into_iter().map(|(k, n)| (k.to_string(), n)).collect() }
    }

    /// Performance panel › the stopwatch: the feature list's regeneration times.
    pub fn show_regen_times(&mut self, cx: &mut Context<Self>) {
        self.ui.show_regen_times = true;
        cx.notify();
    }

    pub fn studio_name(&self, cx: &App) -> String {
        self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_else(|| "Part Studio".into())
    }

    /// Arrow keys / view cube arrows: Onshape's 250 ms rotation steps about
    /// the camera's own axes (`navigation::rotate_step_axis`).
    fn rotate_by<A>(&mut self, _a: &A, dx_deg: f64, dy_deg: f64, cx: &mut Context<Self>) {
        let (dir, deg) = crate::navigation::step_from_degrees(dx_deg, dy_deg);
        self.viewport.update(cx, |vp, cx| vp.rotate_step(dir, deg, cx));
    }
}

/// What a sketch entity is, for selection labels ("Sketch 1 point"): from the
/// sketch definitions, falling back to the displayed sketches.
fn sketch_entity_kinds(studio: Option<&PartStudio>, display: &StudioDisplay) -> std::collections::HashMap<(FeatureId, u32), &'static str> {
    use noshape_sketch::EntityKind;
    let mut kinds = std::collections::HashMap::new();
    for sk in &display.sketches {
        kinds.extend(sk.points.iter().map(|p| ((sk.feature, p.entity), "point")));
        kinds.extend(sk.curves.iter().map(|c| ((sk.feature, c.entity), "curve")));
    }
    for f in studio.map(|s| s.features.as_slice()).unwrap_or_default() {
        let Some(noshape_model::ParamValue::Sketch(data)) = f.params.get("sketch") else { continue };
        for e in data.sketch.entities() {
            let kind = match e.kind() {
                EntityKind::Point => "point",
                EntityKind::Line => "line",
                EntityKind::Circle => "circle",
                EntityKind::Arc => "arc",
                EntityKind::Ellipse => "ellipse",
                EntityKind::EllipticalArc => "elliptical arc",
                EntityKind::BSpline | EntityKind::FitSpline => "spline",
                EntityKind::Conic => "conic",
            };
            kinds.insert((f.id, e.id.0), kind);
        }
    }
    kinds
}
