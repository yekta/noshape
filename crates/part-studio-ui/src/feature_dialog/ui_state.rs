//! The open dialog as the interaction parity suite records it
//! (tests/ui-parity/README.md, `state.dialog`): its title, tabs and visible
//! fields as shown, read from the dialog's own state.

use super::*;
use serde_json::{Value, json};

/// `state.focus` for a text field with the keyboard: its text and selection in characters.
pub fn field_focus(label: &str, st: &InputState) -> Value {
    let text = st.value().to_string();
    let sel = st.selected_range();
    let chars = |b: usize| text.get(..b.min(text.len())).map_or(0, |t| t.chars().count());
    let (a, b) = (chars(sel.start), chars(sel.end));
    json!({ "kind": "field", "label": label, "text": text, "selection": [a, b], "all": a == 0 && b == text.chars().count() && b > 0 })
}

impl FeatureDialog {
    /// The dialog's title as shown.
    pub fn title_text(&self) -> String {
        let shown = self.shown_feature();
        noshape_model::interaction::display_name_computed(&shown, &|k| self.computed.get(k).cloned().or_else(|| shown_length(&shown, k, &self.units, &self.variables)))
    }

    /// A number or text field's input: whether it has the keyboard, its text and selection.
    fn input_of(&self, path: &str, window: &Window, cx: &App) -> Option<(bool, String, std::ops::Range<usize>)> {
        let st = self.inputs.get(path)?.read(cx);
        Some((st.focus_handle(cx).is_focused(window), st.value().to_string(), st.selected_range()))
    }

    /// The focused field (`state.focus`), if one of this dialog's has the keyboard.
    pub fn ui_focus(&self, window: &Window, cx: &App) -> Option<Value> {
        if let Some(c) = &self.create_var {
            return Self::create_var_inputs(c).into_iter().find(|(_, st, _)| st.read(cx).focus_handle(cx).is_focused(window)).map(|(l, st, _)| field_focus(l, st.read(cx)));
        }
        // A select's closed field or its open list (live: its hidden input, no label), a selection field, a checkbox
        // or a button.
        if let Some((path, _)) = self.selects.iter().find(|(_, s)| s.read(cx).has_focus(window) || s.read(cx).is_open()) {
            // A select drawn with its label reads it (Fillet's Measurement); Extrude's end type has none.
            let label = self.spec_for_path(path).filter(|s| s.hints.has(Hints::SHOW_LABEL)).map(|s| crate::i18n::spec(s.label));
            return Some(json!({ "kind": "field", "label": label, "text": "", "selection": [0, 0], "all": false }));
        }
        // (The Sketch dialog's plane field holds a Tab stop but live never shows it focused.)
        if let Some((key, _)) = self.tab_stops.iter().find(|(k, h)| h.is_focused(window) && !(self.feature.type_id == "sketch" && k.starts_with("q:"))) {
            let label = match key.split_once(':') {
                Some(("q" | "cb", path)) => self.spec_for_path(path).map(|s| crate::i18n::spec(s.label)),
                _ => None,
            };
            return Some(json!({ "kind": "field", "label": label, "text": null, "selection": null, "all": false }));
        }
        let Some(path) = self.inputs.keys().find(|p| self.input_of(p, window, cx).is_some_and(|i| i.0)) else {
            // No control has the keyboard: the active selection field has the focus (live), unless the keyboard
            // left the dialog (a press in the graphics, Esc out of a field: live ui-parity extrude-escape-cancels).
            // The Sketch dialog doesn't take the focus at all (live: its plane field isn't focused).
            if self.keyboard_left || self.feature.type_id == "sketch" {
                return None;
            }
            let q = self.active_query.as_deref()?;
            let label = self.spec_for_path(q).map(|s| crate::i18n::spec(s.label)).unwrap_or_else(|| q.to_string());
            return Some(json!({ "kind": "field", "label": label, "text": null, "selection": null, "all": false }));
        };
        let label = self.spec_for_path(path).map(|s| crate::i18n::spec(s.label)).unwrap_or_else(|| path.clone());
        Some(field_focus(&label, self.inputs[path].read(cx)))
    }

    /// The open Create variable popup's inputs, in order: label, input, whether its value is a quantity.
    fn create_var_inputs(c: &CreateVar) -> Vec<(&'static str, &Entity<InputState>, bool)> {
        let text = c.configuration && CREATE_CONFIG_KINDS[c.config_kind].0 == "TEXT";
        if c.configuration {
            let mut v = vec![("Name", &c.name, false), ("Default", &c.value, !text)];
            if !text {
                v.extend([("Minimum", &c.min, true), ("Maximum", &c.max, true)]);
            }
            v
        } else {
            vec![("Name", &c.name, false), ("Value", &c.value, true), ("Description", &c.description, false)]
        }
    }

    /// The top-level parameters the dialog draws, in order: visible ones outside folded groups (as
    /// `render_params`: a group holds the parameters its header hides when off).
    fn shown_params(&self) -> Vec<&'static ParamSpec> {
        let values = &self.feature.params;
        let mut groups: Vec<(&'static str, bool)> = Vec::new();
        let mut out = Vec::new();
        for s in self.ty.params.iter().filter(|s| s.is_visible(values)) {
            let depends_on = |header: &str| {
                let mut off = values.clone();
                off.set(header, ParamValue::Bool(false));
                !s.is_visible(&off)
            };
            while groups.last().is_some_and(|g| !depends_on(g.0)) {
                groups.pop();
            }
            if groups.iter().any(|g| !g.1) {
                continue;
            }
            if s.hints.has(Hints::GROUP_HEADER) {
                groups.push((s.id, values.bool(s.id) && !self.collapsed_groups.contains(s.id)));
            }
            out.push(s);
        }
        out
    }

    /// `state.dialog` while the Create variable popup is open (live: it stands for the dialog).
    fn create_var_state(&self, c: &CreateVar, window: &Window, cx: &App) -> Value {
        let mut tabs = vec![json!({ "label": "Feature", "selected": !c.configuration }), json!({ "label": "Configuration", "selected": c.configuration })];
        let kinds: Vec<(&str, bool)> = if c.configuration {
            CREATE_CONFIG_KINDS.iter().enumerate().map(|(i, (_, l))| (*l, i == c.config_kind)).collect()
        } else {
            CREATE_VAR_KINDS.iter().enumerate().map(|(i, (_, l))| (*l, i == c.kind)).collect()
        };
        tabs.extend(kinds.into_iter().map(|(l, on)| json!({ "label": l, "selected": on })));
        let errors = c.value_errors(&self.units, &self.variables, cx);
        let fields: Vec<Value> = Self::create_var_inputs(c)
            .into_iter()
            .enumerate()
            .map(|(i, (label, st, quantity))| {
                let st = st.read(cx);
                let bad = (i == 0 && c.name_error(cx).is_some()) || (i > 0 && quantity && errors.get(i - 1).copied().unwrap_or(false));
                json!({ "label": label, "kind": if quantity { "number" } else { "text" }, "text": st.value().to_string(), "focused": st.focus_handle(cx).is_focused(window), "state": if bad { "error" } else { "ok" } })
            })
            .collect();
        json!({ "title": "Create variable", "kind": "create-variable", "tabs": tabs, "fields": fields, "error": null })
    }

    /// `state.dialog`.
    pub fn ui_state(&self, window: &Window, cx: &App) -> Value {
        if let Some(c) = &self.create_var {
            return self.create_var_state(c, window, cx);
        }
        let mut tabs = Vec::new();
        let mut fields = Vec::new();
        let values = &self.feature.params;
        let mut prev: Option<&ParamSpec> = None;
        for s in self.shown_params() {
            // A value drawn beside the row before it (Draft's angle) shows no label of its own.
            let companion = prev.is_some_and(|p| p.hints.has(Hints::FIRST_IN_ROW) && p.hints.has(Hints::DISPLAY_SHORT)) && s.hints.has(Hints::DISPLAY_SHORT);
            prev = Some(s);
            // A row drawn with an icon instead of its label (the Hole's diameter, depth, tip angle) reads "" (live).
            let label = if companion {
                Value::Null
            } else if param_icon(self.ty.id, s.id).is_some() {
                json!("")
            } else {
                json!(crate::i18n::spec(s.label))
            };
            match &s.kind {
                ParamKind::Enum { options, .. } if s.hints.has(Hints::HORIZONTAL_ENUM) => {
                    let cur = values.enum_(s.id);
                    tabs.extend(options.iter().map(|(id, l)| json!({ "label": crate::i18n::spec(l), "selected": cur == *id })));
                }
                ParamKind::Enum { options, .. } => {
                    let cur = values.enum_(s.id);
                    let text = options.iter().find(|(id, _)| *id == cur).map(|(_, l)| crate::i18n::spec(l)).unwrap_or_default();
                    // A select shows its label only with SHOW_LABEL (Extrude's end type is "Blind" alone).
                    let label = s.hints.has(Hints::SHOW_LABEL).then_some(label);
                    fields.push(json!({ "label": label, "kind": "enum", "text": text, "state": "ok" }));
                }
                // A direction flip is a button beside the field before it (live: "toggle").
                ParamKind::Bool { .. } if s.hints.has(Hints::OPPOSITE_DIRECTION) || s.hints.has(Hints::OPPOSITE_DIRECTION_CIRCULAR) => {
                    fields.push(json!({ "label": "toggle", "kind": "toggle", "checked": values.bool(s.id), "state": "ok" }))
                }
                // A plain group title (Loft's "End conditions") is no control: live's state doesn't list it.
                ParamKind::Bool { .. } if s.hints.has(Hints::GROUP_HEADER) && s.hints.has(Hints::GROUP_LABEL) => {}
                ParamKind::Bool { .. } => fields.push(json!({ "label": label, "kind": "boolean", "checked": values.bool(s.id), "state": "ok" })),
                ParamKind::Num(n) => {
                    let expr = values.expr(s.id).map(|e| e.text.clone()).unwrap_or_default();
                    let (focused, text) = match self.input_of(s.id, window, cx) {
                        Some((f, t, _)) => (f, t),
                        None => (false, expr),
                    };
                    // As render_inline_value: while typing, the applied text's error shows, not the typed one's.
                    let editing = focused && self.baselines.get(s.id).is_some_and(|b| *b != text);
                    let parse_err = if editing { self.baselines.get(s.id).is_some_and(|b| noshape_units::parse(b).is_err()) } else { self.parse_errors.contains_key(s.id) };
                    let shown = if focused || parse_err { None } else { self.converted_text(s.id, *n, &text) };
                    let applied = self.baselines.get(s.id).cloned().unwrap_or_else(|| text.clone());
                    let bad = parse_err
                        || (!applied.trim().is_empty() && noshape_units::evaluate_as(&applied, n.kind.dim(), &self.units, &self.variables).is_err())
                        || self.out_of_range(s.id, &applied)
                        || self.field_error(s.id).is_some();
                    fields.push(json!({ "label": label, "kind": "number", "text": shown.unwrap_or(text), "state": if bad { "error" } else { "ok" }, "focused": focused }));
                }
                ParamKind::Text { .. } => {
                    let (focused, text) = self.input_of(s.id, window, cx).map(|(f, t, _)| (f, t)).unwrap_or_else(|| (false, values.text(s.id).to_string()));
                    fields.push(json!({ "label": label, "kind": "text", "text": text, "focused": focused, "state": "ok" }));
                }
                ParamKind::Query { .. } => {
                    let refs = values.query(s.id).to_vec();
                    let items: Vec<String> = self.query_rows(s, &refs).into_iter().map(|r| r.0).collect();
                    // Missing picks are red; an empty field the feature's error blames isn't (live: a new Extrude's).
                    let missing = refs.iter().any(|r| self.ref_missing(r)) || (!refs.is_empty() && self.field_error(s.id).is_some());
                    fields.push(json!({ "label": label, "kind": "query", "items": items, "active": self.active_query.as_deref() == Some(s.id), "state": if missing { "error" } else { "ok" } }));
                }
                // A table (the Hole's type and size): one select per level, its name drawn beside it (live: no label).
                ParamKind::Lookup { table } => {
                    let cur = values.lookup(s.id).to_vec();
                    let mut prefix: Vec<String> = Vec::new();
                    for level in 0..7 {
                        let opts = noshape_model::features::hole_tables::table_levels(table, &prefix);
                        if opts.is_empty() {
                            break;
                        }
                        let chosen = cur.get(level).cloned().or_else(|| noshape_model::features::hole_tables::table_level_default(table, &prefix)).unwrap_or_default();
                        let text = opts.iter().find(|o| o.0 == chosen).map(|o| o.1.to_string()).unwrap_or_else(|| chosen.clone());
                        fields.push(json!({ "label": null, "kind": "enum", "text": text, "state": "ok" }));
                        prefix.push(chosen);
                    }
                }
                // An array: live reads its box's text (its label, its entries and an item array's add button).
                ParamKind::Array { item, item_label } => {
                    let specs = item_slice(item);
                    let q = specs.iter().find(|p| matches!(p.kind, ParamKind::Query { .. }));
                    let mut text = vec![crate::i18n::spec(s.label).to_string()];
                    for (k, it) in values.array(s.id).iter().enumerate() {
                        text.push(match s.item_template {
                            Some(t) => self.item_label(t, it, specs, s.id, k),
                            None => q.and_then(|q| it.query(q.id).first()).map(|r| (self.labeler)(r)).unwrap_or_default(),
                        });
                    }
                    if !(s.hints.has(Hints::DRIVEN_QUERY) || s.hints.has(Hints::ALLOW_ARRAY_FOCUS)) {
                        text.push(format!("Add {item_label}"));
                    }
                    fields.push(json!({ "label": label, "kind": "other", "text": text.join(" "), "state": "ok" }));
                }
                _ => {}
            }
        }
        // Errors show as the red title (its tooltip says why); the dialog has no message text of its own.
        let red = self.applied_value_error().is_some() || matches!(self.status.as_ref().map(|s| s.0), Some(FeatureStatus::Error | FeatureStatus::Warning));
        let kind = if self.feature.type_id == "sketch" { "sketch" } else { "feature" };
        json!({ "title": self.title_text(), "titleState": if red { "error" } else { "ok" }, "kind": kind, "tabs": tabs, "fields": fields, "error": null })
    }

    /// `state.list` for the open autofill list.
    pub fn ui_list(&self) -> Option<Value> {
        self.completion.as_ref().map(|c| c.ui_state())
    }
}
