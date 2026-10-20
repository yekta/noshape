//! Assembly tab: toolbar, instances + mate features list, viewport with
//! per-instance transforms, selection, implicit mate connector inference on
//! hover, mate / relation / mate connector / pattern dialogs, insert panel
//! with place-on-click, triad manipulator (move, rotate, typed values),
//! snap mode, show-mates mode, exploded views with a step editor, named
//! positions, per-instance configurations, replace instance, mate
//! animation, interference check and BOM. Panels live in `assembly_panels.rs`.
//!
//! Evaluation (Part Studio regeneration + solve) runs in a background task;
//! drags and animation re-solve the cached [`Prepared`] problem off the UI
//! thread and only update instance transforms. Every change commits through
//! [`DocModel::edit_tab`], folding in the latest solved placements.

use crate::actions::*;
use crate::assembly_mate_dialog::{ConnectorLabeler, MateDialog, MateDialogEvent, mate_icon};
use crate::assembly_scene::{self, AsmBuilt, AsmGpuCache, AsmPickTarget, AsmSceneOptions, Highlights, ManipHandle, OverlayPick, Triad};
use crate::assets::icon_path;
use crate::os_menu::{OsContextMenuExt as _, OsMenuItem};
use crate::doc_model::{DocEvent, DocModel};
use crate::gpu::Gpu;
use crate::navigation::{self as nav, RotateDir};
use crate::scene_builder::PickKind;
use crate::theme::{p, with_alpha};
use crate::viewport::{Viewport, ViewportEvent};
use glam::{DVec2, DVec3};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use crate::os_ui::OsDropdownExt as _;
use gpui_kit::component::{Icon, IconName, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::assembly::analysis::{self, Interference};
use noshape_model::assembly::eval::{AssemblyEvaluator, AssemblyResult, InferencePoint, InstanceNode, NodeKind, SolveOptions, frame_to_placement, inference_candidates, placement_to_frame};
use noshape_model::assembly::solver::Frame;
use noshape_model::assembly::{
    AsmMateConnector, Assembly, ExplodeStep, ExplodedView, Group, Instance, InstanceId, InstancePattern, InstanceSource, Mate, MateConnectorRef, MateId, MateType, NamedPosition, PatternKind, Placement,
    Relation, RelationType, SubassemblyLock,
};
use noshape_model::configuration::ConfigValues;
use noshape_model::display::BodyKind;
use noshape_model::document::{Document, TabContent, TabId};
use noshape_model::reference::{BodyId, FeatureId, Inference, Reference};
use noshape_render::StandardView;
use noshape_units::ExprValue;
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

/// A part offered by the Insert panel: (body, name, bounds).
/// (body, name, bounds, is a surface)
type CatalogPart = (BodyId, String, Option<(DVec3, DVec3)>, bool);
/// Inference candidates for a hovered entity: ((path, entity), occurrence index, points).
type InferCache = ((Vec<InstanceId>, Reference), usize, Vec<InferencePoint>);

/// Resolves parts inserted from other documents (version-pinned snapshots).
struct AppExternal;

impl noshape_model::assembly::eval::ExternalStudios for AppExternal {
    fn studio(&self, key: &str) -> Option<(Arc<Document>, TabId)> {
        let r = crate::derived::ExternalRef::parse(key)?;
        let doc = crate::derived::external_snapshot(&r.path, r.microversion).ok()?;
        let tab = doc.tabs.iter().find(|t| t.id.to_string() == r.tab)?.id;
        Some((doc, tab))
    }
}

/// A Part Studio or Assembly offered by the Insert panel.
#[derive(Clone, Debug)]
struct CatalogStudio {
    tab: TabId,
    name: String,
    parts: Vec<CatalogPart>,
    /// Sketches with geometry (Insert › Sketches filter).
    sketches: Vec<(FeatureId, String)>,
    /// Composite parts among `parts` (Insert › Composite parts filter).
    composites: Vec<BodyId>,
}

#[derive(Clone, Debug, Default)]
struct Catalog {
    studios: Vec<CatalogStudio>,
    assemblies: Vec<(TabId, String)>,
}

/// The floating mate toolbar being moved by its handle (grab offset).
#[derive(Clone)]
pub(crate) struct HudDrag(pub Arc<Mutex<Point<Pixels>>>);

struct HudDragGhost;

impl Render for HudDragGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

struct MateSession {
    dialog: Entity<MateDialog>,
    /// Index of the mate being edited (None for a new mate).
    editing: Option<MateId>,
    /// Solve pressed: previews solve every mate (no stationary side held).
    solve_all: bool,
    /// Heads-up checkmark position (viewport-local) once both connectors are picked.
    hud: Option<DVec2>,
    _sub: Subscription,
}

struct RelationSession {
    kind: RelationType,
    a: Option<MateId>,
    b: Option<MateId>,
    ratio: Entity<InputState>,
    reverse: bool,
    editing: Option<MateId>,
}

#[path = "assembly_panels.rs"]
mod panels;
pub(crate) use panels::*;


/// Key in `expanded` that marks the "Mate features" folder as collapsed.
const MATE_FEATURES_COLLAPSED: InstanceId = InstanceId(u64::MAX);

#[path = "assembly_behaviors.rs"]
mod behaviors;
pub(crate) use behaviors::*;
pub(crate) use props::StaleContexts;

#[path = "assembly_standard.rs"]
mod standard_content;
#[path = "assembly_thumbs.rs"]
mod thumbs;
#[path = "assembly_props.rs"]
mod props;
#[path = "assembly_search.rs"]
mod search;
#[path = "assembly_positions.rs"]
mod positions;
#[path = "assembly_bom.rs"]
mod bom_panel;
#[path = "assembly_mass.rs"]
mod mass;
#[path = "assembly_reflection.rs"]
mod reflection;
#[path = "assembly_curvature.rs"]
mod curvature;
#[path = "assembly_config.rs"]
mod config_panel;
#[path = "assembly_toolbar.rs"]
mod toolbar;
use noshape_model::assembly::config::{self as acfg, ConfigTarget};
use noshape_model::params::ParamValue;
pub(crate) use standard_content::*;

struct DragState {
    top: InstanceId,
    press: DVec2,
    started: bool,
    /// Grab point in instance coordinates, drag plane point/normal (world).
    local: DVec3,
    plane_point: DVec3,
    plane_normal: DVec3,
    pending: Option<DVec3>,
    busy: bool,
    /// Manipulator handle being dragged (translate / rotate), the instances
    /// it moves and their placements when the drag started.
    handle: Option<ManipHandle>,
    ids: Vec<InstanceId>,
    start: HashMap<InstanceId, Placement>,
    /// Ring drag: start direction from the center in the ring plane.
    ring_v0: DVec3,
    /// Current displacement along the axis (m) or angle about it (rad).
    value: f64,
    /// Plane drag: cursor hit on the drag plane at the press, and the
    /// current displacement in that plane.
    grab0: DVec3,
    offset: DVec3,
    /// Center (ball) drag: the manipulator frame being placed.
    triad_preview: Option<Frame>,
    /// Explode step editing: the drag builds a step instead of moving instances.
    explode: bool,
    /// Free drag of a part of a flexible subassembly (the subassembly's child).
    nested: Option<InstanceId>,
}

/// Snap mode drag: a connector of one instance dragged onto another's.
struct SnapDrag {
    top: InstanceId,
    press: DVec2,
    started: bool,
    /// Connectors on the dragged instance (Ctrl cycles) and the current one.
    sources: Vec<MateConnectorRef>,
    source_ix: usize,
    /// Target connector under the cursor.
    target: Option<MateConnectorRef>,
    /// Placement of the dragged instance when the drag started.
    start: Placement,
    /// Grab point (world) and camera plane normal for the free follow.
    grab: DVec3,
    normal: DVec3,
}

/// A row of the Mate features list.
struct FeatureRow {
    id: MateId,
    icon: SharedString,
    name: String,
    ok: bool,
    inactive: bool,
    message: Option<String>,
    has_limits: bool,
    /// Mate indicator hidden (None: no eye icon).
    hidden: Option<bool>,
    tooltip: Option<String>,
    /// Implicit mate connectors listed under the mate (label, connector).
    implicit: Vec<(String, MateConnectorRef)>,
}

/// Explode step edit dialog.
pub(crate) struct StepEdit {
    pub view: usize,
    pub step: usize,
    pub instances: Vec<InstanceId>,
    /// Translation X / Y / Z, rotation angle.
    pub inputs: [Entity<InputState>; 4],
    pub rotation: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum SidePanel {
    Bom,
    /// Assembly configurations (live "Configuration panel").
    Configurations,
    Interference,
    Explode,
    Positions,
    /// Onshape's Variable table (docked like the Part Studio's).
    Variables,
}

pub struct AssemblyView {
    pub tab: TabId,
    doc: Entity<DocModel>,
    gpu: Gpu,
    viewport: Entity<Viewport>,
    /// Variable Studio tables in the Variable table panel.
    var_tables: HashMap<TabId, Entity<crate::variable_studio::VariableStudioView>>,
    /// View cube hover cell and last painted origin (`view_cube`).
    cube_hover: Option<crate::part_studio::view_cube::CubeCell>,
    cube_origin: Option<Point<Pixels>>,
    evaluator: Arc<Mutex<AssemblyEvaluator>>,
    result: Arc<AssemblyResult>,
    built: Option<AsmBuilt>,
    cache: AsmGpuCache,
    eval_seq: u64,
    eval_task: Option<Task<()>>,
    evaluating: bool,
    catalog: Catalog,
    selected: Vec<InstanceId>,
    selected_mate: Option<MateId>,
    list_hover_mate: Option<MateId>,
    hover: Option<AsmPickTarget>,
    /// Hovered manipulator handle and overlay item (origin / connector).
    manip_hover: Option<ManipHandle>,
    hover_overlay: Option<OverlayPick>,
    /// Assembly connector ids in overlay pick order.
    overlay_connectors: Vec<MateId>,
    overlay_sig: Vec<f64>,
    pointer: DVec2,
    /// Inference candidates (body coordinates) for the hovered entity, and the chosen index.
    infer: Option<InferCache>,
    infer_choice: Option<usize>,
    mate: Option<MateSession>,
    relation: Option<RelationSession>,
    insert_open: bool,
    insert_rigid: bool,
    drag: Option<DragState>,
    anim: Option<AnimState>,
    panel: Option<SidePanel>,
    interference: Option<Vec<Interference>>,
    interference_running: bool,
    explode: Option<usize>,
    isolate: Option<HashSet<InstanceId>>,
    show_mates: bool,
    show_connectors: bool,
    /// Onshape behaviours state (menus, filter, rename, transparency...).
    ui: AsmUi,
    expanded: HashSet<Vec<InstanceId>>,
    message: Option<(bool, String)>,
    /// Assembly mate connector dialog.
    mc: Option<McSession>,
    /// Pattern / mirror / replicate dialog.
    pattern: Option<PatternSession>,
    /// Snap mode (Shift+S): `Some(source)` once the first connector is picked.
    snap: Option<Option<MateConnectorRef>>,
    /// Show mates mode (H): hovered (or pinned) instance shows its mates.
    show_mates_mode: bool,
    mates_pinned: Option<InstanceId>,
    /// Inserted instances following the cursor until a click places them.
    placing: Option<Vec<InstanceId>>,
    placing_point: Option<DVec3>,
    /// Typed value for the last manipulator drag.
    manip_value: Option<ManipValue>,
    /// Exploded view step editing, playback position (steps) and the preview step.
    explode_edit: bool,
    explode_progress: Option<f64>,
    explode_preview: Option<ExplodeStep>,
    explode_slider: Option<(usize, Entity<gpui_kit::component::slider::SliderState>, Subscription)>,
    explode_play: Option<Task<()>>,
    renaming: Option<(RenameTarget, Entity<InputState>)>,
    /// Enter handler of the active transient input (rename / named position).
    transient_sub: Option<Subscription>,
    config_panel: Option<ConfigSession>,
    replace_panel: Option<ReplaceSession>,
    /// Repositioned manipulator: instances it belongs to and its frame
    /// relative to the first one's placement (follows the part).
    triad: Option<(Vec<InstanceId>, Frame)>,
    /// Manipulator context menu (position, handle).
    triad_menu: Option<(Point<Pixels>, ManipHandle)>,
    snap_drag: Option<SnapDrag>,
    /// The snap drag was started on a fixed instance (only a click picks).
    snap_fixed: bool,
    /// Snapped placement of the dragged instance, used as the start of the
    /// mate dialog's preview (until the dialog closes).
    snap_start: Option<(InstanceId, Placement)>,
    /// A Part Studio changed: re-sync part patterns / replicates after the next evaluation.
    pattern_resync: bool,
    step_edit: Option<StepEdit>,
    ctrl_down: bool,
    /// Shift held while picking: lock mate inference to the hovered entity.
    infer_locked: bool,
    /// Insert from another document: the document's versions and the chosen one.
    /// Newer version of each external reference (None: up to date), and keys being checked.
    ext_updates: HashMap<String, Option<crate::derived::VersionChoice>>,
    ext_checking: HashSet<String>,
    focus: FocusHandle,
    fitted_once: bool,
    _subs: Vec<Subscription>,
}

/// Requests from the assembly view to the document window.
pub enum AssemblyViewEvent {
    /// Switch to another tab (instance menu > Switch to Part Studio / Assembly).
    ActivateTab(TabId),
    /// Instance menu › Edit in context: open the Part Studio with the
    /// assembly as context graphics.
    EditInContext(TabId, Box<crate::part_studio::AssemblyContext>),
}

impl EventEmitter<AssemblyViewEvent> for AssemblyView {}

impl Focusable for AssemblyView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// Does assembly `tab` (transitively) contain assembly `target`?
fn contains_assembly(doc: &Document, tab: TabId, target: TabId, depth: usize) -> bool {
    if tab == target {
        return true;
    }
    if depth > 16 {
        return true;
    }
    let Some(TabContent::Assembly(a)) = doc.tab(tab).map(|t| &t.content) else { return false };
    a.instances.iter().any(|i| match &i.source {
        InstanceSource::Assembly { assembly, .. } => contains_assembly(doc, *assembly, target, depth + 1),
        _ => false,
    })
}

impl AssemblyView {
    pub fn new(doc: Entity<DocModel>, tab: TabId, gpu: Gpu, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let viewport = cx.new(|cx| Viewport::new(gpu.clone(), window, cx));
        let mut subs = Vec::new();
        subs.push(cx.subscribe_in(&viewport, window, Self::on_viewport_event));
        subs.push(cx.subscribe_in(&doc, window, |this: &mut Self, _, ev: &DocEvent, window, cx| match ev {
            // Any tab can affect an assembly (Part Studios, sub-assemblies).
            DocEvent::TabChanged(t) => {
                // Target geometry of part patterns / replicates may have changed.
                if *t != this.tab {
                    this.pattern_resync = true;
                }
                this.schedule_eval(window, cx)
            }
            DocEvent::TabsChanged => this.schedule_eval(window, cx),
            DocEvent::Reloaded => {
                this.close_mate(window, cx);
                this.schedule_eval(window, cx)
            }
            DocEvent::Error(msg) => {
                this.message = Some((true, msg.clone()));
                cx.notify();
            }
            DocEvent::Info(msg) => {
                this.message = Some((false, msg.clone()));
                cx.notify();
            }
            _ => {}
        }));
        let mut this = Self {
            tab,
            doc,
            gpu,
            viewport,
            var_tables: HashMap::new(),
            cube_hover: None,
            cube_origin: None,
            evaluator: Arc::new(Mutex::new({
                let mut ev = AssemblyEvaluator::new();
                ev.set_external(Arc::new(AppExternal));
                ev
            })),
            result: Arc::new(AssemblyResult::default()),
            built: None,
            cache: AsmGpuCache::default(),
            eval_seq: 0,
            eval_task: None,
            evaluating: false,
            catalog: Catalog::default(),
            selected: Vec::new(),
            selected_mate: None,
            list_hover_mate: None,
            hover: None,
            manip_hover: None,
            hover_overlay: None,
            overlay_connectors: Vec::new(),
            overlay_sig: Vec::new(),
            pointer: DVec2::ZERO,
            infer: None,
            infer_choice: None,
            mate: None,
            relation: None,
            insert_open: false,
            insert_rigid: false,
            drag: None,
            anim: None,
            panel: None,
            interference: None,
            interference_running: false,
            explode: None,
            isolate: None,
            show_mates: false,
            show_connectors: false,
            // Onshape's Insert dialog starts with the Parts filter on.
            ui: AsmUi { insert_filter: (true, false), tb_prefs: toolbar::AsmToolbarPrefs::load(), ..Default::default() },
            expanded: HashSet::new(),
            message: None,
            mc: None,
            pattern: None,
            snap: None,
            show_mates_mode: false,
            mates_pinned: None,
            placing: None,
            placing_point: None,
            manip_value: None,
            explode_edit: false,
            explode_progress: None,
            explode_preview: None,
            explode_slider: None,
            explode_play: None,
            renaming: None,
            transient_sub: None,
            config_panel: None,
            replace_panel: None,
            triad: None,
            triad_menu: None,
            snap_drag: None,
            snap_fixed: false,
            snap_start: None,
            pattern_resync: false,
            step_edit: None,
            ctrl_down: false,
            infer_locked: false,
            ext_updates: HashMap::new(),
            ext_checking: HashSet::new(),
            focus: cx.focus_handle(),
            fitted_once: false,
            _subs: subs,
        };
        this.schedule_eval(window, cx);
        this
    }

    /// A part instance of a Composite part.
    fn is_composite_source(&self, source: &InstanceSource, cx: &App) -> bool {
        match source {
            InstanceSource::Part { studio, body, .. } => match self.doc.read(cx).session.doc.tab(*studio).map(|t| &t.content) {
                Some(TabContent::PartStudio(ps)) => ps.feature(body.feature).is_some_and(|f| f.type_id == "compositePart"),
                _ => false,
            },
            _ => false,
        }
    }

    fn assembly<'a>(&self, cx: &'a App) -> Option<&'a Assembly> {
        match &self.doc.read(cx).session.doc.tab(self.tab)?.content {
            TabContent::Assembly(a) => Some(&**a),
            _ => None,
        }
    }

    fn units(&self, cx: &App) -> noshape_units::UnitPrefs {
        self.doc.read(cx).session.doc.settings.units
    }

    // ------------------------------------------------------------ evaluation

    /// Assembly to evaluate (with the mate being edited applied) and solve options.
    fn effective(&self, cx: &App) -> Option<(Assembly, SolveOptions)> {
        let mut asm = self.assembly(cx)?.clone();
        // The configuration chosen above the instance list.
        if asm.has_configured() {
            asm.apply_configuration(&self.ui.config_values);
        }
        let mut opts = SolveOptions::default();
        if let Some(s) = &self.mate
            && let Some(m) = s.dialog.read(cx).to_mate(cx)
        {
            let fixed = |id: InstanceId| asm.instance(id).is_some_and(|i| i.fixed);
            // The second connector's instance stays put unless the first is fixed
            // (until Solve solves every mate).
            if !s.solve_all && !fixed(m.a.instance) && !fixed(m.b.instance) {
                opts.hold.push(m.b.instance);
            }
            // Until Solve only this mate is solved (help: Mates), so instances
            // mated to the moving one stay where they are.
            if !s.solve_all {
                opts.only_mate = Some(m.id);
            }
            // After a snap drag the dragged instance starts at its snapped placement.
            if let Some((id, p)) = self.snap_start {
                let mut start: HashMap<InstanceId, Placement> = asm.instances.iter().map(|i| (i.id, i.placement)).collect();
                start.insert(id, p);
                opts.start = Some(start);
            }
            match s.editing.and_then(|id| asm.mates.iter().position(|x| x.id == id)) {
                Some(i) => asm.mates[i] = m,
                None => asm.mates.push(m),
            }
        }
        // Mate connector being edited: previewed as a regular assembly connector.
        if let Some(s) = self.mc.as_ref().filter(|s| !s.implicit)
            && let Some(c) = s.to_connector(cx)
        {
            match asm.connectors.iter().position(|x| x.id == c.id) {
                Some(i) => asm.connectors[i] = c,
                None => asm.connectors.push(c),
            }
        }
        Some((asm, opts))
    }

    pub fn schedule_eval(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_implicit_mc(cx);
        let Some((asm, opts)) = self.effective(cx) else { return };
        let doc = self.doc.read(cx).session.doc.clone();
        let evaluator = self.evaluator.clone();
        let explode = self.explode;
        let self_tab = self.tab;
        self.eval_seq += 1;
        let seq = self.eval_seq;
        self.evaluating = true;
        self.eval_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(10)).await;
            let (result, catalog) = cx
                .background_spawn(async move {
                    let mut ev = evaluator.lock();
                    ev.retain_tabs(&doc);
                    let result = ev.evaluate(&doc, &asm, &opts, explode);
                    let mut catalog = Catalog::default();
                    for t in doc.ordered_tabs() {
                        match &t.content {
                            TabContent::PartStudio(ps) => {
                                let r = ev.studio(&doc, t.id, &ConfigValues::default());
                                let composites: Vec<BodyId> = r.as_ref().map(|r| r.display.bodies.iter().filter(|b| ps.feature(b.id.feature).is_some_and(|f| f.type_id == "compositePart")).map(|b| b.id.clone()).collect()).unwrap_or_default();
                                let parts = r
                                    .as_ref()
                                    .map(|r| {
                                        r.display.bodies.iter().filter(|b| matches!(b.kind, BodyKind::Solid | BodyKind::Surface)).map(|b| (b.id.clone(), b.name.clone(), b.bounds, b.kind == BodyKind::Surface)).collect()
                                    })
                                    .unwrap_or_default();
                                let sketches = r.as_ref().map(|r| r.context.sketches.iter().filter(|s| s.sketch.entities().any(|e| !e.construction)).map(|s| (s.feature, s.name.clone())).collect()).unwrap_or_default();
                                catalog.studios.push(CatalogStudio { tab: t.id, name: t.name.clone(), parts, sketches, composites });
                            }
                            TabContent::Assembly(_) if !contains_assembly(&doc, t.id, self_tab, 0) => catalog.assemblies.push((t.id, t.name.clone())),
                            _ => {}
                        }
                    }
                    (result, catalog)
                })
                .await;
            let _ = this.update_in(cx, |this: &mut Self, window, cx| {
                if seq != this.eval_seq {
                    return;
                }
                this.evaluating = false;
                this.catalog = catalog;
                this.apply_result(Arc::new(result), window, cx);
            });
        }));
        cx.notify();
    }

    fn apply_result(&mut self, result: Arc<AssemblyResult>, _window: &mut Window, cx: &mut Context<Self>) {
        self.result = result;
        self.infer = None;
        self.refresh_center_of_mass(cx);
        if std::mem::take(&mut self.pattern_resync) && self.mate.is_none() && self.pattern.is_none() {
            self.resync_patterns(cx);
        }
        self.check_external_updates(cx);
        self.publish_stale_contexts(cx);
        if self.explode.is_some() || self.placing.is_some() {
            // Re-apply display-only state (explode playback, cursor placement).
            self.update_placing(cx);
            self.apply_display(cx);
        }
        self.update_pattern_preview(cx);
        self.rebuild_scene(true, cx);
        if !self.fitted_once && self.result.bounds().is_some() {
            self.fitted_once = true;
            self.viewport.update(cx, |vp, cx| {
                vp.camera.set_view(StandardView::Trimetric);
                vp.zoom_to_fit(false, cx)
            });
        }
        if let Some(s) = &self.mate {
            let id = s.dialog.read(cx).id;
            let msg = self.result.mates.iter().find(|m| m.id == id).and_then(|m| m.message.clone());
            s.dialog.update(cx, |d, cx| {
                if d.a.is_some() && d.b.is_some() {
                    d.status = msg;
                    cx.notify();
                }
            });
        }
        cx.notify();
    }

    fn scene_options(&self) -> AsmSceneOptions {
        let mut o = AsmSceneOptions { isolate: self.isolate.clone(), ..Default::default() };
        if let Some(z) = self.active_zebra() {
            o.zebra = Some(noshape_render::Zebra { stripes: z.stripes, flip: z.flip });
            o.hide_edges = !z.show_edges;
        }
        // Reflection analysis: edges follow Show edges from the moment the
        // dialog opens (live analysis/10), the environment once chosen.
        if let Some(s) = self.active_reflection() {
            o.hide_edges = !s.show_edges;
            o.reflection = self.scene_reflection();
        }
        if let Some(t) = &self.ui.transparent {
            o.ghost_tops.extend(t.iter().copied());
        }
        if let Some(s) = &self.ui.section {
            o.section.push(s.clip());
        }
        if self.isolate.is_some() || self.ui.transparent.is_some() {
            o.dim_alpha = self.ui.transparency;
            // Select transparent geometry / Alt+click: pick through.
            o.pick_dimmed = self.ui.select_transparent || self.ui.alt_through;
        }
        if let Some(s) = &self.snap_drag
            && s.started
        {
            o.ghost_tops.insert(s.top);
        }
        if let (Some(list), Some(SidePanel::Interference)) = (&self.interference, self.panel) {
            for i in list {
                o.error_tops.insert(i.a[0]);
                o.error_tops.insert(i.b[0]);
            }
        }
        o
    }

    fn rebuild_scene(&mut self, geometry_changed: bool, cx: &mut Context<Self>) {
        let built = assembly_scene::build(&self.result, &self.scene_options(), &mut self.cache, self.gpu.uploader());
        let prio = built.picks.priority_fn();
        self.built = Some(built);
        self.viewport.update(cx, |vp, _| vp.pick_priority = prio);
        self.refresh_scene(geometry_changed, cx);
    }

    fn triad_size(&self) -> f64 {
        self.result.bounds().map(|(a, b)| (b - a).length() * 0.06).unwrap_or(0.01).clamp(0.002, 1.0)
    }

    fn mate_instances(&self, id: MateId, cx: &App) -> Vec<InstanceId> {
        let Some(a) = self.assembly(cx) else { return vec![] };
        if let Some(m) = a.mate(id) {
            return m.instances();
        }
        if let Some(p) = a.pattern(id) {
            return p.seeds.iter().copied().chain(p.generated.iter().map(|g| g.instance)).collect();
        }
        if let Some(g) = a.groups.iter().find(|g| g.id == id) {
            return g.instances.clone();
        }
        a.connector(id).map(|c| vec![c.owner()]).unwrap_or_default()
    }

    /// Push highlight state + overlays to the viewport.
    fn refresh_scene(&mut self, geometry_changed: bool, cx: &mut Context<Self>) {
        let Some(built) = &self.built else { return };
        let mut h = Highlights { selected_tops: self.selected.clone(), ..Default::default() };
        let size = self.triad_size();
        let units = self.units(cx);
        if let Some(t) = &self.hover
            && matches!(t.kind, PickKind::Face | PickKind::Edge | PickKind::Vertex)
        {
            h.hover_entity = Some((t.path.clone(), t.reference.clone()));
        }
        let mate_glyph = |h: &mut Highlights, ms: &noshape_model::assembly::eval::MateStatus, strong: bool, scale: f64| {
            for f in [ms.frame_a, ms.frame_b].into_iter().flatten() {
                h.triads.push(Triad { frame: f, size: size * scale, strong });
            }
            if let (Some(a), Some(b)) = (ms.frame_a, ms.frame_b)
                && a.origin.distance(b.origin) > 1e-9
            {
                h.mate_lines.push((a.origin, b.origin, strong));
            }
        };
        // A selected mate with a rotation shows its value arc (live 305z);
        // so does the mate dialog's Animate preview (live 575).
        for d in self.value_dims(cx) {
            match d.shape {
                behaviors::ValueShape::Arc(arc) => h.value_arcs.push(arc),
                behaviors::ValueShape::Line(line) => h.value_lines.push(line),
            }
        }
        // Mate highlight (list hover / selection): instances accent + both connector triads.
        for mid in [self.selected_mate, self.list_hover_mate].into_iter().flatten() {
            h.accent_tops.extend(self.mate_instances(mid, cx));
            if let Some(ms) = self.result.mates.iter().find(|m| m.id == mid) {
                mate_glyph(&mut h, ms, true, 1.0);
            }
            if let Some(c) = self.result.connectors.iter().find(|c| c.id == mid)
                && let Some(f) = c.frame
            {
                h.triads.push(Triad { frame: f, size: size * 1.2, strong: true });
            }
        }
        if self.show_mates {
            let hidden: HashSet<MateId> = self.assembly(cx).map(|a| a.mates.iter().filter(|m| m.hidden).map(|m| m.id).collect()).unwrap_or_default();
            for ms in self.result.mates.iter().filter(|m| !m.inactive && !hidden.contains(&m.id)) {
                mate_glyph(&mut h, ms, false, 0.6);
            }
        }
        // Instance row hovered in the list: highlighted in the graphics area.
        if let Some(top) = self.ui.list_hover {
            h.accent_tops.push(top);
        }
        if let Some(m) = &self.ui.measure {
            h.selected_entities = m.clone();
        }
        if let Some(d) = &self.ui.curve_dialog {
            h.selected_entities.extend(d.picks.iter().cloned());
        }
        h.analysis_lines = self.curve_surface_lines();
        // Group dialog: its instances are shown selected.
        if let Some((_, ids)) = &self.ui.group {
            h.selected_tops = ids.clone();
        }
        // Show mates mode: the hovered (or pinned) instance's mates.
        if self.show_mates_mode {
            let focus = self.mates_pinned.or_else(|| self.hover.as_ref().map(|t| t.path[0]));
            if let (Some(top), Some(asm)) = (focus, self.assembly(cx)) {
                h.accent_tops.push(top);
                for m in asm.mates.iter().filter(|m| m.uses_instance(top)) {
                    h.accent_tops.extend(m.instances());
                    if let Some(ms) = self.result.mates.iter().find(|x| x.id == m.id) {
                        mate_glyph(&mut h, ms, true, 0.9);
                    }
                }
            }
        }
        if self.show_connectors {
            let mut seen = HashSet::new();
            for o in &self.result.occurrences {
                if o.hidden || !seen.insert(o.path.clone()) {
                    continue;
                }
                let t = noshape_model::assembly::eval::matrix_frame(&o.transform);
                for c in &o.context.connectors {
                    let f = t.mul(&Frame::from_axes(c.origin, c.z, c.x));
                    h.triads.push(Triad { frame: f, size: size * 0.7, strong: false });
                }
            }
        }
        let picking = self.pick_mode(cx).is_some();
        let hovered_overlay = self.hover_overlay;
        // Assembly origin and assembly mate connectors (pickable).
        h.origin = (!self.ui.origin_hidden || picking).then_some((size * 0.8, picking && hovered_overlay == Some(OverlayPick::Origin)));
        let mut overlay_connectors = Vec::new();
        if let Some(asm) = self.assembly(cx) {
            let editing_mc = self.mc.as_ref().map(|m| m.id);
            for c in &self.result.connectors {
                let hidden = asm.connector(c.id).is_some_and(|x| x.hidden) && editing_mc != Some(c.id) && self.selected_mate != Some(c.id);
                if let Some(f) = c.frame
                    && !hidden
                {
                    let i = overlay_connectors.len();
                    let hl = hovered_overlay == Some(OverlayPick::Connector(i)) || editing_mc == Some(c.id) || self.selected_mate == Some(c.id);
                    // The connector dialog's preview slider fades the edited one
                    // (as Onshape's feature preview slider, resting at 0.7).
                    let opacity = self.mc.as_ref().filter(|m| m.id == c.id).map_or(1.0, |m| (m.preview / panels::PREVIEW_REST).min(1.0));
                    h.connectors.push((f, size * 0.9, hl, opacity));
                    overlay_connectors.push(c.id);
                }
            }
        }
        self.overlay_connectors = overlay_connectors;
        // Mate indicators (pickable): displayed mates, plus the hovered /
        // selected one; not while a dialog field takes picks.
        let mut overlay_mates = Vec::new();
        if !picking && let Some(asm) = self.assembly(cx) {
            let shown_mode: Vec<MateId> = match (self.show_mates_mode, self.mates_pinned.or_else(|| self.hover.as_ref().map(|t| t.path[0]))) {
                (true, Some(top)) => asm.mates.iter().filter(|m| m.uses_instance(top)).map(|m| m.id).collect(),
                _ => Vec::new(),
            };
            for ms in &self.result.mates {
                let Some(m) = asm.mate(ms.id) else { continue };
                let visible = (self.show_mates && !m.hidden && !ms.inactive) || self.selected_mate == Some(m.id) || self.list_hover_mate == Some(m.id) || shown_mode.contains(&m.id);
                let Some(f) = ms.frame_b.or(ms.frame_a) else { continue };
                if visible {
                    let i = overlay_mates.len();
                    let hl = hovered_overlay == Some(OverlayPick::Mate(i)) || self.selected_mate == Some(m.id);
                    let st = if ms.inactive || m.suppressed {
                        assembly_scene::MateGlyphState::Inactive
                    } else if !ms.ok {
                        assembly_scene::MateGlyphState::Error
                    } else {
                        assembly_scene::MateGlyphState::Ok
                    };
                    h.mate_glyphs.push((f, size * 0.17, hl, st));
                    overlay_mates.push(m.id);
                }
            }
        }
        self.ui.overlay_mates = overlay_mates;
        // Dialog connectors + inference preview.
        let mut chosen: Vec<MateConnectorRef> = Vec::new();
        if let Some(s) = &self.mate {
            chosen.extend(s.dialog.read(cx).connectors().into_iter().cloned());
        }
        if let Some(p) = &self.pattern {
            chosen.extend(p.axis.iter().cloned());
            chosen.extend(p.axis2.iter().cloned());
        }
        if let Some(Some(src)) = &self.snap {
            chosen.push(src.clone());
        }
        if let Some(s) = &self.snap_drag
            && s.started
        {
            chosen.extend(s.sources.get(s.source_ix).cloned());
            chosen.extend(s.target.clone());
        }
        if self.explode.is_some() {
            h.trails = self.result.explode_trails.clone();
        }
        for c in &chosen {
            if let Some(f) = self.result.connector_world(c, &units) {
                h.triads.push(Triad { frame: f, size, strong: false });
            }
        }
        if picking
            && self.pick_mode(cx) == Some(PickMode::Inference)
            && let Some(((_, entity), occ, cands)) = &self.infer
            && let Some(o) = self.result.occurrences.get(*occ)
            && (self.infer_locked || self.hover.as_ref().is_some_and(|h| h.occ == *occ))
        {
            if self.infer_locked {
                // Shift: the locked entity stays highlighted.
                h.hover_entity = Some((o.path.clone(), entity.clone()));
            }
            let t = noshape_model::assembly::eval::matrix_frame(&o.transform);
            for (i, p) in cands.iter().enumerate() {
                let w = t.mul(&p.frame);
                if Some(i) == self.infer_choice {
                    h.triads.push(Triad { frame: w, size, strong: true });
                } else {
                    h.dots.push(w.origin);
                }
            }
        }
        if let Some(p) = self.placing_point
            && self.placing.is_some()
        {
            h.cursor_mark = Some((p, size * 0.4));
        }
        h.manipulator = self.manipulator(cx).map(|f| (f, size * 1.6, self.manip_hover.or(self.ui.manip_selected)));
        // Pickable overlays changed: the pick buffer must be redrawn too.
        let mut sig: Vec<f64> = Vec::new();
        if let Some((f, s, _)) = &h.manipulator {
            sig.extend([f.origin.x, f.origin.y, f.origin.z, *s]);
        }
        if let Some((s, _)) = h.origin {
            sig.push(s);
        }
        for (f, s, _, o) in &h.connectors {
            sig.extend([f.origin.x, f.origin.y, f.origin.z, f.rotation.x, f.rotation.y, f.rotation.z, *s, f64::from(*o)]);
        }
        let geometry_changed = geometry_changed || sig != self.overlay_sig;
        self.overlay_sig = sig;
        let scene = assembly_scene::with_highlights(built, &self.result, &h, self.gpu.uploader());
        self.viewport.update(cx, |vp, cx| vp.set_scene(Arc::new(scene), geometry_changed, cx));
    }

    /// Instances the manipulator moves.
    fn manip_targets(&self, cx: &App) -> Vec<InstanceId> {
        if self.mate.is_some() || self.anim.is_some() || self.placing.is_some() || self.snap.is_some() || self.snap_drag.is_some() || self.pick_mode(cx).is_some() || self.selected.is_empty() || self.ui.group.is_some() {
            return vec![];
        }
        let Some(asm) = self.assembly(cx) else { return vec![] };
        if self.explode_edit {
            // Explode steps move any visible instances (fixed ones too).
            return self.selected.iter().copied().filter(|id| asm.instance(*id).is_some_and(|i| !i.suppressed && !i.hidden)).collect();
        }
        if self.selected.len() != 1 {
            return vec![];
        }
        let id = self.selected[0];
        match asm.instance(id) {
            Some(i) if !i.fixed && !i.suppressed && !i.hidden => vec![id],
            _ => vec![],
        }
    }

    /// Move manipulator frame: at the bounds center of the instances it moves, world-aligned.
    fn manipulator(&self, cx: &App) -> Option<Frame> {
        let ids = self.manip_targets(cx);
        if ids.is_empty() {
            return None;
        }
        let mut bb: Option<(DVec3, DVec3)> = None;
        for id in &ids {
            if let Some((a, b)) = self.result.instance_bounds(*id) {
                bb = Some(bb.map(|(x, y)| (x.min(a), y.max(b))).unwrap_or((a, b)));
            }
        }
        if let Some(f) = self.drag.as_ref().and_then(|d| d.triad_preview) {
            return Some(f);
        }
        if let Some((tids, local)) = &self.triad
            && *tids == ids
            && let Some(p) = self.result.solved.placement(ids[0])
        {
            return Some(placement_to_frame(&p).mul(local));
        }
        let center = bb.map(|(a, b)| (a + b) * 0.5).or_else(|| self.result.solved.placement(ids[0]).map(|p| DVec3::from(p.translation)))?;
        Some(Frame::new(center, glam::DQuat::IDENTITY))
    }

    // ------------------------------------------------------------ editing

    /// Commit a change to the assembly, folding in the latest solved placements.
    fn edit<R>(&mut self, desc: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut Assembly) -> R) -> Option<R> {
        let solved = self.result.solved.placements.clone();
        let nested = self.result.solved.nested.clone();
        let flexible = self.result.prepared.flexible.clone();
        let tab = self.tab;
        self.doc.update(cx, |doc, cx| {
            doc.edit_tab(tab, desc, cx, |c| {
                if let TabContent::Assembly(a) = c {
                    for (id, p) in &solved {
                        if let Some(i) = a.instance_mut(*id) {
                            i.placement = *p;
                            // Flexible subassemblies keep their parts' positions here.
                            if flexible.contains(id) {
                                let mut n: Vec<(InstanceId, Placement)> = nested.iter().filter(|((s, _), _)| s == id).map(|((_, c), q)| (*c, *q)).collect();
                                n.sort_by_key(|(c, _)| *c);
                                i.nested = n;
                            }
                        }
                    }
                    Some(f(a))
                } else {
                    None
                }
            })
            .flatten()
        })
    }

    fn insert_part(&mut self, studio: TabId, body: Option<CatalogPart>, cx: &mut Context<Self>) -> Option<InstanceId> {
        let studio_name = self.doc.read(cx).session.doc.tab(studio).map(|t| t.name.clone()).unwrap_or_default();
        let desc = match &body {
            Some((_, n, _, _)) => format!("Insert : {n}"),
            None => format!("Insert : {studio_name}"),
        };
        // Configuration chosen in the Insert dialog (Onshape: set it before clicking the item).
        let config = self.ui.insert_config.get(&studio).cloned().unwrap_or_default();
        self.edit(&desc, cx, |a| {
            let id = InstanceId(a.next_id());
            let (source, base, width) = match body {
                Some((b, n, bb, _)) => (InstanceSource::Part { studio, body: b, config }, n, bb.map(|(lo, hi)| hi.x - lo.x).unwrap_or(0.02)),
                None => (InstanceSource::PartStudio { studio, config }, studio_name.clone(), 0.05),
            };
            // Later instances of the same item are offset so they don't overlap.
            let same = a.instances.iter().filter(|i| i.source == source).count();
            let placement = Placement { translation: [same as f64 * (width + 0.01), 0.0, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] };
            let name = a.next_instance_name(&base);
            a.instances.push(Instance { id, name, source, placement, fixed: false, hidden: false, suppressed: false, lock: SubassemblyLock::Flexible, nested: Vec::new(), best_tessellation: false, reference_designator: None, unsuppressed_if: None });
            id
        })
    }

    /// Start placing freshly inserted instances: they follow the cursor until a click.
    fn start_placing(&mut self, ids: Vec<InstanceId>, cx: &mut Context<Self>) {
        if ids.is_empty() {
            return;
        }
        if self.insert_open {
            self.ui.insert_inserted.extend(ids.iter().copied());
        }
        // Live Onshape: the instance following the cursor is selected (amber),
        // and deselected once it is placed (assembly/04, 06).
        self.selected = ids.clone();
        self.placing = Some(ids);
        self.placing_point = None;
        self.ui.place_snap = None;
        self.ui.place_src_ix = 0;
        // Live Onshape shows no hint while the instance follows the cursor.
        self.viewport.update(cx, |vp, _| vp.tool_captures_left = false);
        cx.notify();
    }

    fn insert_and_place(&mut self, studio: TabId, body: Option<CatalogPart>, cx: &mut Context<Self>) {
        if let Some(id) = self.insert_part(studio, body, cx) {
            self.start_placing(vec![id], cx);
        }
    }

    /// Insert › Sketches: a sketch of a Part Studio as an instance.
    fn insert_sketch(&mut self, studio: TabId, sketch: FeatureId, name: String, cx: &mut Context<Self>) {
        let id = self.edit(&format!("Insert : {name}"), cx, |a| {
            let id = InstanceId(a.next_id());
            let n = a.next_instance_name(&name);
            a.instances.push(Instance::new(id, n, InstanceSource::Sketch { studio, sketch, config: ConfigValues::default() }, Placement::default()));
            id
        });
        if let Some(id) = id {
            self.start_placing(vec![id], cx);
        }
    }

    fn insert_studio_parts(&mut self, s: CatalogStudio, cx: &mut Context<Self>) {
        if self.insert_rigid {
            self.insert_and_place(s.tab, None, cx);
        } else {
            let mut ids = Vec::new();
            for part in s.parts {
                ids.extend(self.insert_part(s.tab, Some(part), cx));
            }
            self.start_placing(ids, cx);
        }
    }

    fn insert_assembly(&mut self, tab: TabId, name: String, cx: &mut Context<Self>) {
        let config = self.ui.insert_config.get(&tab).cloned().unwrap_or_default();
        let id = self.edit(&format!("Insert : {name}"), cx, |a| {
            let id = InstanceId(a.next_id());
            let n = a.next_instance_name(&name);
            a.instances.push(Instance { id, name: n, source: InstanceSource::Assembly { assembly: tab, config: config.clone() }, placement: Placement::default(), fixed: false, hidden: false, suppressed: false, lock: SubassemblyLock::Flexible, nested: Vec::new(), best_tessellation: false, reference_designator: None, unsuppressed_if: None });
            id
        });
        if let Some(id) = id {
            self.start_placing(vec![id], cx);
        }
    }

    /// World point under the cursor for placement: on the hovered planar
    /// face, else on the Top plane, else on the camera plane through the origin.
    fn cursor_world_point(&self, cx: &App) -> Option<DVec3> {
        let vp = self.viewport.read(cx);
        let (o, d) = vp.ray(self.pointer);
        let hit = |p: DVec3, n: DVec3| -> Option<DVec3> {
            let den = d.dot(n);
            if den.abs() < 1e-9 {
                return None;
            }
            let t = (p - o).dot(n) / den;
            (t > 0.0).then(|| o + d * t)
        };
        if let Some(t) = &self.hover
            && t.kind == PickKind::Face
            && !self.placing.as_ref().is_some_and(|ids| ids.contains(&t.path[0]))
            && let Some(occ) = self.result.occurrences.get(t.occ)
        {
            let units = self.units(cx);
            let r = Reference::Implicit { on: Box::new(t.reference.clone()), inference: Inference::Centroid };
            if let Some(f) = noshape_model::assembly::eval::resolve_connector_in(&occ.context, Some(&occ.body), &r, &units) {
                let w = noshape_model::assembly::eval::matrix_frame(&occ.transform).mul(&f);
                if let Some(p) = hit(w.origin, w.z()) {
                    return Some(p);
                }
            }
        }
        hit(DVec3::ZERO, DVec3::Z).or_else(|| hit(DVec3::ZERO, vp.camera.forward()))
    }

    /// Move the instances being placed so their bounds' bottom center sits at the cursor point.
    fn update_placing(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(ids) = self.placing.clone() else { return false };
        // Snap mode: an inserted instance with explicit mate connectors snaps
        // onto mate connectors of the others (Onshape "Insert and drag to snap").
        if ids.len() == 1 && (self.snap.is_some() || self.std_placing(ids[0], cx)) && self.update_insert_snap(ids[0], cx) {
            return true;
        }
        self.ui.place_snap = None;
        let Some(p) = self.cursor_world_point(cx) else { return false };
        self.placing_point = Some(p);
        let mut bb: Option<(DVec3, DVec3)> = None;
        for id in &ids {
            if let Some((a, b)) = self.result.instance_bounds(*id) {
                bb = Some(bb.map(|(x, y)| (x.min(a), y.max(b))).unwrap_or((a, b)));
            }
        }
        let Some((lo, hi)) = bb else { return false };
        let anchor = DVec3::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5, lo.z);
        let delta = p - anchor;
        if delta.length() < 1e-12 {
            return false;
        }
        let mut solved = self.result.solved.clone();
        for (id, pl) in &mut solved.placements {
            if ids.contains(id) {
                pl.translation = (DVec3::from(pl.translation) + delta).into();
            }
        }
        self.apply_solved(solved, cx);
        true
    }

    fn finish_placing(&mut self, commit: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ids) = self.placing.take() else { return };
        self.placing_point = None;
        self.message = None;
        if commit {
            let name = self.assembly(cx).and_then(|a| a.instance(ids[0]).map(|i| i.name.clone())).unwrap_or_default();
            match self.ui.place_snap.take() {
                // Snapped: a Fastened mate between the two mate connectors.
                Some((src, dst)) => {
                    self.edit(&format!("Insert : Fastened ({name})"), cx, |a| {
                        let id = MateId(a.next_id());
                        let n = a.next_feature_name("Fastened");
                        a.mates.push(Mate::new(id, n, MateType::Fastened, src, dst));
                    });
                }
                None => {
                    self.edit(&format!("Place : {name}"), cx, |_| {});
                }
            }
            self.selected.retain(|s| !ids.contains(s));
        } else {
            self.selected.retain(|s| !ids.contains(s));
            self.schedule_eval(window, cx);
        }
        self.refresh_scene(false, cx);
        cx.notify();
    }

    fn targets(&self, id: Option<InstanceId>) -> Vec<InstanceId> {
        match id {
            Some(i) if !self.selected.contains(&i) => vec![i],
            _ => self.selected.clone(),
        }
    }

    fn set_fixed(&mut self, ids: Vec<InstanceId>, fixed: bool, cx: &mut Context<Self>) {
        if ids.is_empty() {
            self.message = Some((false, "Select instances to fix or unfix.".into()));
            cx.notify();
            return;
        }
        self.edit(if fixed { "Fix" } else { "Unfix" }, cx, |a| {
            for id in &ids {
                if let Some(i) = a.instance_mut(*id) {
                    i.fixed = fixed;
                }
            }
        });
    }

    /// Instance menu "Use best available tessellation" / "Use automatic
    /// tessellation setting" (the same slot, live assembly/110).
    pub(super) fn set_best_tessellation(&mut self, ids: Vec<InstanceId>, on: bool, cx: &mut Context<Self>) {
        self.edit(if on { "Use best available tessellation" } else { "Use automatic tessellation setting" }, cx, |a| {
            for id in &ids {
                if let Some(i) = a.instance_mut(*id) {
                    i.best_tessellation = on;
                }
            }
        });
    }

    fn set_hidden(&mut self, ids: Vec<InstanceId>, hidden: bool, cx: &mut Context<Self>) {
        self.edit(if hidden { "Hide" } else { "Show" }, cx, |a| {
            for id in &ids {
                if let Some(i) = a.instance_mut(*id) {
                    i.hidden = hidden;
                }
            }
        });
    }

    fn set_suppressed(&mut self, ids: Vec<InstanceId>, s: bool, cx: &mut Context<Self>) {
        let values = self.ui.config_values.clone();
        self.edit(if s { "Suppress" } else { "Unsuppress" }, cx, |a| {
            for id in &ids {
                // A configured suppression changes the current configuration.
                if a.set_configured_value(ConfigTarget::Instance(*id), acfg::param::UNSUPPRESSED, &values, ParamValue::Bool(!s)) {
                    continue;
                }
                if let Some(i) = a.instance_mut(*id) {
                    i.suppressed = s;
                }
            }
        });
    }

    fn delete_instances(&mut self, ids: Vec<InstanceId>, cx: &mut Context<Self>) {
        if ids.is_empty() {
            return;
        }
        self.edit("Delete instances", cx, |a| {
            for id in &ids {
                a.remove_instance(*id);
            }
            a.prune_configured();
        });
        self.selected.retain(|i| !ids.contains(i));
    }

    fn delete_mate(&mut self, id: MateId, cx: &mut Context<Self>) {
        let name = self.assembly(cx).and_then(|a| a.mate(id).map(|m| m.name.clone()).or_else(|| a.relations.iter().find(|r| r.id == id).map(|r| r.name.clone())).or_else(|| a.groups.iter().find(|g| g.id == id).map(|g| g.name.clone())));
        self.edit(&format!("Delete : {}", name.unwrap_or_default()), cx, |a| {
            a.remove_mate(id);
            a.prune_configured();
        });
        if self.selected_mate == Some(id) {
            self.selected_mate = None;
        }
    }

    fn toggle_mate_suppressed(&mut self, id: MateId, cx: &mut Context<Self>) {
        let values = self.ui.config_values.clone();
        self.edit("Suppress mate", cx, |a| {
            let now = a.configured(&values).mate(id).map(|m| m.suppressed);
            if let Some(now) = now
                && a.set_configured_value(ConfigTarget::Mate(id), acfg::param::UNSUPPRESSED, &values, ParamValue::Bool(now))
            {
                return;
            }
            if let Some(m) = a.mates.iter_mut().find(|m| m.id == id) {
                m.suppressed = !m.suppressed;
            }
            if let Some(r) = a.relations.iter_mut().find(|r| r.id == id) {
                r.suppressed = !r.suppressed;
            }
            if let Some(g) = a.groups.iter_mut().find(|g| g.id == id) {
                g.suppressed = !g.suppressed;
            }
        });
    }

    /// Group tool: opens the Group dialog with the selection as preselection.
    fn create_group(&mut self, cx: &mut Context<Self>) {
        let ids: Vec<InstanceId> = self.assembly(cx).map(|a| self.selected.iter().copied().filter(|id| a.generated_by(*id).is_none()).collect()).unwrap_or_default();
        self.open_group(None, ids, cx);
    }

    fn accept_group(&mut self, cx: &mut Context<Self>) {
        let Some((editing, ids)) = self.ui.group.take() else { return };
        if ids.len() < 2 {
            self.message = Some((false, "Select at least two instances to group.".into()));
            self.ui.group = Some((editing, ids));
            cx.notify();
            return;
        }
        match editing {
            Some(gid) => {
                self.edit("Edit : Group", cx, |a| {
                    if let Some(g) = a.groups.iter_mut().find(|g| g.id == gid) {
                        g.instances = ids;
                    }
                });
            }
            None => {
                self.edit("Insert : Group", cx, |a| {
                    let id = MateId(a.next_id());
                    let name = a.next_feature_name("Group");
                    a.groups.push(Group { id, name, instances: ids, suppressed: false });
                });
            }
        }
        self.update_capture(cx);
        cx.notify();
    }

    // ------------------------------------------------------------ mate dialog

    fn labeler(&self, cx: &App) -> ConnectorLabeler {
        let names: HashMap<Vec<InstanceId>, String> = {
            let mut m = HashMap::new();
            fn walk(nodes: &[InstanceNode], m: &mut HashMap<Vec<InstanceId>, String>) {
                for n in nodes {
                    m.insert(n.path.clone(), n.name.clone());
                    walk(&n.children, m);
                }
            }
            walk(&self.result.nodes, &mut m);
            if let Some(a) = self.assembly(cx) {
                for i in &a.instances {
                    m.entry(vec![i.id]).or_insert_with(|| i.name.clone());
                }
            }
            m
        };
        let mc_names: HashMap<MateId, String> = self.assembly(cx).map(|a| a.connectors.iter().map(|c| (c.id, c.name.clone())).collect()).unwrap_or_default();
        Arc::new(move |c: &MateConnectorRef| {
            if let Some(id) = c.connector {
                return mc_names.get(&id).cloned().unwrap_or_else(|| "Mate connector".into());
            }
            if c.instance == InstanceId::ORIGIN {
                return "Assembly origin".into();
            }
            let mut path = vec![c.instance];
            path.extend(c.path.iter().copied());
            let inst = names.get(&path).cloned().unwrap_or_else(|| "Instance".into());
            // Onshape names inferred (implicit) connectors "Mate connector of <instance>".
            let what = match &c.reference {
                Reference::Implicit { .. } | Reference::MateConnector { .. } => "Mate connector".to_string(),
                other => other.kind_label().to_string(),
            };
            format!("{what} of {inst}")
        })
    }

    fn start_mate(&mut self, t: MateType, window: &mut Window, cx: &mut Context<Self>) {
        self.close_mate(window, cx);
        self.insert_open = false;
        let Some(asm) = self.assembly(cx) else { return };
        let id = MateId(asm.next_id());
        let name = asm.next_feature_name(t.name());
        let dummy = MateConnectorRef::origin();
        let mate = Mate::new(id, name, t, dummy.clone(), dummy);
        self.open_mate_dialog(mate, None, window, cx);
    }

    fn edit_mate(&mut self, id: MateId, window: &mut Window, cx: &mut Context<Self>) {
        // Configure assembly features mode: the mate's parameters are picked.
        if self.ui.cfg_mode.is_some() {
            self.configure_pick_mate(id, window, cx);
            return;
        }
        self.close_mate(window, cx);
        if let Some(c) = self.assembly(cx).and_then(|a| a.connector(id).cloned()) {
            self.open_mc(Some(c), window, cx);
            return;
        }
        if let Some(p) = self.assembly(cx).and_then(|a| a.pattern(id).cloned()) {
            self.open_pattern_edit(p, window, cx);
            return;
        }
        if let Some(g) = self.assembly(cx).and_then(|a| a.groups.iter().find(|g| g.id == id).cloned()) {
            self.open_group(Some(g.id), g.instances, cx);
            return;
        }
        if let Some(r) = self.assembly(cx).and_then(|a| a.relations.iter().find(|r| r.id == id).cloned()) {
            self.open_relation(r.relation_type, Some(r), window, cx);
            return;
        }
        let Some(m) = self.assembly(cx).map(|a| a.configured(&self.ui.config_values)).and_then(|a| a.mate(id).cloned()) else { return };
        self.open_mate_dialog(m, Some(id), window, cx);
    }

    fn open_mate_dialog(&mut self, mate: Mate, editing: Option<MateId>, window: &mut Window, cx: &mut Context<Self>) {
        let labeler = self.labeler(cx);
        let is_new = editing.is_none();
        let dialog = cx.new(|cx| MateDialog::new(mate, is_new, labeler, window, cx));
        let sub = cx.subscribe_in(&dialog, window, |this: &mut Self, _, ev: &MateDialogEvent, window, cx| match ev {
            MateDialogEvent::Changed => {
                this.schedule_eval(window, cx);
                this.refresh_scene(false, cx);
            }
            MateDialogEvent::Accept => this.accept_mate(window, cx),
            MateDialogEvent::AcceptAndNew => this.accept_mate_and_new(window, cx),
            MateDialogEvent::Animate => this.toggle_mate_preview(window, cx),
            MateDialogEvent::EditConnector(field) => this.edit_dialog_connector(*field, window, cx),
            MateDialogEvent::ConfigureParam(param) => this.configure_mate_param(param, cx),
            MateDialogEvent::Solve => {
                if let Some(s) = &mut this.mate {
                    s.solve_all = true;
                }
                this.schedule_eval(window, cx);
            }
            MateDialogEvent::Cancel => this.close_mate(window, cx),
        });
        dialog.update(cx, |d, cx| window.focus(&d.focus_handle(cx), cx));
        self.mate = Some(MateSession { dialog, editing, solve_all: false, hud: None, _sub: sub });
        self.viewport.update(cx, |vp, _| vp.tool_captures_left = false);
        self.schedule_eval(window, cx);
        cx.notify();
    }

    fn accept_mate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(s) = &self.mate else { return };
        let Some(m) = s.dialog.read(cx).to_mate(cx) else {
            self.message = Some((false, "Select two mate connectors.".into()));
            cx.notify();
            return;
        };
        let editing = s.editing;
        self.mate = None;
        self.snap_start = None;
        if self.anim.as_ref().is_some_and(|a| a.preview) {
            self.anim = None;
        }
        let desc = if editing.is_some() { format!("Edit : {}", m.name) } else { format!("Insert : {}", m.name) };
        let values = self.ui.config_values.clone();
        let mut m = m;
        // Solved placements of the preview (which includes this mate) are folded in.
        self.edit(&desc, cx, |a| match editing.and_then(|id| a.mates.iter().position(|x| x.id == id)) {
            Some(i) => {
                // Configured parameters go to the current configuration's row.
                a.store_mate_edit(&mut m, &values);
                a.mates[i] = m;
            }
            None => a.mates.push(m),
        });
        window.focus(&self.focus, cx);
        self.infer = None;
        cx.notify();
    }

    /// Shift+Enter / heads-up checkmark: commit the mate and start a new one
    /// of the same type (the dialog stays open).
    fn accept_mate_and_new(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(t) = self.mate.as_ref().map(|s| s.dialog.read(cx).mate_type) else { return };
        let ok = self.mate.as_ref().is_some_and(|s| s.dialog.read(cx).to_mate(cx).is_some());
        self.accept_mate(window, cx);
        if ok {
            self.start_mate(t, window, cx);
        }
    }

    fn close_mate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.anim.as_ref().is_some_and(|a| a.preview) {
            self.anim = None;
        }
        self.snap_start = None;
        if self.mate.take().is_some() {
            window.focus(&self.focus, cx);
            self.infer = None;
            self.schedule_eval(window, cx);
            cx.notify();
        }
    }

    // ------------------------------------------------------------ relations

    fn open_relation(&mut self, kind: RelationType, existing: Option<Relation>, window: &mut Window, cx: &mut Context<Self>) {
        let text = existing.as_ref().map(|r| r.ratio.text.clone()).unwrap_or_else(|| match kind {
            RelationType::Gear | RelationType::Linear => "1".into(),
            RelationType::RackAndPinion => "10 mm".into(),
            RelationType::Screw => "1 mm".into(),
        });
        let ratio = cx.new(|cx| InputState::new(window, cx).default_value(text));
        // A new relation starts empty (a selected mate is the preselection);
        // mates are then picked in the list or the graphics area.
        let pre = self.selected_mate.filter(|id| self.assembly(cx).and_then(|a| a.mate(*id)).is_some_and(|m| m.mate_type.dof() > 0));
        self.relation = Some(RelationSession {
            kind: existing.as_ref().map(|r| r.relation_type).unwrap_or(kind),
            a: existing.as_ref().map(|r| r.mates[0]).or(pre),
            b: existing.as_ref().map(|r| r.mates[1]),
            ratio,
            reverse: existing.as_ref().is_some_and(|r| r.reverse),
            editing: existing.map(|r| r.id),
        });
        cx.notify();
    }

    fn accept_relation(&mut self, cx: &mut Context<Self>) {
        let Some(r) = self.relation.take() else { return };
        let screw = r.kind == RelationType::Screw;
        let (Some(a), Some(b)) = (r.a, if screw { r.a } else { r.b }) else {
            self.message = Some((false, "Select two mates for the relation.".into()));
            cx.notify();
            return;
        };
        if a == b && !screw {
            self.message = Some((false, "A relation needs two different mates.".into()));
            cx.notify();
            return;
        }
        let ratio = ExprValue::new(r.ratio.read(cx).value().to_string());
        let (kind, reverse, editing) = (r.kind, r.reverse, r.editing);
        if let Some(asm) = self.assembly(cx)
            && let (Some(ma), Some(mb)) = (asm.mate(a), asm.mate(if kind == RelationType::Screw { a } else { b }))
            && let Err(e) = noshape_model::assembly::eval::relation_values(kind, ma.mate_type, mb.mate_type)
        {
            self.message = Some((true, e));
            self.relation = Some(r);
            cx.notify();
            return;
        }
        // The coupling is measured from the mates' current values.
        let base = self.assembly(cx).and_then(|asm| self.result.relation_base(asm, kind, [a, b]));
        let base_name = match kind {
            RelationType::Gear => "Gear",
            RelationType::RackAndPinion => "Rack and pinion",
            RelationType::Screw => "Screw",
            RelationType::Linear => "Linear",
        };
        self.edit(&format!("Insert : {base_name} relation"), cx, |asm| {
            if let Some(id) = editing
                && let Some(x) = asm.relations.iter_mut().find(|x| x.id == id)
            {
                x.relation_type = kind;
                x.mates = [a, b];
                x.ratio = ratio;
                x.reverse = reverse;
                x.base = base;
                return;
            }
            let id = MateId(asm.next_id());
            let name = asm.next_feature_name(base_name);
            asm.relations.push(Relation { id, name, relation_type: kind, mates: [a, b], ratio, reverse, suppressed: false, base });
        });
    }

    // ------------------------------------------------------------ explode, positions

    fn toggle_explode_panel(&mut self, cx: &mut Context<Self>) {
        self.panel = if self.panel == Some(SidePanel::Explode) { None } else { Some(SidePanel::Explode) };
        if self.panel.is_none() {
            self.explode_edit = false;
        }
        cx.notify();
    }

    fn explode_view(&self, cx: &App) -> Option<ExplodedView> {
        let mut v = self.explode.and_then(|i| self.assembly(cx).and_then(|a| a.exploded_views.get(i).cloned()))?;
        if let Some(p) = &self.explode_preview {
            v.steps.push(p.clone());
        }
        Some(v)
    }

    /// Re-apply placements + exploded view (playback position, preview step)
    /// to the current result without rebuilding the scene.
    fn apply_display(&mut self, cx: &mut Context<Self>) {
        let view = self.explode_view(cx);
        let progress = match (&self.explode_progress, &self.explode_preview) {
            (_, Some(_)) => f64::INFINITY,
            (Some(p), None) => *p,
            (None, None) => f64::INFINITY,
        };
        let r = Arc::make_mut(&mut self.result);
        let solved = r.solved.clone();
        r.apply_placements_progress(solved, view.as_ref().map(|v| (v, progress)));
    }

    /// Re-apply placements + explode to the current result (display only).
    fn reapply_display(&mut self, cx: &mut Context<Self>) {
        self.apply_display(cx);
        self.rebuild_scene(true, cx);
        cx.notify();
    }

    /// Show solved placements (keeping the exploded view, if any).
    fn apply_solved(&mut self, solved: noshape_model::assembly::eval::Solved, cx: &mut Context<Self>) {
        Arc::make_mut(&mut self.result).solved = solved;
        self.apply_display(cx);
    }

    fn activate_explode(&mut self, i: Option<usize>, cx: &mut Context<Self>) {
        self.explode = i;
        self.explode_progress = None;
        self.explode_preview = None;
        self.explode_play = None;
        if i.is_none() {
            self.explode_edit = false;
        }
        self.reapply_display(cx);
    }

    fn add_exploded_view(&mut self, auto: bool, cx: &mut Context<Self>) {
        let n = self.assembly(cx).map(|a| a.exploded_views.len()).unwrap_or(0);
        let name = format!("Explode {}", n + 1);
        let view = if auto {
            // Auto explode from the assembled positions.
            let mut r = (*self.result).clone();
            let solved = r.solved.clone();
            r.apply_placements(solved, None);
            analysis::auto_exploded_view(&r, &name, 0.9)
        } else {
            ExplodedView { name: name.clone(), steps: vec![] }
        };
        self.edit(&format!("Insert : {name}"), cx, |a| a.exploded_views.push(view));
        self.explode_edit = !auto;
        self.activate_explode(Some(n), cx);
        if !auto {
            self.message = Some((false, "Select instances, then drag or rotate them with the triad to add explode steps.".into()));
        }
    }

    fn delete_exploded_view(&mut self, i: usize, cx: &mut Context<Self>) {
        let name = self.assembly(cx).and_then(|a| a.exploded_views.get(i).map(|v| v.name.clone())).unwrap_or_default();
        self.edit(&format!("Delete : {name}"), cx, |a| {
            if i < a.exploded_views.len() {
                a.exploded_views.remove(i);
            }
        });
        match self.explode {
            Some(e) if e == i => self.activate_explode(None, cx),
            Some(e) if e > i => self.explode = Some(e - 1),
            _ => {}
        }
    }

    fn push_explode_step(&mut self, step: ExplodeStep, cx: &mut Context<Self>) {
        let Some(i) = self.explode else { return };
        self.edit("Edit : Explode step", cx, |a| {
            if let Some(v) = a.exploded_views.get_mut(i) {
                v.steps.push(step);
            }
        });
        self.explode_progress = None;
    }

    fn delete_explode_step(&mut self, k: usize, cx: &mut Context<Self>) {
        let Some(i) = self.explode else { return };
        self.edit("Delete : Explode step", cx, |a| {
            if let Some(v) = a.exploded_views.get_mut(i)
                && k < v.steps.len()
            {
                v.steps.remove(k);
            }
        });
        self.explode_progress = None;
        self.reapply_display(cx);
    }

    fn set_explode_progress(&mut self, p: f64, cx: &mut Context<Self>) {
        self.explode_progress = Some(p);
        self.reapply_display(cx);
    }

    /// Play the exploded view from assembled to fully exploded.
    fn play_explode(&mut self, cx: &mut Context<Self>) {
        let Some(n) = self.explode_view(cx).map(|v| v.steps.len()) else { return };
        if n == 0 {
            return;
        }
        self.explode_progress = Some(0.0);
        self.explode_play = Some(cx.spawn(async move |this, cx| {
            let mut t = 0.0f64;
            loop {
                cx.background_executor().timer(Duration::from_millis(33)).await;
                t += 0.033 / 0.8;
                let done = t >= n as f64;
                let ok = this.update(cx, |this: &mut Self, cx| {
                    this.explode_progress = Some(t.min(n as f64));
                    this.reapply_display(cx);
                    // The slider is rebuilt at the new position.
                    this.explode_slider = None;
                    if done {
                        this.explode_play = None;
                    }
                });
                if ok.is_err() || done {
                    break;
                }
            }
        }));
    }

    // ------------------------------------------------------------ animation

    /// Onshape's split toggle at the viewport's left edge (as the Part
    /// Studio's): collapses or restores the Instances list.
    fn render_list_toggle(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let collapsed = self.ui.list_collapsed;
        div()
            .id("asm-list-toggle")
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
            .child(crate::assets::icon("treelist", 20., cx))
            .tooltip(crate::ui::tooltip(if collapsed { "Show the Instances list" } else { "Collapse the Instances list" }))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.ui.list_collapsed = !this.ui.list_collapsed;
                cx.notify();
            }))
            .into_any_element()
    }

    /// Model evaluation tools at the bottom right (live assembly/10, as the
    /// Part Studio's): Measure, Analysis tools and Mass properties, three
    /// 30x30 buttons 5 px apart on white at 75%.
    fn render_evaluation_tools(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let btn = |id: &'static str, icon: &'static str, on: bool, cx: &App| crate::ui::icon_button(id, icon, 30., on, cx);
        h_flex()
            .id("asm-eval-tools")
            .occlude()
            .absolute()
            .bottom_0()
            .right_0()
            .h(px(30.))
            .px(px(5.))
            .gap(px(5.))
            .bg(crate::theme::with_alpha(pal.panel, 0.75))
            .child(btn("asm-eval-measure", "measure", self.ui.measure.is_some(), cx).tooltip("Show measure details ([)").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_measure(cx))))
            // Live: Show analysis tools opens the tool list; Display mass and
            // section properties toggles the Mass properties dialog.
            .child(btn("asm-eval-analysis", "analysis-tools", false, cx).tooltip("Show analysis tools").on_click(cx.listener(|this, ev: &ClickEvent, window, cx| this.open_analysis_menu(ev.position(), window, cx))))
            .child(btn("asm-eval-mass", "mass-properties", self.ui.mass.is_some(), cx).tooltip("Display mass and section properties").on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_mass(cx))))
            .into_any_element()
    }

    // ------------------------------------------------------------ interference

    fn run_interference(&mut self, cx: &mut Context<Self>) {
        self.panel = Some(SidePanel::Interference);
        self.interference_running = true;
        let result = self.result.clone();
        cx.spawn(async move |this, cx| {
            let list = cx.background_spawn(async move { analysis::interference(&result, 1e-12) }).await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                this.interference_running = false;
                this.interference = Some(list);
                this.rebuild_scene(false, cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    // ------------------------------------------------------------ viewport events

    fn pick(&self, id: Option<u32>) -> Option<AsmPickTarget> {
        id.and_then(|id| self.built.as_ref()?.picks.get(id).cloned())
    }

    fn overlay(&self, id: Option<u32>) -> Option<OverlayPick> {
        id.and_then(|id| self.built.as_ref().and_then(|b| assembly_scene::overlay_pick(b, id)))
    }

    /// What viewport clicks currently feed (a dialog's connector field, snap mode...).
    fn pick_mode(&self, cx: &App) -> Option<PickMode> {
        if self.std_auto_size_active() {
            return Some(PickMode::Entity);
        }
        if let Some(s) = &self.mate {
            let d = s.dialog.read(cx);
            if d.awaiting_pick() {
                return Some(if d.picks_entities() { PickMode::Entity } else { PickMode::Inference });
            }
        }
        if self.mc.as_ref().is_some_and(|m| m.active) || self.pattern.as_ref().is_some_and(|p| p.active.is_some()) || self.snap.is_some() {
            return Some(PickMode::Inference);
        }
        None
    }

    fn update_inference(&mut self, cx: &mut Context<Self>) -> bool {
        let awaiting = self.pick_mode(cx) == Some(PickMode::Inference);
        let old = self.infer_choice;
        if !awaiting {
            self.infer_choice = None;
            return old.is_some();
        }
        // Shift (lock mate inference) keeps the current entity's points while
        // the cursor moves elsewhere; the nearest one is chosen.
        let locked = self.infer_locked && self.infer.is_some();
        if !locked {
            let Some(h) = self.hover.clone() else {
                self.infer_choice = None;
                return old.is_some();
            };
            let key = (h.path.clone(), h.reference.clone());
            if self.infer.as_ref().is_none_or(|(k, occ, _)| *k != key || *occ != h.occ) {
                let units = self.units(cx);
                let cands = self.result.occurrences.get(h.occ).map(|o| inference_candidates(&o.context, Some(&o.body), &h.reference, &units)).unwrap_or_default();
                self.infer = Some((key, h.occ, cands));
                self.infer_choice = None;
            }
        }
        let Some((_, occ, cands)) = &self.infer else { return false };
        let Some(o) = self.result.occurrences.get(*occ) else { return false };
        let vp = self.viewport.read(cx);
        let mut best: Option<(usize, f64)> = None;
        for (i, p) in cands.iter().enumerate() {
            let w = o.transform.transform_point3(p.frame.origin);
            if let Some(s) = vp.project(w) {
                let d = s.distance(self.pointer);
                if best.is_none_or(|(_, bd)| d < bd) {
                    best = Some((i, d));
                }
            }
        }
        self.infer_choice = best.map(|b| b.0);
        old != self.infer_choice
    }

    fn update_capture(&mut self, cx: &mut Context<Self>) {
        let free = self.mate.is_none() && self.anim.is_none() && self.placing.is_none() && self.pick_mode(cx).is_none() && !self.show_mates_mode && self.ui.group.is_none() && self.ui.measure.is_none() && self.ui.curve_dialog.is_none();
        // Snap mode: pressing on geometry starts a snap drag (a click still picks).
        let snapping = self.snap.is_some() && self.mate.is_none() && self.hover.is_some();
        let capture = snapping || (free && (self.manip_hover.is_some() || (self.hover.is_some() && !self.explode_edit)));
        self.viewport.update(cx, |vp, _| vp.tool_captures_left = capture);
    }

    /// Connector chosen by a click in pick mode.
    fn chosen_connector(&mut self, pick: Option<u32>, cx: &mut Context<Self>) -> Option<MateConnectorRef> {
        match self.overlay(pick) {
            Some(OverlayPick::Origin) => return Some(MateConnectorRef::origin()),
            Some(OverlayPick::Connector(i)) => {
                let id = *self.overlay_connectors.get(i)?;
                let c = self.result.connectors.iter().find(|c| c.id == id)?;
                return Some(MateConnectorRef::assembly_connector(c.id, c.owner));
            }
            Some(OverlayPick::Manip(_)) | Some(OverlayPick::Mate(_)) => return None,
            None => {}
        }
        if self.pick_mode(cx) == Some(PickMode::Inference) && self.infer_locked {
            // Shift held: the locked entity's chosen point, wherever the click lands.
            self.update_inference(cx);
            return self.infer.as_ref().zip(self.infer_choice).and_then(|(((path, _), _, c), i)| c.get(i).map(|x| x.connector(path)));
        }
        let target = self.pick(pick)?;
        if self.pick_mode(cx) == Some(PickMode::Entity) {
            return Some(MateConnectorRef::on(target.path[0], target.path[1..].to_vec(), target.reference.clone()));
        }
        self.hover = Some(target.clone());
        self.update_inference(cx);
        let (path, c) = self.infer.as_ref().zip(self.infer_choice).and_then(|(((path, _), _, c), i)| c.get(i).map(|x| (path.clone(), x.connector(path))))?;
        (path == target.path).then_some(c)
    }

    /// Feed a picked connector to whatever is waiting for one.
    fn deliver_connector(&mut self, c: MateConnectorRef, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(dialog) = self.mate.as_ref().map(|s| s.dialog.clone())
            && dialog.read(cx).awaiting_pick()
        {
            if dialog.read(cx).picks_entities() && (c.connector.is_some() || c.instance == InstanceId::ORIGIN) {
                self.message = Some((false, "Tangent mates take faces, edges or vertices.".into()));
                return;
            }
            let is_new = self.mate.as_ref().is_some_and(|s| s.editing.is_none());
            dialog.update(cx, |d, cx| d.set_connector(c, cx));
            // Both connectors picked: the heads-up checkmark appears at the cursor.
            if is_new && !dialog.read(cx).awaiting_pick() {
                let at = self.pointer;
                if let Some(s) = &mut self.mate {
                    s.hud = Some(at);
                }
            }
            return;
        }
        if let Some(m) = &mut self.mc
            && m.active
        {
            if c.connector.is_some() {
                self.message = Some((false, "Pick geometry or the origin for a mate connector.".into()));
                return;
            }
            match m.slot {
                0 => m.on = Some(c),
                s => m.extra[s - 1] = Some(c),
            }
            m.active = false;
            m.slot = 0;
            self.schedule_eval(window, cx);
            return;
        }
        if let Some(p) = &mut self.pattern
            && let Some(slot) = p.active
        {
            if slot == 0 {
                p.axis = Some(c);
                p.active = if p.second && p.axis2.is_none() { Some(1) } else { None };
            } else {
                p.axis2 = Some(c);
                p.active = None;
            }
            return;
        }
        if self.snap.is_some() {
            self.snap_pick(c, cx);
        }
    }

    fn snap_pick(&mut self, c: MateConnectorRef, cx: &mut Context<Self>) {
        match self.snap.clone() {
            Some(None) => {
                if c.instance == InstanceId::ORIGIN {
                    self.message = Some((false, "Snap: first click a face or mate connector on the instance to move.".into()));
                    return;
                }
                self.snap = Some(Some(c));
                self.message = Some((false, "Snap: now click the target face or mate connector.".into()));
            }
            Some(Some(src)) => {
                if c.instance == src.instance && c.instance != InstanceId::ORIGIN {
                    self.message = Some((false, "Snap: pick the target on a different instance.".into()));
                    return;
                }
                self.perform_snap(src, c, cx);
                self.snap = Some(None);
            }
            None => {}
        }
    }

    /// Move the source connector's instance face-to-face onto the target and fasten them.
    fn perform_snap(&mut self, src: MateConnectorRef, dst: MateConnectorRef, cx: &mut Context<Self>) {
        let units = self.units(cx);
        let (Some(fs), Some(fd)) = (self.result.connector_world(&src, &units), self.result.connector_world(&dst, &units)) else {
            self.message = Some((true, "Snap: a mate connector could not be resolved.".into()));
            return;
        };
        let fixed = |id: InstanceId, a: &Assembly| id == InstanceId::ORIGIN || a.instance(id).is_some_and(|i| i.fixed);
        let Some(asm) = self.assembly(cx) else { return };
        // The source moves unless it is fixed and the target is free.
        let (mover, moving_conn, stay_conn, fm, ft) = if fixed(src.instance, asm) && !fixed(dst.instance, asm) { (dst.instance, dst.clone(), src.clone(), fd, fs) } else { (src.instance, src.clone(), dst.clone(), fs, fd) };
        if fixed(mover, asm) {
            self.message = Some((false, "Snap: both instances are fixed.".into()));
            return;
        }
        let Some(pose) = self.result.solved.placement(mover) else { return };
        let np = noshape_model::assembly::eval::snap_placement(&pose, &fm, &ft);
        self.edit("Insert : Fastened (snap)", cx, |a| {
            if let Some(i) = a.instance_mut(mover) {
                i.placement = np;
            }
            let id = MateId(a.next_id());
            let name = a.next_feature_name("Fastened");
            a.mates.push(Mate::new(id, name, MateType::Fastened, moving_conn, stay_conn));
        });
        self.message = Some((false, "Snapped and fastened. Pick another pair, or press Shift+S to leave snap mode.".into()));
    }

    fn on_viewport_event(&mut self, _: &Entity<Viewport>, ev: &ViewportEvent, window: &mut Window, cx: &mut Context<Self>) {
        match ev {
            ViewportEvent::Hover(id) => {
                if self.drag.as_ref().is_some_and(|d| d.started) {
                    return;
                }
                let overlay = self.overlay(*id);
                let manip = match overlay {
                    Some(OverlayPick::Manip(h)) => Some(h),
                    _ => None,
                };
                let ov = overlay.filter(|o| !matches!(o, OverlayPick::Manip(_)));
                let changed_overlay = manip != self.manip_hover || ov != self.hover_overlay;
                self.manip_hover = manip;
                self.hover_overlay = ov;
                let t = if overlay.is_some() { None } else { self.pick(*id) };
                let changed = changed_overlay || t.as_ref().map(|t| (&t.path, &t.reference)) != self.hover.as_ref().map(|h| (&h.path, &h.reference));
                self.hover = t;
                self.update_capture(cx);
                let inf = self.update_inference(cx);
                if changed || inf {
                    self.refresh_scene(false, cx);
                }
            }
            ViewportEvent::PointerMove { position, modifiers } => {
                // Alt: transparent instances become pickable (Onshape Alt+click).
                if modifiers.alt != self.ui.alt_through {
                    self.ui.alt_through = modifiers.alt;
                    if self.isolate.is_some() || self.ui.transparent.is_some() {
                        // Pickability changes: the pick buffer must be redrawn.
                        self.rebuild_scene(true, cx);
                    }
                }
                self.pointer = *position;
                // Shift (hold) locks mate inference to the entity under the cursor.
                self.infer_locked = modifiers.shift;
                if self.placing.is_some() {
                    if self.update_placing(cx) {
                        self.rebuild_scene(true, cx);
                    }
                } else if self.pick_mode(cx).is_some() {
                    let before = self.infer_choice;
                    self.update_inference(cx);
                    if before != self.infer_choice {
                        self.refresh_scene(false, cx);
                    }
                }
            }
            ViewportEvent::Click { pick, position, modifiers, .. } => {
                self.focus_from_viewport(window, cx);
                self.ui.select_other = None;
                self.ui.ctx_menu = None;
                self.ui.mini_toolbar = None;
                self.triad_menu = None;
                self.pointer = *position;
                // Clicking a triad handle selects it (its context menu needs that).
                let manip = match self.overlay(*pick) {
                    Some(OverlayPick::Manip(h)) => Some(h),
                    _ => None,
                };
                if manip.is_some() || self.ui.manip_selected.is_some() {
                    self.ui.manip_selected = manip;
                    self.refresh_scene(false, cx);
                    if manip.is_some() {
                        return;
                    }
                }
                if self.placing.is_some() {
                    // Only a click after the cursor moved over the viewport places
                    // (the click that chose the item in the Insert panel does not).
                    if self.placing_point.is_some() {
                        self.update_placing(cx);
                        self.finish_placing(true, window, cx);
                    }
                    return;
                }
                if self.std_auto_size_active() {
                    // Auto-size fastener: the clicked hole sizes the part.
                    if let Some(t) = self.pick(*pick) {
                        self.std_pick_hole(t, cx);
                    }
                    return;
                }
                if self.ui.measure.is_some() {
                    // Measure tool: clicked entities are measured.
                    if let Some(t) = self.pick(*pick) {
                        self.measure_pick(t, cx);
                    }
                } else if self.ui.curve_dialog.is_some() {
                    // Curve/surface analysis: clicked edges and faces are analyzed.
                    if let Some(t) = self.pick(*pick) {
                        self.curve_pick(t, cx);
                    }
                } else if self.ui.group.is_some() {
                    // Group dialog: clicking an instance adds or removes it.
                    if let Some(top) = self.pick(*pick).map(|t| t.path[0]) {
                        self.toggle_group_member(top, cx);
                    }
                } else if self.pick_mode(cx).is_some() {
                    if let Some(c) = self.chosen_connector(*pick, cx) {
                        self.deliver_connector(c, window, cx);
                    }
                } else if self.show_mates_mode {
                    let top = self.pick(*pick).map(|t| t.path[0]);
                    self.mates_pinned = if top == self.mates_pinned { None } else { top };
                    self.selected = top.into_iter().collect();
                } else if let Some(OverlayPick::Mate(i)) = self.overlay(*pick) {
                    // Clicking a mate indicator selects the mate (highlighted in the
                    // list), or fills the relation dialog's next mate field.
                    let id = self.ui.overlay_mates.get(i).copied();
                    if let (Some(id), true) = (id, self.relation.is_some()) {
                        self.pick_relation_mate(id, cx);
                    } else if let (Some(id), true) = (id, self.ui.cfg_mode.is_some()) {
                        self.configure_pick_mate(id, window, cx);
                    } else {
                        self.selected_mate = id;
                        self.selected.clear();
                    }
                } else if let Some(OverlayPick::Connector(i)) = self.overlay(*pick) {
                    // Select an assembly mate connector (highlights it in the list).
                    self.selected_mate = self.overlay_connectors.get(i).copied();
                    self.selected.clear();
                } else {
                    let target = self.pick(*pick);
                    self.select_target(target, modifiers.shift || modifiers.secondary(), cx);
                    self.triad_at_click(*position, cx);
                    self.ui.scroll_to_selection = true;
                }
                self.refresh_scene(false, cx);
                cx.notify();
            }
            ViewportEvent::ToolDown { position, .. } => {
                self.focus_from_viewport(window, cx);
                self.ui.ctx_menu = None;
                self.ui.mini_toolbar = None;
                self.triad_menu = None;
                if let (Some(h), Some(f)) = (self.manip_hover, self.manipulator(cx)) {
                    let ids = self.manip_targets(cx);
                    let top = ids[0];
                    let axes = [f.x(), f.y(), f.z()];
                    let axis = match h {
                        ManipHandle::Axis(k) | ManipHandle::Ring(k) | ManipHandle::Plane(k) => axes[k],
                        ManipHandle::Center => self.viewport.read(cx).camera.forward(),
                    };
                    let pose = self.result.solved.placement(top).map(|p| placement_to_frame(&p)).unwrap_or_default();
                    let local = pose.inverse().transform_point(f.origin);
                    let start: HashMap<InstanceId, Placement> = ids.iter().filter_map(|id| self.result.solved.placement(*id).map(|p| (*id, p))).collect();
                    let ring_v0 = if matches!(h, ManipHandle::Ring(_)) { self.ring_vector(*position, f.origin, axis, cx).unwrap_or(DVec3::X) } else { DVec3::ZERO };
                    let grab0 = if matches!(h, ManipHandle::Plane(_) | ManipHandle::Center) { self.ray_plane(*position, f.origin, axis, cx).unwrap_or(f.origin) } else { DVec3::ZERO };
                    self.manip_value = None;
                    self.drag = Some(DragState {
                        top,
                        press: *position,
                        started: false,
                        local,
                        plane_point: f.origin,
                        plane_normal: axis,
                        pending: None,
                        busy: false,
                        handle: Some(h),
                        ids,
                        start,
                        ring_v0,
                        value: 0.0,
                        grab0,
                        offset: DVec3::ZERO,
                        triad_preview: None,
                        explode: self.explode_edit,
                        nested: None,
                    });
                    return;
                }
                if self.snap.is_some() {
                    self.start_snap_drag(*position, cx);
                    return;
                }
                let Some(t) = self.hover.clone() else { return };
                let top = t.path[0];
                // A part of a flexible subassembly is dragged on its own (Onshape).
                let nested = (t.path.len() > 1 && self.result.prepared.is_flexible_child(top, t.path[1])).then(|| t.path[1]);
                let (origin, dir) = self.viewport.read(cx).ray(*position);
                // Grab point: closest point on the ray to the instance's bounds center.
                let center = match nested {
                    Some(c) => self.result.occurrences.iter().filter(|o| o.path.len() > 1 && o.path[0] == top && o.path[1] == c).filter_map(|o| o.world_bounds()).fold(None, |acc: Option<(DVec3, DVec3)>, (a, b)| Some(acc.map(|(x, y)| (x.min(a), y.max(b))).unwrap_or((a, b)))),
                    None => self.result.instance_bounds(top),
                }
                .map(|(a, b)| (a + b) * 0.5)
                .unwrap_or(DVec3::ZERO);
                let tt = (center - origin).dot(dir);
                let grab = origin + dir * tt;
                let pose = match nested {
                    Some(c) => self.result.solved.nested_world(top, c).unwrap_or_default(),
                    None => self.result.solved.placement(top).map(|p| placement_to_frame(&p)).unwrap_or_default(),
                };
                let local = pose.inverse().transform_point(grab);
                let normal = self.viewport.read(cx).camera.forward();
                self.drag = Some(DragState {
                    top,
                    press: *position,
                    started: false,
                    local,
                    plane_point: grab,
                    plane_normal: normal,
                    pending: None,
                    busy: false,
                    handle: None,
                    ids: vec![top],
                    start: HashMap::new(),
                    ring_v0: DVec3::ZERO,
                    value: 0.0,
                    grab0: DVec3::ZERO,
                    offset: DVec3::ZERO,
                    triad_preview: None,
                    explode: false,
                    nested,
                });
            }
            ViewportEvent::ToolDrag { position, .. } => {
                if self.snap_drag.is_some() {
                    self.update_snap_drag(*position, cx);
                    return;
                }
                let Some(d) = &self.drag else { return };
                if !d.started && position.distance(d.press) > 3.0 {
                    let (top, explode) = (d.top, d.explode);
                    self.drag.as_mut().unwrap().started = true;
                    // A fixed instance does not move (live 562: no-drop cursor and
                    // the fix glyph, no message).
                    if !explode && self.assembly(cx).and_then(|a| a.instance(top)).is_some_and(|i| i.fixed) {
                        self.ui.fixed_drag = Some(*position);
                        // Nothing stays pre-highlighted while dragging it (live 562).
                        self.hover = None;
                        self.refresh_scene(false, cx);
                    }
                }
                let Some(d) = &self.drag else { return };
                if !d.started {
                    return;
                }
                if self.ui.fixed_drag.is_some() {
                    self.ui.fixed_drag = Some(*position);
                    cx.notify();
                }
                let (origin, dir) = self.viewport.read(cx).ray(*position);
                match d.handle {
                    Some(ManipHandle::Ring(_)) => {
                        let (center, axis, v0) = (d.plane_point, d.plane_normal, d.ring_v0);
                        let Some(v) = self.ring_vector(*position, center, axis, cx) else { return };
                        let angle = v0.cross(v).dot(axis).atan2(v0.dot(v));
                        let d = self.drag.as_mut().unwrap();
                        d.value = angle;
                        d.pending = Some(DVec3::ZERO);
                    }
                    Some(ManipHandle::Axis(_)) => {
                        let axis = d.plane_normal;
                        // Closest point on the axis line to the cursor ray.
                        let w = d.plane_point - origin;
                        let (a, b, c) = (axis.dot(axis), axis.dot(dir), dir.dot(dir));
                        let (dd, e) = (axis.dot(w), dir.dot(w));
                        let den = a * c - b * b;
                        if den.abs() < 1e-12 {
                            return;
                        }
                        let s = (b * e - c * dd) / den;
                        let p = d.plane_point + axis * s;
                        let d = self.drag.as_mut().unwrap();
                        d.value = s;
                        d.pending = Some(p);
                    }
                    Some(ManipHandle::Plane(_)) => {
                        let (pp, n, g0) = (d.plane_point, d.plane_normal, d.grab0);
                        let Some(hit) = self.ray_plane(*position, pp, n, cx) else { return };
                        let d = self.drag.as_mut().unwrap();
                        d.offset = hit - g0;
                        d.pending = Some(pp + d.offset);
                    }
                    Some(ManipHandle::Center) => {
                        // Reposition the manipulator itself (snapping to mate connectors).
                        let frame = self.triad_snap(*position, cx).unwrap_or_else(|| {
                            let (pp, n, g0) = (d.plane_point, d.plane_normal, d.grab0);
                            let rot = self.manipulator(cx).map(|f| f.rotation).unwrap_or_default();
                            let hit = self.ray_plane(*position, pp, n, cx).unwrap_or(g0);
                            Frame::new(pp + (hit - g0), rot)
                        });
                        if let Some(d) = self.drag.as_mut() {
                            d.triad_preview = Some(frame);
                        }
                        self.refresh_scene(false, cx);
                        cx.notify();
                        return;
                    }
                    None => {
                        let denom = dir.dot(d.plane_normal);
                        if denom.abs() < 1e-9 {
                            return;
                        }
                        let t = (d.plane_point - origin).dot(d.plane_normal) / denom;
                        self.drag.as_mut().unwrap().pending = Some(origin + dir * t);
                    }
                }
                if self.drag.as_ref().is_some_and(|d| d.explode) {
                    self.update_explode_preview(cx);
                } else if !self.drag.as_ref().is_some_and(|d| d.busy) {
                    self.run_drag_solve(cx);
                }
            }
            ViewportEvent::ToolUp { modifiers, .. } => {
                if self.ui.fixed_drag.take().is_some() {
                    cx.notify();
                }
                if self.snap_drag.is_some() {
                    self.finish_snap_drag(window, cx);
                    return;
                }
                let Some(d) = self.drag.take() else { return };
                if let (false, Some(h)) = (d.started, d.handle) {
                    // A click on a triad handle selects it (right-click it for its menu).
                    self.ui.manip_selected = Some(h);
                    self.refresh_scene(false, cx);
                    cx.notify();
                    return;
                }
                if d.handle == Some(ManipHandle::Center) {
                    // The manipulator keeps its new frame relative to the part.
                    if d.started
                        && let Some(f) = d.triad_preview
                        && let Some(p) = self.result.solved.placement(d.ids[0])
                    {
                        self.triad = Some((d.ids.clone(), placement_to_frame(&p).inverse().mul(&f)));
                    }
                    self.refresh_scene(false, cx);
                    cx.notify();
                    return;
                }
                if d.started {
                    if d.explode {
                        if let Some(step) = self.explode_preview.take()
                            && (d.value.abs() > 1e-9 || d.offset.length() > 1e-9)
                        {
                            self.push_explode_step(step, cx);
                            self.open_manip_value(&d, window, cx);
                        }
                        self.reapply_display(cx);
                    } else {
                        let name = self.assembly(cx).and_then(|a| a.instance(d.top).map(|i| i.name.clone())).unwrap_or_default();
                        let fixed = self.assembly(cx).and_then(|a| a.instance(d.top)).is_some_and(|i| i.fixed);
                        if fixed {
                            self.ui.fixed_drag = None;
                            cx.notify();
                        } else {
                            self.edit(&format!("Move : {name}"), cx, |_| {});
                            if d.handle.is_some() {
                                self.open_manip_value(&d, window, cx);
                            }
                        }
                    }
                } else if d.handle.is_none() {
                    let t = self.hover.clone();
                    self.select_target(t, modifiers.shift || modifiers.secondary(), cx);
                    self.triad_at_click(d.press, cx);
                    self.ui.scroll_to_selection = true;
                    self.refresh_scene(false, cx);
                }
                cx.notify();
            }
            ViewportEvent::BoxSelect { ids, modifiers } => {
                // Box selection selects the instances it touches (Shift/Ctrl adds).
                if self.pick_mode(cx).is_some() || self.placing.is_some() {
                    return;
                }
                let mut tops: Vec<InstanceId> = Vec::new();
                for t in ids.iter().filter_map(|id| self.pick(Some(*id))) {
                    if !tops.contains(&t.path[0]) {
                        tops.push(t.path[0]);
                    }
                }
                if !(modifiers.shift || modifiers.secondary()) {
                    self.selected.clear();
                }
                for t in tops {
                    if !self.selected.contains(&t) {
                        self.selected.push(t);
                    }
                }
                self.selected_mate = None;
                self.ui.scroll_to_selection = true;
                if self.triad.as_ref().is_some_and(|(ids, _)| *ids != self.selected) {
                    self.triad = None;
                }
                self.refresh_scene(false, cx);
                cx.notify();
            }
            ViewportEvent::ContextMenu { pick, position } => {
                // Triad menu: right-click the selected handle (Onshape);
                // otherwise the triad is ignored.
                if let Some(OverlayPick::Manip(h)) = self.overlay(*pick)
                    && self.ui.manip_selected == Some(h)
                {
                    self.ui.ctx_menu = None;
                    self.triad_menu = Some((*position, h));
                    cx.notify();
                    return;
                }
                if let Some(OverlayPick::Mate(i)) = self.overlay(*pick)
                    && let Some(id) = self.ui.overlay_mates.get(i).copied()
                {
                    // Mate indicator context menu (Edit, Animate...).
                    self.selected_mate = Some(id);
                    self.selected.clear();
                    self.open_mate_menu(id, *position, window, cx);
                    return;
                }
                self.triad_menu = None;
                // Onshape: a selected instance gets the instance menu; an
                // unselected one (or empty space) the general menu.
                match self.pick(*pick).map(|t| t.path[0]).filter(|top| self.selected.contains(top)) {
                    Some(top) => self.open_graphics_menu(Some(top), *position, window, cx),
                    None => self.open_graphics_menu(None, *position, window, cx),
                }
                cx.notify();
            }
            _ => {}
        }
    }

    /// Unit vector from a ring center toward the cursor, in the ring plane.
    fn ring_vector(&self, position: DVec2, center: DVec3, axis: DVec3, cx: &App) -> Option<DVec3> {
        let (o, d) = self.viewport.read(cx).ray(position);
        let den = d.dot(axis);
        let p = if den.abs() > 1e-6 {
            o + d * ((center - o).dot(axis) / den)
        } else {
            // Edge-on ring: closest point of the ray to the center.
            o + d * (center - o).dot(d)
        };
        let v = (p - center) - axis * (p - center).dot(axis);
        (v.length() > 1e-12).then(|| v.normalize())
    }

    /// Rigid motion of a manipulator drag: translation along the axis by
    /// `value`, rotation by `value` about it through the triad center, or
    /// translation by `offset` in a plane.
    fn manip_motion(handle: ManipHandle, axis: DVec3, center: DVec3, value: f64, offset: DVec3) -> Frame {
        match handle {
            ManipHandle::Axis(_) => Frame::new(axis * value, glam::DQuat::IDENTITY),
            ManipHandle::Ring(_) => {
                let r = glam::DQuat::from_axis_angle(axis.normalize(), value);
                Frame::new(center - r * center, r)
            }
            ManipHandle::Plane(_) => Frame::new(offset, glam::DQuat::IDENTITY),
            ManipHandle::Center => Frame::default(),
        }
    }

    /// Start placements moved by a rigid motion.
    fn moved_placements(m: &Frame, start: &HashMap<InstanceId, Placement>) -> Vec<(InstanceId, Placement)> {
        start.iter().map(|(id, p)| (*id, frame_to_placement(&m.mul(&placement_to_frame(p))))).collect()
    }

    fn manip_targets_for(handle: ManipHandle, axis: DVec3, center: DVec3, value: f64, start: &HashMap<InstanceId, Placement>) -> Vec<(InstanceId, Placement)> {
        Self::moved_placements(&Self::manip_motion(handle, axis, center, value, DVec3::ZERO), start)
    }

    /// Cursor ray hit on a plane.
    fn ray_plane(&self, position: DVec2, point: DVec3, normal: DVec3, cx: &App) -> Option<DVec3> {
        let (o, d) = self.viewport.read(cx).ray(position);
        let den = d.dot(normal);
        if den.abs() < 1e-9 {
            return None;
        }
        Some(o + d * ((point - o).dot(normal) / den))
    }

    fn update_explode_preview(&mut self, cx: &mut Context<Self>) {
        let Some(d) = &self.drag else { return };
        let step = match d.handle {
            Some(ManipHandle::Axis(_)) => ExplodeStep::translate(d.ids.clone(), (d.plane_normal * d.value).into()),
            Some(ManipHandle::Ring(_)) => ExplodeStep::rotate(d.ids.clone(), d.plane_normal.into(), d.value, d.plane_point.into()),
            Some(ManipHandle::Plane(_)) => ExplodeStep::translate(d.ids.clone(), d.offset.into()),
            Some(ManipHandle::Center) | None => return,
        };
        self.explode_preview = Some(step);
        self.reapply_display(cx);
    }

    fn select_target(&mut self, target: Option<AsmPickTarget>, additive: bool, _cx: &mut Context<Self>) {
        self.selected_mate = None;
        self.manip_value = None;
        match target {
            Some(t) => {
                let top = t.path[0];
                if additive {
                    if let Some(i) = self.selected.iter().position(|x| *x == top) {
                        self.selected.remove(i);
                    } else {
                        self.selected.push(top);
                    }
                } else {
                    self.selected = vec![top];
                }
            }
            None if !additive => self.selected.clear(),
            None => {}
        }
        // A repositioned manipulator belongs to its instances until they are deselected.
        if self.triad.as_ref().is_some_and(|(ids, _)| *ids != self.selected) {
            self.triad = None;
        }
        self.ui.manip_selected = None;
    }

    /// Onshape shows the triad at the point where a single instance was
    /// clicked (axes along the world axes).
    fn triad_at_click(&mut self, position: DVec2, cx: &App) {
        let [top] = self.selected[..] else { return };
        let (o, d) = self.viewport.read(cx).ray(position);
        let mut best: Option<f64> = None;
        for occ in self.result.occurrences.iter().filter(|occ| occ.top() == top && !occ.hidden) {
            let inv = occ.transform.inverse();
            let (lo, ld) = (inv.transform_point3(o), inv.transform_vector3(d));
            let m = &occ.mesh;
            let pos = |i: u32| m.positions.get(i as usize).map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)).unwrap_or_default();
            for t in m.indices.as_chunks::<3>().0 {
                if let Some(tt) = panels::ray_triangle(lo, ld, pos(t[0]), pos(t[1]), pos(t[2]))
                    && best.is_none_or(|b| tt < b)
                {
                    best = Some(tt);
                }
            }
        }
        let (Some(t), Some(p)) = (best, self.result.solved.placement(top)) else { return };
        let world = Frame::new(o + d * t, glam::DQuat::IDENTITY);
        self.triad = Some((vec![top], placement_to_frame(&p).inverse().mul(&world)));
    }

    fn run_drag_solve(&mut self, cx: &mut Context<Self>) {
        let Some(d) = &mut self.drag else { return };
        let Some(target) = d.pending.take() else { return };
        d.busy = true;
        let prepared = self.result.prepared.clone();
        let start: HashMap<InstanceId, Placement> = self.result.solved.placements.iter().cloned().collect();
        let opts = match d.handle {
            Some(h @ ManipHandle::Ring(_)) => SolveOptions { targets: Self::manip_targets_for(h, d.plane_normal, d.plane_point, d.value, &d.start), start: Some(start), nested_start: Some(self.result.solved.nested.clone()), ..Default::default() },
            Some(h @ (ManipHandle::Axis(_) | ManipHandle::Plane(_))) if self.result.solved.instance_dof.get(&d.top).copied() == Some(6) => {
                // Free instance: exact translation from the drag start.
                let m = Self::manip_motion(h, d.plane_normal, d.plane_point, d.value, d.offset);
                SolveOptions { targets: Self::moved_placements(&m, &d.start), start: Some(start), nested_start: Some(self.result.solved.nested.clone()), ..Default::default() }
            }
            _ => match d.nested {
                Some(c) => SolveOptions { nested_drags: vec![((d.top, c), d.local, target)], start: Some(start), nested_start: Some(self.result.solved.nested.clone()), ..Default::default() },
                None => SolveOptions { drags: vec![(d.top, d.local, target)], start: Some(start), nested_start: Some(self.result.solved.nested.clone()), ..Default::default() },
            },
        };
        cx.spawn(async move |this, cx| {
            let solved = cx.background_spawn(async move { prepared.solve(&opts) }).await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                let Some(d) = &mut this.drag else { return };
                d.busy = false;
                let again = d.pending.is_some();
                this.apply_solved(solved, cx);
                this.rebuild_scene(true, cx);
                if again {
                    this.run_drag_solve(cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn view(&mut self, v: StandardView, cx: &mut Context<Self>) {
        self.viewport.update(cx, |vp, cx| vp.animate_to_view(v, cx));
    }

    // ------------------------------------------------------------ render: toolbar

    fn render_toolbar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let sep = crate::ui::tool_separator;
        // Tool search's "Highlight tool in toolbar": `#fcda7f`, border `#fab601`.
        let found = self.search_found_icon();
        let lit = move |b: Button, icon: &str| b.when(found == Some(icon), |b| b.bg(rgb(0xfcda7f)).border_1().border_color(rgb(0xfab601)));
        // Tooltips: title (+ shortcut), then the description after a pause.
        let rich = |id: &'static str, tip: &'static str, icon: &str, b: Button| crate::ui::with_rich_tooltip(SharedString::from(format!("tt-{id}")), tip, search::tool_description(icon), b);
        let tool = |this: &Self, id: &'static str, icon: &'static str, tip: &'static str, on: bool, cx: &mut Context<Self>, f: fn(&mut Self, &mut Window, &mut Context<Self>)| {
            let _ = this;
            rich(id, tip, icon, lit(crate::ui::tool_button(id, icon, on, cx), icon).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| f(this, window, cx))))
        };
        let mate = |this: &Self, id: &'static str, t: MateType, tip: &'static str, cx: &mut Context<Self>| {
            let on = this.mate.as_ref().is_some_and(|m| m.dialog.read(cx).mate_type() == t);
            let icon = crate::assembly_mate_dialog::mate_icon_name(t);
            rich(id, tip, icon, lit(crate::ui::tool_button(id, icon, on, cx), icon).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.start_mate(t, window, cx))))
        };
        let relation = |id: &'static str, icon: &'static str, tip: &'static str, kind: RelationType, cx: &mut Context<Self>| {
            rich(id, tip, icon, lit(crate::ui::tool_button(id, icon, false, cx), icon).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.open_relation(kind, None, window, cx))))
        };
        // Editing a named position: only Undo / Redo, and the yellow
        // "Editing <name> [Done]" banner at the top centre (live 105).
        let banner = self.render_np_banner(cx).or_else(|| self.explode_banner(cx));
        if let Some(banner) = banner {
            return crate::ui::toolbar()
                .relative()
                .child(crate::ui::undo_redo(&self.doc, cx))
                .child(div().absolute().top(px(0.)).left_0().right_0().flex().justify_center().child(banner))
                .into_any_element();
        }
        // Configure assembly features: the toolbar greyed out with the yellow
        // banner over it (live 192).
        let cfg_banner = self.render_cfg_banner(cx);
        // Onshape's assembly toolbar, in its order (live DOM, 1.222): no
        // dropdowns; exploded views, named positions, interference and the
        // BOM live in the right panel rail.
        // Tool sets in the user's order without removed tools (Customize
        // toolbar), each built by its id.
        let prefs = self.toolbar_prefs();
        let mut bar = crate::ui::toolbar().child(crate::ui::undo_redo(&self.doc, cx));
        for (set, ids) in prefs.sets() {
            let tools: Vec<AnyElement> = ids
                .iter()
                .map(|id| match *id {
                    "tb-update-refs" => {
                        // Live 607, 621: enabled (blue) only while another
                        // document has a newer version than the one referenced.
                        let enabled = self.has_newer_references(cx);
                        rich(
                            "tb-update-refs",
                            "Update all references to latest versions",
                            "update-all-button",
                            // Disabled: faded to 0.4 (live 607 vs 621).
                            gpui_kit::component::Disableable::disabled(crate::ui::tool_button("tb-update-refs", "update-all-button", false, cx), !enabled).when(!enabled, |b| b.opacity(0.4)).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.open_reference_manager(None, cx))),
                        )
                        .into_any_element()
                    }
                    "tb-insert" => rich("tb-insert", "Insert parts and assemblies (i)", "insert", lit(crate::ui::text_tool_button("tb-insert", "insert", "Insert", self.insert_open, cx), "insert").on_click(cx.listener(|this, _: &ClickEvent, w, cx| this.toggle_insert(w, cx)))).into_any_element(),
                    "tb-mc" => tool(self, "tb-mc", "mate-connector-assembly", "Mate connector (ctrl+m)", self.mc.is_some(), cx, |this, w, cx| this.open_mc(None, w, cx)).into_any_element(),
                    "tb-fastened" => mate(self, "tb-fastened", MateType::Fastened, "Fastened mate (m)", cx).into_any_element(),
                    "tb-revolute" => mate(self, "tb-revolute", MateType::Revolute, "Revolute mate (m)", cx).into_any_element(),
                    "tb-slider" => mate(self, "tb-slider", MateType::Slider, "Slider mate (m)", cx).into_any_element(),
                    "tb-planar" => mate(self, "tb-planar", MateType::Planar, "Planar mate (m)", cx).into_any_element(),
                    "tb-cyl" => mate(self, "tb-cyl", MateType::Cylindrical, "Cylindrical mate (m)", cx).into_any_element(),
                    "tb-pinslot" => mate(self, "tb-pinslot", MateType::PinSlot, "Pin slot mate (m)", cx).into_any_element(),
                    "tb-ball" => mate(self, "tb-ball", MateType::Ball, "Ball mate (m)", cx).into_any_element(),
                    "tb-parallel" => mate(self, "tb-parallel", MateType::Parallel, "Parallel mate (m)", cx).into_any_element(),
                    "tb-tangent" => mate(self, "tb-tangent", MateType::Tangent, "Tangent mate", cx).into_any_element(),
                    "tb-width" => mate(self, "tb-width", MateType::Width, "Width mate", cx).into_any_element(),
                    "tb-group" => tool(self, "tb-group", "group", "Group", self.ui.group.is_some(), cx, |this, _, cx| this.create_group(cx)).into_any_element(),
                    "tb-snap" => tool(self, "tb-snap", "snap-mode", "Snap mode (shift+s)", self.snap.is_some(), cx, |this, w, cx| this.toggle_snap(w, cx)).into_any_element(),
                    "tb-show-mates" => tool(self, "tb-show-mates", "show-mates", "Show mates mode (h)", self.show_mates_mode, cx, |this, _, cx| this.toggle_show_mates_mode(cx)).into_any_element(),
                    "tb-replicate" => tool(self, "tb-replicate", "replicate", "Replicate", false, cx, |this, w, cx| this.open_pattern(PatternDialogKind::Replicate, w, cx)).into_any_element(),
                    "tb-replace" => tool(self, "tb-replace", "replace-part", "Replace instance", self.replace_panel.is_some(), cx, |this, _, cx| this.replace_selected(cx)).into_any_element(),
                    "tb-linpat" => tool(self, "tb-linpat", "linear-pattern", "Assembly linear pattern", false, cx, |this, w, cx| this.open_pattern(PatternDialogKind::Linear, w, cx)).into_any_element(),
                    "tb-circpat" => tool(self, "tb-circpat", "circular-pattern", "Assembly circular pattern", false, cx, |this, w, cx| this.open_pattern(PatternDialogKind::Circular, w, cx)).into_any_element(),
                    "tb-mirror" => tool(self, "tb-mirror", "mirror", "Assembly mirror", false, cx, |this, w, cx| this.open_pattern(PatternDialogKind::Mirror, w, cx)).into_any_element(),
                    "tb-gear" => relation("tb-gear", "gear-relation", "Gear relation", RelationType::Gear, cx).into_any_element(),
                    "tb-rack" => relation("tb-rack", "rack-pinion", "Rack and pinion relation", RelationType::RackAndPinion, cx).into_any_element(),
                    "tb-screw" => relation("tb-screw", "screw-relation", "Screw relation", RelationType::Screw, cx).into_any_element(),
                    "tb-linear" => relation("tb-linear", "linear-relation", "Linear relation", RelationType::Linear, cx).into_any_element(),
                    "tb-display-states" => tool(self, "tb-display-states", "display-states", "Display states", self.ui.display_states, cx, |this, w, cx| this.toggle_display_states(w, cx)).into_any_element(),
                    _ => tool(self, "tb-ps-in-context", "create-partstudio-incontext", "Create Part Studio in context", self.ui.ps_in_context.is_some(), cx, |this, _, cx| this.open_ps_in_context(cx)).into_any_element(),
                })
                .collect();
            bar = bar.child(sep()).child(self.render_toolset(set, tools, ids, cx));
        }
        let bar = bar
            .child(div().w(px(10.)))
            .when(!prefs.hide_search, |d| {
                d.child(crate::ui::search_tools_toggle("asm-search", self.ui.tool_search.is_some()).on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    if this.ui.tool_search.is_some() {
                        this.close_tool_search(window, cx);
                    } else {
                        this.open_tool_search(window, cx);
                    }
                })))
            })
            .child(div().flex_1())
            .when(self.evaluating, |d| d.child(div().pr(px(8.)).text_xs().text_color(pal.text_faint).child("solving…")));
        // Right-click: Customize toolbar… / Hide "Search tools" (live 331z).
        let menu = self.toolbar_menu(cx.entity());
        let bar = div().id("asm-toolbar").child(bar).os_context_menu(menu);
        match cfg_banner {
            Some(banner) => div()
                .relative()
                .child(div().opacity(0.4).child(bar))
                .child(div().id("tb-cfg-block").occlude().absolute().top_0().left_0().size_full())
                .child(div().absolute().top(px(0.)).left_0().right_0().flex().justify_center().child(banner))
                .into_any_element(),
            None => bar.into_any_element(),
        }
    }

    /// Onshape's right panel rail (§3.6): 33 x 34 white buttons with `#aaa`
    /// outlines, flush with the right edge and vertically centered.
    fn render_panel_rail(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let exploded = self.explode.is_some();
        let btn = |id: &'static str, icon: &'static str, tip: &'static str, on: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .relative()
                .w(px(33.))
                .h(px(34.))
                .mt(px(-1.))
                .flex()
                .items_center()
                .justify_center()
                .bg(if on { pal.row_hover } else { pal.panel })
                .hover(|d| d.bg(pal.panel_hover))
                .cursor_pointer()
                .border_1()
                .border_color(pal.border_strong)
                .child(crate::assets::icon(icon, 20., cx))
                .when(on, |d| d.child(div().absolute().right_0().top_0().bottom_0().w(px(3.)).bg(pal.accent)))
                .tooltip(crate::ui::tooltip(tip))
        };
        let toggle = |this: &mut Self, panel: SidePanel, cx: &mut Context<Self>| {
            this.panel = if this.panel == Some(panel) { None } else { Some(panel) };
            cx.notify();
        };
        div()
            .absolute()
            .top_0()
            .bottom_0()
            .right(px(-1.))
            .flex()
            .flex_col()
            .justify_center()
            // Live strip (assembly/181-*): BOM Table, Configuration panel,
            // Exploded views, Named positions, (Simulation: excluded),
            // Variable table. Interference is in the top row's menu
            // (Check interference…), Animate in the mate row's.
            .child(btn("rail-bom", "bom", "BOM Table", self.panel == Some(SidePanel::Bom), cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| toggle(this, SidePanel::Bom, cx))))
            .child(btn("rail-config", "config-show", "Configuration panel", self.panel == Some(SidePanel::Configurations), cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| toggle(this, SidePanel::Configurations, cx))))
            .child(btn("rail-explode", "explode", "Exploded views", exploded || self.panel == Some(SidePanel::Explode), cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.toggle_explode_panel(cx))))
            .child(btn("rail-named", "named-position", "Named positions", self.panel == Some(SidePanel::Positions), cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| toggle(this, SidePanel::Positions, cx))))
            .child(btn("rail-variables", "variable-table", "Variable table", self.panel == Some(SidePanel::Variables), cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| toggle(this, SidePanel::Variables, cx))))
            .into_any_element()
    }

    // ------------------------------------------------------------ render: list

    fn instance_row(&self, node: Option<&InstanceNode>, inst: Option<&Instance>, path: Vec<InstanceId>, depth: usize, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let top = path[0];
        let is_top = path.len() == 1;
        let name = inst.map(|i| i.name.clone()).or_else(|| node.map(|n| n.name.clone())).unwrap_or_default();
        let kind = node.map(|n| n.kind).unwrap_or(match inst.map(|i| &i.source) {
            Some(InstanceSource::Assembly { .. }) => NodeKind::Assembly,
            Some(InstanceSource::PartStudio { .. }) => NodeKind::PartStudio,
            _ => NodeKind::Part,
        });
        let fixed = inst.map(|i| i.fixed).unwrap_or_else(|| node.is_some_and(|n| n.fixed));
        let hidden = inst.map(|i| i.hidden).unwrap_or_else(|| node.is_some_and(|n| n.hidden));
        // Dynamic suppression shows through the evaluated node.
        let suppressed = inst.is_some_and(|i| i.suppressed) || node.is_some_and(|n| n.suppressed);
        let error = node.and_then(|n| n.error.clone());
        // Show instance list properties: the release state pill.
        let state = inst.filter(|_| self.ui.list_properties).map(|i| self.instance_state(i, cx));
        // Suppressed by a Dynamic suppression expression: gray italic, struck
        // through, an ƒx glyph in place of the DOF glyph (live assembly/141).
        let dyn_suppressed = suppressed && inst.is_some_and(|i| i.unsuppressed_if.is_some());
        let row_tip: SharedString = if dyn_suppressed { format!("{name} (Suppressed with expression)").into() } else { name.clone().into() };
        // Onshape warns when more than one part is fixed.
        let fixed_tip: SharedString = if is_top && fixed && self.assembly(cx).is_some_and(|a| a.fixed_instances().len() > 1) { format!("{name} has warning: More than one part is fixed").into() } else { "Part is fixed".into() };
        let ext_key = inst.and_then(|i| i.source.external_key().map(|k| k.to_string()));
        let update = ext_key.as_ref().and_then(|k| self.ext_updates.get(k).cloned().flatten()).zip(ext_key.clone());
        // Version references (live 617, 620): ⋮ "Versioned part from current /
        // other document", a blue badge once the other document has a newer version.
        let own_path = self.doc.read(cx).session.path.clone();
        let versioned = ext_key.as_ref().and_then(|k| crate::derived::ExternalRef::parse(k)).map(|r| r.path == own_path);
        let newer_badge = update.is_some() && versioned == Some(false);
        let dof = if is_top { node.and_then(|n| n.dof) } else { None };
        let selected = is_top && self.selected.contains(&top);
        let has_children = node.is_some_and(|n| !n.children.is_empty());
        let expanded = self.expanded.contains(&path);
        let icon = match kind {
            // Standard content has its own icon in the Instances list (Onshape).
            NodeKind::Part if inst.is_some_and(|i| i.source.standard().is_some()) => "standard-content",
            // Sketch instances carry the sketch icon (live 123), composite
            // parts the composite icon (live 326).
            NodeKind::Part if inst.is_some_and(|i| matches!(i.source, InstanceSource::Sketch { .. })) => "sketch",
            NodeKind::Part if inst.is_some_and(|i| self.is_composite_source(&i.source, cx)) => "composite-part",
            NodeKind::Part => "part",
            NodeKind::PartStudio => "part-studio",
            NodeKind::Assembly if node.is_some_and(|n| n.rigid) => "sub-assembly-rigid",
            NodeKind::Assembly => "sub-assembly",
        };
        let warning = if is_top { self.instance_warning(top, cx) } else { None }.map(|w| SharedString::from(format!("{name} has warning: {w}")));
        let color = if suppressed || hidden {
            pal.text_faint
        } else if error.is_some() {
            pal.error
        } else {
            pal.text
        };
        let p2 = path.clone();
        let p3 = path.clone();
        let row = h_flex()
            .id(SharedString::from(format!("inst-{}", path.iter().map(|i| i.0.to_string()).collect::<Vec<_>>().join("-"))))
            .group(SharedString::from(format!("inst-row-{}", top.0)))
            .h(px(crate::ui::ROW_H))
            // Live assembly/10: part icon x 66, name x 86.
            .pl(px(7. + depth as f32 * 22.))
            .pr(px(3.))
            .gap(px(3.))
            .text_size(px(crate::ui::ROW_TEXT))
            .text_color(color)
            .when(selected, |d| d.bg(pal.accent_soft))
            .when(!selected, |d| d.hover(|d| d.bg(pal.row_hover)))
            .child(
                div()
                    .id(SharedString::from(format!("exp-{}", path.iter().map(|i| i.0.to_string()).collect::<Vec<_>>().join("-"))))
                    .w(px(12.))
                    .when(has_children, |d| {
                        d.child(crate::ui::glyph(if expanded { "ui-chevron-down" } else { "ui-chevron-right" }, 11., pal.text)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if !this.expanded.remove(&p2) {
                                this.expanded.insert(p2.clone());
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }))
                    }),
            )
            .child(crate::ui::tree_icon(icon, suppressed || hidden, cx))
            .child(match self.rename_field(behaviors::RenameRow::Instance(top)).filter(|_| is_top) {
                Some(field) => field,
                // The state glyphs follow the name (Onshape), not the row's right edge.
                None => div().min_w_0().truncate().mr(px(2.)).when(suppressed, |d| d.line_through()).when(dyn_suppressed, |d| d.italic()).child(name.clone()).into_any_element(),
            })
            .when_some(state, |d, s| d.child(Self::state_pill(s)))
            .when(fixed, |d| d.child(div().id(SharedString::from(format!("fixed-{}", top.0))).flex_none().child(crate::assets::icon(&icon_path("list-fixed"), 14., cx)).tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(fixed_tip.clone()).build(w, cx))))
            .when_some(inst.map(|i| i.lock.clone()).filter(|l| is_top && !l.is_flexible()), |d, lock| {
                // Lock icon of a locked subassembly (click to unlock).
                let tip = match &lock {
                    SubassemblyLock::Named(n) => format!("Locked at named position {n}"),
                    _ => "Locked at current position".to_string(),
                };
                d.child(
                    div()
                        .id(SharedString::from(format!("lock-{}", top.0)))
                        .cursor_pointer()
                        .child(svg().path(icon_path("lock")).size_3().text_color(pal.text_muted))
                        .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.set_sub_lock(top, SubassemblyLock::Flexible, cx);
                        })),
                )
            })
            .when(dyn_suppressed && is_top, |d| d.child(svg().path(icon_path("function")).size(px(14.)).flex_none().text_color(pal.text_faint)))
            .when_some(dof.filter(|d| *d > 0 && !fixed && !dyn_suppressed), |d, n| {
                d.child(div().id(SharedString::from(format!("dof-{}", top.0))).flex_none().child(crate::assets::icon(&icon_path("list-free"), 14., cx)).tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(format!("Has degrees of freedom ({n})")).build(w, cx)))
            })
            // In-context arrows (Assembly contexts of its Part Studio).
            .when(is_top, |d| d.children(self.context_icons(top, cx)))
            .when_some(warning.filter(|_| error.is_none()), |d, w| d.child(div().id(SharedString::from(format!("warn-{}", top.0))).child(crate::assets::icon(&icon_path("warning"), 12., cx)).tooltip(move |win, cx| gpui_kit::component::tooltip::Tooltip::new(w.clone()).build(win, cx))))
            .when_some(error.clone(), |d, e| d.child(div().id(SharedString::from(format!("err-{}", top.0))).size(px(6.)).rounded_full().bg(pal.error).tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(e.clone()).build(w, cx))))
            .when_some(versioned.filter(|_| is_top), |d, own| {
                let tip = if newer_badge {
                    "Versioned part from other document with newer version"
                } else if own {
                    "Versioned part from current document"
                } else {
                    "Versioned part from other document"
                };
                d.child(
                    div()
                        .id(SharedString::from(format!("ver-{}", top.0)))
                        .flex_none()
                        .child(crate::assets::icon(&icon_path(if newer_badge { "inserted-version-update" } else { "list-versioned" }), 14., cx))
                        .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip).build(w, cx)),
                )
            })
            .child(div().flex_1())
            .when(is_top, |d| {
                // Eye icon on hover (kept visible on hidden instances).
                d.child(
                    div().when(!hidden, |d| d.invisible().group_hover(SharedString::from(format!("inst-row-{}", top.0)), |d| d.visible())).child(
                        Button::new(SharedString::from(format!("eye-{}", top.0)))
                            .icon(Icon::empty().path(icon_path(if hidden { "eye-off" } else { "eye" })))
                            .ghost()
                            .xsmall()
                            .tooltip(if hidden { "Show" } else { "Hide" })
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.stop_propagation();
                                this.set_hidden(vec![top], !hidden, cx)
                            })),
                    ),
                )
            })
            .when(!crate::os_menu::is_open(cx), |d| d.tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(row_tip.clone()).build(w, cx)))
            .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                // Hovering a row highlights the instance in the graphics area.
                let new = if *h { Some(top) } else { None };
                if this.ui.list_hover != new && (*h || this.ui.list_hover == Some(top)) {
                    this.ui.list_hover = new;
                    this.refresh_scene(false, cx);
                }
            }))
            .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                if this.ui.group.is_some() {
                    this.toggle_group_member(p3[0], cx);
                    return;
                }
                this.selected_mate = None;
                let top = p3[0];
                if ev.modifiers().shift || ev.modifiers().secondary() {
                    if let Some(i) = this.selected.iter().position(|x| *x == top) {
                        this.selected.remove(i);
                    } else {
                        this.selected.push(top);
                    }
                } else {
                    this.selected = vec![top];
                }
                this.refresh_scene(false, cx);
                cx.notify();
            }));
        if !is_top {
            return row.into_any_element();
        }
        // Drag and drop: reorder (drives BOM order), joining the target's folder.
        let accent = pal.accent;
        let row = row
            .on_drag(behaviors::InstanceDrag { id: top, label: name.clone().into() }, |d, _, _, cx| cx.new(|_| d.clone()))
            .drag_over::<behaviors::InstanceDrag>(move |st, _, _, _| st.border_t_2().border_color(accent))
            .on_drop(cx.listener(move |this, d: &behaviors::InstanceDrag, _, cx| {
                if d.id == top {
                    return;
                }
                let id = d.id;
                // Dropped onto a subassembly: into it (Onshape); else reorder.
                if this.assembly(cx).and_then(|a| a.instance(top)).is_some_and(|i| matches!(i.source, InstanceSource::Assembly { .. })) {
                    let ids = if this.selected.contains(&id) { this.selected.clone() } else { vec![id] };
                    this.move_into_subassembly(ids, top, cx);
                } else {
                    this.edit("Reorder instances", cx, |a| a.move_instance(id, Some(top)));
                }
            }));
        let view = cx.entity();
        row.os_context_menu(move |menu, window, cx| {
            let entries = view.read(cx).instance_menu(top, false, cx);
            fill_menu(menu, entries, view.clone(), window, cx)
        })
        .into_any_element()
    }

    fn push_children(&self, node: &InstanceNode, depth: usize, rows: &mut Vec<AnyElement>, cx: &mut Context<Self>) {
        if !self.expanded.contains(&node.path) {
            return;
        }
        for c in &node.children {
            rows.push(self.instance_row(Some(c), None, c.path.clone(), depth, cx));
            self.push_children(c, depth + 1, rows, cx);
        }
    }

    fn feature_row(&self, f: FeatureRow, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let FeatureRow { id, icon, name, ok, inactive, message, has_limits, hidden, tooltip, implicit } = f;
        let selected = self.selected_mate == Some(id);
        let accent = pal.accent;
        let color = if inactive {
            pal.text_faint
        } else if !ok {
            pal.error
        } else {
            pal.text
        };
        let expanded = self.ui.expanded_mates.contains(&id);
        let can_expand = !implicit.is_empty();
        let tip = match (&message, tooltip) {
            (Some(m), Some(t)) => Some(format!("{name} has error: {m}\n{t}")),
            (Some(m), None) => Some(format!("{name} has error: {m}")),
            (None, t) => t,
        };
        let row = h_flex()
            .id(SharedString::from(format!("mate-{}", id.0)))
            .group(SharedString::from(format!("mate-row-{}", id.0)))
            .h(px(crate::ui::ROW_H))
            .pl(px(5.))
            .pr(px(3.))
            .gap(px(3.))
            .text_size(px(crate::ui::ROW_TEXT))
            .text_color(color)
            .when(selected, |d| d.bg(pal.accent_soft))
            .when(!selected, |d| d.hover(|d| d.bg(pal.row_hover)))
            .child(div().id(SharedString::from(format!("mexp-{}", id.0))).w(px(12.)).when(can_expand, |d| {
                d.child(crate::ui::glyph(if expanded { "ui-chevron-down" } else { "ui-chevron-right" }, 11., pal.text)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if !this.ui.expanded_mates.remove(&id) {
                        this.ui.expanded_mates.insert(id);
                    }
                    cx.stop_propagation();
                    cx.notify();
                }))
            }))
            .child(crate::assets::icon(&icon, 14., cx))
            .child(match self.rename_field(behaviors::RenameRow::Feature(id)) {
                Some(field) => field,
                None => div().flex_1().truncate().when(inactive, |d| d.line_through()).child(name.clone()).into_any_element(),
            })
            .when(has_limits, |d| {
                let n = name.clone();
                d.child(div().id(SharedString::from(format!("mlim-{}", id.0))).child(crate::assets::icon(&icon_path("mate-limits"), 12., cx)).tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(format!("{n} has limits")).build(w, cx)))
            })
            .when(!ok && !inactive, |d| d.child(div().size(px(6.)).rounded_full().bg(pal.error)))
            .when_some(hidden, |d, hidden| {
                // Eye icon (shown on hover, or always while hidden) toggles the mate indicator.
                d.child(
                    div()
                        .invisible()
                        .group_hover(SharedString::from(format!("mate-row-{}", id.0)), |d| d.visible())
                        .child(Button::new(SharedString::from(format!("meye-{}", id.0))).icon(Icon::empty().path(icon_path(if hidden { "eye-off" } else { "eye" }))).ghost().xsmall().tooltip(if hidden { "Show" } else { "Hide" }).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.set_mates_hidden(vec![id], !hidden, cx);
                        }))),
                )
            })
            // No row tooltip while a context menu is open (it would cover the menu).
            .when_some(tip.filter(|_| !crate::os_menu::is_open(cx)), |d, m| d.tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(m.clone()).build(w, cx)))
            // Drag a row onto another of the same kind to reorder (Onshape).
            .on_drag(behaviors::MateDrag { id, label: name.clone().into() }, |d, _, _, cx| cx.new(|_| d.clone()))
            .drag_over::<behaviors::MateDrag>(move |st, _, _, _| st.border_t_2().border_color(accent))
            .on_drop(cx.listener(move |this, d: &behaviors::MateDrag, _, cx| {
                if d.id != id {
                    this.edit("Reorder", cx, |a| {
                        a.move_feature_before(d.id, id);
                    });
                }
            }))
            .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                let new = if *h { Some(id) } else { None };
                if this.list_hover_mate != new && (*h || this.list_hover_mate == Some(id)) {
                    this.list_hover_mate = new;
                    this.refresh_scene(false, cx);
                }
            }))
            .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                if ev.click_count() >= 2 {
                    this.edit_mate(id, window, cx);
                    return;
                }
                // Configure assembly features: a click opens the mate's
                // parameters for picking (live 194).
                if this.ui.cfg_mode.is_some() && this.configure_pick_mate(id, window, cx) {
                    return;
                }
                if this.pick_mode(cx).is_some() {
                    return;
                }
                if this.relation.is_some() {
                    // Relation dialog: clicking a mate fills the next mate field.
                    this.pick_relation_mate(id, cx);
                    return;
                }
                this.selected_mate = if this.selected_mate == Some(id) { None } else { Some(id) };
                this.selected.clear();
                this.refresh_scene(false, cx);
                cx.notify();
            }));
        let view = cx.entity();
        let main = row
            .os_context_menu(move |menu, window, cx| {
                let entries = view.read(cx).mate_menu(id, cx);
                fill_menu(menu, entries, view.clone(), window, cx)
            })
            .into_any_element();
        if !(expanded && can_expand) {
            return main;
        }
        // Implicit mate connectors are listed under the mate that uses them.
        let mut col = v_flex().gap_px().child(main);
        for (k, (label, _)) in implicit.into_iter().enumerate() {
            col = col.child(
                h_flex()
                    .id(SharedString::from(format!("mimp-{}-{k}", id.0)))
                    .h(px(22.))
                    .pl(px(32.))
                    .gap_1p5()
                    .rounded_sm()
                    .text_xs()
                    .text_color(pal.text_muted)
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(crate::assets::icon(&icon_path("mate-connector"), 12., cx))
                    .child(div().flex_1().truncate().child(label))
                    .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                        let new = if *h { Some(id) } else { None };
                        if this.list_hover_mate != new {
                            this.list_hover_mate = new;
                            this.refresh_scene(false, cx);
                        }
                    })),
            );
        }
        col.into_any_element()
    }

    /// Top-level assembly row (click selects every instance; its menu has Unfix all, Expand all...).
    fn assembly_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let name = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default();
        let all_selected = self.assembly(cx).is_some_and(|a| !a.instances.is_empty() && a.instances.iter().all(|i| self.selected.contains(&i.id)));
        let grounded = self.assembly(cx).is_some_and(|a| !a.fixed_instances().is_empty());
        let view = cx.entity();
        h_flex()
            .id("asm-top-row")
            .h(px(crate::ui::ROW_H))
            // Live assembly/10: no caret column; icon x 47, name x 66.
            .pl(px(5.))
            .pr_1()
            .gap(px(4.))
            .text_size(px(crate::ui::ROW_TEXT))
            .text_color(pal.text)
            .when(all_selected, |d| d.bg(pal.accent_soft))
            .hover(|d| d.bg(pal.panel_hover))
            .child(div().w(px(16.)).flex_none().flex().justify_center().child(crate::assets::icon(&icon_path("assembly"), 16., cx)))
            .child(match self.rename_field(behaviors::RenameRow::Instance(InstanceId::ORIGIN)) {
                Some(field) => field,
                None => div().min_w_0().truncate().child(name).into_any_element(),
            })
            // Grounded: something in the assembly is fixed (Onshape's arrow onto ground).
            .when(grounded, |d| d.child(div().id("asm-grounded").flex_none().child(crate::assets::icon(&icon_path("list-grounded"), 14., cx)).tooltip(|w, cx| gpui_kit::component::tooltip::Tooltip::new("Assembly is grounded").build(w, cx))))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.selected = this.assembly(cx).map(|a| a.instances.iter().filter(|i| a.generated_by(i.id).is_none()).map(|i| i.id).collect()).unwrap_or_default();
                this.selected_mate = None;
                this.refresh_scene(false, cx);
                cx.notify();
            }))
            .os_context_menu(move |menu, window, cx| {
                let entries = view.read(cx).assembly_menu(cx);
                fill_menu(menu, entries, view.clone(), window, cx)
            })
            .into_any_element()
    }

    /// Read-only description of a mate's offsets and limits (row tooltip).
    fn mate_tooltip(&self, m: &Mate) -> String {
        let mut lines = vec![format!("{} mate", m.mate_type.name())];
        let mut offs: Vec<String> = Vec::new();
        for (label, e) in [("X", &m.offset_x), ("Y", &m.offset_y), ("Z", &m.offset_z)] {
            if let Some(e) = e.as_ref().filter(|e| !e.text.trim().is_empty()) {
                offs.push(format!("{label} {}", e.text));
            }
        }
        if let Some(e) = m.rotation.as_ref().filter(|e| !e.text.trim().is_empty()) {
            offs.push(format!("rotation {} about {}", e.text, ["X", "Y", "Z"][(m.rotation_axis as usize).min(2)]));
        }
        lines.push(if offs.is_empty() { "Offset: off".into() } else { format!("Offset: {}", offs.join(", ")) });
        if !m.limits.is_empty() {
            let mut ls = Vec::new();
            for (k, l) in &m.limits {
                let label = match k.as_str() {
                    "rz" => "Z angle",
                    "x" => "X",
                    "y" => "Y",
                    "cone" => "Cone angle",
                    _ => "Z",
                };
                let t = |e: &Option<ExprValue>| e.as_ref().map(|e| e.text.clone()).unwrap_or_else(|| "–".into());
                ls.push(format!("{label} {} to {}", t(&l.min), t(&l.max)));
            }
            lines.push(format!("Limits: {}", ls.join(", ")));
        }
        lines.join("\n")
    }

    fn render_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(asm) = self.assembly(cx).cloned() else { return div().into_any_element() };
        let result = self.result.clone();
        let filtering = !self.ui.filter.is_empty();
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut selected_row: Option<usize> = None;
        if !filtering {
            rows.push(self.assembly_row(cx));
            rows.push(self.origin_row(cx));
        }
        let mut shown_folders: HashSet<MateId> = HashSet::new();
        for inst in &asm.instances {
            if asm.generated_by(inst.id).is_some() {
                continue;
            }
            // Folders show at the place of their first member (not while filtering).
            if !filtering && let Some(folder) = asm.folder_of_instance(inst.id) {
                if shown_folders.insert(folder.id) {
                    rows.push(self.folder_row(folder, cx));
                    if self.expanded.contains(&vec![InstanceId(folder.id.0)]) {
                        for m in &folder.instances {
                            let Some(mi) = asm.instance(*m) else { continue };
                            let node = result.node(&[mi.id]);
                            if selected_row.is_none() && self.selected.first() == Some(&mi.id) {
                                selected_row = Some(rows.len());
                            }
                            rows.push(self.instance_row(node, Some(mi), vec![mi.id], 1, cx));
                            if let Some(n) = node {
                                self.push_children(n, 2, &mut rows, cx);
                            }
                        }
                    }
                }
                continue;
            }
            let node = result.node(&[inst.id]);
            if !self.instance_passes(inst, node, cx) {
                continue;
            }
            if selected_row.is_none() && self.selected.first() == Some(&inst.id) {
                selected_row = Some(rows.len());
            }
            rows.push(self.instance_row(node, Some(inst), vec![inst.id], 0, cx));
            if let Some(n) = node
                && !filtering
            {
                self.push_children(n, 1, &mut rows, cx);
            }
        }
        if !filtering {
            // Empty instance folders.
            for folder in asm.folders.iter().filter(|f| f.instances.is_empty() && f.features.is_empty()) {
                rows.push(self.folder_row(folder, cx));
            }
        } else {
            // Filtered rows are flat; `:folder` lists the folders.
            for folder in asm.folders.iter().filter(|f| self.folder_passes(f)) {
                rows.push(self.folder_row(folder, cx));
            }
        }
        for p in &asm.patterns {
            if !self.feature_passes(noshape_model::assembly::ops::RowKind::Pattern, &p.name, false, p.suppressed, false, false) {
                continue;
            }
            let err = result.pattern_errors.iter().find(|(id, _)| *id == p.id).map(|(_, m)| m.clone());
            rows.push(self.pattern_row(p, err, cx));
            if self.expanded.contains(&vec![InstanceId(p.id.0)]) {
                for g in &p.generated {
                    if let Some(inst) = asm.instance(g.instance) {
                        if selected_row.is_none() && self.selected.first() == Some(&inst.id) {
                            selected_row = Some(rows.len());
                        }
                        rows.push(self.instance_row(result.node(&[inst.id]), Some(inst), vec![inst.id], 1, cx));
                    }
                }
            }
        }
        if asm.instances.is_empty() {
            rows.push(div().px_3().py_2().text_xs().text_color(pal.text_faint).child("Insert parts and assemblies (I) to start.").into_any_element());
        }
        // Selecting in the graphics area scrolls the list to the row.
        if std::mem::take(&mut self.ui.scroll_to_selection)
            && let Some(ix) = selected_row
        {
            self.ui.list_scroll.scroll_to_item(ix);
        }
        let labeler = self.labeler(cx);
        let mut mate_rows: Vec<AnyElement> = Vec::new();
        let mut mate_ids: Vec<MateId> = Vec::new();
        for c in &asm.connectors {
            let st = result.connectors.iter().find(|s| s.id == c.id);
            let msg = st.and_then(|s| s.message.clone());
            if !self.feature_passes(noshape_model::assembly::ops::RowKind::Connector, &c.name, msg.is_some(), c.suppressed, c.hidden, false) {
                continue;
            }
            mate_rows.push(self.connector_row(c, msg, cx));
            mate_ids.push(c.id);
        }
        for m in &asm.mates {
            let st = result.mates.iter().find(|s| s.id == m.id);
            let ok = st.is_none_or(|s| s.ok);
            let inactive = m.suppressed || st.is_some_and(|s| s.inactive);
            let hidden = m.hidden || !self.show_mates;
            if !self.feature_passes(noshape_model::assembly::ops::RowKind::Mate, &m.name, !ok, inactive, hidden, !m.limits.is_empty()) {
                continue;
            }
            let implicit: Vec<(String, MateConnectorRef)> = m.connectors().filter(|c| c.connector.is_none() && !c.is_origin()).map(|c| (labeler(c), c.clone())).collect();
            mate_rows.push(self.feature_row(
                FeatureRow { id: m.id, icon: mate_icon(m.mate_type), name: m.name.clone(), ok, inactive, message: st.and_then(|s| s.message.clone()), has_limits: !m.limits.is_empty(), hidden: Some(hidden), tooltip: Some(self.mate_tooltip(m)), implicit },
                cx,
            ));
            mate_ids.push(m.id);
        }
        for r in &asm.relations {
            let st = result.relations.iter().find(|s| s.id == r.id);
            let ok = st.is_none_or(|s| s.ok);
            if !self.feature_passes(noshape_model::assembly::ops::RowKind::Relation, &r.name, !ok, r.suppressed, false, false) {
                continue;
            }
            let icon = icon_path(match r.relation_type {
                RelationType::Gear => "gear-relation",
                RelationType::RackAndPinion => "rack-pinion",
                RelationType::Screw => "screw-relation",
                RelationType::Linear => "linear-relation",
            });
            mate_rows.push(self.feature_row(FeatureRow { id: r.id, icon, name: r.name.clone(), ok, inactive: r.suppressed, message: st.and_then(|s| s.message.clone()), has_limits: false, hidden: None, tooltip: None, implicit: vec![] }, cx));
            mate_ids.push(r.id);
        }
        for g in &asm.groups {
            if !self.feature_passes(noshape_model::assembly::ops::RowKind::Group, &g.name, false, g.suppressed, false, false) {
                continue;
            }
            mate_rows.push(self.feature_row(FeatureRow { id: g.id, icon: icon_path("group"), name: g.name.clone(), ok: true, inactive: g.suppressed, message: None, has_limits: false, hidden: None, tooltip: None, implicit: vec![] }, cx));
            mate_ids.push(g.id);
        }
        let mate_rows = if filtering { mate_rows } else { self.fold_mate_rows(&asm, mate_ids, mate_rows, cx) };
        // Onshape's instance list: the filter row, the "Instances (N)" header
        // with its new-folder button, then one tree that ends with the
        // collapsible "Mate features (N)" folder holding the mates.
        let mates_open = !self.expanded.contains(&vec![MATE_FEATURES_COLLAPSED]);
        let mate_count = asm.mates.len() + asm.relations.len() + asm.groups.len() + asm.connectors.len();
        if !filtering {
            rows.push(
                crate::ui::tree_row("mates-folder", false)
                    .cursor_pointer()
                    .child(crate::ui::caret(mates_open))
                    .child(format!("Mate features ({mate_count})"))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        let key = vec![MATE_FEATURES_COLLAPSED];
                        if !this.expanded.remove(&key) {
                            this.expanded.insert(key);
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        if mates_open || filtering {
            rows.extend(mate_rows.into_iter().map(|r| div().pl(px(if filtering { 0. } else { 17. })).child(r).into_any_element()));
        }
        // A configured assembly: the Configurations box above the list (live 191).
        let config_box = self.render_asm_config_box(cx);
        v_flex()
            .size_full()
            .bg(pal.panel)
            .border_r_1()
            .border_color(pal.border)
            .children(config_box)
            .child(self.render_list_filter(cx))
            .child(
                // Live assembly/10: like the Part Studio's "Features (n)" row.
                h_flex()
                    .h(px(30.))
                    .flex_none()
                    .pl(px(4.))
                    .pr(px(6.))
                    .justify_between()
                    .child(div().text_size(px(crate::ui::ROW_TEXT)).font_weight(FontWeight::SEMIBOLD).text_color(pal.text).child(format!("Instances ({})", asm.instances.len())))
                    .child(
                        h_flex()
                            .gap_1()
                            // New folder (around the selection, or empty).
                            .child(Button::new("asm-new-folder").child(crate::assets::icon("new-folder", 20., cx)).ghost().w(px(24.)).h(px(24.)).p_0().tooltip("New folder").on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                let ids = this.selected.clone();
                                this.folder_from_instances(ids, window, cx);
                            }))),
                    ),
            )
            .child(v_flex().id("instance-list").flex_1().min_h_0().overflow_y_scroll().track_scroll(&self.ui.list_scroll).children(rows))
            .into_any_element()
    }

    // ------------------------------------------------------------ render: panels

    fn panel_frame(title: &str, body: impl IntoElement, close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static, id: &'static str) -> Div {
        let pal = p();
        // Onshape dialog frame: white at 95%, no border or radius, 28 px header (§3.9).
        v_flex()
            .w(px(280.))
            .max_h(px(560.))
            .bg(pal.dialog_bg)
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
            .text_color(pal.text)
            .child(
                h_flex()
                    .h(px(28.))
                    .flex_none()
                    .pl(px(6.))
                    .bg(pal.panel)
                    .border_b_1()
                    .border_color(pal.border)
                    .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).truncate().child(title.to_string()))
                    .child(Button::new(id).ghost().child(svg().path(icon_path("dialog-cancel")).size(px(18.)).text_color(pal.cancel)).w(px(28.)).h(px(28.)).p_0().rounded(px(0.)).on_click(close)),
            )
            .child(body)
    }

    /// Onshape's floating mate toolbar after both connectors are picked:
    /// a drag handle (◻◻◻), the mate type select, flip, reorient, Solve,
    /// commit-and-new (green ✓) and cancel (✕).
    fn render_mate_hud(&self, at: DVec2, dialog: Entity<MateDialog>, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let t = dialog.read(cx).mate_type();
        let border = gpui_kit::rgb(0xcccccc);
        let icon_btn = |id: &'static str, icon: &'static str, tip: &'static str, cx: &App| {
            div()
                .id(id)
                .size(px(28.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(4.))
                .border_1()
                .border_color(border)
                .bg(pal.dialog_bg)
                .cursor_pointer()
                .hover(|d| d.bg(pal.panel_hover))
                .tooltip(crate::ui::tooltip(tip))
                .child(crate::assets::icon(&icon_path(icon), 22., cx))
        };
        let d1 = dialog.clone();
        let types = [MateType::Slider, MateType::Cylindrical, MateType::Revolute, MateType::PinSlot, MateType::Planar, MateType::Ball, MateType::Fastened, MateType::Parallel];
        let type_select = Button::new("mate-hud-type")
            .custom(
                gpui_kit::component::button::ButtonCustomVariant::new(cx)
                    .color(gpui_kit::transparent_white())
                    .foreground(pal.text.into())
                    .hover(gpui_kit::transparent_white())
                    .active(gpui_kit::transparent_white())
                    .shadow(false),
            )
            .w(px(120.))
            .h(px(26.))
            .px(px(6.))
            .rounded(px(0.))
            .border_b_1()
            .border_color(gpui_kit::rgb(0xd7d7d7))
            .tooltip("Mate type")
            .child(h_flex().w_full().child(div().flex_1().text_size(px(14.)).text_color(pal.text).child(t.name())).child(crate::ui::glyph("ui-caret-down", 8., pal.text)))
            .os_dropdown_menu("mate-hud-type-dd", true, move |mut menu, _, _| {
                for mt in types {
                    let d = d1.clone();
                    menu = menu.item(crate::os_menu::OsMenuItem::new(mt.name()).icon_name(crate::assembly_mate_dialog::mate_icon_name(mt)).on_click(move |_, _, cx| d.update(cx, |d, cx| d.set_type(mt, cx))));
                }
                menu
            });
        let grab = self.ui.hud_grab.clone();
        let (d2, d3, d4) = (dialog.clone(), dialog.clone(), dialog);
        div()
            .id("mate-hud")
            .absolute()
            .left(px(at.x as f32 + 14.))
            .top(px(at.y as f32 + 14.))
            .w(px(310.))
            .bg(pal.dialog_bg)
            .border_1()
            .border_color(border)
            .rounded(px(3.))
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_drag_move(cx.listener(|this, e: &DragMoveEvent<HudDrag>, _, cx| {
                let g = *e.drag(cx).0.lock();
                let delta = e.event.position - e.bounds.origin - g;
                if let Some(s) = &mut this.mate
                    && let Some(h) = &mut s.hud
                {
                    h.x += f32::from(delta.x) as f64;
                    h.y += f32::from(delta.y) as f64;
                    cx.notify();
                }
            }))
            .child(
                // Drag handle header.
                div()
                    .id("mate-hud-drag")
                    .h(px(12.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_b_1()
                    .border_color(border)
                    .cursor_grab()
                    .text_size(px(9.33))
                    .line_height(px(11.))
                    .text_color(pal.text)
                    .child("◻◻◻")
                    .on_drag(HudDrag(grab), |d, offset, _, cx| {
                        *d.0.lock() = offset;
                        cx.new(|_| HudDragGhost)
                    }),
            )
            .child(
                h_flex()
                    .h(px(35.))
                    .px(px(5.))
                    .gap(px(3.))
                    .child(type_select)
                    .child(div().flex_1())
                    .child(icon_btn("mate-hud-flip", "flip", "Flip primary axis", cx).on_click(move |_, _, cx| d2.update(cx, |d, cx| d.flip(cx))))
                    .child(icon_btn("mate-hud-reorient", "flip-circular", "Reorient secondary axis", cx).on_click(move |_, _, cx| d3.update(cx, |d, cx| d.reorient(cx))))
                    .child(
                        div()
                            .id("mate-hud-solve")
                            .h(px(28.))
                            .px(px(7.))
                            .flex()
                            .items_center()
                            .rounded(px(4.))
                            .border_1()
                            .border_color(gpui_kit::rgb(0xaaaaaa))
                            .bg(gpui_kit::white())
                            .cursor_pointer()
                            .hover(|d| d.bg(pal.panel_hover))
                            .text_size(px(14.))
                            .tooltip(crate::ui::tooltip("Solve full assembly"))
                            .child("Solve")
                            .on_click(move |_, _, cx| d4.update(cx, |_, cx| cx.emit(MateDialogEvent::Solve))),
                    )
                    .child(
                        div()
                            .id("mate-hud-ok")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .bg(gpui_kit::rgb(0x009400))
                            .cursor_pointer()
                            .tooltip(crate::ui::tooltip("Commit current mate and start a new mate"))
                            .child(crate::ui::glyph("check", 18., gpui_kit::white()))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.accept_mate_and_new(window, cx))),
                    )
                    .child(
                        div()
                            .id("mate-hud-cancel")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(4.))
                            .border_1()
                            .border_color(gpui_kit::rgb(0xaaaaaa))
                            .bg(gpui_kit::white())
                            .cursor_pointer()
                            .tooltip(crate::ui::tooltip("Cancel"))
                            .child(crate::assets::icon("dialog-cancel", 18., cx))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.close_mate(window, cx))),
                    ),
            )
            .into_any_element()
    }

    fn render_insert_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let query = self.ui.insert_search.as_ref().map(|s| s.read(cx).value().trim().to_lowercase()).unwrap_or_default();
        let hit = |name: &str| query.is_empty() || name.to_lowercase().contains(&query);
        let (source, tab) = (self.ui.insert_source, self.ui.insert_tab);
        // Onshape dialog tabs: 12 px, the active one blue (500) with a 2 px
        // blue bottom line, a light gray line under the group.
        let dtab = |id: &'static str, label: &'static str, on: bool, w: f32, h: f32, cx: &mut Context<Self>, f: fn(&mut AsmUi)| {
            div()
                .id(id)
                .w(px(w))
                .h(px(h))
                .px(px(14.))
                .flex()
                .items_center()
                .justify_center()
                .text_center()
                .text_size(px(12.))
                .line_height(px(21.6))
                .cursor_pointer()
                .text_color(if on { pal.accent } else { pal.text })
                .when(on, |d| d.font_weight(FontWeight::MEDIUM).border_b_2().border_color(pal.accent))
                // 100 px tabs wrap their label on two lines (Onshape).
                .child(div().w_full().whitespace_normal().child(label))
                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                    f(&mut this.ui);
                    let ph = if this.ui.insert_tab == 0 { "Search Part Studios" } else { "Search Assemblies" };
                    if let Some(st) = &this.ui.insert_search {
                        st.update(cx, |s, cx| s.set_placeholder(ph, window, cx));
                    }
                    cx.notify();
                }))
        };
        let group_line = gpui_kit::rgb(0xeeeeee);
        let mut head = v_flex().flex_none().child(
            h_flex()
                .h(px(54.))
                .border_b_1()
                .border_color(group_line)
                .child(dtab("ins-src-cur", "Current document", source == 0, 100., 54., cx, |u| u.insert_source = 0))
                .child(dtab("ins-src-other", "Other documents", source == 1, 100., 54., cx, |u| u.insert_source = 1))
                .child(dtab("ins-src-std", "Standard content", source == 2, 100., 54., cx, |u| u.insert_source = 2)),
        );
        if source == 0 {
            // Document name, workspace and the version buttons.
            let doc_name = self.doc.read(cx).name();
            let square = |id: &'static str, icon: &'static str, tip: &'static str, cx: &App| {
                div()
                    .id(id)
                    .size(px(30.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(3.))
                    .border_1()
                    .border_color(gpui_kit::rgb(0xaaaaaa))
                    .cursor_pointer()
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(crate::assets::icon(&icon_path(icon), 20., cx))
                    .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip).build(w, cx))
            };
            head = head
                .child(
                    h_flex()
                        .h(px(40.))
                        .px(px(6.))
                        .gap(px(6.))
                        .border_b_1()
                        .border_color(group_line)
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .child(div().truncate().text_size(px(14.67)).font_weight(FontWeight::MEDIUM).line_height(px(19.)).child(doc_name))
                                .child(h_flex().gap_1().opacity(0.5).text_size(px(12.)).line_height(px(16.)).child(crate::ui::glyph("branch", 10., pal.text)).child("Main")),
                        )
                        .child(square("ins-create-version", "version", "Create version…", cx).on_click({
                            let doc = self.doc.clone();
                            move |_, w, cx| crate::history_panel::open_create_version_in(doc.clone(), w, cx)
                        }))
                        .child(square("ins-version-graph", "version-graph", "Version graph", cx).on_click(|_, w, cx| w.dispatch_action(Box::new(crate::actions::ToggleVersionGraph), cx))),
                )
                .child(
                    h_flex()
                        .h(px(32.))
                        .border_b_1()
                        .border_color(group_line)
                        .child(dtab("ins-tab-ps", "Part Studios", tab == 0, 150., 32., cx, |u| u.insert_tab = 0))
                        .child(dtab("ins-tab-asm", "Assemblies", tab == 1, 150., 32., cx, |u| u.insert_tab = 1)),
                );
            if let Some(st) = &self.ui.insert_search {
                head = head.child(div().pt(px(6.)).px(px(6.)).pb(px(6.)).child(Input::new(st).small().cleanable(true)));
            }
            if tab == 0 {
                // Type filters (Parts, and Surfaces when there are any) and
                // "Insert entire Part Studio as rigid" at the right.
                let has_surfaces = self.catalog.studios.iter().any(|s| s.parts.iter().any(|p| p.3));
                let has_sketches = self.catalog.studios.iter().any(|s| !s.sketches.is_empty());
                let fk = self.ui.insert_sketches;
                let fc = self.ui.insert_composites;
                let (fp, fs) = self.ui.insert_filter;
                let toggle = |id: &'static str, icon: &'static str, tip: &'static str, on: bool, cx: &mut Context<Self>| {
                    div()
                        .id(id)
                        .size(px(30.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(3.))
                        .cursor_pointer()
                        .when(on, |d| d.bg(gpui_kit::rgb(0xeaeaea)).shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.25), offset: point(px(0.), px(1.)), blur_radius: px(1.), spread_radius: px(0.), inset: true }]))
                        .when(!on, |d| d.hover(|d| d.bg(pal.panel_hover)))
                        .child(crate::assets::icon(&icon_path(icon), 20., cx))
                        .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip).build(w, cx))
                };
                let rigid = self.insert_rigid;
                head = head.child(
                    h_flex()
                        .h(px(42.))
                        .pt(px(6.))
                        .px(px(6.))
                        .pb(px(6.))
                        .gap(px(6.))
                        .border_b_1()
                        .border_color(pal.border)
                        .child(toggle("ins-f-parts", "part", "Parts", fp, cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.ui.insert_filter.0 = !this.ui.insert_filter.0;
                            cx.notify();
                        })))
                        // Live 320: Parts, Composite parts (off), Sketches (off).
                        .child(toggle("ins-f-composite", "composite-part", "Composite parts", fc, cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.ui.insert_composites = !this.ui.insert_composites;
                            cx.notify();
                        })))
                        .when(has_sketches, |d| {
                            d.child(toggle("ins-f-sketch", "sketch", "Sketches", fk, cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.ui.insert_sketches = !this.ui.insert_sketches;
                                cx.notify();
                            })))
                        })
                        .when(has_surfaces, |d| {
                            d.child(toggle("ins-f-surf", "surface-body", "Surfaces", fs, cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.ui.insert_filter.1 = !this.ui.insert_filter.1;
                                cx.notify();
                            })))
                        })
                        .child(div().flex_1())
                        .child(toggle("insert-rigid", "part-studio", "Insert entire Part Studio as rigid", rigid, cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.insert_rigid = !this.insert_rigid;
                            cx.notify();
                        }))),
                );
            }
        }
        // Tree rows: expander, 70x40 thumbnail cell, name (13.33 px light).
        self.request_insert_thumbs(cx);
        let item_row = |id: SharedString, indent: f32, expander: Option<AnyElement>, icon: &'static str, thumb: Option<Arc<RenderImage>>, name: String, cx: &App| {
            h_flex()
                .id(id)
                .flex_none()
                .h(px(40.))
                .pl(px(4. + indent))
                .cursor_pointer()
                .hover(|d| d.bg(pal.row_hover))
                .child(div().w(px(14.)).h_full().flex_none().flex().items_center().children(expander))
                .child(div().w(px(70.)).h(px(40.)).flex_none().flex().items_center().justify_center().child(match thumb {
                    Some(img) => gpui_kit::img(img).w(px(70.)).h(px(40.)).object_fit(ObjectFit::Contain).into_any_element(),
                    None => crate::assets::icon(&icon_path(icon), 28., cx).into_any_element(),
                }))
                .child(div().flex_1().min_w_0().pl(px(4.)).truncate().text_size(px(13.33)).font_weight(FontWeight::LIGHT).child(name))
        };
        let mut body = v_flex().id("insert-body").flex_1().min_h_0().overflow_y_scroll();
        if source == 2 {
            body = body.child(div().p_2().child(self.render_standard_content(false, cx)));
        } else if source == 1 {
            body = body.child(self.render_insert_other(cx));
        } else if tab == 0 {
            let mut any = false;
            for s in self.catalog.studios.clone() {
                let studio_hit = hit(&s.name);
                // Type filters: none on shows everything, else the union of the
                // selected types (Parts, Sketches, Surfaces).
                let (only_parts, only_surfaces) = self.ui.insert_filter;
                let only_sketches = self.ui.insert_sketches;
                let composites_on = self.ui.insert_composites;
                let any_filter = only_parts || only_surfaces || only_sketches || composites_on;
                let is_composite = |p: &CatalogPart| s.composites.contains(&p.0);
                let kind_ok = |p: &CatalogPart| if is_composite(p) { composites_on } else { !any_filter || (only_parts && !p.3) || (only_surfaces && p.3) };
                // Composite parts first, then the parts (live 320).
                let mut parts: Vec<CatalogPart> = s.parts.iter().filter(|p| (studio_hit || hit(&p.1)) && kind_ok(p)).cloned().collect();
                parts.sort_by_key(|p| !is_composite(p));
                let sketches: Vec<(FeatureId, String)> = if !any_filter || only_sketches { s.sketches.iter().filter(|k| studio_hit || hit(&k.1)).cloned().collect() } else { Vec::new() };
                if !studio_hit && parts.is_empty() && sketches.is_empty() {
                    continue;
                }
                any = true;
                let s2 = s.clone();
                let open = !self.ui.insert_collapsed.contains(&s.tab) && !self.insert_rigid;
                let st = s.tab;
                // The chevron expands / collapses; the rest of the row inserts the studio.
                let expander = (!self.insert_rigid).then(|| {
                    div()
                        .id(SharedString::from(format!("ins-ps-exp-{}", s.tab)))
                        .size(px(14.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(crate::ui::glyph(if open { "ui-chevron-down" } else { "ui-chevron-right" }, 10., pal.text))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if !this.ui.insert_collapsed.remove(&st) {
                                this.ui.insert_collapsed.insert(st);
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }))
                        .into_any_element()
                });
                let cfg_button = self.insert_config_button(s.tab, cx);
                body = body.child(item_row(SharedString::from(format!("ins-ps-{}", s.tab)), 0., expander, "part-studio", self.insert_thumb(s.tab, None), s.name.clone(), cx).children(cfg_button).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.insert_studio_parts(s2.clone(), cx))));
                if open && let Some(el) = self.render_insert_config(s.tab, cx) {
                    body = body.child(div().pl(px(50.)).pr_2().child(el));
                }
                if open {
                    for part in parts {
                        let part2 = part.clone();
                        let tab = s.tab;
                        body = body.child(
                            item_row(SharedString::from(format!("ins-part-{}-{}", s.tab, part.0)), 22., None, if s.composites.contains(&part.0) { "composite-part" } else if part.3 { "surface-body" } else { "part" }, self.insert_thumb(s.tab, Some(&part.0)), part.1.clone(), cx)
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.insert_and_place(tab, Some(part2.clone()), cx))),
                        );
                    }
                    // Sketches are listed under the parts (live assembly/121).
                    for (feature, name) in sketches {
                        let tab = s.tab;
                        let n2 = name.clone();
                        let thumb = self.insert_thumb(s.tab, Some(&noshape_model::assembly::eval::sketch_body_id(feature)));
                        body = body.child(
                            item_row(SharedString::from(format!("ins-sk-{}-{}", s.tab, feature.0)), 22., None, "sketch", thumb, name, cx)
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.insert_sketch(tab, feature, n2.clone(), cx))),
                        );
                    }
                }
            }
            if !any {
                body = body.child(div().p_2().text_xs().text_color(pal.text_faint).child(if query.is_empty() { "No Part Studios with parts." } else { "No results." }));
            }
        } else {
            let list: Vec<(TabId, String)> = self.catalog.assemblies.iter().filter(|(_, n)| hit(n)).cloned().collect();
            if list.is_empty() {
                body = body.child(div().p_2().text_xs().text_color(pal.text_faint).child(if self.catalog.assemblies.is_empty() { "No other assemblies can be inserted." } else { "No results." }));
            }
            for (tab, name) in list {
                let n2 = name.clone();
                let cfg_button = self.insert_config_button(tab, cx);
                body = body.child(item_row(SharedString::from(format!("ins-asm-{tab}")), 0., None, "assembly", None, name, cx).children(cfg_button).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.insert_assembly(tab, n2.clone(), cx))));
                // A configured assembly: its configuration inputs before inserting.
                if let Some(el) = self.render_insert_config(tab, cx) {
                    body = body.child(div().pl(px(40.)).pr_2().child(el));
                }
            }
        }
        // Footer: inserted count, then the gray "Undo to remove instances" info row.
        let inserted = self.inserted_count(cx);
        let footer = v_flex()
            .flex_none()
            .child(h_flex().h(px(22.)).pr(px(4.)).justify_end().text_size(px(13.33)).font_weight(FontWeight::SEMIBOLD).child(format!("Inserted: {inserted}")))
            .child(
                h_flex()
                    .h(px(28.))
                    .px(px(5.))
                    .gap(px(3.))
                    .bg(gpui_kit::rgb(0xeeeeee))
                    .border_t_1()
                    .border_color(gpui_kit::rgb(0xd7d7d7))
                    .text_size(px(12.))
                    .text_color(gpui_kit::rgb(0x666666))
                    .child(
                        div()
                            .id("ins-undo")
                            .cursor_pointer()
                            .child(crate::ui::glyph("undo", 16., gpui_kit::rgb(0x666666)))
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.undo_last_insert(cx))),
                    )
                    .child(div().flex_1().child("Undo to remove instances"))
                    // Live dialog-assembly-insert: the 14 px help 4 px from the bottom right.
                    .relative()
                    .child(div().absolute().right(px(4.)).bottom(px(4.)).child(props::help_dot())),
            );
        let pal = p();
        v_flex()
            .w(px(300.))
            .h(px(587.))
            .max_h_full()
            .bg(pal.dialog_bg)
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
            .text_color(pal.text)
            .child(
                // Header: title, green check (dim until something is inserted), red X.
                h_flex()
                    .h(px(29.))
                    .flex_none()
                    .pl(px(6.))
                    .bg(gpui_kit::white())
                    .border_b_1()
                    .border_color(gpui_kit::rgb(0xd7d7d7))
                    .child(div().flex_1().text_size(px(13.)).font_weight(FontWeight::MEDIUM).truncate().child("Insert parts and assemblies"))
                    .child(
                        div()
                            .id("insert-ok")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(gpui_kit::rgb(0x009400))
                            .when(inserted == 0, |d| d.opacity(0.2))
                            .cursor_pointer()
                            .child(crate::ui::glyph("check", 18., gpui_kit::white()))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.close_insert(window, cx))),
                    )
                    .child(Button::new("insert-close").ghost().child(crate::assets::icon("dialog-cancel", 18., cx)).w(px(28.)).h(px(28.)).p_0().rounded(px(0.)).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.close_insert(window, cx)))),
            )
            .child(head)
            .child(body)
            .child(footer)
            .into_any_element()
    }

    fn render_relation_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(r) = &self.relation else { return div().into_any_element() };
        let mates: Vec<(MateId, String, MateType)> = self.assembly(cx).map(|a| a.mates.iter().map(|m| (m.id, m.name.clone(), m.mate_type)).collect()).unwrap_or_default();
        let name_of = |id: Option<MateId>| id.and_then(|id| mates.iter().find(|m| m.0 == id).map(|m| m.1.clone())).unwrap_or_else(|| "Select mate".into());
        let view = cx.entity();
        let picker = |slot: usize, current: Option<MateId>| {
            let mates = mates.clone();
            let view = view.clone();
            Button::new(SharedString::from(format!("rel-pick-{slot}"))).label(name_of(current)).outline().small().dropdown_caret(true).os_dropdown_menu(SharedString::from(format!("rel-pick-{slot}-dd")), true, move |mut menu, _, _| {
                for (id, name, t) in &mates {
                    let v = view.clone();
                    let id = *id;
                    menu = menu.item(crate::os_menu::OsMenuItem::new(name.clone()).icon(Icon::empty().path(mate_icon(*t))).on_click(move |_, _, cx| {
                        v.update(cx, |this, cx| {
                            if let Some(r) = &mut this.relation {
                                if slot == 0 {
                                    r.a = Some(id);
                                } else {
                                    r.b = Some(id);
                                }
                            }
                            cx.notify();
                        })
                    }));
                }
                menu
            })
        };
        let kinds = [(RelationType::Gear, "Gear"), (RelationType::RackAndPinion, "Rack & pinion"), (RelationType::Screw, "Screw"), (RelationType::Linear, "Linear")];
        let mut seg = h_flex().gap_1().flex_wrap();
        for (k, label) in kinds {
            seg = seg.child(Button::new(SharedString::from(format!("rel-k-{label}"))).label(label).ghost().xsmall().selected(r.kind == k).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                if let Some(r) = &mut this.relation {
                    r.kind = k;
                }
                cx.notify();
            })));
        }
        let (la, lb, lr) = match r.kind {
            RelationType::Gear => ("Driver (revolute)", "Driven (revolute)", "Ratio"),
            RelationType::RackAndPinion => ("Pinion (revolute)", "Rack (slider)", "Pitch radius"),
            RelationType::Screw => ("Screw (cylindrical)", "Same mate", "Lead per radian"),
            RelationType::Linear => ("Driver (slider)", "Driven (slider)", "Ratio"),
        };
        let body = v_flex()
            .p_3()
            .gap_2()
            .child(seg)
            .child(div().text_xs().text_color(pal.text_muted).child(la))
            .child(picker(0, r.a))
            .child(div().text_xs().text_color(pal.text_muted).child(lb))
            .child(picker(1, r.b))
            .child(div().text_xs().text_color(pal.text_muted).child(lr))
            .child(Input::new(&r.ratio).xsmall())
            .child(Checkbox::new("rel-rev").label("Reverse direction").checked(r.reverse).on_click(cx.listener(|this, v: &bool, _, cx| {
                if let Some(r) = &mut this.relation {
                    r.reverse = *v;
                }
                cx.notify();
            })))
            .child(h_flex().justify_end().gap_2().child(Button::new("rel-ok").label("OK").primary().small().on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.accept_relation(cx)))));
        Self::panel_frame("Relation", body, cx.listener(|this, _: &ClickEvent, _, cx| {
            this.relation = None;
            cx.notify();
        }), "rel-close")
        .into_any_element()
    }

    /// One table view per Variable Studio shown in this Assembly.
    fn sync_var_tables(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let studios: Vec<TabId> = noshape_model::variables::element_studios(&self.doc.read(cx).session.doc, self.tab).into_iter().filter(|r| r.external.is_none()).map(|r| r.studio).collect();
        self.var_tables.retain(|k, _| studios.contains(k));
        for s in studios {
            if !self.var_tables.contains_key(&s) {
                let (doc, host) = (self.doc.clone(), self.tab);
                let view = cx.new(|cx| crate::variable_studio::VariableStudioView::new_in_panel(doc, s, host, window, cx));
                self.var_tables.insert(s, view);
            }
        }
    }

    /// Onshape's Assembly Variable table: the same docked panel as the Part
    /// Studio's (712 px, "Variable table" tab title, `#eee` footer with the
    /// "Create Variable Studio | ▴" split button and `?`), listing the
    /// Variable Studio tables inserted here.
    fn render_variables_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let refs = noshape_model::variables::element_studios(&self.doc.read(cx).session.doc, self.tab);
        let target = crate::variable_insert::InsertTarget::Element(self.tab);
        let tables: Vec<AnyElement> = refs
            .iter()
            .enumerate()
            .filter_map(|(k, r)| match &r.external {
                // Another document's studio: read-only (Onshape).
                Some(_) => crate::variable_insert::readonly_table(&self.doc, target, r, k, refs.len(), cx),
                None => self.var_tables.get(&r.studio).map(|v| v.clone().into_any_element()),
            })
            .map(|t| div().pb(px(10.)).child(t).into_any_element())
            .collect();
        let empty = tables.is_empty();
        v_flex()
            .id("asm-variables")
            .occlude()
            .absolute()
            .top_0()
            .bottom_0()
            .right(px(33.))
            .w(px(712.))
            .max_w(relative(0.5))
            .bg(pal.panel)
            .border_l_1()
            .border_color(pal.border)
            .child(
                div().h(px(30.)).flex_none().pt(px(4.)).child(
                    div()
                        .h(px(26.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .border_b_2()
                        .border_color(pal.accent)
                        .text_size(px(13.33))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(pal.accent)
                        .child("Variable table"),
                ),
            )
            .child(
                v_flex()
                    .id("asm-variables-body")
                    .flex_1()
                    .p(px(5.))
                    .overflow_y_scroll()
                    .children(tables)
                    .when(empty, |d| {
                        d.child(
                            v_flex()
                                .items_center()
                                .pt(px(120.))
                                .gap(px(10.))
                                .child(div().opacity(0.15).child(crate::assets::icon("variable-table", 180., cx)))
                                .child(div().max_w(px(400.)).text_size(px(13.33)).text_color(pal.text_muted).text_center().child("Add variables to drive features in the Assembly.")),
                        )
                    }),
            )
            .child(
                h_flex()
                    .h(px(40.))
                    .flex_none()
                    .px(px(10.))
                    .bg(gpui_kit::rgb(0xeeeeee))
                    .border_t_1()
                    .border_color(gpui_kit::rgb(0xcccccc))
                    .child(div().flex_1().flex().justify_center().child(crate::variable_insert::panel_footer(self.doc.clone(), self.tab, cx)))
                    .child(div().size(px(14.)).rounded_full().bg(gpui_kit::rgb(0xaaaaaa)).flex().items_center().justify_center().text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb(0xffffff)).child("?")),
            )
            .into_any_element()
    }

    fn render_side_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let names: HashMap<InstanceId, String> = self.assembly(cx).map(|a| a.instances.iter().map(|i| (i.id, i.name.clone())).collect()).unwrap_or_default();
        let path_name = |p: &Vec<InstanceId>| -> String {
            let mut s = names.get(&p[0]).cloned().unwrap_or_default();
            if p.len() > 1
                && let Some(n) = self.result.node(p)
            {
                s = format!("{s} / {}", n.name);
            }
            s
        };
        match self.panel {
            Some(SidePanel::Interference) => {
                let units = self.units(cx);
                let mut body = v_flex().id("intf-body").p_2().gap_1().overflow_y_scroll().max_h(px(460.));
                if self.interference_running {
                    body = body.child(div().p_2().text_sm().text_color(pal.text_muted).child("Checking interference…"));
                } else if let Some(list) = &self.interference {
                    if list.is_empty() {
                        body = body.child(div().p_2().text_sm().text_color(pal.success).child("No interference found."));
                    }
                    for (i, it) in list.iter().enumerate() {
                        let (a, b) = (it.a[0], it.b[0]);
                        body = body.child(
                            v_flex()
                                .id(SharedString::from(format!("intf-{i}")))
                                .px_2()
                                .py_1()
                                .rounded_sm()
                                .cursor_pointer()
                                .hover(|d| d.bg(pal.panel_hover))
                                .child(div().text_sm().text_color(pal.error).child(format!("{}  ×  {}", path_name(&it.a), path_name(&it.b))))
                                .child(div().text_xs().text_color(pal.text_muted).child(format!("Volume ≈ {}", noshape_units::format_quantity(noshape_units::Quantity::new(it.volume, noshape_units::Dim::VOLUME), &units))))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.selected = vec![a, b];
                                    this.refresh_scene(false, cx);
                                    cx.notify();
                                })),
                        );
                    }
                }
                Self::panel_frame("Interference detection", body, cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.panel = None;
                    this.rebuild_scene(false, cx);
                    cx.notify();
                }), "intf-close")
                .into_any_element()
            }
            Some(SidePanel::Explode) => self.render_explode_panel(cx),
            Some(SidePanel::Bom) | Some(SidePanel::Configurations) | Some(SidePanel::Positions) | Some(SidePanel::Variables) | None => div().into_any_element(),
        }
    }

    fn render_overlay(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let pal = p();
        let mut out = Vec::new();
        // Left column: insert panel / mate dialog / relation dialog / animation.
        let mut left = v_flex().id("asm-left-dialogs").occlude().absolute().top_3().left_3().gap_2();
        let mut any_left = false;
        if self.insert_open {
            left = left.child(self.render_insert_panel(cx));
            any_left = true;
        }
        if self.ui.std.editing.is_some() {
            left = left.child(self.render_standard_edit_panel(cx));
            any_left = true;
        }
        // A connector opened from the mate dialog's chip docks to its right
        // (live 578: the Mate connector dialog at x 467, beside the mate's).
        let mc_beside = self.mate.is_some() && self.mc.as_ref().is_some_and(|m| m.for_mate.is_some());
        if let Some((dialog, hud)) = self.mate.as_ref().map(|s| (s.dialog.clone(), s.hud)) {
            if mc_beside {
                let mc = self.render_mc_panel(cx);
                left = left.child(h_flex().items_start().gap(px(1.)).child(dialog.clone()).child(mc));
            } else {
                left = left.child(dialog.clone());
            }
            any_left = true;
            if let Some(at) = hud.filter(|_| !dialog.read(cx).awaiting_pick()) {
                out.push(self.render_mate_hud(at, dialog, cx));
            }
        }
        if self.relation.is_some() {
            left = left.child(self.render_relation_panel(cx));
            any_left = true;
        }
        if self.anim.as_ref().is_some_and(|a| !a.preview) {
            left = left.child(self.render_anim_panel(cx));
            any_left = true;
        }
        if let Some(d) = self.render_folder_name(cx) {
            left = left.child(d);
            any_left = true;
        }
        if let Some(c) = self.render_curve_dialog(cx) {
            left = left.child(c);
            any_left = true;
        }
        if let Some(r) = self.render_reflection_dialog(cx) {
            left = left.child(r);
            any_left = true;
        }
        if let Some(z) = self.render_zebra_dialog(cx) {
            left = left.child(z);
            any_left = true;
        }
        if self.ui.mass.is_some() {
            let mut els = self.render_mass_dialog(cx).into_iter();
            if let Some(dialog) = els.next() {
                left = left.child(dialog);
                any_left = true;
            }
            out.extend(els);
        }
        if self.mc.is_some() && !mc_beside {
            left = left.child(self.render_mc_panel(cx));
            any_left = true;
        }
        if self.pattern.is_some() {
            left = left.child(self.render_pattern_panel(cx));
            any_left = true;
        }
        if self.config_panel.is_some() {
            left = left.child(self.render_config_panel(cx));
            any_left = true;
        }
        if self.replace_panel.is_some() {
            left = left.child(self.render_replace_panel(cx));
            any_left = true;
        }
        if self.step_edit.is_some() {
            left = left.child(self.render_step_edit(cx));
            any_left = true;
        }
        if self.ui.display_states {
            left = left.child(self.render_display_states(cx));
            any_left = true;
        }
        if self.ui.group.is_some() {
            left = left.child(self.render_group_dialog(cx));
            any_left = true;
        }
        if let Some(el) = self.render_np_driver(cx) {
            left = left.child(el);
            any_left = true;
        }
        if let Some(el) = self.render_ps_in_context_dialog(cx) {
            left = left.child(el);
            any_left = true;
        }
        if let Some(el) = self.render_reference_manager(cx) {
            left = left.child(el);
            any_left = true;
        }
        if let Some(el) = self.render_dynamic_suppression(cx) {
            left = left.child(el);
            any_left = true;
        }
        if let Some(el) = self.render_section(cx) {
            left = left.child(el);
            any_left = true;
        }
        if let Some(el) = self.render_isolate_panel(cx) {
            left = left.child(el);
            any_left = true;
        }
        if any_left {
            out.push(left.into_any_element());
        }
        if self.panel == Some(SidePanel::Variables) {
            out.push(self.render_variables_panel(cx));
        } else if self.panel == Some(SidePanel::Configurations) {
            out.push(div().absolute().top_0().bottom_0().right(px(33.)).max_w(relative(0.5)).border_l_1().border_color(pal.border).child(self.render_asm_config_panel(cx)).into_any_element());
        } else if self.panel == Some(SidePanel::Bom) {
            // Docked 712 px pane, like the variable table (live 181).
            out.push(div().absolute().top_0().bottom_0().right(px(33.)).max_w(relative(0.5)).border_l_1().border_color(pal.border).child(self.render_bom_panel(cx)).into_any_element());
        } else if self.panel == Some(SidePanel::Positions) {
            // Docked 712 px pane, like the variable table (live 150).
            out.push(div().absolute().top_0().bottom_0().right(px(33.)).max_w(relative(0.5)).border_l_1().border_color(pal.border).child(self.render_np_pane(cx)).into_any_element());
        } else if self.panel.is_some() {
            out.push(div().absolute().top_3().right(px(56.)).child(self.render_side_panel(cx)).into_any_element());
        }
        out.push(self.render_view_controls(cx));
        if let Some(el) = self.render_manip_value(cx) {
            out.push(el);
        }
        if let Some(el) = self.render_mode_banner(cx) {
            out.push(el);
        }
        if let Some(el) = self.render_cfg_toast(cx) {
            out.push(el);
        }
        if self.explode.is_some() && !self.explode_edit {
            out.push(div().absolute().bottom_3().right_3().px_2().py_1().rounded_md().bg(with_alpha(pal.accent, 0.25)).text_xs().child("Exploded view").into_any_element());
        }
        if let Some((err, msg)) = self.message.clone() {
            // Onshape's message bubble, top center of the viewport.
            let kind = if err { crate::ui::AlertKind::Error } else { crate::ui::AlertKind::Info };
            out.push(
                div()
                    .absolute()
                    .top(px(9.))
                    .left_0()
                    .right_0()
                    // Centered on the window (live), not the graphics area.
                    .pr(px(crate::ui::RAIL_W + if self.ui.list_collapsed { 0. } else { crate::ui::LEFT_PANEL_W }))
                    .flex()
                    .justify_center()
                    .child(crate::os_ui::message_bubble(
                        "asm-toast",
                        kind,
                        msg,
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.message = None;
                            cx.notify();
                        }),
                        cx,
                    ))
                    .into_any_element(),
            );
        }
        if let Some(el) = self.render_mini_toolbar(cx) {
            out.push(el);
        }
        if let Some(el) = self.render_shortcuts(cx) {
            out.push(el);
        }
        if let Some(el) = self.render_select_other(cx) {
            out.push(el);
        }
        if let Some(el) = self.render_tool_search(cx) {
            out.push(el);
        }
        if let Some(el) = self.render_np_toast(cx) {
            out.push(el);
        }
        out.extend(self.render_mate_values(cx));
        out.extend(self.render_center_of_mass(cx));
        if let Some(at) = self.ui.fixed_drag {
            // Fixed instance drag: the fix glyph at the cursor, `no-drop` cursor.
            out.push(div().absolute().top_0().left_0().size_full().cursor(CursorStyle::OperationNotAllowed).child(
                div()
                    .absolute()
                    .left(px(at.x as f32 - 10.))
                    .top(px(at.y as f32 - 8.))
                    .w(px(20.))
                    .h(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(3.))
                    .bg(rgb(0xffffff))
                    .child(crate::assets::icon(&icon_path("list-fixed"), 14., cx)),
            ).into_any_element());
        }
        if let Some(el) = self.render_measure(cx) {
            out.push(el);
        }
        if let Some(m) = self.render_graphics_menu() {
            out.push(m);
        }
        if let Some(m) = self.render_triad_menu(cx) {
            out.push(m);
        }
        out
    }

    /// Arrow keys / view cube arrows: Onshape's 250 ms rotation steps.
    fn rotate_by(&mut self, dx_deg: f64, dy_deg: f64, cx: &mut Context<Self>) {
        let (dir, deg) = nav::step_from_degrees(dx_deg, dy_deg);
        self.viewport.update(cx, |vp, cx| vp.rotate_step(dir, deg, cx));
    }

    fn render_view_controls(&mut self, cx: &mut Context<Self>) -> AnyElement {
        // Onshape: the same View cube as the Part Studio, 15 px from the
        // graphics area's top-right corner, with the camera menu under it.
        use crate::part_studio::view_cube::{CubeHooks, view_cube};
        let rotation = self.viewport.read(cx).camera.rotation;
        let menu = self.render_camera_menu(cx);
        let hooks = CubeHooks {
            state: |this: &mut Self| (&mut this.cube_hover, &mut this.cube_origin),
            rotation: |this: &Self, cx| this.viewport.read(cx).camera.rotation,
            click: |this, cell, cx| {
                let rot = cell.rotation();
                this.viewport.update(cx, |vp, cx| vp.animate_to_rotation_fit(rot, cx));
            },
            rotate: |this, dx, dy, cx| this.rotate_by(dx, dy, cx),
            roll: |this, deg, cx| {
                let dir = if deg > 0.0 { RotateDir::CounterClockwise } else { RotateDir::Clockwise };
                this.viewport.update(cx, |vp, cx| vp.rotate_step(dir, deg.abs(), cx));
            },
        };
        let cube = view_cube(rotation, self.cube_hover, menu, hooks, cx);
        // A docked 712 px pane narrows the graphics area (live 181): the cube
        // stays 15 px from the pane's left edge.
        let docked = matches!(self.panel, Some(SidePanel::Bom | SidePanel::Configurations | SidePanel::Positions | SidePanel::Variables));
        let right = if docked { 15. + 33. + 712f32.min(self.viewport.read(cx).size().x as f32 * 0.5) } else { 15. };
        div().absolute().top(px(15.)).right(px(right)).child(cube).into_any_element()
    }

    /// The camera menu under the cube (Onshape's "Camera and render options").
    fn render_camera_menu(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let view = cx.entity();
        let perspective = self.viewport.read(cx).camera.projection == noshape_render::Projection::Perspective;
        let show_mates = self.show_mates;
        let variant = gpui_kit::component::button::ButtonCustomVariant::new(cx).color(with_alpha(pal.panel, 0.35).into()).foreground(pal.text.into()).hover(pal.panel_hover.into()).active(pal.panel_active.into()).shadow(false);
        Button::new("asm-camera-menu")
            .custom(variant)
            .child(h_flex().gap(px(4.)).child(crate::assets::icon("view-cube", 20., cx)).child(crate::ui::glyph("ui-chevron-down", 8., pal.text)))
            .w(px(44.))
            .h(px(28.))
            .p_0()
            .rounded(px(3.))
            .tooltip("Camera and render options")
            .os_dropdown_menu("asm-camera-menu-dd", false, move |mut m, _, _| {
                for (label, sv) in [("Isometric", StandardView::Isometric), ("Dimetric", StandardView::Dimetric), ("Trimetric", StandardView::Trimetric)] {
                    let v = view.clone();
                    m = m.item(crate::os_menu::OsMenuItem::new(label).on_click(move |_, _, cx| v.update(cx, |this, cx| this.view(sv, cx))));
                }
                let (v1, v2, v3) = (view.clone(), view.clone(), view.clone());
                m.separator()
                    .item(crate::os_menu::OsMenuItem::new("Zoom to fit").on_click(move |_, _, cx| v1.update(cx, |this, cx| this.viewport.update(cx, |vp, cx| vp.zoom_to_fit(true, cx)))))
                    .separator()
                    .item(crate::os_menu::OsMenuItem::new("Perspective view").checked(perspective).on_click(move |_, _, cx| {
                        v2.update(cx, |this, cx| {
                            this.viewport.update(cx, |vp, cx| {
                                let p = if perspective { noshape_render::Projection::Orthographic } else { noshape_render::Projection::Perspective };
                                vp.set_projection(p, cx);
                            })
                        })
                    }))
                    .item(crate::os_menu::OsMenuItem::new("Show mates").checked(show_mates).on_click(move |_, _, cx| {
                        v3.update(cx, |this, cx| {
                            this.show_mates = !this.show_mates;
                            this.refresh_scene(false, cx);
                            cx.notify();
                        })
                    }))
            })
            .into_any_element()
    }
}

impl Render for AssemblyView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        self.ensure_filter_input(window, cx);
        if std::mem::take(&mut self.ui.preview_done) {
            self.finish_mate_preview(window, cx);
        }
        if self.panel == Some(SidePanel::Variables) {
            self.sync_var_tables(window, cx);
        }
        let toolbar = self.render_toolbar(cx);
        let tb_panel = self.render_tb_custom_panel(cx);
        let list = self.render_list(cx);
        let overlay = self.render_overlay(cx);
        let rail = self.render_panel_rail(cx);
        v_flex()
            .size_full()
            .key_context("Assembly")
            .track_focus(&self.focus)
            .capture_any_mouse_down(cx.listener(|this, _: &MouseDownEvent, window, cx| {
                // Keep keyboard shortcuts working after clicking the viewport or lists,
                // without stealing focus from an input that just took it.
                if !this.focus.contains_focused(window, cx) {
                    window.focus(&this.focus, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &InsertInstance, window, cx| this.toggle_insert(window, cx)))
            .on_action(cx.listener(|this, _: &FastenMate, window, cx| this.start_mate(MateType::Fastened, window, cx)))
            .on_action(cx.listener(|this, _: &ToggleMates, _, cx| {
                this.show_mates = !this.show_mates;
                this.refresh_scene(false, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleShowMatesMode, _, cx| this.toggle_show_mates_mode(cx)))
            .on_action(cx.listener(|this, _: &ToggleSnapMode, window, cx| this.toggle_snap(window, cx)))
            .on_action(cx.listener(|this, _: &ToggleMateConnectors, _, cx| {
                this.show_connectors = !this.show_connectors;
                this.refresh_scene(false, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &DeleteSelection, _, cx| {
                if let Some(m) = this.selected_mate {
                    this.delete_mate(m, cx);
                } else {
                    let ids = this.selected.clone();
                    this.delete_instances(ids, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &HideSelected, _, cx| this.hide_hovered_or_selected(cx)))
            .on_action(cx.listener(|this, _: &ShowAll, _, cx| this.show_hidden(cx)))
            .on_action(cx.listener(|this, _: &IsolateSelected, _, cx| {
                let ids = this.hovered_or_selected();
                this.isolate_cmd(false, ids, cx);
            }))
            .on_action(cx.listener(|this, _: &TransparentSelected, _, cx| {
                let ids = this.hovered_or_selected();
                this.isolate_cmd(true, ids, cx);
            }))
            .on_action(cx.listener(|this, _: &Copy, _, cx| this.copy_selection(cx)))
            .on_action(cx.listener(|this, _: &Paste, _, cx| this.paste_instances(cx)))
            .on_action(cx.listener(|this, _: &RenameSelected, window, cx| this.rename_selected(window, cx)))
            .on_action(cx.listener(|this, _: &ShortcutToolbar, _, cx| this.toggle_mini_toolbar(cx)))
            .on_action(cx.listener(|this, _: &ToggleShortcutHelp, _, cx| {
                this.ui.show_shortcuts = !this.ui.show_shortcuts;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &MateConnectorTool, window, cx| this.open_mc(None, window, cx)))
            .on_action(cx.listener(|this, _: &SelectOther, _, cx| this.select_other(false, cx)))
            .on_action(cx.listener(|this, _: &MeasureTool, _, cx| this.toggle_measure(cx)))
            .on_action(cx.listener(|this, _: &SectionView, window, cx| this.toggle_section(window, cx)))
            .on_action(cx.listener(|this, _: &SelectOtherPrevious, _, cx| this.select_other(true, cx)))
            .on_action(cx.listener(|this, _: &FlipPrimaryAxis, window, cx| this.flip_or_reorient(true, window, cx)))
            .on_action(cx.listener(|this, _: &ReorientSecondaryAxis, window, cx| this.flip_or_reorient(false, window, cx)))
            .on_action(cx.listener(|this, _: &ClearSelection, _, cx| {
                this.selected.clear();
                this.selected_mate = None;
                this.refresh_scene(false, cx);
                cx.notify();
            }))
            .on_modifiers_changed(cx.listener(|this, ev: &ModifiersChangedEvent, _, cx| {
                // Ctrl (tap) cycles the dragged connector in snap mode.
                let ctrl = ev.modifiers.control;
                if ctrl && !this.ctrl_down {
                    this.cycle_snap_source(cx);
                }
                this.ctrl_down = ctrl;
                // Shift (hold) locks mate inference to the hovered entity.
                if ev.modifiers.shift != this.infer_locked {
                    this.infer_locked = ev.modifiers.shift;
                    if this.update_inference(cx) || this.pick_mode(cx).is_some() {
                        this.refresh_scene(false, cx);
                    }
                }
            }))
            .on_action(cx.listener(|this, _: &SearchTools, window, cx| this.open_tool_search(window, cx)))
            .on_action(cx.listener(|this, _: &Cancel, window, cx| {
                if this.cancel_bom_cell(cx) {
                    window.focus(&this.focus, cx);
                } else if this.triad_menu.is_some() {
                    this.triad_menu = None;
                    cx.notify();
                } else if this.step_edit.is_some() {
                    this.step_edit = None;
                    cx.notify();
                } else if this.placing.is_some() || this.insert_open {
                    // Esc ends the insertion: placed instances stay (Undo removes them).
                    this.close_insert(window, cx);
                } else if this.mate.is_some() {
                    this.close_mate(window, cx);
                } else if this.ui.cfg_mode.is_some() {
                    this.finish_configure_features(window, cx);
                } else if this.snap.is_some() {
                    this.toggle_snap(window, cx);
                } else if this.mc.is_some() {
                    this.close_mc(window, cx);
                } else if this.ui.folder_name.is_some() {
                    this.close_folder_name(false, window, cx);
                } else if this.ui.curve_dialog.is_some() {
                    this.close_curve_surface(cx);
                } else if this.ui.reflection_dialog.is_some() {
                    this.close_reflection(false, cx);
                } else if this.ui.zebra_dialog.is_some() {
                    this.close_zebra(false, cx);
                } else if this.ui.mass.is_some() {
                    this.ui.mass = None;
                    cx.notify();
                } else if this.pattern.is_some() {
                    this.pattern = None;
                    cx.notify();
                } else if this.ui.group.is_some() {
                    this.ui.group = None;
                    this.update_capture(cx);
                    cx.notify();
                } else if this.ui.np_driver.is_some() {
                    this.ui.np_driver = None;
                    cx.notify();
                } else if this.ui.ps_in_context.is_some() || this.ui.ref_manager.is_some() {
                    this.ui.ps_in_context = None;
                    this.ui.ref_manager = None;
                    cx.notify();
                } else if this.ui.dynamic_suppression.is_some() {
                    this.ui.dynamic_suppression = None;
                    cx.notify();
                } else if this.show_mates_mode {
                    this.show_mates_mode = false;
                    this.mates_pinned = None;
                    this.update_capture(cx);
                    this.refresh_scene(false, cx);
                    cx.notify();
                } else if this.explode_edit {
                    this.explode_edit = false;
                    cx.notify();
                } else if this.ui.ctx_menu.is_some() || this.ui.mini_toolbar.is_some() || this.ui.show_shortcuts || this.ui.select_other.is_some() {
                    this.ui.select_other = None;
                    this.ui.ctx_menu = None;
                    this.ui.mini_toolbar = None;
                    this.ui.show_shortcuts = false;
                    cx.notify();
                } else if this.ui.measure.is_some() {
                    this.toggle_measure(cx);
                } else if this.ui.section.is_some() {
                    this.toggle_section(window, cx);
                } else if this.exit_isolate(cx) {
                } else {
                    // Esc keeps the selection (live 312; Space clears it).
                    this.insert_open = false;
                    this.relation = None;
                    this.refresh_scene(false, cx);
                    cx.notify();
                }
            }))
            .on_action(cx.listener(|this, _: &ViewFront, _, cx| this.view(StandardView::Front, cx)))
            .on_action(cx.listener(|this, _: &ViewBack, _, cx| this.view(StandardView::Back, cx)))
            .on_action(cx.listener(|this, _: &ViewLeft, _, cx| this.view(StandardView::Left, cx)))
            .on_action(cx.listener(|this, _: &ViewRight, _, cx| this.view(StandardView::Right, cx)))
            .on_action(cx.listener(|this, _: &ViewTop, _, cx| this.view(StandardView::Top, cx)))
            .on_action(cx.listener(|this, _: &ViewBottom, _, cx| this.view(StandardView::Bottom, cx)))
            .on_action(cx.listener(|this, _: &ViewIsometric, _, cx| this.view(StandardView::Isometric, cx)))
            .on_action(cx.listener(|this, _: &ZoomToFit, _, cx| this.viewport.update(cx, |vp, cx| vp.zoom_to_fit(true, cx))))
            // View navigation keys, as in the Part Studio (navigation.rs).
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| this.viewport.update(cx, |vp, cx| vp.zoom_step(true, cx))))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| this.viewport.update(cx, |vp, cx| vp.zoom_step(false, cx))))
            .on_action(cx.listener(|this, _: &RotateLeft, _, cx| this.rotate_by(-nav::ROTATE_ANGLE_DEG, 0.0, cx)))
            .on_action(cx.listener(|this, _: &RotateRight, _, cx| this.rotate_by(nav::ROTATE_ANGLE_DEG, 0.0, cx)))
            .on_action(cx.listener(|this, _: &RotateUp, _, cx| this.rotate_by(0.0, -nav::ROTATE_ANGLE_DEG, cx)))
            .on_action(cx.listener(|this, _: &RotateDown, _, cx| this.rotate_by(0.0, nav::ROTATE_ANGLE_DEG, cx)))
            .on_action(cx.listener(|this, _: &RotateLeftFine, _, cx| this.rotate_by(-nav::PRECISION_ROTATE_ANGLE_DEG, 0.0, cx)))
            .on_action(cx.listener(|this, _: &RotateRightFine, _, cx| this.rotate_by(nav::PRECISION_ROTATE_ANGLE_DEG, 0.0, cx)))
            .on_action(cx.listener(|this, _: &RotateUpFine, _, cx| this.rotate_by(0.0, -nav::PRECISION_ROTATE_ANGLE_DEG, cx)))
            .on_action(cx.listener(|this, _: &RotateDownFine, _, cx| this.rotate_by(0.0, nav::PRECISION_ROTATE_ANGLE_DEG, cx)))
            .on_action(cx.listener(|this, _: &RotateLeft90, _, cx| this.rotate_by(-nav::COARSE_ROTATE_ANGLE_DEG, 0.0, cx)))
            .on_action(cx.listener(|this, _: &RotateRight90, _, cx| this.rotate_by(nav::COARSE_ROTATE_ANGLE_DEG, 0.0, cx)))
            .on_action(cx.listener(|this, _: &RotateUp90, _, cx| this.rotate_by(0.0, -nav::COARSE_ROTATE_ANGLE_DEG, cx)))
            .on_action(cx.listener(|this, _: &RotateDown90, _, cx| this.rotate_by(0.0, nav::COARSE_ROTATE_ANGLE_DEG, cx)))
            .on_action(cx.listener(|this, _: &PanLeft, _, cx| this.viewport.update(cx, |vp, cx| vp.pan_step(-1.0, 0.0, cx))))
            .on_action(cx.listener(|this, _: &PanRight, _, cx| this.viewport.update(cx, |vp, cx| vp.pan_step(1.0, 0.0, cx))))
            .on_action(cx.listener(|this, _: &PanUp, _, cx| this.viewport.update(cx, |vp, cx| vp.pan_step(0.0, -1.0, cx))))
            .on_action(cx.listener(|this, _: &PanDown, _, cx| this.viewport.update(cx, |vp, cx| vp.pan_step(0.0, 1.0, cx))))
            .child(toolbar)
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .when(!self.ui.list_collapsed, |d| d.child(div().w(px(crate::ui::LEFT_PANEL_W)).h_full().flex_none().child(list)))
                    .child(
                        div()
                            .flex_1()
                            .h_full()
                            .relative()
                            .bg(pal.app_bg)
                            .child(self.viewport.clone())
                            // Live assembly/10: the Part Studio's list toggle and evaluation tools.
                            .child(div().absolute().top_0().bottom_0().left_0().flex().items_center().child(self.render_list_toggle(cx)))
                            .child(self.render_evaluation_tools(cx))
                            .children(overlay)
                            .child(rail),
                    )
                    // Customize toolbar's Tools panel, over the list and graphics.
                    .children(tb_panel),
            )
    }
}
