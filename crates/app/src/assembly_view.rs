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
use crate::doc_model::{DocEvent, DocModel};
use crate::gpu::Gpu;
use crate::scene_builder::PickKind;
use crate::theme::{p, with_alpha};
use crate::viewport::{Viewport, ViewportEvent};
use glam::{DVec2, DVec3};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem};
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
use noshape_model::reference::{BodyId, Inference, Reference};
use noshape_render::StandardView;
use noshape_units::ExprValue;
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

/// A part offered by the Insert panel: (body, name, bounds).
type CatalogPart = (BodyId, String, Option<(DVec3, DVec3)>);
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
}

#[derive(Clone, Debug, Default)]
struct Catalog {
    studios: Vec<CatalogStudio>,
    assemblies: Vec<(TabId, String)>,
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

#[path = "assembly_behaviors.rs"]
mod behaviors;
pub(crate) use behaviors::*;

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
    Interference,
    Explode,
    Positions,
}

pub struct AssemblyView {
    pub tab: TabId,
    doc: Entity<DocModel>,
    gpu: Gpu,
    viewport: Entity<Viewport>,
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
    naming_position: Option<Entity<InputState>>,
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
    ext_pick: Option<(crate::derived::ExternalDocInfo, usize)>,
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
        InstanceSource::Assembly { assembly } => contains_assembly(doc, *assembly, target, depth + 1),
        _ => false,
    })
}

fn inference_label(i: Inference) -> &'static str {
    match i {
        Inference::Centroid => "centroid",
        Inference::Center => "center",
        Inference::Midpoint => "midpoint",
        Inference::Vertex => "vertex",
        Inference::TopAxis => "top axis point",
        Inference::MidAxis => "mid axis point",
        Inference::BottomAxis => "bottom axis point",
        Inference::Origin => "origin",
    }
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
            ui: AsmUi::default(),
            expanded: HashSet::new(),
            message: None,
            naming_position: None,
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
            ext_pick: None,
            ext_updates: HashMap::new(),
            ext_checking: HashSet::new(),
            focus: cx.focus_handle(),
            fitted_once: false,
            _subs: subs,
        };
        this.schedule_eval(window, cx);
        this
    }

    fn assembly<'a>(&self, cx: &'a App) -> Option<&'a Assembly> {
        match &self.doc.read(cx).session.doc.tab(self.tab)?.content {
            TabContent::Assembly(a) => Some(a),
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
        if let Some(s) = &self.mc
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
                            TabContent::PartStudio(_) => {
                                let parts = ev
                                    .studio(&doc, t.id, &ConfigValues::default())
                                    .map(|r| {
                                        r.display.bodies.iter().filter(|b| matches!(b.kind, BodyKind::Solid | BodyKind::Surface)).map(|b| (b.id.clone(), b.name.clone(), b.bounds)).collect()
                                    })
                                    .unwrap_or_default();
                                catalog.studios.push(CatalogStudio { tab: t.id, name: t.name.clone(), parts });
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
        if std::mem::take(&mut self.pattern_resync) && self.mate.is_none() && self.pattern.is_none() {
            self.resync_patterns(cx);
        }
        self.check_external_updates(cx);
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
                vp.camera.set_view(StandardView::Isometric);
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
        if let Some(t) = &self.ui.transparent {
            o.ghost_tops.extend(t.iter().copied());
        }
        if let Some(s) = &self.ui.section {
            o.section.push(s.clip());
        }
        if self.isolate.is_some() || self.ui.transparent.is_some() {
            o.dim_alpha = self.ui.transparency;
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
        h.origin = Some((size * 0.8, picking && hovered_overlay == Some(OverlayPick::Origin)));
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
                    h.connectors.push((f, size * 0.9, hl));
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
                    h.mate_glyphs.push((f.origin, hl, st));
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
        h.manipulator = self.manipulator(cx).map(|f| (f, size * 1.6, self.manip_hover));
        // Pickable overlays changed: the pick buffer must be redrawn too.
        let mut sig: Vec<f64> = Vec::new();
        if let Some((f, s, _)) = &h.manipulator {
            sig.extend([f.origin.x, f.origin.y, f.origin.z, *s]);
        }
        if let Some((s, _)) = h.origin {
            sig.push(s);
        }
        for (f, s, _) in &h.connectors {
            sig.extend([f.origin.x, f.origin.y, f.origin.z, f.rotation.x, f.rotation.y, f.rotation.z, *s]);
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
            Some((_, n, _)) => format!("Insert : {n}"),
            None => format!("Insert : {studio_name}"),
        };
        // Configuration chosen in the Insert dialog (Onshape: set it before clicking the item).
        let config = self.ui.insert_config.get(&studio).cloned().unwrap_or_default();
        self.edit(&desc, cx, |a| {
            let id = InstanceId(a.next_id());
            let (source, base, width) = match body {
                Some((b, n, bb)) => (InstanceSource::Part { studio, body: b, config }, n, bb.map(|(lo, hi)| hi.x - lo.x).unwrap_or(0.02)),
                None => (InstanceSource::PartStudio { studio, config }, studio_name.clone(), 0.05),
            };
            // Later instances of the same item are offset so they don't overlap.
            let same = a.instances.iter().filter(|i| i.source == source).count();
            let placement = Placement { translation: [same as f64 * (width + 0.01), 0.0, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] };
            let name = a.next_instance_name(&base);
            a.instances.push(Instance { id, name, source, placement, fixed: false, hidden: false, suppressed: false, lock: SubassemblyLock::Flexible, nested: Vec::new() });
            id
        })
    }

    /// Start placing freshly inserted instances: they follow the cursor until a click.
    fn start_placing(&mut self, ids: Vec<InstanceId>, cx: &mut Context<Self>) {
        if ids.is_empty() {
            return;
        }
        self.placing = Some(ids);
        self.placing_point = None;
        self.ui.place_snap = None;
        self.ui.place_src_ix = 0;
        self.selected.clear();
        self.message = Some((false, "Click to place the inserted instance (Esc keeps it where it is).".into()));
        self.viewport.update(cx, |vp, _| vp.tool_captures_left = false);
        cx.notify();
    }

    fn insert_and_place(&mut self, studio: TabId, body: Option<CatalogPart>, cx: &mut Context<Self>) {
        if let Some(id) = self.insert_part(studio, body, cx) {
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
        let id = self.edit(&format!("Insert : {name}"), cx, |a| {
            let id = InstanceId(a.next_id());
            let n = a.next_instance_name(&name);
            a.instances.push(Instance { id, name: n, source: InstanceSource::Assembly { assembly: tab }, placement: Placement::default(), fixed: false, hidden: false, suppressed: false, lock: SubassemblyLock::Flexible, nested: Vec::new() });
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
        if self.snap.is_some() && ids.len() == 1 && self.update_insert_snap(ids[0], cx) {
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
            self.selected = ids;
        } else {
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

    fn toggle_fix_selected(&mut self, cx: &mut Context<Self>) {
        let ids = self.selected.clone();
        let all_fixed = self.assembly(cx).is_some_and(|a| ids.iter().all(|id| a.instance(*id).is_some_and(|i| i.fixed)));
        self.set_fixed(ids, !all_fixed, cx);
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
        self.edit(if s { "Suppress" } else { "Unsuppress" }, cx, |a| {
            for id in &ids {
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
        });
        self.selected.retain(|i| !ids.contains(i));
    }

    fn delete_mate(&mut self, id: MateId, cx: &mut Context<Self>) {
        let name = self.assembly(cx).and_then(|a| a.mate(id).map(|m| m.name.clone()).or_else(|| a.relations.iter().find(|r| r.id == id).map(|r| r.name.clone())).or_else(|| a.groups.iter().find(|g| g.id == id).map(|g| g.name.clone())));
        self.edit(&format!("Delete : {}", name.unwrap_or_default()), cx, |a| a.remove_mate(id));
        if self.selected_mate == Some(id) {
            self.selected_mate = None;
        }
    }

    fn toggle_mate_suppressed(&mut self, id: MateId, cx: &mut Context<Self>) {
        self.edit("Suppress mate", cx, |a| {
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
            let what = match &c.reference {
                Reference::Implicit { on, inference } => format!("{} {}", on.kind_label(), inference_label(*inference)),
                Reference::MateConnector { .. } => "Mate connector".into(),
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
        let Some(m) = self.assembly(cx).and_then(|a| a.mate(id).cloned()) else { return };
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
        // Solved placements of the preview (which includes this mate) are folded in.
        self.edit(&desc, cx, |a| match editing.and_then(|id| a.mates.iter().position(|x| x.id == id)) {
            Some(i) => a.mates[i] = m,
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

    fn save_named_position(&mut self, name: String, cx: &mut Context<Self>) {
        let placements = self.result.solved.placements.clone();
        let mate_values = self.assembly(cx).map(|a| self.result.named_position_values(a)).unwrap_or_default();
        self.edit(&format!("Insert : Named position {name}"), cx, |a| {
            a.named_positions.retain(|n| n.name != name);
            a.named_positions.push(NamedPosition { name, placements, mate_values });
        });
    }

    fn update_named_position(&mut self, i: usize, cx: &mut Context<Self>) {
        let placements = self.result.solved.placements.clone();
        let mate_values = self.assembly(cx).map(|a| self.result.named_position_values(a)).unwrap_or_default();
        let name = self.assembly(cx).and_then(|a| a.named_positions.get(i).map(|n| n.name.clone())).unwrap_or_default();
        self.edit(&format!("Update named position : {name}"), cx, |a| {
            if let Some(np) = a.named_positions.get_mut(i) {
                np.placements = placements;
                np.mate_values = mate_values;
            }
        });
        self.message = Some((false, format!("{name} updated to the current positions.")));
    }

    fn delete_named_position(&mut self, i: usize, cx: &mut Context<Self>) {
        let name = self.assembly(cx).and_then(|a| a.named_positions.get(i).map(|n| n.name.clone())).unwrap_or_default();
        self.edit(&format!("Delete named position : {name}"), cx, |a| {
            if i < a.named_positions.len() {
                a.named_positions.remove(i);
            }
        });
    }

    fn apply_named_position(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(np) = self.assembly(cx).and_then(|a| a.named_positions.get(i).cloned()) else { return };
        // Stored placements as the start, stored driving mate values imposed.
        let solved = self.result.prepared.solve_named_position(&np);
        let mut placements = np.placements.clone();
        for (id, p) in &solved.placements {
            match placements.iter_mut().find(|(i, _)| i == id) {
                Some(x) => x.1 = *p,
                None => placements.push((*id, *p)),
            }
        }
        let tab = self.tab;
        self.doc.update(cx, |doc, cx| {
            doc.edit_tab(tab, &format!("Apply named position : {}", np.name), cx, |c| {
                if let TabContent::Assembly(a) = c {
                    for (id, p) in &placements {
                        if let Some(inst) = a.instance_mut(*id) {
                            inst.placement = *p;
                        }
                    }
                }
            });
        });
    }

    // ------------------------------------------------------------ animation

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
        let free = self.mate.is_none() && self.anim.is_none() && self.placing.is_none() && self.pick_mode(cx).is_none() && !self.show_mates_mode && self.ui.group.is_none() && self.ui.measure.is_none();
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
            m.on = Some(c);
            m.active = false;
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
                if self.placing.is_some() {
                    // Only a click after the cursor moved over the viewport places
                    // (the click that chose the item in the Insert panel does not).
                    if self.placing_point.is_some() {
                        self.update_placing(cx);
                        self.finish_placing(true, window, cx);
                    }
                    return;
                }
                if self.ui.measure.is_some() {
                    // Measure tool: clicked entities are measured.
                    if let Some(t) = self.pick(*pick) {
                        self.measure_pick(t, cx);
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
                    // A fixed instance does not move: say so right away.
                    if !explode && let Some(i) = self.assembly(cx).and_then(|a| a.instance(top)).filter(|i| i.fixed) {
                        self.message = Some((false, format!("{} is fixed and cannot be dragged.", i.name)));
                        cx.notify();
                    }
                }
                let Some(d) = &self.drag else { return };
                if !d.started {
                    return;
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
                if self.snap_drag.is_some() {
                    self.finish_snap_drag(window, cx);
                    return;
                }
                let Some(d) = self.drag.take() else { return };
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
                            self.message = Some((false, format!("{name} is fixed and cannot be dragged.")));
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
                if let Some(OverlayPick::Manip(h)) = self.overlay(*pick) {
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
                match self.pick(*pick) {
                    Some(t) => {
                        let top = t.path[0];
                        if !self.selected.contains(&top) {
                            self.selected = vec![top];
                            self.selected_mate = None;
                        }
                        self.refresh_scene(false, cx);
                        self.open_graphics_menu(Some(top), *position, window, cx);
                    }
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

    fn tool(&self, id: &'static str, icon: &'static str, tip: &'static str, cx: &mut Context<Self>, f: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static) -> Button {
        // Onshape toolbar button: 30 px cell, 20 px full-color icon (§3.2).
        crate::ui::tool_button(id, icon, false, cx).tooltip(tip).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| f(this, window, cx)))
    }

    fn render_toolbar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let sep = crate::ui::tool_separator;
        let view = cx.entity();
        let mate_more = {
            let view = view.clone();
            crate::ui::tool_caret("tb-mate-more", cx).dropdown_menu(move |mut menu, _, _| {
                for t in MateType::ALL {
                    let v = view.clone();
                    menu = menu.item(PopupMenuItem::new(format!("{} mate", t.name())).icon(Icon::empty().path(mate_icon(t))).on_click(move |_, window, cx| v.update(cx, |this, cx| this.start_mate(t, window, cx))));
                }
                menu
            })
        };
        let relations = {
            let view = view.clone();
            crate::ui::tool_button("tb-relations", "gear-relation", false, cx).w(px(crate::ui::TOOL_CELL + 12.)).child(crate::ui::glyph("ui-chevron-down", 9., pal.text)).tooltip("Relations").dropdown_menu(move |mut menu, _, _| {
                for (label, icon, kind) in [
                    ("Gear relation", "gear-relation", RelationType::Gear),
                    ("Rack and pinion relation", "rack-pinion", RelationType::RackAndPinion),
                    ("Screw relation", "screw-relation", RelationType::Screw),
                    ("Linear relation", "linear-relation", RelationType::Linear),
                ] {
                    let v = view.clone();
                    menu = menu.item(PopupMenuItem::new(label).icon(Icon::empty().path(icon_path(icon))).on_click(move |_, window, cx| v.update(cx, |this, cx| this.open_relation(kind, None, window, cx))));
                }
                menu
            })
        };
        let patterns = {
            let view = view.clone();
            crate::ui::tool_caret("tb-pattern-more", cx).dropdown_menu(move |mut menu, _, _| {
                for (label, icon, kind) in [
                    ("Linear pattern", "linear-pattern", PatternDialogKind::Linear),
                    ("Circular pattern", "circular-pattern", PatternDialogKind::Circular),
                    ("Part pattern (follow a Part Studio pattern)", "linear-pattern", PatternDialogKind::PartPattern),
                    ("Mirror", "mirror", PatternDialogKind::Mirror),
                    ("Replicate", "replicate", PatternDialogKind::Replicate),
                ] {
                    let v = view.clone();
                    menu = menu.item(PopupMenuItem::new(label).icon(Icon::empty().path(icon_path(icon))).on_click(move |_, window, cx| v.update(cx, |this, cx| this.open_pattern(kind, window, cx))));
                }
                menu
            })
        };
        let exploded = self.explode.is_some();
        crate::ui::toolbar()
            .child(crate::ui::undo_redo(&self.doc, cx))
            .child(sep())
            .child(self.tool("tb-insert", "insert", "Insert parts and assemblies (I)", cx, |this, w, cx| this.toggle_insert(w, cx)).selected(self.insert_open))
            .child(sep())
            .child(self.tool("tb-replicate", "replicate", "Replicate (seed instance + its mate onto matching geometry)", cx, |this, w, cx| this.open_pattern(PatternDialogKind::Replicate, w, cx)))
            .child(h_flex().child(self.tool("tb-linpat", "linear-pattern", "Linear pattern", cx, |this, w, cx| this.open_pattern(PatternDialogKind::Linear, w, cx))).child(patterns))
            .child(self.tool("tb-circpat", "circular-pattern", "Circular pattern", cx, |this, w, cx| this.open_pattern(PatternDialogKind::Circular, w, cx)))
            .child(self.tool("tb-mirror", "mirror", "Mirror", cx, |this, w, cx| this.open_pattern(PatternDialogKind::Mirror, w, cx)))
            .child(sep())
            .child(self.tool("tb-mc", "mate-connector-assembly", "Mate connector", cx, |this, w, cx| this.open_mc(None, w, cx)).selected(self.mc.is_some()))
            .child(self.tool("tb-snap", "snap-mode", "Snap mode (Shift+S)", cx, |this, w, cx| this.toggle_snap(w, cx)).selected(self.snap.is_some()))
            .child(h_flex().child(self.tool("tb-fastened", "fastened", "Fastened mate (M)", cx, |this, w, cx| this.start_mate(MateType::Fastened, w, cx))).child(mate_more))
            .child(self.tool("tb-revolute", "revolute", "Revolute mate", cx, |this, w, cx| this.start_mate(MateType::Revolute, w, cx)))
            .child(self.tool("tb-slider", "slider", "Slider mate", cx, |this, w, cx| this.start_mate(MateType::Slider, w, cx)))
            .child(self.tool("tb-planar", "planar", "Planar mate", cx, |this, w, cx| this.start_mate(MateType::Planar, w, cx)))
            .child(self.tool("tb-cyl", "cylindrical", "Cylindrical mate", cx, |this, w, cx| this.start_mate(MateType::Cylindrical, w, cx)))
            .child(self.tool("tb-pinslot", "pin-slot", "Pin slot mate", cx, |this, w, cx| this.start_mate(MateType::PinSlot, w, cx)))
            .child(self.tool("tb-ball", "ball", "Ball mate", cx, |this, w, cx| this.start_mate(MateType::Ball, w, cx)))
            .child(self.tool("tb-parallel", "parallel-mate", "Parallel mate", cx, |this, w, cx| this.start_mate(MateType::Parallel, w, cx)))
            .child(self.tool("tb-tangent", "tangent-mate", "Tangent mate", cx, |this, w, cx| this.start_mate(MateType::Tangent, w, cx)))
            .child(self.tool("tb-width", "width-mate", "Width mate", cx, |this, w, cx| this.start_mate(MateType::Width, w, cx)))
            .child(sep())
            .child(self.tool("tb-group", "group", "Group (select 2+ instances)", cx, |this, _, cx| this.create_group(cx)))
            .child(relations)
            .child(self.tool("tb-fix", "fix-instance", "Fix / unfix selected", cx, |this, _, cx| this.toggle_fix_selected(cx)))
            .child(self.tool("tb-replace", "replace-part", "Replace instance", cx, |this, _, cx| this.replace_selected(cx)).selected(self.replace_panel.is_some()))
            .child(self.tool("tb-display-states", "display-states", "Display states", cx, |this, w, cx| this.toggle_display_states(w, cx)).selected(self.ui.display_states))
            .child(self.tool("tb-show-mates", "show-mates", "Show mates (H)", cx, |this, _, cx| this.toggle_show_mates_mode(cx)).selected(self.show_mates_mode))
            .child(sep())
            .child(self.tool("tb-explode", "explode", "Exploded views", cx, |this, _, cx| this.toggle_explode_panel(cx)).selected(exploded || self.panel == Some(SidePanel::Explode)))
            .child(self.tool("tb-named", "named-position", "Named positions", cx, |this, _, cx| {
                this.panel = if this.panel == Some(SidePanel::Positions) { None } else { Some(SidePanel::Positions) };
                cx.notify();
            }).selected(self.panel == Some(SidePanel::Positions)))
            .child(self.tool("tb-animate", "animate", "Animate mate", cx, |this, w, cx| this.start_animation(w, cx)).selected(self.anim.is_some()))
            .child(sep())
            .child(self.tool("tb-interference", "interference", "Interference detection", cx, |this, _, cx| {
                if this.panel == Some(SidePanel::Interference) {
                    this.panel = None;
                    this.rebuild_scene(false, cx);
                    cx.notify();
                } else {
                    this.run_interference(cx);
                }
            }).selected(self.panel == Some(SidePanel::Interference)))
            .child(self.tool("tb-bom", "bom", "Bill of materials", cx, |this, _, cx| {
                this.panel = if this.panel == Some(SidePanel::Bom) { None } else { Some(SidePanel::Bom) };
                cx.notify();
            }).selected(self.panel == Some(SidePanel::Bom)))
            .child(div().flex_1())
            .when(self.evaluating, |d| d.child(div().text_xs().text_color(pal.text_faint).child("solving…")))
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
        let suppressed = inst.map(|i| i.suppressed).unwrap_or_else(|| node.is_some_and(|n| n.suppressed));
        let error = node.and_then(|n| n.error.clone());
        // Onshape warns when more than one part is fixed.
        let fixed_tip: SharedString = if is_top && fixed && self.assembly(cx).is_some_and(|a| a.fixed_instances().len() > 1) { format!("{name} has warning: More than one part is fixed").into() } else { "Part is fixed".into() };
        let ext_key = inst.and_then(|i| i.source.external_key().map(|k| k.to_string()));
        let update = ext_key.as_ref().and_then(|k| self.ext_updates.get(k).cloned().flatten()).zip(ext_key.clone());
        let dof = if is_top { node.and_then(|n| n.dof) } else { None };
        let selected = is_top && self.selected.contains(&top);
        let has_children = node.is_some_and(|n| !n.children.is_empty());
        let expanded = self.expanded.contains(&path);
        let icon = match kind {
            NodeKind::Part => "part",
            NodeKind::PartStudio => "part-studio",
            NodeKind::Assembly => "sub-assembly",
        };
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
            .pl(px(5. + depth as f32 * 22.))
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
                None => div().flex_1().truncate().when(suppressed, |d| d.line_through()).child(name.clone()).into_any_element(),
            })
            .when(fixed, |d| d.child(div().id(SharedString::from(format!("fixed-{}", top.0))).child(crate::assets::icon(&icon_path("fix"), 12., cx)).tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(fixed_tip.clone()).build(w, cx))))
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
            .when_some(dof.filter(|d| *d > 0 && !fixed), |d, n| {
                d.child(div().id(SharedString::from(format!("dof-{}", top.0))).text_xs().text_color(pal.info).child(format!("{n}")).tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(format!("Has degrees of freedom ({n})")).build(w, cx)))
            })
            .when_some(error.clone(), |d, e| d.child(div().id(SharedString::from(format!("err-{}", top.0))).size(px(6.)).rounded_full().bg(pal.error).tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(e.clone()).build(w, cx))))
            .when_some(update.clone(), |d, (v, key)| {
                let tip = format!("Update available: {} (click to update)", v.label);
                d.child(
                    div()
                        .id(SharedString::from(format!("upd-{}", top.0)))
                        .px_1()
                        .rounded_sm()
                        .bg(with_alpha(pal.info, 0.2))
                        .text_xs()
                        .text_color(pal.info)
                        .child("update")
                        .tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(tip.clone()).build(w, cx))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            this.update_external(&key, &v, cx);
                        })),
                )
            })
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
        row.context_menu(move |menu, window, cx| {
            let mut entries = view.read(cx).instance_menu(top, false, cx);
            if let Some((ver, key)) = update.clone() {
                entries.push(MenuEntry::Sep);
                entries.push(MenuEntry::Item {
                    label: format!("Update to {}", ver.label),
                    icon: None,
                    disabled: false,
                    act: std::rc::Rc::new(move |t: &mut AssemblyView, _: &mut Window, cx: &mut Context<AssemblyView>| t.update_external(&key, &ver, cx)),
                });
            }
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
            .when_some(tip, |d, m| d.tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(m.clone()).build(w, cx)))
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
            .context_menu(move |menu, window, cx| {
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
        let view = cx.entity();
        h_flex()
            .id("asm-top-row")
            .h(px(crate::ui::ROW_H))
            .pl(px(5.))
            .pr_1()
            .gap_1p5()
            .rounded_sm()
            .text_sm()
            .text_color(pal.text)
            .when(all_selected, |d| d.bg(pal.accent_soft))
            .hover(|d| d.bg(pal.panel_hover))
            .child(div().w(px(12.)))
            .child(crate::assets::icon(&icon_path("assembly"), 14., cx))
            .child(match self.rename_field(behaviors::RenameRow::Instance(InstanceId::ORIGIN)) {
                Some(field) => field,
                None => div().flex_1().truncate().child(name).into_any_element(),
            })
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.selected = this.assembly(cx).map(|a| a.instances.iter().filter(|i| a.generated_by(i.id).is_none()).map(|i| i.id).collect()).unwrap_or_default();
                this.selected_mate = None;
                this.refresh_scene(false, cx);
                cx.notify();
            }))
            .context_menu(move |menu, window, cx| {
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
            if !self.instance_passes(inst, node) {
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
        let dof = result.solved.dof;
        v_flex()
            .size_full()
            .bg(pal.panel)
            .border_r_1()
            .border_color(pal.border)
            .child(
                h_flex()
                    .h(px(30.))
                    .flex_none()
                    .px(px(6.))
                    .justify_between()
                    .child(crate::ui::panel_title(format!("Instances ({})", asm.instances.len())))
                    .child(
                        h_flex()
                            .gap_1()
                            .child(div().text_xs().text_color(pal.text_faint).child(if dof > 0 { format!("{dof} DOF") } else { String::new() }))
                            // New folder (around the selection, or empty).
                            .child(Button::new("asm-new-folder").icon(Icon::empty().path(icon_path("folder"))).ghost().xsmall().tooltip("New folder").on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                let ids = this.selected.clone();
                                this.folder_from_instances(ids, window, cx);
                            }))),
                    ),
            )
            .child(self.render_list_filter(cx))
            .child(v_flex().id("instance-list").flex_1().min_h_0().pt(px(2.)).overflow_y_scroll().track_scroll(&self.ui.list_scroll).children(rows))
            .child(
                v_flex()
                    .max_h(px(280.))
                    .border_t_1()
                    .border_color(pal.border)
                    .pt(px(4.))
                    .child(crate::ui::tree_row("mates-folder", false).child(crate::ui::caret(true)).child(format!("Mates ({})", asm.mates.len() + asm.relations.len() + asm.groups.len() + asm.connectors.len())))
                    .child(v_flex().id("mate-list").pl(px(17.)).overflow_y_scroll().children(mate_rows)),
            )
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
                    .child(Button::new(id).ghost().child(crate::ui::glyph("close", 14., pal.cancel)).w(px(28.)).h(px(28.)).p_0().rounded(px(0.)).on_click(close)),
            )
            .child(body)
    }

    fn render_insert_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let query = self.ui.insert_search.as_ref().map(|s| s.read(cx).value().trim().to_lowercase()).unwrap_or_default();
        let hit = |name: &str| query.is_empty() || name.to_lowercase().contains(&query);
        let (source, tab) = (self.ui.insert_source, self.ui.insert_tab);
        let seg = |id: &'static str, label: &'static str, on: bool, cx: &mut Context<Self>, f: fn(&mut AsmUi)| {
            Button::new(id).label(label).ghost().xsmall().selected(on).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                f(&mut this.ui);
                cx.notify();
            }))
        };
        // Source tabs (Onshape: Current document | Other documents), element
        // tabs (Part Studios | Assemblies) and the search box.
        let mut head = v_flex().px_2().pt_2().gap_1().child(
            h_flex()
                .gap_1()
                .child(seg("ins-src-cur", "Current document", source == 0, cx, |u| u.insert_source = 0))
                .child(seg("ins-src-other", "Other documents", source == 1, cx, |u| u.insert_source = 1)),
        );
        if source == 0 {
            head = head.child(h_flex().gap_1().child(seg("ins-tab-ps", "Part Studios", tab == 0, cx, |u| u.insert_tab = 0)).child(seg("ins-tab-asm", "Assemblies", tab == 1, cx, |u| u.insert_tab = 1)));
            if let Some(st) = &self.ui.insert_search {
                head = head.child(Input::new(st).xsmall().cleanable(true));
            }
        }
        let mut body = v_flex().id("insert-body").p_2().gap_1().overflow_y_scroll().max_h(px(440.));
        if source == 1 {
            body = body.child(
                h_flex()
                    .id("ins-other-doc")
                    .h(px(26.))
                    .px_2()
                    .gap_2()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(crate::assets::icon(&icon_path("document"), 16., cx))
                    .child(div().flex_1().text_sm().child("Choose a document…"))
                    .on_click(cx.listener(|_, _: &ClickEvent, window, cx| {
                        let v = cx.entity();
                        crate::files::request_open_path("Insert from document", "noshape documents", &["noshape"], window, cx, move |path, cx| {
                            v.update(cx, |this, cx| {
                                match crate::derived::external_doc_info(&path) {
                                    Ok(info) => this.ext_pick = Some((info, 0)),
                                    Err(e) => this.message = Some((true, format!("Could not open {}: {e:#}", path.display()))),
                                }
                                cx.notify();
                            });
                        });
                    })),
            );
            if let Some(ch) = self.render_ext_insert_chooser(cx) {
                body = body.child(ch);
            }
        } else if tab == 0 {
            body = body.child(Checkbox::new("insert-rigid").label("Insert entire Part Studio as rigid").checked(self.insert_rigid).on_click(cx.listener(|this, v: &bool, _, cx| {
                this.insert_rigid = *v;
                cx.notify();
            })));
            let mut any = false;
            for s in self.catalog.studios.clone() {
                let studio_hit = hit(&s.name);
                let parts: Vec<CatalogPart> = s.parts.iter().filter(|p| studio_hit || hit(&p.1)).cloned().collect();
                if !studio_hit && parts.is_empty() {
                    continue;
                }
                any = true;
                let s2 = s.clone();
                body = body.child(
                    h_flex()
                        .id(SharedString::from(format!("ins-ps-{}", s.tab)))
                        .h(px(26.))
                        .px_2()
                        .gap_2()
                        .rounded_sm()
                        .cursor_pointer()
                        .hover(|d| d.bg(pal.panel_hover))
                        .child(crate::assets::icon(&icon_path("part-studio"), 16., cx))
                        .child(div().flex_1().text_sm().truncate().child(s.name.clone()))
                        .child(div().text_xs().text_color(pal.text_faint).child(if self.insert_rigid { "rigid".to_string() } else { format!("{} parts", s.parts.len()) }))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.insert_studio_parts(s2.clone(), cx))),
                );
                if let Some(el) = self.render_insert_config(s.tab, cx) {
                    body = body.child(el);
                }
                if !self.insert_rigid {
                    for part in parts {
                        let part2 = part.clone();
                        let tab = s.tab;
                        body = body.child(
                            h_flex()
                                .id(SharedString::from(format!("ins-part-{}-{}", s.tab, part.0)))
                                .h(px(24.))
                                .pl(px(26.))
                                .pr_2()
                                .gap_2()
                                .rounded_sm()
                                .cursor_pointer()
                                .text_sm()
                                .hover(|d| d.bg(pal.panel_hover))
                                .child(crate::assets::icon(&icon_path("part"), 14., cx))
                                .child(div().flex_1().truncate().child(part.1.clone()))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.insert_and_place(tab, Some(part2.clone()), cx))),
                        );
                    }
                }
            }
            if !any {
                body = body.child(div().px_2().text_xs().text_color(pal.text_faint).child(if query.is_empty() { "No Part Studios with parts." } else { "No results." }));
            }
        } else {
            let list: Vec<(TabId, String)> = self.catalog.assemblies.iter().filter(|(_, n)| hit(n)).cloned().collect();
            if list.is_empty() {
                body = body.child(div().px_2().text_xs().text_color(pal.text_faint).child(if self.catalog.assemblies.is_empty() { "No other assemblies can be inserted." } else { "No results." }));
            }
            for (tab, name) in list {
                let n2 = name.clone();
                body = body.child(
                    h_flex()
                        .id(SharedString::from(format!("ins-asm-{tab}")))
                        .h(px(26.))
                        .px_2()
                        .gap_2()
                        .rounded_sm()
                        .cursor_pointer()
                        .hover(|d| d.bg(pal.panel_hover))
                        .child(crate::assets::icon(&icon_path("assembly"), 16., cx))
                        .child(div().flex_1().text_sm().truncate().child(name))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.insert_assembly(tab, n2.clone(), cx))),
                );
            }
        }
        Self::panel_frame("Insert parts and assemblies", v_flex().child(head).child(body), cx.listener(|this, _: &ClickEvent, window, cx| this.close_insert(window, cx)), "insert-close").into_any_element()
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
            Button::new(SharedString::from(format!("rel-pick-{slot}"))).label(name_of(current)).outline().small().dropdown_caret(true).dropdown_menu(move |mut menu, _, _| {
                for (id, name, t) in &mates {
                    let v = view.clone();
                    let id = *id;
                    menu = menu.item(PopupMenuItem::new(name.clone()).icon(Icon::empty().path(mate_icon(*t))).on_click(move |_, _, cx| {
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
            Some(SidePanel::Bom) => {
                let rows = analysis::bom(&self.result);
                let studio_name = |t: TabId| self.doc.read(cx).session.doc.tab(t).map(|t| t.name.clone()).unwrap_or_default();
                // Configured parts: name the non-default configuration, so rows
                // of the same part in different configurations can be told apart.
                let config_text = |r: &analysis::BomRow| -> String {
                    match self.doc.read(cx).session.doc.tab(r.studio).map(|t| &t.content) {
                        Some(noshape_model::TabContent::PartStudio(ps)) => ps.configuration.describe(&r.config),
                        _ => String::new(),
                    }
                };
                let mut body = v_flex().id("bom-body").p_2().gap_px().overflow_y_scroll().max_h(px(460.));
                body = body.child(
                    h_flex()
                        .px_2()
                        .h(px(22.))
                        .text_xs()
                        .text_color(pal.text_muted)
                        .child(div().w(px(28.)).child("#"))
                        .child(div().flex_1().child("Name"))
                        .child(div().w(px(90.)).child("Source"))
                        .child(div().w(px(32.)).child("Qty")),
                );
                for r in &rows {
                    body = body.child(
                        h_flex()
                            .px_2()
                            .h(px(24.))
                            .text_sm()
                            .rounded_sm()
                            .hover(|d| d.bg(pal.panel_hover))
                            .child(div().w(px(28.)).text_color(pal.text_faint).child(format!("{}", r.item)))
                            .child(div().flex_1().truncate().child(match config_text(r) {
                                c if c.is_empty() => r.name.clone(),
                                c => format!("{} ({c})", r.name),
                            }))
                            .child(div().w(px(90.)).truncate().text_xs().text_color(pal.text_muted).child(match r.external.as_deref().and_then(crate::derived::ExternalRef::parse) {
                                Some(e) => e.path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "Other document".into()),
                                None => studio_name(r.studio),
                            }))
                            .child(div().w(px(32.)).child(format!("{}", r.quantity))),
                    );
                }
                if rows.is_empty() {
                    body = body.child(div().p_2().text_xs().text_color(pal.text_faint).child("Add parts and assemblies to build a bill of materials."));
                }
                Self::panel_frame("Bill of materials", body, cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.panel = None;
                    cx.notify();
                }), "bom-close")
                .into_any_element()
            }
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
            Some(SidePanel::Positions) => self.render_positions_panel(cx),
            None => div().into_any_element(),
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
        if let Some(s) = &self.mate {
            left = left.child(s.dialog.clone());
            any_left = true;
            if let Some(at) = s.hud.filter(|_| !s.dialog.read(cx).awaiting_pick()) {
                out.push(
                    div()
                        .absolute()
                        .left(px(at.x as f32 + 14.))
                        .top(px(at.y as f32 + 14.))
                        .child(
                            Button::new("mate-hud-ok")
                                .icon(Icon::empty().path(icon_path("check")))
                                .primary()
                                .xsmall()
                                .tooltip("Commit current mate and start a new mate")
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.accept_mate_and_new(window, cx))),
                        )
                        .into_any_element(),
                );
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
        if self.mc.is_some() {
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
        if let Some(el) = self.render_section(cx) {
            left = left.child(el);
            any_left = true;
        }
        if any_left {
            out.push(left.into_any_element());
        }
        if self.panel.is_some() {
            out.push(div().absolute().top_3().right(px(56.)).child(self.render_side_panel(cx)).into_any_element());
        }
        out.push(self.render_view_controls(cx));
        if let Some(el) = self.render_manip_value(cx) {
            out.push(el);
        }
        if let Some(el) = self.render_mode_banner(cx) {
            out.push(el);
        }
        if self.explode.is_some() && !self.explode_edit {
            out.push(div().absolute().bottom_3().right_3().px_2().py_1().rounded_md().bg(with_alpha(pal.accent, 0.25)).text_xs().child("Exploded view").into_any_element());
        }
        if let Some((err, msg)) = self.message.clone() {
            let c = if err { pal.error } else { pal.info };
            out.push(
                h_flex()
                    .id("asm-toast")
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
                    .child(Button::new("asm-toast-x").icon(IconName::Close).ghost().xsmall().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.message = None;
                        cx.notify();
                    })))
                    .into_any_element(),
            );
        }
        if let Some(el) = self.render_isolate_panel(cx) {
            out.push(el);
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
        out.extend(self.render_mate_values(cx));
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

    fn render_view_controls(&mut self, cx: &mut Context<Self>) -> AnyElement {
        // Onshape-style rail of white 32 px buttons at the viewport's top-right.
        let pal = p();
        let cell = |id: &'static str, icon: &'static str, tip: &'static str, on: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .size(px(32.))
                .flex()
                .items_center()
                .justify_center()
                .border_t_1()
                .border_color(pal.border)
                .bg(if on { pal.row_hover } else { pal.panel })
                .hover(|d| d.bg(pal.panel_hover))
                .cursor_pointer()
                .child(crate::assets::icon(icon, 20., cx))
                .tooltip(crate::ui::tooltip(tip))
        };
        let view = |id: &'static str, icon: &'static str, tip: &'static str, v: StandardView, cx: &mut Context<Self>| cell(id, icon, tip, false, cx).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.view(v, cx)));
        v_flex()
            .absolute()
            .top(px(15.))
            .right(px(15.))
            .border_l_1()
            .border_r_1()
            .border_b_1()
            .border_color(pal.border)
            .child(view("av-iso", "iso-view", "Isometric (Shift+7)", StandardView::Isometric, cx))
            .child(view("av-front", "front-view", "Front (Shift+1)", StandardView::Front, cx))
            .child(view("av-top", "top-view", "Top (Shift+5)", StandardView::Top, cx))
            .child(view("av-right", "right-view", "Right (Shift+4)", StandardView::Right, cx))
            .child(cell("av-fit", "zoom-fit", "Zoom to fit (F)", false, cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.viewport.update(cx, |vp, cx| vp.zoom_to_fit(true, cx));
            })))
            .child(cell("av-mates", "show-mates", "Show mates (J)", self.show_mates, cx).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.show_mates = !this.show_mates;
                this.refresh_scene(false, cx);
                cx.notify();
            })))
            .into_any_element()
    }
}

impl Render for AssemblyView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        self.ensure_filter_input(window, cx);
        let toolbar = self.render_toolbar(cx);
        let list = self.render_list(cx);
        let overlay = self.render_overlay(cx);
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
            .on_action(cx.listener(|this, _: &Cancel, window, cx| {
                if this.triad_menu.is_some() {
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
                } else if this.snap.is_some() {
                    this.toggle_snap(window, cx);
                } else if this.mc.is_some() {
                    this.close_mc(window, cx);
                } else if this.pattern.is_some() {
                    this.pattern = None;
                    cx.notify();
                } else if this.ui.group.is_some() {
                    this.ui.group = None;
                    this.update_capture(cx);
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
                    this.insert_open = false;
                    this.relation = None;
                    this.selected.clear();
                    this.selected_mate = None;
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
            .child(toolbar)
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(div().w(px(250.)).h_full().flex_none().child(list))
                    .child(div().flex_1().h_full().relative().bg(pal.app_bg).child(self.viewport.clone()).children(overlay)),
            )
    }
}
