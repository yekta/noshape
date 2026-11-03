//! The open dialog as the interaction parity suite records it
//! (tests/ui-parity/README.md, `state.dialog`): its title, tabs and visible
//! fields as shown, read from the dialog's own state.

use super::*;
use serde_json::{Value, json};

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
        let (path, (_, text, sel)) = self.inputs.keys().find_map(|p| self.input_of(p, window, cx).filter(|i| i.0).map(|i| (p.clone(), i)))?;
        let label = self.spec_for_path(&path).map(|s| crate::i18n::spec(s.label)).unwrap_or(path);
        let chars = |b: usize| text.get(..b.min(text.len())).map_or(0, |t| t.chars().count());
        let (a, b) = (chars(sel.start), chars(sel.end));
        Some(json!({ "kind": "field", "label": label, "text": text, "selection": [a, b], "all": a == 0 && b == text.chars().count() && b > 0 }))
    }

    /// `state.dialog`.
    pub fn ui_state(&self, window: &Window, cx: &App) -> Value {
        let mut tabs = Vec::new();
        let mut fields = Vec::new();
        let values = &self.feature.params;
        for s in self.ty.params.iter().filter(|s| s.is_visible(values)) {
            let label = crate::i18n::spec(s.label);
            match &s.kind {
                ParamKind::Enum { options, .. } if s.hints.has(Hints::HORIZONTAL_ENUM) => {
                    let cur = values.enum_(s.id);
                    tabs.extend(options.iter().map(|(id, l)| json!({ "label": crate::i18n::spec(l), "selected": cur == *id })));
                }
                ParamKind::Enum { options, .. } => {
                    let cur = values.enum_(s.id);
                    let text = options.iter().find(|(id, _)| *id == cur).map(|(_, l)| crate::i18n::spec(l)).unwrap_or_default();
                    fields.push(json!({ "label": label, "kind": "enum", "text": text }));
                }
                ParamKind::Bool { .. } => fields.push(json!({ "label": label, "kind": "boolean", "checked": values.bool(s.id) })),
                ParamKind::Num(n) => {
                    let expr = values.expr(s.id).map(|e| e.text.clone()).unwrap_or_default();
                    let (focused, text) = match self.input_of(s.id, window, cx) {
                        Some((f, t, _)) => (f, t),
                        None => (false, expr),
                    };
                    let parse_ok = !self.parse_errors.contains_key(s.id);
                    let shown = if focused || !parse_ok { None } else { self.converted_text(s.id, *n, &text) };
                    let applied = self.baselines.get(s.id).cloned().unwrap_or_else(|| text.clone());
                    let bad = !parse_ok
                        || (!applied.trim().is_empty() && noshape_units::evaluate_as(&applied, n.kind.dim(), &self.units, &self.variables).is_err())
                        || self.out_of_range(s.id, &applied)
                        || self.field_error(s.id).is_some();
                    fields.push(json!({ "label": label, "kind": "number", "text": shown.unwrap_or(text), "state": if bad { "error" } else { "ok" }, "focused": focused }));
                }
                ParamKind::Text { .. } => {
                    let (focused, text) = self.input_of(s.id, window, cx).map(|(f, t, _)| (f, t)).unwrap_or_else(|| (false, values.text(s.id).to_string()));
                    fields.push(json!({ "label": label, "kind": "text", "text": text, "focused": focused }));
                }
                ParamKind::Query { .. } => {
                    let refs = values.query(s.id).to_vec();
                    let items: Vec<String> = self.query_rows(s, &refs).into_iter().map(|r| r.0).collect();
                    fields.push(json!({ "label": label, "kind": "query", "items": items, "active": self.active_query.as_deref() == Some(s.id) }));
                }
                _ => {}
            }
        }
        let error = self.status.as_ref().filter(|s| matches!(s.0, FeatureStatus::Error | FeatureStatus::Warning)).map(|s| s.1.clone());
        let kind = if self.feature.type_id == "sketch" { "sketch" } else { "feature" };
        json!({ "title": self.title_text(), "kind": kind, "tabs": tabs, "fields": fields, "error": error })
    }

    /// `state.list` for the open autofill list.
    pub fn ui_list(&self) -> Option<Value> {
        self.completion.as_ref().map(|c| c.ui_state())
    }
}
