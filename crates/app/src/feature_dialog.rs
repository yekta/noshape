//! Generic feature dialog rendered from a feature's parameter schema.
//!
//! Behaves like Onshape's feature dialogs: fields appear/disappear live from
//! visibility predicates, flip arrows sit at the end of the previous row,
//! the focused selection field receives viewport picks, Enter accepts,
//! Shift+Enter accepts and repeats, Esc cancels.

use crate::assets::icon_path;
use crate::theme::{p, with_alpha};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::DropdownMenu as _;
use gpui_kit::component::{Icon, IconName, Selectable as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::display::FeatureStatus;
use noshape_model::features::FeatureType;
use noshape_model::params::{Hints, ParamKind, ParamSpec, ParamValue, ParamValues, QueryFilter, fill_defaults};
use noshape_model::partstudio::Feature;
use noshape_model::reference::Reference;
use noshape_units::UnitPrefs;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub enum DialogEvent {
    /// The draft changed; regenerate the preview.
    Changed,
    Accept { repeat: bool },
    Cancel,
    /// A selection field gained focus (viewport picks go there).
    QueryFocused { path: String, filter: QueryFilter },
}

pub type RefLabeler = Arc<dyn Fn(&Reference) -> String + Send + Sync>;

/// A Part Studio that can be used as a Derived source, with its parts.
#[derive(Clone, Debug)]
pub struct DerivedSource {
    pub tab: String,
    pub name: String,
    pub parts: Vec<(noshape_model::BodyId, String)>,
}

pub struct FeatureDialog {
    pub feature: Feature,
    pub ty: &'static FeatureType,
    pub is_new: bool,
    units: UnitPrefs,
    inputs: HashMap<String, Entity<InputState>>,
    pub active_query: Option<String>,
    subs: Vec<Subscription>,
    parse_errors: HashMap<String, String>,
    pub status: Option<(FeatureStatus, String)>,
    labeler: RefLabeler,
    collapsed_groups: std::collections::HashSet<String>,
    focus: FocusHandle,
    renaming: Option<Entity<InputState>>,
    /// Sources offered by the Derived feature's Part Studio picker.
    pub derived_sources: Vec<DerivedSource>,
    /// Preview the end of the history instead of rolling to this feature.
    pub show_final: bool,
}

impl EventEmitter<DialogEvent> for FeatureDialog {}

impl Focusable for FeatureDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// Path helpers: top-level params use their id; array items use "array.i.param".
fn item_path(array: &str, i: usize, id: &str) -> String {
    format!("{array}.{i}.{id}")
}

impl FeatureDialog {
    pub fn new(feature: Feature, is_new: bool, units: UnitPrefs, labeler: RefLabeler, cx: &mut Context<Self>) -> Self {
        let ty = feature.feature_type().expect("registered feature type");
        let mut feature = feature;
        fill_defaults(&ty.params, &mut feature.params, &units);
        let mut d = Self {
            feature,
            ty,
            is_new,
            units,
            inputs: HashMap::new(),
            active_query: None,
            subs: Vec::new(),
            parse_errors: HashMap::new(),
            status: None,
            labeler,
            collapsed_groups: Default::default(),
            focus: cx.focus_handle(),
            renaming: None,
            derived_sources: Vec::new(),
            show_final: false,
        };
        d.active_query = d.first_visible_query();
        d
    }

    /// First visible query parameter (initial selection focus).
    pub fn first_visible_query(&self) -> Option<String> {
        self.ty
            .params
            .iter()
            .filter(|s| s.is_visible(&self.feature.params))
            .find(|s| matches!(s.kind, ParamKind::Query { .. }))
            .map(|s| s.id.to_string())
    }

    pub fn active_filter(&self) -> Option<(String, QueryFilter, Option<usize>)> {
        let path = self.active_query.clone()?;
        let spec = self.spec_for_path(&path)?;
        match spec.kind {
            ParamKind::Query { filter, max_picks } => Some((path, filter, max_picks)),
            _ => None,
        }
    }

    fn spec_for_path(&self, path: &str) -> Option<&'static ParamSpec> {
        let parts: Vec<&str> = path.split('.').collect();
        match parts.as_slice() {
            [id] => self.ty.params.iter().find(|s| s.id == *id),
            [arr, _, id] => {
                let a = self.ty.params.iter().find(|s| s.id == *arr)?;
                match &a.kind {
                    ParamKind::Array { item, .. } => item.iter().find(|s| s.id == *id),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn values_for_path_mut(&mut self, path: &str) -> Option<(&mut ParamValues, String)> {
        let parts: Vec<&str> = path.split('.').collect();
        match parts.as_slice() {
            [id] => Some((&mut self.feature.params, id.to_string())),
            [arr, i, id] => {
                let i: usize = i.parse().ok()?;
                match self.feature.params.0.get_mut(*arr) {
                    Some(ParamValue::Array(items)) => items.get_mut(i).map(|v| (v, id.to_string())),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn get_value(&self, path: &str) -> Option<&ParamValue> {
        let parts: Vec<&str> = path.split('.').collect();
        match parts.as_slice() {
            [id] => self.feature.params.get(id),
            [arr, i, id] => {
                let i: usize = i.parse().ok()?;
                match self.feature.params.get(arr) {
                    Some(ParamValue::Array(items)) => items.get(i).and_then(|v| v.get(id)),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub fn set_value(&mut self, path: &str, v: ParamValue, cx: &mut Context<Self>) {
        if let Some((vals, id)) = self.values_for_path_mut(path) {
            vals.set(&id, v);
        }
        cx.emit(DialogEvent::Changed);
        cx.notify();
    }

    /// Set a numeric field from outside (manipulator drag): updates the value
    /// and the visible input text.
    pub fn set_expr_text(&mut self, path: &str, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(st) = self.inputs.get(path).cloned() {
            st.update(cx, |s, cx| s.set_value(text.to_string(), window, cx));
        }
        self.parse_errors.remove(path);
        self.set_value(path, ParamValue::Expr(noshape_units::ExprValue::new(text)), cx);
    }

    /// Add a picked reference to the active selection field. Returns false if
    /// no field is active.
    pub fn add_pick(&mut self, r: Reference, cx: &mut Context<Self>) -> bool {
        let Some((path, _filter, max)) = self.active_filter() else { return false };
        let Some((vals, id)) = self.values_for_path_mut(&path) else { return false };
        let q = vals.query_mut(&id);
        if let Some(pos) = q.iter().position(|x| *x == r) {
            // Clicking a selected entity again toggles it off (Onshape behavior).
            q.remove(pos);
        } else if max == Some(1) {
            q.clear();
            q.push(r);
        } else {
            q.push(r);
        }
        let single = max == Some(1);
        // Single-pick fields advance focus to the next empty selection field.
        if single {
            self.advance_query_focus(&path);
        }
        cx.emit(DialogEvent::Changed);
        cx.notify();
        true
    }

    fn advance_query_focus(&mut self, from: &str) {
        let visible: Vec<&ParamSpec> = self.ty.params.iter().filter(|s| s.is_visible(&self.feature.params)).collect();
        let pos = visible.iter().position(|s| s.id == from);
        if let Some(pos) = pos {
            for s in &visible[pos + 1..] {
                if matches!(s.kind, ParamKind::Query { .. }) && self.feature.params.query(s.id).is_empty() {
                    self.active_query = Some(s.id.to_string());
                    return;
                }
            }
        }
    }

    pub fn selected_refs(&self) -> Vec<Reference> {
        let mut out = Vec::new();
        for s in &self.ty.params {
            if !s.is_visible(&self.feature.params) {
                continue;
            }
            match &s.kind {
                ParamKind::Query { .. } => out.extend(self.feature.params.query(s.id).iter().cloned()),
                ParamKind::Array { item, .. } => {
                    for it in self.feature.params.array(s.id) {
                        for is in item.iter() {
                            if matches!(is.kind, ParamKind::Query { .. }) {
                                out.extend(it.query(is.id).iter().cloned());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// Refs of the active field (highlighted in the selection color).
    pub fn active_refs(&self) -> Vec<Reference> {
        match self.active_query.as_deref().and_then(|p| self.get_value(p)) {
            Some(ParamValue::Query(q)) => q.clone(),
            _ => Vec::new(),
        }
    }

    /// Mutable sketch payload of a Sketch feature draft.
    pub fn sketch_data_mut(&mut self) -> Option<&mut noshape_model::sketch_feature::SketchData> {
        match self.feature.params.0.get_mut("sketch") {
            Some(ParamValue::Sketch(d)) => Some(d),
            _ => None,
        }
    }

    pub fn sketch_data(&self) -> Option<&noshape_model::sketch_feature::SketchData> {
        match self.feature.params.get("sketch") {
            Some(ParamValue::Sketch(d)) => Some(d),
            _ => None,
        }
    }

    /// Notify listeners that the draft changed (e.g. after sketch edits).
    pub fn mark_changed(&mut self, cx: &mut Context<Self>) {
        cx.emit(DialogEvent::Changed);
        cx.notify();
    }

    pub fn set_status(&mut self, s: Option<(FeatureStatus, String)>, cx: &mut Context<Self>) {
        if self.status != s {
            self.status = s;
            cx.notify();
        }
    }

    // ------------------------------------------------------------ inputs

    fn ensure_input(&mut self, path: &str, initial: &str, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        if let Some(i) = self.inputs.get(path) {
            return i.clone();
        }
        let state = cx.new(|cx| InputState::new(window, cx).default_value(initial.to_string()));
        let path_s = path.to_string();
        let sub = cx.subscribe_in(&state, window, move |this: &mut FeatureDialog, state, ev: &InputEvent, _window, cx| match ev {
            InputEvent::Change => {
                let text = state.read(cx).value().to_string();
                this.on_text_changed(&path_s, text, cx);
            }
            InputEvent::PressEnter { shift, .. } => cx.emit(DialogEvent::Accept { repeat: *shift }),
            InputEvent::Focus => {
                this.active_query = None;
                cx.notify();
            }
            InputEvent::Blur => {}
        });
        self.subs.push(sub);
        self.inputs.insert(path.to_string(), state.clone());
        state
    }

    fn on_text_changed(&mut self, path: &str, text: String, cx: &mut Context<Self>) {
        let Some(spec) = self.spec_for_path(path) else { return };
        let v = match spec.kind {
            ParamKind::Num(_) => {
                match noshape_units::parse(&text) {
                    Ok(_) => {
                        self.parse_errors.remove(path);
                    }
                    Err(e) => {
                        self.parse_errors.insert(path.to_string(), e.to_string());
                    }
                }
                ParamValue::Expr(noshape_units::ExprValue::new(text))
            }
            ParamKind::Text { .. } => ParamValue::Text(text),
            _ => return,
        };
        if let Some((vals, id)) = self.values_for_path_mut(path) {
            vals.set(&id, v);
        }
        cx.emit(DialogEvent::Changed);
        cx.notify();
    }

    // ------------------------------------------------------------ render helpers

    fn render_params(&mut self, specs: &'static [ParamSpec], prefix: Option<(&'static str, usize)>, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let values: ParamValues = match prefix {
            None => self.feature.params.clone(),
            Some((arr, i)) => self.feature.params.array(arr).get(i).cloned().unwrap_or_default(),
        };
        let mut rows: Vec<AnyElement> = Vec::new();
        let mut i = 0;
        let mut hidden_by_group: Option<&'static str> = None;
        while i < specs.len() {
            let s = &specs[i];
            i += 1;
            if !s.is_visible(&values) {
                continue;
            }
            let path = match prefix {
                None => s.id.to_string(),
                Some((arr, idx)) => item_path(arr, idx, s.id),
            };
            // Collapsed group: skip params until the next group header.
            if s.hints.has(Hints::GROUP_HEADER) {
                hidden_by_group = None;
            } else if hidden_by_group.is_some() {
                continue;
            }
            // Gather trailing flip toggles that belong on this row.
            let mut trailing: Vec<&'static ParamSpec> = Vec::new();
            let mut companion: Option<&'static ParamSpec> = None;
            if s.hints.has(Hints::FIRST_IN_ROW) && s.hints.has(Hints::DISPLAY_SHORT) && i < specs.len() {
                let n = &specs[i];
                if n.hints.has(Hints::DISPLAY_SHORT) && n.is_visible(&values) {
                    companion = Some(n);
                    i += 1;
                }
            }
            while i < specs.len() {
                let n = &specs[i];
                let is_flip = n.hints.has(Hints::OPPOSITE_DIRECTION) || n.hints.has(Hints::OPPOSITE_DIRECTION_CIRCULAR);
                if is_flip && matches!(n.kind, ParamKind::Bool { .. }) {
                    if n.is_visible(&values) {
                        trailing.push(n);
                    }
                    i += 1;
                } else {
                    break;
                }
            }
            let row = self.render_param(s, &path, &values, companion, &trailing, prefix, window, cx);
            if s.hints.has(Hints::GROUP_HEADER) && (!values.bool(s.id) || self.collapsed_groups.contains(&path)) {
                hidden_by_group = Some(s.id);
            }
            if let Some(row) = row {
                rows.push(row);
            }
        }
        rows
    }

    #[allow(clippy::too_many_arguments)]
    fn render_param(
        &mut self,
        s: &'static ParamSpec,
        path: &str,
        values: &ParamValues,
        companion: Option<&'static ParamSpec>,
        trailing: &[&'static ParamSpec],
        prefix: Option<(&'static str, usize)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let pal = p();
        let label_el = |text: &'static str| div().text_xs().text_color(pal.text_muted).child(text);
        let flips: Vec<AnyElement> = trailing
            .iter()
            .map(|f| {
                let fpath = match prefix {
                    None => f.id.to_string(),
                    Some((arr, idx)) => item_path(arr, idx, f.id),
                };
                let on = values.bool(f.id);
                let circular = f.hints.has(Hints::OPPOSITE_DIRECTION_CIRCULAR);
                let icon = if circular { "flip-circular" } else { "flip" };
                Button::new(SharedString::from(format!("flip-{fpath}")))
                    .icon(Icon::empty().path(icon_path(icon)))
                    .ghost()
                    .xsmall()
                    .selected(on)
                    .tooltip(f.label)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        let cur = matches!(this.get_value(&fpath), Some(ParamValue::Bool(true)));
                        this.set_value(&fpath, ParamValue::Bool(!cur), cx);
                    }))
                    .into_any_element()
            })
            .collect();

        let el: AnyElement = match &s.kind {
            ParamKind::Sketch => return None,
            ParamKind::Bool { .. } => {
                let on = values.bool(s.id);
                let path_s = path.to_string();
                let header = s.hints.has(Hints::GROUP_HEADER);
                let mut row = h_flex()
                    .gap_2()
                    .w_full()
                    .child(
                        gpui_kit::component::checkbox::Checkbox::new(SharedString::from(format!("cb-{path}")))
                            .label(s.label)
                            .checked(on)
                            .small()
                            .on_click(cx.listener(move |this, v: &bool, _, cx| this.set_value(&path_s, ParamValue::Bool(*v), cx))),
                    );
                if let Some(c) = companion {
                    let cpath = match prefix {
                        None => c.id.to_string(),
                        Some((arr, idx)) => item_path(arr, idx, c.id),
                    };
                    if let Some(e) = self.render_inline_value(c, &cpath, values, window, cx) {
                        row = row.child(div().flex_1().child(e));
                    }
                }
                let row = row.children(flips);
                if header {
                    div().pt_1().mt_1().border_t_1().border_color(pal.border).child(row).into_any_element()
                } else {
                    row.into_any_element()
                }
            }
            ParamKind::Enum { options, .. } => {
                let cur = values.enum_(s.id).to_string();
                if s.hints.has(Hints::HORIZONTAL_ENUM) {
                    let group = h_flex().gap_0p5().p_0p5().rounded_md().bg(pal.app_bg).w_full().children(options.iter().map(|(id, label)| {
                        let selected = cur == *id;
                        let path_s = path.to_string();
                        let id_s = id.to_string();
                        div()
                            .id(SharedString::from(format!("seg-{path}-{id}")))
                            .flex_1()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .text_xs()
                            .text_center()
                            .cursor_pointer()
                            .when(selected, |d| d.bg(pal.accent).text_color(rgb(0xffffff)))
                            .when(!selected, |d| d.text_color(pal.text_muted).hover(|d| d.bg(pal.panel_hover)))
                            .child(*label)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_value(&path_s, ParamValue::Enum(id_s.clone()), cx)))
                    }));
                    h_flex().gap_1().w_full().child(group).children(flips).into_any_element()
                } else {
                    let label = options.iter().find(|(id, _)| *id == cur).map(|(_, l)| *l).unwrap_or("");
                    let path_s = path.to_string();
                    let opts = *options;
                    let view = cx.entity();
                    let dd = Button::new(SharedString::from(format!("dd-{path}")))
                        .label(label)
                        .outline()
                        .small()
                        .dropdown_caret(true)
                        .dropdown_menu(move |mut menu, _, _| {
                            for (id, l) in opts.iter() {
                                let id_s = id.to_string();
                                let p2 = path_s.clone();
                                let v = view.clone();
                                menu = menu.item(gpui_kit::component::menu::PopupMenuItem::new(*l).on_click(move |_, _, cx| {
                                    v.update(cx, |this, cx| this.set_value(&p2, ParamValue::Enum(id_s.clone()), cx));
                                }));
                            }
                            menu
                        });
                    let mut row = h_flex().gap_2().w_full();
                    if s.hints.has(Hints::SHOW_LABEL) {
                        row = row.child(div().w(px(96.)).child(label_el(s.label)));
                    }
                    row.child(div().flex_1().child(dd)).children(flips).into_any_element()
                }
            }
            ParamKind::Text { .. } if s.id == "partStudio" && self.ty.id == "importDerived" => self.render_derived_picker(cx),
            ParamKind::Num(_) | ParamKind::Text { .. } => {
                let e = self.render_inline_value(s, path, values, window, cx)?;
                h_flex().gap_2().w_full().child(div().w(px(96.)).flex_none().child(label_el(s.label))).child(div().flex_1().child(e)).children(flips).into_any_element()
            }
            ParamKind::Query { max_picks, .. } => {
                let refs = values.query(s.id).to_vec();
                let active = self.active_query.as_deref() == Some(path);
                let path_s = path.to_string();
                let filter = match s.kind {
                    ParamKind::Query { filter, .. } => filter,
                    _ => QueryFilter::default(),
                };
                let chips: Vec<AnyElement> = refs
                    .iter()
                    .enumerate()
                    .map(|(k, r)| {
                        let label = (self.labeler)(r);
                        let p2 = path_s.clone();
                        h_flex()
                            .id(SharedString::from(format!("chip-{path}-{k}")))
                            .gap_1()
                            .px_1p5()
                            .py_0p5()
                            .rounded_sm()
                            .bg(pal.panel_hover)
                            .text_xs()
                            .child(div().text_color(pal.text).child(label))
                            .child(
                                div()
                                    .id(SharedString::from(format!("chipx-{path}-{k}")))
                                    .text_color(pal.text_faint)
                                    .hover(|d| d.text_color(pal.error))
                                    .cursor_pointer()
                                    .child("×")
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        if let Some((vals, id)) = this.values_for_path_mut(&p2) {
                                            let q = vals.query_mut(&id);
                                            if k < q.len() {
                                                q.remove(k);
                                            }
                                        }
                                        cx.emit(DialogEvent::Changed);
                                        cx.notify();
                                    })),
                            )
                            .into_any_element()
                    })
                    .collect();
                let placeholder = if refs.is_empty() {
                    Some(div().text_xs().text_color(if active { pal.accent_hover } else { pal.text_faint }).child(if max_picks == &Some(1) { "Select an entity" } else { "Select entities" }))
                } else {
                    None
                };
                v_flex()
                    .gap_1()
                    .w_full()
                    .child(h_flex().justify_between().child(label_el(s.label)).children(flips))
                    .child(
                        div()
                            .id(SharedString::from(format!("q-{path}")))
                            .w_full()
                            .min_h(px(30.))
                            .p_1()
                            .rounded_md()
                            .border_1()
                            .border_color(if active { pal.accent } else { pal.border })
                            .bg(if active { with_alpha(pal.accent, 0.08) } else { pal.app_bg })
                            .cursor_pointer()
                            .flex()
                            .flex_wrap()
                            .gap_1()
                            .children(chips)
                            .children(placeholder)
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                this.active_query = Some(path_s.clone());
                                window.focus(&this.focus, cx);
                                cx.emit(DialogEvent::QueryFocused { path: path_s.clone(), filter });
                                cx.notify();
                            })),
                    )
                    .into_any_element()
            }
            ParamKind::Array { item, item_label } => {
                let items = values.array(s.id).len();
                let arr_id = s.id;
                let item_specs: &'static [ParamSpec] = item_slice(item);
                let mut col = v_flex().gap_1().w_full().child(
                    h_flex()
                        .justify_between()
                        .child(label_el(s.label))
                        .child(
                            Button::new(SharedString::from(format!("add-{path}")))
                                .icon(IconName::Plus)
                                .ghost()
                                .xsmall()
                                .tooltip(format!("Add {}", item_label.to_lowercase()))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let mut v = ParamValues::default();
                                    fill_defaults(item_specs, &mut v, &this.units);
                                    let arr = this.feature.params.0.entry(arr_id.to_string()).or_insert_with(|| ParamValue::Array(vec![]));
                                    if let ParamValue::Array(a) = arr {
                                        a.push(v);
                                        // New items take focus on their first selection field.
                                        if let Some(q) = item_specs.iter().find(|p| matches!(p.kind, ParamKind::Query { .. })) {
                                            this.active_query = Some(item_path(arr_id, a.len() - 1, q.id));
                                        }
                                    }
                                    cx.emit(DialogEvent::Changed);
                                    cx.notify();
                                })),
                        ),
                );
                for k in 0..items {
                    let inner = self.render_params(item_specs, Some((arr_id, k)), window, cx);
                    col = col.child(
                        v_flex()
                            .gap_1()
                            .p_1p5()
                            .rounded_md()
                            .border_1()
                            .border_color(pal.border)
                            .child(
                                h_flex()
                                    .justify_between()
                                    .child(div().text_xs().text_color(pal.text_faint).child(format!("{item_label} {}", k + 1)))
                                    .child(
                                        Button::new(SharedString::from(format!("rm-{path}-{k}")))
                                            .icon(IconName::Close)
                                            .ghost()
                                            .xsmall()
                                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                if let Some(ParamValue::Array(a)) = this.feature.params.0.get_mut(arr_id)
                                                    && k < a.len()
                                                {
                                                    a.remove(k);
                                                }
                                                this.inputs.retain(|p, _| !p.starts_with(&format!("{arr_id}.")));
                                                cx.emit(DialogEvent::Changed);
                                                cx.notify();
                                            })),
                                    ),
                            )
                            .children(inner),
                    );
                }
                col.into_any_element()
            }
            ParamKind::Lookup { table } => {
                let cur = values.lookup(s.id).to_vec();
                let path_s = path.to_string();
                let table: &'static str = table;
                let mut row = v_flex().gap_1().w_full().child(label_el(s.label));
                let mut prefix_path: Vec<String> = Vec::new();
                let mut level = 0;
                loop {
                    let opts = noshape_model::features::hole_tables::table_levels(table, &prefix_path);
                    if opts.is_empty() || level > 6 {
                        break;
                    }
                    let name = noshape_model::features::hole_tables::table_level_name(table, &prefix_path).unwrap_or("");
                    let chosen = cur.get(level).cloned().or_else(|| noshape_model::features::hole_tables::table_level_default(table, &prefix_path)).unwrap_or_default();
                    let label = opts.iter().find(|o| o.0 == chosen).map(|o| o.1.to_string()).unwrap_or(chosen.clone());
                    let lvl = level;
                    let ps2 = path_s.clone();
                    let base_path = prefix_path.clone();
                    let opts2: Vec<(String, String)> = opts.iter().map(|o| (o.0.to_string(), o.1.to_string())).collect();
                    let view = cx.entity();
                    row = row.child(
                        h_flex().gap_2().child(div().w(px(96.)).text_xs().text_color(pal.text_faint).child(name)).child(
                            Button::new(SharedString::from(format!("lk-{path}-{level}")))
                                .label(label)
                                .outline()
                                .small()
                                .dropdown_caret(true)
                                .dropdown_menu(move |mut menu, _, _| {
                                    for (id, l) in &opts2 {
                                        let mut np = base_path.clone();
                                        np.truncate(lvl);
                                        np.push(id.clone());
                                        let p2 = ps2.clone();
                                        let v = view.clone();
                                        menu = menu.item(gpui_kit::component::menu::PopupMenuItem::new(l.clone()).on_click(move |_, _, cx| {
                                            v.update(cx, |this, cx| this.set_lookup(&p2, np.clone(), cx));
                                        }));
                                    }
                                    menu
                                }),
                        ),
                    );
                    prefix_path.push(chosen);
                    level += 1;
                }
                row.into_any_element()
            }
        };
        Some(el)
    }

    /// Part Studio picker and part checklist for Derived.
    fn render_derived_picker(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let current = self.feature.params.text("partStudio").to_string();
        let selected_parts: Vec<noshape_model::BodyId> = self.feature.params.query("parts").iter().filter_map(|r| match r {
            Reference::Body(b) => Some(b.clone()),
            _ => None,
        }).collect();
        let sources = self.derived_sources.clone();
        let label = sources.iter().find(|s| s.tab == current).map(|s| s.name.clone()).unwrap_or_else(|| "Select a Part Studio".into());
        let view = cx.entity();
        let picker = Button::new("derived-src")
            .label(label)
            .outline()
            .small()
            .dropdown_caret(true)
            .dropdown_menu({
                let sources = sources.clone();
                move |mut m, _, _| {
                    if sources.is_empty() {
                        m = m.label("No other Part Studios in this document");
                    }
                    for s in &sources {
                        let v = view.clone();
                        let tab = s.tab.clone();
                        m = m.item(gpui_kit::component::menu::PopupMenuItem::new(s.name.clone()).on_click(move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                this.feature.params.set("parts", ParamValue::Query(Vec::new()));
                                this.set_value("partStudio", ParamValue::Text(tab.clone()), cx);
                            })
                        }));
                    }
                    m
                }
            });
        let parts = sources.iter().find(|s| s.tab == current).map(|s| s.parts.clone()).unwrap_or_default();
        let checks: Vec<AnyElement> = parts
            .into_iter()
            .map(|(id, name)| {
                let on = selected_parts.is_empty() || selected_parts.contains(&id);
                let id2 = id.clone();
                gpui_kit::component::checkbox::Checkbox::new(SharedString::from(format!("dp-{id}")))
                    .label(name)
                    .checked(on)
                    .small()
                    .on_click(cx.listener(move |this, v: &bool, _, cx| {
                        let all: Vec<noshape_model::BodyId> = this
                            .derived_sources
                            .iter()
                            .find(|s| s.tab == this.feature.params.text("partStudio"))
                            .map(|s| s.parts.iter().map(|p| p.0.clone()).collect())
                            .unwrap_or_default();
                        let mut sel: Vec<noshape_model::BodyId> = this.feature.params.query("parts").iter().filter_map(|r| match r {
                            Reference::Body(b) => Some(b.clone()),
                            _ => None,
                        }).collect();
                        if sel.is_empty() {
                            sel = all.clone();
                        }
                        sel.retain(|b| *b != id2);
                        if *v {
                            sel.push(id2.clone());
                        }
                        // All selected is stored as "all parts" (empty).
                        let q = if sel.len() == all.len() { Vec::new() } else { sel.into_iter().map(Reference::Body).collect() };
                        this.set_value("parts", ParamValue::Query(q), cx);
                    }))
                    .into_any_element()
            })
            .collect();
        v_flex()
            .gap_1p5()
            .w_full()
            .child(div().text_xs().text_color(pal.text_muted).child("Part Studio"))
            .child(picker)
            .children(checks)
            .into_any_element()
    }

    /// Input element for a numeric or text value.
    fn render_inline_value(&mut self, s: &ParamSpec, path: &str, values: &ParamValues, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let text = match (&s.kind, values.get(s.id)) {
            (ParamKind::Num(_), Some(ParamValue::Expr(e))) => e.text.clone(),
            (ParamKind::Text { .. }, Some(ParamValue::Text(t))) => t.clone(),
            (ParamKind::Num(_), _) | (ParamKind::Text { .. }, _) => String::new(),
            _ => return None,
        };
        let state = self.ensure_input(path, &text, window, cx);
        let err = self.parse_errors.get(path).cloned();
        let pal = p();
        Some(
            v_flex()
                .gap_0p5()
                .child(Input::new(&state).small().when(err.is_some(), |i| i.border_color(pal.error)))
                .when_some(err, |d, e| d.child(div().text_xs().text_color(pal.error).child(e)))
                .into_any_element(),
        )
    }

    /// Set a lookup path, completing lower levels with their defaults.
    fn set_lookup(&mut self, path: &str, lp: Vec<String>, cx: &mut Context<Self>) {
        let spec = self.spec_for_path(path);
        let mut full = lp;
        if let Some(ParamKind::Lookup { table }) = spec.map(|s| &s.kind) {
            while let Some(d) = noshape_model::features::hole_tables::table_level_default(table, &full) {
                full.push(d);
                if full.len() > 8 {
                    break;
                }
            }
        }
        self.set_value(path, ParamValue::Lookup(full), cx);
        if self.feature.type_id == "hole" {
            // Copy the standard's sizes into the numeric fields (Onshape behavior)
            // and rebuild the inputs so they show the new values.
            noshape_model::features::hole_tables::apply_table_to_params(&mut self.feature.params);
            self.inputs.clear();
            cx.emit(DialogEvent::Changed);
        }
    }
}


/// Feature schemas are static (registry), so array item specs live forever.
fn item_slice(item: &Arc<Vec<ParamSpec>>) -> &'static [ParamSpec] {
    // SAFETY-free: the registry is a `LazyLock` static that is never dropped,
    // so the Arc it owns lives for the whole program. Leak a clone of the Arc
    // once per call site; the Vec itself is shared, not copied.
    let leaked: &'static Arc<Vec<ParamSpec>> = Box::leak(Box::new(item.clone()));
    leaked.as_slice()
}

impl Render for FeatureDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let params: &'static [ParamSpec] = self.ty.params.as_slice();
        let rows = self.render_params(params, None, window, cx);
        let status = self.status.clone();
        let title_color = match status.as_ref().map(|s| s.0) {
            Some(FeatureStatus::Error) => pal.error,
            Some(FeatureStatus::Warning) => pal.warning,
            _ => pal.text,
        };
        let name = self.feature.name.clone();
        let renaming = self.renaming.clone();
        v_flex()
            .id("feature-dialog")
            .key_context("FeatureDialog")
            .track_focus(&self.focus)
            .w(px(318.))
            .max_h(px(720.))
            .rounded_lg()
            .border_1()
            .border_color(pal.border_strong)
            .bg(pal.panel_raised)
            .shadow_lg()
            .text_color(pal.text)
            .on_action(cx.listener(|_, _: &crate::actions::Accept, _, cx| cx.emit(DialogEvent::Accept { repeat: false })))
            .on_action(cx.listener(|_, _: &crate::actions::AcceptAndRepeat, _, cx| cx.emit(DialogEvent::Accept { repeat: true })))
            .on_action(cx.listener(|_, _: &crate::actions::Cancel, _, cx| cx.emit(DialogEvent::Cancel)))
            .child(
                h_flex()
                    .px_3()
                    .py_2()
                    .gap_2()
                    .border_b_1()
                    .border_color(pal.border)
                    .child(svg().path(icon_path(self.ty.icon)).size_4().text_color(pal.accent))
                    .child(match renaming {
                        Some(state) => div().flex_1().child(Input::new(&state).xsmall()).into_any_element(),
                        None => div()
                            .id("dialog-title")
                            .flex_1()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(title_color)
                            .truncate()
                            .child(name)
                            .on_click(cx.listener(|this, ev: &ClickEvent, window, cx| {
                                if ev.click_count() >= 2 {
                                    let name = this.feature.name.clone();
                                    let st = cx.new(|cx| InputState::new(window, cx).default_value(name));
                                    let sub = cx.subscribe_in(&st, window, |this: &mut FeatureDialog, st, ev: &InputEvent, _, cx| {
                                        if matches!(ev, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                                            let v = st.read(cx).value().to_string();
                                            if !v.trim().is_empty() {
                                                this.feature.name = v;
                                                this.feature.custom_name = true;
                                            }
                                            this.renaming = None;
                                            cx.emit(DialogEvent::Changed);
                                            cx.notify();
                                        }
                                    });
                                    this.subs.push(sub);
                                    st.update(cx, |s, cx| s.focus(window, cx));
                                    this.renaming = Some(st);
                                    cx.notify();
                                }
                            }))
                            .into_any_element(),
                    })
                    .child(
                        Button::new("dlg-ok")
                            .icon(Icon::empty().path(icon_path("check")))
                            .ghost()
                            .xsmall()
                            .tooltip("Accept (Enter)")
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(DialogEvent::Accept { repeat: false }))),
                    )
                    .child(
                        Button::new("dlg-cancel")
                            .icon(IconName::Close)
                            .ghost()
                            .xsmall()
                            .tooltip("Cancel (Esc)")
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(DialogEvent::Cancel))),
                    ),
            )
            .child(v_flex().id("dialog-body").gap_2p5().p_3().overflow_y_scroll().children(rows))
            .child(
                h_flex()
                    .px_3()
                    .py_1p5()
                    .gap_2()
                    .border_t_1()
                    .border_color(pal.border)
                    .text_xs()
                    .text_color(pal.text_muted)
                    .child(
                        gpui_kit::component::checkbox::Checkbox::new("dlg-final")
                            .checked(self.show_final)
                            .label("Final")
                            .xsmall()
                            .tooltip("Preview the finished model instead of rolling back to this feature")
                            .on_click(cx.listener(|this, v: &bool, _, cx| {
                                this.show_final = *v;
                                cx.emit(DialogEvent::Changed);
                                cx.notify();
                            })),
                    ),
            )
            .when_some(status.filter(|s| s.0 != FeatureStatus::Ok), |d, (st, msg)| {
                let c = match st {
                    FeatureStatus::Error => pal.error,
                    FeatureStatus::Warning => pal.warning,
                    _ => pal.info,
                };
                d.child(div().mx_3().mb_3().p_2().rounded_md().bg(with_alpha(c, 0.12)).border_1().border_color(with_alpha(c, 0.4)).text_xs().text_color(c).child(msg))
            })
    }
}
