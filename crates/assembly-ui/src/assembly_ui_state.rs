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
    /// `vertex` [x, y, z] (mm, assembly coordinates), `origin`.
    pub fn ui_target_point(&self, t: &Value, cx: &App) -> Result<Point<Pixels>, String> {
        let vp = self.viewport.read(cx);
        let mm = |v: &Value| -> Option<DVec3> {
            let a = v.as_array()?;
            Some(DVec3::new(a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?) * 1e-3)
        };
        let world = if t.get("origin").is_some() {
            DVec3::ZERO
        } else if let Some(p) = ["face", "edge", "vertex"].iter().find_map(|k| t.get(*k)) {
            mm(p).ok_or("a model point: [x, y, z] mm")?
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
        }
        let mate_selected = |id: MateId| self.selected_mate == Some(id) || (self.selected_mate.is_some() && self.ui.more_mates.contains(&id));
        let mut items: Vec<(MateId, Value)> = asm.connectors.iter().map(|c| (c.id, row(&c.name, "ok", mate_selected(c.id), false, mate_selected(c.id), c.hidden))).collect();
        items.extend(asm.mates.iter().filter(|m| asm.mate_generated_by(m.id).is_none()).map(|m| (m.id, row(&m.name, mate_state(m), mate_selected(m.id), editing == Some(m.id), mate_selected(m.id), m.hidden || !self.show_mates))));
        items.extend(asm.relations.iter().map(|r| (r.id, row(&r.name, "ok", mate_selected(r.id), false, mate_selected(r.id), false))));
        items.extend(asm.groups.iter().map(|g| (g.id, row(&g.name, "ok", mate_selected(g.id), false, mate_selected(g.id), g.hidden || !self.show_mates))));
        items.sort_by_key(|(id, _)| id.0);
        // A new mate's row while its dialog is open: selected, red until both connectors are picked.
        let new_mate = self.mate.as_ref().filter(|m| m.editing.is_none()).map(|m| m.dialog.read(cx)).filter(|d| !asm.mates.iter().any(|m| m.id == d.id));
        if let Some(d) = new_mate {
            items.push((d.id, row(&d.name, if d.a.is_some() && d.b.is_some() { "ok" } else { "error" }, true, true, false, !self.show_mates)));
        }
        let folder_related = self.selected_mate.is_some_and(|id| asm.mates.iter().any(|m| m.id == id));
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
        let instances: Vec<Value> = asm.instances.iter().map(|i| json!({ "name": i.name, "fixed": i.fixed, "transform": transform(placed.get(&i.id).unwrap_or(&i.placement)) })).collect();
        // As live's REST gives them: the mateType enum ("PIN_SLOT") and the feature status ("OK").
        let mates: Vec<Value> = asm.mates.iter().map(|m| json!({ "name": m.name, "type": m.mate_type.name().to_uppercase().replace(' ', "_"), "status": mate_state(m).to_uppercase() })).collect();
        // A mate selected in the list is a FEATURE (live), listed before the instances.
        let mates_selected = self.selected_mate.into_iter().chain(self.ui.more_mates.iter().copied().filter(|_| self.selected_mate.is_some()));
        let mut items: Vec<Value> = mates_selected.filter_map(|id| asm.mates.iter().find(|m| m.id == id)).map(|m| json!({ "kind": "FEATURE", "owner": m.name })).collect();
        items.extend(asm.instances.iter().filter(|i| self.selected.contains(&i.id)).map(|i| json!({ "kind": "INSTANCE", "owner": i.name })));
        let dialog = self.mate.as_ref().map(|s| {
            let d = s.dialog.read(cx);
            let side = |c: &Option<MateConnectorRef>| c.as_ref().and_then(|c| asm.instance(c.instance)).map(|i| i.name.clone());
            let complete = d.a.is_some() && d.b.is_some();
            let picked: Vec<Vec<String>> = if d.mate_type() == MateType::Tangent { vec![side(&d.a).into_iter().collect(), side(&d.b).into_iter().collect()] } else { vec![[&d.a, &d.b].into_iter().filter_map(side).collect()] };
            json!({
                "title": d.name, "titleState": if complete { "ok" } else { "error" }, "kind": "feature", "tabs": [],
                "fields": d.ui_fields(&picked),
                "error": null, "ok": if complete { "enabled" } else { "disabled" }
            })
        });
        // The mate dialog's empty query box has the focus (live: field "Mate connectors").
        let focus = self.mate.as_ref().map(|s| s.dialog.read(cx)).filter(|d| d.active.is_some()).map(|d| json!({ "kind": "field", "label": if matches!(d.mate_type(), MateType::Tangent) { "Face, edge or vertex" } else if d.mate_type() == MateType::Width { "Tab mate connector(s)" } else { "Mate connectors" } })).unwrap_or_else(|| json!({ "kind": "none" }));
        let doc = self.doc.read(cx);
        json!({
            "focus": focus,
            "dialog": dialog,
            "dialogs": dialog.iter().cloned().collect::<Vec<_>>(),
            "list": null,
            "selection": { "count": items.len(), "items": items },
            "features": { "rows": rows, "rollbackAfter": null },
            "parts": [],
            "message": self.message.as_ref().map(|(_, m)| m.clone()),
            "undo": { "undo": doc.can_undo(), "redo": doc.can_redo() },
            "sketch": null,
            "assembly": { "instances": instances, "mates": mates }
        })
    }
}
