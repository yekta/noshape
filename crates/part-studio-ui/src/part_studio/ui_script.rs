//! The interaction parity suite's view of the Part Studio
//! (tests/ui-parity/README.md): the recorded state, read from the studio's
//! own model, and the window point of a geometry target (a plane, a model
//! point, a sketch point) for the driver to click.

use super::*;
use glam::{DVec2, DVec3};
use gpui_kit::component::input::InputState;
use serde_json::{Value, json};

impl PartStudioView {
    /// The graphics' context menu's return focus (the ui-parity driver reads a state with it).
    pub fn context_menu_return_focus(&self, cx: &App) -> Option<FocusHandle> {
        self.context_menu.as_ref().and_then(|(_, m, _)| crate::os_menu::return_focus_of(m, cx))
    }

    /// `state` after a step.
    pub fn ui_state(&self, window: &Window, cx: &App) -> Value {
        let dialog = self.edit.as_ref().map(|e| e.dialog.read(cx));
        let filter_focused = self.ui.filter.as_ref().is_some_and(|f| f.0.read(cx).focus_handle(cx).is_focused(window));
        let dim_box = self.dim_input.as_ref().map(|d| d.1.read(cx)).filter(|s| s.focus_handle(cx).is_focused(window)).or_else(|| self.focused_value_box(window, cx));
        let in_dialog = dialog.and_then(|d| d.ui_focus(window, cx));
        let focus = if let Some(f) = in_dialog.clone() {
            f
        } else if let Some((label, st)) = self.ui.var_input.as_ref().and_then(|d| d.focused_field(window, cx)) {
            // The Variable input dialog's field (live: its label, "Name" on opening).
            crate::feature_dialog::field_focus(label, st)
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
        } else if let Some(st) = self.ui.dyn_suppression.as_ref().map(|d| d.1.read(cx)).filter(|st| st.focus_handle(cx).is_focused(window)) {
            // Dynamic suppression's box (live feature-menu-dynamic-suppression-add: its field).
            crate::feature_dialog::field_focus("Unsuppressed if", st)
        } else if let Some(st) = self.panel_inputs.values().map(|(e, _)| e.read(cx)).find(|st| st.focus_handle(cx).is_focused(window)) {
            // A right panel's table cell (a configuration's Name row, a variable): live "other".
            let mut f = crate::feature_dialog::field_focus("", st);
            f["kind"] = "other".into();
            f["label"] = Value::Null;
            f
        } else if let Some(st) = self.ui.add_features.as_ref().and_then(|a| a.panel.read(cx).focused_input(window, cx)) {
            // Add custom features' search box (live custom-features-menu-empty: a field without a label).
            let mut f = crate::feature_dialog::field_focus("", st.read(cx));
            f["label"] = Value::Null;
            f
        } else if let Some(st) = self.ui.ct.add.as_ref().and_then(|a| a.focused_input(window, cx)) {
            // Add custom tables' search boxes (live fs-tables-add: a field without a label).
            let mut f = crate::feature_dialog::field_focus("", st.read(cx));
            f["label"] = Value::Null;
            f
        } else if self.ui.add_features.as_ref().is_some_and(|a| a.panel.read(cx).has_keyboard(window)) {
            // Add custom features with the keyboard outside its search box (live: its clicked tab or row, a div).
            json!({ "kind": "field", "label": null })
        } else if self.panel_row_focus.is_focused(window) || self.context_select_focus.is_focused(window) {
            // A pressed panel row, the contexts selector (live: a focused div, "other").
            json!({ "kind": "other", "label": null, "text": null, "selection": null, "all": false })
        } else if let Some(st) = self.palette.as_ref().map(|p| p.0.read(cx)).filter(|s| s.focus_handle(cx).is_focused(window)) {
            let mut f = crate::feature_dialog::field_focus("", st);
            f["kind"] = "search-tools".into();
            f["label"] = Value::Null;
            f
        } else if let Some(sd) = self.ui.section_dialog.as_ref().filter(|sd| !sd.keyboard_left) {
            // The Section view panel's active field (live reads it as the focused control).
            json!({ "kind": "field", "label": if sd.items_active { if sd.include { "Items to include" } else { "Items to exclude" } } else { "Section planes" }, "text": null, "selection": null, "all": false })
        } else if self.ui.measure_pinned && self.focus.is_focused(window) {
            // [ › Measure: its selection field has the keyboard (live key-bracket-measure).
            json!({ "kind": "field", "label": "Select entities to measure", "text": null, "selection": null, "all": false })
        } else if let Some(d) = self.ui.draft.as_ref().filter(|_| self.focus.is_focused(window)) {
            // Draft analysis: its active field has the keyboard (live analysis-draft: "Entities to check" once the
            // pull direction is picked).
            json!({ "kind": "field", "label": if d.active == 0 { "Mold split direction" } else { "Entities to check" }, "text": null, "selection": null, "all": false })
        } else if self.shortcut_sheet.is_some() {
            json!({ "kind": "other", "label": null, "text": null, "selection": null, "all": false })
        } else if let Some(st) = self.ui.filter.as_ref().map(|f| f.0.read(cx)).filter(|_| filter_focused) {
            let mut f = crate::feature_dialog::field_focus("Filter by name or type", st);
            f["kind"] = "filter".into();
            f
        } else if self.ui.appearance_edit.as_ref().is_some_and(|e| self.appearance_heading(&e.target, cx).1.is_some()) && self.focus.is_focused(window) {
            // The appearance editor's "Faces or features" field has the keyboard (live face-menu-add-appearance-to-face).
            json!({ "kind": "field", "label": "Faces or features", "text": null, "selection": null, "all": false })
        } else if let Some(st) = crate::ui_script::open_dialog()
            .filter(|d| d["kind"] == "modal")
            .and(self.ui.cad_export_file.as_ref())
            .and_then(|f| f.upgrade())
            .filter(|f| f.read(cx).focus_handle(cx).is_focused(window))
        {
            // Export as DXF/DWG's File name (live sketch-row-menu-export-dxf: "other", its text).
            let mut f = crate::feature_dialog::field_focus("", st.read(cx));
            f["kind"] = "other".into();
            f["label"] = Value::Null;
            f
        } else if crate::ui_script::open_dialog().is_some_and(|d| d["kind"] == "modal") {
            // A modal holds the keyboard (live part-menu-properties: "other").
            json!({ "kind": "other", "label": null, "text": null, "selection": null, "all": false })
        } else if let Some(f) = crate::ui_script::noted_focus() {
            f
        } else {
            json!({ "kind": "none", "label": null, "text": null, "selection": null, "all": false })
        };
        let list = crate::os_menu::ui_state(cx)
            .or_else(|| self.context_menu.as_ref().and_then(|(_, m, _)| crate::os_menu::menu_state(m, cx)))
            .or_else(|| dialog.and_then(|d| d.ui_list()))
            .or_else(|| self.dim_completion_state(cx))
            .or_else(|| {
                let c = self.ui.cell_completion.as_ref()?;
                let units = self.doc.read(cx).session.doc.settings.units;
                Some(c.ui_state((c.key == super::behaviors::DYN_SUPPRESSION_KEY).then_some((&self.display.variables, &units))))
            })
            .or_else(|| self.ui_select_other_state(cx))
            .or_else(|| self.ui_mini_toolbar_state())
            .or_else(crate::ui_script::open_list);
        let feature_name = |id: FeatureId| self.studio(cx).and_then(|s| s.features.iter().find(|f| f.id == id).map(|f| f.name.clone())).unwrap_or_default();
        // In a sketch the selection is its own, with what the Dimension tool picked (drawn selected); in a feature
        // dialog what its active field picked (live).
        let in_sketch = self.sketch.as_ref().map(|ed| {
            let f = ed.feature;
            let picks = if ed.tool == crate::sketch_mode::Tool::Dimension { ed.tool_picks() } else { Vec::new() };
            // A linked plane, edge or vertex picked into the sketch reads as itself (live: PLANE Right).
            let linked = self.sketch_data(cx).map(|d| d.external_refs.clone()).unwrap_or_default();
            ed.selection.iter().chain(&picks).map(|e| linked.get(&e.0).cloned().unwrap_or(Reference::SketchEntity { sketch: f, entity: e.0 })).chain(ed.selected_regions.iter().map(|k| Reference::SketchRegion { sketch: f, key: k.clone() })).chain(ed.model_selection.iter().cloned()).collect::<Vec<_>>()
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
                // An inferred point (an edge's midpoint) reads as a POINT of its entity (live's reader since 11-04 00:34).
                match r {
                    Reference::Implicit { .. } => json!({ "kind": "POINT", "owner": owner, "of": kind }),
                    _ => json!({ "kind": kind, "owner": if own { Value::Null } else { owner.into() } }),
                }
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
                let picked_rows = self.picked_features(cx);
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
                                    Some(FeatureStatus::Suppressed) => "suppressed",
                                    // An info feature's row too is drawn as an ok one (live plane-line-angle: no ns-list-item-info).
                                    _ => "ok",
                                }
                            };
                            // A custom feature's row reads with its initials box first (live 2361: "uF uip Feature 1").
                            let name = match features::get(&f.type_id).filter(|_| noshape_model::custom::is_custom(&f.type_id)) {
                                Some(t) => format!("{} {}", crate::ui::feature_initials(t.name), self.shown_feature_name(f, cx)),
                                None => self.shown_feature_name(f, cx),
                            };
                            rows.push(json!({ "name": name, "state": state, "selected": self.selected_features.contains(&f.id) || editing == Some(f.id) || picked_rows.contains(&f.id), "editing": editing == Some(f.id), "related": related.contains(&f.id), "hidden": f.hidden && state != "rolled-back" && !(editing == Some(f.id) || self.edit_uses(f.id, cx) || self.rolled_to(f.id, cx)) }));
                        }
                        // A new feature's row while its dialog is open (red while it fails, as drawn).
                        super::ListItem::Placeholder => {
                            let state = if self.edit_invalid(cx) { "error" } else { "ok" };
                            let ty = self.edit.as_ref().map(|e| e.dialog.read(cx).feature.type_id.clone()).unwrap_or_default();
                            let name = match features::get(&ty).filter(|_| noshape_model::custom::is_custom(&ty)) {
                                Some(t) => format!("{} {}", crate::ui::feature_initials(t.name), self.placeholder_name(cx)),
                                None => self.placeholder_name(cx),
                            };
                            rows.push(json!({ "name": name, "state": state, "selected": true, "editing": true, "related": editing.is_some_and(|f| related.contains(&f)), "hidden": false }));
                        }
                        // A new folder's row is related-highlighted until the next click (live ui 10-25 b (1)).
                        super::ListItem::Folder(_, fd) => rows.push(json!({ "name": format!("{} ({})", fd.name, s.folder_count(fd.id)), "state": "ok", "selected": self.ui.selected_folder == Some(fd.id), "editing": false, "related": self.ui.fresh_folder == Some(fd.id), "hidden": false })),
                        // Rows a filter leaves out show as one "..." row (live ui-parity feature-filter).
                        super::ListItem::Ellipsis => rows.push(json!({ "name": "...", "state": "ok", "selected": false, "editing": false, "related": false, "hidden": false })),
                        // At the top the bar sits under Default geometry (live).
                        super::ListItem::Rollback(..) => after = Some(rows.last().and_then(|r| r["name"].as_str()).unwrap_or("Default geometry").to_string()),
                        _ => {}
                    }
                }
                (rows, after)
            }
            None => (Vec::new(), None),
        };
        // As the list draws them (render_parts_list): Parts' rows, then each other section's heading ("Surfaces (1)",
        // live ui-parity offset-surface-tool) and its rows unless collapsed; a run of parts a filter leaves out is one
        // "…" row.
        let mut parts: Vec<String> = Vec::new();
        for (title, key) in [("Parts", "parts"), ("Composite parts", "composites"), ("Surfaces", "surfaces"), ("Curves", "curves")] {
            let bodies: Vec<_> = self.display.bodies.iter().filter(|b| super::part_section(b) == key).collect();
            if key != "parts" {
                if bodies.is_empty() {
                    continue;
                }
                parts.push(format!("{title} ({})", bodies.len()));
                if self.ui.collapsed_sections.contains(key) {
                    continue;
                }
            }
            let mut gap = false;
            for b in bodies {
                if noshape_model::interaction::filter_row(&self.ui.filter_query, &b.name, b.hidden) == Some(false) {
                    if !std::mem::replace(&mut gap, true) {
                        parts.push("…".into());
                    }
                    continue;
                }
                gap = false;
                parts.push(b.name.clone());
            }
        }
        // The rollback bar a click gave the keyboard is a selected item in live's reader (its tree node, no kind).
        let mut items = items;
        if self.edit.is_none() && self.ui.list_focus.is_focused(window) && self.ui.rollback_keyboard.as_ref() == Some(&self.selected_features) {
            items.push(json!({ "kind": null, "owner": null, "id": "rollback-bar", "name": null }));
        }
        let message = self.message.as_ref().filter(|(_, m)| self.edit.is_some() || m != behaviors::NO_INTERSECTION).map(|(_, m)| m.clone());
        // A sketch tool's prompt at the bottom of the view is live's message too.
        let message = message.or_else(|| self.sketch.as_ref().and_then(|ed| ed.message.clone()));
        // The Configuration panel's info box under an input nothing configures yet (live config-*).
        // (With a toast too, live reads both: "<note> / <toast>".)
        let note = (self.right_panel == Some(super::config_panel::RightPanel::Configurations) && self.ui.cfg_note_shown)
            .then_some("No features are configured Right click sketch dimensions, features, or feature parameters to configure them.");
        let message = match (note, message) {
            (Some(n), Some(m)) => Some(format!("{n} / {m}")),
            (Some(n), None) => Some(n.to_string()),
            (None, m) => m,
        };
        // The canceled-sketch toast's Restore link is part of its text in live.
        let message = message.map(|m| if self.canceled_edit.as_ref().is_some_and(|c| c.3 == m) { format!("{m} Restore") } else { crate::doc_model::notice_text(&self.doc, m, cx) });
        // The parts list's heading, as drawn (render_parts_list).
        let parts_heading = format!("Parts ({})", self.display.bodies.iter().filter(|b| super::part_section(b) == "parts").count());
        let dialog_state = dialog.map(|d| d.ui_state(window, cx)).or_else(|| self.section_dialog_state()).or_else(|| self.appearance_dialog_state(cx)).or_else(|| self.isolate_panel_state()).or_else(|| self.insert_go_state(cx)).or_else(|| self.show_mass.then(|| self.ui.mass_state.clone()).flatten()).or_else(|| self.panel_ui_state(cx)).or_else(crate::ui_script::open_dialog);
        // The dialog the focused field is in (live `focus.dialog`; the Create variable popup stands for its dialog).
        let mut focus = focus;
        // Mass properties' Face tab: its faces field has the keyboard (live ui-parity mass-properties-face-tab).
        if focus["kind"] == "none" && self.show_mass && self.ui.mass_faces && !self.ui.mass_ref_active {
            focus = json!({ "kind": "field", "label": "Faces to measure", "dialog": "Mass and section properties" });
        }
        if in_dialog.is_some() {
            focus["dialog"] = dialog_state.as_ref().map(|d| d["title"].clone()).unwrap_or(Value::Null);
        }
        json!({
            "focus": focus,
            "dialogs": dialog_state.iter().cloned().collect::<Vec<_>>(),
            "dialog": dialog_state,
            "partsHeading": parts_heading,
            "list": list,
            "selection": { "count": items.len(), "items": items },
            "features": { "rows": rows, "rollbackAfter": rollback_after },
            "parts": parts,
            "message": message,
            "tooltip": null,
            "undo": self.ui_undo_state(cx),
            "sketch": self.ui_sketch_state(cx),
            "camera": self.ui_camera_state(cx),
            // (No readout while a feature dialog is open: the readout isn't rebuilt then.)
            "measure": if self.ui.measure_readout.is_empty() || self.edit.is_some() { Value::Null } else { json!({ "readout": self.ui.measure_readout, "panel": null }) },
        })
    }

    /// `camera` as live's recorder reads it (since 2026-11-12): view direction and up (unit vectors), target and eye
    /// in mm, the visible width and height in mm, and whether the view is orthographic.
    fn ui_camera_state(&self, cx: &App) -> Value {
        let vp = self.viewport.read(cx);
        let c = &vp.camera;
        let size = vp.size();
        let r = |v: f64| (v * 1e6).round() / 1e6;
        let v3 = |v: DVec3, k: f64| json!([r(v.x * k), r(v.y * k), r(v.z * k)]);
        let height = 2.0 * c.half_height() * 1000.0;
        let width = if size.y > 0.0 { height * size.x / size.y } else { 0.0 };
        json!({
            "direction": v3(c.forward(), 1.0), "up": v3(c.up(), 1.0), "target_mm": v3(c.target, 1000.0), "eye_mm": v3(c.eye(), 1000.0),
            "width_mm": r(width), "height_mm": r(height), "mmPerPixel": if size.y > 0.0 { r(height / size.y) } else { 0.0 },
            "orthographic": c.projection == noshape_render::camera::Projection::Orthographic,
        })
    }

    /// The Select other popup (live: "select-other", its rows; the bold one isn't read as highlighted).
    fn ui_select_other_state(&self, cx: &App) -> Option<Value> {
        let so = self.ui.select_other.as_ref()?;
        let labeler = self.labeler(cx);
        let rows: Vec<String> = so.hits.iter().map(|r| labeler(r)).collect();
        Some(json!({ "kind": "select-other", "rows": rows, "highlighted": null }))
    }

    /// The S shortcut toolbar outside a sketch (live: "shortcut-toolbar", its tools by icon name: "new-sketch",
    /// "boolean-bodies").
    fn ui_mini_toolbar_state(&self) -> Option<Value> {
        self.ui.mini_toolbar?;
        if self.sketch.is_some() {
            return None;
        }
        let kebab = |id: &str| {
            let mut out = String::new();
            for c in id.chars() {
                if c.is_ascii_uppercase() {
                    out.push('-');
                }
                out.push(c.to_ascii_lowercase());
            }
            out
        };
        let rows: Vec<String> = self.ui.toolbar.shortcut_tools().iter().filter(|id| noshape_model::features::get(id).is_some()).map(|id| if id == "sketch" { "new-sketch".to_string() } else { kebab(id) }).collect();
        Some(json!({ "kind": "shortcut-toolbar", "rows": rows, "disabled": [], "highlighted": null }))
    }

    /// A side panel as live's recorder reads it since 2026-11-12 (`kind` "panel": its title and texts): Dependencies
    /// of … (its rows), Named views (the name field, or "(New view name)", and "No views created" with none), Material.
    fn panel_ui_state(&self, cx: &App) -> Option<Value> {
        let panel = |title: String, text: Vec<String>| Some(json!({ "kind": "panel", "title": title, "text": text }));
        if let Some(v) = self.analysis_panel_state(cx) {
            return Some(v);
        }
        if let Some(r) = &self.ui.plane_dependencies {
            let studio = self.studio(cx)?;
            let rows = noshape_model::interaction::features_using(&studio.features, r).into_iter().filter_map(|i| studio.feature(i).map(|f| f.name.clone())).collect();
            return panel(format!("Dependencies of {}", self.default_name(r, cx)), rows);
        }
        if let Some(id) = self.ui.dependencies {
            let studio = self.studio(cx)?;
            let idx = studio.index_of(id)?;
            let name = |i: FeatureId| studio.feature(i).map(|f| f.name.clone());
            let mut rows: Vec<String> = noshape_model::interaction::feature_dependencies(&studio.features, idx).into_iter().filter_map(name).collect();
            rows.push(studio.features[idx].name.clone());
            rows.extend(noshape_model::interaction::feature_dependents(&studio.features, idx).into_iter().filter_map(name));
            return panel(format!("Dependencies of {}", studio.features[idx].name), rows);
        }
        if let Some((st, _)) = &self.named_views {
            let typed = st.read(cx).value().to_string();
            let mut text = vec![if typed.is_empty() { "(New view name)".to_string() } else { typed }];
            if self.studio(cx).is_none_or(|s| s.named_views.is_empty()) {
                text.push("No views created".into());
            }
            return panel("Named views".into(), text);
        }
        // Select other (live 2026-11-13 select-other-row-click): its rows, or "(No selections found)".
        if let Some(so) = &self.ui.select_other {
            let labeler = self.labeler(cx);
            let mut rows: Vec<String> = so.hits.iter().map(|r| labeler(r).to_string()).collect();
            if rows.is_empty() {
                rows.push("(No selections found)".into());
            }
            return Some(json!({ "kind": "panel", "title": "Select other", "text": rows, "fields": [] }));
        }
        // New folder's "Folder name" popover (live feature-list-new-folder-empty): its field.
        if let Some((st, target, _)) = &self.folder_prompt
            && matches!(target, super::FolderTarget::Features(_) | super::FolderTarget::Folder(_))
        {
            let v = st.read(cx).value().to_string();
            return Some(json!({ "kind": "panel", "title": "Folder name", "text": [v], "fields": [{ "label": "Folder name", "kind": "input", "value": v }] }));
        }
        if let Some(m) = &self.ui.material_panel {
            // Live: its tabs, the picker's text and, with none picked, "No material assigned".
            let mut text = vec!["Library".to_string(), "Custom".to_string()];
            if !m.is_custom() {
                text.push(m.picked_name().unwrap_or_else(|| "- None -".into()));
                if m.picked_name().is_none() && !m.is_list_open() {
                    text.push("No material assigned".into());
                }
            }
            return panel("Material".into(), text);
        }
        None
    }

    /// `undo`: whether the toolbar's Undo and Redo do something (in a sketch
    /// or dialog, its own history).
    fn ui_undo_state(&self, cx: &App) -> Value {
        let (undo, redo) = if self.ui.dyn_suppression.is_some() {
            (false, false)
        } else if self.edit.is_some() {
            let (undo, redo) = self.draft_history_state();
            (undo || self.doc_edited_in_dialog(cx), redo)
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
        let Some(ed) = self.sketch.as_ref() else { return json!({ "tool": null, "entities": null, "constraints": null, "dimensions": null }) };
        let sk = &data.sketch;
        // The first active toolbar button (live): Construction while it shows active, unless a drawing tool (left of it) is on.
        use crate::sketch_mode::Tool as T;
        let right_of_construction = matches!(
            ed.tool,
            T::Select | T::Fillet | T::Chamfer | T::Trim | T::Extend | T::Split | T::Offset | T::Slot | T::Mirror | T::LinearPattern | T::CircularPattern | T::Transform | T::InsertDxf | T::InsertImage | T::Dimension | T::Constraint(_)
        );
        let tool = if right_of_construction && ed.construction_shown(sk) {
            Some("Construction".to_string())
        } else {
            let shown = ed.shown_tool();
            (shown != T::Select).then(|| crate::ui::onshape_title(shown.label()).split(" (").next().unwrap_or_default().to_string())
        };
        let owned: std::collections::HashSet<_> = sk.entities().filter(|e| e.kind() != EntityKind::Point).flat_map(|e| e.point_refs()).collect();
        let mut entities: Vec<String> = sk
            .entities()
            // (A spline's tangent handles are part of it in live's model: not listed.)
            .filter(|e| !e.external && !e.reference_only && e.handle_of.is_none() && !(e.kind() == EntityKind::Point && owned.contains(&e.id)))
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
        for e in sk.entities().filter(|e| e.kind() != EntityKind::Point && !e.external && e.handle_of.is_none()) {
            let mut pts = e.point_refs().to_vec();
            pts.sort();
            pts.dedup();
            for p in pts {
                *users.entry(p).or_default() += 1;
            }
        }
        let mut constraints: Vec<String> = users.values().filter(|n| **n > 1).flat_map(|n| std::iter::repeat_n("COINCIDENT".to_string(), n - 1)).collect();
        // Live's model holds an open spline's, Bezier's or conic's ends as points of their own, each Coincident with
        // it, and a Bezier's degree as a constraint (sketch-spline-menu-confirm, sketch-tool-bezier, sketch-tool-conic).
        for e in sk.entities().filter(|e| !e.external && e.handle_of.is_none() && matches!(e.kind(), EntityKind::BSpline | EntityKind::FitSpline | EntityKind::Conic)) {
            if e.endpoints().is_some_and(|(a, b)| a != b) {
                constraints.extend(["COINCIDENT".to_string(), "COINCIDENT".to_string()]);
            }
            if let noshape_sketch::Geometry::BSpline { control, degree, .. } = &e.geometry
                && control.len() == degree + 1
            {
                constraints.push("BEZIER_DEGREE".into());
            }
        }
        let mut dimensions: Vec<(String, f64, Option<String>)> = Vec::new();
        let units = self.doc.read(cx).session.doc.settings.units;
        // A driven (reference) dimension: its kind without a value (live sketch-dim-menu-driven).
        let mut driven: Vec<String> = Vec::new();
        for c in sk.constraints().filter(|c| !c.driving && c.kind.is_dimension()) {
            let kind = match &c.kind {
                K::Length { .. } | K::Distance { .. } | K::HorizontalDistance { .. } | K::VerticalDistance { .. } => "LINEAR".to_string(),
                K::Radius { .. } => "RADIUS".into(),
                K::Diameter { .. } => "DIAMETER".into(),
                K::Angle { .. } | K::ArcAngle { .. } => "ANGLE".into(),
                k => k.name().to_uppercase().replace(' ', "_"),
            };
            driven.push(kind);
        }
        let mut pattern_seeds = Vec::new();
        for c in sk.constraints().filter(|c| c.driving) {
            let mm = |v: f64| (v * 1e3 * 1e6).round() / 1e6;
            let typed = |unit: &str| data.dimension_exprs.get(&c.id.0).map(|e| onshape_dimension_expression(&e.text, unit));
            let length = typed(units.length.symbol());
            match &c.kind {
                K::Length { value, .. } | K::Distance { value, .. } | K::HorizontalDistance { value, .. } | K::VerticalDistance { value, .. } => dimensions.push(("LINEAR".into(), mm(*value), length)),
                K::Radius { value, .. } => dimensions.push(("RADIUS".into(), mm(*value), length)),
                K::Diameter { value, .. } => dimensions.push(("DIAMETER".into(), mm(*value), length)),
                K::Angle { value, .. } | K::ArcAngle { value, .. } => dimensions.push(("ANGLE".into(), (value.to_degrees() * 1e6).round() / 1e6, typed(units.angle.symbol()))),
                // A conic's Rho is one of its constraints in live's model (sketch-tool-conic: "RHO").
                K::Rho { .. } => constraints.push("RHO".into()),
                k if k.is_dimension() => dimensions.push((k.name().to_uppercase().replace(' ', "_"), c.kind.value().unwrap_or(0.0), length)),
                // Live's model lists a pattern by its seed's points (a polygon's: two, ui-parity sketch-circumscribed-polygon),
                // not one link per instance point.
                K::CircularPatternLink { seed, center, .. } => {
                    if !pattern_seeds.contains(&(*seed, *center)) {
                        pattern_seeds.push((*seed, *center));
                        constraints.push("CIRCULAR_PATTERN".into());
                    }
                }
                k => constraints.push(k.name().to_uppercase().replace(' ', "_")),
            }
        }
        // A mirrored line: live's model holds the lines' symmetry besides their ends' (ui-parity sketch-mirror-tool).
        let sym: Vec<(noshape_sketch::EntityId, noshape_sketch::EntityId, noshape_sketch::EntityId)> = sk.constraints().filter_map(|c| match c.kind { K::Symmetric { a, b, axis } => Some((a, b, axis)), _ => None }).collect();
        let pair = |p: noshape_sketch::EntityId, q: noshape_sketch::EntityId, axis| sym.iter().any(|s| s.2 == axis && ((s.0 == p && s.1 == q) || (s.0 == q && s.1 == p)));
        let lines: Vec<(noshape_sketch::EntityId, noshape_sketch::EntityId)> = sk.entities().filter(|e| e.kind() == EntityKind::Line && e.handle_of.is_none()).filter_map(|e| e.endpoints()).collect();
        for (i, (p1, p2)) in lines.iter().enumerate() {
            for (q1, q2) in &lines[i + 1..] {
                let axes = sym.iter().map(|s| s.2).collect::<std::collections::HashSet<_>>();
                if axes.into_iter().any(|ax| (pair(*p1, *q1, ax) && pair(*p2, *q2, ax)) || (pair(*p1, *q2, ax) && pair(*p2, *q1, ax))) {
                    constraints.push("SYMMETRIC".into());
                }
            }
        }
        constraints.sort();
        dimensions.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let mut dimensions: Vec<Value> = dimensions.into_iter().map(|(kind, value, expression)| json!({ "kind": kind, "value": value, "expression": expression })).collect();
        dimensions.extend(driven.into_iter().map(|kind| json!({ "kind": kind, "expression": null })));
        json!({ "tool": tool, "entities": entities, "constraints": constraints, "dimensions": dimensions })
    }

    /// A selected entity's kind and owner as the parity state names them (an inferred point: its entity's).
    fn selection_kind_owner(&self, r: &Reference, cx: &App) -> (&'static str, String) {
        let feature_name = |id: FeatureId| self.studio(cx).and_then(|s| s.feature(id).map(|f| f.name.clone())).or_else(|| self.ui.feature_names.get(&id).cloned()).unwrap_or_default();
        // The preview may have consumed the entity: then the model before the edited feature has it.
        let body = |r: &Reference| {
            let before = self.ui.preview_before.as_deref();
            std::iter::once(&*self.display).chain(before).find_map(|d| noshape_ops::studio::body_of(d, r).and_then(|b| d.body(&b)).map(|b| b.name.clone())).unwrap_or_default()
        };
        // A pick that no longer resolves (a missing chip): live's selection names the feature that made it
        // (fillet-chip-edit-healthy-moment: FEATURE, "Extrude 1").
        let topo = |kind: &'static str, _: &noshape_model::reference::TopoName| match self.unresolved_pick_creator(r) {
            Some(f) => ("FEATURE", feature_name(f)),
            None => (kind, body(r)),
        };
        match r {
            Reference::Face(n) => topo("FACE", n),
            Reference::Edge(n) => topo("EDGE", n),
            Reference::Vertex(n) => topo("VERTEX", n),
            Reference::Body(_) => ("BODY", body(r)),
            Reference::Plane(p) => ("PLANE", format!("{p:?}")),
            Reference::Origin => ("VERTEX", "Origin".to_string()),
            Reference::SketchEntity { sketch, .. } => ("SKETCH_ENTITY", feature_name(*sketch)),
            Reference::SketchRegion { sketch, .. } => ("FACE", feature_name(*sketch)),
            Reference::SketchRegionEdge { sketch, end: None, .. } => ("EDGE", feature_name(*sketch)),
            Reference::SketchRegionEdge { sketch, .. } | Reference::SketchCurveEnd { sketch, .. } => ("VERTEX", feature_name(*sketch)),
            Reference::Sketch(f) | Reference::SketchBody { sketch: f, .. } | Reference::Feature(f) => ("FEATURE", feature_name(*f)),
            Reference::FeaturePlane(f) | Reference::ConstructionPlane { feature: f, .. } => ("PLANE", feature_name(*f)),
            Reference::MateConnector { feature, .. } => ("MATE_CONNECTOR", feature_name(*feature)),
            Reference::Implicit { on, .. } => self.selection_kind_owner(on, cx),
            Reference::QueryVariable(_) => ("OTHER", String::new()),
        }
    }

    /// A point of plane `p`'s rectangle in front of every other shown plane and part, near its labelled corner.
    /// A shown plane is under viewport point `at`, or its outline within the pick's reach.
    fn plane_near(&self, at: DVec2, cx: &App) -> bool {
        let vp = self.viewport.read(cx);
        let (o, d) = vp.ray(at);
        let d = d.normalize();
        self.display.planes.iter().filter(|q| !q.hidden).any(|q| {
            let f = &q.frame;
            let denom = d.dot(f.normal);
            let inside = denom.abs() > 1e-9 && {
                let t = (f.origin - o).dot(f.normal) / denom;
                let local = f.to_local(o + d * t) - q.center;
                t > 0.0 && local.x.abs() <= q.half_width && local.y.abs() <= q.half_height
            };
            let c = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)].map(|(u, v)| vp.project(q.corner(u, v)));
            let outline = (0..4).any(|k| match (c[k], c[(k + 1) % 4]) {
                (Some(a), Some(b)) => {
                    let t = ((at - a).dot(b - a) / (b - a).length_squared().max(1e-9)).clamp(0.0, 1.0);
                    (a + (b - a) * t).distance(at) < 15.0
                }
                _ => false,
            });
            inside || outline
        })
    }

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
    /// The Isolate / Make transparent panel as live's state reads it (a dialog frame with no fields of its own).
    fn isolate_panel_state(&self) -> Option<Value> {
        let iso = self.ui.isolate.as_ref()?;
        let title = if iso.mode == super::behaviors::IsolateMode::Isolate { "Isolate" } else { "Make transparent" };
        Some(json!({ "title": title, "titleState": "ok", "kind": "feature", "tabs": [], "fields": [], "error": null }))
    }

    fn unlabelled_input_focused<'a>(&self, window: &Window, cx: &'a App) -> Option<&'a InputState> {
        let appearance = self.ui.appearance_edit.as_ref().map(|a| std::iter::once(&a.hex).chain(&a.channels).chain(std::iter::once(&a.alpha)));
        let inputs = self
            .renaming
            .as_ref()
            .map(|r| &r.1)
            .into_iter()
            .chain(self.folder_prompt.as_ref().map(|f| &f.0))
            .chain(self.ui.part_renaming.as_ref().map(|r| &r.1))
            .chain(self.ui.dg_renaming.as_ref().map(|r| &r.1))
            .chain(self.named_views.as_ref().map(|n| &n.0))
            .chain(appearance.into_iter().flatten());
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
            return super::view_cube::ui_cell_point(name, self.ui.cube_origin, vp.camera.rotation);
        }
        if let Some(letter) = t.get("annotation").and_then(Value::as_str) {
            let at = self.annotation_point(letter, cx).ok_or_else(|| format!("no annotation {letter} shown"))?;
            return Ok(vp.origin() + point(px(at.x as f32), px(at.y as f32)));
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
            // From the middle outwards, away from the open dialog (a click there doesn't reach the graphics) and
            // from the sketch's curves.
            let dialog = crate::ui_script::bounds_of("dialog");
            let origin = vp.origin();
            let mut grid: Vec<DVec2> = (1..8).flat_map(|i| (1..8).map(move |j| DVec2::new(size.x * i as f64 / 8.0, size.y * j as f64 / 8.0))).collect();
            grid.sort_by(|a, b| (*a - size * 0.5).length().total_cmp(&(*b - size * 0.5).length()));
            let near_sketch = |at: DVec2| {
                let Some(ed) = self.sketch.as_ref() else { return false };
                let Some(data) = self.sketch_data(cx) else { return false };
                let (o, d) = vp.ray(at);
                let Some(p) = ed.plane_point(o, d) else { return true };
                data.sketch.pick(p, ed.world_per_px * 20.0).is_some()
            };
            let free = grid.into_iter().find(|at| {
                let win = origin + point(px(at.x as f32), px(at.y as f32));
                if dialog.is_some_and(|b| b.dilate(px(10.)).contains(&win)) || near_sketch(*at) || self.plane_near(*at, cx) {
                    return false;
                }
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
        // An open dialog's model before the feature and std's editing logic on it (a new Extrude opens as New and
        // turns Add once that ran) count too.
        self.pending.is_some() || self.context.is_none() || self.ui.preview_task.is_some() || self.ui.scene_task.is_some() || self.ui.restore_check.is_some() || self.ui.std_edit_task.is_some() || self.ui.material_notice_pending
    }

    /// The hover under a resting pointer isn't settled yet: a hover query in flight, or an edge's midpoint dwell
    /// pending (the driver waits for it before pressing, as a hand rests before clicking).
    pub fn ui_hover_settling(&self, cx: &App) -> bool {
        self.ui.midpoint_dwell_task.is_some() || self.viewport.read(cx).hover_pending()
    }

    /// Redraw the studio and its dialog, so every named control records where it is now.
    pub fn ui_redraw(&self, cx: &mut Context<Self>) {
        if let Some(e) = &self.edit {
            e.dialog.update(cx, |_, cx| cx.notify());
        }
        cx.notify();
    }
}

/// A typed sketch dimension as Onshape stores it (live `sketch.dimensions[].expression`): a plain number with the
/// document unit ("25" → "25 mm"), a unitless expression wrapped ("2*15" → "(2*15) mm"), anything with a unit or a
/// variable as typed.
fn onshape_dimension_expression(text: &str, unit: &str) -> String {
    let t = text.trim();
    if t.parse::<f64>().is_ok() {
        return format!("{t} {unit}");
    }
    if t.chars().all(|c| c.is_ascii_digit() || " .+-*/()^".contains(c)) {
        return format!("({t}) {unit}");
    }
    t.to_string()
}
