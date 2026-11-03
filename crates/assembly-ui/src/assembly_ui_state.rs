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
        let mut rows: Vec<Value> = asm.instances.iter().map(|i| json!({ "name": i.name, "state": if i.suppressed { "suppressed" } else { "ok" }, "selected": self.selected.contains(&i.id), "editing": false, "hidden": i.hidden })).collect();
        rows.extend(asm.mates.iter().map(|m| json!({ "name": m.name, "state": mate_state(m), "selected": self.selected_mate == Some(m.id), "editing": editing == Some(m.id), "hidden": false })));
        // A transform as live's occurrences give it: 16 numbers, row-major,
        // the translation in mm (rounded to 1e-6).
        let transform = |p: &Placement| {
            let m = glam::DMat4::from_rotation_translation(glam::DQuat::from_array(p.rotation), glam::DVec3::from(p.translation));
            let r = |x: f64| (x * 1e6).round() / 1e6;
            (0..4).flat_map(|row| (0..4).map(move |col| (row, col))).map(|(row, col)| r(m.col(col)[row] * if col == 3 && row < 3 { 1e3 } else { 1.0 })).collect::<Vec<f64>>()
        };
        let instances: Vec<Value> = asm.instances.iter().map(|i| json!({ "name": i.name, "fixed": i.fixed, "transform": transform(placed.get(&i.id).unwrap_or(&i.placement)) })).collect();
        let mates: Vec<Value> = asm.mates.iter().map(|m| json!({ "name": m.name, "type": m.mate_type.name(), "status": mate_state(m) })).collect();
        let mut items: Vec<Value> = asm.instances.iter().filter(|i| self.selected.contains(&i.id)).map(|i| json!({ "kind": "INSTANCE", "owner": i.name })).collect();
        items.extend(self.selected_mate.and_then(|id| asm.mates.iter().find(|m| m.id == id)).map(|m| json!({ "kind": "MATE", "owner": m.name })));
        let dialog = self.mate.as_ref().map(|s| {
            let d = s.dialog.read(cx);
            let side = |c: &Option<MateConnectorRef>| c.as_ref().and_then(|c| asm.instance(c.instance)).map(|i| Value::String(i.name.clone()));
            let picked: Vec<Value> = [&d.a, &d.b].into_iter().filter_map(side).collect();
            let complete = picked.len() == 2;
            json!({
                "title": d.name, "titleState": if complete { "ok" } else { "error" }, "kind": "feature", "tabs": [],
                "fields": [{ "label": "Mate connectors", "kind": "query", "items": picked, "active": d.active.is_some(), "state": "ok" }],
                "error": null, "ok": if complete { "enabled" } else { "disabled" }
            })
        });
        let doc = self.doc.read(cx);
        json!({
            "focus": { "kind": "none" },
            "dialog": dialog,
            "dialogs": dialog.iter().cloned().collect::<Vec<_>>(),
            "list": null,
            "selection": { "count": items.len(), "items": items },
            "features": { "rows": rows, "rollbackAfter": null },
            "parts": null,
            "message": self.message.as_ref().map(|(_, m)| m.clone()),
            "undo": { "undo": doc.can_undo(), "redo": doc.can_redo() },
            "sketch": null,
            "assembly": { "instances": instances, "mates": mates }
        })
    }
}
