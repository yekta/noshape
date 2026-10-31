//! Instance list operations that work like Onshape's: the list filter with
//! its colon commands, copy / paste of instances with their mates and mate
//! connectors, renaming, hide / show helpers and Unfix all.

use super::{Assembly, Instance, InstanceId, MateConnectorRef, MateId, PatternKind, Placement, SubassemblyLock};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Kinds of rows the instance list filter's colon commands select.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterCommand {
    /// `:part` parts (and rigid Part Studio instances).
    Part,
    /// `:assembly` subassemblies.
    Assembly,
    /// `:mate` mates and relations.
    Mate,
    /// `:matelimits` mates with at least one limit.
    MateLimits,
    /// `:connector` mate connectors.
    Connector,
    /// `:item` non-geometric items.
    Item,
    /// `:folder` folders.
    Folder,
    /// `:errors`
    Errors,
    /// `:warnings`
    Warnings,
    /// `:suppressed`
    Suppressed,
    /// `:hidden`
    Hidden,
    /// `:shown`
    Shown,
}

impl FilterCommand {
    pub const ALL: [FilterCommand; 12] = [
        FilterCommand::Part,
        FilterCommand::Assembly,
        FilterCommand::Mate,
        FilterCommand::MateLimits,
        FilterCommand::Connector,
        FilterCommand::Item,
        FilterCommand::Folder,
        FilterCommand::Errors,
        FilterCommand::Warnings,
        FilterCommand::Suppressed,
        FilterCommand::Hidden,
        FilterCommand::Shown,
    ];

    /// The colon command as typed (`:part`).
    pub fn token(self) -> &'static str {
        match self {
            FilterCommand::Part => ":part",
            FilterCommand::Assembly => ":assembly",
            FilterCommand::Mate => ":mate",
            FilterCommand::MateLimits => ":matelimits",
            FilterCommand::Connector => ":connector",
            FilterCommand::Item => ":item",
            FilterCommand::Folder => ":folder",
            FilterCommand::Errors => ":errors",
            FilterCommand::Warnings => ":warnings",
            FilterCommand::Suppressed => ":suppressed",
            FilterCommand::Hidden => ":hidden",
            FilterCommand::Shown => ":shown",
        }
    }

    /// Label in the filter dropdown.
    pub fn label(self) -> &'static str {
        match self {
            FilterCommand::Part => "Filter by parts",
            FilterCommand::Assembly => "Filter by subassemblies",
            FilterCommand::Mate => "Filter by mates and relations",
            FilterCommand::MateLimits => "Filter by mates with limits",
            FilterCommand::Connector => "Filter by mate connectors",
            FilterCommand::Item => "Filter by items",
            FilterCommand::Folder => "Filter by folders",
            FilterCommand::Errors => "Filter by errors",
            FilterCommand::Warnings => "Filter by warnings",
            FilterCommand::Suppressed => "Filter by suppressed",
            FilterCommand::Hidden => "Filter by hidden",
            FilterCommand::Shown => "Filter by shown",
        }
    }

    fn parse(token: &str) -> Option<FilterCommand> {
        let t = token.to_ascii_lowercase();
        Self::ALL.into_iter().find(|c| c.token() == t)
    }
}

/// What a list row is, for filtering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    Part,
    Assembly,
    Mate,
    Relation,
    Group,
    Connector,
    Pattern,
    Folder,
    Item,
}

/// State of one list row the filter looks at.
#[derive(Clone, Copy, Debug)]
pub struct RowInfo<'a> {
    pub kind: RowKind,
    pub name: &'a str,
    pub error: bool,
    pub warning: bool,
    pub suppressed: bool,
    pub hidden: bool,
    pub has_limits: bool,
}

/// Parsed instance list filter: colon commands (all must match) plus a
/// case-insensitive name substring. `:mate :errors`, `:part :hidden bolt`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListFilter {
    pub commands: Vec<FilterCommand>,
    pub text: String,
}

impl ListFilter {
    pub fn parse(s: &str) -> Self {
        let mut f = ListFilter::default();
        let mut words = Vec::new();
        for w in s.split_whitespace() {
            match w.starts_with(':').then(|| FilterCommand::parse(w)).flatten() {
                Some(c) => {
                    if !f.commands.contains(&c) {
                        f.commands.push(c);
                    }
                }
                None => words.push(w),
            }
        }
        f.text = words.join(" ").to_lowercase();
        f
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty() && self.text.is_empty()
    }

    /// Does a row pass the filter?
    pub fn matches(&self, row: &RowInfo) -> bool {
        if !self.text.is_empty() && !row.name.to_lowercase().contains(&self.text) {
            return false;
        }
        self.commands.iter().all(|c| match c {
            FilterCommand::Part => row.kind == RowKind::Part,
            FilterCommand::Assembly => row.kind == RowKind::Assembly,
            FilterCommand::Mate => matches!(row.kind, RowKind::Mate | RowKind::Relation),
            FilterCommand::MateLimits => row.kind == RowKind::Mate && row.has_limits,
            FilterCommand::Connector => row.kind == RowKind::Connector,
            FilterCommand::Item => row.kind == RowKind::Item,
            FilterCommand::Folder => row.kind == RowKind::Folder,
            FilterCommand::Errors => row.error,
            FilterCommand::Warnings => row.warning,
            FilterCommand::Suppressed => row.suppressed,
            FilterCommand::Hidden => row.hidden,
            FilterCommand::Shown => !row.hidden,
        })
    }

    /// Does the filter admit instance rows at all (some commands only
    /// select mate features and vice versa)?
    pub fn wants_instances(&self) -> bool {
        !self.commands.iter().any(|c| matches!(c, FilterCommand::Mate | FilterCommand::MateLimits | FilterCommand::Connector))
    }

    pub fn wants_mate_features(&self) -> bool {
        !self.commands.iter().any(|c| matches!(c, FilterCommand::Part | FilterCommand::Assembly | FilterCommand::Item))
    }
}

/// Copied instances with the mates and mate connectors among them
/// (Onshape: "the entities, Mate connectors, and Mates are also duplicated").
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct InstanceClipboard {
    pub instances: Vec<Instance>,
    pub mates: Vec<super::Mate>,
    pub connectors: Vec<super::AsmMateConnector>,
}

impl InstanceClipboard {
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// Copy instances: every mate whose instances are all in the set (or the
    /// assembly origin) and every assembly mate connector they own.
    pub fn copy(asm: &Assembly, ids: &[InstanceId]) -> Self {
        let instances: Vec<Instance> = asm.instances.iter().filter(|i| ids.contains(&i.id)).cloned().collect();
        let inside = |c: &MateConnectorRef| c.instance == InstanceId::ORIGIN || ids.contains(&c.instance);
        let connectors: Vec<super::AsmMateConnector> = asm.connectors.iter().filter(|c| ids.contains(&c.owner())).cloned().collect();
        let mates = asm
            .mates
            .iter()
            .filter(|m| asm.mate_generated_by(m.id).is_none() && m.connectors().all(inside) && !m.instances().is_empty())
            .filter(|m| m.connectors().all(|c| c.connector.is_none_or(|id| connectors.iter().any(|x| x.id == id))))
            .cloned()
            .collect();
        InstanceClipboard { instances, mates, connectors }
    }

    /// Paste into `asm` with new ids and `Name <n>` names, the copies moved
    /// by `offset` (applied after their placement: world translation).
    /// Pasted instances are never fixed. Returns the new instance ids.
    pub fn paste(&self, asm: &mut Assembly, offset: [f64; 3]) -> Vec<InstanceId> {
        let mut next = asm.next_id();
        let mut inst_map: HashMap<InstanceId, InstanceId> = HashMap::new();
        let mut mate_map: HashMap<MateId, MateId> = HashMap::new();
        let mut out = Vec::new();
        for i in &self.instances {
            let id = InstanceId(next);
            next += 1;
            inst_map.insert(i.id, id);
            let name = asm.next_instance_name(Assembly::base_name(&i.name));
            let t = i.placement.translation;
            let placement = Placement { translation: [t[0] + offset[0], t[1] + offset[1], t[2] + offset[2]], rotation: i.placement.rotation };
            asm.instances.push(Instance { id, name, placement, fixed: false, ..i.clone() });
            out.push(id);
        }
        let map_ref = |c: &MateConnectorRef, inst_map: &HashMap<InstanceId, InstanceId>, mate_map: &HashMap<MateId, MateId>| {
            let mut c = c.clone();
            if let Some(n) = inst_map.get(&c.instance) {
                c.instance = *n;
            }
            if let Some(id) = c.connector
                && let Some(n) = mate_map.get(&id)
            {
                c.connector = Some(*n);
            }
            c
        };
        for c in &self.connectors {
            let id = MateId(next);
            next += 1;
            mate_map.insert(c.id, id);
            let mut n = c.clone();
            n.id = id;
            n.name = asm.next_feature_name("Mate connector");
            n.on = map_ref(&c.on, &inst_map, &mate_map);
            asm.connectors.push(n);
        }
        for m in &self.mates {
            let id = MateId(next);
            next += 1;
            let mut n = m.clone();
            n.id = id;
            n.name = asm.next_feature_name(m.mate_type.name());
            n.a = map_ref(&m.a, &inst_map, &mate_map);
            n.b = map_ref(&m.b, &inst_map, &mate_map);
            n.extra = m.extra.iter().map(|c| map_ref(c, &inst_map, &mate_map)).collect();
            asm.mates.push(n);
        }
        out
    }
}

impl Assembly {
    /// Rename an instance or a mate feature (mate, relation, group, mate
    /// connector, pattern). Empty names are ignored. Returns true if renamed.
    pub fn rename(&mut self, instance: Option<InstanceId>, feature: Option<MateId>, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() {
            return false;
        }
        if let Some(i) = instance.and_then(|id| self.instance_mut(id)) {
            i.name = name.to_string();
            return true;
        }
        let Some(id) = feature else { return false };
        if let Some(m) = self.mates.iter_mut().find(|m| m.id == id) {
            m.name = name.to_string();
        } else if let Some(r) = self.relations.iter_mut().find(|r| r.id == id) {
            r.name = name.to_string();
        } else if let Some(g) = self.groups.iter_mut().find(|g| g.id == id) {
            g.name = name.to_string();
        } else if let Some(c) = self.connectors.iter_mut().find(|c| c.id == id) {
            c.name = name.to_string();
        } else if let Some(p) = self.patterns.iter_mut().find(|p| p.id == id) {
            p.name = name.to_string();
        } else if let Some(f) = self.folders.iter_mut().find(|f| f.id == id) {
            f.name = name.to_string();
        } else {
            return false;
        }
        true
    }

    /// Hide other instances: every instance except `keep` is hidden, `keep` shown.
    pub fn hide_other_instances(&mut self, keep: &[InstanceId]) {
        for i in &mut self.instances {
            i.hidden = !keep.contains(&i.id);
        }
    }

    /// Hide / show all instances.
    pub fn set_all_hidden(&mut self, hidden: bool) {
        for i in &mut self.instances {
            i.hidden = hidden;
        }
    }

    /// Unfix all: every instance becomes unfixed. Returns how many changed.
    pub fn unfix_all(&mut self) -> usize {
        let mut n = 0;
        for i in self.instances.iter_mut().filter(|i| i.fixed) {
            i.fixed = false;
            n += 1;
        }
        n
    }

    /// Hide / show mate indicators (and assembly mate connectors).
    pub fn set_mates_hidden(&mut self, ids: &[MateId], hidden: bool) {
        for m in self.mates.iter_mut().filter(|m| ids.contains(&m.id)) {
            m.hidden = hidden;
        }
        for c in self.connectors.iter_mut().filter(|c| ids.contains(&c.id)) {
            c.hidden = hidden;
        }
    }

    /// Hide every mate and mate connector except `keep`.
    pub fn hide_other_mates(&mut self, keep: &[MateId]) {
        for m in &mut self.mates {
            m.hidden = !keep.contains(&m.id);
        }
        for c in &mut self.connectors {
            c.hidden = !keep.contains(&c.id);
        }
    }

    /// Instances that are fixed (pin icon) — used for the "More than one
    /// part is fixed" warning.
    pub fn fixed_instances(&self) -> Vec<InstanceId> {
        self.instances.iter().filter(|i| i.fixed && !i.suppressed).map(|i| i.id).collect()
    }
}

// ---------------------------------------------------------------- subassemblies

/// Re-path a connector of the parent that points into moved instances so it
/// points through the new subassembly instance.
fn repath_into(c: &mut MateConnectorRef, moved: &[InstanceId], sub: InstanceId) {
    if c.connector.is_none() && moved.contains(&c.instance) {
        c.path.insert(0, c.instance);
        c.instance = sub;
    }
}

impl Assembly {
    /// Patterns that go along when the instances `ids` move to a new
    /// subassembly (live request 1564: "Dependent references to be moved"):
    /// those whose seeds all move and whose references (directions, axis,
    /// mirror plane, seed mate) all end up inside it.
    pub fn dependent_patterns(&self, ids: &[InstanceId]) -> Vec<MateId> {
        let base: Vec<InstanceId> = self.instances.iter().filter(|i| ids.contains(&i.id) && self.generated_by(i.id).is_none()).map(|i| i.id).collect();
        self.subassembly_split(&base).3
    }

    /// What moves with the instances `base`: (instances with the moving
    /// patterns' copies, mates, mate connectors, patterns).
    fn subassembly_split(&self, base: &[InstanceId]) -> (Vec<InstanceId>, Vec<MateId>, Vec<MateId>, Vec<MateId>) {
        let mut pats: Vec<MateId> = self.patterns.iter().filter(|p| !p.seeds.is_empty() && p.seeds.iter().all(|s| base.contains(s))).map(|p| p.id).collect();
        loop {
            let mut moved = base.to_vec();
            for p in self.patterns.iter().filter(|p| pats.contains(&p.id)) {
                moved.extend(p.generated.iter().map(|g| g.instance));
            }
            let inside = |c: &MateConnectorRef| c.instance != InstanceId::ORIGIN && moved.contains(&c.instance);
            // Mates entirely among the moved instances (and the connectors they use).
            let internal: Vec<MateId> = self.mates.iter().filter(|m| m.connectors().all(inside)).map(|m| m.id).collect();
            let mate_users = |cid: MateId| self.mates.iter().filter(move |m| m.connectors().any(|c| c.connector == Some(cid))).map(|m| m.id);
            let pattern_users = |cid: MateId| self.patterns.iter().filter(move |p| p.connector_refs().iter().any(|c| c.connector == Some(cid))).map(|p| p.id);
            // A connector moves when its owner does (or it sits on the origin
            // and only moving patterns use it) and nothing staying uses it.
            let moved_conns: Vec<MateId> = self
                .connectors
                .iter()
                .filter(|c| moved.contains(&c.owner()) || (c.owner() == InstanceId::ORIGIN && pattern_users(c.id).any(|p| pats.contains(&p))))
                .filter(|c| mate_users(c.id).all(|m| internal.contains(&m)) && pattern_users(c.id).all(|p| pats.contains(&p)))
                .map(|c| c.id)
                .collect();
            // A mate using a connector that stays in the parent stays too.
            let internal: Vec<MateId> = internal.into_iter().filter(|id| self.mate(*id).is_some_and(|m| m.connectors().all(|c| c.connector.is_none_or(|x| moved_conns.contains(&x))))).collect();
            let resolves = |c: &&MateConnectorRef| match c.connector {
                Some(x) => moved_conns.contains(&x),
                None => c.instance == InstanceId::ORIGIN || moved.contains(&c.instance),
            };
            let keep: Vec<MateId> = pats
                .iter()
                .copied()
                .filter(|id| {
                    self.pattern(*id).is_some_and(|p| {
                        p.connector_refs().iter().all(resolves) && p.seed_mate().is_none_or(|m| internal.contains(&m)) && p.generated.iter().filter_map(|g| g.mate).all(|m| internal.contains(&m))
                    })
                })
                .collect();
            if keep.len() == pats.len() {
                return (moved, internal, moved_conns, pats);
            }
            pats = keep;
        }
    }

    /// Onshape "Move to new subassembly": the instances `ids` (with the mates,
    /// mate connectors, relations and groups entirely among them, and the
    /// patterns of [`Self::dependent_patterns`] with their copies) move into
    /// a new assembly, returned; the parent gets one instance of `sub_tab`
    /// (named `sub_name <n>`, fixed if a moved instance was fixed) at the
    /// identity, so nothing moves. Mates between moved and remaining
    /// instances stay in the parent and now reach the moved instances
    /// through the subassembly.
    /// Returns (subassembly content, new subassembly instance id).
    pub fn move_to_new_subassembly(&mut self, ids: &[InstanceId], sub_tab: crate::document::TabId, sub_name: &str) -> (Assembly, InstanceId) {
        let base: Vec<InstanceId> = self.instances.iter().filter(|i| ids.contains(&i.id) && self.generated_by(i.id).is_none()).map(|i| i.id).collect();
        let (moved, internal, moved_conns, moved_pats) = self.subassembly_split(&base);
        let mut sub = Assembly::default();
        let sub_id = InstanceId(self.next_id());
        sub.instances = self.instances.iter().filter(|i| moved.contains(&i.id)).cloned().collect();
        let any_fixed = sub.instances.iter().any(|i| i.fixed);
        sub.mates = self.mates.iter().filter(|m| internal.contains(&m.id)).cloned().collect();
        sub.connectors = self.connectors.iter().filter(|c| moved_conns.contains(&c.id)).cloned().collect();
        sub.patterns = self.patterns.iter().filter(|p| moved_pats.contains(&p.id)).cloned().collect();
        sub.relations = self.relations.iter().filter(|r| r.mates.iter().all(|m| internal.contains(m))).cloned().collect();
        sub.groups = self.groups.iter().filter(|g| g.instances.iter().all(|i| moved.contains(i)) && g.patterns.iter().all(|p| moved_pats.contains(p))).cloned().collect();
        let moved_groups: Vec<MateId> = sub.groups.iter().map(|g| g.id).collect();
        let moved_rels: Vec<MateId> = sub.relations.iter().map(|r| r.id).collect();
        self.patterns.retain(|p| !moved_pats.contains(&p.id));
        // Parent: drop what moved, re-path what still points into it.
        self.mates.retain(|m| !internal.contains(&m.id));
        self.connectors.retain(|c| !moved_conns.contains(&c.id));
        self.relations.retain(|r| !moved_rels.contains(&r.id));
        self.groups.retain(|g| !moved_groups.contains(&g.id));
        for g in &mut self.groups {
            g.instances.retain(|i| !moved.contains(i));
        }
        self.groups.retain(|g| !g.is_empty());
        for m in &mut self.mates {
            repath_into(&mut m.a, &moved, sub_id);
            repath_into(&mut m.b, &moved, sub_id);
            for c in &mut m.extra {
                repath_into(c, &moved, sub_id);
            }
        }
        for c in &mut self.connectors {
            repath_into(&mut c.on, &moved, sub_id);
        }
        let pos = self.instances.iter().position(|i| moved.contains(&i.id)).unwrap_or(self.instances.len());
        self.instances.retain(|i| !moved.contains(&i.id));
        let name = self.next_instance_name(sub_name);
        let inst = Instance { id: sub_id, name, source: super::InstanceSource::Assembly { assembly: sub_tab, config: Default::default() }, placement: Placement::default(), fixed: any_fixed, hidden: false, suppressed: false, lock: SubassemblyLock::Flexible, nested: Vec::new(), best_tessellation: false, reference_designator: None, unsuppressed_if: None };
        self.instances.insert(pos.min(self.instances.len()), inst);
        (sub, sub_id)
    }

    /// Onshape "Dissolve subassembly": the subassembly instance `sub_inst`
    /// (content `sub`) is replaced by copies of its instances, mates, mate
    /// connectors, relations, groups and patterns at the same world positions; parent
    /// mates into the subassembly are re-pathed. Returns the new instance ids.
    pub fn dissolve_subassembly(&mut self, sub_inst: InstanceId, sub: &Assembly) -> Vec<InstanceId> {
        let Some(si) = self.instance(sub_inst).cloned() else { return vec![] };
        let base = si.placement.to_dmat4();
        let mut next = self.next_id();
        let mut inst_map: HashMap<InstanceId, InstanceId> = HashMap::new();
        let mut feat_map: HashMap<MateId, MateId> = HashMap::new();
        let fresh = |n: &mut u64| {
            let v = *n;
            *n += 1;
            v
        };
        let mut new_instances = Vec::new();
        for i in &sub.instances {
            let id = InstanceId(fresh(&mut next));
            inst_map.insert(i.id, id);
            let placement = Placement::from_dmat4(&(base * i.placement.to_dmat4()));
            new_instances.push(Instance { id, placement, fixed: i.fixed && si.fixed, ..i.clone() });
        }
        for c in &sub.connectors {
            feat_map.insert(c.id, MateId(fresh(&mut next)));
        }
        for m in &sub.mates {
            feat_map.insert(m.id, MateId(fresh(&mut next)));
        }
        for r in &sub.relations {
            feat_map.insert(r.id, MateId(fresh(&mut next)));
        }
        for g in &sub.groups {
            feat_map.insert(g.id, MateId(fresh(&mut next)));
        }
        for p in &sub.patterns {
            feat_map.insert(p.id, MateId(fresh(&mut next)));
        }
        let map_inner = |c: &MateConnectorRef| {
            let mut c = c.clone();
            if let Some(n) = inst_map.get(&c.instance) {
                c.instance = *n;
            }
            if let Some(id) = c.connector
                && let Some(n) = feat_map.get(&id)
            {
                c.connector = Some(*n);
            }
            c
        };
        // Parent references into the subassembly: [sub, child, ...] -> [child copy, ...].
        let map_outer = |c: &mut MateConnectorRef| {
            if c.instance == sub_inst && c.connector.is_none() && !c.path.is_empty() {
                let child = c.path.remove(0);
                if let Some(n) = inst_map.get(&child) {
                    c.instance = *n;
                }
            }
        };
        for m in &mut self.mates {
            map_outer(&mut m.a);
            map_outer(&mut m.b);
            for c in &mut m.extra {
                map_outer(c);
            }
        }
        for c in &mut self.connectors {
            map_outer(&mut c.on);
        }
        for c in &sub.connectors {
            let mut n = c.clone();
            n.id = feat_map[&c.id];
            n.on = map_inner(&c.on);
            self.connectors.push(n);
        }
        for m in &sub.mates {
            let mut n = m.clone();
            n.id = feat_map[&m.id];
            n.a = map_inner(&m.a);
            n.b = map_inner(&m.b);
            n.extra = m.extra.iter().map(map_inner).collect();
            self.mates.push(n);
        }
        for r in &sub.relations {
            let mut n = r.clone();
            n.id = feat_map[&r.id];
            n.mates = r.mates.map(|m| feat_map.get(&m).copied().unwrap_or(m));
            self.relations.push(n);
        }
        for g in &sub.groups {
            let mut n = g.clone();
            n.id = feat_map[&g.id];
            n.instances = g.instances.iter().filter_map(|i| inst_map.get(i).copied()).collect();
            n.patterns = g.patterns.iter().filter_map(|p| feat_map.get(p).copied()).collect();
            self.groups.push(n);
        }
        // Patterns come back with their seeds and copies (live request 1564).
        let inst = |i: InstanceId| inst_map.get(&i).copied().unwrap_or(i);
        let feat = |m: MateId| feat_map.get(&m).copied().unwrap_or(m);
        for p in &sub.patterns {
            let mut n = p.clone();
            n.id = feat_map[&p.id];
            n.seeds = p.seeds.iter().map(|i| inst(*i)).collect();
            for g in &mut n.generated {
                g.seed = inst(g.seed);
                g.instance = inst(g.instance);
                g.mate = g.mate.map(feat);
                g.target = g.target.as_ref().map(map_inner);
            }
            for c in n.connector_refs_mut() {
                *c = map_inner(c);
            }
            match &mut n.kind {
                PatternKind::PartPattern { mate } => *mate = feat(*mate),
                PatternKind::Replicate { mate, scope_refs, .. } => {
                    *mate = feat(*mate);
                    for e in scope_refs {
                        e.instance = inst(e.instance);
                    }
                }
                PatternKind::Mirror { derived, placement, .. } => {
                    for i in derived {
                        *i = inst(*i);
                    }
                    for (i, _) in placement {
                        *i = inst(*i);
                    }
                }
                PatternKind::Linear { .. } | PatternKind::Circular { .. } => {}
            }
            self.patterns.push(n);
        }
        let pos = self.instances.iter().position(|i| i.id == sub_inst).unwrap_or(self.instances.len());
        self.instances.retain(|i| i.id != sub_inst);
        let ids: Vec<InstanceId> = new_instances.iter().map(|i| i.id).collect();
        for (k, i) in new_instances.into_iter().enumerate() {
            self.instances.insert((pos + k).min(self.instances.len()), i);
        }
        for g in &mut self.groups {
            g.instances.retain(|i| *i != sub_inst);
        }
        self.groups.retain(|g| !g.is_empty());
        ids
    }
}

/// Next free "Assembly N" tab name.
pub fn next_assembly_tab_name(doc: &crate::document::Document) -> String {
    let mut n = 1;
    loop {
        let name = format!("Assembly {n}");
        if !doc.tabs.iter().any(|t| t.name == name) {
            return name;
        }
        n += 1;
    }
}

// ---------------------------------------------------------------- folders and display states

impl Assembly {
    /// Folder holding an instance.
    pub fn folder_of_instance(&self, id: InstanceId) -> Option<&super::ListFolder> {
        self.folders.iter().find(|f| f.instances.contains(&id))
    }

    /// Folder holding a mate feature.
    pub fn folder_of_feature(&self, id: MateId) -> Option<&super::ListFolder> {
        self.folders.iter().find(|f| f.features.contains(&id))
    }

    /// A new folder's name: always "New folder" (live assembly/536; the list
    /// shows the member count after it, "New folder (2)").
    pub fn next_folder_name(&self) -> String {
        "New folder".to_string()
    }

    /// New folder ("New folder", live assembly/536–537) around instances or
    /// mate features (taken out
    /// of the folders they were in). Returns its id.
    pub fn add_folder(&mut self, instances: &[InstanceId], features: &[MateId]) -> MateId {
        for f in &mut self.folders {
            f.instances.retain(|i| !instances.contains(i));
            f.features.retain(|i| !features.contains(i));
        }
        let id = MateId(self.next_id());
        let name = self.next_folder_name();
        // Members keep the list order.
        let inst: Vec<InstanceId> = self.instances.iter().map(|i| i.id).filter(|i| instances.contains(i)).collect();
        self.folders.push(super::ListFolder { id, name, instances: inst, features: features.to_vec(), unsuppressed_if: None });
        id
    }

    /// Unpack folder: the folder goes, its contents stay.
    pub fn unpack_folder(&mut self, id: MateId) {
        self.folders.retain(|f| f.id != id);
    }

    /// Delete folder: the folder and its contents.
    pub fn delete_folder(&mut self, id: MateId) {
        let Some(f) = self.folders.iter().find(|f| f.id == id).cloned() else { return };
        self.folders.retain(|x| x.id != id);
        for i in f.instances {
            self.remove_instance(i);
        }
        for m in f.features {
            self.remove_mate(m);
        }
    }

    /// Drag and drop in the instance list: move instance `id` before
    /// `before` (None: to the end). It joins `before`'s folder (or leaves
    /// its folder when dropped among unfoldered instances). The list order
    /// drives the BOM order.
    pub fn move_instance(&mut self, id: InstanceId, before: Option<InstanceId>) {
        if before == Some(id) {
            return;
        }
        let Some(from) = self.instances.iter().position(|i| i.id == id) else { return };
        let inst = self.instances.remove(from);
        let to = before.and_then(|b| self.instances.iter().position(|i| i.id == b)).unwrap_or(self.instances.len());
        self.instances.insert(to, inst);
        let target = before.and_then(|b| self.folder_of_instance(b).map(|f| f.id));
        for f in &mut self.folders {
            f.instances.retain(|i| *i != id);
        }
        if let Some(fid) = target
            && let Some(f) = self.folders.iter_mut().find(|f| f.id == fid)
        {
            f.instances.push(id);
        }
        // Folder members keep the list order.
        let order: Vec<InstanceId> = self.instances.iter().map(|i| i.id).collect();
        for f in &mut self.folders {
            f.instances.sort_by_key(|i| order.iter().position(|o| o == i));
        }
    }

    /// Drop an instance onto a folder row: into that folder.
    pub fn move_into_folder(&mut self, id: InstanceId, folder: MateId) {
        for f in &mut self.folders {
            f.instances.retain(|i| *i != id);
        }
        if let Some(f) = self.folders.iter_mut().find(|f| f.id == folder) {
            f.instances.push(id);
        }
        let order: Vec<InstanceId> = self.instances.iter().map(|i| i.id).collect();
        for f in &mut self.folders {
            f.instances.sort_by_key(|i| order.iter().position(|o| o == i));
        }
    }

    pub fn rename_folder(&mut self, id: MateId, name: &str) -> bool {
        let name = name.trim();
        match self.folders.iter_mut().find(|f| f.id == id) {
            Some(f) if !name.is_empty() => {
                f.name = name.to_string();
                true
            }
            _ => false,
        }
    }

    /// Capture the current visibility as a display state (replacing one of
    /// the same name). Returns false for an empty name.
    pub fn save_display_state(&mut self, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() {
            return false;
        }
        let state = super::DisplayState {
            name: name.to_string(),
            hidden_instances: self.instances.iter().filter(|i| i.hidden).map(|i| i.id).collect(),
            hidden_features: self.mates.iter().filter(|m| m.hidden).map(|m| m.id).chain(self.connectors.iter().filter(|c| c.hidden).map(|c| c.id)).collect(),
            known: self.instances.iter().map(|i| i.id.0).chain(self.mates.iter().map(|m| m.id.0)).chain(self.connectors.iter().map(|c| c.id.0)).collect(),
        };
        match self.display_states.iter_mut().find(|d| d.name == state.name) {
            Some(d) => *d = state,
            None => self.display_states.push(state),
        }
        true
    }

    /// Apply a display state. Items created after it was captured keep
    /// Onshape's defaults: instances and explicit mate connectors shown,
    /// mates hidden.
    pub fn apply_display_state(&mut self, index: usize) -> bool {
        let Some(d) = self.display_states.get(index).cloned() else { return false };
        let known = |id: u64| d.known.contains(&id);
        for i in &mut self.instances {
            i.hidden = if known(i.id.0) { d.hidden_instances.contains(&i.id) } else { false };
        }
        for m in &mut self.mates {
            m.hidden = if known(m.id.0) { d.hidden_features.contains(&m.id) } else { true };
        }
        for c in &mut self.connectors {
            c.hidden = if known(c.id.0) { d.hidden_features.contains(&c.id) } else { false };
        }
        true
    }
}

impl Assembly {
    /// Drag and drop onto a subassembly row: the instances `ids` move into
    /// the subassembly `sub_inst` (content `sub`, edited in place) at the same
    /// world positions, with the mates and mate connectors entirely among
    /// them; mates to the rest of this assembly reach them through the
    /// subassembly. Returns the instances' new ids in the subassembly.
    pub fn move_into_subassembly(&mut self, ids: &[InstanceId], sub_inst: InstanceId, sub: &mut Assembly) -> Vec<InstanceId> {
        let Some(si) = self.instance(sub_inst).cloned() else { return vec![] };
        let moved: Vec<InstanceId> = self.instances.iter().filter(|i| ids.contains(&i.id) && i.id != sub_inst && self.generated_by(i.id).is_none()).map(|i| i.id).collect();
        if moved.is_empty() {
            return vec![];
        }
        let inv = si.placement.to_dmat4().inverse();
        let mut next = sub.next_id();
        let mut inst_map: HashMap<InstanceId, InstanceId> = HashMap::new();
        let mut new_ids = Vec::new();
        for i in self.instances.iter().filter(|i| moved.contains(&i.id)) {
            let id = InstanceId(next);
            next += 1;
            inst_map.insert(i.id, id);
            let placement = Placement::from_dmat4(&(inv * i.placement.to_dmat4()));
            let name = sub.next_instance_name(Assembly::base_name(&i.name));
            sub.instances.push(Instance { id, name, placement, ..i.clone() });
            new_ids.push(id);
        }
        let inside = |c: &MateConnectorRef| c.instance != InstanceId::ORIGIN && moved.contains(&c.instance);
        let internal: Vec<MateId> = self.mates.iter().filter(|m| m.connectors().all(inside)).map(|m| m.id).collect();
        let users = |cid: MateId, a: &Assembly| a.mates.iter().filter(|m| m.connectors().any(|c| c.connector == Some(cid))).map(|m| m.id).collect::<Vec<_>>();
        let moved_conns: Vec<MateId> = self.connectors.iter().filter(|c| moved.contains(&c.owner()) && users(c.id, self).iter().all(|m| internal.contains(m))).map(|c| c.id).collect();
        let internal: Vec<MateId> = internal.into_iter().filter(|id| self.mate(*id).is_some_and(|m| m.connectors().all(|c| c.connector.is_none_or(|x| moved_conns.contains(&x))))).collect();
        let mut feat_map: HashMap<MateId, MateId> = HashMap::new();
        for c in self.connectors.iter().filter(|c| moved_conns.contains(&c.id)) {
            feat_map.insert(c.id, MateId(next));
            next += 1;
        }
        let remap = |c: &MateConnectorRef, feat_map: &HashMap<MateId, MateId>| {
            let mut c = c.clone();
            if let Some(n) = inst_map.get(&c.instance) {
                c.instance = *n;
            }
            if let Some(id) = c.connector
                && let Some(n) = feat_map.get(&id)
            {
                c.connector = Some(*n);
            }
            c
        };
        for c in self.connectors.iter().filter(|c| moved_conns.contains(&c.id)) {
            let mut n = c.clone();
            n.id = feat_map[&c.id];
            n.name = sub.next_feature_name("Mate connector");
            n.on = remap(&c.on, &feat_map);
            sub.connectors.push(n);
        }
        for m in self.mates.iter().filter(|m| internal.contains(&m.id)) {
            let mut n = m.clone();
            n.id = MateId(next);
            next += 1;
            n.name = sub.next_feature_name(m.mate_type.name());
            n.a = remap(&m.a, &feat_map);
            n.b = remap(&m.b, &feat_map);
            n.extra = m.extra.iter().map(|c| remap(c, &feat_map)).collect();
            sub.mates.push(n);
        }
        self.mates.retain(|m| !internal.contains(&m.id));
        self.connectors.retain(|c| !moved_conns.contains(&c.id));
        // Remaining references into the moved instances go through the subassembly.
        let repath = |c: &mut MateConnectorRef| {
            if c.connector.is_none()
                && let Some(n) = inst_map.get(&c.instance)
            {
                c.path.insert(0, *n);
                c.instance = sub_inst;
            }
        };
        for m in &mut self.mates {
            repath(&mut m.a);
            repath(&mut m.b);
            for c in &mut m.extra {
                repath(c);
            }
        }
        for c in &mut self.connectors {
            repath(&mut c.on);
        }
        self.instances.retain(|i| !moved.contains(&i.id));
        for f in &mut self.folders {
            f.instances.retain(|i| !moved.contains(i));
        }
        for g in &mut self.groups {
            g.instances.retain(|i| !moved.contains(i));
        }
        self.groups.retain(|g| !g.is_empty());
        // A flexible subassembly keeps them where they are.
        if let Some(s) = self.instance_mut(sub_inst) {
            for (old, new) in &inst_map {
                if let Some(p) = sub.instance(*new).map(|i| i.placement) {
                    let _ = old;
                    s.nested.retain(|(c, _)| c != new);
                    s.nested.push((*new, p));
                }
            }
        }
        new_ids
    }
}

// ---------------------------------------------------------------- feature order

fn move_before<T>(v: &mut Vec<T>, id: impl Fn(&T) -> MateId, moved: MateId, target: MateId) -> Option<bool> {
    let from = v.iter().position(|x| id(x) == moved)?;
    let to = v.iter().position(|x| id(x) == target)?;
    if from == to {
        return Some(false);
    }
    let item = v.remove(from);
    let to = v.iter().position(|x| id(x) == target).unwrap_or(to);
    v.insert(to, item);
    Some(true)
}

impl Assembly {
    /// Drag to reorder in the Mates list: `moved` goes right before `target`
    /// (both mate connectors, mates, relations or groups). False when they
    /// are of different kinds or the same feature.
    pub fn move_feature_before(&mut self, moved: MateId, target: MateId) -> bool {
        move_before(&mut self.mates, |m| m.id, moved, target)
            .or_else(|| move_before(&mut self.connectors, |c| c.id, moved, target))
            .or_else(|| move_before(&mut self.relations, |r| r.id, moved, target))
            .or_else(|| move_before(&mut self.groups, |g| g.id, moved, target))
            .unwrap_or(false)
    }
}
