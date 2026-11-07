//! Opening an assembly: its cached evaluation (`crate::display_cache`) shown at once while the real one runs, view
//! only until that lands (lead 11-06: "Regenerating…" at the top of the instance list, a dialog asked for waits with a
//! message and opens then, a drag says so), the real result swapped in when it is applied (its scene is rebuilt in
//! that same update: no frame between). `NOSHAPE_PERF`'s open milestones (the frame that first draws the model, as
//! the Part Studio's: tools/perf/open-times.py) and `NOSHAPE_PERF_SCRIPT=open`'s quit once it is up.

use super::AssemblyView;
use crate::display_cache::{self as cache, WAITING};
use glam::{DMat4, DQuat, DVec3};
use gpui_kit::*;
use noshape_model::assembly::eval::{AssemblyResult, ConnectorStatus, InstanceNode, MateStatus, NodeKind, PartOccurrence, RelationStatus, Solved};
use noshape_model::assembly::solver::Frame;
use noshape_model::assembly::{Assembly, InstanceId, MateId, Placement};
use noshape_model::configuration::ConfigValues;
use noshape_model::display::{BodyKind, MeshBuffers};
use noshape_model::document::TabId;
use noshape_model::reference::{BodyId, Reference};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// An action that waits for the real evaluation.
pub(super) type Deferred = Box<dyn FnOnce(&mut AssemblyView, &mut Window, &mut Context<AssemblyView>)>;

/// How long after an evaluation the cache is written (edits in a row write once).
const SAVE_AFTER: Duration = Duration::from_secs(2);

/// What the cache keeps of an evaluation: what the graphics and the instance list show. Each mesh once (1000 pins
/// share one), the occurrences pointing at it; no OCCT context and no solver (view-only until the real one lands).
#[derive(Serialize, Deserialize)]
struct Snapshot {
    meshes: Vec<Arc<MeshBuffers>>,
    occurrences: Vec<Occurrence>,
    nodes: Vec<Node>,
    mates: Vec<Status>,
    relations: Vec<Status>,
    connectors: Vec<Connector>,
    pattern_errors: Vec<(MateId, String)>,
    placements: Vec<(InstanceId, Placement)>,
    converged: bool,
    dof: usize,
    instance_dof: Vec<(InstanceId, usize)>,
    mate_ok: Vec<(MateId, bool)>,
    relation_ok: Vec<(MateId, bool)>,
    nested: Vec<((InstanceId, InstanceId), Placement)>,
    explode_trails: Vec<(DVec3, DVec3)>,
}

#[derive(Serialize, Deserialize)]
struct Occurrence {
    path: Vec<InstanceId>,
    body: BodyId,
    studio: TabId,
    config: ConfigValues,
    instance_name: String,
    body_name: String,
    kind: BodyKind,
    color: [f32; 4],
    mesh: u32,
    coarser: Vec<u32>,
    levels_from: u8,
    inner: DMat4,
    transform: DMat4,
    face_refs: Vec<Reference>,
    edge_refs: Vec<Reference>,
    vertex_refs: Vec<Reference>,
    hidden: bool,
    local_bounds: Option<(DVec3, DVec3)>,
    mirrored: bool,
    external: Option<String>,
    flex: Option<(InstanceId, DMat4)>,
}

#[derive(Serialize, Deserialize)]
struct Node {
    path: Vec<InstanceId>,
    name: String,
    kind: u8,
    fixed: bool,
    hidden: bool,
    suppressed: bool,
    dof: Option<usize>,
    error: Option<String>,
    children: Vec<Node>,
    rigid: bool,
    /// A linked assembly's name and content (as JSON: document data's serde attributes need a self-describing format).
    linked: Option<(String, String)>,
}

/// A mate's or relation's state (its frames come with the real evaluation).
#[derive(Serialize, Deserialize)]
struct Status {
    id: MateId,
    ok: bool,
    inactive: bool,
    message: Option<String>,
    values: [f64; 4],
}

#[derive(Serialize, Deserialize)]
struct Connector {
    id: MateId,
    owner: InstanceId,
    child: Option<InstanceId>,
    local: Option<(DVec3, DQuat)>,
    frame: Option<(DVec3, DQuat)>,
    message: Option<String>,
}

fn frame_of(f: Option<Frame>) -> Option<(DVec3, DQuat)> {
    f.map(|f| (f.origin, f.rotation))
}

fn to_frame(f: Option<(DVec3, DQuat)>) -> Option<Frame> {
    f.map(|(o, r)| Frame::new(o, r))
}

fn node_of(n: &InstanceNode) -> Node {
    let kind = match n.kind {
        NodeKind::Part => 0,
        NodeKind::PartStudio => 1,
        NodeKind::Assembly => 2,
    };
    Node { path: n.path.clone(), name: n.name.clone(), kind, fixed: n.fixed, hidden: n.hidden, suppressed: n.suppressed, dof: n.dof, error: n.error.clone(), children: n.children.iter().map(node_of).collect(), rigid: n.rigid, linked: n.linked.as_ref().and_then(|(name, a)| Some((name.clone(), serde_json::to_string(&**a).ok()?))) }
}

fn to_node(n: Node) -> InstanceNode {
    let kind = match n.kind {
        0 => NodeKind::Part,
        1 => NodeKind::PartStudio,
        _ => NodeKind::Assembly,
    };
    InstanceNode { path: n.path, name: n.name, kind, fixed: n.fixed, hidden: n.hidden, suppressed: n.suppressed, dof: n.dof, error: n.error, children: n.children.into_iter().map(to_node).collect(), rigid: n.rigid, linked: n.linked.and_then(|(name, a)| Some((name, Arc::new(serde_json::from_str(&a).ok()?)))) }
}

impl Snapshot {
    fn of(r: &AssemblyResult) -> Self {
        let mut meshes: Vec<Arc<MeshBuffers>> = Vec::new();
        let mut index: HashMap<*const MeshBuffers, u32> = HashMap::new();
        let mut mesh_ix = |m: &Arc<MeshBuffers>| {
            *index.entry(Arc::as_ptr(m)).or_insert_with(|| {
                meshes.push(m.clone());
                meshes.len() as u32 - 1
            })
        };
        let occurrences = r
            .occurrences
            .iter()
            .map(|o| Occurrence {
                path: o.path.clone(),
                body: o.body.clone(),
                studio: o.studio,
                config: o.config.clone(),
                instance_name: o.instance_name.clone(),
                body_name: o.body_name.clone(),
                kind: o.kind,
                color: o.color,
                mesh: mesh_ix(&o.mesh),
                coarser: o.coarser.iter().map(&mut mesh_ix).collect(),
                levels_from: o.levels_from,
                inner: o.inner,
                transform: o.transform,
                face_refs: o.face_refs.clone(),
                edge_refs: o.edge_refs.clone(),
                vertex_refs: o.vertex_refs.clone(),
                hidden: o.hidden,
                local_bounds: o.local_bounds,
                mirrored: o.mirrored,
                external: o.external.clone(),
                flex: o.flex,
            })
            .collect();
        let status = |id, ok, inactive, message: &Option<String>, values| Status { id, ok, inactive, message: message.clone(), values };
        let s = &r.solved;
        Snapshot {
            meshes,
            occurrences,
            nodes: r.nodes.iter().map(node_of).collect(),
            mates: r.mates.iter().map(|m| status(m.id, m.ok, m.inactive, &m.message, m.values)).collect(),
            relations: r.relations.iter().map(|m| status(m.id, m.ok, m.inactive, &m.message, [0.0; 4])).collect(),
            connectors: r.connectors.iter().map(|c| Connector { id: c.id, owner: c.owner, child: c.child, local: frame_of(c.local), frame: frame_of(c.frame), message: c.message.clone() }).collect(),
            pattern_errors: r.pattern_errors.clone(),
            placements: s.placements.clone(),
            converged: s.converged,
            dof: s.dof,
            instance_dof: s.instance_dof.iter().map(|(k, v)| (*k, *v)).collect(),
            mate_ok: s.mate_ok.iter().map(|(k, v)| (*k, *v)).collect(),
            relation_ok: s.relation_ok.iter().map(|(k, v)| (*k, *v)).collect(),
            nested: s.nested.iter().map(|(k, v)| (*k, *v)).collect(),
            explode_trails: r.explode_trails.clone(),
        }
    }

    fn result(self) -> Option<AssemblyResult> {
        let meshes = self.meshes;
        let mesh = |i: u32| meshes.get(i as usize).cloned();
        let mut occurrences = Vec::with_capacity(self.occurrences.len());
        for o in self.occurrences {
            occurrences.push(PartOccurrence {
                path: o.path,
                body: o.body,
                studio: o.studio,
                config: o.config,
                instance_name: o.instance_name,
                body_name: o.body_name,
                kind: o.kind,
                color: o.color,
                mesh: mesh(o.mesh)?,
                coarser: o.coarser.into_iter().map(mesh).collect::<Option<_>>()?,
                levels_from: o.levels_from,
                inner: o.inner,
                transform: o.transform,
                face_refs: o.face_refs,
                edge_refs: o.edge_refs,
                vertex_refs: o.vertex_refs,
                hidden: o.hidden,
                local_bounds: o.local_bounds,
                context: Default::default(),
                mirrored: o.mirrored,
                external: o.external,
                flex: o.flex,
            });
        }
        let solved = Solved {
            placements: self.placements,
            converged: self.converged,
            dof: self.dof,
            instance_dof: self.instance_dof.into_iter().collect(),
            mate_ok: self.mate_ok.into_iter().collect(),
            relation_ok: self.relation_ok.into_iter().collect(),
            nested: self.nested.into_iter().collect(),
            ..Default::default()
        };
        Some(AssemblyResult {
            occurrences,
            nodes: self.nodes.into_iter().map(to_node).collect(),
            solved,
            mates: self.mates.into_iter().map(|m| MateStatus { id: m.id, ok: m.ok, inactive: m.inactive, message: m.message, frame_a: None, frame_b: None, values: m.values }).collect(),
            relations: self.relations.into_iter().map(|m| RelationStatus { id: m.id, ok: m.ok, inactive: m.inactive, message: m.message }).collect(),
            connectors: self.connectors.into_iter().map(|c| ConnectorStatus { id: c.id, owner: c.owner, child: c.child, local: to_frame(c.local), frame: to_frame(c.frame), message: c.message }).collect(),
            pattern_errors: self.pattern_errors,
            explode_trails: self.explode_trails,
            ..Default::default()
        })
    }
}

/// The cache key of the assembly as evaluated (its configuration applied) with its exploded view.
fn key(doc: &noshape_model::Document, tab: TabId, asm: &Assembly, explode: Option<usize>) -> Option<String> {
    cache::key(doc, &("assembly", tab, asm, explode))
}

impl AssemblyView {
    /// On the first evaluation: the cached one, if there is one, shown until the real one lands.
    pub(super) fn load_cached_result(&mut self, asm: &Assembly, window: &mut Window, cx: &mut Context<Self>) {
        if !cache::enabled() || std::mem::replace(&mut self.ui.cache_checked, true) {
            return;
        }
        let (doc, tab, asm, explode) = (self.doc.read(cx).session.doc.clone(), self.tab, asm.clone(), self.explode);
        let load = cx.background_spawn(async move {
            let key = key(&doc, tab, &asm, explode)?;
            let snap: Snapshot = cache::load(&cache::default_dir(), &key)?;
            Some((key, snap.result()?))
        });
        self.ui.cache_task = Some(cx.spawn_in(window, async move |this, cx| {
            let Some((key, result)) = load.await else { return };
            let _ = this.update_in(cx, |this: &mut Self, window, cx| {
                this.ui.cache_task = None;
                this.show_cached_result(key, result, window, cx);
            });
        }));
    }

    /// The cached evaluation stands in until the real one is in (unless that was quicker).
    pub(super) fn show_cached_result(&mut self, key: String, result: AssemblyResult, window: &mut Window, cx: &mut Context<Self>) {
        if self.ui.real_result_in {
            return;
        }
        noshape_render::perf::milestone("open: cached display in");
        noshape_render::perf::arm_frame_milestone("open: cached model frame presented");
        self.ui.cached_view = true;
        self.ui.cache_key = Some(key);
        self.apply_result(Arc::new(result), window, cx);
        cx.notify();
    }

    /// A real evaluation is applied: the view is the model's own again, a waiting action runs, and the result is
    /// cached once the assembly is idle.
    pub(super) fn note_real_result(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ui.real_result_in = true;
        self.ui.cache_task = None;
        if std::mem::take(&mut self.ui.cached_view) {
            if self.message.as_ref().is_some_and(|(_, m)| m == WAITING) {
                self.message = None;
            }
            cx.notify();
        }
        if let Some(action) = self.ui.after_eval.take() {
            let view = cx.entity();
            window.defer(cx, move |window, cx| view.update(cx, |this, cx| action(this, window, cx)));
        }
        if cache::enabled() && self.idle_for_cache() {
            self.schedule_cache_save(window, cx);
        }
    }

    /// No dialog previews anything (the result is the document's own).
    fn idle_for_cache(&self) -> bool {
        self.mate.is_none() && self.pattern.is_none() && self.replace_panel.is_none() && self.placing.is_none() && self.drag.is_none()
    }

    /// While the cached result stands in: `action` waits for the real one, and the user is told at once. Returns
    /// whether it waits.
    pub(super) fn wait_for_model(&mut self, action: Deferred, cx: &mut Context<Self>) -> bool {
        if !self.ui.cached_view {
            return false;
        }
        self.ui.after_eval = Some(action);
        self.say_waiting(cx);
        true
    }

    /// What can't wait (a drag) only says so.
    pub(super) fn say_waiting(&mut self, cx: &mut Context<Self>) {
        self.message = Some((false, WAITING.into()));
        cx.notify();
    }

    fn schedule_cache_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let seq = self.eval_seq;
        // The opened assembly's first evaluation at once (it may be closed soon); edits in a row once they pause.
        let wait = if self.ui.cache_key.is_none() { Duration::ZERO } else { SAVE_AFTER };
        self.ui.cache_save_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(wait).await;
            let job = this.update(cx, |this: &mut Self, cx| {
                if this.eval_seq != seq || !this.idle_for_cache() {
                    return None;
                }
                let (asm, _) = this.effective(cx)?;
                Some((this.doc.read(cx).session.doc.clone(), this.tab, asm, this.explode, this.result.clone(), this.ui.cache_key.clone()))
            });
            let Ok(Some((doc, tab, asm, explode, result, known))) = job else { return };
            let saved = cx
                .background_spawn(async move {
                    let key = key(&doc, tab, &asm, explode)?;
                    let dir = cache::default_dir();
                    if known.as_ref() == Some(&key) || cache::contains(&dir, &key) {
                        return Some(key);
                    }
                    cache::store(&dir, &key, &Snapshot::of(&result)).ok()?;
                    Some(key)
                })
                .await;
            let _ = this.update(cx, |this: &mut Self, _| {
                this.ui.cache_save_task = None;
                if saved.is_some() {
                    this.ui.cache_key = saved;
                }
            });
        }));
    }

    /// The instance list's top row while the cached result stands in.
    pub(super) fn render_regenerating_row(&self) -> Option<AnyElement> {
        self.ui.cached_view.then(cache::regenerating_row)
    }

    /// The opened assembly's first evaluation is applied: its frame armed, the `open` script's quit scheduled.
    pub(super) fn note_open_result(&mut self, cx: &mut Context<Self>) {
        if !noshape_render::perf::enabled() || std::mem::replace(&mut self.ui.open_noted, true) {
            return;
        }
        noshape_render::perf::milestone(&format!("open: full display in (regen {:.1} ms)", self.result.eval_ms));
        noshape_render::perf::arm_frame_milestone("open: full model frame presented");
        if std::env::var("NOSHAPE_PERF_SCRIPT").as_deref() != Ok("open") {
            return;
        }
        // Long enough for the armed frame to be presented and logged, and the cache written.
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(1500)).await;
            for _ in 0..200 {
                if this.update(cx, |this, _| this.ui.cache_save_task.is_none()).unwrap_or(true) {
                    break;
                }
                cx.background_executor().timer(Duration::from_millis(100)).await;
            }
            noshape_render::perf::flush();
            cx.update(|cx| cx.quit());
        })
        .detach();
    }
}

/// Tests: `r` written to the cache in `dir` and read back.
#[cfg(test)]
pub(super) fn cache_round_trip(r: &AssemblyResult, dir: &std::path::Path) -> Option<AssemblyResult> {
    cache::store(dir, "k", &Snapshot::of(r)).ok()?;
    cache::load::<Snapshot>(dir, "k")?.result()
}

#[cfg(test)]
mod tests {
    use super::{AssemblyResult, BodyId, BodyKind, ConfigValues, DMat4, DVec3, InstanceId, MeshBuffers, PartOccurrence, Snapshot, TabId, cache};
    use noshape_model::reference::FeatureId;
    use std::sync::Arc;

    /// A snapshot keeps each shared mesh once and gives back occurrences sharing it.
    #[test]
    fn a_snapshot_keeps_a_shared_mesh_once() {
        let mesh = Arc::new(MeshBuffers { positions: vec![[0.0; 3]; 3], indices: vec![0, 1, 2], ..Default::default() });
        let occ = |i: u64| PartOccurrence {
            path: vec![InstanceId(i)],
            body: BodyId { feature: FeatureId(1), index: 0 },
            studio: TabId(1),
            config: ConfigValues::default(),
            instance_name: format!("Pin <{i}>"),
            body_name: "Part 1".into(),
            kind: BodyKind::Solid,
            color: [0.5; 4],
            mesh: mesh.clone(),
            coarser: vec![mesh.clone()],
            levels_from: 0,
            inner: DMat4::IDENTITY,
            transform: DMat4::from_translation(DVec3::X * i as f64),
            face_refs: Vec::new(),
            edge_refs: Vec::new(),
            vertex_refs: Vec::new(),
            hidden: false,
            local_bounds: Some((DVec3::ZERO, DVec3::ONE)),
            context: Default::default(),
            mirrored: false,
            external: None,
            flex: None,
        };
        let r = AssemblyResult { occurrences: (1..=3).map(occ).collect(), ..Default::default() };
        let snap = Snapshot::of(&r);
        assert_eq!(snap.meshes.len(), 1);
        let dir = std::env::temp_dir().join(format!("noshape-asm-snapshot-{}", std::process::id()));
        cache::store(&dir, "k", &snap).unwrap();
        let back: Snapshot = cache::load(&dir, "k").unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let r2 = back.result().unwrap();
        assert_eq!(r2.occurrences.len(), 3);
        assert!(Arc::ptr_eq(&r2.occurrences[0].mesh, &r2.occurrences[2].mesh) && Arc::ptr_eq(&r2.occurrences[0].mesh, &r2.occurrences[0].coarser[0]));
        assert_eq!(r2.occurrences[2].transform, r.occurrences[2].transform);
        assert_eq!(r2.bounds(), r.bounds());
    }
}
