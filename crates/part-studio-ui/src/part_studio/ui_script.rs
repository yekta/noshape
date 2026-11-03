//! The interaction parity suite's view of the Part Studio
//! (tests/ui-parity/README.md): the recorded state, read from the studio's
//! own model, and the window point of a geometry target (a plane, a model
//! point, a sketch point) for the driver to click.

use super::*;
use glam::{DVec2, DVec3};
use gpui_kit::component::input::InputState;
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
            // Live: an input outside the dialogs.
            let mut f = crate::feature_dialog::field_focus("", st);
            f["kind"] = "other".into();
            f["label"] = Value::Null;
            f
        } else if let Some(st) = self.unlabelled_input_focused(window, cx) {
            // A row being renamed, the folder name popup: inputs without a label (live).
            let mut f = crate::feature_dialog::field_focus("", st);
            f["label"] = Value::Null;
            f
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
        // In a sketch the selection is its own, with what the Dimension tool picked (drawn selected); in a feature
        // dialog what its active field picked (live).
        let in_sketch = self.sketch.as_ref().map(|ed| {
            let f = ed.feature;
            let picks = if ed.tool == crate::sketch_mode::Tool::Dimension { ed.tool_picks() } else { Vec::new() };
            ed.selection.iter().chain(&picks).map(|e| Reference::SketchEntity { sketch: f, entity: e.0 }).chain(ed.selected_regions.iter().map(|k| Reference::SketchRegion { sketch: f, key: k.clone() })).chain(ed.model_selection.iter().cloned()).collect::<Vec<_>>()
        });
        let picked = in_sketch.or_else(|| dialog.map(|d| d.active_refs()));
        let items: Vec<Value> = picked
            .as_ref()
            .unwrap_or(&self.selection)
            .iter()
            .map(|r| {
                let (kind, owner) = self.selection_kind_owner(r, cx);
                // Live gives the open sketch's own entities no owner.
                let own = matches!(r, Reference::SketchEntity { sketch, .. } | Reference::SketchRegion { sketch, .. } if self.sketch.as_ref().is_some_and(|ed| ed.feature == *sketch));
                json!({ "kind": kind, "owner": if own { Value::Null } else { owner.into() } })
            })
            .chain(self.selected_features.iter().filter(|f| picked.is_none() && !self.selection.contains(&Reference::Feature(**f))).map(|f| json!({ "kind": "FEATURE", "owner": feature_name(*f) })))
            .collect();
        // The edited row is drawn selected (live marks it selected and editing).
        let (rows, rollback_after) = match self.studio(cx) {
            Some(s) => {
                // The rows as the list draws them (folders, collapsed ones closed; the bar's place).
                let (items, rollback) = self.list_items(s, cx);
                let editing = self.edit.as_ref().map(|e| e.dialog.read(cx).feature.id);
                let related = self.related_features(cx);
                let index = |id: FeatureId| s.features.iter().position(|f| f.id == id);
                let mut rows: Vec<Value> = Vec::new();
                let mut after = None;
                for item in &items {
                    match item {
                        super::ListItem::Feature(_, id) => {
                            let (Some(i), Some(f)) = (index(*id), s.feature(*id)) else { continue };
                            // As the row draws it: the edited feature is red while its dialog holds an invalid value.
                            let status = if editing == Some(f.id) && self.edit_invalid(cx) { Some(FeatureStatus::Error) } else { self.display.feature_state(f.id).map(|s| s.status) };
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
                            rows.push(json!({ "name": self.shown_feature_name(f, cx), "state": state, "selected": self.selected_features.contains(&f.id) || editing == Some(f.id), "editing": editing == Some(f.id), "related": related.contains(&f.id), "hidden": f.hidden && !(editing == Some(f.id) || self.edit_uses(f.id, cx) || self.rolled_to(f.id, cx)) }));
                        }
                        // A new feature's row while its dialog is open (red while it fails, as drawn).
                        super::ListItem::Placeholder => {
                            let state = if self.edit_invalid(cx) { "error" } else { "ok" };
                            rows.push(json!({ "name": self.placeholder_name(cx), "state": state, "selected": true, "editing": true, "related": editing.is_some_and(|f| related.contains(&f)), "hidden": false }));
                        }
                        // A new folder's row is related-highlighted until the next click (live ui 10-25 b (1)).
                        super::ListItem::Folder(_, fd) => rows.push(json!({ "name": fd.name, "state": "ok", "selected": self.ui.selected_folder == Some(fd.id), "editing": false, "related": self.ui.fresh_folder == Some(fd.id), "hidden": false })),
                        // At the top the bar sits under Default geometry (live).
                        super::ListItem::Rollback(..) => after = Some(rows.last().and_then(|r| r["name"].as_str()).unwrap_or("Default geometry").to_string()),
                        _ => {}
                    }
                }
                (rows, after)
            }
            None => (Vec::new(), None),
        };
        let parts: Vec<String> = self.display.bodies.iter().map(|b| b.name.clone()).collect();
        let message = self.message.as_ref().filter(|(_, m)| self.edit.is_some() || m != behaviors::NO_INTERSECTION).map(|(_, m)| m.clone());
        // The canceled-sketch toast's Restore link is part of its text in live.
        let message = message.map(|m| if self.canceled_edit.as_ref().is_some_and(|c| c.3 == m) { format!("{m} Restore") } else { m });
        json!({
            "focus": focus,
            "dialog": dialog.map(|d| d.ui_state(window, cx)),
            "list": list,
            "selection": { "count": items.len(), "items": items },
            "features": { "rows": rows, "rollbackAfter": rollback_after },
            "parts": parts,
            "message": message,
            "tooltip": null,
            "undo": self.ui_undo_state(cx),
            "sketch": self.ui_sketch_state(cx),
        })
    }

    /// `undo`: whether the toolbar's Undo and Redo do something (in a sketch
    /// or dialog, its own history).
    fn ui_undo_state(&self, cx: &App) -> Value {
        let (undo, redo) = if self.edit.is_some() {
            self.draft_history_state()
        } else {
            let d = self.doc.read(cx);
            (d.can_undo(), d.can_redo())
        };
        json!({ "undo": undo, "redo": redo })
    }

    /// `sketch`: the open sketch's tool and content as Onshape's sketch
    /// lists them (null outside a sketch).
    fn ui_sketch_state(&self, cx: &App) -> Value {
        use noshape_sketch::{ConstraintKind as K, EntityKind};
        let Some(data) = self.sketch_data(cx) else { return Value::Null };
        // The Sketch dialog before its plane is picked: no sketch model yet.
        let Some(ed) = self.sketch.as_ref() else { return json!({ "tool": null }) };
        let sk = &data.sketch;
        // The first active toolbar button (live): the tool, else Construction while it is on.
        let tool = (ed.tool != crate::sketch_mode::Tool::Select)
            .then(|| crate::ui::onshape_title(ed.tool.label()).split(" (").next().unwrap_or_default().to_string())
            .or_else(|| ed.construction.then(|| "Construction".to_string()));
        let owned: std::collections::HashSet<_> = sk.entities().filter(|e| e.kind() != EntityKind::Point).flat_map(|e| e.point_refs()).collect();
        let mut entities: Vec<String> = sk
            .entities()
            .filter(|e| !e.external && !e.reference_only && !(e.kind() == EntityKind::Point && owned.contains(&e.id)))
            .map(|e| {
                let kind = match e.kind() {
                    EntityKind::Line => "line",
                    EntityKind::Circle => "circle",
                    EntityKind::Arc => "arc",
                    EntityKind::Ellipse => "ellipse",
                    EntityKind::EllipticalArc => "elliptical arc",
                    EntityKind::BSpline | EntityKind::FitSpline => "spline",
                    EntityKind::Conic => "conic",
                    EntityKind::Point => "point",
                };
                if e.construction { format!("construction {kind}") } else { kind.to_string() }
            })
            .collect();
        entities.sort();
        // A point the tools share between curves is a Coincident in live.
        let mut users: std::collections::HashMap<_, usize> = std::collections::HashMap::new();
        for e in sk.entities().filter(|e| e.kind() != EntityKind::Point && !e.external) {
            let mut pts = e.point_refs().to_vec();
            pts.sort();
            pts.dedup();
            for p in pts {
                *users.entry(p).or_default() += 1;
            }
        }
        let mut constraints: Vec<String> = users.values().filter(|n| **n > 1).flat_map(|n| std::iter::repeat_n("COINCIDENT".to_string(), n - 1)).collect();
        let mut dimensions: Vec<(String, f64)> = Vec::new();
        for c in sk.constraints().filter(|c| c.driving) {
            let mm = |v: f64| (v * 1e3 * 1e6).round() / 1e6;
            match &c.kind {
                K::Length { value, .. } | K::Distance { value, .. } | K::HorizontalDistance { value, .. } | K::VerticalDistance { value, .. } => dimensions.push(("LINEAR".into(), mm(*value))),
                K::Radius { value, .. } => dimensions.push(("RADIUS".into(), mm(*value))),
                K::Diameter { value, .. } => dimensions.push(("DIAMETER".into(), mm(*value))),
                K::Angle { value, .. } | K::ArcAngle { value, .. } => dimensions.push(("ANGLE".into(), (value.to_degrees() * 1e6).round() / 1e6)),
                k if k.is_dimension() => dimensions.push((k.name().to_uppercase().replace(' ', "_"), c.kind.value().unwrap_or(0.0))),
                k => constraints.push(k.name().to_uppercase().replace(' ', "_")),
            }
        }
        constraints.sort();
        dimensions.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let dimensions: Vec<Value> = dimensions.into_iter().map(|(kind, value)| json!({ "kind": kind, "value": value })).collect();
        json!({ "tool": tool, "entities": entities, "constraints": constraints, "dimensions": dimensions })
    }

    /// A selected entity's kind and owner as the parity state names them (an inferred point: its entity's).
    fn selection_kind_owner(&self, r: &Reference, cx: &App) -> (&'static str, String) {
        let feature_name = |id: FeatureId| self.studio(cx).and_then(|s| s.feature(id).map(|f| f.name.clone())).unwrap_or_default();
        // The preview may have consumed the entity: then the model before the edited feature has it.
        let body = |r: &Reference| {
            let before = self.ui.preview_before.as_deref();
            std::iter::once(&*self.display).chain(before).find_map(|d| noshape_ops::studio::body_of(d, r).and_then(|b| d.body(&b)).map(|b| b.name.clone())).unwrap_or_default()
        };
        match r {
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
            Reference::Implicit { on, .. } => self.selection_kind_owner(on, cx),
            Reference::QueryVariable(_) => ("OTHER", String::new()),
        }
    }

    /// A point of plane `p`'s rectangle in front of every other shown plane and part, near its labelled corner.
    fn visible_plane_point(&self, p: &noshape_model::display::PlaneDisplay, cx: &App) -> Option<DVec3> {
        let vp = self.viewport.read(cx);
        let mut grid: Vec<(f64, f64)> = (-6..=6).flat_map(|i| (-6..=6).map(move |j| (i as f64 * 0.15, j as f64 * 0.15))).collect();
        grid.sort_by(|a, b| ((a.0 + 0.8).hypot(a.1 - 0.8)).total_cmp(&(b.0 + 0.8).hypot(b.1 - 0.8)));
        grid.into_iter().map(|(u, v)| p.corner(u, v)).find(|w| {
            let Some(at) = vp.project(*w) else { return false };
            let (o, d) = vp.ray(at);
            let depth = (*w - o).dot(d.normalize());
            let plane_in_front = self.display.planes.iter().filter(|q| !q.hidden && q.reference != p.reference).any(|q| {
                let f = &q.frame;
                let denom = d.normalize().dot(f.normal);
                if denom.abs() < 1e-9 {
                    return false;
                }
                let t = (f.origin - o).dot(f.normal) / denom;
                let local = f.to_local(o + d.normalize() * t) - q.center;
                // Level with this plane (where they cross) counts as covered: the pick can go either way there.
                t > 0.0 && t < depth * (1.0 + 1e-6) && local.x.abs() <= q.half_width && local.y.abs() <= q.half_height
            });
            let part_in_front = noshape_model::interaction::entities_along_ray(&self.display, o, d, 0.0, &|_| false, 1).first().is_some_and(|h| h.depth < depth - 1e-9);
            // Clear of the other planes' outlines too: the pick takes a line within its radius.
            let near_outline = self.display.planes.iter().filter(|q| !q.hidden && q.reference != p.reference).any(|q| {
                let c = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(u, v)| vp.project(q.corner(u, v)));
                (0..4).any(|k| match (c[k], c[(k + 1) % 4]) {
                    (Some(a), Some(b)) => {
                        let t = ((at - a).dot(b - a) / (b - a).length_squared().max(1e-9)).clamp(0.0, 1.0);
                        (a + (b - a) * t).distance(at) < 15.0
                    }
                    _ => false,
                })
            });
            !plane_in_front && !part_in_front && !near_outline
        })
    }

    /// The focused rename or folder name input, if any.
    fn unlabelled_input_focused<'a>(&self, window: &Window, cx: &'a App) -> Option<&'a InputState> {
        let inputs = self.renaming.as_ref().map(|r| &r.1).into_iter().chain(self.folder_prompt.as_ref().map(|f| &f.0)).chain(self.ui.part_renaming.as_ref().map(|r| &r.1)).chain(self.ui.dg_renaming.as_ref().map(|r| &r.1));
        inputs.map(|e| e.read(cx)).find(|st| st.focus_handle(cx).is_focused(window))
    }

    /// Where target `t` (a geometry target of tests/ui-parity/README.md)
    /// shows, in window coordinates.
    pub fn ui_target_point(&self, t: &Value, cx: &App) -> Result<Point<Pixels>, String> {
        let vp = self.viewport.read(cx);
        let mm = |v: &Value| -> Option<DVec3> {
            let a = v.as_array()?;
            Some(DVec3::new(a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2).and_then(Value::as_f64).unwrap_or(0.0)) * 1e-3)
        };
        if let Some(name) = t.get("view_cube").and_then(Value::as_str) {
            let cell = match name {
                "Top" => [0, 0, 1],
                "Bottom" => [0, 0, -1],
                "Front" => [0, -1, 0],
                "Back" => [0, 1, 0],
                "Right" => [1, 0, 0],
                "Left" => [-1, 0, 0],
                "Isometric" => [1, -1, 1],
                _ => return Err(format!("no view cube cell {name}")),
            };
            let origin = self.ui.cube_origin.ok_or("the view cube isn't drawn")?;
            let rot = vp.camera.rotation;
            // The points the cube's own hit test gives to that cell; their centre (pixels from the widget centre, y up).
            let hits: Vec<DVec2> = (-25..=25).flat_map(|i| (-25..=25).map(move |j| DVec2::new(i as f64 * 3.0, j as f64 * 3.0))).filter(|p| super::view_cube::widget_hit(rot, *p) == Some(super::view_cube::CubeCell(cell))).collect();
            if hits.is_empty() {
                return Err(format!("the view cube's {name} isn't visible"));
            }
            let c = hits.iter().sum::<DVec2>() / hits.len() as f64;
            let center = DVec2::from(noshape_render::overlay::view_cube::CENTER);
            return Ok(origin + point(px((center.x + c.x) as f32), px((center.y - c.y) as f32)));
        }
        let world = if let Some(name) = t.get("plane").and_then(Value::as_str) {
            let p = self.display.planes.iter().find(|p| p.name == name && !p.hidden).ok_or_else(|| format!("no shown plane {name}"))?;
            self.visible_plane_point(p, cx).ok_or_else(|| format!("plane {name} is covered everywhere"))?
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
