//! The interaction parity suite's view of the Part Studio
//! (tests/ui-parity/README.md): the recorded state, read from the studio's
//! own model, and the window point of a geometry target (a plane, a model
//! point, a sketch point) for the driver to click.

use super::*;
use glam::{DVec2, DVec3};
use serde_json::{Value, json};

impl PartStudioView {
    /// `state` after a step.
    pub fn ui_state(&self, window: &Window, cx: &App) -> Value {
        let dialog = self.edit.as_ref().map(|e| e.dialog.read(cx));
        let filter_focused = self.ui.filter.as_ref().is_some_and(|f| f.0.read(cx).focus_handle(cx).is_focused(window));
        let dim_box = self.dim_input.as_ref().map(|d| d.1.read(cx)).filter(|s| s.focus_handle(cx).is_focused(window));
        let focus = if let Some(f) = dialog.and_then(|d| d.ui_focus(window, cx)) {
            f
        } else if let Some(st) = dim_box {
            let text = st.value().to_string();
            let sel = st.selected_range();
            let chars = |b: usize| text.get(..b.min(text.len())).map_or(0, |t| t.chars().count());
            let (a, b) = (chars(sel.start), chars(sel.end));
            json!({ "kind": "field", "label": "dimension", "text": text, "selection": [a, b], "all": a == 0 && b == text.chars().count() && b > 0 })
        } else if filter_focused {
            let text = self.ui.filter.as_ref().map(|f| f.0.read(cx).value().to_string()).unwrap_or_default();
            json!({ "kind": "filter", "label": "Filter by name or type", "text": text, "selection": null, "all": false })
        } else {
            json!({ "kind": "none", "label": null, "text": null, "selection": null, "all": false })
        };
        let list = crate::os_menu::ui_state(cx)
            .or_else(|| dialog.and_then(|d| d.ui_list()))
            .or_else(|| self.dim_completion_state())
            .or_else(|| self.ui.cell_completion.as_ref().map(|c| c.ui_state()))
            .or_else(crate::ui_script::open_list);
        let feature_name = |id: FeatureId| self.studio(cx).and_then(|s| s.features.iter().find(|f| f.id == id).map(|f| f.name.clone())).unwrap_or_default();
        let items: Vec<Value> = self
            .selection
            .iter()
            .map(|r| {
                let body = |r: &Reference| self.body_of(r).and_then(|b| self.display.body(&b)).map(|b| b.name.clone()).unwrap_or_default();
                let (kind, owner) = match r {
                    Reference::Face(_) => ("FACE", body(r)),
                    Reference::Edge(_) => ("EDGE", body(r)),
                    Reference::Vertex(_) => ("VERTEX", body(r)),
                    Reference::Body(_) => ("BODY", body(r)),
                    Reference::Plane(p) => ("PLANE", format!("{p:?}")),
                    Reference::Origin => ("VERTEX", "Origin".to_string()),
                    Reference::SketchEntity { sketch, .. } => ("SKETCH_ENTITY", feature_name(*sketch)),
                    Reference::SketchRegion { sketch, .. } => ("FACE", feature_name(*sketch)),
                    Reference::Sketch(f) | Reference::Feature(f) => ("FEATURE", feature_name(*f)),
                    Reference::FeaturePlane(f) | Reference::ConstructionPlane { feature: f, .. } => ("PLANE", feature_name(*f)),
                    Reference::MateConnector { feature, .. } => ("MATE_CONNECTOR", feature_name(*feature)),
                    _ => ("OTHER", String::new()),
                };
                json!({ "kind": kind, "owner": owner })
            })
            .chain(self.selected_features.iter().filter(|f| !self.selection.contains(&Reference::Feature(**f))).map(|f| json!({ "kind": "FEATURE", "owner": feature_name(*f) })))
            .collect();
        let (rows, rollback_after) = match self.studio(cx) {
            Some(s) => {
                let rollback = s.rollback_index();
                let editing = self.edit.as_ref().map(|e| e.dialog.read(cx).feature.id);
                let rows: Vec<Value> = s
                    .features
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        let status = self.display.feature_state(f.id).map(|s| s.status);
                        let state = if i >= rollback {
                            "rolled-back"
                        } else if f.suppressed {
                            "suppressed"
                        } else {
                            match status {
                                Some(FeatureStatus::Error) => "error",
                                Some(FeatureStatus::Warning) => "warning",
                                Some(FeatureStatus::Info) => "info",
                                Some(FeatureStatus::Suppressed) => "suppressed",
                                _ => "ok",
                            }
                        };
                        json!({ "name": self.shown_feature_name(f, cx), "state": state, "selected": self.selected_features.contains(&f.id), "editing": editing == Some(f.id) })
                    })
                    .collect();
                let after = rollback.checked_sub(1).and_then(|i| s.features.get(i)).map(|f| self.shown_feature_name(f, cx));
                (rows, after)
            }
            None => (Vec::new(), None),
        };
        let parts: Vec<String> = self.display.bodies.iter().map(|b| b.name.clone()).collect();
        let message = self.message.as_ref().filter(|(_, m)| self.edit.is_some() || m != behaviors::NO_INTERSECTION).map(|(_, m)| m.clone());
        json!({
            "focus": focus,
            "dialog": dialog.map(|d| d.ui_state(window, cx)),
            "list": list,
            "selection": { "count": items.len(), "items": items },
            "features": { "rows": rows, "rollbackAfter": rollback_after },
            "parts": parts,
            "message": message,
            "tooltip": null,
        })
    }

    /// Where target `t` (a geometry target of tests/ui-parity/README.md)
    /// shows, in window coordinates.
    pub fn ui_target_point(&self, t: &Value, cx: &App) -> Result<Point<Pixels>, String> {
        let vp = self.viewport.read(cx);
        let mm = |v: &Value| -> Option<DVec3> {
            let a = v.as_array()?;
            Some(DVec3::new(a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2).and_then(Value::as_f64).unwrap_or(0.0)) * 1e-3)
        };
        let world = if let Some(name) = t.get("plane").and_then(Value::as_str) {
            let p = self.display.planes.iter().find(|p| p.name == name && !p.hidden).ok_or_else(|| format!("no shown plane {name}"))?;
            // Inside the rectangle near its labelled corner, clear of the other planes.
            p.corner(-0.85, 0.85)
        } else if t.get("origin").is_some() {
            DVec3::ZERO
        } else if let Some(p) = t.get("sketch_point") {
            let ed = self.sketch.as_ref().ok_or("not in a sketch")?;
            let p = mm(p).ok_or("sketch_point: [x, y] mm")?;
            ed.frame.to_world(DVec2::new(p.x, p.y))
        } else if let Some(p) = ["face", "edge", "vertex"].iter().find_map(|k| t.get(*k)) {
            mm(p).ok_or("a model point: [x, y, z] mm")?
        } else if t.get("empty").is_some() {
            let size = vp.size();
            let free = (1..8).flat_map(|i| (1..8).map(move |j| DVec2::new(size.x * i as f64 / 8.0, size.y * j as f64 / 8.0))).find(|at| {
                let (o, d) = vp.ray(*at);
                noshape_model::interaction::entities_along_ray(&self.display, o, d, 0.0, &|_| false, 1).is_empty()
            });
            let at = free.ok_or("no empty spot in the graphics area")?;
            return Ok(vp.origin() + point(px(at.x as f32), px(at.y as f32)));
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
}

impl PartStudioView {
    /// A regeneration is under way (the driver waits for it after each step).
    pub fn ui_busy(&self) -> bool {
        self.pending.is_some() || self.context.is_none()
    }

    /// Redraw the studio and its dialog, so every named control records where it is now.
    pub fn ui_redraw(&self, cx: &mut Context<Self>) {
        if let Some(e) = &self.edit {
            e.dialog.update(cx, |_, cx| cx.notify());
        }
        cx.notify();
    }
}
