//! Part Studio tab: toolbar, feature list (with rollback bar), parts list,
//! 3D viewport with selection, and feature dialogs with live preview.

mod analysis;
mod appearance;
mod behaviors;
// What an agent's call asks of this view (live mode).
#[path = "../agent_window/studio.rs"]
mod agent_window;
mod create_selection;
mod hole_callout;
mod triad;
mod material_data;
mod material_panel;
mod config_panel;
mod custom_tables;
pub(crate) mod input_propagation;
mod config_ids;
pub(crate) mod config_var_dialog;
mod annotations_ui;
mod scope;
mod default_tolerances;
mod pending;
mod perf_script;
mod fab_panel;
mod flat_view;
mod sheet_metal_panel;
mod feature_ops;
mod manipulators;
mod menus;
mod box_select;
mod section;
mod section_manip;
mod sketch;
pub(crate) mod shortcut_cmds;
mod analysis_tools;
pub(crate) use noshape_scene::environment_maps;
pub(crate) mod reflection;
mod thickness;
mod flatten_tool;
mod draft_tool;
pub(crate) mod export;
mod connector_look;
mod plane_handles;
mod shown_dims;
pub(crate) mod toolbar_prefs;
pub mod view_cube;
mod view_tools;
mod in_context;
pub use in_context::{AssemblyContext, ContextOccurrence, PartStudioEvent, context_arrow_icon};

use crate::actions::*;
use crate::sketch_mode::SketchEditor;
use crate::assets::{has_icon, icon_path};
use crate::doc_model::{DocEvent, DocModel};
use crate::feature_dialog::{DialogEvent, FeatureDialog, RefLabeler};
use crate::gpu::Gpu;
use crate::scene_builder::{self, BuiltScene, GpuCache, PickKind, SceneOptions};
use crate::theme::p;
use crate::viewport::{Viewport, ViewportEvent};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Icon, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use crate::os_menu::OsContextMenuExt as _;
use gpui_kit::*;
use crate::ui::TipExt as _;
use noshape_model::display::{FeatureStatus, StudioDisplay};
use noshape_model::document::TabId;
use noshape_model::features::{self, FeatureType};
use noshape_model::params::{ParamKind, ParamValue, QueryFilter};
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
    &[&["sheetMetalStart", "sheetMetalFlange", "sheetMetalHem", "sheetMetalTab", "sheetMetalBend", "sheetMetalJog", "sheetMetalFormed", "sheetMetalLoft", "sheetMetalMakeJoint", "sheetMetalCorner", "sheetMetalBendRelief", "sheetMetalCornerBreak", "sheetMetalEnd", "pcbStart", "pcbEnd"]],
];

/// Customize toolbar's tool sets: the toolbar groups, then ⊞ "Add custom
/// features" (live 341: it has a sled and is listed as a tool).
const TOOLSETS: usize = TOOLBAR.len() + 1;
const ADD_CUSTOM_SET: usize = TOOLBAR.len();
static ADD_CUSTOM: [&str; 1] = ["addCustomFeatures"];

/// A narrow window (live part-studio/220-221, dom/toolbar-narrow-900*.txt):
/// a collapsed group is one split tool, its face the tool last used from it
/// (else its first), every tool of the group in its dropdown.
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
        // Live (behaviours "icons2 (10-23) feature-list row icons"): these
        // have no icon of their own; the client shows its fallback, the red
        // warning triangle, whatever the feature's status.
        "cPoint" | "copyPart" | "sheetMetalRip" | "sheetMetalUnfold" | "sheetMetalRefold" => "feature-studio-error",
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
        "sheetMetalJoint" => "joint",
        "sheetMetalMakeJoint" => "sheet-metal-make-joint",
        "sheetMetalJog" => "jog",
        "sheetMetalFormed" => "form",
        "pcbStart" => "pcb-start",
        "pcbEnd" => "pcb-end",
        "endcap" => "end-cap",
        "cutlist" => "cut-list",
        "importDerived" => "derived",
        "importForeign" => "import-foreign",
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
            // A type without an icon: the client's fallback (live), not a part.
            return icon_path(if has_icon(&s) { &s } else { "feature-studio-error" });
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

/// What a new folder from the "Folder name" popup wraps: features (Add
/// selection to folder…, the header's New folder) or a folder (its row's
/// Add selection to folder…, live ui 10-25 b (2)).
enum FolderTarget {
    Features(Vec<FeatureId>),
    Folder(u32),
}

pub struct PartStudioView {
    pub tab: TabId,
    doc: Entity<DocModel>,
    gpu: Gpu,
    /// Versions and history › Compare graphics.
    compare_gfx: crate::compare_graphics::CompareGraphics,
    pub(crate) viewport: Entity<Viewport>,
    display: Arc<StudioDisplay>,
    built: Option<BuiltScene>,
    /// While a feature dialog previews: the model before the feature, the
    /// preview blend's base (`preview_mix_base`).
    built_base: Option<BuiltScene>,
    cache: GpuCache,
    pub selection: Vec<Reference>,
    hover: Option<Reference>,
    opts: SceneOptions,
    edit: Option<EditSession>,
    selected_features: Vec<FeatureId>,
    regen: Arc<Mutex<Regenerator>>,
    regen_seq: u64,
    regen_task: Option<Task<()>>,
    /// The running regenerations (model regen::runs: a newer request
    /// aborts or supersedes each).
    regen_runs: noshape_model::regen::runs::Runs,
    /// A regeneration whose result isn't shown yet, its overlay timer and
    /// spinner.
    pending: Option<pending::Pending>,
    pending_timer: Option<Task<()>>,
    pending_spinner: Option<Entity<pending::Spinner>>,
    /// Custom features shown in the toolbar (for its width).
    focus: FocusHandle,
    /// Cached parts of the window (toolbar, viewport overlay, lists): see `cached_slice`.
    slices: Option<PsSlices>,
    /// The features the list's rows last showed for the filter (None: no
    /// filter), so a keystroke that keeps them re-renders only the head.
    filter_rows: FilterMatches,
    /// What the list reads from the model every frame, kept between frames
    /// (see `ListCache`).
    list_cache: std::cell::RefCell<ListCache>,
    /// What the feature rows read from the list as a whole, set when the
    /// list renders.
    row_ctx: RowCtx,
    fitted_once: bool,
    message: Option<(FeatureStatus, String)>,
    /// A sketch cancelled with ✕: its draft, index and whether it was new,
    /// for the toast's Restore link (live sketch/68), with the toast text.
    canceled_edit: Option<(Feature, usize, bool, String)>,
    sketch: Option<SketchEditor>,
    /// The sketch fillet radius last used in this tab: live's default is
    /// R5 on a fresh load and then sticks per tab (behaviours.md 568).
    sketch_fillet_radius: Option<f64>,
    /// Sketch dialog options toggled before the sketch plane is picked (the
    /// editor opens with them).
    sketch_pending_options: Option<crate::sketch_mode::SketchOptions>,
    context: Option<Arc<noshape_model::regen::context::Context>>,
    context_menu: Option<(Point<Pixels>, Entity<crate::os_menu::OsMenu>, Subscription)>,
    measure_cache: Option<MeasureCache>,
    show_mass: bool,
    right_panel: Option<config_panel::RightPanel>,
    panel_inputs: std::collections::HashMap<String, (Entity<gpui_kit::component::input::InputState>, String)>,
    /// Panel cells whose value must pass a check before it commits.
    panel_checks: std::collections::HashMap<String, fn(&str) -> bool>,
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
    /// The "Folder name" popup: its input and what the new folder wraps.
    folder_prompt: Option<(Entity<gpui_kit::component::input::InputState>, FolderTarget, Subscription)>,
    named_views: Option<(Entity<gpui_kit::component::input::InputState>, Subscription)>,
    /// The Named views dialog's "Select a view" list and the view picked in it.
    named_view_picked: Option<String>,
    flat_cache: Option<fab_panel::FlatCache>,
    flat_task: Option<(usize, Task<()>)>,
    /// The Sheet metal table and flat view panel's graphics area.
    flat_view: Option<Entity<flat_view::FlatView>>,
    /// Onshape interaction state (hide/isolate, select other, S toolbar...).
    ui: behaviors::UiState,
    /// `NOSHAPE_PERF_SCRIPT` (benchmarks).
    perf_script: Option<perf_script::Script>,
    _subs: Vec<Subscription>,
}

impl Focusable for PartStudioView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl PartStudioView {
    /// The 3D viewport (smoke test).
    pub(crate) fn viewport_entity(&self) -> Entity<Viewport> {
        self.viewport.clone()
    }

    pub fn new(doc: Entity<DocModel>, tab: TabId, gpu: Gpu, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let viewport = cx.new(|cx| Viewport::new(gpu.clone(), window, cx));
        let mut subs = Vec::new();
        subs.push(cx.subscribe_in(&viewport, window, |this, vp, ev, window, cx| {
            this.on_viewport_event(vp, ev, window, cx);
            this.push_tool_cursor(cx);
        }));
        subs.push(cx.subscribe_in(&doc, window, |this: &mut Self, _, ev: &DocEvent, window, cx| {
            // Any document change may change what the list's caches hold.
            *this.list_cache.get_mut() = ListCache::default();
            match ev {
                DocEvent::TabChanged(t) if *t == this.tab => this.schedule_regen(window, cx),
                DocEvent::TabChanged(t) => {
                    // Variable Studio edits affect every Part Studio.
                    let doc = &this.doc.read(cx).session.doc;
                    let is_vars = matches!(doc.tab(*t).map(|t| &t.content), Some(noshape_model::TabContent::VariableStudio(_)));
                    // A changed Part Studio may be the source of our Derived features.
                    let is_source = matches!(doc.tab(*t).map(|t| &t.content), Some(noshape_model::TabContent::PartStudio(_)))
                        && this.studio(cx).is_some_and(|s| s.features.iter().any(|f| noshape_model::features::importing::is_derived(&f.type_id) && f.params.text("partStudio") == t.to_string()));
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
                // Live: Workspace units ✓ keeps an open sketch or feature dialog;
                // its unitless sketch dimensions keep their size like the stored
                // ones (`pin_sketch_units`).
                DocEvent::UnitsChanged { old } => {
                    this.pin_draft_units(old, cx);
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
            }
        }));
        let mut this = Self {
            tab,
            doc,
            gpu,
            compare_gfx: Default::default(),
            viewport,
            display: Arc::new(StudioDisplay::defaults()),
            built: None,
            built_base: None,
            cache: GpuCache::default(),
            selection: Vec::new(),
            hover: None,
            opts: SceneOptions { show_planes: true, show_sketches: true, show_origin: true, ..Default::default() },
            edit: None,
            selected_features: Vec::new(),
            regen: Arc::new(Mutex::new(Regenerator::new())),
            regen_seq: 0,
            regen_task: None,
            regen_runs: Default::default(),
            pending: None,
            pending_timer: None,
            pending_spinner: None,
            focus: cx.focus_handle(),
            slices: None,
            filter_rows: None,
            list_cache: Default::default(),
            row_ctx: RowCtx::default(),
            fitted_once: false,
            message: None,
            canceled_edit: None,
            sketch: None,
            sketch_fillet_radius: None,
            sketch_pending_options: None,
            context: None,
            context_menu: None,
            measure_cache: None,
            show_mass: false,
            right_panel: None,
            panel_inputs: Default::default(),
            panel_checks: Default::default(),
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
            named_view_picked: None,
            flat_cache: None,
            flat_task: None,
            flat_view: None,
            ui: behaviors::UiState::new(cx),
            perf_script: perf_script::from_env(),
            _subs: subs,
        };
        // Graphics preferences apply live (camera and render options dialog).
        this._subs.push(cx.observe_global::<crate::graphics_prefs::GraphicsGlobal>(|this, cx| this.apply_graphics_prefs(cx)));
        this.apply_graphics_prefs(cx);
        // The document thumbnail draws this view's display while it is open.
        let path = this.doc.read(cx).session.path.clone();
        cx.on_release(move |this: &mut Self, cx: &mut App| crate::start_thumbnail::view_closed(&path, this.tab, cx)).detach();
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

    #[track_caller]
    pub fn schedule_regen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let caller = std::panic::Location::caller();
        // Paused regeneration (Onshape): edits wait until "Regenerate".
        if self.ui.regen_paused.is_some() && !std::mem::take(&mut self.ui.force_regen) {
            // Nothing starts: an "Opening…" waiting for this run goes.
            self.pending.take_if(|p| p.seq == 0);
            cx.notify();
            return;
        }
        let Some((studio, upto)) = self.effective_studio(cx) else { return };
        let path = self.doc.read(cx).session.path.clone();
        crate::start_thumbnail::regen_started(&path, self.tab, cx);
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
        if noshape_render::perf::enabled() {
            log::info!(target: "perf", "schedule_regen #{seq} from {caller} (upto {upto:?}, {} features)", studio.features.len());
        }
        let editing_sketch = self.sketch.as_ref().map(|s| s.feature).or_else(|| self.editing_sketch_id(cx));
        // The studio's head as the document thumbnail draws it.
        let head = upto.is_none() && editing_sketch.is_none() && config == studio.configuration.defaults();
        let shown_vars = doc_vars.clone();
        // The newest request wins (pending.rs): the running one is aborted, or
        // finishes its features into the cache when this one only extends it.
        let studio = Arc::new(studio);
        let cancel = noshape_model::regen::RegenCancel::default();
        self.regen_runs.start(noshape_model::regen::runs::Run { cancel: cancel.clone(), studio: studio.clone(), upto, config: config.clone(), doc_vars: doc_vars.clone(), editing_sketch });
        self.begin_pending(seq, window, cx);
        let cancel2 = cancel.clone();
        // View in high quality (Shift+R, kept for the session once used):
        // the Very fine level, refined to view by view.
        let deflection = RegenOptions::default().relative_deflection * if self.ui.high_quality_seen { 0.25 } else { 1.0 };
        self.regen_task = Some(cx.spawn_in(window, async move |this, cx| {
            // Coalesce bursts of edits (typing in a field).
            cx.background_executor().timer(Duration::from_millis(12)).await;
            let result = cx
                .background_spawn(async move {
                    let opts = RegenOptions { upto, editing_sketch, document_variables: doc_vars.clone(), preview_meshes: true, relative_deflection: deflection, derived: Some(derived), custom_features: Some(custom), cancel: Some(cancel), ..Default::default() };
                    let r = regen.lock().regenerate(&studio, &config, &units, &opts);
                    (r, studio, config, units, upto, editing_sketch, doc_vars)
                })
                .await;
            let (result, studio, config, units, upto, editing_sketch, doc_vars) = result;
            // A newer request took over.
            if result.cancelled {
                if noshape_render::perf::enabled() {
                    log::info!(target: "perf", "regen #{seq} cancelled (preview pass)");
                }
                return;
            }
            let refine = result.needs_refine;
            let _ = this.update_in(cx, |this: &mut Self, window, cx| {
                if seq != this.regen_seq {
                    return;
                }
                if let Some(p) = &mut this.pending {
                    p.opening = None;
                }
                if !refine {
                    this.end_pending(seq, cx);
                }
                this.context = Some(result.context.clone());
                scene_builder::log_regen_perf(if refine { "preview" } else { "full" }, &result.display, &studio);
                let display = Arc::new(result.display);
                if !refine {
                    let made = head.then(|| (studio.clone(), shown_vars.clone(), display.clone()));
                    crate::start_thumbnail::regen_done(&path, this.tab, made, cx);
                }
                this.apply_display(display, window, cx);
                if !refine {
                    this.perf_script_step(cx);
                }
                // Remove / Intersect: the see-through tool body follows.
                this.schedule_tool_preview(window, cx);
            });
            // Progressive meshing: refine preview meshes in the background.
            if refine {
                let regen2 = regen2;
                let fine = cx
                    .background_spawn(async move {
                        let opts = RegenOptions { upto, editing_sketch, document_variables: doc_vars, preview_meshes: false, relative_deflection: deflection, derived: Some(derived2), custom_features: Some(custom2), cancel: Some(cancel2), ..Default::default() };
                        let r = regen2.lock().regenerate(&studio, &config, &units, &opts);
                        (r, studio)
                    })
                    .await;
                let (fine, studio) = fine;
                if fine.cancelled {
                    return;
                }
                let _ = this.update_in(cx, |this: &mut Self, window, cx| {
                    if seq != this.regen_seq {
                        return;
                    }
                    this.end_pending(seq, cx);
                    this.context = Some(fine.context.clone());
                    scene_builder::log_regen_perf("refine", &fine.display, &studio);
                    let display = Arc::new(fine.display);
                    crate::start_thumbnail::regen_done(&path, this.tab, head.then(|| (studio.clone(), shown_vars, display.clone())), cx);
                    this.apply_display(display, window, cx);
                    this.perf_script_step(cx);
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
    /// The name a feature shows in the list, and in the chips that name it:
    /// a Hole's callout, a Variable's "#name = value" (see [`listed_name`]).
    fn shown_feature_name(&self, f: &Feature, cx: &App) -> String {
        listed_name(f, &self.computed_values(f.id, cx), &self.doc.read(cx).session.doc.settings.units, &self.display.variables)
    }

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
            // The regenerated preview's values too: the title reads
            // "#d = 95.635 mm" (`measuredValue`).
            let mut out: std::collections::HashMap<String, String> = self.computed_values(d.feature.id, cx).into_iter().collect();
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
        // A shown center of mass follows its part (gone with it).
        if geometry_changed && let Some((b, _)) = self.ui.com.take() {
            self.toggle_center_of_mass(&b, cx);
        }
        // A different set of parts (a derived studio's parts arriving after
        // its source regenerated, qa5 TAB1): redraw the whole window once.
        // The cached workspace kept painting the old parts list and strip
        // ("Parts (0)") although this view was notified.
        let parts_changed = self.display.bodies.len() != display.bodies.len() || self.display.bodies.iter().zip(&display.bodies).any(|(a, b)| a.id != b.id || a.name != b.name);
        self.display = display;
        if parts_changed {
            window.refresh();
        }
        // Measurements follow the new geometry (qa5 RD1: a stale readout).
        if geometry_changed {
            self.measure_cache = None;
        }
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
        self.build_preview_base(&opts, cx);
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
            // Live (qa5 DER6): a second Derived of the same source and
            // configuration only fails once accepted; its dialog stays black
            // with ✓ enabled.
            let st = st.filter(|(_, m)| !m.starts_with("Creating multiple Derived features from the same source"));
            let faulty = self.display.feature_state(id).map(|s| s.faulty.clone()).unwrap_or_default();
            // Live (sketch/00-sketch-dialog-empty.png): a sketch without a
            // plane asks for one in a blue bubble over the graphics.
            const NO_PLANE: &str = "Select a sketch plane";
            let no_plane = e.dialog.read(cx).feature.type_id == "sketch" && st.as_ref().is_some_and(|(k, m)| *k == FeatureStatus::Error && m == NO_PLANE);
            // Live (qa5 BOOL1): a feature's info ("Boolean resulted in no
            // geometry change…") is a blue bubble at the top centre of the
            // graphics while the dialog is open; the dialog shows nothing.
            // A warning (live L27, Fill) is a yellow bubble there, with the
            // title red.
            let bubble = st.as_ref().filter(|(k, _)| matches!(k, FeatureStatus::Info | FeatureStatus::Warning)).cloned();
            let info = bubble.as_ref().map(|(_, m)| m.clone());
            let st = st.filter(|(k, _)| *k != FeatureStatus::Info);
            if no_plane {
                self.message = Some((FeatureStatus::Info, NO_PLANE.into()));
            } else if let Some(b) = &bubble {
                self.message = Some(b.clone());
            } else if self.message.as_ref().is_some_and(|(_, m)| m == NO_PLANE || self.ui.dialog_info.as_ref() == Some(m)) {
                self.message = None;
            }
            self.ui.dialog_info = info;
            let regen_ok = self.display.feature_state(id).is_some_and(|s| matches!(s.status, FeatureStatus::Ok | FeatureStatus::Info | FeatureStatus::Warning));
            e.dialog.update(cx, |d, cx| {
                d.note_regenerated(regen_ok);
                d.set_status(st, cx);
                d.set_faulty(faulty, cx);
            });
        } else if let Some(m) = self.ui.dialog_info.take()
            && self.message.as_ref().is_some_and(|(_, x)| *x == m)
        {
            // Live (qa5 BOOL1): no bubble or toast once the dialog closes.
            self.message = None;
        }
        // A new Extrude / Revolve / Sweep / Loft: operation and merge scope
        // from what the previewed tool touches (unless std's editing logic
        // decides them, `std_edit_logic`).
        let std_logic = self.edit.as_ref().is_some_and(|e| noshape_model::custom::std_feature::has_editing_logic(&e.dialog.read(cx).feature.type_id));
        if !std_logic {
            self.boolean_edit_logic(cx);
        }
        self.hole_scope_logic(cx);
        self.orient_for_sketch(cx);
        self.refresh_scene(true, cx);
        if !self.fitted_once {
            self.fitted_once = true;
            // Onshape opens a Part Studio at Trimetric, orthographic, fitted to
            // its parts and the default planes' box even when those are hidden
            // (live "Zoom to fit (perf 10-23 22:40)" (3)).
            let bounds = self.load_fit_bounds();
            self.viewport.update(cx, |vp, cx| {
                vp.camera.rotation = noshape_render::Camera::rotation_for(noshape_render::StandardView::Trimetric);
                if let Some((lo, hi)) = bounds {
                    let size = vp.size();
                    crate::viewport::fit_camera_to_box(&mut vp.camera, lo, hi, size, 0.0);
                }
                vp.request_redraw(cx);
            });
        }
        cx.notify();
    }

    /// Rebuild highlight state and push the scene to the viewport.
    /// Scene options with the view state that isn't kept in `opts`: zebra
    /// stripes (which can hide the edges).
    fn scene_options(&mut self, cx: &App) -> SceneOptions {
        let mut o = self.opts.clone();
        o.replaced_meshes = self.thickness_meshes(cx);
        o.preview_bodies = self.preview_body_ids();
        // The preview blend's model with the feature (`preview_mix_base`).
        if self.preview_mix_base(cx).is_some() {
            o.body_layer = noshape_render::scene::compare::TARGET;
        }
        o.display_only = self.ui.transform_ghost.clone();
        // Isolate (live ui/iso-901): the planes fade to faint outlines; below
        // 10 % the other parts are solid washed-out ghosts.
        // Show dimensions: the parts' shadowed faces come up a little (live).
        o.dims_shown = self.ui.shown_dims.is_some();
        if let Some(iso) = &self.ui.isolate {
            o.faint_planes = iso.mode == behaviors::IsolateMode::Isolate;
            if matches!(behaviors::iso_look(iso.opacity), behaviors::IsoLook::Ghost) {
                o.ghost_bodies = self.isolation_transparent().into_iter().collect();
            }
        }
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

    /// The model before the edited feature while its dialog previews
    /// (live previewBlend: both models drawn, mixed at the preview slider's
    /// value). Not for sketches (no preview slider), nor while a Remove /
    /// Intersect tool (or a failing Add's) or a Transform's ghosts show
    /// (drawn directly).
    fn preview_mix_base(&self, cx: &App) -> Option<Arc<StudioDisplay>> {
        let e = self.edit.as_ref()?;
        if self.sketch.is_some() || e.dialog.read(cx).feature.type_id == "sketch" || !self.ui.transform_ghost.is_empty() || (self.ui.preview_tool.is_some() && !self.preview_tool_blended()) {
            return None;
        }
        self.ui.preview_before.clone()
    }

    /// The tool preview is an Add's that regenerates: blended like any
    /// preview (`display_for_scene_inner`).
    pub(super) fn preview_tool_blended(&self) -> bool {
        self.ui.preview_tool_adds && !self.tool_missed() && !self.ui.preview_tool.as_ref().is_some_and(|(_, fid)| self.display.features.iter().any(|f| f.id == *fid && f.status == noshape_model::display::FeatureStatus::Error))
    }

    /// Build [`Self::built_base`] with the view options of the scene.
    fn build_preview_base(&mut self, opts: &SceneOptions, cx: &App) {
        let base = self.preview_mix_base(cx);
        self.built_base = base.map(|d| {
            let o = SceneOptions { show_planes: false, show_sketches: false, show_origin: false, body_layer: noshape_render::scene::compare::BASE, preview_bodies: Vec::new(), display_only: Vec::new(), ..opts.clone() };
            scene_builder::build(&d, &o, &mut self.cache, self.gpu.uploader())
        });
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
        self.build_preview_base(&opts, cx);
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
                // A feature picked in a field (Features to pattern) shows its
                // geometry (live part-studio/2022–2023: the seed wall outlined).
                let with_geometry = |refs: Vec<Reference>| -> Vec<Reference> {
                    let mut out = Vec::new();
                    for r in refs {
                        if let Reference::Feature(f) = r {
                            out.extend(self.feature_geometry(f));
                        }
                        out.push(r);
                    }
                    out
                };
                // Only the active field's picks are highlighted (live
                // psui/m-03, lp-03); the other fields' stay unlit.
                (with_geometry(d.active_refs()), Vec::new())
            }
            None => (self.highlighted_selection(), Vec::new()),
        };
        // Hovering a feature or part in the list lights up all of its
        // geometry; a single hovered entity uses the hover highlight.
        let hovered = self.highlighted_hover(cx);
        // Trim (live, qa5 sketch tools): only the piece that would go is
        // highlighted (the sketch overlay draws it), not the whole curve.
        let trimming = self.sketch.as_ref().is_some_and(|s| s.tool == crate::sketch_mode::Tool::Trim);
        let hovered: Vec<Reference> = hovered.into_iter().filter(|r| !(trimming && matches!(r, Reference::SketchEntity { .. }))).collect();
        // A pattern instance point under the cursor hides the geometry hover
        // (live p1audit skip-03: the edge under the hovered point stays unlit).
        let hovered = if self.ui.toggle_hover.is_some() { Vec::new() } else { hovered };
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
        accent.extend(self.sm_hover_refs());
        let cs = self.cs_preview();
        if let Some((seeds, _)) = &cs {
            selection.extend(seeds.iter().cloned());
        }
        let mut extra = self.sketch_overlay(&mut selection, &mut accent, cx);
        // The manipulator's glyph follows the camera: the renderer builds it
        // for every frame (camera_geometry.rs).
        let mut manip: Vec<std::sync::Arc<dyn noshape_render::camera_geometry::CameraGeometry>> = self.manipulator_geometry(cx).into_iter().map(|g| g as std::sync::Arc<dyn noshape_render::camera_geometry::CameraGeometry>).collect();
        // A selected plane's resize rings (plane_handles.rs).
        manip.extend(self.plane_ring_geometry().map(|g| g as std::sync::Arc<dyn noshape_render::camera_geometry::CameraGeometry>));
        // Mate connector glyphs and their owner parts (connector_look.rs).
        let (mc_glyphs, mc_owners) = self.connector_overlays(&selection, cx);
        manip.extend(mc_glyphs.map(|g| g as std::sync::Arc<dyn noshape_render::camera_geometry::CameraGeometry>));
        extra.extend(mc_owners);
        self.viewport.update(cx, |vp, cx| vp.set_camera_geometry(manip, cx));
        extra.extend(self.consumed_picks_overlay(&selection, &accent));
        extra.extend(self.select_other_overlay());
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
        // Live (behaviours "URGENT (10-22)"): a failing feature's inputs are
        // drawn red while its dialog is open.
        let failing = self.edit.as_ref().is_some_and(|e| matches!(e.dialog.read(cx).status, Some((noshape_model::display::FeatureStatus::Error, _))));
        // A failed Feature pattern shows the instances it couldn't make, and
        // its inputs keep the usual selection colors (the user's live shot
        // 2026-10-23: the seed wall orange, the instances red).
        let error_mesh = self.edit.as_ref().filter(|_| failing).and_then(|e| self.display.feature_state(e.dialog.read(cx).feature.id)).and_then(|s| s.error_mesh.clone());
        if let Some(m) = &error_mesh {
            extra.extend(scene_builder::error_geometry(m, self.gpu.uploader()));
        }
        // Debug geometry of the open feature (Helix profiles, FeatureScript debug()).
        if let Some(st) = self.edit.as_ref().and_then(|e| self.display.feature_state(e.dialog.read(cx).feature.id))
            && (!st.debug.lines.is_empty() || !st.debug.points.is_empty() || !st.debug.colored.is_empty())
        {
            extra.extend(scene_builder::debug_geometry(&st.debug, self.gpu.uploader()));
        }
        // Live (p1audit skip-02..04): the instance points on top, 12 px
        // white discs ringed #606a72, the hovered one #134e96, a skipped
        // one #a2b9df; the seed's a 4 px white dot.
        if let Some(pts) = self.toggle_points(cx).filter(|p| !p.is_empty()) {
            use noshape_render::{LineSegment, PointSprite};
            let sprites = pts
                .iter()
                .map(|t| {
                    let (color, size, style) = if t.seed {
                        ([255, 255, 255, 255], 4.0, LineSegment::ON_TOP)
                    } else if self.ui.toggle_hover == Some(t.index) {
                        ([0x13, 0x4e, 0x96, 255], TOGGLE_POINT_PX, LineSegment::ON_TOP | PointSprite::HOLLOW | PointSprite::THICK)
                    } else if t.selected {
                        ([0xa2, 0xb9, 0xdf, 255], TOGGLE_POINT_PX, LineSegment::ON_TOP | PointSprite::HOLLOW | PointSprite::THICK)
                    } else {
                        ([0x60, 0x6a, 0x72, 255], TOGGLE_POINT_PX, LineSegment::ON_TOP | PointSprite::HOLLOW | PointSprite::THICK)
                    };
                    PointSprite { position: glam::DVec3::from_array(t.pos).as_vec3().into(), color, pick_id: 0, size, style, _pad: 0 }
                })
                .collect();
            let up = self.gpu.uploader();
            extra.push(noshape_render::Instance::new(noshape_render::Drawable::Points(up.upload_points(&noshape_render::PointData { points: sprites }))).per_primitive_color().unpickable().ignore_clip().layer(22));
        }
        crate::nav_trace::highlights("part_studio", &selection, single_hover.as_ref());
        // Planes picked in the graphics stop dragging their rings once
        // deselected (a later pick from the list doesn't drag, live).
        let sel = &self.selection;
        self.ui.planes_picked_in_graphics.retain(|r| sel.contains(r));
        // A plane being resized: its highlight follows the drag.
        let shown = self.with_plane_drag(self.display.clone());
        let mut scene = scene_builder::with_highlights(built, &shown, &selection, single_hover.as_ref(), &accent, extra, failing && error_mesh.is_none(), self.gpu.uploader());
        // A selected Decal row tints its image like a selected face (live).
        for (i, _, f) in &built.decals {
            if self.selected_features.contains(f)
                && let Some(inst) = scene.instances.get_mut(*i)
            {
                inst.decal_selected = true;
            }
        }
        // Overlays the active field can't take (a shown sketch line on a
        // part edge for Fillet's edges) leave the pick pass, so what lies
        // under them is picked. Faces keep hiding what's behind them.
        if let Some(kinds) = self.pick_kinds(cx) {
            let overlay = |k: PickKind| matches!(k, PickKind::SketchCurve | PickKind::SketchPoint | PickKind::SketchRegion | PickKind::Edge | PickKind::Vertex);
            for id in built.picks.ids_where(|t| overlay(t.kind) && !kinds.contains(&t.kind)) {
                scene.add_flag(id, noshape_render::scene::state::NO_PICK);
            }
        }
        // Live (selection/10): a feature hovered in the list outlines its
        // part's silhouette in the hover orange, on the outline itself; its
        // faces keep their shading and its inner edges stay black.
        if let Some(f) = self.ui.list_hover.or(self.ui.list_menu).or(self.ui.insp.hover_feature) {
            let mut faces = Vec::new();
            for g in self.feature_geometry(f) {
                let face = matches!(g, Reference::Face(_));
                if !face && !matches!(g, Reference::Edge(_)) {
                    continue;
                }
                for &id in built.picks.ids_for(&g) {
                    if let Some(s) = scene.states.get_mut(id as usize) {
                        *s &= !noshape_render::state::ACCENT;
                    }
                }
                if face {
                    faces.push(g);
                }
            }
            scene.instances.extend(scene_builder::hover_silhouette(&self.display, &faces, self.gpu.uploader()));
        }
        // Live (lp wall over 2258): the pattern's other inputs (its
        // Direction) are drawn red; the seed keeps the selection colors.
        if error_mesh.is_some()
            && let Some(e) = &self.edit
        {
            let d = e.dialog.read(cx);
            for r in d.active_refs().into_iter().chain(d.selected_refs()) {
                match r {
                    // The seed keeps its shading with a light selection
                    // tint, outlined orange, its hidden edges dashed (live
                    // #717560 end face, #ffa90a edges).
                    Reference::Feature(f) => {
                        let faces: Vec<Reference> = self.feature_geometry(f).into_iter().filter(|g| matches!(g, Reference::Face(_))).collect();
                        for g in &faces {
                            for &id in built.picks.ids_for(g) {
                                if let Some(s) = scene.states.get_mut(id as usize) {
                                    *s = (*s & !(noshape_render::state::SELECTED | noshape_render::state::ACCENT)) | noshape_render::state::TINT;
                                }
                            }
                        }
                        scene.instances.extend(scene_builder::hidden_edges_dashed(&self.display, &faces, self.gpu.uploader()));
                    }
                    // Red, not selected: a selected stroke draws over the
                    // error color (qa4 gate v0.4.7: the edge was orange).
                    r => {
                        for &id in built.picks.ids_for(&r) {
                            if let Some(s) = scene.states.get_mut(id as usize) {
                                *s &= !(noshape_render::state::SELECTED | noshape_render::state::ACCENT | noshape_render::state::BODY);
                            }
                            scene.add_flag(id, noshape_render::state::ERROR);
                        }
                    }
                }
            }
        }
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
        // With a feature dialog open, the dialog's picks (live dom/cursors-count:
        // a new Extrude with 0 / 1 / 2 faces shows default / 1 / 2, Fillet with
        // 3 edges 3).
        let count = match (&self.sketch, &self.edit) {
            (Some(ed), _) => ed.selection.len(),
            (None, None) => self.selection.len(),
            (None, Some(e)) => count_picks(&e.dialog.read(cx).feature.params),
        };
        // A feature preview (live previewBlend): the model before the feature
        // under the one with it, mixed at the preview slider's value.
        if let Some(base) = &self.built_base {
            scene.instances.extend(base.scene.instances[..base.static_instances].iter().cloned().map(|mut i| {
                i.pickable = false;
                // Past any highlight state.
                i.pick_base = 1 << 30;
                i
            }));
            scene.preview_mix = Some(self.ui.preview_opacity);
        }
        // Versions and history › Compare: both sides (crate::compare_graphics).
        let (mode, up) = (self.opts.display_mode, self.gpu.uploader());
        let owner = crate::compare_graphics::Owner::<Self> { get: |this: &mut Self| &mut this.compare_gfx, then: |this, cx| this.refresh_scene(false, cx) };
        match self.compare_gfx.scene(&self.doc, self.tab, mode, up, owner, cx) {
            Some(compare) => scene = compare,
            None if crate::compare_mode::blend(&self.doc, cx).is_none() => self.compare_gfx.clear(),
            None => {}
        }
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
        // Chips name a feature as its row does (live: a Hole's chip is its
        // callout "Ø 6.35 mm ↧ 12 mm", behaviours "qa5 (10-25 p1)" (3)).
        let names: Vec<(FeatureId, String)> = self.studio(cx).map(|s| s.features.iter().map(|f| (f.id, self.shown_feature_name(f, cx))).collect()).unwrap_or_default();
        // A part the edited feature consumes (Delete part) is named as it was
        // before the feature (live part-studio/577, qa5 DEL1: "Part 1").
        let before = self.ui.preview_before.as_ref().map(|d| d.bodies.as_slice()).unwrap_or_default();
        let bodies: Vec<(noshape_model::BodyId, String)> = self.display.bodies.iter().chain(before).map(|b| (b.id.clone(), b.name.clone())).collect();
        let entity_kinds = sketch_entity_kinds(self.studio(cx), &self.display);
        let renamed: Vec<(Reference, String)> = self.studio(cx).map(|s| s.default_names.clone()).unwrap_or_default();
        // Assembly context geometry reads "Face of Context 1" (live 504).
        let contexts: Vec<(u32, String)> = self.studio(cx).map(|s| s.contexts.iter().map(|c| (c.id, c.name.clone())).collect()).unwrap_or_default();
        // Derived parts keep their source's face names (so references follow
        // the source); their faces and edges read "… of Derived 1" (live,
        // qa5 DER4).
        let derived_ids: Vec<FeatureId> = self.studio(cx).map(|s| s.features.iter().filter(|f| f.type_id == "importDerived").map(|f| f.id).collect()).unwrap_or_default();
        let derived: Vec<(FeatureId, Vec<Reference>)> = self
            .display
            .bodies
            .iter()
            .filter(|b| derived_ids.contains(&b.id.feature))
            .map(|b| (b.id.feature, b.face_refs.iter().chain(&b.edge_refs).cloned().collect()))
            .collect();
        // …also when the entity is gone from the shown model (a fillet's
        // edge while that fillet is edited): the source feature that created
        // it belongs to a Derived feature's source studio.
        let derived_sources: Vec<(FeatureId, std::collections::HashSet<FeatureId>)> = self
            .studio(cx)
            .map(|s| {
                let doc = &self.doc.read(cx).session.doc;
                s.features
                    .iter()
                    .filter(|f| f.type_id == "importDerived")
                    .filter_map(|f| {
                        let key = f.params.text("partStudio").trim().to_string();
                        match doc.tabs.iter().find(|t| t.id.to_string() == key).map(|t| &t.content) {
                            Some(noshape_model::document::TabContent::PartStudio(src)) => Some((f.id, src.features.iter().map(|g| g.id).collect())),
                            _ => None,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        Arc::new(move |r: &Reference| {
            let owner = |id: Option<FeatureId>| match id {
                Some(i) if names.iter().any(|(x, _)| *x == i) || noshape_model::in_context::context_of(i).is_some() => Some(i),
                _ => derived
                    .iter()
                    .find(|(_, refs)| refs.contains(r))
                    .map(|(f, _)| *f)
                    .or_else(|| id.and_then(|i| derived_sources.iter().find(|(_, src)| src.contains(&i)).map(|(f, _)| *f)))
                    .or(id),
            };
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
                Reference::Face(n) => format!("Face of {}", owner(n.creator()).map(fname).unwrap_or_else(|| "part".into())),
                Reference::Edge(n) => format!("Edge of {}", owner(edge_creator(n)).map(fname).unwrap_or_else(|| "part".into())),
                // A vertex reads "Vertex of <feature>", named by the latest
                // feature among its faces' creators (live "qa5 sm" (7): the
                // inner bend corner of two flanges is "Vertex of Flange 1").
                Reference::Vertex(TopoName::Vertex { faces, .. }) => {
                    let latest = faces.iter().filter_map(|f| owner(f.creator())).max_by_key(|id| names.iter().position(|(i, _)| i == id));
                    latest.map(|id| format!("Vertex of {}", fname(id))).unwrap_or_else(|| "Vertex".into())
                }
                Reference::Vertex(_) => "Vertex".into(),
                Reference::Body(b) => bodies.iter().find(|(i, _)| i == b).map(|(_, n)| n.clone()).unwrap_or_else(|| "Part".into()),
                // Live (qa5 P1-12, behaviours 208 / 2693): a sketch curve is
                // "Edge of Sketch N", a sketch point "Vertex of Sketch N".
                Reference::SketchEntity { sketch, entity } => match entity_kinds.get(&(*sketch, *entity)).copied() {
                    Some("point") => format!("Vertex of {}", fname(*sketch)),
                    _ => format!("Edge of {}", fname(*sketch)),
                },
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
        // Clicking the graphics area takes the keyboard focus from any text
        // field (a panel cell, which then commits, the feature-list filter, a
        // dialog value), so shortcuts like F or Shift+E don't type into it and
        // its caret stops blinking (each blink redrew the window), like
        // Onshape. An open feature dialog takes it (Enter, Esc, Tab stay its
        // own, as the assembly's mate dialog). Sketch mode does this itself
        // (pressed dimension labels keep their field).
        let press = matches!(ev, ViewportEvent::Click { .. } | ViewportEvent::ContextMenu { .. } | ViewportEvent::ToolDown { .. } | ViewportEvent::BoxSelect { .. });
        let target = match &self.edit {
            Some(e) => e.dialog.read(cx).focus_handle(cx),
            None => self.focus.clone(),
        };
        if press && (self.panel_focused.is_some() || (self.sketch.is_none() && !target.is_focused(window))) {
            window.focus(&target, cx);
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

    /// The edited pattern's instance points while its "Instances to skip"
    /// field is active (FeatureScript `TOGGLE_POINTS`).
    fn toggle_points(&self, cx: &App) -> Option<&[noshape_model::display::TogglePoint]> {
        let d = self.edit.as_ref()?.dialog.read(cx);
        d.toggle_points_field()?;
        Some(&self.display.feature_state(d.feature.id)?.toggle_points)
    }

    /// The instance point under `p` (viewport px), the seed's excepted.
    fn toggle_point_at(&self, p: glam::DVec2, cx: &App) -> Option<(i64, i64)> {
        let vp = self.viewport.read(cx);
        self.toggle_points(cx)?
            .iter()
            .filter(|t| !t.seed)
            .filter_map(|t| Some((t.index, vp.project(glam::DVec3::from_array(t.pos))?.distance(p))))
            .filter(|(_, d)| *d <= f64::from(TOGGLE_POINT_PX) / 2.0 + 1.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// Live (p1audit skip-02..04): with "Instances to skip" active, a point
    /// under the pointer turns blue and a click skips or keeps its instance.
    fn toggle_points_event(&mut self, ev: &ViewportEvent, cx: &mut Context<Self>) -> bool {
        if self.toggle_points(cx).is_none() {
            if self.ui.toggle_hover.take().is_some() {
                self.refresh_scene(false, cx);
            }
            return false;
        }
        match ev {
            ViewportEvent::PointerMove { position, .. } => {
                let h = self.toggle_point_at(*position, cx);
                if h != self.ui.toggle_hover {
                    self.ui.toggle_hover = h;
                    self.refresh_scene(false, cx);
                }
                false
            }
            ViewportEvent::Click { position, .. } => {
                let Some(i) = self.toggle_point_at(*position, cx) else { return false };
                if let Some(e) = &self.edit {
                    e.dialog.update(cx, |d, cx| d.toggle_instance(i, cx));
                }
                true
            }
            _ => false,
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
            // Hole locations: every point of the sketch (Onshape), shown as
            // one "Vertices of Sketch N" row (qa5 P1-8).
            dialog.update(cx, |d, _| d.grouped_sketches.insert(id));
            sketch_points(sk)
        } else if sketch.is_some() && filter.intersects(QueryFilter::SKETCH_EDGE) {
            // Curve fields (Rib's Sketch profiles, Sweep's path): the whole
            // sketch, shown as "Edges of Sketch 2" (live "Edges of Rib sketch").
            vec![Reference::Sketch(id)]
        } else if let Some(pl) = self.display.planes.iter().find(|p| p.reference == Reference::FeaturePlane(id)).filter(|_| filter.intersects(QueryFilter::PLANE)) {
            // A Plane feature's row fills a plane field (Sketch plane, Mirror
            // plane), like the default planes' rows (qa5 ADV1).
            vec![pl.reference.clone()]
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
        // Onshape's auto-dismissed message bubbles last 5000 ms.
        self.flash_for(status, text, Duration::from_secs(5), cx);
    }

    /// [`Self::flash`] for `time`.
    pub(super) fn flash_for(&mut self, status: FeatureStatus, text: String, time: Duration, cx: &mut Context<Self>) {
        let shown = (status, text);
        self.message = Some(shown.clone());
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(time).await;
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
        // Dragging a datum box (live psb/r-921).
        match ev {
            ViewportEvent::ToolDown { position, .. } if self.annotation_drag_start(*position, cx) => return,
            ViewportEvent::ToolDrag { position } if self.annotation_drag_move(*position, cx) => return,
            ViewportEvent::ToolUp { .. } if self.annotation_drag_end(cx) => return,
            // Dragging a selected plane's corner ring (plane_handles.rs).
            ViewportEvent::ToolDown { position, .. } if self.sketch.is_none() && self.plane_drag_start(*position, cx) => return,
            ViewportEvent::ToolDrag { position } if self.plane_drag_move(*position, window, cx) => return,
            ViewportEvent::ToolUp { .. } if self.plane_drag_end(window, cx) => return,
            _ => {}
        }
        // Screen-sized geometry built here follows the zoom: resize it when
        // the scale changes, without re-rendering (perf; the manipulators'
        // glyphs are built by the renderer per frame).
        if matches!(ev, ViewportEvent::CameraChanged) && self.sketch.is_none() {
            let wpp = {
                let v = vp.read(cx);
                v.camera.world_per_pixel(v.size().y.max(1.0))
            };
            if (wpp - self.ui.sized_for_wpp).abs() > wpp * 1e-6 {
                self.ui.sized_for_wpp = wpp;
                if self.edit.is_some() {
                    self.refresh_scene(false, cx);
                    self.sync_manipulator(cx);
                }
            }
        }
        match ev {
            ViewportEvent::PointerMove { position, .. } => {
                self.ui.pointer = Some(*position);
                self.select_other_pointer(*position, cx);
                // A plane's resize ring under the pointer: darker, nothing
                // pre-highlighted (live).
                if self.plane_ring_pointer(*position, cx) {
                    if self.ui.plane_ring_hover.is_some() {
                        self.ui.hover_pick = None;
                        self.update_hover(cx);
                    }
                    self.refresh_scene(false, cx);
                }
                // Near a hovered edge's midpoint the midpoint becomes (or
                // stops being) the target without a new pick (not while
                // Select other is open).
                if self.sketch.is_none() && self.ui.select_other.is_none() && self.hover.is_some() && (self.ui.midpoint_armed.is_some() || self.ui.hover_pick.as_ref().is_some_and(|t| matches!(t.kind, PickKind::Edge | PickKind::SketchCurve))) && self.update_hover(cx) {
                    self.refresh_scene(false, cx);
                }
                // Hovering a manipulator arrow outlines it (Onshape).
                let on_manip = vp.read(cx).manipulator_at(*position);
                if on_manip != self.ui.manip_hover {
                    self.ui.manip_hover = on_manip;
                    self.refresh_scene(false, cx);
                }
                // The section manipulator outlines the handle under the cursor.
                let sh = match self.current_manip(cx) {
                    Some(manipulators::Manip::Section(s)) => self.section_handle_at(&s, *position, cx),
                    _ => None,
                };
                if sh != self.ui.section_hover {
                    self.ui.section_hover = sh;
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
                // Select other takes the click (its entry or closing).
                if self.ui.select_other.is_some() {
                    self.select_other_click(*position, cx);
                    return;
                }
                // A click elsewhere closes the S toolbar.
                if self.ui.mini_toolbar.take().is_some() | self.ui.selected_folder.take().is_some() | self.ui.fresh_folder.take().is_some() {
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
        if self.sketch.is_none() && self.toggle_points_event(ev, cx) {
            return;
        }
        if self.sketch.is_none() && self.manipulator_event(ev, window, cx) {
            return;
        }
        match ev {
            // Picking under the pointer is off while Select other is open.
            ViewportEvent::Hover(_) if self.ui.select_other.is_some() => {}
            ViewportEvent::Hover(id) => {
                // Live: with a dialog field active, what it doesn't take isn't
                // highlighted; on a plane's resize ring nothing is.
                let id = id.filter(|_| self.ui.plane_ring_hover.is_none());
                self.ui.hover_pick = id.and_then(|id| self.built.as_ref()?.picks.get(id).filter(|t| self.field_takes(t, cx) != Some(false)).cloned());
                if self.update_hover(cx) {
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
            // The Datum dialog places the symbol on the clicked face.
            ViewportEvent::Click { position, .. } if self.ui.datum_edit.is_some() => {
                self.datum_pick(*position, cx);
            }
            // Mass properties' reference field takes a picked mate connector.
            ViewportEvent::Click { pick, .. } if self.show_mass && self.ui.mass_ref_active => {
                if let Some(t) = pick.and_then(|id| self.built.as_ref()?.picks.get(id).cloned()).filter(|t| t.kind == PickKind::Connector) {
                    self.ui.mass_reference = Some(t.reference);
                    cx.notify();
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
                // A click picks the hovered midpoint target (update_hover:
                // only once its edge is hovered, with a field that takes
                // points).
                let target = match &self.hover {
                    Some(r @ Reference::Implicit { on, inference: noshape_model::reference::Inference::Midpoint }) if self.sketch.is_none() => {
                        Some(scene_builder::PickTarget { reference: r.clone(), kind: PickKind::Vertex, anchor: self.edge_midpoint(on) })
                    }
                    _ => target,
                };
                match target {
                    // Double-clicking a sketch in the graphics area edits it.
                    Some(t) if self.edit.is_none() && self.is_double_click(&t.reference) && matches!(t.kind, PickKind::SketchCurve | PickKind::SketchPoint | PickKind::SketchRegion) => {
                        if let Reference::SketchEntity { sketch, .. } | Reference::SketchRegion { sketch, .. } = t.reference {
                            self.selection.clear();
                            self.edit_feature(sketch, window, cx);
                        }
                    }
                    Some(t) => {
                        // Only planes picked here drag their rings (live).
                        if t.kind == PickKind::Plane && !self.ui.planes_picked_in_graphics.contains(&t.reference) {
                            self.ui.planes_picked_in_graphics.push(t.reference.clone());
                        }
                        self.pick(t, cx)
                    }
                    // Clicking empty space clears the selection, but never a
                    // dialog's chips (live qa5 pass 7: neither a feature
                    // dialog's fields nor Mass properties' "Parts to measure"
                    // or Measure's entities, which show the selection).
                    // Shift+click on empty space keeps it (live "nav2 selection" (3)).
                    None if self.edit.is_none() && !self.show_mass && !self.ui.measure_pinned && !modifiers.shift => {
                        self.selection.clear();
                        self.selected_features.clear();
                        self.refresh_scene(false, cx);
                    }
                    None => {}
                }
                cx.notify();
            }
            ViewportEvent::BoxSelect { modifiers, crossing, rect, .. } => {
                // Ctrl+drag deselects (Onshape). The box reaches behind what
                // is in front (box_select.rs).
                let ids = self.box_ids(*rect, *crossing, cx);
                self.box_select(&ids, modifiers.control || modifiers.platform, cx);
            }
            ViewportEvent::ContextMenu { pick, position } => {
                // A datum symbol under the pointer has its own menu.
                if let Some(id) = self.ui.pointer.and_then(|p| self.annotation_at(p, cx)) {
                    self.open_annotation_menu(id, *position, window, cx);
                    return;
                }
                let target = pick.and_then(|id| self.built.as_ref()?.picks.get(id).cloned());
                self.open_context_menu(target, *position, window, cx);
            }
            _ => {}
        }
        let _ = window;
    }

    // ------------------------------------------------------------ editing

    /// A tool's shortcut. Live (ui 10-21 20:00): with a new dialog of the same
    /// tool already open it changes nothing; another open dialog is replaced.
    fn shortcut_feature(&mut self, type_id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.edit.as_ref().is_some_and(|e| e.is_new && e.dialog.read(cx).feature.type_id == type_id) {
            return;
        }
        self.start_feature(type_id, window, cx);
    }

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
                // helixEditLogic (live L34): the tab follows the preselection.
                if type_id == "helix"
                    && let Some(t) = self.helix_type_for(&sel)
                {
                    feature.params.set("axisType", ParamValue::Enum(t.into()));
                }
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
        // Live: a slow model shows "Opening <feature>…" under the spinner.
        self.pending_opening(feature.name.clone());
        self.open_dialog(feature, index, false, window, cx);
    }

    /// Tabs a FeatureScript reference parameter can pick (files, other
    /// Part Studios with their parts) and the document name.
    fn set_reference_sources(&self, dialog: &Entity<FeatureDialog>, cx: &mut Context<Self>) {
        use noshape_model::regen::derived::DerivedProvider as _;
        let provider = self.doc.read(cx).derived_provider();
        let doc = self.doc.read(cx).session.doc.clone();
        let name = self.doc.read(cx).name();
        // Parts as the source names them now (qa5 DER9: a part renamed
        // "Block" there reads "Block" here too).
        let renamed = |id: &str, mut v: Vec<(noshape_model::BodyId, String)>| {
            if let Some(noshape_model::document::TabContent::PartStudio(ps)) = doc.tabs.iter().find(|t| t.id.to_string() == id).map(|t| &t.content) {
                for (b, n) in v.iter_mut() {
                    if let Some(name) = ps.part_properties.get(b).and_then(|p| p.name.clone()) {
                        *n = name;
                    }
                }
            }
            v
        };
        let sources = crate::fs_reference::sources(&doc, &self.tab.to_string(), |id| provider.context(id).map(|c| renamed(id, crate::fs_reference::parts_and_sketches(&c))).unwrap_or_default());
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
        let feature_id = feature.id;
        // The edited row shows as being edited, not selected.
        self.selected_features.clear();
        let units = self.doc.read(cx).session.doc.settings.units;
        let labeler = self.labeler(cx);
        let is_derived = feature.type_id == "importDerived";
        let dialog = cx.new(|cx| {
            let mut d = FeatureDialog::new(feature, units, labeler, cx);
            if is_new {
                d.release_prefilled_single_pick();
            } else {
                d.focus_for_edit();
            }
            d
        });
        if is_derived {
            let provider = self.doc.read(cx).derived_provider();
            let doc = self.doc.read(cx).session.doc.clone();
            let me = self.tab.to_string();
            let sources: Vec<crate::feature_dialog::DerivedSource> = crate::derived::DocStudios::sources(&doc)
                .into_iter()
                .filter(|(id, _)| *id != me)
                .map(|(id, _)| {
                    use noshape_model::regen::derived::DerivedProvider as _;
                    // Parts and sketches (a sketch alone can be derived, live "psb (10-26 a)" D3).
                    let parts = provider.context(&id).map(|c| crate::fs_reference::parts_and_sketches(&c)).unwrap_or_default();
                    crate::feature_dialog::DerivedSource { tab: id, parts }
                })
                .collect();
            dialog.update(cx, |d, _| d.derived_sources = sources);
        }
        // Derived's Part Studio field names its studio from these too (qa5
        // DOC3: a pasted part opened with the empty "Select Part Studio…").
        if is_derived || dialog.read(cx).ty.params.iter().any(|s| matches!(s.kind, noshape_model::params::ParamKind::Reference { .. })) {
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
            // Live (behaviours "psui (10-27, PANEL-BLEED)"): an analysis panel
            // opened over the dialog closes on the first Esc, the dialog on
            // the second.
            DialogEvent::Cancel => {
                if !this.close_analysis_tool(cx) {
                    this.cancel_edit(window, cx);
                }
            }
            DialogEvent::QueryFocused => this.refresh_scene(false, cx),
            // Live (psb 10-19 h): Tab past the dialog's last control goes to
            // the navbar; Shift+Tab before its first one to <body> (here the
            // Part Studio itself, so shortcuts keep working).
            DialogEvent::LeaveFocus { forward: true } => window.dispatch_action(Box::new(crate::actions::FocusNavbar), cx),
            DialogEvent::LeaveFocus { forward: false } => window.focus(&this.focus, cx),
            DialogEvent::CreateSelection(field) => this.open_create_selection_for_field(field.clone(), cx),
            DialogEvent::CreateVariable(v) => this.create_variable_from_field(v, cx),
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
            DialogEvent::ConfigureParam(param) => this.configure_dialog_param(param, true, cx),
            DialogEvent::UnconfigureParam(param) => this.configure_dialog_param(param, false, cx),
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
        // Only variables that exist where this feature evaluates (Onshape),
        // configuration variables included.
        let vars = self.scope_variables(Some(index), cx);
        let later = self.studio(cx).is_some_and(|s| !is_new && index + 1 < s.features.len());
        // Missing references: their creating feature is gone or suppressed.
        let missing: Arc<dyn Fn(&Reference) -> bool + Send + Sync> = match self.studio(cx) {
            Some(s) => Arc::new(noshape_ops::studio::missing_reference(&self.doc.read(cx).session.doc, s)),
            None => Arc::new(|_: &Reference| false),
        };
        let (configured, configurable_inputs) = self.configured_of(feature_id, cx);
        let region_counts: std::collections::HashMap<FeatureId, usize> = self.display.sketches.iter().map(|s| (s.feature, s.regions.len())).collect();
        let vertex_sets: std::collections::HashMap<FeatureId, Vec<Reference>> = self.display.sketches.iter().map(|s| (s.feature, sketch_points(s))).collect();
        let studio_view = cx.entity().downgrade();
        // Options ticked in an earlier, cancelled sketch dialog don't carry over.
        self.sketch_pending_options = None;
        dialog.update(cx, |d, cx| {
            if d.feature.type_id == "sketch" {
                d.extra = Some(Self::sketch_dialog_extra(studio_view));
            }
            d.missing = missing;
            d.configured = configured;
            d.configurable_inputs = configurable_inputs;
            d.sketch_region_count = Arc::new(move |id| region_counts.get(&id).copied().unwrap_or(0));
            // A reopened feature holding all of a sketch's vertices shows
            // them as "Vertices of Sketch N", as picked from its row.
            let picked = d.selected_refs();
            d.grouped_sketches = vertex_sets.iter().filter(|(_, v)| v.len() >= 2 && v.iter().all(|r| picked.contains(r))).map(|(id, _)| *id).collect();
            d.sketch_vertices = Arc::new(move |id| vertex_sets.get(&id).cloned().unwrap_or_default());
            d.width = crate::preferences_window::ListPrefs::load().feature_dialog_widths.get(&d.feature.type_id).map(|w| px(*w as f32));
            d.variables = vars;
            d.final_available = later;
            window.focus(&d.focus_handle(cx), cx);
            d.focus_first_field(window, cx);
        });
        let history = DraftHistory::new(&dialog.read(cx).feature);
        self.ui.orient_pending = self.ui.orient_on_edit && dialog.read(cx).feature.type_id == "sketch";
        self.ui.preview_opacity = crate::feature_dialog::PREVIEW_REST;
        self.ui.preview_tone = matches!(dialog.read(cx).feature.type_id.as_str(), "extrude" | "revolve" | "sweep" | "loft");
        self.ui.preview_before = None;
        self.ui.before_context = None;
        // A new feature's first change runs std's editing logic with an
        // empty old definition; an edited one starts from its values.
        self.ui.std_edit_old = (!is_new).then(|| dialog.read(cx).feature.params.clone());
        self.edit = Some(EditSession { dialog, index, is_new, _sub: sub, history });
        // Live ("Panels vs feature dialogs"): opening a feature dialog closes
        // the tool panels (Mass and section properties, Named views) for good;
        // Isolate stays, under the dialog.
        if self.show_mass {
            self.set_show_mass(false, cx);
        }
        self.named_views = None;
        self.auto_operation(cx);
        self.schedule_regen(window, cx);
        self.schedule_preview_before(window, cx);
        cx.notify();
    }

    /// Accepts the open feature dialog or sketch, if any (a document-level
    /// panel such as Workspace units takes its place).
    pub(crate) fn accept_open_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.edit.is_some() {
            self.accept_edit(false, window, cx);
        }
    }

    fn accept_edit(&mut self, repeat: bool, window: &mut Window, cx: &mut Context<Self>) {
        // Live (ui (10-26 c), VAR-INVALID): a value of the wrong type keeps
        // the dialog open; nothing is created.
        if self.edit.as_ref().is_some_and(|e| e.dialog.read(cx).value_error(cx).is_some()) {
            return;
        }
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
            let desc = if is_new { noshape_ops::studio::insert_description(&name) } else { noshape_ops::studio::edit_description(&name) };
            doc.edit_studio(self.tab, &desc, cx, |ps| {
                if is_new {
                    noshape_ops::studio::insert_feature(ps, feature, index);
                } else {
                    noshape_ops::studio::edit_feature(ps, index, feature, &config_values);
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
        let desc = self.studio(cx).map(|s| noshape_ops::studio::delete_description(s, &ids)).unwrap_or_else(|| noshape_ops::studio::delete_description(&Default::default(), &ids));
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, &desc, cx, |ps| noshape_ops::studio::delete_features(ps, &ids));
        });
        self.selected_features.clear();
    }

    fn set_suppressed(&mut self, ids: Vec<FeatureId>, suppressed: bool, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, noshape_ops::studio::suppress_description(suppressed), cx, |ps| noshape_ops::studio::set_suppressed(ps, &ids, suppressed));
        });
    }

    /// Feature-list autoscroll while a row or the rollback bar is dragged
    /// (live "psb (10-24 k)"): the speed for the pointer's height, kept while
    /// the pointer rests; one frame loop scrolls until the drag ends or the
    /// pointer leaves the edge zones, stopping at the list's ends.
    fn list_autoscroll(&mut self, y: Pixels, list: Bounds<Pixels>, cx: &mut Context<Self>) {
        self.ui.list_autoscroll = behaviors::list_autoscroll_speed(f32::from(y), f32::from(list.top()), f32::from(list.bottom()));
        if self.ui.list_autoscroll == 0.0 || self.ui.list_autoscrolling {
            return;
        }
        self.ui.list_autoscrolling = true;
        cx.spawn(async move |this, cx| {
            let mut last = std::time::Instant::now();
            loop {
                cx.background_executor().timer(Duration::from_millis(16)).await;
                let now = std::time::Instant::now();
                let dt = (now - last).as_secs_f32();
                last = now;
                let go = this.update(cx, |this, cx| {
                    let v = this.ui.list_autoscroll;
                    if v == 0.0 || !cx.has_active_drag() {
                        this.ui.list_autoscroll = 0.0;
                        this.ui.list_autoscrolling = false;
                        return false;
                    }
                    let s = &this.ui.list_scroll;
                    let mut o = s.offset();
                    let max = f32::from(s.max_offset().y).max(0.0);
                    let y = (f32::from(o.y) - v * dt).clamp(-max, 0.0);
                    if y != f32::from(o.y) {
                        o.y = px(y);
                        s.set_offset(o);
                        cx.notify();
                    }
                    true
                });
                if !matches!(go, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }

    fn set_rollback(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, noshape_ops::studio::ROLLBACK_DESCRIPTION, cx, |ps| noshape_ops::studio::set_rollback(ps, index));
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
        if !ids.is_empty() {
            self.delete_features(ids, cx);
            return;
        }
        // Live (behaviours "Delete key", ui 10-22 a): selected parts open the
        // Delete part dialog with them prefilled; selected faces or edges
        // delete the features that made them, at once (Ctrl+Z restores).
        if self.selection.iter().any(|r| matches!(r, Reference::Body(_))) {
            self.selection.retain(|r| matches!(r, Reference::Body(_)));
            self.start_feature("deleteBodies", window, cx);
            return;
        }
        let mut creators: Vec<FeatureId> = Vec::new();
        for r in self.selection.iter().filter(|r| matches!(r, Reference::Face(_) | Reference::Edge(_))) {
            if let Some(f) = self.creating_feature(r).filter(|f| !creators.contains(f)) {
                creators.push(f);
            }
        }
        if !creators.is_empty() {
            self.selection.clear();
            self.delete_features(creators, cx);
            self.refresh_scene(false, cx);
            cx.notify();
        }
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
            let fwd = noshape_scene::camera_ops::normal_forward(cur, f.normal);
            self.viewport.update(cx, |vp, cx| vp.animate_to_direction(fwd, f.y, cx));
        }
    }

    /// "View normal to section plane" (live ui-com/03: the face menu while
    /// a section view is on): normal to the first section plane.
    pub(super) fn view_normal_to_section(&mut self, cx: &mut Context<Self>) {
        let Some(pl) = self.ui.section.first() else { return };
        let n = if pl.flipped { -pl.normal } else { pl.normal };
        let up = n.any_orthonormal_vector();
        self.push_camera_history(cx);
        let cur = self.viewport.read(cx).camera.forward();
        let fwd = noshape_scene::camera_ops::normal_forward(cur, n);
        self.viewport.update(cx, |vp, cx| vp.animate_to_direction(fwd, up, cx));
    }

    // ------------------------------------------------------------ render pieces

    /// Live's toolbar fit (client `Toolbar` component, woolsthorpe
    /// @14754280; docs/handoff/theme-layout.md "Toolbar collapse"): the
    /// longest run of tool sets from the left that fits stays expanded, every
    /// later one collapses into one split tool (no priorities, no
    /// hysteresis). Widths are Onshape's CSS, not ours, so the breakpoints
    /// are live's: a tool 34 px (32 + 1 px margins), a split tool 52, the
    /// "Sketch" label tool 85, 4 px per tool set (separator + margins); a
    /// collapsed set 56 (38 for a single tool); Undo / Redo 70, "Search
    /// tools…" 173, 7 px at the end; room = the window minus the 40 px rail
    /// column. Returns the index in `order` from which sets collapse.
    fn toolbar_collapse_from(order: &[usize], removed: &[String], custom: usize, search_hidden: bool, viewport_w: f32) -> usize {
        let is_removed = |id: &str| removed.iter().any(|r| r == id);
        let widths: Vec<(f32, f32)> = order
            .iter()
            .map(|&gi| {
                if gi == ADD_CUSTOM_SET {
                    let w = if is_removed(ADD_CUSTOM[0]) { 0. } else { 38. } + if custom > 0 { 56. } else { 0. };
                    return (w, w);
                }
                let items: Vec<Vec<&str>> = TOOLBAR[gi].iter().map(|it| it.iter().copied().filter(|id| !is_removed(id)).collect::<Vec<_>>()).filter(|v| !v.is_empty()).collect();
                if items.is_empty() {
                    return (0., 0.);
                }
                let expanded = 4. + items.iter().map(|it| if it[0] == "sketch" { 85. } else if it.len() > 1 { 52. } else { 34. }).sum::<f32>();
                let tools: usize = items.iter().map(Vec::len).sum();
                (expanded, 4. + if tools > 1 { 52. } else { 34. })
            })
            .collect();
        let fixed = 70. + if search_hidden { 0. } else { 173. } + 7.;
        let room = viewport_w - 40.;
        (0..=widths.len())
            .rev()
            .find(|&k| fixed + widths[..k].iter().map(|w| w.0).sum::<f32>() + widths[k..].iter().map(|w| w.1).sum::<f32>() <= room)
            .unwrap_or(0)
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
        // Tool sets collapse from the right as the window narrows
        // (`toolbar_collapse_from`); the bar clips them when narrower still.
        let mut bar = h_flex().flex_none().relative().gap(px(2.)).child(crate::ui::undo_redo(&self.doc, cx)).child(crate::ui::tool_separator());
        let customizing = self.ui.tb_custom.is_some();
        let hidden = self.ui.toolbar.hide_search;
        // Customize toolbar… edits a draft: the toolbar shows it.
        let (order, removed) = match &self.ui.tb_custom {
            Some(c) => (c.order.clone(), c.removed.clone()),
            None => (crate::toolbar_customize::full_order(&self.ui.toolbar.group_order, TOOLSETS), self.ui.toolbar.removed.clone()),
        };
        let is_removed = |id: &str| removed.iter().any(|r| r == id);
        let custom = self.doc.read(cx).custom_features().feature_types();
        let collapse_from = if customizing { usize::MAX } else { Self::toolbar_collapse_from(&order, &removed, custom.len(), hidden, viewport_w) };
        fn field(v: &mut PartStudioView) -> Option<&mut crate::toolbar_customize::Customize> {
            v.ui.tb_custom.as_mut()
        }
        let mut first = true;
        let mut custom_shown = false;
        for (pos, gi) in order.into_iter().enumerate() {
            let collapsed = pos >= collapse_from;
            // ⊞ "Add custom features" is a set of its own (live 341); the
            // document's custom features follow it, after a separator (live
            // part-studio/495: ⊞ at x 1637, the {cube} ▾ at 1675).
            if gi == ADD_CUSTOM_SET {
                if !is_removed(ADD_CUSTOM[0]) {
                    let btn = self.add_custom_features_button(custom.clone(), cx);
                    let el = if customizing {
                        let t = crate::toolbar_customize::tool(btn, &ADD_CUSTOM, crate::assets::icon_path("add-custom-features"), cx);
                        crate::toolbar_customize::toolset(gi, t, true, field, cx)
                    } else {
                        btn
                    };
                    bar = bar.child(crate::ui::tool_separator()).child(el);
                    first = false;
                }
                if !custom.is_empty() {
                    bar = bar.child(crate::ui::tool_separator()).child(self.custom_tools(custom.clone(), cx));
                    custom_shown = true;
                }
                continue;
            }
            let group = TOOLBAR[gi];
            if collapsed && gi > 0 {
                let items = collapsed_group(gi);
                let live: Vec<&'static str> = items.iter().copied().filter(|id| !is_removed(id)).collect();
                let Some(rep) = live.first().and_then(|id| features::get(id)) else { continue };
                let key = COLLAPSED_KEY[gi];
                let face = self.ui.toolbar_faces.get(key).and_then(|id| features::get(id)).unwrap_or(rep);
                if !first {
                    bar = bar.child(crate::ui::tool_separator());
                }
                bar = bar.child(self.tool_button(face, items, key, cx));
                first = false;
                continue;
            }
            let mut buttons: Vec<AnyElement> = Vec::new();
            for items in group.iter() {
                let Some(primary) = items.iter().find(|id| !is_removed(id)).and_then(|id| features::get(id)) else { continue };
                // Onshape: the tool last picked from a dropdown becomes the
                // group's face for the rest of the session.
                let key = items[0];
                let face = self.ui.toolbar_faces.get(key).and_then(|id| features::get(id)).unwrap_or(primary);
                let btn = self.tool_button(face, items, key, cx);
                // Customize: the tool drags with its whole dropdown (live 345).
                buttons.push(if customizing { crate::toolbar_customize::tool(btn, items, feature_icon(face.id), cx) } else { btn });
            }
            if buttons.is_empty() {
                continue;
            }
            if !first {
                bar = bar.child(crate::ui::tool_separator());
            }
            first = false;
            bar = if customizing {
                bar.child(crate::toolbar_customize::toolset(gi, h_flex().gap(px(2.)).children(buttons), true, field, cx))
            } else {
                bar.children(buttons)
            };
        }
        // ⊞ removed: the custom features still end the toolbar.
        if !custom.is_empty() && !custom_shown {
            bar = bar.child(crate::ui::tool_separator()).child(self.custom_tools(custom, cx));
        }
        let view = cx.entity();
        crate::ui::toolbar()
            .id("part-studio-toolbar")
            // The slice's width, not the tools': they are clipped (and the
            // fit measures this row).
            .w_full()
            .min_w_0()
            .relative()
            // Right-click the toolbar (live 331z): Customize toolbar… · Hide /
            // Show "Search tools".
            .os_context_menu(move |m, _, _| {
                let (v1, v2) = (view.clone(), view.clone());
                m.list_no_icons()
                    .item(crate::os_menu::OsMenuItem::new("Customize toolbar…").on_click(move |_, window, cx| v1.update(cx, |this, cx| this.start_toolbar_customize(window, cx))))
                    .item(crate::os_menu::OsMenuItem::new(if hidden { "Show \"Search tools\"" } else { "Hide \"Search tools\"" }).on_click(move |_, _, cx| {
                        v2.update(cx, |this, cx| {
                            this.ui.toolbar.hide_search = !this.ui.toolbar.hide_search;
                            this.ui.toolbar.save();
                            cx.notify();
                        })
                    }))
            })
            .child(h_flex().flex_initial().min_w_0().overflow_hidden().child(bar))
            .when(!hidden, |d| {
                d.child(
                    crate::ui::search_tools_toggle("tb-search", self.palette.is_some())
                        .flex_none()
                        .relative()
                        .ml(px(10.))
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.toggle_palette(window, cx))),
                )
            })
            .children(self.render_configure_banner(cx))
            .into_any_element()
    }

    /// Configure features mode (live psb 10-21 g): the toolbar greys out
    /// under a white veil and a yellow bar centred over it reads "Select
    /// sketches or features to configure" (with a feature open: "Select
    /// parameters to configure") with [Done].
    fn render_configure_banner(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        if self.ui.configure_pick.is_none() {
            return Vec::new();
        }
        let text = if self.edit.is_some() { "Select parameters to configure" } else { "Select sketches or features to configure" };
        let veil = div().id("cfg-mode-veil").absolute().top_0().left_0().size_full().bg(gpui_kit::rgba(0xffffffbf)).occlude();
        let done = div()
            .id("cfg-mode-done")
            .h(px(22.))
            .w(px(52.))
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui_kit::rgb(0xffffff))
            .border_1()
            .border_color(gpui_kit::rgb(0xaaaaaa))
            .rounded(px(3.5))
            .cursor_pointer()
            .text_size(px(13.))
            .text_color(p().text)
            .child("Done")
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                // Done leaves the mode and closes the feature dialog.
                this.ui.configure_pick = None;
                this.cancel_edit(window, cx);
                cx.notify();
            }));
        let bar = h_flex()
            .h(px(28.))
            .w(px(352.))
            .pt(px(3.))
            .pr(px(3.))
            .pl(px(10.))
            .bg(gpui_kit::rgb(0xfab601))
            .text_size(px(13.))
            .text_color(gpui_kit::rgb(0x333333))
            .child(div().flex_1().child(text))
            .child(done);
        vec![veil.into_any_element(), div().absolute().top_0().left_0().size_full().flex().justify_center().child(bar).into_any_element()]
    }

    fn start_toolbar_customize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (order, removed) = (self.ui.toolbar.group_order.clone(), self.ui.toolbar.removed.clone());
        self.ui.tb_custom = Some(crate::toolbar_customize::Customize::new(TOOLSETS, &order, &removed, window, cx));
        cx.notify();
    }

    /// Customize toolbar's "Tools" panel, under the toolbar at the left.
    fn render_toolbar_customize_panel(&mut self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        static TOOLS: std::sync::OnceLock<Vec<crate::toolbar_customize::PanelTool>> = std::sync::OnceLock::new();
        let tools = TOOLS.get_or_init(|| {
            let mut out: Vec<crate::toolbar_customize::PanelTool> = Vec::new();
            for group in TOOLBAR {
                for items in group.iter() {
                    for id in items.iter() {
                        let Some(t) = features::get(id) else { continue };
                        out.push(crate::toolbar_customize::PanelTool { ids: std::slice::from_ref(id), label: t.name.into(), icon: feature_icon(id) });
                    }
                }
            }
            out.push(crate::toolbar_customize::PanelTool { ids: &ADD_CUSTOM, label: "Add custom features".into(), icon: crate::assets::icon_path("add-custom-features") });
            out
        });
        let panel = crate::toolbar_customize::panel(
            self,
            tools,
            |v: &mut Self| v.ui.tb_custom.as_mut(),
            |v: &mut Self, saved, cx| {
                if let Some((order, removed)) = saved {
                    v.ui.toolbar.group_order = order;
                    v.ui.toolbar.removed = removed;
                    v.ui.toolbar.save();
                }
                v.ui.tb_custom = None;
                cx.notify();
            },
            window,
            cx,
        )?;
        // Live (dom): x 40 (the studio's left edge), 10 px under the toolbar
        // (the sleds hang there).
        Some(div().absolute().top(px(crate::ui::TOOLBAR_H + 10.)).left_0().child(panel).into_any_element())
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
            .text_color(gpui_kit::rgb_static(0xffffff))
            .child(format!("Return to {branch}"))
            .on_click(move |_, _, cx| {
                doc.update(cx, |d, cx| {
                    if d.session.view_microversion(None).is_ok() {
                        d.reloaded(cx);
                    }
                })
            });
        let help = div().size(px(16.)).rounded_full().flex().items_center().justify_center().text_size(px(12.)).font_weight(FontWeight::BOLD).child(crate::ui::glyph("help-dot", 17.8, crate::theme::help_dot(0x999999)));
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

    /// The workspace's custom features: one {cube} ▾ dropdown after ⊞ (live
    /// part-studio/495; behaviours.md: "Custom features in this workspace"),
    /// 52 px like a split tool.
    fn custom_tools(&mut self, custom: Vec<&'static FeatureType>, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity();
        h_flex()
            .flex_none()
            .child(
                crate::ui::tool_button("tb-custom-features", "custom-feature", false, cx)
                    .w(px(crate::ui::TOOL_CELL + crate::ui::TOOL_CARET_W))
                    .child(crate::ui::glyph("tool-caret", 10., p().text))
                    .tooltip("Custom features in this workspace")
                    // Live (part-studio/1251–1252, qa5 FS4): no heading; each
                    // entry has its initials icon ("Qc") and the name, with
                    // the name as a side tooltip that expands with "Feature
                    // from current workspace".
                    .dropdown_menu(move |mut menu, _, _| {
                        // Live (part-studio/1252): 178 px at least.
                        menu = menu.min_w(px(178.));
                        for t in &custom {
                            let v = view.clone();
                            let (id, name): (&'static str, &'static str) = (t.id, t.name);
                            menu = menu.item(
                                PopupMenuItem::element(move |_, _| {
                                    crate::ui::with_side_tooltip_expanded(h_flex().id(SharedString::from(format!("cf-{id}"))).flex_1().gap(px(6.)).child(crate::ui::initials_icon(name)).child(name), name, "Feature from current workspace")
                                })
                                .on_click(move |_, window, cx| {
                                    v.update(cx, |this, cx| this.start_feature(id, window, cx));
                                }),
                            );
                        }
                        menu
                    }),
            )
            .into_any_element()
    }

    /// The features list's rows, in their own cached slice (see `PsSlices`);
    /// the head is `render_feature_list_head`.
    fn render_feature_list(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        // The model is only borrowed (not cloned): walk it into what the rows
        // are, then build them.
        let Some(studio) = self.studio(cx) else { return div().into_any_element() };
        let n_features = studio.features.len();
        let rollback = studio.rollback_index();
        // Editing a feature rolls the model to just after it: the rows
        // below show rolled back (Onshape), unless "Final" is on.
        let rollback = match &self.edit {
            Some(e) if !e.is_new && !e.dialog.read(cx).show_final => rollback.min(e.index + 1),
            _ => rollback,
        };
        let filtered = self.filtered(cx);
        let hidden_defaults = studio.hidden_defaults.clone();
        let editing_id = self.edit.as_ref().map(|e| e.dialog.read(cx).feature.id);
        let editing_new_at = self.edit.as_ref().filter(|e| e.is_new).map(|e| e.index);
        // The Folder name popup sits beside the first selected feature, or
        // the rollback bar (live 586, 589).
        let prompt_anchor = self.folder_prompt.as_ref().and_then(|(_, t, _)| match t {
            FolderTarget::Features(ids) => Some(ids.first().copied()),
            FolderTarget::Folder(_) => None,
        });
        // Wrapping a folder: the popup sits under the folder's row.
        let prompt_folder = self.folder_prompt.as_ref().and_then(|(_, t, _)| match t {
            FolderTarget::Folder(f) => Some(*f),
            FolderTarget::Features(_) => None,
        });
        let mut items: Vec<ListItem> = Vec::new();
        let mut open_folder: Vec<u32> = Vec::new();
        if filtered.is_none() {
            items.extend(studio.empty_folders_after(None).into_iter().map(|fd| ListItem::Folder(0., fd.clone())));
        }
        // Filtered (live part-studio/564): each run of hidden features is
        // one "..." row and the rollback bar keeps its place.
        let mut in_hidden_run = false;
        for (i, f) in studio.features.iter().enumerate() {
            if editing_new_at == Some(i) {
                items.push(ListItem::Placeholder);
            }
            if i == rollback {
                in_hidden_run = false;
                items.push(ListItem::Rollback(i));
            }
            if filtered.as_ref().is_some_and(|set| !set.contains(&f.id)) {
                if !std::mem::replace(&mut in_hidden_run, true) {
                    items.push(ListItem::Ellipsis);
                }
                continue;
            }
            in_hidden_run = false;
            // Nested folders: emit headers for folders newly entered, indent
            // by depth, and hide rows under a collapsed ancestor.
            let chain = studio.folder_chain(f.folder);
            let common = open_folder.iter().zip(&chain).take_while(|(a, b)| a == b).count();
            for (depth, fid) in chain.iter().enumerate().skip(common) {
                let ancestors_open = chain[..depth].iter().all(|a| studio.folder(*a).is_some_and(|fd| !self.folder_collapsed(fd)));
                if let (Some(fd), true) = (studio.folder(*fid), ancestors_open || filtered.is_some()) {
                    items.push(ListItem::Folder(FOLDER_INDENT * depth as f32, fd.clone()));
                    if prompt_folder == Some(fd.id) {
                        items.push(ListItem::Prompt);
                    }
                }
            }
            open_folder = chain.clone();
            let collapsed = chain.iter().any(|c| studio.folder(*c).is_some_and(|fd| self.folder_collapsed(fd)));
            if collapsed && editing_id != Some(f.id) && filtered.is_none() {
                continue;
            }
            items.push(ListItem::Feature(FOLDER_INDENT * chain.len() as f32, f.id));
            // Live (qa5 derived, part-studio/1582): an open Derived row lists
            // the sketches it brought in.
            if self.ui.derived_open.contains(&f.id) {
                // Live (ui 10-22 b): one row per location's copy ("Sketch 1" twice).
                for (k, (_, sk)) in self.display.derived_sketches.iter().filter(|(d, _)| *d == f.id).enumerate() {
                    items.push(ListItem::DerivedSketch(FOLDER_INDENT * chain.len() as f32, *sk, k));
                }
            }
            if prompt_anchor == Some(Some(f.id)) {
                items.push(ListItem::Prompt);
            }
            if filtered.is_none() {
                items.extend(studio.empty_folders_after(Some(f.id)).into_iter().map(|fd| ListItem::Folder(0., fd.clone())));
            }
        }
        if editing_new_at.is_some_and(|i| i >= n_features) {
            items.push(ListItem::Placeholder);
        }
        if rollback >= n_features {
            items.push(ListItem::Rollback(n_features));
        }
        if prompt_anchor == Some(None) {
            items.push(ListItem::Prompt);
        }
        let (appearances, first_config_input) = match self.studio(cx) {
            Some(s) => (
                s.appearances
                    .iter()
                    .filter_map(|a| match a.target {
                        noshape_model::partstudio::AppearanceTarget::Feature(id) => Some(id),
                        _ => None,
                    })
                    .collect(),
                s.configuration.inputs.first().map(|i| i.id.clone()),
            ),
            None => Default::default(),
        };
        let d = &self.display;
        self.row_ctx = RowCtx {
            rollback,
            editing_id,
            related: self.related_features(cx),
            // A Derived's bodies keep their source's face names: it makes the
            // bodies it creates (live qa5 DER14: "Add appearance to feature…").
            makes_faces: d.bodies.iter().flat_map(|b| b.face_refs.iter().filter_map(|r| r.depends_on()).chain([b.id.feature])).collect(),
            sketches: d.sketches.iter().map(|s| s.feature).collect(),
            planes: d.planes.iter().filter_map(|p| match p.reference {
                Reference::FeaturePlane(id) => Some(id),
                _ => None,
            }).collect(),
            appearances,
            first_config_input,
        };
        self.filter_rows.clone_from(&filtered);
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
            let hidden = hidden_defaults.contains(&r);
            let dimmed = default_row_dimmed(hidden, self.ui.isolate.as_ref());
            let r2 = r.clone();
            let r3 = r.clone();
            let icon = if label == "Origin" { "origin" } else { "plane" };
            let view = cx.entity();
            let name = self.default_name(&r, cx);
            let renaming = self.ui.dg_renaming.as_ref().filter(|(x, ..)| *x == r).map(|(_, st, _)| st.clone());
            dg_rows.push(
                // Live (part-studio/16): the hover band spans the list (x 40
                // to 239); the indent is inside the row.
                crate::ui::with_user_side_tooltip(crate::ui::tree_row(SharedString::from(format!("dg-{label}")), selected), name.clone())
                    .mr(px(5.))
                    .pl(px(23.))
                    // Live: the eye's center 19 px from the row's end.
                    .pr(px(11.))
                    .group("dg-row")
                    // Onshape: hidden default geometry is listed in gray, and
                    // so is all of it while parts are isolated (qa5 KB3).
                    .when(dimmed, |d| d.text_color(pal.text_faint))
                    .child(crate::ui::tree_icon(icon, dimmed, cx))
                    .child(match renaming {
                        Some(st) => div().flex_1().child(gpui_kit::component::input::Input::new(&st).xsmall()).into_any_element(),
                        None => div().flex_1().truncate().child(crate::ui::user_text(name)).into_any_element(),
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
                        // So does an open analysis tool (Draft analysis's Mold
                        // split direction, qa5 ADV1).
                        if this.edit.is_none() && this.ui.analysis_tool.is_some() && this.analysis_pick(&r3, cx) {
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
                                .item(crate::os_menu::OsMenuItem::new("New sketch…").icon_name("sketch").on_click(move |_, w, cx| {
                                    v1.update(cx, |this, cx| {
                                        this.selection = vec![r1.clone()];
                                        this.start_feature("sketch", w, cx);
                                    })
                                }))
                                .item(crate::os_menu::OsMenuItem::new("Offset plane…").icon_name("plane").on_click(move |_, w, cx| {
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
                        m = m.item(crate::os_menu::OsMenuItem::new(if hidden { "Show" } else { "Hide" }).icon_name(if hidden { "eye" } else { "eye-off" }).on_click(move |_, _, cx| v2.update(cx, |this, cx| this.set_visibility(std::slice::from_ref(&r2), !hidden, cx))));
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
        // Filtered: the folder stays with one "..." child (live 564).
        if filtered.is_some() {
            dg_rows.push(div().h(px(26.)).flex_none().pl(px(23.)).flex().items_center().text_size(px(crate::ui::ROW_TEXT)).font_weight(FontWeight::BOLD).text_color(pal.text).child("...").into_any_element());
        }
        let dg_count = dg_rows.len();
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
                    // Live (qa4 round 5 s2 rollback follow-up (3)): the bar
                    // dropped over the group lands at the top slot.
                    .when(!open, |d| d.drag_over::<RollbackDrag>(|s, _, _, _| s.border_b_2().border_color(p().accent)))
                    .on_drop(cx.listener(|this, _: &RollbackDrag, _, cx| this.set_rollback(Some(0), cx)))
                    .into_any_element(),
            );
            if open {
                rows.push(
                    v_flex()
                        .relative()
                        .id("dg-rows")
                        .children(dg_rows)
                        .drag_over::<RollbackDrag>(|s, _, _, _| s.border_b_2().border_color(p().accent))
                        .on_drop(cx.listener(|this, _: &RollbackDrag, _, cx| this.set_rollback(Some(0), cx)))
                        // Live: the line ends in a 9 px tick; drawn over the rows.
                        .child(div().absolute().left(px(9.)).top(px(2.)).bottom(px(1.)).w(px(1.)).bg(pal.border))
                        .child(div().absolute().left(px(5.)).bottom(px(1.)).w(px(9.)).h(px(1.)).bg(pal.border))
                        .into_any_element(),
                );
            }
        }

        let ellipsis = || div().h(px(26.)).flex_none().pl(px(5.)).flex().items_center().text_size(px(crate::ui::ROW_TEXT)).font_weight(FontWeight::BOLD).text_color(pal.text).child("...").into_any_element();
        // Live (qa4 r5 s8 (6), ui 10-25 c): rows in a folder are indented
        // 19 px per level, with a 1 px #d7d7d7 guide under each enclosing
        // folder's chevron (9 px in from its row).
        let indent = |indent: f32, row: AnyElement| {
            if indent <= 0. {
                return row;
            }
            let levels = (indent / FOLDER_INDENT).round() as usize;
            let guides = (0..levels).map(|k| div().absolute().top_0().bottom_0().left(px(9. + FOLDER_INDENT * k as f32)).w(px(1.)).bg(gpui_kit::rgb(0xd7d7d7)));
            div().relative().pl(px(indent)).child(row).children(guides).into_any_element()
        };
        // Rows out of the list's view (with 2 rows to spare) are left out,
        // each run of them replaced by one empty block of their height: a
        // filter key that brings back every feature builds and lays out only
        // what shows (qa4). Feature, folder and "..." rows are all ROW_H
        // tall; the others are always drawn and only estimated here. A scroll
        // notifies this slice, which then draws the rows that came into view.
        let row_h = crate::ui::ROW_H;
        let shown = {
            let (b, offset) = (self.ui.list_scroll.bounds(), self.ui.list_scroll.offset());
            let spare = 2. * row_h;
            let top = -f32::from(offset.y) - spare;
            (f32::from(b.size.height) > 0.).then(|| (top, top + f32::from(b.size.height) + 2. * spare))
        };
        // The "Default geometry" row and its open rows come first.
        let mut y = if dg_count == 0 { 0. } else { row_h * (1 + if self.ui.default_geometry_collapsed { 0 } else { dg_count }) as f32 };
        let mut skipped = 0usize;
        for item in items {
            let h = match &item {
                ListItem::Rollback(_) => 13.,
                ListItem::Prompt => 60.,
                _ => row_h,
            };
            let off_view = matches!(item, ListItem::Ellipsis | ListItem::Folder(..) | ListItem::Feature(..) | ListItem::DerivedSketch(..)) && shown.is_some_and(|(top, bottom)| y + h < top || y > bottom);
            y += h;
            if off_view {
                skipped += 1;
                continue;
            }
            let run = std::mem::take(&mut skipped);
            if run > 0 {
                rows.push(div().h(px(row_h * run as f32)).flex_none().into_any_element());
            }
            // A row's place in the list, for its side tooltip.
            let slot = (rows.len(), item_indent(&item));
            rows.push(match item {
                ListItem::Placeholder => self.render_editing_placeholder(cx),
                ListItem::Rollback(i) => self.render_rollback_bar(i, window, cx),
                ListItem::Ellipsis => ellipsis(),
                ListItem::Folder(x, fd) => indent(x, self.render_folder_row(&fd, slot, cx)),
                ListItem::Feature(x, id) => indent(x, self.render_feature_row_view(id, slot, cx)),
                ListItem::DerivedSketch(x, id, k) => indent(x, self.render_derived_sketch_row(id, k, cx)),
                ListItem::Prompt => self.render_folder_prompt(cx),
            });
        }
        if skipped > 0 {
            rows.push(div().h(px(row_h * skipped as f32)).flex_none().into_any_element());
        }
        v_flex()
            .id("feature-list")
            .key_context("FeatureList")
            .track_focus(&self.ui.list_focus)
            .track_scroll(&self.ui.list_scroll)
            .size_full()
            .overflow_y_scroll()
            // Autoscroll (live: the bar and rows alike, also with the pointer
            // beside the panel; only its height counts).
            .on_drag_move(cx.listener(|this, e: &DragMoveEvent<RollbackDrag>, _, cx| this.list_autoscroll(e.event.position.y, e.bounds, cx)))
            .on_drag_move(cx.listener(|this, e: &DragMoveEvent<FeatureDrag>, _, cx| this.list_autoscroll(e.event.position.y, e.bounds, cx)))
            .children(rows)
            .child(
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
                    // Clicking empty list space clears the feature selection
                    // (and a new folder's highlight).
                    if !this.selected_features.is_empty() || this.ui.selected_folder.is_some() || this.ui.fresh_folder.take().is_some() {
                        this.selected_features.clear();
                        this.ui.selected_folder = None;
                        this.refresh_scene(false, cx);
                        cx.notify();
                    }
                })),
        )
        .into_any_element()
    }

    /// The parts list under the features, in its own cached slice (see
    /// `PsSlices`).
    fn render_parts_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
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
        let mut col = v_flex().size_full().id("parts-area").overflow_y_scroll();
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
        col.into_any_element()
    }

    /// The head of the features list: configurations, contexts, the filter
    /// box and the "Features (N)" row. Rendered with the studio, outside the
    /// list's cached slices, so the filter box's keystrokes and caret blinks
    /// leave the rows cached.
    fn render_feature_list_head(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let pal = p();
        let editing_id = self.edit.as_ref().map(|e| e.dialog.read(cx).feature.id);
        let Some(studio) = self.studio(cx) else { return Vec::new() };
        let filtered = self.filtered(cx);
        // A new feature whose dialog is open counts at once (live
        // sketch/501: "Features (5)" while Sketch 1 is being made).
        let n_features = listed_feature_count(studio.features.len(), self.edit.as_ref().is_some_and(|e| e.is_new));
        let failed = |id: noshape_model::FeatureId| self.display.feature_state(id).is_some_and(|s| s.status == FeatureStatus::Error);
        // The open dialog counts when it can't regenerate (a new feature
        // isn't in the list yet).
        let errors = studio.features.iter().filter(|f| failed(f.id)).count()
            + usize::from(self.edit_invalid(cx) && !editing_id.is_some_and(|id| studio.features.iter().any(|f| f.id == id && failed(f.id))));
        let dialog_open = self.edit.is_some();
        let viewing_version = self.doc.read(cx).session.viewing_version().is_some();
        let filter_input = self.ui.filter.as_ref().map(|(st, _)| st.clone());
        let filtered_count = filtered.as_ref().map(|f| f.len());
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
        let mut head: Vec<AnyElement> = config_box.into_iter().chain(contexts_section).collect();
        head.extend(filter_input.map(|st| {
            h_flex()
                .h(px(30.))
                .mt(px(7.))
                .flex_none()
                .pl(px(5.))
                .pr(px(8.))
                .gap(px(14.))
                // Live: the funnel is a button (`os-button os-clear`): pointer.
                // Filtering: a filled blue funnel (live 564).
                .child(div().flex_none().cursor_pointer().child(crate::assets::icon(if filtered_count.is_some() { "filter-active" } else { "filter" }, 20., cx)))
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
                            // The input draws its italic placeholder itself, so the caret
                            // starts on it. Live (dom/caret-inputs.json): border 1 + padding 5,
                            // the caret 6 px from the box's outer edge (1 + 1 + 4 here).
                            .child(gpui_kit::component::input::Input::new(&st).appearance(false).text_size(px(13.33)).pl(px(4.)))
                            // A grey ⊗ clears the filter (live 564).
                            .when(!empty, |d| {
                                d.child(
                                    div()
                                        .id("filter-clear")
                                        .size(px(16.))
                                        .mr(px(4.))
                                        .flex_none()
                                        .rounded_full()
                                        .bg(gpui_kit::rgb(0xdddddd))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .child(crate::ui::glyph("ui-clear", 7., gpui_kit::rgb_static(0xffffff)))
                                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.set_filter("", window, cx))),
                                )
                            }),
                    )
                })
                .into_any_element()
        }));
        head.push(
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
                // Filtered: "Features (k of N)", truncating (live 564).
                // With the error icon the title no longer fits the default
                // panel width: live shows "Features …" (Chrome keeps the space
                // before its ellipsis; part-studio/94, 570). A wider panel
                // shows it whole (qa4 F16), a narrower one truncates.
                .child(div().min_w_0().truncate().text_size(px(crate::ui::ROW_TEXT)).font_weight(FontWeight::MEDIUM).text_color(pal.text).child(match &filtered_count {
                    // (Other languages: the translated title, truncated where it
                    // meets the icons, live "フィーチ…".)
                    _ if errors > 0 && crate::ui::left_panel_w() < crate::ui::LEFT_PANEL_W + 10. && crate::i18n::current() == crate::i18n::DEFAULT => "Features …".to_string(),
                    Some(k) => format!("Features ({k} of {n_features})"),
                    None => format!("Features ({n_features})"),
                }))
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
                )
                .into_any_element(),
        );
        head
    }

    /// One feature's row. (Not a cached view of its own: gpui re-renders
    /// every cached view nested in one being re-rendered, here the list's.)
    /// `slot`: its child index in the list and its indent (see `render_feature_row`).
    fn render_feature_row_view(&mut self, id: FeatureId, slot: (usize, f32), cx: &mut Context<Self>) -> AnyElement {
        let Some((i, f)) = self.studio(cx).and_then(|s| s.features.iter().position(|f| f.id == id).map(|i| (i, s.features[i].clone()))) else {
            return div().into_any_element();
        };
        let (rolled_back, editing) = (i >= self.row_ctx.rollback, self.row_ctx.editing_id == Some(id));
        self.render_feature_row(i, &f, rolled_back, editing, slot, cx)
    }

    /// A row of the parts list: click selects (or feeds a dialog's part
    /// field), hover highlights the part, the eye hides or shows it, and the
    /// context menu has Onshape's part commands.
    fn render_part_row(&self, b: &noshape_model::display::BodyDisplay, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let r = Reference::Body(b.id.clone());
        // A part picked in the open dialog's active field is selected in the
        // list too (live part-studio/440; not while another field is active,
        // psui/m-03).
        let selected = match &self.edit {
            Some(e) => e.dialog.read(cx).active_refs().contains(&r),
            None => self.selection.contains(&r),
        };
        let hidden = b.hidden;
        let (id1, id2, id3) = (b.id.clone(), b.id.clone(), b.id.clone());
        let r_hover = r.clone();
        let view = cx.entity();
        let part_name = b.name.clone();
        let several_parts = self.display.bodies.len() > 1;
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
        // While renaming the row is white with no hover fill: `tree_row` gets
        // `selected` so it sets no hover style (a second `.hover` panics, CRASH1).
        crate::ui::tree_row(SharedString::from(format!("part-{}", b.id)), selected || renaming)
            .h(px(24.))
            .pl(px(23.))
            .mr(px(5.))
            .group("part-row")
            .when(renaming, |d| d.bg(gpui_kit::rgb(0xffffff)))
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
                    .child(crate::ui::user_text(b.name.clone()))
                    .tooltip(crate::ui::user_tooltip(match &b.composite {
                        Some(c) => format!("{} ({}) ({} parts)", b.name, if c.closed { "closed" } else { "open" }, c.members.len()),
                        None => b.name.clone(),
                    }))
                    .into_any_element(),
            })
            // Parts list properties (part number, revision).
            .when_some(self.ui.show_part_props.then(|| self.studio(cx).and_then(|s| s.part_properties.get(&b.id)).map(|p| [p.part_number.clone().unwrap_or_default(), p.revision.clone().unwrap_or_default()].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(" · "))).flatten().filter(|t| !t.is_empty()), |d, t| d.child(div().text_xs().text_color(pal.text_faint).child(crate::ui::user_text(t))))
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
                m = m
                    .separator()
                    .item(item(if hidden { "Show" } else { "Hide" }, Box::new(move |this, _, cx| {
                        let t: Vec<Reference> = targets(this, &b5).into_iter().map(Reference::Body).collect();
                        this.set_visibility(&t, !hidden, cx);
                    })).icon_name(if hidden { "eye" } else { "eye-off" }));
                // Live (qa5 MENU1, part-studio/1624): with several parts,
                // Hide other parts and Hide all parts follow.
                if several_parts {
                    let keep = b.clone();
                    m = m
                        .item(item(&crate::i18n::template("Hide other __label__", &[("label", "parts")]), Box::new(move |this, _, cx| {
                            let keep = targets(this, &keep);
                            let others: Vec<Reference> = this.display.bodies.iter().filter(|d| !keep.contains(&d.id)).map(|d| Reference::Body(d.id.clone())).collect();
                            this.set_visibility(&others, true, cx);
                        })))
                        .item(item(&crate::i18n::template("Hide all __label__", &[("label", "parts")]), Box::new(|this, _, cx| {
                            let all: Vec<Reference> = this.display.bodies.iter().map(|d| Reference::Body(d.id.clone())).collect();
                            this.set_visibility(&all, true, cx);
                        })));
                }
                m = m
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
                    // Live (qa5 DEL2, part-studio/1627): the "Delete part 1"
                    // dialog with the parts prefilled; ✓ commits.
                    this.selection = targets(this, &b11).into_iter().map(Reference::Body).collect();
                    this.start_feature("deleteBodies", w, cx);
                })).icon_name("menu-delete"))
            })
            .into_any_element()
    }

    /// The header's New folder: the "Folder name" popup, "New folder"
    /// selected (live 586).
    fn open_folder_prompt(&mut self, ids: Vec<FeatureId>, window: &mut Window, cx: &mut Context<Self>) {
        self.open_folder_prompt_for(FolderTarget::Features(ids), window, cx);
    }

    fn open_folder_prompt_for(&mut self, target: FolderTarget, window: &mut Window, cx: &mut Context<Self>) {
        let st = cx.new(|cx| gpui_kit::component::input::InputState::new(window, cx).default_value(noshape_ops::studio::NEW_FOLDER_NAME));
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        let sub = cx.subscribe_in(&st, window, |this: &mut Self, _, ev: &gpui_kit::component::input::InputEvent, window, cx| {
            if matches!(ev, gpui_kit::component::input::InputEvent::PressEnter { .. }) {
                this.commit_folder_prompt(window, cx);
            }
        });
        self.folder_prompt = Some((st, target, sub));
        cx.notify();
    }

    /// ✓ / Enter: an empty folder at the rollback bar, or the selection wrapped.
    fn commit_folder_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((st, target, _)) = self.folder_prompt.take() else { return };
        let name = st.read(cx).value().to_string();
        let made = self.doc.update(cx, |doc, cx| {
            doc.edit_studio(self.tab, noshape_ops::studio::ADD_FOLDER_DESCRIPTION, cx, |ps| match &target {
                FolderTarget::Features(ids) => Some(noshape_ops::studio::add_folder(ps, ids, &name)),
                FolderTarget::Folder(f) => {
                    let name = name.trim();
                    ps.wrap_folder(*f, if name.is_empty() { noshape_ops::studio::NEW_FOLDER_NAME } else { name }.to_string())
                }
            })
        });
        // Live (ui 10-25 b (1)): the new folder's row replaces the selection,
        // nothing selected, the row related-highlighted until the next click.
        self.selected_features.clear();
        self.ui.selected_folder = None;
        self.ui.fresh_folder = made.flatten();
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
                    .child(div().id("folder-ok").size(px(28.)).flex().items_center().justify_center().bg(gpui_kit::rgb(0x009400)).cursor_pointer().child(crate::ui::glyph("check", 16., gpui_kit::rgb_static(0xffffff))).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.commit_folder_prompt(window, cx))))
                    .child(div().id("folder-cancel").size(px(28.)).flex().items_center().justify_center().cursor_pointer().child(crate::ui::glyph("close", 14., gpui_kit::rgb(0xd52b1e))).on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.folder_prompt = None;
                        let fh = this.focus.clone();
                        window.focus(&fh, cx);
                        cx.notify();
                    }))),
            )
            .child(div().m(px(6.)).border_1().border_color(pal.accent).child(gpui_kit::component::input::Input::new(st).small().appearance(false).text_size(px(13.33)).text_color(gpui_kit::rgb(0x000000)).pl(px(6.))));
        // A zero-height box under the anchor row; the popup rises beside it.
        div().relative().w_full().h_0().child(div().absolute().left_full().top(px(-26.)).size_0().child(deferred(anchored().snap_to_window().child(popup)).with_priority(5))).into_any_element()
    }

    /// A sketch listed under its Derived row (live part-studio/1582): the
    /// sketch icon and name, 12 px in; a click selects the sketch.
    fn render_derived_sketch_row(&self, id: FeatureId, copy: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(sk) = self.display.sketches.iter().find(|s| s.feature == id) else { return div().h(px(crate::ui::ROW_H)).into_any_element() };
        let r = Reference::Sketch(id);
        let selected = self.selection.contains(&r);
        crate::ui::tree_row(SharedString::from(format!("dsk-{id}-{copy}")), selected)
            // Live 1582: the sketch icon at x 66, the name at 86.
            .pl(px(23.))
            .mr(px(5.))
            .pr(px(11.))
            .child(div().flex_none().child(crate::ui::tree_icon(&feature_icon("sketch"), sk.hidden, cx)))
            .child(div().flex_1().truncate().child(crate::ui::user_text(sk.name.clone())))
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.selection = vec![r.clone()];
                this.refresh_scene(false, cx);
                cx.notify();
            }))
            .into_any_element()
    }

    /// Where a list row is on screen, for its side tooltip: painted at the
    /// list's scroll offset; an indented row sits in a wrapper as wide as
    /// the list, inset by the indent and its 5 px right margin.
    fn row_anchor(&self, (ix, indent): (usize, f32)) -> impl Fn() -> Bounds<Pixels> + 'static {
        let scroll = self.ui.list_scroll.clone();
        move || {
            let b = scroll.bounds_for_item(ix).unwrap_or_default();
            let b = Bounds::new(b.origin + scroll.offset(), b.size);
            if indent > 0. { Bounds::new(point(b.left() + px(indent), b.top()), size(b.size.width - px(indent + 5.), b.size.height)) } else { b }
        }
    }

    /// A folder row (live ui 10-25 b): a click selects it, only the chevron
    /// opens or closes it, a double-click does nothing; its menu is Rename |
    /// Add selection to folder… | Suppress, Dynamic suppression ▸ | Roll to
    /// here | Expand ▸, Collapse ▸ | Delete, Unpack folder.
    fn render_folder_row(&self, fd: &noshape_model::partstudio::Folder, slot: (usize, f32), cx: &mut Context<Self>) -> AnyElement {
        let id = fd.id;
        let collapsed = self.folder_collapsed(fd);
        let selected = self.ui.selected_folder == Some(id);
        let fresh = self.ui.fresh_folder == Some(id) && !selected;
        let suppressed = self.folder_suppressed(id, cx);
        let has_expr = self.folder_has_expr(id, cx);
        // Live (ui 10-25 b (4), 10-25 c): the hover tooltip is the plain name,
        // "(Suppressed)" after it while everything inside is.
        let tip = if suppressed { format!("{} (Suppressed)", fd.name) } else { fd.name.clone() };
        // Roll to here is offered unless the bar already sits right after
        // the folder's last feature (live qa4 r5 s2: Roll to here on the
        // rolled-back last feature brings the bar back to the end).
        let is_last = self.studio(cx).is_some_and(|s| s.folder_features(id).last().and_then(|l| s.index_of(*l)).is_some_and(|i| i + 1 == s.rollback_index()));
        let row = crate::ui::tree_row(SharedString::from(format!("folder-{id}")), selected)
            .mr(px(5.))
            // Live `related-highlight`: #ECF0F4 with a 1 px #90CEF1 line under it.
            .when(fresh, |d| d.bg(p().row_related).border_b_1().border_color(gpui_kit::rgb(0x90cef1)))
            .child(
                div()
                    .id(SharedString::from(format!("folder-chev-{id}")))
                    .flex_none()
                    .child(crate::ui::caret(!collapsed))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        this.toggle_folder(id, cx);
                    })),
            )
            .child(crate::ui::tree_icon("folder", suppressed, cx))
            .child(match self.folder_renaming.as_ref().filter(|(fid, _, _)| *fid == id) {
                Some((_, st, _)) => div().flex_1().child(gpui_kit::component::input::Input::new(st).xsmall().text_size(px(13.33)).pl(px(3.))).into_any_element(),
                // Live 587, 590: "New folder (0)" — the name and its feature count;
                // a suppressed folder grey, italic and struck through (2d).
                // Live (ui 10-25 c): the name 20 px after the folder glyph.
                None => div()
                    .flex_1()
                    .ml(px(-2.))
                    .truncate()
                    .when(suppressed, |d| d.italic().line_through().text_color(p().text_faint))
                    .child(crate::ui::user_text(format!("{} ({})", fd.name, self.studio(cx).map(|s| s.folder_count(id)).unwrap_or(0))))
                    .into_any_element(),
            })
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.select_folder(id, cx)))
            // Dropping features on it puts them in as its first children (live
            // qa4 r5 s8 (6)); the row lights up under the dragged feature.
            .drag_over::<FeatureDrag>(|s, _, _, _| s.bg(gpui_kit::rgb(0xe0f0fb)))
            .on_drop(cx.listener(move |this, d: &FeatureDrag, _, cx| this.drop_into_folder(d.id, id, cx)));
        // Beside the row, as a feature row's (live ui 10-25 b (4)).
        let row = crate::ui::with_lazy_side_tooltip(row, self.row_anchor(slot), move |_| SharedString::from(tip.clone()));
        let view = cx.entity();
        row.os_context_menu(move |menu, window, cx| {
            view.update(cx, |this, cx| this.select_folder(id, cx));
            let item = |label: &str, f: ViewAction| {
                let v = view.clone();
                crate::os_menu::OsMenuItem::new(label.to_string()).on_click(move |_, w, cx| v.update(cx, |this, cx| f(this, w, cx)))
            };
            let (ve, vc, vd) = (view.clone(), view.clone(), view.clone());
            let mut m = menu
                .item(item("Rename", Box::new(move |this, w, cx| this.start_folder_rename(id, w, cx))))
                .separator()
                .item(item("Add selection to folder…", Box::new(move |this, w, cx| this.open_folder_prompt_for(FolderTarget::Folder(id), w, cx))))
                .separator();
            m = m.item(item(if suppressed { "Unsuppress" } else { "Suppress" }, Box::new(move |this, _, cx| this.suppress_folder(id, !suppressed, cx))));
            // Live (ui 10-25 c (2)): with expressions on its features the folder
            // offers "Edit dynamic suppression ▸ Remove expression" only.
            m = if has_expr {
                m.submenu("Edit dynamic suppression", window, cx, move |sm, _, _| {
                    let v = vd.clone();
                    sm.item(crate::os_menu::OsMenuItem::new("Remove expression").on_click(move |_, _, cx| v.update(cx, |this, cx| this.clear_folder_dynamic_suppression(id, cx))))
                })
            } else {
                m.submenu("Dynamic suppression", window, cx, move |sm, _, _| {
                    let v = vd.clone();
                    sm.item(crate::os_menu::OsMenuItem::new("Add expression…").on_click(move |_, w, cx| v.update(cx, |this, cx| this.edit_folder_dynamic_suppression(id, w, cx))))
                })
            };
            if !is_last {
                m = m.separator().item(item("Roll to here", Box::new(move |this, _, cx| this.roll_to_folder(id, cx))));
            }
            m = m.separator();
            m = m.submenu("Expand", window, cx, move |sm, _, _| {
                let (v1, v2) = (ve.clone(), ve.clone());
                sm.item(crate::os_menu::OsMenuItem::new("Expand selected").on_click(move |_, _, cx| v1.update(cx, |this, cx| this.set_folder_collapsed(id, false, cx))))
                    .item(crate::os_menu::OsMenuItem::new("Expand all").on_click(move |_, _, cx| v2.update(cx, |this, cx| this.set_all_folders_collapsed(false, cx))))
            });
            m = m.submenu("Collapse", window, cx, move |sm, _, _| {
                let (v1, v2) = (vc.clone(), vc.clone());
                sm.item(crate::os_menu::OsMenuItem::new("Collapse selected").on_click(move |_, _, cx| v1.update(cx, |this, cx| this.set_folder_collapsed(id, true, cx))))
                    .item(crate::os_menu::OsMenuItem::new("Collapse all").on_click(move |_, _, cx| v2.update(cx, |this, cx| this.set_all_folders_collapsed(true, cx))))
            });
            m.separator()
                .item(item("Delete", Box::new(move |this, _, cx| this.delete_folder(id, cx))).icon_name("menu-delete"))
                .item(item("Unpack folder", Box::new(move |this, _, cx| this.dissolve_folder(id, cx))))
        })
        .into_any_element()
    }

    /// A click on a folder row selects it alone (live ui 10-25 b (4)).
    fn select_folder(&mut self, id: u32, cx: &mut Context<Self>) {
        self.ui.fresh_folder = None;
        if self.ui.selected_folder != Some(id) || !self.selected_features.is_empty() {
            self.ui.selected_folder = Some(id);
            self.selected_features.clear();
            self.refresh_scene(false, cx);
            cx.notify();
        }
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
                let f = e.dialog.read(cx).row_feature();
                let units = self.doc.read(cx).session.doc.settings.units;
                let computed = self.computed_values(f.id, cx);
                noshape_model::interaction::display_name_computed(&f, &|k| {
                    computed.iter().find(|(c, _)| c == k).map(|(_, v)| v.clone()).or_else(|| crate::feature_dialog::shown_length(&f, k, &units, &self.display.variables))
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
            .child(div().flex_1().truncate().child(crate::ui::user_text(name)))
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

    /// `slot`: the row's child index in the list and its indent: its
    /// tooltip asks the list's scroll handle where the row is when it opens
    /// (rather than each row recording its bounds as it paints).
    fn render_feature_row(&self, i: usize, f: &Feature, rolled_back: bool, editing: bool, slot: (usize, f32), cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let related = self.row_ctx.related.contains(&f.id);
        // The feature being edited shows its draft's name (live: a Hole's
        // row reads "Ø 6.35 mm ↧ 12 mm" while its dialog is open).
        let draft = self.edit.as_ref().map(|e| e.dialog.read(cx).row_feature()).filter(|d| d.id == f.id);
        // The name shows the active configuration's values (a configured
        // Variable reads "#w = 80 mm" under L; qa4 round 4 S4-a).
        let configured = draft.is_none().then(|| self.studio(cx).filter(|s| s.configuration.configured.iter().any(|c| c.feature == f.id)).map(|s| s.configuration.for_editing(f, &self.current_config(s)))).flatten();
        let f = draft.as_ref().or(configured.as_ref()).unwrap_or(f);
        let name = self.shown_feature_name(f, cx);
        let state = self.display.feature_state(f.id);
        let status = if editing && self.edit_invalid(cx) { FeatureStatus::Error } else { state.map(|s| s.status).unwrap_or(FeatureStatus::Ok) };
        let selected = self.selected_features.contains(&f.id);
        // Onshape (§3.4): rolled back = italic #ccc, suppressed = #999 struck
        // through, error = #aa252d; everything else (sketches included) #333.
        // Suppressed directly, or by a configuration / expression (the
        // regenerated state says so).
        let suppressed = f.suppressed || (!rolled_back && status == FeatureStatus::Suppressed);
        // Hidden features (a sketch consumed by a feature) read gray too (live part-studio/92),
        // unless they fail: a failing hidden sketch is red (qa5 P1-7, behaviours 6432 / 6491).
        let failing = matches!(status, FeatureStatus::Error | FeatureStatus::Warning);
        let color = if rolled_back {
            pal.text_rolled_back
        } else if suppressed || (f.hidden && !failing) {
            pal.text_faint
        } else {
            // Live L27: a warning's row is red like an error's.
            match status {
                FeatureStatus::Error | FeatureStatus::Warning => pal.text_invalid,
                _ => pal.text,
            }
        };
        let underdefined = !rolled_back && state.is_some_and(|s| s.underdefined);
        let id = f.id;
        // Onshape (live part-studio/16, 90, 95): hovering a row shows its
        // error, else what it depends on ("Extrude 1 <- Sketch 1"), else its
        // name, in a box beside the row. Worked out when it opens (the
        // dependencies scan the features above).
        let tip = {
            // Live (ui 10-22 d, behaviours "Feature-row hover tooltips"): the
            // row tooltip reads `<name>[ [Type]][ (Hidden)][ is not fully
            // defined | did not regenerate properly: <msg>]`, or `<head> <-
            // <inputs>`. The [Type] tag shows unless the name contains the
            // type's name, case-insensitively (as `typed_name`: "MC to hide
            // [Mate connector]", a Hole's callout "… [Hole]"). Infos stay out
            // of it (qa5 BOOL1: the graphics bubble).
            fn typed(name: &str, type_name: &str) -> String {
                if needs_type_tag(name, type_name) { format!("{name} [{type_name}]") } else { name.to_string() }
            }
            let head = format!("{}{}", typed(&name, f.feature_type().map(|t| t.name).unwrap_or("")), if f.hidden { " (Hidden)" } else { "" });
            let msg = state.and_then(|s| {
                let m = s.message.clone()?;
                match s.status {
                    // Live: unresolved selections add a second line ("1 missing selection").
                    FeatureStatus::Error | FeatureStatus::Warning => {
                        let more = noshape_model::regen::missing_text(s.missing);
                        Some(if s.missing > 0 && m != more { format!("{head} did not regenerate properly: {m}\n{more}") } else { format!("{head} did not regenerate properly: {m}") })
                    }
                    FeatureStatus::Info => None,
                    _ => Some(m),
                }
            });
            // A sketch lists no inputs; an under-defined one says so.
            let sketch_tip = (f.type_id == "sketch").then(|| format!("{head}{}", if underdefined { " is not fully defined" } else { "" }));
            // Live (qa4 r5 s8 (3), ui 10-25 c): a suppressed row reads "Extrude 1
            // (Suppressed)", one its expression suppresses "Fillet 3 (Suppressed
            // with expression)".
            let suppressed_tip = (f.suppressed || state.is_some_and(|s| s.status == FeatureStatus::Suppressed))
                .then(|| format!("{head} ({})", if f.suppression_expr.is_some() { "Suppressed with expression" } else { "Suppressed" }));
            // Live (ui 10-22 b): a Derived reads "Derived 1 <- <source studio>".
            let derived_source = (f.type_id == "importDerived")
                .then(|| {
                    let key = f.params.text("partStudio").trim().to_string();
                    self.doc.read(cx).session.doc.tabs.iter().find(|t| t.id.to_string() == key).map(|t| t.name.clone())
                })
                .flatten();
            let view = cx.entity().downgrade();
            move |cx: &App| -> SharedString {
                if let Some(t) = &suppressed_tip {
                    return t.clone().into();
                }
                if let Some(m) = &msg {
                    return m.clone().into();
                }
                if let Some(t) = &sketch_tip {
                    return t.clone().into();
                }
                let parents: Vec<String> = if let Some(src) = &derived_source {
                    vec![src.clone()]
                } else {
                    view.upgrade()
                        .and_then(|v| {
                            let studio = v.read(cx).studio(cx)?;
                            let i = studio.features.iter().position(|g| g.id == id)?;
                            // Live lists only the sketches a feature uses ("Extrude 1 <-
                            // Sketch 1", "[Hole] <- Sketch 2", "<- Path [Sketch]"); parts
                            // picked in a Boolean or a merge scope aren't named (qa5 BOOL1b,
                            // behaviours B4; Fillet / Chamfer / Shell: the name alone).
                            Some(noshape_model::interaction::feature_dependencies(&studio.features, i).into_iter().filter_map(|p| studio.feature(p).filter(|g| g.type_id == "sketch").map(typed_name)).collect())
                        })
                        .unwrap_or_default()
                };
                if parents.is_empty() { head.clone().into() } else { format!("{head} <- {}", parents.join(", ")).into() }
            }
        };
        let manually_suppressed = f.suppressed;
        let hidden = f.hidden;
        // Sketches, planes and mate connectors can be hidden from the list.
        let rc = &self.row_ctx;
        let hideable = matches!(f.type_id.as_str(), "sketch" | "cPlane" | "mateConnector") || rc.sketches.contains(&id);
        let first_config_input = rc.first_config_input.clone();
        let dyn_expr = f.suppression_expr.clone();
        let makes_faces = rc.makes_faces.contains(&id);
        let has_appearance = rc.appearances.contains(&id);
        let is_sketch = rc.sketches.contains(&id);
        let swatch = self.sketch_swatch(id, cx);
        let has_plane = is_sketch || rc.planes.contains(&id);
        // Live (part-studio/495): the icon's ink starts at x 46, the name at 67.
        let row = crate::ui::tree_row(SharedString::from(format!("feat-{}", f.id)), selected || editing)
            .pl(px(4.))
            .mr(px(5.))
            .pr(px(11.))
            .when(self.ui.list_menu == Some(id) && !(selected || editing), |d| d.bg(pal.row_hover))
            .group("feature-row")
            .relative()
            // Rollback drop line (live part-studio/30): a small hollow blue
            // circle at its left end, shown while the bar is dragged over
            // (only built during a drag: its start redraws every row).
            .when(cx.has_active_drag(), |d| d.child(
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
            ))
            // The same dot on a feature drag's line, top or bottom edge (F33).
            .when(cx.has_active_drag(), |d| {
                let after = self.ui.feature_drop.get() == Some((i, true));
                d.child(
                    div()
                        .absolute()
                        .left(px(0.))
                        .when(after, |d| d.bottom(px(-3.5)))
                        .when(!after, |d| d.top(px(-2.5)))
                        .size(px(7.))
                        .rounded_full()
                        .border_2()
                        .border_color(pal.accent)
                        .bg(gpui_kit::rgb(0xffffff))
                        .invisible()
                        .group_drag_over::<FeatureDrag>("feature-row", |s| s.visible()),
                )
            })
            .text_color(color)
            // Onshape (part-studio/31, 92): rolled back and suppressed rows are italic.
            .when(rolled_back || suppressed, |d| d.italic())
            .when(related && !selected && !editing, |d| d.bg(pal.row_related))
            .when(editing, |d| d.font_weight(FontWeight::SEMIBOLD))
            // Live (qa5 derived, 1580–1582): a Derived feature that brought
            // sketches has a ▸ before its icon; it lists them.
            .when(self.display.derived_sketches.iter().any(|(d, _)| *d == f.id), |d| {
                let open = self.ui.derived_open.contains(&f.id);
                let fid = f.id;
                d.child(
                    div()
                        .id(SharedString::from(format!("feat-exp-{fid}")))
                        .flex_none()
                        // Live 1582: caret ink at x 44, icon at 66.
                        .w(px(17.))
                        .child(crate::ui::caret(open))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            if !this.ui.derived_open.remove(&fid) {
                                this.ui.derived_open.insert(fid);
                            }
                            cx.notify();
                        })),
                )
            })
            .child(
                div()
                    .relative()
                    .flex_none()
                    // A custom feature's row shows its initials box (live qa5 FS4).
                    .child(match features::get(&f.type_id).filter(|_| noshape_model::custom::is_custom(&f.type_id)) {
                        Some(t) => crate::ui::initials_icon_status(t.name, status == FeatureStatus::Error && !rolled_back).when(rolled_back || suppressed || hidden, |d| d.opacity(0.5)),
                        None => crate::ui::tree_icon(&feature_icon(&f.type_id), rolled_back || suppressed || hidden, cx),
                    })
                    // Onshape list badges: under-defined sketch (12 px blue circle,
                    // undimmed, 8 px in from the icon's top-left). An error only turns
                    // the name red (part-studio/94).
                    // Missing selections / warnings: Onshape's 12 px yellow triangle
                    // badge on the icon's bottom right (live 2071, 2458 Refold),
                    // also on a failed feature that lost a selection (qa5 P1-18).
                    .when(warning_badge(status, state.map_or(0, |s| s.missing), rolled_back), |d| d.child(div().absolute().left(px(9.)).top(px(8.)).child(crate::assets::icon("list-warning-overlay", 12., cx))))
                    .when(underdefined, |d| d.child(div().absolute().left(px(9.5)).top(px(8.)).child(crate::assets::icon("list-underdefined-overlay", 12., cx)))),
            )
            .child(match self.renaming.as_ref().filter(|(rid, _, _)| *rid == f.id) {
                // Live (qa4 r5 s8 (5)): Esc cancels; Enter or a click elsewhere
                // commits (dropping the subscription keeps the blur from it).
                Some((_, st, _)) => div()
                    .flex_1()
                    .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                        if ev.keystroke.key == "escape" {
                            this.renaming = None;
                            cx.stop_propagation();
                            let fh = this.focus.clone();
                            window.focus(&fh, cx);
                            cx.notify();
                        }
                    }))
                    .child(gpui_kit::component::input::Input::new(st).xsmall().text_size(px(13.33)).pl(px(3.)))
                    .into_any_element(),
                // Live (psb 10-21 p): a Hole's row shows its whole callout,
                // both parts ("Ø 6.6 mm ↧ 10 mm | ⌴Ø 11 mm ↧ 6.5 mm"), cut by
                // the ellipsis at the list's width.
                None => div().flex_1().truncate().when(suppressed, |d| d.line_through()).child(crate::ui::user_text(name.clone())).into_any_element(),
            })
            // In-context arrow (features referencing an Assembly context).
            .children(self.context_arrow(f, cx))
            // Onshape's expression glyph on features with dynamic suppression,
            // suppressed or not (live ui 10-25 c: a grey #888 italic "fx" at the
            // row's right, tooltip "Dynamic suppression: {#s == 0}").
            .when_some(dyn_expr.clone().map(|e| format!("Dynamic suppression: {{{e}}}")), |d, e| {
                d.child(
                    div()
                        .id(SharedString::from(format!("feat-dyn-{id}")))
                        .flex_none()
                        .text_size(px(13.))
                        .italic()
                        .text_color(gpui_kit::rgb(0x888888))
                        .cursor_pointer()
                        .child("fx")
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
                // A Plane feature's row feeds the section view dialog and an
                // open analysis tool like the default planes' rows (qa5 ADV1).
                let plane = this.display.planes.iter().find(|p| p.reference == Reference::FeaturePlane(id)).map(|p| p.reference.clone());
                if let Some(r) = &plane {
                    if this.ui.section_dialog.is_some() {
                        this.section_pick(r, window, cx);
                        return;
                    }
                    if this.edit.is_none() && this.ui.analysis_tool.is_some() && this.analysis_pick(r, cx) {
                        return;
                    }
                }
                if this.pick_feature_into_dialog(id, cx) {
                    return;
                }
                // A dialog field that can't take it: nothing happens (live).
                if this.active_field_filter(cx).is_some() {
                    return;
                }
                this.click_feature(id, ev.modifiers().shift, cx);
                // Onshape: after clicking a row, ↑/↓ move the rollback bar.
                window.focus(&this.ui.list_focus, cx);
            }))
            .drag_over::<RollbackDrag>(|s, _, _, _| s.border_t_2().border_color(p().accent))
            .on_drop(cx.listener(move |this, _: &RollbackDrag, _, cx| this.set_rollback(Some(i), cx)))
            // Live (F33): a blue pill with the feature's name, "2 items" for several.
            .on_drag(
                FeatureDrag {
                    id,
                    name: if selected && self.selected_features.len() > 1 { format!("{} items", self.selected_features.len()).into() } else { name.clone().into() },
                },
                |d, grab, _, cx| cx.new(|_| FeatureDragPill(grab, d.name.clone())),
            )
            // Live (F33): the half of the hovered row the cursor is in picks
            // the slot, whichever way the drag goes: the upper half lands
            // before the row (line on its top edge), the lower half after it
            // (line on its bottom edge); the row gets the hover fill.
            .on_drag_move(cx.listener(move |this, e: &DragMoveEvent<FeatureDrag>, _, cx| {
                if !e.bounds.contains(&e.event.position) {
                    return;
                }
                let slot = Some((i, e.event.position.y >= e.bounds.center().y));
                if this.ui.feature_drop.get() != slot {
                    this.ui.feature_drop.set(slot);
                    cx.notify();
                }
            }))
            .drag_over::<FeatureDrag>({
                let drop = self.ui.feature_drop.clone();
                move |s, _, _, _| {
                    let s = s.bg(gpui_kit::rgb(0xe0f0fb)).border_color(p().accent);
                    if drop.get() == Some((i, true)) { s.border_b_2() } else { s.border_t_2() }
                }
            })
            .on_drop(cx.listener(move |this, d: &FeatureDrag, _, cx| {
                let after = this.ui.feature_drop.take().is_some_and(|(r, a)| r == i && a);
                this.drop_features(d.id, i, after, cx);
            }));
        let row = crate::ui::with_lazy_side_tooltip(row, self.row_anchor(slot), tip);
        let view = cx.entity();
        // Roll to here unless the bar already sits right after this
        // feature, Roll to end while it isn't at the end (live qa4 r5 s2,
        // ROLL-LAST; `behaviors::roll_items`).
        let (here, to_end) = self.studio(cx).map(|s| behaviors::roll_items(i, s.rollback_index(), s.features.len())).unwrap_or((false, false));
        let source_sketch = self.studio(cx).and_then(|s| noshape_model::interaction::source_sketch(&s.features, f).and_then(|sid| s.feature(sid).map(|g| (sid, typed_name(g)))));
        let feature_name = f.name.clone();
        // A Derived feature's source studio ("Switch to qa5 src", live "qa5
        // derived" (6)).
        let derived_source: Option<(TabId, String)> = (f.type_id == "importDerived")
            .then(|| {
                let key = f.params.text("partStudio").trim().to_string();
                self.doc.read(cx).session.doc.tabs.iter().find(|t| t.id.to_string() == key).map(|t| (t.id, t.name.clone()))
            })
            .flatten();
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
            // Unsuppress on a feature its expression suppresses drops the
            // expression (live assembly/142); otherwise Suppress / Unsuppress.
            let suppress = if has_expr && suppressed {
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
                // Live (ui 10-25 c (2)): Edit dynamic suppression ▸ Edit
                // expression…, Remove expression.
                if has_expr {
                    let (v1, v2) = (vd.clone(), vd.clone());
                    return m.submenu("Edit dynamic suppression", window, cx, move |sm, _, _| {
                        sm.item(crate::os_menu::OsMenuItem::new("Edit expression…").on_click(move |_, w, cx| v1.update(cx, |this, cx| this.edit_dynamic_suppression(id, w, cx))))
                            .item(crate::os_menu::OsMenuItem::new("Remove expression").on_click(move |_, _, cx| v2.update(cx, |this, cx| this.remove_dynamic_suppression(id, cx))))
                    });
                }
                m.submenu("Dynamic suppression", window, cx, move |sm, _, _| {
                    let v1 = vd.clone();
                    sm.item(crate::os_menu::OsMenuItem::new("Add expression…").on_click(move |_, w, cx| v1.update(cx, |this, cx| this.edit_dynamic_suppression(id, w, cx))))
                })
            };
            let appearance = |m: crate::os_menu::OsMenu| {
                if is_sketch {
                    m.item(item("Edit sketch appearance…", Box::new(move |this, w, cx| this.edit_appearance(appearance::EditTarget::Feature(id, None), w, cx))).swatch(swatch))
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
                // Live (ui 10-25 b (1)): "Add appearance to 2 features…".
                let n = view.read(cx).selected_features.len();
                let add = item(&format!("Add appearance to {n} features…"), Box::new(move |this, w, cx| {
                    let t = targets(this);
                    this.edit_appearance(appearance::EditTarget::Features(t), w, cx);
                }))
                .icon_name("add-appearance");
                return m.separator().item(add).separator().item(delete);
            }
            // Roll to here unless the bar is right after the feature; Roll to
            // end while the bar isn't at the end (live qa4 r5 s2 rollback
            // follow-up).
            let rolls = move |m: crate::os_menu::OsMenu| {
                let mut m = m;
                if here || to_end {
                    m = m.separator();
                }
                if here {
                    m = m.item(item("Roll to here", Box::new(move |this, _, cx| this.set_rollback(Some(i + 1), cx))));
                }
                if to_end {
                    m = m.item(item("Roll to end", Box::new(move |this, _, cx| this.set_rollback(None, cx))));
                }
                m
            };
            // A suppressed feature, by hand or by its expression, and a rolled
            // back one (live qa4 r5 s8 (3), ui 10-25 c (2), s2 rollback
            // follow-up): Rename | Add selection to folder… | Suppress or
            // Unsuppress, (Edit) dynamic suppression ▸ | Roll to here, Roll to
            // end | Delete (Add comment is collaboration, cloud).
            if suppressed || rolled_back {
                let mut m = menu.item(item("Rename", Box::new(move |this, w, cx| this.start_rename(id, w, cx)))).separator().item(folder).separator().item(suppress);
                m = dynamic(m, window, cx);
                return rolls(m).separator().item(delete);
            }
            let rename = item("Rename", Box::new(move |this, w, cx| this.start_rename(id, w, cx)));
            let mut m = menu.item(rename).separator().item(item("Edit…", Box::new(move |this, w, cx| this.edit_feature(id, w, cx))));
            if let Some((sid, sname)) = source_sketch.clone() {
                // Live (de-sc-4): „Sketch 1 [Skizze]“ bearbeiten …, the sketch's
                // type in brackets when its name doesn't start with it (in the
                // UI language; English "Edit Sketch 1…").
                let sketch_type = crate::i18n::type_name("Sketch");
                let shown = if sname.starts_with(sketch_type.as_str()) { sname.clone() } else { format!("{sname} [{sketch_type}]") };
                m = m.item(item(&format!("Edit {shown}…"), Box::new(move |this, w, cx| this.edit_feature(sid, w, cx))));
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
                })).icon_name(if hidden { "eye" } else { "eye-off" }));
            }
            if is_sketch {
                // Live order: Create Drawing of Sketch N…, Export as DXF/DWG… (a sketch view; asb).
                let sk_name = feature_name.clone();
                m = m.separator().item(item(&format!("Create Drawing of {sk_name}…"), Box::new(move |this, w, cx| crate::drawing_create::request_create_drawing_of_sketch(this.tab, id, &sk_name, w, cx))).icon_name("menu-create-drawing"));
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
            m = dynamic(m, window, cx);
            if let Some((tab, name)) = derived_source.clone() {
                m = m.separator().item(item(&format!("Switch to {name}"), Box::new(move |_, _, cx| cx.emit(PartStudioEvent::ActivateTab(tab)))).icon_name("part-studio"));
            }
            m = m.separator().item(item("Zoom to selection", Box::new(move |this, _, cx| {
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
            m = rolls(m);
            if is_sketch || makes_faces {
                m = appearance(m.separator());
            }
            m.separator().item(delete)
        })
        .into_any_element()
    }

    /// What the feature dialog covers: plane names and the graphics
    /// preferences dialog.
    fn render_viewport_underlay(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let pal = p();
        let mut out = Vec::new();
        // Plane names are drawn by the scene (scene_builder: in the plane,
        // hidden by parts in front, live selection/02); in-plane text the
        // parts don't hide goes here.
        let mut fills = Vec::new();
        // Inspection datums: black, the selected one in the selection
        // color.
        let sel = Rgba::from(Hsla::from(pal.accent));
        for (contours, selected) in self.annotation_contours(cx) {
            let color = if selected { [sel.r, sel.g, sel.b, 1.0] } else { [0.0, 0.0, 0.0, 1.0] };
            fills.push(noshape_render::overlay::WorldFill { contours: std::sync::Arc::new(contours), color });
        }
        // Shown dimensions' labels, in their sketch plane.
        fills.extend(self.shown_dims_fills(cx));
        // The sketch's name, in its plane.
        let name = self.sketch_name_contours(cx);
        if !name.is_empty() {
            let c = Rgba::from(Hsla::from(rgb(crate::sketch_mode::SKETCH_BOX_COLOR)));
            fills.push(noshape_render::overlay::WorldFill { contours: std::sync::Arc::new(name), color: [c.r, c.g, c.b, c.a] });
        }
        // The boxes keep their screen size: their drag handles follow the camera.
        if self.studio(cx).is_some_and(|s| !s.annotations.is_empty()) {
            self.sync_manipulator(cx);
        }
        self.viewport.update(cx, |vp, cx| vp.set_plane_labels(fills, cx));
        // Graphics preferences (Camera and render options), in the dialog place.
        if let Some((d, _)) = &self.ui.graphics_dialog {
            out.push(div().absolute().top(px(3.)).left(px(0.)).child(d.clone()).into_any_element());
        }
        out
    }

    /// The feature dialog. It sits between the two overlay slices rather
    /// than in one, so a keystroke or caret blink in its fields leaves them
    /// cached.
    fn render_dialog_layer(&self) -> Option<AnyElement> {
        let e = self.edit.as_ref()?;
        // Live (dom/dialog-sketch-text): the Text dialog takes the sketch
        // dialog's place while open.
        if self.sketch.as_ref().is_some_and(|s| s.text_dialog_open()) {
            return None;
        }
        // Moved dialogs keep their place (Onshape lets you drag them).
        let d = div().absolute().child(e.dialog.clone());
        // Onshape: at the viewport's left edge, 3 px under the toolbar.
        let d = match self.ui.dialog_pos {
            Some(p) => d.left(p.x).top(p.y),
            None => d.top(px(3.)).left(px(0.)),
        };
        Some(d.into_any_element())
    }

    /// What is drawn over the feature dialog.
    fn render_viewport_overlay(&mut self, window: &Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let mut out = Vec::new();
        // View cube / quick views.
        out.push(self.render_view_controls(cx));
        if let Some(nv) = self.render_named_views(cx) {
            out.push(crate::ui::moved_panel(nv, self.ui.panels.get("ps-named-views")));
        }
        // Message toast.
        // The tool-miss toast belongs to the open Remove / Intersect dialog.
        if let Some((st, msg)) = self.message.clone().filter(|(_, m)| self.edit.is_some() || m != behaviors::NO_INTERSECTION) {
            // Info messages close by themselves after 5 s however they were
            // set (live: message bubbles auto-dismiss after 5000 ms; qa5 C5 /
            // SM10); errors and warnings stay until closed.
            let key = (st, msg.clone());
            if st == FeatureStatus::Info && self.ui.message_timed.as_ref() != Some(&key) {
                self.ui.message_timed = Some(key.clone());
                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_secs(5)).await;
                    let _ = this.update(cx, |this, cx| {
                        if this.message.as_ref() == Some(&key) {
                            this.message = None;
                            cx.notify();
                        }
                        if this.ui.message_timed.as_ref() == Some(&key) {
                            this.ui.message_timed = None;
                        }
                    });
                })
                .detach();
            }
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
            // Live (document/01; behaviours "F19", configure toast: 814,85
            // and 769,85 at 1920): at y 85 of the window, centered on the
            // window, not the graphics area, and over a right panel (qa5 C5).
            let w = window.viewport_size().width;
            out.push(
                deferred(anchored().position_mode(gpui_kit::AnchoredPositionMode::Window).position(point(px(0.), px(85.))).child(div().w(w).flex().justify_center().child(bubble)))
                    .with_priority(1)
                    .into_any_element(),
            );
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
        // Onshape: the view cube sits 14 px below the viewport's top and 15 px
        // from its right (live part-studio/53, pixel-matched); view and render
        // options live in its dropdown menu (§3.7).
        let cube = self.render_view_cube(cx);
        div().absolute().top(px(14.)).right(px(15.)).child(cube).into_any_element()
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
            .text_color(gpui_kit::rgb_static(0xffffff))
            .child("Rollback bar"))
    }
}

#[derive(Clone)]
struct FeatureDrag {
    id: FeatureId,
    name: SharedString,
}

/// A feature drag's badge: the "Rollback bar" pill with the feature's name.
struct FeatureDragPill(Point<Pixels>, SharedString);

impl Render for FeatureDragPill {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
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
            .text_color(gpui_kit::rgb_static(0xffffff))
            .child(crate::ui::user_text(self.1.clone())))
    }
}

// The feature that made an edge or vertex (shared with the agent API).
use noshape_ops::studio::edge_creator;

/// Whether a picked target fits a query filter.
/// The points a sketch offers a point field (Hole locations): its sketch
/// points, else every vertex (curve ends and centers).
pub(super) fn sketch_points(sk: &noshape_model::display::SketchDisplay) -> Vec<Reference> {
    // Projected points (the origin an inference referenced) aren't the
    // sketch's vertices (qa4 F10: live places one hole per drawn point).
    let own = || sk.points.iter().filter(|p| !p.external);
    let standalone: Vec<Reference> = own().filter(|p| p.standalone).map(|p| Reference::SketchEntity { sketch: sk.feature, entity: p.entity }).collect();
    if !standalone.is_empty() {
        return standalone;
    }
    own().filter(|p| !p.handle).map(|p| Reference::SketchEntity { sketch: sk.feature, entity: p.entity }).collect()
}

/// The model-edge filter bits (edges, part-only edges, linear / circular
/// edges). A sketch curve needs SKETCH_EDGE: fields the std marks
/// `SketchObject.NO` (Fillet, Chamfer: SOLID_EDGE) don't take one (qa5 SEL1).
const MODEL_EDGES: QueryFilter = QueryFilter(QueryFilter::EDGE.0 | QueryFilter::SOLID_EDGE.0 | QueryFilter::LINEAR_EDGE.0 | QueryFilter::CIRCULAR_EDGE.0);

pub fn kind_matches(kind: PickKind, r: &Reference, f: QueryFilter) -> bool {
    // A parts field that also takes mate connectors or point bodies
    // (Transform, Pattern, Mirror entities) takes a clicked face, edge or
    // vertex as its part, not as an implicit mate connector (Onshape:
    // part-studio/445 reads "Part 1").
    let part_geometry = matches!(kind, PickKind::Face | PickKind::Edge | PickKind::Vertex) && f.intersects(QueryFilter::BODY);
    let f = if part_geometry { QueryFilter(f.0 & !(QueryFilter::MATE_CONNECTOR.0 | QueryFilter::POINT_BODY.0)) } else { f };
    // A field of sketch points or mate connectors (Hole locations) takes
    // explicit mate connectors only: a face or a part vertex is neither
    // highlighted nor added (live "qa4 round 5 s1 follow-up 2" (1)).
    let point_field = matches!(kind, PickKind::Face | PickKind::Vertex) && f.intersects(QueryFilter::SKETCH_POINT);
    let f = if point_field { QueryFilter(f.0 & !QueryFilter::MATE_CONNECTOR.0) } else { f };
    let want = |q: QueryFilter| f.intersects(q);
    match kind {
        PickKind::Face => want(QueryFilter::ANY_FACE) || want(QueryFilter::PLANE_LIKE),
        // Edge bits only: AXIS also holds cylindrical faces (their axis), so
        // a face field would take edges (qa5 I2: a hole wall in Move face).
        PickKind::Edge => want(MODEL_EDGES),
        // A part vertex isn't a sketch point (Hole locations take neither it nor a face).
        PickKind::Vertex => want(QueryFilter(QueryFilter::POINT_LIKE.0 & !QueryFilter::SKETCH_POINT.0)),
        PickKind::Plane => want(QueryFilter::PLANE),
        PickKind::Origin => want(QueryFilter::POINT_LIKE),
        PickKind::SketchCurve => want(QueryFilter::SKETCH_EDGE),
        PickKind::SketchPoint => want(QueryFilter::SKETCH_POINT) || want(QueryFilter::POINT_LIKE),
        PickKind::SketchRegion => want(QueryFilter::SKETCH_REGION),
        PickKind::Connector => want(QueryFilter::MATE_CONNECTOR),
    }
    .then_some(())
    .is_some()
        && !matches!(r, Reference::Body(_) if !want(QueryFilter::BODY))
}

impl PartStudioView {
    /// The Helix tab for a preselection: a line, linear edge or mate
    /// connector → Axis; a circle or arc → Circle; a face → Cylinder/Cone.
    fn helix_type_for(&self, sel: &[Reference]) -> Option<&'static str> {
        use noshape_sketch::Curve2;
        let ctx = self.context.as_ref()?;
        let sketch_kind = |c: &Curve2| match c {
            Curve2::Line { .. } => Some("AXIS"),
            Curve2::Circle { .. } | Curve2::Arc { .. } => Some("CIRCLE"),
            _ => None,
        };
        sel.iter().find_map(|r| match r {
            Reference::Face(_) => Some("SURFACE"),
            Reference::MateConnector { .. } => Some("AXIS"),
            Reference::Edge(_) => match noshape_model::regen::measure::resolve_shapes(ctx, r).into_iter().next()?.edge_curve().ok()? {
                noshape_kernel::Curve::Line { .. } => Some("AXIS"),
                noshape_kernel::Curve::Circle { .. } => Some("CIRCLE"),
                _ => None,
            },
            Reference::SketchEntity { sketch, entity } => sketch_kind(&ctx.sketch(*sketch)?.sketch.curve(noshape_sketch::EntityId(*entity))?),
            Reference::Sketch(sketch) => {
                let sk = &ctx.sketch(*sketch)?.sketch;
                sk.entities().filter(|e| !e.construction).find_map(|e| sk.curve(e.id).and_then(|c| sketch_kind(&c)))
            }
            _ => None,
        })
    }
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


/// A label laid out for [`in_plane_contours`]: its width (in ascender
/// units) and glyph outlines.
pub(super) struct PlaneText {
    pub width: f64,
    pub polylines: Vec<Vec<glam::DVec2>>,
}

/// [`PlaneText`] of `text`, laid out once: the plane and sketch names are
/// drawn every frame (a keystroke anywhere re-renders the Part Studio), and
/// laying out the font outlines each time was a measurable part of a frame.
/// Text drawn *in a plane* like Onshape's plane and sketch names: glyph
/// outlines laid out from the corner `top_left` (inside, along `x`, with
/// `y` up) with ascender height `asc`, as contours in world space (drawn
/// into the 3D frame).
fn in_plane_contours(text: &str, top_left: glam::DVec3, x: glam::DVec3, y: glam::DVec3, asc: f64, out: &mut Vec<Vec<glam::DVec3>>) {
    let layout = plane_text(text);
    let base = top_left + x * (0.25 * asc) - y * (1.3 * asc);
    out.extend(layout.polylines.iter().filter(|c| c.len() >= 3).map(|c| c.iter().map(|q| base + x * (q.x * asc) + y * (q.y * asc)).collect()));
}

pub(super) fn plane_text(text: &str) -> std::rc::Rc<PlaneText> {
    thread_local! {
        static CACHE: std::cell::RefCell<std::collections::HashMap<String, std::rc::Rc<PlaneText>>> = Default::default();
    }
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some(t) = c.get(text) {
            return t.clone();
        }
        // Names are few; renames could grow it without bound.
        if c.len() > 512 {
            c.clear();
        }
        let layout = noshape_sketch::text::layout_text(text, noshape_sketch::text::DEFAULT_FONT, false, false);
        let t = std::rc::Rc::new(PlaneText { width: layout.width, polylines: layout.polylines(6) });
        c.insert(text.to_string(), t.clone());
        t
    })
}

/// A feature list level's indent (live qa4 r5 s8 (6): 19 px).
const FOLDER_INDENT: f32 = 19.;

/// A list item's indent (feature and folder rows in folders).
fn item_indent(item: &ListItem) -> f32 {
    match item {
        ListItem::Feature(x, _) | ListItem::Folder(x, _) => *x,
        _ => 0.,
    }
}

/// What the features list shows, in order (`render_feature_list`).
enum ListItem {
    Placeholder,
    Rollback(usize),
    Ellipsis,
    /// Indent, folder.
    Folder(f32, noshape_model::partstudio::Folder),
    /// Indent, feature.
    Feature(f32, FeatureId),
    /// Indent, a sketch a Derived feature brought in (its source id).
    /// Indent, sketch, which location's copy.
    DerivedSketch(f32, FeatureId, usize),
    Prompt,
}

/// What a feature row reads from the list as a whole.
#[derive(Default)]
struct RowCtx {
    rollback: usize,
    editing_id: Option<FeatureId>,
    related: std::collections::HashSet<FeatureId>,
    /// Gathered once per list instead of per row (each was a scan of the
    /// display or the model): features whose faces are in a part, sketches,
    /// features with a plane, features with an appearance.
    makes_faces: std::collections::HashSet<FeatureId>,
    sketches: std::collections::HashSet<FeatureId>,
    planes: std::collections::HashSet<FeatureId>,
    appearances: std::collections::HashSet<FeatureId>,
    first_config_input: Option<String>,
}

/// Model results the features list and its head read every frame, kept
/// until the document changes (any `DocEvent`): the filter's matches (also
/// keyed by the query and the display: a weak pointer, so a new display
/// never reuses the address of the one this was worked out for) and the
/// contexts the features reference.
#[derive(Default)]
struct ListCache {
    filter: Option<(String, std::sync::Weak<StudioDisplay>, FilterMatches)>,
    contexts: Option<std::rc::Rc<Vec<u32>>>,
}

/// The features a filter matches (None: no filter).
type FilterMatches = Option<std::rc::Rc<std::collections::HashSet<FeatureId>>>;

/// The Part Studio's cached slices (see `crate::cached_slice`): a keystroke
/// or a caret blink in a field re-renders the studio but reuses these.
#[derive(Clone)]
struct PsSlices {
    toolbar: Entity<crate::cached_slice::Slice<PartStudioView>>,
    /// Under the feature dialog.
    underlay: Entity<crate::cached_slice::Slice<PartStudioView>>,
    /// Over it.
    overlay: Entity<crate::cached_slice::Slice<PartStudioView>>,
    /// The features list's rows (the filter notifies it when the query
    /// changes).
    features: Entity<crate::cached_slice::Slice<PartStudioView>>,
    parts: Entity<crate::cached_slice::Slice<PartStudioView>>,
}

impl PartStudioView {
    fn slices(&mut self, cx: &mut Context<Self>) -> PsSlices {
        if let Some(s) = &self.slices {
            return s.clone();
        }
        use crate::cached_slice::Slice;
        let me = cx.entity();
        let toolbar = Slice::new(&me, |this, window, cx| {
            let viewport_w = f32::from(window.viewport_size().width);
            // A new sketch's dialog shows the (disabled) sketch toolbar before
            // its plane is picked (live sketch/00).
            let sketch_dialog = this.edit.as_ref().is_some_and(|e| e.dialog.read(cx).feature.type_id == "sketch");
            if this.sketch.is_some() || sketch_dialog { this.render_sketch_toolbar(viewport_w, cx) } else { this.render_toolbar(viewport_w, cx) }
        }, cx);
        let underlay = Slice::new(&me, |this, window, cx| this.render_underlay_slice(window, cx), cx);
        let overlay = Slice::new(&me, |this, window, cx| this.render_overlay_slice(window, cx), cx);
        let features = Slice::new(&me, |this, window, cx| this.render_feature_list(window, cx), cx);
        let parts = Slice::new(&me, |this, _, cx| this.render_parts_list(cx), cx);
        // Labels, the view cube and manipulators follow the camera, and the
        // lists the highlights; all read the document (undo state,
        // features) besides the studio.
        for s in [&underlay, &overlay, &features, &parts] {
            Slice::observing(s, &self.viewport, cx);
            Slice::observing(s, &self.doc, cx);
        }
        Slice::observing(&toolbar, &self.doc, cx);
        let s = PsSlices { toolbar, underlay, overlay, features, parts };
        self.slices = Some(s.clone());
        s
    }

    /// The features the list's filter matches (None: no filter). The list
    /// head, the rows and the filter box's change handler all ask, each
    /// frame: worked out once per query, display and document state.
    fn filtered(&self, cx: &App) -> FilterMatches {
        let mut c = self.list_cache.borrow_mut();
        if let Some((q, d, set)) = &c.filter
            && *q == self.ui.filter_query
            && std::ptr::eq(d.as_ptr(), Arc::as_ptr(&self.display))
        {
            return set.clone();
        }
        let set = self.studio(cx).and_then(|s| noshape_model::interaction::filter_features(s, &self.display, &self.ui.filter_query)).map(std::rc::Rc::new);
        c.filter = Some((self.ui.filter_query.clone(), Arc::downgrade(&self.display), set.clone()));
        set
    }

    /// The assembly contexts the features reference (a walk over every
    /// feature's parameters), once per document state.
    pub(super) fn referenced_contexts(&self, cx: &App) -> std::rc::Rc<Vec<u32>> {
        let mut c = self.list_cache.borrow_mut();
        c.contexts.get_or_insert_with(|| std::rc::Rc::new(self.studio(cx).map(|s| noshape_model::in_context::referenced_contexts(&s.features)).unwrap_or_default())).clone()
    }

    /// The filter query changed: of the cached slices only the features
    /// list's rows show it, and only which features match (the head
    /// re-renders with the filter box anyway).
    fn filter_rows_changed(&mut self, cx: &mut Context<Self>) {
        let shown = self.filtered(cx);
        if shown != self.filter_rows
            && let Some(s) = &self.slices
        {
            s.features.update(cx, |_, cx| cx.notify());
        }
    }

    /// What is drawn over the graphics area under the feature dialog (live:
    /// the dialog hides sketch names and the list toggle).
    fn render_underlay_slice(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        // Live: centered 8 px above the viewport's middle, like the right strip.
        let mut underlay = vec![div().absolute().top_0().bottom(px(16.)).left_0().flex().items_center().child(self.render_list_toggle(cx)).into_any_element()];
        underlay.extend(self.render_sketch_labels(window, cx));
        underlay.extend(self.render_viewport_underlay(cx));
        // Live ("Panels vs feature dialogs"): Isolate stays open under a
        // feature dialog that opens in the same top-left spot.
        underlay.extend(self.render_isolate_panel(cx));
        div().size_full().relative().children(underlay).into_any_element()
    }

    /// What is drawn over the graphics area and the feature dialog.
    fn render_overlay_slice(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mut overlay = self.render_viewport_overlay(window, cx);
        // Live (part-studio/00, 77): the strip is centered 8 px above the
        // viewport's middle (4 or 5 buttons alike).
        overlay.push(div().absolute().top_0().bottom(px(16.)).right_0().flex().items_center().child(self.render_panel_toggles(cx)).into_any_element());
        // Live (sketch/01, part-studio/230, 234): also while a sketch or a
        // feature dialog is open.
        overlay.push(self.render_evaluation_tools(cx));
        // Live (part-studio/82): the Measure panel covers those buttons.
        overlay.extend(self.render_measure_panel(cx));
        overlay.extend(self.render_context_menu());
        overlay.extend(self.render_section_panel(cx).map(|e| crate::ui::moved_panel(e, self.ui.panels.get("ps-section"))));
        // Value boxes at model points are drawn in the 3D frame
        // (NOSHAPE_GPUI_LABELS: as GPUI elements, for comparing).
        // The offset box takes a typed value (its click target / field).
        overlay.extend(self.section_value_field(cx));
        overlay.extend(self.render_center_of_mass(cx));
        if crate::overlay_labels::gpui_labels() {
            overlay.extend(self.render_section_value(cx).filter(|_| self.ui.section_offset_edit.is_none()));
        } else {
            let mut labels: Vec<_> = self.section_value_label(window, cx).into_iter().collect();
            labels.extend(self.analysis_labels(window, cx));
            labels.extend(self.sketch_dimension_overlay(window, cx));
            self.viewport.update(cx, |vp, cx| vp.set_overlay_labels(labels, cx));
        }
        overlay.extend(self.render_palette(window, cx));
        overlay.extend(self.render_shortcut_sheet(window, cx));
        overlay.extend(self.render_create_selection(cx));
        overlay.extend(self.render_material_panel(cx).map(|e| crate::ui::moved_panel(e, self.ui.panels.get("ps-material"))));
        overlay.extend(self.render_variable_input(cx));
        overlay.extend(self.render_datum_dialog(cx));
        overlay.extend(self.render_default_tolerances_picker(cx));
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
        div().size_full().relative().children(overlay).into_any_element()
    }
}

impl Render for PartStudioView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        // What GPUI still draws that follows the camera (perf: otherwise camera
        // moves are only presented): the sketch's boxes, glyphs and counts
        // (not its dimension labels at rest or its name), shown dimensions,
        // analysis and section labels, inspection datums (sized by the
        // projected pixel, built in the underlay).
        let follows = (self.sketch.is_some() && self.ui.sketch_follows_camera) || self.ui.shown_dims.is_some() || ((self.ui.analysis_tool.is_some() || self.ui.section_dialog.is_some()) && crate::overlay_labels::gpui_labels()) || self.studio(cx).is_some_and(|s| !s.annotations.is_empty());
        self.viewport.update(cx, |vp, _| vp.set_ui_follows_camera(follows));
        if self.compare_gfx.changed(&self.doc, cx) {
            self.refresh_scene(false, cx);
        }
        self.ensure_filter_input(window, cx);
        self.ensure_config_box_inputs(window, cx);
        // A feature row's context menu closed: drop its highlight.
        if self.ui.list_menu.is_some() && !crate::os_menu::is_open(cx) {
            self.ui.list_menu = None;
            self.refresh_scene(false, cx);
        }
        let slices = self.slices(cx);
        let list_head = self.render_feature_list_head(cx);
        if self.needs_regen {
            self.needs_regen = false;
            self.schedule_regen(window, cx);
        }
        let right = self.render_right_panel(window, cx);
        let tb_panel = self.render_toolbar_customize_panel(window, cx);
        let appearance_editor = self.render_appearance_editor(window, cx).map(|e| crate::ui::moved_panel(e, self.ui.panels.get("ps-appearance")));
        let in_sketch = self.sketch.is_some();
        let root = v_flex()
            .size_full()
            .relative()
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
                // then isolate / make transparent; never the selection.
                if this.show_shortcuts {
                    this.toggle_shortcut_sheet(window, cx);
                    return;
                }
                if this.close_insert_go(cx) {
                    return;
                }
                // An active Inspection panel tool ends.
                if this.ui.inspection_tool.take().is_some() {
                    this.ui.datum_edit = None;
                    this.redraw_annotations(cx);
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
                } else if this.close_tool_panel(window, cx) {
                } else {
                    // Live ("nav2 selection" (4)): Esc never clears the
                    // selection in the graphics (Space does).
                    this.exit_isolate(window, cx);
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
                this.set_show_mass(false, cx);
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
                } else if let [Reference::Body(b)] = this.selection.as_slice() {
                    // Shift+N on a selected part renames it (qa5 KB1).
                    let b = b.clone();
                    this.start_part_rename(b, window, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &Extrude, window, cx| this.shortcut_feature("extrude", window, cx)))
            .on_action(cx.listener(|this, _: &Revolve, window, cx| this.shortcut_feature("revolve", window, cx)))
            .on_action(cx.listener(|this, _: &Fillet, window, cx| this.shortcut_feature("fillet", window, cx)))
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
            // Select other takes ↑ / ↓ first (Onshape listenToFirst KEYDOWN).
            .on_action(cx.listener(|this, a: &RotateUp, _, cx| if this.ui.select_other.is_some() { this.select_other_step(true, cx) } else { this.rotate_by(a, 0.0, -15.0, cx) }))
            .on_action(cx.listener(|this, a: &RotateDown, _, cx| if this.ui.select_other.is_some() { this.select_other_step(false, cx) } else { this.rotate_by(a, 0.0, 15.0, cx) }))
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
            // Customizing: drawn every frame (a dragged tool leaves the bar
            // while the drag runs, and nothing notifies the cached slice).
            .child(if self.ui.tb_custom.is_some() {
                div().w_full().h(px(crate::ui::TOOLBAR_H)).flex_none().child(slices.toolbar.clone()).into_any_element()
            } else {
                slices.toolbar.clone().cached(StyleRefinement::default().w_full().h(px(crate::ui::TOOLBAR_H)).flex_none()).into_any_element()
            })
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .on_drag_move(cx.listener(|_, e: &DragMoveEvent<crate::ui::LeftSplitDrag>, _, cx| {
                        if crate::ui::drag_left_panel(e) {
                            cx.notify();
                        }
                    }))
                    // Compare mode (compare_mode.rs, live ui2/953): the compare
                    // panel takes the Features and parts list's place.
                    .child(if let Some(panel) = crate::compare_mode::render_panel(&self.doc, self.tab, cx) {
                        panel
                    } else if self.ui.list_collapsed {
                        // Collapsed Features and parts list: Onshape collapses it
                        // to nothing; the toggle tab at the viewport's left edge reopens it.
                        div().into_any_element()
                    } else {
                        div()
                            .id("list-panel")
                            .w(px(crate::ui::left_panel_w()))
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
                            .child(
                                v_flex()
                                    .size_full()
                                    .bg(pal.panel)
                                    .border_r_1()
                                    .border_t_1()
                                    .border_b_1()
                                    .border_color(pal.border)
                                    .children(list_head)
                                    .child(slices.features.clone().cached(StyleRefinement::default().w_full().flex_1().min_h_0()))
                                    // Drag the separator to share space between features and parts.
                                    // Live (part-studio/00): a 1 px #d7d7d7 line over the list's
                                    // 200 px (5 px short of the panel border), the "Parts (N)" row
                                    // right under it; the drag target is 5 px around it.
                                    .child(
                                        div().h(px(1.)).mr(px(5.)).flex_none().relative().bg(pal.border).child(
                                            div()
                                                .id("parts-separator")
                                                .absolute()
                                                .left_0()
                                                .right_0()
                                                .top(px(-2.))
                                                .h(px(5.))
                                                // Live: `div.os-splitter` is row-resize.
                                                .cursor(CursorStyle::ResizeRow)
                                                .on_drag(SeparatorDrag, |_, _, _, cx| cx.new(|_| SeparatorDrag)),
                                        ),
                                    )
                                    // Live (part-studio/00, 220): the parts take the lower 35% of the
                                    // panel until the separator is dragged. A share of the panel, so
                                    // both sections keep room when the panel gets shorter (the
                                    // FeatureScript notices panel under the tab bar).
                                    .child(slices.parts.clone().cached(StyleRefinement::default().w_full().h(relative(self.ui.parts_height.unwrap_or(0.35))).min_h(px(30.)).flex_none())),
                            )
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
                            // Resizing the dialog by its right edge (width only, min 220).
                            .on_drag_move(cx.listener(|this, e: &DragMoveEvent<crate::feature_dialog::DialogResize>, _, cx| {
                                let origin = this.ui.dialog_pos.unwrap_or(point(px(0.), px(3.)));
                                let p = e.event.position - e.bounds.origin - origin;
                                let width = p.x.max(px(220.)).round();
                                let changed = this.ui.dialog_width != Some(width);
                                this.ui.dialog_width = Some(width);
                                if let Some(ed) = &this.edit {
                                    ed.dialog.update(cx, |d, cx| {
                                        d.width = Some(width);
                                        // Live writes the width as the edge moves, per feature type.
                                        if changed {
                                            let mut prefs = crate::preferences_window::ListPrefs::load();
                                            prefs.feature_dialog_widths.insert(d.feature.type_id.clone(), f32::from(width) as u32);
                                            prefs.save();
                                        }
                                        cx.notify();
                                    });
                                }
                                cx.notify();
                            }))
                            .on_drag_move(cx.listener(|this, e: &DragMoveEvent<create_selection::CsMove>, _, cx| this.cs_drag(e, cx)))
                            .on_drag_move(cx.listener(|this, e: &DragMoveEvent<crate::ui::PanelMove>, _, cx| {
                                this.ui.panels.drag(e, cx);
                                cx.notify();
                            }))
                            .on_drag_move(cx.listener(|this, e: &DragMoveEvent<crate::feature_dialog::DialogMove>, _, cx| {
                                let grab = *e.drag(cx).0.lock();
                                let p = e.event.position - e.bounds.origin - grab;
                                let max_x = (e.bounds.size.width - px(80.)).max(px(0.));
                                let max_y = (e.bounds.size.height - px(40.)).max(px(0.));
                                this.ui.dialog_pos = Some(point(p.x.clamp(px(0.), max_x), p.y.clamp(px(0.), max_y)));
                                cx.notify();
                            }))
                            .child(self.viewport.clone())
                            // Under the dialogs and the list toggle, as live.
                            .when(!self.ui.list_collapsed, |d| d.child(crate::ui::left_splitter()))
                            .child(slices.underlay.clone().cached(StyleRefinement::default().absolute().top_0().left_0().size_full()))
                            .children(self.render_dialog_layer())
                            // Text fields stay out of the cached slices (qa4).
                            .children(appearance_editor)
                            .child(slices.overlay.clone().cached(StyleRefinement::default().absolute().top_0().left_0().size_full()))
                            // Slow regeneration: the working overlay (pending.rs).
                            .children(self.render_pending_overlay(cx))
                            // Compare mode: the Main ○—— V3 blend slider, top centre (live 953).
                            .children(crate::compare_mode::render_slider(&self.doc, cx).map(|slider| div().absolute().top(px(4.)).left_0().right_0().flex().justify_center().child(slider))),
                    )
                    // Versions and history › View in repair (repair_mode.rs, live
                    // rp-906): the 567 px Repair panel right of the graphics; its
                    // view-only second view comes from perf.
                    .children(crate::repair_mode::render_panel(&self.doc, None, cx))
                    .children(right),
            )
            .children(tb_panel);
        self.with_sketch_actions(root, cx)
    }
}

impl PartStudioView {
    /// The FeatureScript notices panel's "Result:" line for this Part Studio.
    /// Live (qa5 FS5): "Regeneration complete" also when features fail (their
    /// errors are the notices above it); a regeneration here always completes.
    pub fn regen_result(&self) -> String {
        "Result: Regeneration complete".to_string()
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
        let primitives = [("Faces", sum(|b| b.face_refs.len())), ("Edges", sum(|b| b.mesh.edges.iter().filter(|p| !p.is_empty()).count())), ("Vertices", sum(|b| b.vertex_refs.len())), ("Sketch entities", sketch), ("Planes", d.planes.len())];
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

/// What a sketch entity is, for selection labels ("Vertex of Sketch 1"): from the
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

/// Entities picked in a feature's query parameters (array items included).
fn count_picks(p: &noshape_model::params::ParamValues) -> usize {
    p.0.values()
        .map(|v| match v {
            noshape_model::params::ParamValue::Query(q) => q.len(),
            noshape_model::params::ParamValue::Array(items) => items.iter().map(count_picks).sum(),
            _ => 0,
        })
        .sum()
}

/// A feature's name as Onshape shows it in tooltips and menus: with its
/// type in brackets unless the name contains the type, case-insensitively
/// (the bundle: `-1 === (" " + name.toLowerCase() + " ").indexOf(type
/// .toLowerCase())`), the type as named in the UI language. Live: "Mount
/// [Extrude]", "<- Path [Sketch]", but "<- Boss sketch 2" (qa4 r5 s8);
/// German "Sketch 1 [Skizze]"; default names ("Draft 1") stay as they are.
pub(crate) fn typed_name(f: &noshape_model::partstudio::Feature) -> String {
    match f.feature_type().map(|t| crate::i18n::type_name(t.name)) {
        Some(t) if needs_type_tag(&f.name, &t) => format!("{} [{t}]", f.name),
        _ => f.name.clone(),
    }
}

/// Whether a name gets its feature type in brackets (see [`typed_name`]).
fn needs_type_tag(name: &str, type_name: &str) -> bool {
    !type_name.is_empty() && !format!(" {} ", name.to_lowercase()).contains(&type_name.to_lowercase())
}

#[cfg(test)]
mod type_tag_tests {
    use super::needs_type_tag;
    use std::prelude::v1::test;

    #[test]
    fn the_type_tag_shows_unless_the_name_contains_the_type() {
        // Live: "Boss 2 [Extrude] <- Boss sketch 2" (qa4 r5 s8), "<- Path [Sketch]",
        // "Move L5 rotate no axis [Move face]", "Sketch 1".
        assert!(needs_type_tag("Boss 2", "Extrude"));
        assert!(!needs_type_tag("Boss sketch 2", "Sketch"));
        assert!(needs_type_tag("Path", "Sketch"));
        assert!(needs_type_tag("Move L5 rotate no axis", "Move face"));
        assert!(!needs_type_tag("Sketch 1", "Sketch"));
        assert!(!needs_type_tag("Draft 1", ""));
    }
}

/// A feature's listed name with the dialog's shown values: computed ones
/// first, else lengths in the workspace units ("Ø 6.35 mm ↧ 12 mm" for a
/// Drilled 1/4 hole in a mm document).
fn listed_name(f: &Feature, computed: &[(String, String)], units: &noshape_units::UnitPrefs, vars: &std::collections::BTreeMap<String, noshape_units::Quantity>) -> String {
    noshape_model::interaction::display_name_computed(f, &|k| computed.iter().find(|(c, _)| c == k).map(|(_, v)| v.clone()).or_else(|| crate::feature_dialog::shown_length(f, k, units, vars)))
}

#[cfg(test)]
mod listed_name_tests {
    use super::listed_name;
    use noshape_model::partstudio::PartStudio;

    /// qa5 P1-9: the name chips use for a Hole is its callout, as its row's
    /// (live: Feature pattern › "Ø 6.35 mm ↧ 12 mm", not "Hole 1").
    #[::core::prelude::v1::test]
    fn a_hole_is_named_by_its_callout() {
        let units = noshape_units::UnitPrefs::default();
        let mut f = PartStudio::new().new_feature("hole", &units).unwrap();
        f.name = "Hole 1".into();
        let name = listed_name(&f, &[], &units, &Default::default());
        assert!(name.starts_with("Ø 6.35 mm"), "{name}");
        assert_ne!(name, "Hole 1");
    }
}

/// Whether a feature row gets the yellow ⚠ badge on its icon: a warning, or
/// any unresolved selection, also on a failed (red) feature (live qa5/p7
/// "Chamfer 1", behaviours 8292: "Rows with an unresolved selection get a ⚠
/// warning-triangle icon on the row (in addition to the red text)").
fn warning_badge(status: FeatureStatus, missing: usize, rolled_back: bool) -> bool {
    !rolled_back && (status == FeatureStatus::Warning || missing > 0)
}

#[cfg(test)]
mod warning_badge_tests {
    use super::{warning_badge, FeatureStatus};

    /// qa5 P1-18: a feature that fails on a missing selection keeps the
    /// badge; a plain error (red name only, part-studio/94) doesn't.
    #[::core::prelude::v1::test]
    fn missing_selection_rows_get_the_warning_badge() {
        assert!(warning_badge(FeatureStatus::Error, 1, false));
        assert!(warning_badge(FeatureStatus::Warning, 0, false));
        assert!(!warning_badge(FeatureStatus::Error, 0, false));
        assert!(!warning_badge(FeatureStatus::Ok, 0, false));
        assert!(!warning_badge(FeatureStatus::Error, 1, true));
    }
}

/// The "Features (N)" count: the four default geometry rows, the features,
/// and a new feature whose dialog is open (live sketch/501, qa5 P1-1).
fn listed_feature_count(features: usize, new_dialog_open: bool) -> usize {
    features + 4 + usize::from(new_dialog_open)
}

#[cfg(test)]
mod feature_count_tests {
    use super::listed_feature_count;
    use std::prelude::v1::test;

    /// Live sketch/501: a new Part Studio with Sketch 1's dialog open reads
    /// "Features (5)"; the new feature counts before it is accepted.
    #[test]
    fn features_header_counts_the_new_feature() {
        assert_eq!(listed_feature_count(0, false), 4);
        assert_eq!(listed_feature_count(0, true), 5);
        assert_eq!(listed_feature_count(7, true), 12);
    }
}

/// A Default geometry row drawn gray: hidden, or any while parts are
/// isolated (Isolate takes the planes away; live qa5 KB3, ui/pn-900).
fn default_row_dimmed(hidden: bool, isolate: Option<&behaviors::Isolation>) -> bool {
    hidden || isolate.is_some_and(|i| i.mode == behaviors::IsolateMode::Isolate)
}

#[cfg(test)]
mod default_row_tests {
    use super::*;
    use std::prelude::v1::test;

    #[test]
    fn default_geometry_rows_gray_while_isolating() {
        let iso = |mode| behaviors::Isolation { mode, bodies: Vec::new(), opacity: 0.0, select_transparent: false, expand: 0 };
        assert!(!default_row_dimmed(false, None));
        assert!(default_row_dimmed(true, None));
        assert!(default_row_dimmed(false, Some(&iso(behaviors::IsolateMode::Isolate))));
        // Make transparent keeps the planes, so their rows stay black.
        assert!(!default_row_dimmed(false, Some(&iso(behaviors::IsolateMode::Transparent))));
    }

    /// qa5 SEL1: a sketch curve goes only into fields that take sketch
    /// curves (fillet.fs / chamfer.fs say `SketchObject.NO`); a model edge
    /// only into fields that take model edges.
    #[test]
    fn sketch_curves_and_model_edges_pick_into_their_own_fields() {
        let r = Reference::SketchEntity { sketch: FeatureId(1), entity: 0 };
        let field = |ty: &str, id: &str| match noshape_model::features::get(ty).and_then(|t| t.params.iter().find(|p| p.id == id)).map(|p| &p.kind) {
            Some(noshape_model::params::ParamKind::Query { filter, .. }) => *filter,
            _ => panic!("{ty}.{id} is not a query field"),
        };
        for (ty, id) in [("fillet", "entities"), ("chamfer", "entities")] {
            assert!(!kind_matches(PickKind::SketchCurve, &r, field(ty, id)), "{ty}.{id} takes a sketch curve");
            assert!(kind_matches(PickKind::Edge, &r, field(ty, id)), "{ty}.{id} refuses a model edge");
        }
        assert!(kind_matches(PickKind::SketchCurve, &r, field("helix", "edge")));
        assert!(!kind_matches(PickKind::Edge, &r, QueryFilter::SKETCH_EDGE));
    }
}

/// A pattern instance point's diameter (live p1audit skip-02: 12 px, a
/// 3 px ring).
const TOGGLE_POINT_PX: f32 = 12.0;
