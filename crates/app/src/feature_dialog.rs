//! Generic feature dialog rendered from a feature's parameter schema.
//!
//! Behaves like Onshape's feature dialogs: fields appear/disappear live from
//! visibility predicates, flip arrows sit at the end of the previous row,
//! the focused selection field receives viewport picks, Enter accepts,
//! Shift+Enter accepts and repeats, Esc cancels.

use crate::theme::{p, with_alpha};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _};
use gpui_kit::component::{Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::display::FeatureStatus;
use noshape_model::features::FeatureType;
use noshape_model::params::{Hints, ParamKind, ParamSpec, ParamValue, ParamValues, QueryFilter, fill_defaults};
use noshape_model::partstudio::Feature;
use noshape_model::reference::{FeatureId, Reference};
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
    QueryFocused,
    /// The pointer is over a selection chip (Onshape highlights the entity).
    HoverRef(Option<Reference>),
    /// Preview slider: 0 = the model before the feature, 1 = after.
    PreviewOpacity(f32),
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
    /// "Other document…" chooser: the document, the selected version index.
    ext_pick: Option<(crate::derived::ExternalDocInfo, usize)>,
    /// Resolved external source: key, label, parts, newer version.
    ext_info: Option<ExtInfo>,
    ext_task: Option<Task<()>>,
    /// Parameters the user changed in this session (automatic choices such
    /// as Extrude's Add/Remove direction leave them alone).
    pub user_set: std::collections::HashSet<String>,
    /// Variables visible to the feature (evaluated value hints and wheel
    /// nudging of expressions).
    pub variables: std::collections::BTreeMap<String, noshape_units::Quantity>,
    /// "Final" is offered only when later features exist (Onshape hides it
    /// for new features and for the last feature).
    pub final_available: bool,
    /// Selection field flashing after a rejected pick.
    flash: Option<String>,
    /// Help text shown under the title ("?").
    show_help: bool,
    /// Preview slider (before/after blend).
    preview: f32,
    /// Track bounds of the preview slider (last paint), for dragging.
    preview_track: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>>,
    /// Where the title was grabbed when dragging the dialog.
    drag_grab: Arc<parking_lot::Mutex<Point<Pixels>>>,
    /// Whether a reference is missing (its feature was deleted/suppressed).
    pub missing: Arc<dyn Fn(&Reference) -> bool + Send + Sync>,
    /// Number of regions of a sketch (a field holding all of them shows
    /// one "Faces of Sketch 1" row).
    pub sketch_region_count: Arc<dyn Fn(FeatureId) -> usize + Send + Sync>,
    /// Open `#` variable autofill list of a numeric field.
    completion: Option<crate::var_complete::VarCompletion>,
    /// Field text when it was focused or last applied with Enter: Enter
    /// applies a changed value and Esc reverts it before they accept or
    /// cancel the dialog (Onshape).
    baselines: HashMap<String, String>,
    /// Enter applied a field value; the same keystroke's Accept is dropped.
    enter_applied: bool,
    /// Rows the owner adds under the parameters and a footer element left
    /// of `?` (the Sketch dialog's display options and diagnostics button).
    pub extra: Option<DialogExtra>,
}

/// See [`FeatureDialog::extra`].
pub type DialogExtra = Arc<dyn Fn(&mut Window, &mut Context<FeatureDialog>) -> (Vec<AnyElement>, Option<AnyElement>)>;

#[derive(Clone)]
struct ExtInfo {
    key: String,
    label: String,
    parts: Vec<(noshape_model::BodyId, String)>,
    newer: Option<crate::derived::VersionChoice>,
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
    pub fn new(feature: Feature, units: UnitPrefs, labeler: RefLabeler, cx: &mut Context<Self>) -> Self {
        let ty = feature.feature_type().expect("registered feature type");
        let mut feature = feature;
        fill_defaults(&ty.params, &mut feature.params, &units);
        let mut d = Self {
            feature,
            ty,
            units,
            inputs: HashMap::new(),
            baselines: HashMap::new(),
            enter_applied: false,
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
            ext_pick: None,
            ext_info: None,
            ext_task: None,
            user_set: Default::default(),
            variables: Default::default(),
            final_available: true,
            flash: None,
            show_help: false,
            preview: 1.0,
            preview_track: Default::default(),
            drag_grab: Default::default(),
            missing: Arc::new(|_| false),
            sketch_region_count: Arc::new(|_| 0),
            completion: None,
            extra: None,
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

    /// Onshape: when an edit hides the active selection field (Part pattern
    /// switched to Feature pattern, say), the first visible one becomes active
    /// so picks keep going somewhere.
    fn refocus_hidden_query(&mut self) {
        let Some(path) = self.active_query.as_deref() else { return };
        if path.contains('.') {
            return;
        }
        if !self.ty.params.iter().any(|s| s.id == path && s.is_visible(&self.feature.params)) {
            self.active_query = self.first_visible_query();
        }
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
        self.user_set.insert(path.to_string());
        self.set_value_auto(path, v, cx);
    }

    /// Set a value on the user's behalf (automatic defaults): the field is
    /// not marked as user-set.
    pub fn set_value_auto(&mut self, path: &str, v: ParamValue, cx: &mut Context<Self>) {
        if let Some((vals, id)) = self.values_for_path_mut(path) {
            vals.set(&id, v);
        }
        self.refocus_hidden_query();
        // Hole: switching units or style shows another Standard; copy its values.
        if self.feature.type_id == "hole" && matches!(path, "unitsSystem" | "styleV2") && noshape_model::features::hole_tables::sync_hole_standard(&mut self.feature.params) {
            self.inputs.clear();
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

    /// Focus a field with its text selected (after a manipulator drag, as
    /// Onshape does).
    pub fn focus_field_selected(&mut self, path: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(st) = self.inputs.get(path).cloned() {
            st.update(cx, |s, cx| {
                s.focus(window, cx);
                s.select_all(window, cx);
            });
            cx.notify();
        }
    }

    /// Add a picked reference to the active selection field. Returns false if
    /// no field is active.
    pub fn add_pick(&mut self, r: Reference, cx: &mut Context<Self>) -> bool {
        let Some((path, _filter, max)) = self.active_filter() else { return false };
        self.user_set.insert(path.clone());
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

    /// Set or clear the tolerance of a top-level quantity (`key` =
    /// `<param>__tolerance`).
    fn set_tolerance(&mut self, key: &str, t: Option<Tolerance>, cx: &mut Context<Self>) {
        match t {
            Some(t) => self.feature.params.set(key, ParamValue::Text(t.encode())),
            None => {
                self.feature.params.0.shift_remove(key);
            }
        }
        self.inputs.retain(|k, _| !k.starts_with(key));
        self.user_set.insert(key.to_string());
        cx.emit(DialogEvent::Changed);
        cx.notify();
    }

    /// Tolerance row: type, upper/lower values, precision.
    fn render_tolerance(&mut self, key: &str, t: Tolerance, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let view = cx.entity();
        let k1 = key.to_string();
        let current = t.clone();
        let type_menu = Button::new(SharedString::from(format!("toltype-{key}"))).label(t.kind.clone()).outline().xsmall().dropdown_caret(true).dropdown_menu(move |mut m, _, _| {
            for kind in Tolerance::KINDS {
                let (v, k, mut nt) = (view.clone(), k1.clone(), current.clone());
                nt.kind = kind.to_string();
                m = m.item(gpui_kit::component::menu::PopupMenuItem::new(*kind).checked(current.kind == *kind).on_click(move |_, _, cx| v.update(cx, |this, cx| this.set_tolerance(&k, Some(nt.clone()), cx))));
            }
            m
        });
        let view = cx.entity();
        let k2 = key.to_string();
        let current = t.clone();
        let prec_label = t.precision.map(|p| format!("{p} places")).unwrap_or_else(|| "Workspace precision".into());
        let prec_menu = Button::new(SharedString::from(format!("tolprec-{key}"))).label(prec_label).ghost().xsmall().dropdown_caret(true).dropdown_menu(move |mut m, _, _| {
            for p in std::iter::once(None).chain((0..=6u8).map(Some)) {
                let (v, k, mut nt) = (view.clone(), k2.clone(), current.clone());
                nt.precision = p;
                let label = p.map(|p| format!("{p} places")).unwrap_or_else(|| "Workspace precision".into());
                m = m.item(gpui_kit::component::menu::PopupMenuItem::new(label).checked(current.precision == p).on_click(move |_, _, cx| v.update(cx, |this, cx| this.set_tolerance(&k, Some(nt.clone()), cx))));
            }
            m
        });
        let mut fields: Vec<AnyElement> = Vec::new();
        for (slot, label) in t.value_labels() {
            let text = if slot == 0 { t.upper.clone() } else { t.lower.clone() };
            let ik = format!("{key}.{slot}");
            let st = match self.inputs.get(&ik) {
                Some(st) => st.clone(),
                None => {
                    let st = cx.new(|cx| InputState::new(window, cx).default_value(text));
                    let (k, tt) = (key.to_string(), t.clone());
                    let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, _, cx| {
                        if matches!(ev, InputEvent::Change) {
                            let mut nt = Tolerance::parse(this.get_value(&k)).unwrap_or_else(|| tt.clone());
                            let v = st.read(cx).value().to_string();
                            if slot == 0 {
                                nt.upper = v;
                            } else {
                                nt.lower = v;
                            }
                            this.feature.params.set(&k, ParamValue::Text(nt.encode()));
                            cx.emit(DialogEvent::Changed);
                        }
                    });
                    self.subs.push(sub);
                    self.inputs.insert(ik.clone(), st.clone());
                    st
                }
            };
            fields.push(h_flex().gap_1().child(div().text_xs().text_color(pal.text_muted).child(label)).child(div().w(px(70.)).child(Input::new(&st).xsmall())).into_any_element());
        }
        v_flex().pl(px(10.)).gap_1().child(h_flex().gap_1().child(type_menu).child(prec_menu)).children(fields).into_any_element()
    }

    /// Move pick `from` to position `to` within a selection field.
    pub fn move_pick(&mut self, path: &str, from: usize, to: usize, cx: &mut Context<Self>) {
        let Some((vals, id)) = self.values_for_path_mut(path) else { return };
        let q = vals.query_mut(&id);
        if from >= q.len() || to >= q.len() || from == to {
            return;
        }
        let r = q.remove(from);
        q.insert(to, r);
        self.user_set.insert(path.to_string());
        cx.emit(DialogEvent::Changed);
        cx.notify();
    }

    /// Flash the active selection field red (a rejected pick).
    pub fn flash_active_field(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.active_query.clone() else { return };
        self.flash = Some(path);
        cx.notify();
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(std::time::Duration::from_millis(450)).await;
            let _ = this.update(cx, |this, cx| {
                this.flash = None;
                cx.notify();
            });
        })
        .detach();
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

    /// Replace the whole draft (dialog-local undo/redo) and rebuild the
    /// value fields from it.
    pub fn restore_draft(&mut self, feature: Feature, cx: &mut Context<Self>) {
        self.feature = feature;
        self.inputs.clear();
        self.parse_errors.clear();
        self.refocus_hidden_query();
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
        let sub = cx.subscribe_in(&state, window, move |this: &mut FeatureDialog, state, ev: &InputEvent, window, cx| match ev {
            InputEvent::Change => {
                let text = state.read(cx).value().to_string();
                this.on_text_changed(&path_s, text, cx);
                let numeric = this.spec_for_path(&path_s).is_some_and(|s| matches!(s.kind, ParamKind::Num(_)));
                this.completion = numeric.then(|| crate::var_complete::VarCompletion::compute(&path_s, state.read(cx), &this.variables)).flatten();
            }
            InputEvent::PressEnter { shift, .. } => {
                // Onshape: Enter in a changed field applies the value (the
                // text stays selected); the next Enter commits the feature.
                // The Accept action of the same keystroke may come before
                // or after this event; `enter_applied` drops the second.
                if *shift {
                    cx.emit(DialogEvent::Accept { repeat: true });
                } else if this.apply_on_enter(&path_s, state, window, cx) {
                    this.enter_applied = true;
                } else if !std::mem::take(&mut this.enter_applied) {
                    cx.emit(DialogEvent::Accept { repeat: false });
                }
            }
            // Onshape keeps the selection field active while a number field
            // has keyboard focus ("two fields are active"), so picks still
            // go to it.
            InputEvent::Focus => cx.notify(),
            InputEvent::Blur => {
                // Re-render: the field shows its converted value again.
                this.completion = None;
                cx.notify();
            }
        });
        self.subs.push(sub);
        self.inputs.insert(path.to_string(), state.clone());
        state
    }

    /// Enter in a field whose text changed since it was focused (or last
    /// applied) applies it: the text is selected and the dialog stays open.
    fn apply_on_enter(&mut self, path: &str, state: &Entity<InputState>, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let text = state.read(cx).value().to_string();
        if self.baselines.get(path).is_none_or(|b| *b == text) {
            return false;
        }
        self.baselines.insert(path.to_string(), text);
        state.update(cx, |s, cx| s.select_all(window, cx));
        cx.notify();
        true
    }

    /// Insert autofill entry `index` (the selected one when `None`) into the
    /// field and close the list.
    fn accept_completion(&mut self, index: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = self.completion.take() else { return };
        let Some(state) = self.inputs.get(&c.key).cloned() else { return };
        if let Some(text) = c.accept(index, &state, window, cx) {
            self.on_text_changed(&c.key, text, cx);
        }
        cx.notify();
    }

    /// Key handling while a field's autofill list is open: ↑/↓ move, Enter
    /// inserts, Esc closes. Capture listeners run before the input's own
    /// handling and stop the key there; with no list open for this field
    /// the key passes through untouched.
    fn completion_keys(&self, el: Stateful<Div>, path: &str, cx: &mut Context<Self>) -> Stateful<Div> {
        use gpui_kit::component::input::{Enter, Escape, MoveDown, MoveUp};
        let open = |this: &Self, p: &str| this.completion.as_ref().is_some_and(|c| c.key == p);
        let (p1, p2, p3, p4) = (path.to_string(), path.to_string(), path.to_string(), path.to_string());
        el.capture_action(cx.listener(move |this, _: &MoveUp, _, cx| {
            if open(this, &p1) {
                cx.stop_propagation();
                if let Some(c) = &mut this.completion {
                    c.move_selection(-1);
                }
                cx.notify();
            }
        }))
        .capture_action(cx.listener(move |this, _: &MoveDown, _, cx| {
            if open(this, &p2) {
                cx.stop_propagation();
                if let Some(c) = &mut this.completion {
                    c.move_selection(1);
                }
                cx.notify();
            }
        }))
        .capture_action(cx.listener(move |this, _: &Enter, window, cx| {
            if open(this, &p3) {
                cx.stop_propagation();
                this.accept_completion(None, window, cx);
            }
        }))
        .capture_action(cx.listener(move |this, _: &Escape, _, cx| {
            if open(this, &p4) {
                cx.stop_propagation();
                this.completion = None;
                cx.notify();
            }
        }))
    }

    fn on_text_changed(&mut self, path: &str, text: String, cx: &mut Context<Self>) {
        let Some(spec) = self.spec_for_path(path) else { return };
        self.user_set.insert(path.to_string());
        let v = match spec.kind {
            ParamKind::Num(_) | ParamKind::Bool { .. } => {
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
            // Collapsed group: skip its params (those shown only while the
            // header is checked). The first param that doesn't depend on the
            // header ends the group (Extrude's Symmetric and Draft follow the
            // Starting offset group).
            if s.hints.has(Hints::GROUP_HEADER) {
                hidden_by_group = None;
            } else if let Some(header) = hidden_by_group {
                let mut off = values.clone();
                off.set(header, ParamValue::Bool(false));
                if !s.is_visible(&off) {
                    continue;
                }
                hidden_by_group = None;
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
        // Onshape dialog field label: 13.33 px #666 (§3.9).
        let label_el = |text: &'static str| div().text_size(px(13.33)).text_color(pal.text_muted).truncate().child(text);
        let flips: Vec<AnyElement> = trailing
            .iter()
            .map(|f| {
                let fpath = match prefix {
                    None => f.id.to_string(),
                    Some((arr, idx)) => item_path(arr, idx, f.id),
                };
                let on = values.bool(f.id);
                let circular = f.hints.has(Hints::OPPOSITE_DIRECTION_CIRCULAR);
                let icon = if circular { "flip-circular" } else { "flip-direction" };
                Button::new(SharedString::from(format!("flip-{fpath}")))
                    .custom(dialog_icon_variant(cx, on))
                    .child(crate::assets::icon(icon, 20., cx))
                    .w(px(26.))
                    .h(px(26.))
                    .p_0()
                    .rounded(px(0.))
                    .flex_none()
                    .when(on, |b| b.bg(pal.panel_active))
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
            // A checkbox converted to an expression (Onshape "Convert to
            // expression"): a text field; right-click > Convert to checkbox.
            ParamKind::Bool { .. } if matches!(values.get(s.id), Some(ParamValue::Expr(_))) => {
                let e = self.render_inline_value(s, path, values, window, cx)?;
                let view = cx.entity();
                let p2 = path.to_string();
                h_flex()
                    .id(SharedString::from(format!("bexpr-{path}")))
                    .gap_2()
                    .w_full()
                    .child(div().w(px(96.)).flex_none().text_xs().text_color(pal.text_muted).child(s.label))
                    .child(div().flex_1().child(e))
                    .context_menu(move |m, _, _| {
                        let v = view.clone();
                        let p = p2.clone();
                        m.item(gpui_kit::component::menu::PopupMenuItem::new("Convert to checkbox").on_click(move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                this.inputs.remove(&p);
                                this.parse_errors.remove(&p);
                                this.set_value(&p, ParamValue::Bool(false), cx);
                            })
                        }))
                    })
                    .into_any_element()
            }
            ParamKind::Bool { .. } => {
                let on = values.bool(s.id);
                let path_s = path.to_string();
                let header = s.hints.has(Hints::GROUP_HEADER);
                // Onshape: group headers are "› ☐ Direction" rows; the chevron
                // folds the group's parameters (indented 20 px below).
                let collapsed = self.collapsed_groups.contains(path);
                let path_c = path.to_string();
                let chevron = header.then(|| {
                    div()
                        .id(SharedString::from(format!("chev-{path}")))
                        .w(px(20.))
                        .flex_none()
                        .flex()
                        .justify_center()
                        .cursor_pointer()
                        .child(crate::ui::glyph(if on && !collapsed { "ui-chevron-down" } else { "ui-chevron-right" }, 11., if on { gpui_kit::rgb(0xa0522d) } else { pal.border }))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if !this.collapsed_groups.remove(&path_c) {
                                this.collapsed_groups.insert(path_c.clone());
                            }
                            cx.notify();
                        }))
                });
                // Onshape: a 20 px expander column before group checkboxes; the
                // checkbox itself is inset 4 px.
                let mut row = h_flex()
                    .w_full()
                    .min_h(px(27.))
                    .children(chevron)
                    .child({
                        let view = cx.entity();
                        let p2 = path.to_string();
                        div()
                            .id(SharedString::from(format!("cbw-{path}")))
                            .child(crate::ui::os_checkbox(SharedString::from(format!("cb-{path}")), s.label, on).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_value(&path_s, ParamValue::Bool(!on), cx))))
                            // Right-click > Convert to expression (e.g. `#Length > 3`).
                            .context_menu(move |m, _, _| {
                                let v = view.clone();
                                let p = p2.clone();
                                m.item(gpui_kit::component::menu::PopupMenuItem::new("Convert to expression").on_click(move |_, window, cx| {
                                    v.update(cx, |this, cx| {
                                        let text = if matches!(this.get_value(&p), Some(ParamValue::Bool(true))) { "true" } else { "false" };
                                        this.inputs.remove(&p);
                                        this.set_value(&p, ParamValue::Expr(noshape_units::ExprValue::new(text)), cx);
                                        // The new expression field takes the keyboard.
                                        let input = this.ensure_input(&p, text, window, cx);
                                        input.update(cx, |s, cx| {
                                            s.focus(window, cx);
                                            s.select_all(window, cx);
                                        });
                                    })
                                }))
                            })
                    });
                if let Some(c) = companion {
                    let cpath = match prefix {
                        None => c.id.to_string(),
                        Some((arr, idx)) => item_path(arr, idx, c.id),
                    };
                    if let Some(e) = self.render_inline_value(c, &cpath, values, window, cx) {
                        row = row.child(div().flex_1().child(e));
                    }
                }
                row.children(flips).into_any_element()
            }
            ParamKind::Enum { options, .. } => {
                let cur = values.enum_(s.id).to_string();
                if s.hints.has(Hints::HORIZONTAL_ENUM) {
                    // Onshape underline tabs (Solid / Surface / Thin, New / Add / ...).
                    let group = h_flex().w_full().h(px(26.)).children(options.iter().map(|(id, label)| {
                        let selected = cur == *id;
                        let path_s = path.to_string();
                        let id_s = id.to_string();
                        // Onshape (live DOM): segments share the row by content
                        // width plus an equal share of the slack; 12 px text,
                        // the selected one 500 blue with a 2 px blue underline,
                        // the others a 1 px #d7d7d7 underline.
                        div()
                            .id(SharedString::from(format!("seg-{path}-{id}")))
                            .flex_grow(1.)
                            .px(px(8.))
                            .h_full()
                            .relative()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(12.))
                            .cursor_pointer()
                            .when(selected, |d| d.text_color(pal.accent).font_weight(FontWeight::MEDIUM).child(div().absolute().left_0().right_0().bottom_0().h(px(2.)).bg(pal.accent)))
                            .when(!selected, |d| d.text_color(pal.text).child(div().absolute().left_0().right_0().bottom_0().h(px(1.)).bg(pal.border)).hover(|d| d.text_color(pal.text)))
                            .child(*label)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_value(&path_s, ParamValue::Enum(id_s.clone()), cx)))
                    }));
                    h_flex().w_full().mb(px(2.)).child(group).children(flips).into_any_element()
                } else {
                    let label = options.iter().find(|(id, _)| *id == cur).map(|(_, l)| *l).unwrap_or("");
                    let path_s = path.to_string();
                    let opts = *options;
                    let view = cx.entity();
                    // Live: Tab skips the dropdowns (Shift+Tab from Depth lands on the flip button).
                    let dd = Button::new(SharedString::from(format!("dd-{path}")))
                        .tab_stop(false)
                        .custom(dialog_field_variant(cx))
                        .child(h_flex().w_full().child(div().flex_1().text_size(px(14.)).text_color(pal.text).truncate().child(label)).child(crate::ui::glyph("ui-caret-down", 10., pal.text)))
                        .w_full()
                        .h(px(28.))
                        .pl(px(6.))
                        .pr(px(4.))
                        .rounded(px(0.))
                        .border_b_1()
                        .border_color(pal.border)
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
                    let mut row = h_flex().gap(px(4.)).w_full().min_h(px(28.));
                    if s.hints.has(Hints::SHOW_LABEL) {
                        row = row.child(div().pl(px(5.)).max_w(px(90.)).flex_none().child(label_el(s.label)));
                    }
                    row.child(div().flex_1().min_w_0().child(dd)).children(flips).into_any_element()
                }
            }
            ParamKind::Text { .. } if s.id == "partStudio" && self.ty.id == "importDerived" => self.render_derived_picker(cx),
            ParamKind::Num(_) | ParamKind::Text { .. } => {
                let e = self.render_inline_value(s, path, values, window, cx)?;
                // Tolerance options on tolerant quantities (Onshape ± icon).
                let tolerant = matches!(s.kind, ParamKind::Num(_)) && s.hints.has(Hints::CAN_BE_TOLERANT) && !path.contains('.');
                let tol_key = format!("{path}__tolerance");
                let tol = tolerant.then(|| Tolerance::parse(self.get_value(&tol_key))).flatten();
                let tol_btn = tolerant.then(|| {
                    let key = tol_key.clone();
                    let on = tol.is_some();
                    // Onshape: a 20 px tolerance toggle 5 px after the field.
                    div()
                        .id(SharedString::from(format!("tolbtn-{path}")))
                        .mt(px(3.))
                        .ml(px(5.))
                        .size(px(20.))
                        .flex_none()
                        .rounded(px(3.))
                        .cursor_pointer()
                        .when(on, |d| d.bg(pal.panel_active))
                        .hover(|d| d.bg(pal.panel_hover))
                        .child(crate::assets::icon("tolerance", 20., cx))
                        .tooltip(crate::ui::tooltip("Tolerance"))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            let next = if on { None } else { Some(Tolerance::default()) };
                            this.set_tolerance(&key, next, cx);
                        }))
                });
                // Onshape (live DOM): tolerant quantities start with a 20 px
                // expander (gray `›`, red-brown `⌄` with tolerances shown); the
                // label is 12 px `#666` with 5 px padding, the field fills the
                // rest of the first column.
                let expander = tolerant.then(|| {
                    let open = tol.is_some();
                    div().w(px(20.)).h(px(26.)).flex_none().flex().items_center().justify_center().child(crate::ui::glyph(if open { "ui-chevron-down" } else { "ui-chevron-right" }, 11., if open { gpui_kit::rgb(0xa0522d) } else { pal.border }))
                });
                let row = h_flex()
                    .w_full()
                    .min_h(px(26.))
                    .items_start()
                    .children(expander)
                    .child(div().px(px(5.)).pt(px(6.)).max_w(px(110.)).flex_none().text_size(px(12.)).text_color(pal.text_muted).truncate().child(s.label))
                    .child(div().flex_1().min_w_0().child(e))
                    .children(tol_btn)
                    // The first column is 194 px; flips sit in the 26 px right one.
                    .when(flips.is_empty(), |d| d.pr(px(26.)))
                    .children(flips);
                match tol {
                    Some(t) => v_flex().w_full().child(row).child(self.render_tolerance(&tol_key, t, window, cx)).into_any_element(),
                    None => row.into_any_element(),
                }
            }
            ParamKind::Query { max_picks, .. } => {
                let refs = values.query(s.id).to_vec();
                let active = self.active_query.as_deref() == Some(path);
                let flashing = self.flash.as_deref() == Some(path);
                let path_s = path.to_string();
                let any_missing = refs.iter().any(|r| (self.missing)(r));
                // Every region of a sketch shows as one "Faces of Sketch 1"
                // row (Onshape, after clicking the sketch in the list).
                let mut rows: Vec<(String, Vec<usize>, Reference)> = Vec::new();
                for (k, r) in refs.iter().enumerate() {
                    if let Reference::SketchRegion { sketch, .. } = r {
                        let all: Vec<usize> = refs.iter().enumerate().filter(|(_, x)| matches!(x, Reference::SketchRegion { sketch: s2, .. } if s2 == sketch)).map(|(i, _)| i).collect();
                        let count = (self.sketch_region_count)(*sketch);
                        if count >= 2 && all.len() == count {
                            if all[0] == k {
                                rows.push((format!("Faces of {}", (self.labeler)(&Reference::Sketch(*sketch))), all, Reference::Sketch(*sketch)));
                            }
                            continue;
                        }
                    }
                    rows.push(((self.labeler)(r), vec![k], r.clone()));
                }
                let chips: Vec<AnyElement> = rows
                    .into_iter()
                    .enumerate()
                    .map(|(row, (label, indices, r))| {
                        let k = indices[0];
                        let single = indices.len() == 1;
                        let label_for_drag = label.clone();
                        // Missing references (deleted/suppressed creators) show red.
                        let missing = indices.iter().any(|i| (self.missing)(&refs[*i]));
                        let p2 = path_s.clone();
                        let hover_ref = r.clone();
                        let tip = label.clone();
                        // Onshape selection rows: 24 px plain text, × on hover, white
                        // 35% overlay on hover, the entity name as tooltip.
                        h_flex()
                            .id(SharedString::from(format!("chip-{path}-{row}")))
                            .group("qrow")
                            .on_hover(cx.listener(move |_, h: &bool, _, cx| cx.emit(DialogEvent::HoverRef(h.then(|| hover_ref.clone())))))
                            .tooltip(crate::ui::tooltip(tip))
                            .w_full()
                            .h(px(24.))
                            .pl(px(6.))
                            .pr(px(4.))
                            .hover(|d| d.bg(with_alpha(gpui_kit::rgb(0xffffff), 0.35)))
                            .text_size(px(13.33))
                            .text_color(if missing { pal.error } else if active { gpui_kit::rgb(0x555555) } else { pal.text_muted })
                            .child(div().flex_1().truncate().child(label))
                            .child(
                                div()
                                    .id(SharedString::from(format!("chipx-{path}-{row}")))
                                    .invisible()
                                    .group_hover("qrow", |d| d.visible())
                                    .text_color(pal.text_muted)
                                    .hover(|d| d.text_color(pal.error))
                                    .cursor_pointer()
                                    .child(Icon::new(IconName::Close).with_size(px(12.)))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        cx.stop_propagation();
                                        cx.emit(DialogEvent::HoverRef(None));
                                        this.user_set.insert(p2.clone());
                                        if let Some((vals, id)) = this.values_for_path_mut(&p2) {
                                            let q = vals.query_mut(&id);
                                            for i in indices.iter().rev() {
                                                if *i < q.len() {
                                                    q.remove(*i);
                                                }
                                            }
                                        }
                                        cx.emit(DialogEvent::Changed);
                                        cx.notify();
                                    })),
                            )
                            // Drag a pick onto another to reorder (loft profiles, sweep paths…).
                            .when(single, |d| {
                                d.on_drag(PickDrag { path: path_s.clone(), index: k, label: SharedString::from(label_for_drag.clone()) }, |d, _, _, cx| cx.new(|_| d.clone()))
                                    .drag_over::<PickDrag>(|st, _, _, _| st.border_t_2().border_color(p().accent))
                                    .on_drop(cx.listener({
                                        let pd = path_s.clone();
                                        move |this, d: &PickDrag, _, cx| {
                                            if d.path == pd {
                                                this.move_pick(&pd, d.index, k, cx);
                                            }
                                        }
                                    }))
                            })
                            .context_menu({
                                // Onshape: right-click a pick > Clear all (and
                                // Clear missing references when some are).
                                let view = cx.entity();
                                let p3 = path_s.clone();
                                move |menu, _, _| {
                                    let v2 = view.clone();
                                    let pb = p3.clone();
                                    let mut m = menu.item(gpui_kit::component::menu::PopupMenuItem::new("Clear all").on_click(move |_, _, cx| {
                                        v2.update(cx, |this, cx| {
                                            this.set_value(&pb, ParamValue::Query(Vec::new()), cx);
                                            cx.emit(DialogEvent::HoverRef(None));
                                        })
                                    }));
                                    if any_missing {
                                        let v = view.clone();
                                        let pc = p3.clone();
                                        m = m.item(gpui_kit::component::menu::PopupMenuItem::new("Clear missing references").on_click(move |_, _, cx| {
                                            v.update(cx, |this, cx| {
                                                let missing = this.missing.clone();
                                                if let Some((vals, id)) = this.values_for_path_mut(&pc) {
                                                    vals.query_mut(&id).retain(|r| !missing(r));
                                                }
                                                this.user_set.insert(pc.clone());
                                                cx.emit(DialogEvent::Changed);
                                                cx.notify();
                                            })
                                        }));
                                    }
                                    m
                                }
                            })
                            .into_any_element()
                    })
                    .collect();
                // Onshape: the field label is the placeholder inside the box; once
                // entities are picked it shrinks to the top-left corner (§3.9).
                let _ = max_picks;
                let empty = refs.is_empty();
                let caption = div()
                    .px(px(6.))
                    .when(empty, |d| d.py(px(5.)).text_size(px(12.)))
                    .when(!empty, |d| d.pt(px(2.)).text_size(px(10.)))
                    .text_color(pal.text_muted)
                    .truncate()
                    .child(s.label);
                h_flex()
                    .w_full()
                    .items_start()
                    .gap(px(2.))
                    .px(px(2.))
                    .py(px(2.))
                    .child(
                        div()
                            .id(SharedString::from(format!("q-{path}")))
                            .flex_1()
                            .min_w_0()
                            .min_h(px(26.))
                            .max_h(px(160.))
                            .overflow_y_scroll()
                            .rounded(px(2.))
                            .border_1()
                            .border_color(if flashing { pal.error } else if active { pal.accent } else { pal.border })
                            .bg(if active { pal.row_hover } else { pal.panel })
                            .cursor_pointer()
                            .flex()
                            .flex_col()
                            .child(caption)
                            .children(chips)
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                this.active_query = Some(path_s.clone());
                                window.focus(&this.focus, cx);
                                cx.emit(DialogEvent::QueryFocused);
                                cx.notify();
                            })),
                        // Right-click > Clear all lives on the pick rows (a
                        // field-level menu would open a second menu).
                    )
                    .children(flips)
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
                            .ml(px(4.))
                            .pl(px(6.))
                            .border_l_1()
                            .border_color(gpui_kit::rgb(0xcccccc))
                            .child(
                                h_flex()
                                    .justify_between()
                                    .child(div().text_size(px(13.33)).text_color(pal.text_muted).child(format!("{item_label} {}", k + 1)))
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
                        h_flex().gap_2().min_h(px(26.)).child(div().pl(px(5.)).w(px(80.)).text_size(px(13.33)).text_color(pal.text_muted).truncate().child(name)).child(
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
        let external = crate::derived::ExternalRef::parse(&current);
        if let Some(r) = &external {
            self.resolve_external(r.clone(), cx);
        }
        let ext_info = self.ext_info.clone().filter(|i| i.key == current);
        let label = match (&external, &ext_info) {
            (Some(_), Some(i)) => i.label.clone(),
            (Some(_), None) => "Loading other document…".into(),
            _ => sources.iter().find(|s| s.tab == current).map(|s| s.name.clone()).unwrap_or_else(|| "Select a Part Studio".into()),
        };
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
                    let v = view.clone();
                    m.separator().item(gpui_kit::component::menu::PopupMenuItem::new("Other document…").on_click(move |_, window, cx| {
                        let v = v.clone();
                        crate::files::request_open_path("Derive from document", "noshape documents", &["noshape"], window, cx, move |path, cx| {
                            v.update(cx, |this, cx| {
                                match crate::derived::external_doc_info(&path) {
                                    Ok(info) => this.ext_pick = Some((info, 0)),
                                    Err(e) => this.status = Some((FeatureStatus::Error, format!("Could not open {}: {e:#}", path.display()))),
                                }
                                cx.notify();
                            });
                        });
                    }))
                }
            });
        let parts = match &ext_info {
            Some(i) => i.parts.clone(),
            None => sources.iter().find(|s| s.tab == current).map(|s| s.parts.clone()).unwrap_or_default(),
        };
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
                        let src = this.feature.params.text("partStudio").to_string();
                        let all: Vec<noshape_model::BodyId> = match this.ext_info.as_ref().filter(|i| i.key == src) {
                            Some(i) => i.parts.iter().map(|p| p.0.clone()).collect(),
                            None => this.derived_sources.iter().find(|s| s.tab == src).map(|s| s.parts.iter().map(|p| p.0.clone()).collect()).unwrap_or_default(),
                        };
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
        let update = ext_info.as_ref().and_then(|i| i.newer.clone()).zip(external.clone()).map(|(newer, r)| {
            h_flex()
                .gap_2()
                .p_1p5()
                .rounded_md()
                .bg(with_alpha(pal.info, 0.12))
                .text_xs()
                .text_color(pal.info)
                .child(div().flex_1().child(format!("Newer version available: {}", newer.label)))
                .child(Button::new("derived-update").label("Update").xsmall().outline().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    let to = crate::derived::ExternalRef { microversion: newer.microversion, ..r.clone() };
                    this.set_value("partStudio", ParamValue::Text(to.key()), cx);
                })))
                .into_any_element()
        });
        let chooser = self.render_ext_chooser(cx);
        v_flex()
            .gap_1p5()
            .w_full()
            .child(div().text_xs().text_color(pal.text_muted).child("Part Studio"))
            .child(picker)
            .children(chooser)
            .children(update)
            .children(checks)
            .into_any_element()
    }

    /// Load the label, parts and update status of an external source in the background.
    fn resolve_external(&mut self, r: crate::derived::ExternalRef, cx: &mut Context<Self>) {
        let key = r.key();
        if self.ext_info.as_ref().is_some_and(|i| i.key == key) || self.ext_task.is_some() {
            return;
        }
        self.ext_task = Some(cx.spawn(async move |this, cx| {
            let r2 = r.clone();
            let info = cx
                .background_spawn(async move {
                    let label = crate::derived::describe_external(&r2);
                    let parts = crate::derived::external_context(&r2).map(|c| c.bodies.iter().map(|b| (b.id.clone(), b.name.clone())).collect()).unwrap_or_default();
                    let newer = crate::derived::newer_version(&r2);
                    ExtInfo { key: r2.key(), label, parts, newer }
                })
                .await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                this.ext_info = Some(info);
                this.ext_task = None;
                cx.notify();
            });
        }));
    }

    /// Version and Part Studio chooser shown after picking another document.
    fn render_ext_chooser(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (info, vi) = self.ext_pick.clone()?;
        let pal = p();
        let version = info.versions.get(vi)?.clone();
        let studios: Vec<(String, String)> = crate::derived::external_snapshot(&info.path, version.microversion)
            .map(|d| crate::derived::DocStudios::sources(&d))
            .unwrap_or_default();
        let view = cx.entity();
        let versions = info.versions.clone();
        let version_menu = Button::new("ext-version").label(version.label.clone()).outline().xsmall().dropdown_caret(true).dropdown_menu(move |mut m, _, _| {
            for (i, v) in versions.iter().enumerate() {
                let view = view.clone();
                m = m.item(gpui_kit::component::menu::PopupMenuItem::new(v.label.clone()).checked(i == vi).on_click(move |_, _, cx| {
                    view.update(cx, |this, cx| {
                        if let Some(p) = &mut this.ext_pick {
                            p.1 = i;
                        }
                        cx.notify();
                    })
                }));
            }
            m
        });
        let rows: Vec<AnyElement> = studios
            .into_iter()
            .map(|(tab, name)| {
                let r = crate::derived::ExternalRef { path: info.path.clone(), microversion: version.microversion, tab: tab.clone() };
                div()
                    .id(SharedString::from(format!("ext-{tab}")))
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .text_xs()
                    .hover(|d| d.bg(pal.panel_hover))
                    .child(name)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.ext_pick = None;
                        this.feature.params.set("parts", ParamValue::Query(Vec::new()));
                        this.set_value("partStudio", ParamValue::Text(r.key()), cx);
                    }))
                    .into_any_element()
            })
            .collect();
        let empty = rows.is_empty();
        Some(
            v_flex()
                .gap_1()
                .p_1p5()
                .rounded_md()
                .border_1()
                .border_color(pal.border_strong)
                .child(h_flex().gap_2().child(div().flex_1().text_xs().font_weight(FontWeight::SEMIBOLD).truncate().child(info.name.clone())).child(version_menu))
                .when(empty, |d| d.child(div().text_xs().text_color(pal.text_faint).child("No Part Studios in this version")))
                .children(rows)
                .into_any_element(),
        )
    }

    /// Input element for a numeric or text value.
    fn render_inline_value(&mut self, s: &ParamSpec, path: &str, values: &ParamValues, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let text = match (&s.kind, values.get(s.id)) {
            (ParamKind::Num(_) | ParamKind::Bool { .. }, Some(ParamValue::Expr(e))) => e.text.clone(),
            (ParamKind::Text { .. }, Some(ParamValue::Text(t))) => t.clone(),
            (ParamKind::Num(_), _) | (ParamKind::Text { .. }, _) => String::new(),
            _ => return None,
        };
        let state = self.ensure_input(path, &text, window, cx);
        let err = self.parse_errors.get(path).cloned();
        let pal = p();
        let num = match &s.kind {
            ParamKind::Num(n) => Some(*n),
            _ => None,
        };
        // Onshape shows what an expression evaluates to.
        let hint = num.filter(|_| err.is_none()).and_then(|n| noshape_model::interaction::evaluated_hint(&text, n.kind.dim(), &self.units, &self.variables));
        // Onshape: an accepted value displays converted to the document
        // units (and an expression its result); focusing shows the text.
        let focused = state.read(cx).focus_handle(cx).is_focused(window);
        // The text before editing started: Esc reverts to it (see `baselines`).
        if !focused {
            self.baselines.insert(path.to_string(), text.clone());
        }
        let converted = num
            .filter(|_| !focused && err.is_none() && self.completion.is_none())
            .and_then(|n| noshape_units::evaluate_as(&text, n.kind.dim(), &self.units, &self.variables).ok())
            .map(|q| noshape_units::format_quantity(q, &self.units))
            .filter(|c| c.replace(' ', "") != text.trim().replace(' ', ""));
        let hint = hint.filter(|_| converted.is_none());
        let path_s = path.to_string();
        let st2 = state.clone();
        let st3 = state.clone();
        let field = v_flex().id(SharedString::from(format!("num-{path}")));
        let field = self.completion_keys(field, path, cx);
        // Enter in a changed field applies it; the next Enter commits.
        let (p_ent, st_ent) = (path.to_string(), state.clone());
        let field = field.capture_action(cx.listener(move |this, _: &crate::actions::Accept, window, cx| {
            if this.apply_on_enter(&p_ent, &st_ent, window, cx) {
                this.enter_applied = true;
                cx.stop_propagation();
            } else if std::mem::take(&mut this.enter_applied) {
                cx.stop_propagation();
            }
        }));
        // Esc in a changed field reverts it; the next Esc cancels the dialog.
        let (p_esc, st_esc) = (path.to_string(), state.clone());
        let field = field.capture_action(cx.listener(move |this, _: &crate::actions::Cancel, window, cx| {
            let text = st_esc.read(cx).value().to_string();
            let Some(base) = this.baselines.get(&p_esc).filter(|b| **b != text).cloned() else { return };
            cx.stop_propagation();
            st_esc.update(cx, |s, cx| s.set_value(base.clone(), window, cx));
            this.on_text_changed(&p_esc, base, cx);
            cx.notify();
        }));
        Some(
            field
                .gap_0p5()
                // Onshape underline field: no box, 1 px #d7d7d7 underline (blue on hover/focus),
                // invalid values get a pink fill and red underline (§3.9, §4.2).
                .child(
                    div()
                        .border_b_1()
                        .border_color(if err.is_some() { pal.error } else { pal.border })
                        .when(err.is_some(), |d| d.bg(pal.error_fill))
                        .hover(|d| d.border_color(pal.accent))
                        // The input stays in the tree (so clicks and Tab can
                        // focus it); unfocused, the converted value covers it.
                        .relative()
                        .child(div().when(converted.is_some(), |d| d.opacity(0.)).child(Input::new(&state).small().appearance(false).text_size(px(13.33))))
                        .when_some(converted, |d, c| {
                            d.child(
                                div()
                                    .id(SharedString::from(format!("numv-{path}")))
                                    .absolute()
                                    .inset_0()
                                    .px_1()
                                    .flex()
                                    .items_center()
                                    // Onshape right-aligns numbers.
                                    .justify_end()
                                    .text_size(px(13.33))
                                    .text_color(pal.text)
                                    .cursor_text()
                                    .child(c)
                                    .on_mouse_down(MouseButton::Left, cx.listener(move |_, _: &MouseDownEvent, window, cx| {
                                        st3.update(cx, |s, cx| s.focus(window, cx));
                                        cx.notify();
                                    })),
                            )
                        }),
                )
                .when_some(self.completion.as_ref().filter(|c| c.key == path), |d, c| {
                    let view = cx.entity();
                    d.child(c.render(&self.variables, &self.units, move |i, window, cx| view.update(cx, |this, cx| this.accept_completion(Some(i), window, cx))))
                })
                .when_some(err, |d, e| d.child(div().text_xs().text_color(pal.text_invalid).child(e)))
                .when_some(hint, |d, h| d.child(div().text_xs().text_color(pal.text_faint).child(format!("= {h}"))))
                .when_some(num, |d, n| {
                    // Mouse wheel over a focused numeric field: ±0.1, Ctrl ±0.01, Shift ±1.
                    d.on_scroll_wheel(cx.listener(move |this, e: &ScrollWheelEvent, window, cx| {
                        if !st2.read(cx).focus_handle(cx).is_focused(window) {
                            return;
                        }
                        let dy = match e.delta {
                            ScrollDelta::Lines(l) => f64::from(l.y),
                            ScrollDelta::Pixels(p) => f64::from(f32::from(p.y)),
                        };
                        if dy.abs() < 1e-6 {
                            return;
                        }
                        cx.stop_propagation();
                        let step = if n.kind == noshape_model::params::NumKind::Integer { 1.0 } else { noshape_model::interaction::wheel_step(e.modifiers.control || e.modifiers.platform, e.modifiers.shift) } * dy.signum();
                        let text = st2.read(cx).value().to_string();
                        if let Some(next) = noshape_model::interaction::nudge_value_text(&text, step, n.kind.dim(), &this.units, &this.variables, n.min, n.max) {
                            st2.update(cx, |s, cx| s.set_value(next.clone(), window, cx));
                            this.on_text_changed(&path_s, next, cx);
                        }
                    }))
                })
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
        let mut rows = self.render_params(params, None, window, cx);
        let (extra_rows, footer_extra) = match self.extra.clone() {
            Some(f) => f(window, cx),
            None => (Vec::new(), None),
        };
        rows.extend(extra_rows);
        // The Sketch dialog has no preview slider (live).
        let sketch = self.feature.type_id == "sketch";
        let status = self.status.clone();
        // Onshape: the title turns red while the feature is invalid.
        let invalid = matches!(status.as_ref().map(|s| s.0), Some(FeatureStatus::Error));
        let title_color = if invalid { pal.text_invalid } else { pal.text };
        let error_tip: Option<String> = status.as_ref().filter(|s| s.0 == FeatureStatus::Error).map(|s| format!("{} did not regenerate properly: {}", self.feature.name, s.1));
        let name = self.feature.name.clone();
        let renaming = self.renaming.clone();
        v_flex()
            .id("feature-dialog")
            // Clicks on the dialog never reach the graphics area beneath.
            .occlude()
            .key_context("FeatureDialog")
            .track_focus(&self.focus)
            // Onshape feature dialog frame (§3.9): 220 px, white at 95%, no
            // border or radius, shadow 0 5px 10px rgba(0,0,0,.2).
            .w(px(220.))
            .max_h(px(720.))
            .bg(pal.dialog_bg)
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(10.), spread_radius: px(0.), inset: false }])
            .text_color(pal.text)
            .on_action(cx.listener(|_, _: &crate::actions::Accept, _, cx| cx.emit(DialogEvent::Accept { repeat: false })))
            .on_action(cx.listener(|_, _: &crate::actions::AcceptAndRepeat, _, cx| cx.emit(DialogEvent::Accept { repeat: true })))
            // A: flip the primary axis of a mate connector being edited.
            .on_action(cx.listener(|this, _: &crate::actions::FlipPrimaryAxis, _, cx| {
                if this.ty.params.iter().any(|p| p.id == "flipPrimary") {
                    let cur = this.feature.params.bool("flipPrimary");
                    this.set_value("flipPrimary", ParamValue::Bool(!cur), cx);
                }
            }))
            // Q: turn a mate connector's secondary axis (+X → +Y → -X → -Y).
            .on_action(cx.listener(|this, _: &crate::actions::ReorientSecondaryAxis, _, cx| {
                if this.ty.params.iter().any(|p| p.id == "secondaryAxisType") {
                    let order = ["PLUS_X", "PLUS_Y", "MINUS_X", "MINUS_Y"];
                    let cur = match this.feature.params.get("secondaryAxisType") {
                        Some(ParamValue::Enum(e)) => order.iter().position(|o| o == e).unwrap_or(0),
                        _ => 0,
                    };
                    this.set_value("secondaryAxisType", ParamValue::Enum(order[(cur + 1) % 4].to_string()), cx);
                }
            }))
            .on_action(cx.listener(|_, _: &crate::actions::Cancel, _, cx| cx.emit(DialogEvent::Cancel)))
            .child(
                h_flex()
                    .h(px(28.))
                    .flex_none()
                    .pl(px(6.))
                    .bg(pal.panel)
                    .border_b_1()
                    .border_color(pal.border)
                    .child(match renaming {
                        Some(state) => div().flex_1().child(Input::new(&state).xsmall()).into_any_element(),
                        None => div()
                            .id("dialog-title")
                            // Drag the title to move the dialog (Onshape).
                            .on_drag(DialogMove(self.drag_grab.clone()), |d, offset, _, cx| {
                                *d.0.lock() = offset;
                                cx.new(|_| DialogMoveGhost)
                            })
                            .flex_1()
                            .min_w_0()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(title_color)
                            .truncate()
                            .child(name)
                            .when_some(error_tip, |d, m| d.tooltip(crate::ui::tooltip(m)))
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
                            .custom(
                                gpui_kit::component::button::ButtonCustomVariant::new(cx)
                                    .color(if invalid { gpui_kit::rgb(0xcce9cc).into() } else { pal.accept.into() })
                                    .hover(gpui_kit::rgb(0x74ce3a).into())
                                    .active(pal.accept.into())
                                    .shadow(false),
                            )
                            .child(crate::assets::icon("dialog-ok", 20., cx))
                            .w(px(28.))
                            .h(px(28.))
                            .p_0()
                            .rounded(px(0.))
                            .bg(if invalid { gpui_kit::rgb(0xcce9cc) } else { pal.accept })
                            .tooltip("Accept (Enter)")
                            // Onshape: the ✓ is disabled while the feature errors
                            // (Enter still commits it).
                            .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                                if !invalid {
                                    cx.emit(DialogEvent::Accept { repeat: false })
                                }
                            })),
                    )
                    .child(
                        Button::new("dlg-cancel")
                            .custom(dialog_icon_variant(cx, false))
                            .child(crate::assets::icon("dialog-cancel", 18., cx))
                            .w(px(28.))
                            .h(px(28.))
                            .p_0()
                            .rounded(px(0.))
                            .tooltip("Cancel (Esc)")
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(DialogEvent::Cancel))),
                    ),
            )
            .child(v_flex().id("dialog-body").px(px(2.)).pb(px(4.)).overflow_y_scroll().children(rows))
            .child(
                // Footer (live): no band; a 112 px preview slider at the
                // left and the round ? help button at the right.
                h_flex()
                    .h(px(if sketch { 25. } else { 22. }))
                    .flex_none()
                    .pl(px(7.))
                    .pr(px(4.))
                    .gap_2()
                    .text_size(px(13.33))
                    .text_color(pal.text_muted)
                    .when(self.final_available, |d| d.child(
                        gpui_kit::component::checkbox::Checkbox::new("dlg-final")
                            .checked(self.show_final)
                            .label("Final")
                            .small()
                            .tooltip("Preview the finished model instead of rolling back to this feature")
                            .on_click(cx.listener(|this, v: &bool, _, cx| {
                                this.show_final = *v;
                                cx.emit(DialogEvent::Changed);
                                cx.notify();
                            })),
                    ))
                    // Preview slider: before (left) to after (right) the feature.
                    .when(!sketch, |d| d.child(self.render_preview_slider(cx)))
                    .child(div().flex_1())
                    .children(footer_extra)
                    .child(
                        div()
                            .id("dlg-help")
                            .size(px(14.))
                            .rounded_full()
                            .bg(gpui_kit::rgb(0x888888))
                            .hover(|d| d.bg(pal.text_muted))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(11.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(gpui_kit::rgb(0xffffff))
                            .child("?")
                            .cursor_pointer()
                            .tooltip(crate::ui::tooltip(format!("{} help", self.ty.name)))
                            // Help: the feature's description (offline help).
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.show_help = !this.show_help;
                                cx.notify();
                            })),
                    ),
            )
            .when(self.show_help && !self.ty.description.is_empty(), |d| {
                d.child(div().mx_3().mb_2().p_2().rounded_md().bg(pal.app_bg).text_xs().text_color(pal.text_muted).child(format!("{}: {}.", self.ty.name, self.ty.description)))
            })
            // Onshape shows regeneration errors as the red title (with the
            // message as its tooltip), not as an inline alert; warnings get the alert.
            .when_some(status.filter(|s| !matches!(s.0, FeatureStatus::Ok | FeatureStatus::Error)), |d, (st, msg)| {
                let kind = match st {
                    FeatureStatus::Error => crate::ui::AlertKind::Error,
                    FeatureStatus::Warning => crate::ui::AlertKind::Warning,
                    _ => crate::ui::AlertKind::Info,
                };
                d.child(div().p(px(4.)).child(crate::ui::alert(kind).text_size(px(12.)).child(div().flex_1().min_w_0().whitespace_normal().child(msg))))
            })
    }
}

impl FeatureDialog {
    /// Onshape's preview slider (noUi, live): a 112 px, 2 px `#777` track
    /// and an 11 px white handle with a 1 px `#888` ring and a soft shadow.
    /// Left = the model before the feature, right = after it.
    fn render_preview_slider(&self, cx: &mut Context<Self>) -> AnyElement {
        const W: f32 = 112.;
        const HANDLE: f32 = 11.;
        let track = self.preview_track.clone();
        let set = move |this: &mut Self, x: Pixels, cx: &mut Context<Self>| {
            let b = this.preview_track.get();
            let v = (f32::from(x - b.left()) / f32::from(b.size.width).max(1.)).clamp(0., 1.);
            if (v - this.preview).abs() > f32::EPSILON {
                this.preview = v;
                cx.emit(DialogEvent::PreviewOpacity(v));
                cx.notify();
            }
        };
        div()
            .id("dlg-preview")
            .relative()
            .w(px(W))
            .h(px(HANDLE + 2.))
            .flex_none()
            .cursor_pointer()
            .child(div().absolute().left_0().right_0().top(px((HANDLE + 2.) * 0.5 - 1.)).h(px(2.)).bg(gpui_kit::rgb(0x777777)))
            .child(
                div()
                    .absolute()
                    .top(px(1.))
                    .left(px(self.preview * W - HANDLE * 0.5))
                    .size(px(HANDLE))
                    .rounded_full()
                    .bg(gpui_kit::rgb(0xffffff))
                    .border_1()
                    .border_color(gpui_kit::rgb(0x888888))
                    .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.6), offset: point(px(0.), px(0.)), blur_radius: px(3.), spread_radius: px(0.), inset: false }]),
            )
            .child(canvas(move |b, _, _| track.set(b), |_, _, _, _| {}).absolute().size_full())
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                set(this, e.position.x, cx);
            }))
            .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, _, cx| {
                if e.pressed_button == Some(MouseButton::Left) {
                    set(this, e.position.x, cx);
                }
            }))
            .into_any_element()
    }
}

/// Dialog side icon buttons (flip, cancel): transparent, `#eaeaea` hover (§4.1).
fn dialog_icon_variant(cx: &App, on: bool) -> gpui_kit::component::button::ButtonCustomVariant {
    let pal = p();
    gpui_kit::component::button::ButtonCustomVariant::new(cx)
        .color(if on { pal.panel_active.into() } else { gpui_kit::transparent_white() })
        .foreground(pal.text.into())
        .hover(pal.panel_hover.into())
        .active(pal.panel_active.into())
        .shadow(false)
}

/// Underline-style dialog dropdown field: `#def1fb` hover (§4.5).
fn dialog_field_variant(cx: &App) -> gpui_kit::component::button::ButtonCustomVariant {
    let pal = p();
    gpui_kit::component::button::ButtonCustomVariant::new(cx).color(gpui_kit::transparent_white()).foreground(pal.text.into()).hover(pal.row_hover.into()).active(pal.accent_soft.into()).shadow(false)
}

/// A selection row being dragged to a new position.
#[derive(Clone)]
struct PickDrag {
    path: String,
    index: usize,
    label: SharedString,
}

impl Render for PickDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        div().px_2().py_0p5().rounded_sm().bg(pal.panel_raised).border_1().border_color(pal.accent).text_xs().text_color(pal.text).child(self.label.clone())
    }
}

/// The dialog being moved by its title; holds the grab offset.
#[derive(Clone)]
pub struct DialogMove(pub Arc<parking_lot::Mutex<Point<Pixels>>>);

struct DialogMoveGhost;

impl Render for DialogMoveGhost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// Tolerance options of a tolerant quantity (Onshape: type, deviation or
/// limit values, precision), stored as `<param>__tolerance` text.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Tolerance {
    pub kind: String,
    pub upper: String,
    pub lower: String,
    pub precision: Option<u8>,
}

impl Default for Tolerance {
    fn default() -> Self {
        Self { kind: "Symmetrical".into(), upper: "0.1".into(), lower: "0.1".into(), precision: None }
    }
}

impl Tolerance {
    pub const KINDS: &'static [&'static str] = &["Symmetrical", "Deviation", "Limits", "Min", "Max", "Basic"];

    pub fn parse(v: Option<&ParamValue>) -> Option<Self> {
        match v {
            Some(ParamValue::Text(t)) => serde_json::from_str(t).ok(),
            _ => None,
        }
    }

    pub fn encode(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    /// Value fields for the type: (slot, label).
    pub fn value_labels(&self) -> Vec<(u8, &'static str)> {
        match self.kind.as_str() {
            "Symmetrical" => vec![(0, "Deviation")],
            "Deviation" => vec![(0, "Upper"), (1, "Lower")],
            "Limits" => vec![(0, "Upper"), (1, "Lower")],
            _ => Vec::new(),
        }
    }
}

