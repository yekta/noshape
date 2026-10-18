//! Assembly tab: toolbar, instances + mate features list, viewport with
//! per-instance transforms, selection, implicit mate connector inference on
//! hover, mate / relation dialogs, insert panel, drag within DOF, explode,
//! named positions, mate animation, interference check and BOM.
//!
//! Evaluation (Part Studio regeneration + solve) runs in a background task;
//! drags and animation re-solve the cached [`Prepared`] problem off the UI
//! thread and only update instance transforms. Every change commits through
//! [`DocModel::edit_tab`], folding in the latest solved placements.

use crate::actions::*;
use crate::assembly_mate_dialog::{ConnectorLabeler, MateDialog, MateDialogEvent, mate_icon};
use crate::assembly_scene::{self, AsmBuilt, AsmGpuCache, AsmPickTarget, AsmSceneOptions, Highlights, Triad};
use crate::assets::icon_path;
use crate::doc_model::{DocEvent, DocModel};
use crate::gpu::Gpu;
use crate::scene_builder::PickKind;
use crate::theme::{p, with_alpha};
use crate::viewport::{Viewport, ViewportEvent};
use glam::{DVec2, DVec3};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{Icon, IconName, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::assembly::analysis::{self, Interference};
use noshape_model::assembly::eval::{AssemblyEvaluator, AssemblyResult, InstanceNode, NodeKind, SolveOptions, inference_candidates, placement_to_frame};
use noshape_model::assembly::solver::Frame;
use noshape_model::assembly::{
    Assembly, Group, Instance, InstanceId, InstanceSource, Mate, MateConnectorRef, MateId, MateType, NamedPosition, Placement, Relation, RelationType,
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

/// A Part Studio or Assembly offered by the Insert panel.
#[derive(Clone, Debug)]
struct CatalogStudio {
    tab: TabId,
    name: String,
    parts: Vec<(BodyId, String, Option<(DVec3, DVec3)>)>,
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
    /// Manipulator drag: constrained to this world axis through `plane_point`.
    axis: Option<DVec3>,
}

#[derive(Clone, Copy, PartialEq)]
enum Playback {
    Loop,
    Reciprocate,
}

struct AnimState {
    mate: MateId,
    start: f64,
    end: f64,
    t: f64,
    dir: f64,
    playing: bool,
    playback: Playback,
    angular: bool,
    busy: bool,
    task: Option<Task<()>>,
}

#[derive(Clone, Copy, PartialEq)]
enum SidePanel {
    Bom,
    Interference,
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
    /// Hovered manipulator axis.
    manip_hover: Option<usize>,
    pointer: DVec2,
    /// Inference candidates (body coordinates) for the hovered entity, and the chosen index.
    infer: Option<((Vec<InstanceId>, Reference), Vec<(Reference, Frame)>)>,
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
    context_menu: Option<(Point<Pixels>, InstanceId)>,
    expanded: HashSet<Vec<InstanceId>>,
    message: Option<(bool, String)>,
    naming_position: Option<Entity<InputState>>,
    focus: FocusHandle,
    fitted_once: bool,
    _subs: Vec<Subscription>,
}

impl Focusable for AssemblyView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

fn wrap_angle(a: f64) -> f64 {
    let t = std::f64::consts::TAU;
    let mut a = a % t;
    if a > std::f64::consts::PI {
        a -= t;
    }
    if a < -std::f64::consts::PI {
        a += t;
    }
    a
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
            DocEvent::TabChanged(_) | DocEvent::TabsChanged => this.schedule_eval(window, cx),
            DocEvent::Reloaded => {
                this.close_mate(window, cx);
                this.schedule_eval(window, cx)
            }
            DocEvent::Error(msg) => {
                this.message = Some((true, msg.clone()));
                cx.notify();
            }
            _ => {}
        }));
        let mut this = Self {
            tab,
            doc,
            gpu,
            viewport,
            evaluator: Arc::new(Mutex::new(AssemblyEvaluator::new())),
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
            context_menu: None,
            expanded: HashSet::new(),
            message: None,
            naming_position: None,
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
            // The second connector's instance stays put unless the first is fixed.
            if !fixed(m.a.instance) && !fixed(m.b.instance) {
                opts.hold.push(m.b.instance);
            }
            match s.editing.and_then(|id| asm.mates.iter().position(|x| x.id == id)) {
                Some(i) => asm.mates[i] = m,
                None => asm.mates.push(m),
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
        self.assembly(cx).and_then(|a| a.mate(id)).map(|m| vec![m.a.instance, m.b.instance]).unwrap_or_default()
    }

    /// Push highlight state + overlays to the viewport.
    fn refresh_scene(&mut self, geometry_changed: bool, cx: &mut Context<Self>) {
        let Some(built) = &self.built else { return };
        let mut h = Highlights { selected_tops: self.selected.clone(), ..Default::default() };
        let size = self.triad_size();
        if let Some(t) = &self.hover
            && matches!(t.kind, PickKind::Face | PickKind::Edge | PickKind::Vertex)
        {
            h.hover_entity = Some((t.path.clone(), t.reference.clone()));
        }
        // Mate highlight (list hover / selection): instances accent + both connector triads.
        for mid in [self.selected_mate, self.list_hover_mate].into_iter().flatten() {
            h.accent_tops.extend(self.mate_instances(mid, cx));
            if let Some(ms) = self.result.mates.iter().find(|m| m.id == mid) {
                for f in [ms.frame_a, ms.frame_b].into_iter().flatten() {
                    h.triads.push(Triad { frame: f, size, strong: true });
                }
            }
        }
        if self.show_mates {
            for ms in &self.result.mates {
                if let Some(f) = ms.frame_a {
                    h.triads.push(Triad { frame: f, size: size * 0.6, strong: false });
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
        // Mate dialog: chosen connectors + inference preview.
        if let Some(s) = &self.mate {
            let d = s.dialog.read(cx);
            for c in [&d.a, &d.b].into_iter().flatten() {
                if let Some(f) = self.connector_world(c, cx) {
                    h.triads.push(Triad { frame: f, size, strong: false });
                }
            }
            if d.awaiting_pick()
                && let (Some(((path, _), cands)), Some(hov)) = (&self.infer, &self.hover)
                && let Some(o) = self.result.occurrences.get(hov.occ)
            {
                let t = noshape_model::assembly::eval::matrix_frame(&o.transform);
                for (i, (_, f)) in cands.iter().enumerate() {
                    let w = t.mul(f);
                    if Some(i) == self.infer_choice && *path == hov.path {
                        h.triads.push(Triad { frame: w, size, strong: true });
                    } else {
                        h.dots.push(w.origin);
                    }
                }
            }
        }
        h.manipulator = self.manipulator(cx).map(|f| (f, size * 1.6, self.manip_hover));
        let scene = assembly_scene::with_highlights(built, &self.result, &h, self.gpu.uploader());
        self.viewport.update(cx, |vp, cx| vp.set_scene(Arc::new(scene), geometry_changed, cx));
    }

    /// Move manipulator frame: at the single selected, non-fixed instance's origin.
    fn manipulator(&self, cx: &App) -> Option<Frame> {
        if self.mate.is_some() || self.anim.is_some() || self.selected.len() != 1 {
            return None;
        }
        let id = self.selected[0];
        let inst = self.assembly(cx)?.instance(id)?;
        if inst.fixed || inst.suppressed || inst.hidden {
            return None;
        }
        let p = self.result.solved.placement(id)?;
        // Triad at the instance's bounds center, aligned with world axes.
        let center = self
            .result
            .occurrences
            .iter()
            .filter(|o| o.top() == id)
            .filter_map(|o| o.world_bounds())
            .fold(None, |acc: Option<(DVec3, DVec3)>, (a, b)| Some(acc.map(|(x, y)| (x.min(a), y.max(b))).unwrap_or((a, b))))
            .map(|(a, b)| (a + b) * 0.5)
            .unwrap_or(DVec3::from(p.translation));
        Some(Frame::new(center, glam::DQuat::IDENTITY))
    }

    /// World frame of a connector reference (resolved against the current result).
    fn connector_world(&self, c: &MateConnectorRef, cx: &App) -> Option<Frame> {
        let mut path = vec![c.instance];
        path.extend(c.path.iter().copied());
        let o = self.result.occurrences.iter().find(|o| o.path == path)?;
        let local = noshape_model::assembly::eval::resolve_connector_in(&o.context, Some(&o.body), &c.reference, &self.units(cx))?;
        Some(noshape_model::assembly::eval::matrix_frame(&o.transform).mul(&local))
    }

    // ------------------------------------------------------------ editing

    /// Commit a change to the assembly, folding in the latest solved placements.
    fn edit(&mut self, desc: &str, cx: &mut Context<Self>, f: impl FnOnce(&mut Assembly)) {
        let solved = self.result.solved.placements.clone();
        let tab = self.tab;
        self.doc.update(cx, |doc, cx| {
            doc.edit_tab(tab, desc, cx, |c| {
                if let TabContent::Assembly(a) = c {
                    for (id, p) in &solved {
                        if let Some(i) = a.instance_mut(*id) {
                            i.placement = *p;
                        }
                    }
                    f(a);
                }
            });
        });
    }

    fn insert_part(&mut self, studio: TabId, body: Option<(BodyId, String, Option<(DVec3, DVec3)>)>, cx: &mut Context<Self>) {
        let studio_name = self.doc.read(cx).session.doc.tab(studio).map(|t| t.name.clone()).unwrap_or_default();
        let desc = match &body {
            Some((_, n, _)) => format!("Insert : {n}"),
            None => format!("Insert : {studio_name}"),
        };
        self.edit(&desc, cx, |a| {
            let id = InstanceId(a.next_id());
            let (source, base, width) = match body {
                Some((b, n, bb)) => (InstanceSource::Part { studio, body: b, config: ConfigValues::default() }, n, bb.map(|(lo, hi)| hi.x - lo.x).unwrap_or(0.02)),
                None => (InstanceSource::PartStudio { studio, config: ConfigValues::default() }, studio_name.clone(), 0.05),
            };
            // Later instances of the same item are offset so they don't overlap.
            let same = a.instances.iter().filter(|i| i.source == source).count();
            let placement = Placement { translation: [same as f64 * (width + 0.01), 0.0, 0.0], rotation: [0.0, 0.0, 0.0, 1.0] };
            let name = a.next_instance_name(&base);
            a.instances.push(Instance { id, name, source, placement, fixed: false, hidden: false, suppressed: false });
        });
    }

    fn insert_studio_parts(&mut self, s: CatalogStudio, cx: &mut Context<Self>) {
        if self.insert_rigid {
            self.insert_part(s.tab, None, cx);
        } else {
            for part in s.parts {
                self.insert_part(s.tab, Some(part), cx);
            }
        }
    }

    fn insert_assembly(&mut self, tab: TabId, name: String, cx: &mut Context<Self>) {
        self.edit(&format!("Insert : {name}"), cx, |a| {
            let id = InstanceId(a.next_id());
            let n = a.next_instance_name(&name);
            a.instances.push(Instance { id, name: n, source: InstanceSource::Assembly { assembly: tab }, placement: Placement::default(), fixed: false, hidden: false, suppressed: false });
        });
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

    fn isolate(&mut self, ids: Vec<InstanceId>, cx: &mut Context<Self>) {
        self.isolate = if ids.is_empty() || self.isolate.is_some() { None } else { Some(ids.into_iter().collect()) };
        self.rebuild_scene(false, cx);
        cx.notify();
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

    fn create_group(&mut self, cx: &mut Context<Self>) {
        let ids = self.selected.clone();
        if ids.len() < 2 {
            self.message = Some((false, "Select at least two instances to group.".into()));
            cx.notify();
            return;
        }
        self.edit("Insert : Group", cx, |a| {
            let id = MateId(a.next_id());
            let name = a.next_feature_name("Group");
            a.groups.push(Group { id, name, instances: ids, suppressed: false });
        });
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
        Arc::new(move |c: &MateConnectorRef| {
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
        let dummy = MateConnectorRef { instance: InstanceId(0), path: vec![], reference: Reference::Origin, flip: false, rotate_quarters: 0 };
        let mate = Mate {
            id,
            name,
            mate_type: t,
            a: dummy.clone(),
            b: dummy,
            offset_z: None,
            offset_x: None,
            offset_y: None,
            rotation: None,
            flip_primary: false,
            reorient_secondary: 0,
            limits: vec![],
            suppressed: false,
        };
        self.open_mate_dialog(mate, None, window, cx);
    }

    fn edit_mate(&mut self, id: MateId, window: &mut Window, cx: &mut Context<Self>) {
        self.close_mate(window, cx);
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
            MateDialogEvent::Cancel => this.close_mate(window, cx),
        });
        dialog.update(cx, |d, cx| window.focus(&d.focus_handle(cx), cx));
        self.mate = Some(MateSession { dialog, editing, _sub: sub });
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

    fn close_mate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        let mates: Vec<MateId> = self.assembly(cx).map(|a| a.mates.iter().filter(|m| m.mate_type.dof() > 0).map(|m| m.id).collect()).unwrap_or_default();
        self.relation = Some(RelationSession {
            kind: existing.as_ref().map(|r| r.relation_type).unwrap_or(kind),
            a: existing.as_ref().map(|r| r.mates[0]).or(mates.first().copied()),
            b: existing.as_ref().map(|r| r.mates[1]).or(mates.get(1).copied()),
            ratio,
            reverse: existing.as_ref().is_some_and(|r| r.reverse),
            editing: existing.map(|r| r.id),
        });
        cx.notify();
    }

    fn accept_relation(&mut self, cx: &mut Context<Self>) {
        let Some(r) = self.relation.take() else { return };
        let (Some(a), Some(b)) = (r.a, r.b) else {
            self.message = Some((false, "Select two mates for the relation.".into()));
            cx.notify();
            return;
        };
        if a == b {
            self.message = Some((false, "A relation needs two different mates.".into()));
            cx.notify();
            return;
        }
        let ratio = ExprValue::new(r.ratio.read(cx).value().to_string());
        let (kind, reverse, editing) = (r.kind, r.reverse, r.editing);
        let base = match kind {
            RelationType::Gear => "Gear",
            RelationType::RackAndPinion => "Rack and pinion",
            RelationType::Screw => "Screw",
            RelationType::Linear => "Linear",
        };
        self.edit(&format!("Insert : {base} relation"), cx, |asm| {
            if let Some(id) = editing
                && let Some(x) = asm.relations.iter_mut().find(|x| x.id == id)
            {
                x.relation_type = kind;
                x.mates = [a, b];
                x.ratio = ratio;
                x.reverse = reverse;
                return;
            }
            let id = MateId(asm.next_id());
            let name = asm.next_feature_name(base);
            asm.relations.push(Relation { id, name, relation_type: kind, mates: [a, b], ratio, reverse, suppressed: false });
        });
    }

    // ------------------------------------------------------------ explode, positions

    fn toggle_explode(&mut self, cx: &mut Context<Self>) {
        if self.explode.is_some() {
            self.explode = None;
        } else {
            let has = self.assembly(cx).is_some_and(|a| !a.exploded_views.is_empty());
            if !has {
                let view = analysis::auto_exploded_view(&self.result, "Exploded view 1", 0.9);
                self.edit("Insert : Exploded view 1", cx, |a| a.exploded_views.push(view));
            }
            self.explode = Some(0);
        }
        self.reapply_display(cx);
    }

    /// Re-apply placements + explode to the current result (display only).
    fn reapply_display(&mut self, cx: &mut Context<Self>) {
        let view = self.explode.and_then(|i| self.assembly(cx).and_then(|a| a.exploded_views.get(i).cloned()));
        let r = Arc::make_mut(&mut self.result);
        let solved = r.solved.clone();
        r.apply_placements(solved, view.as_ref());
        self.rebuild_scene(true, cx);
        cx.notify();
    }

    fn save_named_position(&mut self, name: String, cx: &mut Context<Self>) {
        let placements = self.result.solved.placements.clone();
        self.edit(&format!("Insert : Named position {name}"), cx, |a| {
            a.named_positions.retain(|n| n.name != name);
            a.named_positions.push(NamedPosition { name, placements });
        });
    }

    fn apply_named_position(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(np) = self.assembly(cx).and_then(|a| a.named_positions.get(i).cloned()) else { return };
        let tab = self.tab;
        self.doc.update(cx, |doc, cx| {
            doc.edit_tab(tab, &format!("Apply named position : {}", np.name), cx, |c| {
                if let TabContent::Assembly(a) = c {
                    for (id, p) in &np.placements {
                        if let Some(inst) = a.instance_mut(*id) {
                            inst.placement = *p;
                        }
                    }
                }
            });
        });
    }

    // ------------------------------------------------------------ animation

    fn start_animation(&mut self, cx: &mut Context<Self>) {
        let Some(asm) = self.assembly(cx) else { return };
        let pick = self.selected_mate.and_then(|id| asm.mate(id)).filter(|m| matches!(m.mate_type, MateType::Revolute | MateType::Slider | MateType::Cylindrical)).or_else(|| {
            asm.mates.iter().find(|m| !m.suppressed && matches!(m.mate_type, MateType::Revolute | MateType::Slider | MateType::Cylindrical))
        });
        let Some(m) = pick.cloned() else {
            self.message = Some((false, "Animate needs a revolute, slider or cylindrical mate (select it in the list).".into()));
            cx.notify();
            return;
        };
        let angular = m.mate_type != MateType::Slider;
        let units = self.units(cx);
        let key = if angular { "rz" } else { "z" };
        let lim = m.limits.iter().find(|(k, _)| k == key).map(|(_, l)| l.clone());
        let dim = if angular { noshape_units::Dim::ANGLE } else { noshape_units::Dim::LENGTH };
        let ev = |e: &Option<ExprValue>| e.as_ref().and_then(|e| noshape_units::evaluate_as(&e.text, dim, &units, &noshape_units::EmptyScope).ok()).map(|q| q.value);
        let cur = self.result.solved.mate_values.get(&m.id).copied().unwrap_or([0.0; 4]);
        let cur_v = if angular { cur[0] } else { cur[3] };
        let size = self.result.bounds().map(|(a, b)| (b - a).length()).unwrap_or(0.1);
        let (start, end) = match (lim.as_ref().and_then(|l| ev(&l.min)), lim.as_ref().and_then(|l| ev(&l.max))) {
            (Some(a), Some(b)) => (a, b),
            _ if angular => (cur_v, cur_v + std::f64::consts::TAU),
            _ => (cur_v, cur_v + size * 0.5),
        };
        self.selected_mate = Some(m.id);
        self.anim = Some(AnimState { mate: m.id, start, end, t: 0.0, dir: 1.0, playing: false, playback: if angular && lim.is_none() { Playback::Loop } else { Playback::Reciprocate }, angular, busy: false, task: None });
        self.play_animation(cx);
    }

    fn play_animation(&mut self, cx: &mut Context<Self>) {
        let Some(a) = &mut self.anim else { return };
        a.playing = true;
        a.task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(33)).await;
                let job = this.update(cx, |this: &mut Self, _| {
                    let a = this.anim.as_mut()?;
                    if !a.playing || a.busy {
                        return a.playing.then_some(None);
                    }
                    let step = 1.0 / 90.0;
                    a.t += a.dir * step;
                    if a.t > 1.0 || a.t < 0.0 {
                        match a.playback {
                            Playback::Loop => a.t = a.t.rem_euclid(1.0),
                            Playback::Reciprocate => {
                                a.dir = -a.dir;
                                a.t = a.t.clamp(0.0, 1.0);
                            }
                        }
                    }
                    let mut v = a.start + (a.end - a.start) * a.t;
                    if a.angular {
                        v = wrap_angle(v);
                    }
                    a.busy = true;
                    let start: HashMap<InstanceId, Placement> = this.result.solved.placements.iter().cloned().collect();
                    Some(Some((this.result.prepared.clone(), SolveOptions { drive: Some((a.mate, v)), start: Some(start), ..Default::default() })))
                });
                let Ok(Some(job)) = job else { break };
                let Some((prepared, opts)) = job else { continue };
                let solved = cx.background_spawn(async move { prepared.solve(&opts) }).await;
                let ok = this.update(cx, |this: &mut Self, cx| {
                    if let Some(a) = &mut this.anim {
                        a.busy = false;
                    }
                    let view = this.explode.and_then(|i| this.assembly(cx).and_then(|a| a.exploded_views.get(i).cloned()));
                    Arc::make_mut(&mut this.result).apply_placements(solved, view.as_ref());
                    this.rebuild_scene(true, cx);
                    cx.notify();
                });
                if ok.is_err() {
                    break;
                }
            }
        }));
        cx.notify();
    }

    fn stop_animation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(a) = &mut self.anim {
            a.playing = false;
            a.task = None;
        }
        cx.notify();
        let _ = window;
    }

    fn close_animation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.anim = None;
        // Return to the document's positions.
        self.schedule_eval(window, cx);
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

    fn update_inference(&mut self, cx: &mut Context<Self>) -> bool {
        let awaiting = self.mate.as_ref().is_some_and(|s| s.dialog.read(cx).awaiting_pick());
        let old = self.infer_choice;
        if !awaiting {
            self.infer_choice = None;
            return old.is_some();
        }
        let Some(h) = self.hover.clone() else {
            self.infer_choice = None;
            return old.is_some();
        };
        let key = (h.path.clone(), h.reference.clone());
        if self.infer.as_ref().is_none_or(|(k, _)| *k != key) {
            let units = self.units(cx);
            let cands = self.result.occurrences.get(h.occ).map(|o| inference_candidates(&o.context, Some(&o.body), &h.reference, &units)).unwrap_or_default();
            self.infer = Some((key, cands));
            self.infer_choice = None;
        }
        let Some((_, cands)) = &self.infer else { return false };
        let Some(o) = self.result.occurrences.get(h.occ) else { return false };
        let vp = self.viewport.read(cx);
        let mut best: Option<(usize, f64)> = None;
        for (i, (_, f)) in cands.iter().enumerate() {
            let w = o.transform.transform_point3(f.origin);
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
        let capture = self.mate.is_none() && self.anim.is_none() && (self.hover.is_some() || self.manip_hover.is_some());
        self.viewport.update(cx, |vp, _| vp.tool_captures_left = capture);
    }

    fn on_viewport_event(&mut self, _: &Entity<Viewport>, ev: &ViewportEvent, window: &mut Window, cx: &mut Context<Self>) {
        match ev {
            ViewportEvent::Hover(id) => {
                if self.drag.as_ref().is_some_and(|d| d.started) {
                    return;
                }
                let manip = id.and_then(|id| self.built.as_ref().and_then(|b| assembly_scene::manip_axis(b, id)));
                let manip_changed = manip != self.manip_hover;
                self.manip_hover = manip;
                let t = if manip.is_some() { None } else { self.pick(*id) };
                let changed = manip_changed || t.as_ref().map(|t| (&t.path, &t.reference)) != self.hover.as_ref().map(|h| (&h.path, &h.reference));
                self.hover = t;
                self.update_capture(cx);
                let inf = self.update_inference(cx);
                if changed || inf {
                    self.refresh_scene(false, cx);
                }
            }
            ViewportEvent::PointerMove { position, .. } => {
                self.pointer = *position;
                if self.mate.is_some() {
                    let before = self.infer_choice;
                    self.update_inference(cx);
                    if before != self.infer_choice {
                        self.refresh_scene(false, cx);
                    }
                }
            }
            ViewportEvent::Click { pick, position, modifiers, .. } => {
                self.context_menu = None;
                self.pointer = *position;
                let target = self.pick(*pick);
                if let Some(s) = &self.mate {
                    if s.dialog.read(cx).awaiting_pick() {
                        if let Some(t) = &target {
                            self.hover = Some(t.clone());
                            self.update_inference(cx);
                        }
                        let chosen = self.infer.as_ref().zip(self.infer_choice).and_then(|((k, c), i)| c.get(i).map(|x| (k.0.clone(), x.0.clone())));
                        if let (Some(t), Some((path, r))) = (target, chosen)
                            && path == t.path
                        {
                            let c = MateConnectorRef { instance: path[0], path: path[1..].to_vec(), reference: r, flip: false, rotate_quarters: 0 };
                            s.dialog.clone().update(cx, |d, cx| d.set_connector(c, cx));
                        }
                    }
                } else {
                    self.select_target(target, modifiers.shift || modifiers.secondary(), cx);
                }
                self.refresh_scene(false, cx);
                cx.notify();
            }
            ViewportEvent::ToolDown { position, .. } => {
                self.context_menu = None;
                if let (Some(k), Some(f)) = (self.manip_hover, self.manipulator(cx)) {
                    let top = self.selected[0];
                    let axis = [f.x(), f.y(), f.z()][k];
                    let pose = self.result.solved.placement(top).map(|p| placement_to_frame(&p)).unwrap_or_default();
                    let local = pose.inverse().transform_point(f.origin);
                    self.drag = Some(DragState { top, press: *position, started: false, local, plane_point: f.origin, plane_normal: axis, pending: None, busy: false, axis: Some(axis) });
                    return;
                }
                let Some(t) = self.hover.clone() else { return };
                let top = t.path[0];
                let (origin, dir) = self.viewport.read(cx).ray(*position);
                // Grab point: closest point on the ray to the instance's bounds center.
                let center = self
                    .result
                    .occurrences
                    .iter()
                    .filter(|o| o.top() == top)
                    .filter_map(|o| o.world_bounds())
                    .fold(None, |acc: Option<(DVec3, DVec3)>, (a, b)| Some(acc.map(|(x, y)| (x.min(a), y.max(b))).unwrap_or((a, b))))
                    .map(|(a, b)| (a + b) * 0.5)
                    .unwrap_or(DVec3::ZERO);
                let tt = (center - origin).dot(dir);
                let grab = origin + dir * tt;
                let pose = self.result.solved.placement(top).map(|p| placement_to_frame(&p)).unwrap_or_default();
                let local = pose.inverse().transform_point(grab);
                let normal = self.viewport.read(cx).camera.forward();
                self.drag = Some(DragState { top, press: *position, started: false, local, plane_point: grab, plane_normal: normal, pending: None, busy: false, axis: None });
            }
            ViewportEvent::ToolDrag { position, .. } => {
                let Some(d) = &mut self.drag else { return };
                if !d.started && position.distance(d.press) > 3.0 {
                    d.started = true;
                }
                if !d.started {
                    return;
                }
                let (origin, dir) = self.viewport.read(cx).ray(*position);
                if let Some(axis) = d.axis {
                    // Closest point on the axis line to the cursor ray.
                    let w = d.plane_point - origin;
                    let (a, b, c) = (axis.dot(axis), axis.dot(dir), dir.dot(dir));
                    let (dd, e) = (axis.dot(w), dir.dot(w));
                    let den = a * c - b * b;
                    if den.abs() < 1e-12 {
                        return;
                    }
                    let s = (b * e - c * dd) / den;
                    d.pending = Some(d.plane_point + axis * s);
                } else {
                    let denom = dir.dot(d.plane_normal);
                    if denom.abs() < 1e-9 {
                        return;
                    }
                    let t = (d.plane_point - origin).dot(d.plane_normal) / denom;
                    d.pending = Some(origin + dir * t);
                }
                if !d.busy {
                    self.run_drag_solve(cx);
                }
            }
            ViewportEvent::ToolUp { modifiers, .. } => {
                let Some(d) = self.drag.take() else { return };
                if d.started {
                    let name = self.assembly(cx).and_then(|a| a.instance(d.top).map(|i| i.name.clone())).unwrap_or_default();
                    let fixed = self.assembly(cx).and_then(|a| a.instance(d.top)).is_some_and(|i| i.fixed);
                    if fixed {
                        self.message = Some((false, format!("{name} is fixed and cannot be dragged.")));
                    } else {
                        self.edit(&format!("Move : {name}"), cx, |_| {});
                    }
                } else if d.axis.is_none() {
                    let t = self.hover.clone();
                    self.select_target(t, modifiers.shift || modifiers.secondary(), cx);
                    self.refresh_scene(false, cx);
                }
                cx.notify();
            }
            ViewportEvent::ContextMenu { pick, position } => {
                match self.pick(*pick) {
                    Some(t) => {
                        let top = t.path[0];
                        if !self.selected.contains(&top) {
                            self.selected = vec![top];
                        }
                        self.context_menu = Some((*position, top));
                        self.refresh_scene(false, cx);
                    }
                    None => self.context_menu = None,
                }
                cx.notify();
            }
            _ => {}
        }
        let _ = window;
    }

    fn select_target(&mut self, target: Option<AsmPickTarget>, additive: bool, _cx: &mut Context<Self>) {
        self.selected_mate = None;
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
    }

    fn run_drag_solve(&mut self, cx: &mut Context<Self>) {
        let Some(d) = &mut self.drag else { return };
        let Some(target) = d.pending.take() else { return };
        d.busy = true;
        let prepared = self.result.prepared.clone();
        let start: HashMap<InstanceId, Placement> = self.result.solved.placements.iter().cloned().collect();
        let opts = SolveOptions { drags: vec![(d.top, d.local, target)], start: Some(start), ..Default::default() };
        cx.spawn(async move |this, cx| {
            let solved = cx.background_spawn(async move { prepared.solve(&opts) }).await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                let Some(d) = &mut this.drag else { return };
                d.busy = false;
                let again = d.pending.is_some();
                let view = this.explode.and_then(|i| this.assembly(cx).and_then(|a| a.exploded_views.get(i).cloned()));
                Arc::make_mut(&mut this.result).apply_placements(solved, view.as_ref());
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
        let pal = p();
        Button::new(id).icon(Icon::empty().path(icon_path(icon)).text_color(pal.text)).ghost().small().tooltip(tip).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| f(this, window, cx)))
    }

    fn render_toolbar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let sep = || div().w(px(1.)).h(px(22.)).mx_1().bg(pal.border);
        let view = cx.entity();
        let mate_more = {
            let view = view.clone();
            Button::new("tb-mate-more").icon(Icon::new(IconName::ChevronDown).xsmall()).ghost().xsmall().w(px(14.)).dropdown_menu(move |mut menu, _, _| {
                for t in MateType::ALL {
                    let v = view.clone();
                    menu = menu.item(PopupMenuItem::new(format!("{} mate", t.name())).icon(Icon::empty().path(mate_icon(t))).on_click(move |_, window, cx| v.update(cx, |this, cx| this.start_mate(t, window, cx))));
                }
                menu
            })
        };
        let relations = {
            let view = view.clone();
            Button::new("tb-relations").icon(Icon::empty().path(icon_path("gear-relation")).text_color(pal.text)).ghost().small().tooltip("Relations").dropdown_menu(move |mut menu, _, _| {
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
        let positions: Vec<String> = self.assembly(cx).map(|a| a.named_positions.iter().map(|n| n.name.clone()).collect()).unwrap_or_default();
        let named = {
            let view = view.clone();
            Button::new("tb-named").icon(Icon::empty().path(icon_path("named-position")).text_color(pal.text)).ghost().small().tooltip("Named positions").dropdown_menu(move |mut menu, _, _| {
                let v = view.clone();
                menu = menu.item(PopupMenuItem::new("Save current position…").icon(IconName::Plus).on_click(move |_, window, cx| {
                    v.update(cx, |this, cx| {
                        let n = this.assembly(cx).map(|a| a.named_positions.len() + 1).unwrap_or(1);
                        let st = cx.new(|cx| InputState::new(window, cx).default_value(format!("Position {n}")));
                        st.update(cx, |s, cx| s.focus(window, cx));
                        this.naming_position = Some(st);
                        cx.notify();
                    })
                }));
                if !positions.is_empty() {
                    menu = menu.separator();
                }
                for (i, name) in positions.iter().enumerate() {
                    let v = view.clone();
                    menu = menu.item(PopupMenuItem::new(format!("Apply {name}")).on_click(move |_, _, cx| v.update(cx, |this, cx| this.apply_named_position(i, cx))));
                }
                menu
            })
        };
        let exploded = self.explode.is_some();
        h_flex()
            .h(px(40.))
            .px_2()
            .gap_1()
            .bg(pal.panel)
            .border_b_1()
            .border_color(pal.border)
            .overflow_hidden()
            .child(self.tool("tb-insert", "insert", "Insert parts and assemblies (I)", cx, |this, _, cx| {
                this.insert_open = !this.insert_open;
                cx.notify();
            }).selected(self.insert_open))
            .child(sep())
            .child(self.tool("tb-mc", "mate-connector-assembly", "Show mate connectors (K)", cx, |this, _, cx| {
                this.show_connectors = !this.show_connectors;
                this.refresh_scene(false, cx);
                cx.notify();
            }).selected(self.show_connectors))
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
            .child(sep())
            .child(self.tool("tb-explode", "explode", "Exploded view", cx, |this, _, cx| this.toggle_explode(cx)).selected(exploded))
            .child(named)
            .child(self.tool("tb-animate", "animate", "Animate mate", cx, |this, _, cx| this.start_animation(cx)).selected(self.anim.is_some()))
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
        let dof = if is_top { node.and_then(|n| n.dof) } else { None };
        let selected = is_top && self.selected.contains(&top);
        let has_children = node.is_some_and(|n| !n.children.is_empty());
        let expanded = self.expanded.contains(&path);
        let icon = match kind {
            NodeKind::Part => "part",
            NodeKind::PartStudio => "part-studio",
            NodeKind::Assembly => "assembly",
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
            .h(px(24.))
            .pl(px(6. + depth as f32 * 14.))
            .pr_1()
            .gap_1p5()
            .rounded_sm()
            .text_sm()
            .text_color(color)
            .when(selected, |d| d.bg(pal.accent_soft))
            .hover(|d| d.bg(pal.panel_hover))
            .child(
                div()
                    .id(SharedString::from(format!("exp-{}", path.iter().map(|i| i.0.to_string()).collect::<Vec<_>>().join("-"))))
                    .w(px(12.))
                    .when(has_children, |d| {
                        d.child(svg().path(icon_path(if expanded { "chevron-down" } else { "chevron-right" })).size_3().text_color(pal.text_faint)).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if !this.expanded.remove(&p2) {
                                this.expanded.insert(p2.clone());
                            }
                            cx.stop_propagation();
                            cx.notify();
                        }))
                    }),
            )
            .child(svg().path(icon_path(icon)).size_3p5().text_color(if suppressed { pal.text_faint } else { pal.text_muted }))
            .child(div().flex_1().truncate().when(suppressed, |d| d.line_through()).child(name.clone()))
            .when(fixed, |d| d.child(svg().path(icon_path("fix")).size_3().text_color(pal.text_muted)))
            .when_some(dof.filter(|d| *d > 0 && !fixed), |d, n| {
                d.child(div().id(SharedString::from(format!("dof-{}", top.0))).text_xs().text_color(pal.info).child(format!("{n}")).tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(format!("Has {n} degrees of freedom")).build(w, cx)))
            })
            .when_some(error.clone(), |d, e| d.child(div().id(SharedString::from(format!("err-{}", top.0))).size(px(6.)).rounded_full().bg(pal.error).tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(e.clone()).build(w, cx))))
            .when(is_top, |d| {
                d.child(
                    Button::new(SharedString::from(format!("eye-{}", top.0)))
                        .icon(Icon::empty().path(icon_path(if hidden { "eye-off" } else { "eye" })))
                        .ghost()
                        .xsmall()
                        .tooltip(if hidden { "Show" } else { "Hide" })
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_hidden(vec![top], !hidden, cx))),
                )
            })
            .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
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
        let view = cx.entity();
        row.context_menu(move |menu, _, _| {
            let (v1, v2, v3, v4, v5) = (view.clone(), view.clone(), view.clone(), view.clone(), view.clone());
            menu.item(PopupMenuItem::new(if fixed { "Unfix" } else { "Fix" }).on_click(move |_, _, cx| v1.update(cx, |t, cx| {
                let ids = t.targets(Some(top));
                t.set_fixed(ids, !fixed, cx)
            })))
            .item(PopupMenuItem::new(if hidden { "Show" } else { "Hide" }).on_click(move |_, _, cx| v2.update(cx, |t, cx| {
                let ids = t.targets(Some(top));
                t.set_hidden(ids, !hidden, cx)
            })))
            .item(PopupMenuItem::new(if suppressed { "Unsuppress" } else { "Suppress" }).on_click(move |_, _, cx| v3.update(cx, |t, cx| {
                let ids = t.targets(Some(top));
                t.set_suppressed(ids, !suppressed, cx)
            })))
            .item(PopupMenuItem::new("Isolate").on_click(move |_, _, cx| v4.update(cx, |t, cx| {
                let ids = t.targets(Some(top));
                t.isolate(ids, cx)
            })))
            .separator()
            .item(PopupMenuItem::new("Delete").on_click(move |_, _, cx| v5.update(cx, |t, cx| {
                let ids = t.targets(Some(top));
                t.delete_instances(ids, cx)
            })))
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

    fn feature_row(&self, id: MateId, icon: SharedString, name: String, ok: bool, inactive: bool, message: Option<String>, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let selected = self.selected_mate == Some(id);
        let color = if inactive {
            pal.text_faint
        } else if !ok {
            pal.error
        } else {
            pal.text
        };
        let row = h_flex()
            .id(SharedString::from(format!("mate-{}", id.0)))
            .h(px(24.))
            .px_2()
            .gap_2()
            .rounded_sm()
            .text_sm()
            .text_color(color)
            .when(selected, |d| d.bg(pal.accent_soft))
            .hover(|d| d.bg(pal.panel_hover))
            .child(svg().path(icon).size_3p5().text_color(if ok { pal.text_muted } else { pal.error }))
            .child(div().flex_1().truncate().when(inactive, |d| d.line_through()).child(name))
            .when(!ok && !inactive, |d| d.child(div().size(px(6.)).rounded_full().bg(pal.error)))
            .when_some(message, |d, m| d.tooltip(move |w, cx| gpui_kit::component::tooltip::Tooltip::new(m.clone()).build(w, cx)))
            .on_hover(cx.listener(move |this, h: &bool, _, cx| {
                let new = if *h { Some(id) } else { None };
                if this.list_hover_mate != new {
                    this.list_hover_mate = new;
                    this.refresh_scene(false, cx);
                }
            }))
            .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                if ev.click_count() >= 2 {
                    this.edit_mate(id, window, cx);
                    return;
                }
                this.selected_mate = if this.selected_mate == Some(id) { None } else { Some(id) };
                this.selected.clear();
                this.refresh_scene(false, cx);
                cx.notify();
            }));
        let view = cx.entity();
        row.context_menu(move |menu, _, _| {
            let (v1, v2, v3, v4) = (view.clone(), view.clone(), view.clone(), view.clone());
            menu.item(PopupMenuItem::new("Edit…").on_click(move |_, window, cx| v1.update(cx, |t, cx| t.edit_mate(id, window, cx))))
                .item(PopupMenuItem::new("Animate…").on_click(move |_, _, cx| v2.update(cx, |t, cx| {
                    t.selected_mate = Some(id);
                    t.start_animation(cx)
                })))
                .item(PopupMenuItem::new(if inactive { "Unsuppress" } else { "Suppress" }).on_click(move |_, _, cx| v3.update(cx, |t, cx| t.toggle_mate_suppressed(id, cx))))
                .separator()
                .item(PopupMenuItem::new("Delete").on_click(move |_, _, cx| v4.update(cx, |t, cx| t.delete_mate(id, cx))))
        })
        .into_any_element()
    }

    fn render_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(asm) = self.assembly(cx).cloned() else { return div().into_any_element() };
        let result = self.result.clone();
        let mut rows: Vec<AnyElement> = Vec::new();
        for inst in &asm.instances {
            let node = result.node(&[inst.id]);
            rows.push(self.instance_row(node, Some(inst), vec![inst.id], 0, cx));
            if let Some(n) = node {
                self.push_children(n, 1, &mut rows, cx);
            }
        }
        if asm.instances.is_empty() {
            rows.push(div().px_3().py_2().text_xs().text_color(pal.text_faint).child("Insert parts and assemblies (I) to start.").into_any_element());
        }
        let mut mate_rows: Vec<AnyElement> = Vec::new();
        for m in &asm.mates {
            let st = result.mates.iter().find(|s| s.id == m.id);
            let ok = st.is_none_or(|s| s.ok);
            let inactive = m.suppressed || st.is_some_and(|s| s.inactive);
            mate_rows.push(self.feature_row(m.id, mate_icon(m.mate_type), m.name.clone(), ok, inactive, st.and_then(|s| s.message.clone()), cx));
        }
        for r in &asm.relations {
            let st = result.relations.iter().find(|s| s.id == r.id);
            let icon = icon_path(match r.relation_type {
                RelationType::Gear => "gear-relation",
                RelationType::RackAndPinion => "rack-pinion",
                RelationType::Screw => "screw-relation",
                RelationType::Linear => "linear-relation",
            });
            mate_rows.push(self.feature_row(r.id, icon, r.name.clone(), st.is_none_or(|s| s.ok), r.suppressed, st.and_then(|s| s.message.clone()), cx));
        }
        for g in &asm.groups {
            mate_rows.push(self.feature_row(g.id, icon_path("group"), g.name.clone(), true, g.suppressed, None, cx));
        }
        let dof = result.solved.dof;
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
                    .child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text_muted).child(format!("INSTANCES ({})", asm.instances.len())))
                    .child(div().text_xs().text_color(pal.text_faint).child(if dof > 0 { format!("{dof} DOF") } else { String::new() })),
            )
            .child(v_flex().id("instance-list").flex_1().min_h_0().p_1().gap_px().overflow_y_scroll().children(rows))
            .child(
                v_flex()
                    .max_h(px(280.))
                    .border_t_1()
                    .border_color(pal.border)
                    .child(h_flex().h(px(28.)).px_3().child(div().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text_muted).child(format!("MATE FEATURES ({})", asm.mates.len() + asm.relations.len() + asm.groups.len()))))
                    .child(v_flex().id("mate-list").p_1().gap_px().overflow_y_scroll().children(mate_rows)),
            )
            .into_any_element()
    }

    // ------------------------------------------------------------ render: panels

    fn panel_frame(title: &str, body: impl IntoElement, close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static, id: &'static str) -> Div {
        let pal = p();
        v_flex()
            .w(px(300.))
            .max_h(px(560.))
            .rounded_lg()
            .border_1()
            .border_color(pal.border_strong)
            .bg(pal.panel_raised)
            .shadow_lg()
            .text_color(pal.text)
            .child(
                h_flex()
                    .px_3()
                    .py_2()
                    .gap_2()
                    .border_b_1()
                    .border_color(pal.border)
                    .child(div().flex_1().text_sm().font_weight(FontWeight::SEMIBOLD).child(title.to_string()))
                    .child(Button::new(id).icon(IconName::Close).ghost().xsmall().on_click(close)),
            )
            .child(body)
    }

    fn render_insert_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let mut body = v_flex().id("insert-body").p_2().gap_1().overflow_y_scroll().max_h(px(480.));
        body = body.child(Checkbox::new("insert-rigid").label("Insert entire Part Studio as rigid").checked(self.insert_rigid).on_click(cx.listener(|this, v: &bool, _, cx| {
            this.insert_rigid = *v;
            cx.notify();
        })));
        body = body.child(div().pt_1().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text_muted).child("PART STUDIOS"));
        for s in self.catalog.studios.clone() {
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
                    .child(svg().path(icon_path("part-studio")).size_4().text_color(pal.accent))
                    .child(div().flex_1().text_sm().truncate().child(s.name.clone()))
                    .child(div().text_xs().text_color(pal.text_faint).child(if self.insert_rigid { "rigid".to_string() } else { format!("{} parts", s.parts.len()) }))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.insert_studio_parts(s2.clone(), cx))),
            );
            if !self.insert_rigid {
                for part in &s.parts {
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
                            .child(svg().path(icon_path("part")).size_3p5().text_color(pal.text_muted))
                            .child(div().flex_1().truncate().child(part.1.clone()))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.insert_part(tab, Some(part2.clone()), cx))),
                    );
                }
            }
        }
        body = body.child(div().pt_2().text_xs().font_weight(FontWeight::SEMIBOLD).text_color(pal.text_muted).child("ASSEMBLIES"));
        if self.catalog.assemblies.is_empty() {
            body = body.child(div().px_2().text_xs().text_color(pal.text_faint).child("No other assemblies can be inserted."));
        }
        for (tab, name) in self.catalog.assemblies.clone() {
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
                    .child(svg().path(icon_path("assembly")).size_4().text_color(pal.accent))
                    .child(div().flex_1().text_sm().truncate().child(name))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.insert_assembly(tab, n2.clone(), cx))),
            );
        }
        Self::panel_frame("Insert parts and assemblies", body, cx.listener(|this, _: &ClickEvent, _, cx| {
            this.insert_open = false;
            cx.notify();
        }), "insert-close")
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

    fn render_anim_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(a) = &self.anim else { return div().into_any_element() };
        let name = self.assembly(cx).and_then(|x| x.mate(a.mate).map(|m| m.name.clone())).unwrap_or_default();
        let units = self.units(cx);
        let fmt = |v: f64| {
            let q = if a.angular { noshape_units::Quantity::radians(v) } else { noshape_units::Quantity::meters(v) };
            noshape_units::format_quantity(q, &units)
        };
        let cur = a.start + (a.end - a.start) * a.t;
        let playing = a.playing;
        let pb = a.playback;
        let body = v_flex()
            .p_3()
            .gap_2()
            .child(div().text_xs().text_color(pal.text_muted).child(format!("From {} to {}", fmt(a.start), fmt(a.end))))
            .child(div().text_sm().child(format!("Current: {}", fmt(cur))))
            .child(
                h_flex()
                    .gap_1()
                    .child(Button::new("anim-play").icon(if playing { IconName::Pause } else { IconName::Play }).small().ghost().label(if playing { "Stop" } else { "Play" }).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        if playing {
                            this.stop_animation(window, cx);
                        } else {
                            this.play_animation(cx);
                        }
                    })))
                    .child(Button::new("anim-loop").label("Loop").xsmall().ghost().selected(pb == Playback::Loop).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if let Some(a) = &mut this.anim {
                            a.playback = Playback::Loop;
                        }
                        cx.notify();
                    })))
                    .child(Button::new("anim-recip").label("Reciprocate").xsmall().ghost().selected(pb == Playback::Reciprocate).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if let Some(a) = &mut this.anim {
                            a.playback = Playback::Reciprocate;
                        }
                        cx.notify();
                    }))),
            );
        Self::panel_frame(&format!("Animate {name}"), body, cx.listener(|this, _: &ClickEvent, window, cx| this.close_animation(window, cx)), "anim-close").into_any_element()
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
                            .child(div().flex_1().truncate().child(r.name.clone()))
                            .child(div().w(px(90.)).truncate().text_xs().text_color(pal.text_muted).child(studio_name(r.studio)))
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
                        let vol = it.volume.cbrt();
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
                                .child(div().text_xs().text_color(pal.text_muted).child(format!("Volume ≈ ({})³", noshape_units::format_quantity(noshape_units::Quantity::meters(vol), &units))))
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
            None => div().into_any_element(),
        }
    }

    fn render_context_menu(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let pal = p();
        let (pos, top) = self.context_menu?;
        let inst = self.assembly(cx)?.instance(top)?.clone();
        let item = |id: &'static str, label: String, cx: &mut Context<Self>, f: Box<dyn Fn(&mut Self, &mut Context<Self>)>| {
            h_flex()
                .id(id)
                .h(px(26.))
                .px_3()
                .rounded_sm()
                .text_sm()
                .cursor_pointer()
                .hover(|d| d.bg(pal.panel_hover))
                .child(label)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.context_menu = None;
                    f(this, cx);
                    cx.notify();
                }))
        };
        let (fixed, hidden, suppressed) = (inst.fixed, inst.hidden, inst.suppressed);
        let menu = v_flex()
            .id("asm-ctx")
            .w(px(180.))
            .p_1()
            .rounded_md()
            .border_1()
            .border_color(pal.border_strong)
            .bg(pal.panel_raised)
            .shadow_lg()
            .text_color(pal.text)
            .child(div().px_3().py_1().text_xs().text_color(pal.text_faint).child(inst.name.clone()))
            .child(item("ctx-fix", (if fixed { "Unfix" } else { "Fix" }).into(), cx, Box::new(move |t, cx| {
                let ids = t.targets(Some(top));
                t.set_fixed(ids, !fixed, cx)
            })))
            .child(item("ctx-hide", (if hidden { "Show" } else { "Hide" }).into(), cx, Box::new(move |t, cx| {
                let ids = t.targets(Some(top));
                t.set_hidden(ids, !hidden, cx)
            })))
            .child(item("ctx-supp", (if suppressed { "Unsuppress" } else { "Suppress" }).into(), cx, Box::new(move |t, cx| {
                let ids = t.targets(Some(top));
                t.set_suppressed(ids, !suppressed, cx)
            })))
            .child(item("ctx-iso", (if self.isolate.is_some() { "Exit isolate" } else { "Isolate" }).into(), cx, Box::new(move |t, cx| {
                let ids = t.targets(Some(top));
                t.isolate(ids, cx)
            })))
            .child(div().h(px(1.)).my_1().bg(pal.border))
            .child(item("ctx-del", "Delete".into(), cx, Box::new(move |t, cx| {
                let ids = t.targets(Some(top));
                t.delete_instances(ids, cx)
            })));
        Some(deferred(anchored().position(pos).snap_to_window().child(menu)).with_priority(3).into_any_element())
    }

    fn render_overlay(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let pal = p();
        let mut out = Vec::new();
        // Left column: insert panel / mate dialog / relation dialog / animation.
        let mut left = v_flex().absolute().top_3().left_3().gap_2();
        let mut any_left = false;
        if self.insert_open {
            left = left.child(self.render_insert_panel(cx));
            any_left = true;
        }
        if let Some(s) = &self.mate {
            left = left.child(s.dialog.clone());
            any_left = true;
        }
        if self.relation.is_some() {
            left = left.child(self.render_relation_panel(cx));
            any_left = true;
        }
        if self.anim.is_some() {
            left = left.child(self.render_anim_panel(cx));
            any_left = true;
        }
        if let Some(st) = self.naming_position.clone() {
            left = left.child(Self::panel_frame(
                "Save named position",
                v_flex().p_3().gap_2().child(Input::new(&st).small()).child(h_flex().justify_end().child(Button::new("np-ok").label("Save").primary().small().on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    if let Some(st) = this.naming_position.take() {
                        let name = st.read(cx).value().to_string();
                        if !name.trim().is_empty() {
                            this.save_named_position(name, cx);
                        }
                    }
                    cx.notify();
                })))),
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.naming_position = None;
                    cx.notify();
                }),
                "np-close",
            ));
            any_left = true;
        }
        if any_left {
            out.push(left.into_any_element());
        }
        if self.panel.is_some() {
            out.push(div().absolute().top_3().right(px(56.)).child(self.render_side_panel(cx)).into_any_element());
        }
        out.push(self.render_view_controls(cx));
        if self.explode.is_some() {
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
        if let Some(m) = self.render_context_menu(cx) {
            out.push(m);
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
            .child(btn("av-iso", "iso-view", "Isometric (Shift+7)", StandardView::Isometric, cx))
            .child(btn("av-front", "front-view", "Front (Shift+1)", StandardView::Front, cx))
            .child(btn("av-top", "top-view", "Top (Shift+5)", StandardView::Top, cx))
            .child(btn("av-right", "right-view", "Right (Shift+4)", StandardView::Right, cx))
            .child(Button::new("av-fit").icon(Icon::empty().path(icon_path("zoom-fit"))).ghost().xsmall().tooltip("Zoom to fit (F)").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.viewport.update(cx, |vp, cx| vp.zoom_to_fit(true, cx));
            })))
            .child(Button::new("av-mates").icon(Icon::empty().path(icon_path("fastened"))).ghost().xsmall().selected(self.show_mates).tooltip("Show mates (J)").on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                this.show_mates = !this.show_mates;
                this.refresh_scene(false, cx);
                cx.notify();
            })))
            .into_any_element()
    }
}

impl Render for AssemblyView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let toolbar = self.render_toolbar(cx);
        let list = self.render_list(cx);
        let overlay = self.render_overlay(cx);
        v_flex()
            .size_full()
            .key_context("Assembly")
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _: &InsertInstance, _, cx| {
                this.insert_open = !this.insert_open;
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &FastenMate, window, cx| this.start_mate(MateType::Fastened, window, cx)))
            .on_action(cx.listener(|this, _: &ToggleMates, _, cx| {
                this.show_mates = !this.show_mates;
                this.refresh_scene(false, cx);
                cx.notify();
            }))
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
            .on_action(cx.listener(|this, _: &HideSelected, _, cx| {
                let ids = this.selected.clone();
                this.set_hidden(ids, true, cx);
            }))
            .on_action(cx.listener(|this, _: &ShowAll, _, cx| {
                let ids: Vec<InstanceId> = this.assembly(cx).map(|a| a.instances.iter().filter(|i| i.hidden).map(|i| i.id).collect()).unwrap_or_default();
                if !ids.is_empty() {
                    this.set_hidden(ids, false, cx);
                }
            }))
            .on_action(cx.listener(|this, _: &IsolateSelected, _, cx| {
                let ids = this.selected.clone();
                this.isolate(ids, cx);
            }))
            .on_action(cx.listener(|this, _: &ClearSelection, _, cx| {
                this.selected.clear();
                this.selected_mate = None;
                this.refresh_scene(false, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Cancel, window, cx| {
                if this.mate.is_some() {
                    this.close_mate(window, cx);
                } else if this.context_menu.is_some() {
                    this.context_menu = None;
                    cx.notify();
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
