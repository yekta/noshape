//! The ui-parity state of an assembly tab (tests/ui-parity/README.md: the
//! same keys as a Part Studio's, the list's rows being its instances and
//! mates, plus `assembly`).

use super::*;
use serde_json::{json, Value};

impl AssemblyView {
    /// Every slice painted again (its named controls record where they are).
    pub fn ui_redraw(&self, cx: &mut Context<Self>) {
        if let Some(s) = &self.slices {
            for slice in [&s.toolbar, &s.rows, &s.overlay, &s.under, &s.front] {
                slice.update(cx, |_, cx| cx.notify());
            }
        }
        if let Some(m) = &self.mate {
            m.dialog.update(cx, |_, cx| cx.notify());
        }
        cx.notify();
    }

    /// Where target `t` shows now, in window coordinates: `face` / `edge` /
    /// `vertex` [x, y, z] (mm; with `instance`, in that part's coordinates
    /// through its current placement, as live's runner reads it; else
    /// assembly coordinates), `origin`.
    pub fn ui_target_point(&self, t: &Value, cx: &App) -> Result<Point<Pixels>, String> {
        let vp = self.viewport.read(cx);
        let mm = |v: &Value| -> Option<DVec3> {
            let a = v.as_array()?;
            Some(DVec3::new(a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?) * 1e-3)
        };
        if let Some(name) = t.get("view_cube").and_then(Value::as_str) {
            return crate::view_cube::ui_cell_point(name, self.cube_origin, vp.camera.rotation);
        }
        // The move triad's handles (README "triad"): an axis's shaft, the ring at its tip, the centre; drawn as the
        // scene draws the manipulator (asm_scene, world sizes).
        let triad_point = |handle: &str| -> Result<DVec3, String> {
            let f = self.manipulator(cx).ok_or("no triad shown")?;
            let size = self.triad_size() * 1.6;
            let axes = [f.x(), f.y(), f.z()];
            let k = |c: &str| ["x", "y", "z"].iter().position(|a| *a == c).ok_or(format!("no triad handle {handle}"));
            if handle == "origin" {
                return Ok(f.origin);
            }
            if let Some(c) = handle.strip_prefix("ring-") {
                let k = k(c)?;
                return Ok(f.origin + axes[k] * (size * 0.93) + axes[(k + 1) % 3] * (size * 0.12));
            }
            Ok(f.origin + axes[k(handle)?] * (size * 0.4))
        };
        let world = match t.get("triad").and_then(Value::as_str) {
            Some(h) => Some(triad_point(h)?),
            None => None,
        };
        if t.get("empty").is_some() {
            // From the middle outwards, off every part's box on screen and the open dialog (as the Part Studio's).
            let size = vp.size();
            let boxes: Vec<(glam::DVec2, glam::DVec2)> = self
                .result
                .occurrences
                .iter()
                .filter(|o| !o.hidden)
                .filter_map(|o| {
                    let (lo, hi) = o.local_bounds?;
                    let corners: Vec<glam::DVec2> = (0..8).filter_map(|i| vp.project(o.transform.transform_point3(DVec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z })))).collect();
                    Some(corners.iter().fold((glam::DVec2::splat(f64::INFINITY), glam::DVec2::splat(f64::NEG_INFINITY)), |(a, b), c| (a.min(*c), b.max(*c))))
                })
                .collect();
            let dialog = crate::ui_script::bounds_of("dialog");
            let mut grid: Vec<glam::DVec2> = (1..8).flat_map(|i| (1..8).map(move |j| glam::DVec2::new(size.x * i as f64 / 8.0, size.y * j as f64 / 8.0))).collect();
            grid.sort_by(|a, b| (*a - size * 0.5).length().total_cmp(&(*b - size * 0.5).length()));
            let free = grid.into_iter().find(|at| {
                let win = vp.origin() + point(px(at.x as f32), px(at.y as f32));
                let on_part = boxes.iter().any(|(min, max)| at.cmpge(*min - glam::DVec2::splat(10.0)).all() && at.cmple(*max + glam::DVec2::splat(10.0)).all());
                !on_part && !dialog.is_some_and(|b| b.dilate(px(10.)).contains(&win))
            });
            let at = free.ok_or("no empty spot in the graphics area")?;
            return Ok(vp.origin() + point(px(at.x as f32), px(at.y as f32)));
        }
        let world = if let Some(w) = world {
            w
        } else if t.get("origin").is_some() {
            DVec3::ZERO
        } else if let Some(p) = ["face", "edge", "vertex"].iter().find_map(|k| t.get(*k)) {
            let local = mm(p).ok_or("a model point: [x, y, z] mm")?;
            match t.get("instance").and_then(Value::as_str) {
                Some(name) => {
                    let asm = self.assembly(cx).ok_or("no assembly")?;
                    let inst = asm.instances.iter().find(|i| i.name == name).ok_or_else(|| format!("no instance {name}"))?;
                    let pl = self.result.solved.placement(inst.id).unwrap_or(inst.placement);
                    glam::DQuat::from_array(pl.rotation) * local + DVec3::from(pl.translation)
                }
                None => local,
            }
        } else {
            return Err(format!("not a geometry target: {t}"));
        };
        let at = vp.project(world).ok_or("behind the camera")?;
        let size = vp.size();
        if at.x < 0.0 || at.y < 0.0 || at.x > size.x || at.y > size.y {
            return Err(format!("{t} is outside the graphics area"));
        }
        Ok(vp.origin() + point(px(at.x as f32), px(at.y as f32)))
    }

    /// The state the ui-parity runner records after each step.
    pub fn ui_state(&self, cx: &App) -> Value {
        let Some(asm) = self.assembly(cx) else { return json!({ "error": "no assembly" }) };
        let placed: HashMap<InstanceId, Placement> = self.result.solved.placements.iter().cloned().collect();
        let mate_state = |m: &Mate| match self.result.mates.iter().find(|s| s.id == m.id) {
            _ if m.suppressed => "suppressed",
            Some(s) if s.inactive => "suppressed",
            Some(s) if !s.ok => "error",
            _ => "ok",
        };
        let editing = self.mate.as_ref().and_then(|s| s.editing);
        let related = self.related_instances(cx);
        let row = |name: &str, state: &str, selected: bool, editing: bool, related: bool, hidden: bool| json!({ "name": name, "state": state, "selected": selected, "editing": editing, "related": related, "hidden": hidden });
        // As the list draws them: the assembly's own row, the Origin, the
        // top-level instances (a pattern's or a folder's row where its first
        // instance is), then "Mate features (n)" and its rows by creation.
        let tab = self.doc.read(cx).session.doc.tab(self.tab).map(|t| t.name.clone()).unwrap_or_default();
        let mut rows: Vec<Value> = vec![row(&tab, "ok", false, false, false, false), row("Origin", "ok", false, false, false, false)];
        let mut placed_groups: HashSet<MateId> = HashSet::new();
        for i in &asm.instances {
            if let Some(p) = asm.generated_by(i.id) {
                if placed_groups.insert(p.id) {
                    rows.push(row(&p.name, "ok", false, false, false, false));
                }
                continue;
            }
            if let Some(f) = asm.folder_of_instance(i.id) {
                if placed_groups.insert(f.id) {
                    rows.push(row(&f.name, "ok", false, false, false, false));
                }
                if !self.expanded.contains(&vec![InstanceId(f.id.0)]) {
                    continue;
                }
            }
            let selected = self.selected.contains(&i.id);
            rows.push(row(&i.name, if i.suppressed { "suppressed" } else { "ok" }, selected, false, selected || related.contains(&i.id), i.hidden));
            if let Some(n) = self.result.node(&[i.id]) {
                self.nested_state_rows(n, &mut rows, cx);
            }
        }
        let mate_selected = |id: MateId| self.selected_mate == Some(id) || (self.selected_mate.is_some() && self.ui.more_mates.contains(&id));
        let mut items: Vec<(MateId, Value)> = asm.connectors.iter().map(|c| (c.id, row(&c.name, "ok", mate_selected(c.id), false, mate_selected(c.id), c.hidden))).collect();
        items.extend(asm.mates.iter().filter(|m| asm.mate_generated_by(m.id).is_none()).map(|m| (m.id, row(&m.name, mate_state(m), mate_selected(m.id), editing == Some(m.id), mate_selected(m.id), m.hidden || !self.show_mates))));
        items.extend(asm.relations.iter().map(|r| (r.id, row(&r.name, "ok", mate_selected(r.id), false, mate_selected(r.id), false))));
        items.extend(asm.groups.iter().map(|g| (g.id, row(&g.name, "ok", mate_selected(g.id), false, mate_selected(g.id), g.hidden || !self.show_mates))));
        items.sort_by_key(|(id, _)| id.0);
        // A new mate's row while its dialog is open: selected, red until both connectors are picked.
        let new_mate = self.mate.as_ref().filter(|m| m.editing.is_none()).map(|m| m.dialog.read(cx)).filter(|d| !asm.mates.iter().any(|m| m.id == d.id));
        // Live: related (with the folder) once a connector is picked.
        let picked = new_mate.is_some_and(|d| d.a.is_some() || d.b.is_some());
        if let Some(d) = new_mate {
            items.push((d.id, row(&d.name, if d.a.is_some() && d.b.is_some() && !d.same_instance() { "ok" } else { "error" }, true, true, picked, !self.show_mates)));
        }
        let folder_related = self.selected_mate.is_some_and(|id| asm.mates.iter().any(|m| m.id == id)) || picked;
        rows.push(row(&format!("Mate features ({})", items.len()), "ok", false, false, folder_related, false));
        if !self.expanded.contains(&vec![MATE_FEATURES_COLLAPSED]) {
            rows.extend(items.into_iter().map(|(_, r)| r));
        }
        // A transform as live's occurrences give it: 16 numbers, row-major,
        // the translation in mm (rounded to 1e-6).
        let transform = |p: &Placement| {
            let m = glam::DMat4::from_rotation_translation(glam::DQuat::from_array(p.rotation), glam::DVec3::from(p.translation));
            let r = |x: f64| (x * 1e6).round() / 1e6;
            (0..4).flat_map(|row| (0..4).map(move |col| (row, col))).map(|(row, col)| r(m.col(col)[row] * if col == 3 && row < 3 { 1e3 } else { 1.0 })).collect::<Vec<f64>>()
        };
        // Live reads them over REST: a mate dialog's preview isn't stored yet.
        let previewing = self.mate.is_some();
        let instances: Vec<Value> = asm.instances.iter().map(|i| json!({ "name": i.name, "fixed": i.fixed, "transform": transform(if previewing { &i.placement } else { placed.get(&i.id).unwrap_or(&i.placement) }) })).collect();
        // As live's REST gives them: the mateType enum ("PIN_SLOT") and the feature status ("OK").
        let mates: Vec<Value> = asm.mates.iter().map(|m| json!({ "name": m.name, "type": m.mate_type.name().to_uppercase().replace(' ', "_"), "status": mate_state(m).to_uppercase() })).collect();
        // A mate selected in the list is a FEATURE (live), listed before the instances.
        let mates_selected = self.selected_mate.into_iter().chain(self.ui.more_mates.iter().copied().filter(|_| self.selected_mate.is_some()));
        let mut items: Vec<Value> = mates_selected.filter_map(|id| asm.mates.iter().find(|m| m.id == id)).map(|m| json!({ "kind": "FEATURE", "owner": m.name })).collect();
        items.extend(asm.instances.iter().filter(|i| self.selected.contains(&i.id)).map(|i| json!({ "kind": "INSTANCE", "owner": i.name })));
        // A mate dialog's picked connectors are selected (live: implicit mate connector FEATUREs).
        if let Some(d) = self.mate.as_ref().map(|s| s.dialog.read(cx)) {
            items.extend([&d.a, &d.b].into_iter().flatten().map(|_| json!({ "kind": "FEATURE", "owner": null })));
        }
        let mut dialogs: Vec<Value> = self
            .mate
            .as_ref()
            .map(|s| {
                let d = s.dialog.read(cx);
                let complete = d.a.is_some() && d.b.is_some() && !d.same_instance();
                json!({
                    "title": d.name, "titleState": if complete { "ok" } else { "error" }, "kind": "feature", "tabs": [],
                    "fields": d.ui_fields(),
                    "error": null, "ok": if complete { "enabled" } else { "disabled" }
                })
            })
            .into_iter()
            .collect();
        dialogs.extend(self.other_dialogs(cx));
        // A modal (Properties…, Instance properties…), as the Part Studio records one.
        let modal = crate::ui_script::open_dialog().filter(|d| d["kind"] == "modal");
        dialogs.extend(modal.clone());
        let dialog = dialogs.last().cloned();
        // The mate dialog's empty query box has the focus (live: field "Mate connectors").
        let focus = if modal.is_some() { Some(json!({ "kind": "other", "label": null, "text": null, "selection": null, "all": false })) } else { None };
        let focus = focus.or_else(|| self.mate.as_ref().map(|s| s.dialog.read(cx)).filter(|d| d.active.is_some() && self.ui.mate_field_focus).map(|d| json!({ "kind": "field", "label": if matches!(d.mate_type(), MateType::Tangent) { "Face, edge or vertex" } else if d.mate_type() == MateType::Width { "Tab mate connector(s)" } else { "Mate connectors" } }))).unwrap_or_else(|| json!({ "kind": "none" }));
        let history = self.history_state(cx);
        json!({
            "focus": focus,
            "dialog": dialog,
            "dialogs": dialogs,
            // An open menu (a row's, the graphics area's): its rows, as the Part Studio's.
            "list": crate::os_menu::ui_state(cx).or_else(|| self.ui.ctx_menu.as_ref().and_then(|(_, m, _)| crate::os_menu::menu_state(m, cx))),
            "selection": { "count": items.len(), "items": items },
            "features": { "rows": rows, "rollbackAfter": null },
            "parts": [],
            "message": self.message.as_ref().map(|(_, m)| m.clone()),
            "undo": { "undo": history.0, "redo": history.1 },
            "sketch": null,
            "assembly": { "instances": instances, "mates": mates }
        })
    }
}

impl AssemblyView {
    /// An expanded subassembly's rows as the list draws them (`push_children`): its instances, its patterns (their
    /// copies when open) and its own "Mate features (n)" with its mates (when open).
    fn nested_state_rows(&self, node: &InstanceNode, rows: &mut Vec<Value>, cx: &App) {
        if !self.expanded.contains(&node.path) {
            return;
        }
        let row = |name: &str, state: &str, hidden: bool| json!({ "name": name, "state": state, "selected": false, "editing": false, "related": false, "hidden": hidden });
        let sub = if node.kind == NodeKind::Assembly { self.node_assembly(&node.path, cx) } else { None };
        let generated = |c: &InstanceNode| sub.as_ref().is_some_and(|a| c.path.last().is_some_and(|id| a.generated_by(*id).is_some()));
        for c in node.children.iter().filter(|c| !generated(c)) {
            rows.push(row(&c.name, if c.suppressed { "suppressed" } else { "ok" }, c.hidden));
            self.nested_state_rows(c, rows, cx);
        }
        let Some(sub) = sub else { return };
        for pat in &sub.patterns {
            rows.push(row(&pat.name, "ok", false));
            let mut key = node.path.clone();
            key.push(InstanceId(NESTED_PATTERN_BIT | pat.id.0));
            if self.expanded.contains(&key) {
                for c in node.children.iter().filter(|c| c.path.last().is_some_and(|id| pat.generated.iter().any(|g| g.instance == *id))) {
                    rows.push(row(&c.name, if c.suppressed { "suppressed" } else { "ok" }, c.hidden));
                    self.nested_state_rows(c, rows, cx);
                }
            }
        }
        rows.push(row(&format!("Mate features ({})", sub.listed_mate_count()), "ok", false));
        let mut key = node.path.clone();
        key.push(NESTED_MATES_COLLAPSED);
        if !self.expanded.contains(&key) {
            rows.extend(sub.mates.iter().filter(|m| sub.mate_generated_by(m.id).is_none()).map(|m| row(&m.name, "ok", false)));
        }
    }

    /// The assembly's other open dialogs as the suite records them, in live's field format (as the mate dialog's):
    /// Mate connector, Group, the patterns / Mirror / Replicate, the relations, Replace instance, Insert.
    fn other_dialogs(&self, cx: &App) -> Vec<Value> {
        let mut out = Vec::new();
        let labeler = self.labeler(cx);
        let asm = self.assembly(cx);
        let names: HashMap<InstanceId, String> = asm.map(|a| a.instances.iter().map(|i| (i.id, i.name.clone())).collect()).unwrap_or_default();
        let mate_name = |id: MateId| asm.and_then(|a| a.mates.iter().find(|m| m.id == id).map(|m| m.name.clone())).unwrap_or_default();
        let query = |label: &str, items: Vec<String>, active: bool, missing: bool| json!({ "label": label, "kind": "query", "items": items, "active": active, "state": if missing { "error" } else { "ok" } });
        let number = |label: Value, text: String| json!({ "label": label, "kind": "number", "text": text, "state": "ok" });
        let quantity = |st: &Entity<crate::quantity_field::QuantityInput>| st.read(cx).text().to_string();
        let check = |label: &str, on: bool| json!({ "label": label, "kind": "boolean", "checked": on, "state": "ok" });
        let toggle = |on: bool| json!({ "label": "toggle", "kind": "toggle", "checked": on, "state": "ok" });
        let choice = |text: &str| json!({ "label": null, "kind": "enum", "text": text, "state": "ok" });
        let other = |text: &str| json!({ "label": null, "kind": "other", "text": text, "state": "ok" });
        let dialog = |title: String, complete: bool, ok: bool, fields: Vec<Value>| json!({ "title": title, "titleState": if complete { "ok" } else { "error" }, "kind": "feature", "tabs": [], "fields": fields, "error": null, "ok": if ok { "enabled" } else { "disabled" } });
        // An entity as its field names it ("Face of Part 1 <3>"; the origin "Origin").
        let entity = |r: &MateConnectorRef| -> String {
            if r.instance == InstanceId::ORIGIN && r.connector.is_none() {
                return "Origin".into();
            }
            let mut r = r.clone();
            if let Reference::Implicit { on, .. } = &r.reference {
                r.reference = (**on).clone();
            }
            labeler(&r)
        };
        if let Some(s) = &self.mc {
            // Rows as the Part Studio's Mate connector (its rows read the same in live), less owner and attachment.
            let slot = |k: usize| s.active && s.slot == k;
            let picked = |k: usize| if k == 0 { s.on.as_ref() } else { s.extra[k - 1].as_ref() }.map(&entity).into_iter().collect::<Vec<_>>();
            let mut fields = vec![choice(if s.between { "Between entities" } else { "On entity" }), query("Origin entity", picked(0), slot(0), false)];
            if s.between {
                fields.push(query("Between entity", picked(1), slot(1), false));
            }
            fields.push(check("Realign", s.realign));
            if s.realign {
                fields.push(query("Primary axis entity", picked(2), slot(2), false));
                fields.push(query("Secondary axis entity", picked(3), slot(3), false));
            }
            fields.push(check("Move", s.moving));
            if s.moving {
                // X / Y / Z rows show an axis icon for their label (read "").
                fields.extend(s.inputs[..3].iter().map(|st| number(json!(""), quantity(st))));
                fields.push(choice(["Rotate about X", "Rotate about Y", "Rotate about Z"][usize::from(s.rot_axis.min(2))]));
                fields.push(number(json!("Rotation angle"), quantity(&s.inputs[3])));
            }
            fields.push(toggle(s.flip));
            fields.push(other(""));
            let failed = self.result.connectors.iter().any(|c| c.id == s.id && c.message.is_some());
            let title = if s.implicit { "Mate connector".to_string() } else { s.name.clone() };
            out.push(dialog(title, s.on.is_some() && !failed, s.on.is_some(), fields));
        }
        if let Some(s) = &self.pattern {
            let seeds: Vec<String> = s.seeds.iter().filter_map(|i| names.get(i).cloned()).collect();
            let axis = |r: &Option<MateConnectorRef>, mirror: bool| -> Vec<String> {
                r.iter()
                    .map(|r| match (&r.reference, &r.connector) {
                        (Reference::Implicit { .. }, None) if mirror => "Mate connector".to_string(),
                        _ => entity(r),
                    })
                    .collect()
            };
            let mut fields = vec![query(if s.kind == PatternDialogKind::Mirror { "Instances to mirror" } else { "Instances" }, seeds, s.active.is_none(), false)];
            match s.kind {
                PatternDialogKind::Linear | PatternDialogKind::Circular => {
                    let circular = s.kind == PatternDialogKind::Circular;
                    fields.push(query(if circular { "Axis of pattern" } else { "Direction" }, axis(&s.axis, false), s.active == Some(0), false));
                    fields.push(number(json!(if circular { "Angle" } else { "Distance" }), quantity(&s.distance)));
                    fields.push(number(json!("Instance count"), quantity(&s.count)));
                    fields.push(toggle(s.opposite));
                    fields.push(check("Equal spacing", s.equal));
                    fields.push(check("Centered", s.centered));
                    if !circular {
                        fields.push(check("Second direction", s.second));
                        if s.second {
                            fields.push(query("Direction", axis(&s.axis2, false), s.active == Some(1), false));
                            fields.push(number(json!("Distance"), quantity(&s.distance2)));
                            fields.push(number(json!("Instance count"), quantity(&s.count2)));
                            fields.push(toggle(s.opposite2));
                        }
                    }
                }
                PatternDialogKind::Mirror => {
                    fields.push(query("Mirror plane", axis(&s.axis, true), s.active == Some(0), false));
                    fields.push(other("Mirror strategy…"));
                }
                PatternDialogKind::PartPattern | PatternDialogKind::Replicate => {
                    fields.push(query("Mate to be matched", s.mate.map(mate_name).into_iter().collect(), false, false));
                    if s.kind == PatternDialogKind::Replicate {
                        use noshape_model::assembly::ReplicateScope;
                        let scope = self.replicate_scope(s, cx);
                        fields.push(choice(scope.label()));
                        let refs: Vec<String> = s
                            .scope_refs
                            .iter()
                            .map(|r| {
                                let part = names.get(&r.instance).cloned().unwrap_or_default();
                                match &r.entity {
                                    Some(Reference::Face(_)) => format!("Face of {part}"),
                                    Some(Reference::Edge(_)) => format!("Edge of {part}"),
                                    _ => part,
                                }
                            })
                            .collect();
                        let empty = refs.is_empty();
                        fields.push(query(if scope == ReplicateScope::EdgesOnFace { "Faces to find match in" } else { "Parts to find match in" }, refs, s.active == Some(2), empty));
                    }
                }
            }
            let complete = !s.seeds.is_empty()
                && match s.kind {
                    PatternDialogKind::Linear | PatternDialogKind::Circular | PatternDialogKind::Mirror => s.axis.is_some(),
                    PatternDialogKind::PartPattern => s.mate.is_some(),
                    PatternDialogKind::Replicate => s.mate.is_some() && !s.scope_refs.is_empty(),
                };
            out.push(dialog(s.name.clone(), complete, complete, fields));
        }
        if let Some((editing, ids)) = &self.ui.group {
            let title = match editing {
                Some(g) => asm.and_then(|a| a.groups.iter().find(|x| x.id == *g).map(|x| x.name.clone())).unwrap_or_else(|| "Group".into()),
                None => asm.map(|a| a.next_feature_name("Group")).unwrap_or_else(|| "Group 1".into()),
            };
            let complete = ids.len() >= 2 || !self.ui.group_patterns.is_empty();
            out.push(dialog(title, complete, true, vec![query("Instances", ids.iter().filter_map(|i| names.get(i).cloned()).collect(), true, false)]));
        }
        if let Some(r) = &self.relation {
            let mates: Vec<String> = [r.a, r.b].into_iter().flatten().map(mate_name).collect();
            let complete = r.a.is_some() && r.b.is_some();
            let fields = vec![
                choice(noshape_ops::assembly::relation_name(r.kind)),
                query("Mate(s)", mates, true, false),
                number(json!(noshape_ops::assembly::relation_value_label(r.kind)), r.ratio.read(cx).value().to_string()),
                check("Reverse direction", r.reverse),
            ];
            out.push(dialog(self.relation_name(cx), complete, complete, fields));
        }
        if let Some(s) = &self.replace_panel {
            let fields = vec![
                query("Instances to replace", s.instances.iter().filter_map(|i| names.get(i).cloned()).collect(), !s.picking, false),
                check("Replace all instances", s.all),
                other(s.replacement.as_ref().map_or("Select replacement component", |(_, n)| n.as_str())),
            ];
            let complete = s.replacement.is_some();
            out.push(dialog("Replace instance".into(), complete, complete, fields));
        }
        // Origin of new Part Studio (live assembly/114, 542): its one field, the Origin when opened from its row.
        if let Some(o) = self.ui.ps_in_context {
            let chip = o.chip(|id| asm.and_then(|a| a.connectors.iter().find(|c| c.id == id).map(|c| c.name.clone())));
            out.push(dialog("Origin of new Part Studio".into(), true, true, vec![query("Select origin/mate connector", chip.into_iter().collect(), true, false)]));
        }
        // Dynamic suppression (live assembly/104): one expression.
        if let Some((_, input, _)) = &self.ui.dynamic_suppression {
            out.push(dialog("Dynamic suppression".into(), true, true, vec![number(json!("Unsuppressed if"), input.read(cx).value().to_string())]));
        }
        // Driving mate selector (live assembly/105–107): the mates listed.
        if let Some(d) = &self.ui.np_driver {
            out.push(dialog("Driving mate selector".into(), true, true, vec![query("Mates to drive positions", d.mates.iter().map(|m| mate_name(*m)).collect(), true, false)]));
        }
        // Reference manager (live assembly/111–112): its two tabs.
        if let Some(r) = &self.ui.ref_manager {
            out.push(json!({ "title": "Reference manager", "titleState": "ok", "kind": "other", "tabs": [{ "label": "Update to latest", "selected": r.tab == 0 }, { "label": "Selective update", "selected": r.tab == 1 }], "fields": [], "error": null, "ok": null }));
        }
        if let Some(p) = self.section_props(cx) {
            out.push(crate::section_dialog::state(&p));
        }
        if self.insert_open {
            // A list panel, not a feature dialog: its title (its rows aren't fields).
            out.push(json!({ "title": "Insert parts and assemblies", "titleState": "ok", "kind": "other", "tabs": [], "fields": [], "error": null, "ok": null }));
        }
        out
    }
}
