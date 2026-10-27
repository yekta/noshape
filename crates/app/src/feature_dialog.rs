//! Generic feature dialog rendered from a feature's parameter schema.
//!
//! Behaves like Onshape's feature dialogs: fields appear/disappear live from
//! visibility predicates, flip arrows sit at the end of the previous row,
//! the focused selection field receives viewport picks, Enter accepts,
//! Shift+Enter accepts and repeats, Esc cancels.

use crate::theme::{p, with_alpha};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use crate::os_menu::OsContextMenuExt as _;
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
    /// Configure this parameter (configure mode click, right-click Configure).
    ConfigureParam(String),
    /// Right-click › Unconfigure.
    UnconfigureParam(String),
    /// The field's "Create selection" side button was clicked.
    CreateSelection(String),
    /// A reference parameter's "Import…": add the file as a tab and pick it.
    ImportReference { param: String, file: std::path::PathBuf },
    /// A reference panel's Create version…: this document (`None`) or another one.
    CreateVersion(Option<std::path::PathBuf>),
    /// Tab past the last control (`forward`) or Shift+Tab before the first
    /// one: the focus leaves the dialog (live: to the navbar / to <body>).
    LeaveFocus { forward: bool },
    /// The "Create variable" popup's ✓ (live part-studio/153): a Variable
    /// feature (or, `configuration`, a configuration variable) named `name`.
    CreateVariable(NewVariable),
}

/// What the "Create variable" popup's ✓ creates.
#[derive(Clone, Debug)]
pub struct NewVariable {
    pub name: String,
    /// Feature: LENGTH, ANGLE, NUMBER, ANY; configuration: LENGTH, ANGLE,
    /// INTEGER, REAL, TEXT.
    pub kind: &'static str,
    pub value: String,
    pub description: String,
    /// A configuration variable instead of a Variable feature.
    pub configuration: bool,
    /// A configuration variable's (minimum, maximum).
    pub range: Option<(String, String)>,
}

/// The "Create variable" popup of a numeric field (live
/// dom/dialog-create-variable-popup.txt).
struct CreateVar {
    path: String,
    /// The name typed in the field (`#initial`), replaced by the final one.
    initial: String,
    configuration: bool,
    /// Feature tab: LENGTH, ANGLE, NUMBER, ANY.
    kind: usize,
    /// Configuration tab: LENGTH, ANGLE, INTEGER, REAL, TEXT.
    config_kind: usize,
    name: Entity<InputState>,
    /// Value (Feature) / Default (Configuration).
    value: Entity<InputState>,
    description: Entity<InputState>,
    min: Entity<InputState>,
    max: Entity<InputState>,
}

const CREATE_VAR_KINDS: [(&str, &str); 4] = [("LENGTH", "Length"), ("ANGLE", "Angle"), ("NUMBER", "Number"), ("ANY", "Any")];
const CREATE_CONFIG_KINDS: [(&str, &str); 5] = [("LENGTH", "Length"), ("ANGLE", "Angle"), ("INTEGER", "Integer"), ("REAL", "Real"), ("TEXT", "Text")];

impl CreateVar {
    /// The name's error (bundle: `^[a-zA-Z_][a-zA-Z_0-9]*$`, no empty name).
    fn name_error(&self, cx: &App) -> Option<&'static str> {
        let n = self.name.read(cx).value().to_string();
        let n = n.trim().trim_start_matches('#');
        (!noshape_model::variable_table::is_identifier(n)).then_some("Variable name must be a letter followed by a string of letters or numbers.")
    }

    /// Quantity fields that don't evaluate (Value / Default, Minimum,
    /// Maximum): the bundle's quantity parameter errors, which disable save.
    fn value_errors(&self, units: &noshape_units::UnitPrefs, vars: &dyn noshape_units::VariableScope, cx: &App) -> [bool; 3] {
        let kind = if self.configuration { CREATE_CONFIG_KINDS[self.config_kind].0 } else { CREATE_VAR_KINDS[self.kind].0 };
        let dim = match kind {
            "LENGTH" => noshape_units::Dim::LENGTH,
            "ANGLE" => noshape_units::Dim::ANGLE,
            "NUMBER" | "INTEGER" | "REAL" => noshape_units::Dim::UNITLESS,
            _ => return [false; 3],
        };
        let bad = |st: &Entity<InputState>| noshape_units::evaluate_as(&st.read(cx).value(), dim, units, vars).is_err();
        if self.configuration { [bad(&self.value), bad(&self.min), bad(&self.max)] } else { [bad(&self.value), false, false] }
    }

    /// A configuration variable's range error and the fields it marks
    /// (default, minimum, maximum) (bundle `validateRange`; shared with the
    /// Configurations panel's "Variable input" dialog).
    fn range_error(&self, units: &noshape_units::UnitPrefs, cx: &App) -> Option<(&'static str, [bool; 3])> {
        if !self.configuration {
            return None;
        }
        let dim = match CREATE_CONFIG_KINDS[self.config_kind].0 {
            "LENGTH" => noshape_units::Dim::LENGTH,
            "ANGLE" => noshape_units::Dim::ANGLE,
            "TEXT" => return None,
            _ => noshape_units::Dim::UNITLESS,
        };
        let t = [&self.value, &self.min, &self.max].map(|st| st.read(cx).value().to_string());
        crate::part_studio::config_var_dialog::range_error(Some(dim), [t[0].as_str(), t[1].as_str(), t[2].as_str()], units)
    }
}

pub type RefLabeler = Arc<dyn Fn(&Reference) -> String + Send + Sync>;

/// A Part Studio that can be used as a Derived source, with its parts.
#[derive(Clone, Debug)]
pub struct DerivedSource {
    pub tab: String,
    pub parts: Vec<(noshape_model::BodyId, String)>,
}

/// Live's title tooltip reason for a value of the wrong type ("#d = 0 mm
/// [Variable] did not regenerate properly: A parameter value is not valid.").
const VALUE_NOT_VALID: &str = "A parameter value is not valid.";

pub struct FeatureDialog {
    pub feature: Feature,
    pub ty: &'static FeatureType,
    units: UnitPrefs,
    inputs: HashMap<String, Entity<InputState>>,
    pub active_query: Option<String>,
    /// The field the open Create selection dialog fills (white with a blue
    /// border while it is open, live part-studio/395).
    pub cs_target: Option<String>,
    subs: Vec<Subscription>,
    parse_errors: HashMap<String, String>,
    /// A number field typed into but not yet passed on to the preview (it
    /// waits for the field's commit, `flush_typed`).
    typed_pending: bool,
    pub status: Option<(FeatureStatus, String)>,
    /// Parameters the regeneration error points at (Onshape paints those
    /// fields pink with the message as their tooltip).
    pub faulty: Vec<String>,
    labeler: RefLabeler,
    collapsed_groups: std::collections::HashSet<String>,
    /// Where live's layout is in the parameter list, unrounded (see
    /// `live_h`); reset on each render.
    live_y: std::cell::Cell<f32>,
    /// Focus handles of the controls Tab reaches besides the text fields
    /// (checkboxes, tolerance buttons, help), by control key.
    tab_stops: HashMap<String, FocusHandle>,
    /// Tolerance options shown (`<param>__tolerance` keys); Onshape
    /// collapses them until the row's chevron is clicked.
    tol_open: std::collections::HashSet<String>,
    focus: FocusHandle,
    renaming: Option<Entity<InputState>>,
    /// Sources offered by the Derived feature's Part Studio picker.
    pub derived_sources: Vec<DerivedSource>,
    /// Tabs reference parameters can pick (file tabs, Part Studios).
    pub ref_sources: Vec<crate::fs_reference::RefSource>,
    /// The open "Select …" panel of a reference parameter.
    pub(crate) ref_picker: Option<crate::fs_reference::RefPicker>,
    /// Its "Import…" was clicked (handled at the next render, which has a window).
    pub(crate) ref_import: Option<noshape_model::params::ReferenceKind>,
    /// The document name its "Current document" tab shows.
    pub ref_document_name: String,
    /// Pictures of Part Studios and parts in that panel (`tab`, `tab#part`).
    pub ref_thumbs: HashMap<String, Arc<RenderImage>>,
    /// This document's file (its versions) and the Part Studio being edited
    /// (not offered as a reference).
    pub ref_document_path: Option<std::path::PathBuf>,
    pub ref_except: String,
    /// Renders the panel's Part Studio pictures.
    pub ref_gpu: Option<crate::gpu::Gpu>,
    /// Preview the end of the history instead of rolling to this feature.
    pub show_final: bool,
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
    /// A sketch's vertices as a point field takes them (all of them picked
    /// show as one "Vertices of Sketch 2" row, live Hole).
    pub sketch_vertices: Arc<dyn Fn(FeatureId) -> Vec<Reference> + Send + Sync>,
    /// Sketches whose vertices were picked as a whole (their list row; or
    /// all of them in a reopened feature): only these show as one
    /// "Vertices of Sketch N" row. Points clicked one by one keep a
    /// "Vertex of Sketch N" chip each (live qa5 P1-8).
    pub grouped_sketches: std::collections::HashSet<FeatureId>,
    /// Open `#` variable autofill list of a numeric field.
    completion: Option<crate::var_complete::VarCompletion>,
    /// Open "Create variable" popup.
    create_var: Option<CreateVar>,
    /// A field outside the dialog that opened the Create variable popup (the
    /// sketch's dimension box): path, input, value kind.
    external_field: Option<(String, Entity<InputState>, Option<noshape_model::params::NumKind>)>,
    /// Field text when it was focused or last applied with Enter: Enter
    /// applies a changed value and Esc reverts it before they accept or
    /// cancel the dialog (Onshape).
    baselines: HashMap<String, String>,
    /// Edited by the user, or regenerated without an error once (live
    /// `isDirty`): ✓ stays disabled until then.
    dirty: bool,
    /// Each number field's last text that evaluated as its type: the title
    /// shows it while the field holds a value of the wrong type, and a
    /// second Enter puts it back (live ui-var/03, 09).
    valid_texts: HashMap<&'static str, String>,
    /// Width after the user dragged the right edge (Onshape dialogs resize).
    pub width: Option<Pixels>,
    /// Selection lists in reorder mode (their "DONE" showing), by path.
    reordering: std::collections::HashSet<String>,
    /// `COLLAPSE_ARRAY_ITEMS` entries opened with their › ("array.index").
    open_items: std::collections::HashSet<String>,
    /// While a pick is dragged in reorder mode: list path, the row it would
    /// land next to, after it.
    pick_drop: Option<(String, usize, bool)>,
    /// "Configure features" mode: clicking a parameter configures it; the
    /// header has only ✕ and an "Unsuppressed" checkbox ends the list
    /// (live psb 10-21 g).
    pub configure_mode: bool,
    /// This feature's configured parameters (`__suppressed` included): a
    /// dotted amber outline and the tooltip "Configured" (live psb 10-21 g).
    pub configured: std::collections::HashSet<String>,
    /// The studio has a list or checkbox configuration input: parameters
    /// get Configure / Unconfigure on right-click.
    pub configurable_inputs: bool,
    /// Rows the owner adds under the parameters and a footer element left
    /// of `?` (the Sketch dialog's display options and diagnostics button).
    pub extra: Option<DialogExtra>,
    /// Onshape selects (End type, Hole type…) by field key, and what picking
    /// an item does (refreshed on every render).
    selects: HashMap<String, Entity<crate::os_select::OsSelect>>,
    select_actions: HashMap<String, SelectAction>,
    /// Values the model computed for read-only parameters (by path).
    computed: HashMap<String, String>,
    /// The initial-focus text field got the keyboard once.
    initial_focus_done: bool,
}

type SelectAction = std::rc::Rc<dyn Fn(&mut FeatureDialog, SharedString, &mut Context<FeatureDialog>)>;

/// See [`FeatureDialog::extra`].
pub type DialogExtra = Arc<dyn Fn(&mut Window, &mut Context<FeatureDialog>) -> (Vec<AnyElement>, Option<AnyElement>)>;

#[derive(Clone)]
struct ExtInfo {
    key: String,
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
    /// Names chips anew (the model before the feature has arrived).
    pub fn set_labeler(&mut self, labeler: RefLabeler, cx: &mut Context<Self>) {
        self.labeler = labeler;
        cx.notify();
    }

    pub fn new(feature: Feature, units: UnitPrefs, labeler: RefLabeler, cx: &mut Context<Self>) -> Self {
        let ty = feature.feature_type().expect("registered feature type");
        let mut feature = feature;
        fill_defaults(&ty.params, &mut feature.params, &units);
        // Live (dialogs/fillet-01): a group that is on by default starts folded
        // ("› ☑ Allow edge overflow"); checking a group unfolds it.
        let collapsed_groups = ty
            .params
            .iter()
            .filter(|s| s.hints.has(Hints::GROUP_HEADER) && !s.hints.has(Hints::GROUP_LABEL) && matches!(s.kind, ParamKind::Bool { default: true }) && feature.params.bool(s.id))
            .map(|s| s.id.to_string())
            .collect();
        let mut d = Self {
            feature,
            ty,
            units,
            inputs: HashMap::new(),
            selects: HashMap::new(),
            select_actions: HashMap::new(),
            computed: HashMap::new(),
            initial_focus_done: false,
            baselines: HashMap::new(),
            dirty: false,
            valid_texts: HashMap::new(),
            width: None,
            reordering: std::collections::HashSet::new(),
            open_items: std::collections::HashSet::new(),
            pick_drop: None,
            configure_mode: false,
            configured: Default::default(),
            configurable_inputs: false,
            active_query: None,
            cs_target: None,
            subs: Vec::new(),
            parse_errors: HashMap::new(),
            typed_pending: false,
            status: None,
            faulty: Vec::new(),
            labeler,
            collapsed_groups,
            live_y: Default::default(),
            tab_stops: HashMap::new(),
            tol_open: Default::default(),
            focus: cx.focus_handle(),
            renaming: None,
            derived_sources: Vec::new(),
            ref_sources: Vec::new(),
            ref_picker: None,
            ref_import: None,
            ref_document_name: String::new(),
            ref_thumbs: HashMap::new(),
            ref_document_path: None,
            ref_except: String::new(),
            ref_gpu: None,
            show_final: false,
            ext_info: None,
            ext_task: None,
            user_set: Default::default(),
            variables: Default::default(),
            final_available: true,
            show_help: false,
            preview: PREVIEW_REST,
            preview_track: Default::default(),
            drag_grab: Default::default(),
            missing: Arc::new(|_| false),
            sketch_region_count: Arc::new(|_| 0),
            sketch_vertices: Arc::new(|_| Vec::new()),
            grouped_sketches: Default::default(),
            completion: None,
            create_var: None,
            external_field: None,
            extra: None,
        };
        d.active_query = d.first_visible_query();
        // A sketch opened on its plane lets the filled plane field go
        // (defocusFilledSketchPlaneQLV, live sketch/21: editing Sketch 1 the
        // field is white and sketch curves pre-highlight).
        if d.feature.type_id == "sketch" && d.active_query.as_deref().is_some_and(|q| !d.feature.params.query(q).is_empty()) {
            d.active_query = None;
        }
        d
    }

    /// First visible query parameter (initial selection focus).
    pub fn first_visible_query(&self) -> Option<String> {
        self.ty
            .params
            .iter()
            .filter(|s| s.is_visible(&self.feature.params))
            // Selection-driven arrays pick too (Loft's Profiles, live loft-01).
            .find(|s| matches!(s.kind, ParamKind::Query { .. }) || s.hints.has(Hints::DRIVEN_QUERY) || s.hints.has(Hints::ALLOW_ARRAY_FOCUS))
            .map(|s| s.id.to_string())
    }

    /// A new feature whose first selection field is a single pick the
    /// preselection already filled lets that field go, as a pick does (live
    /// sketch/01: Top selected, then Sketch — "Top plane" in an inactive
    /// field).
    pub fn release_prefilled_single_pick(&mut self) {
        let Some(id) = self.active_query.clone() else { return };
        let single = self.ty.params.iter().any(|s| s.id == id && matches!(s.kind, ParamKind::Query { max_picks: Some(1), .. }));
        if single && !self.feature.params.query(&id).is_empty() {
            self.advance_query_focus(&id);
        }
    }

    /// Editing an existing feature: the first visible selection field marked
    /// INITIAL_FOCUS_ON_EDIT is the active one (uihint.gen.fs), else the
    /// first visible one as for a new feature.
    pub fn focus_for_edit(&mut self) {
        if let Some(s) = self.ty.params.iter().find(|s| s.is_visible(&self.feature.params) && s.hints.has(Hints::INITIAL_FOCUS_ON_EDIT) && matches!(s.kind, ParamKind::Query { .. })) {
            self.active_query = Some(s.id.to_string());
        }
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
        match &spec.kind {
            ParamKind::Query { filter, max_picks } => Some((path, *filter, *max_picks)),
            // A driven array picks what its items' first query takes.
            ParamKind::Array { item, .. } if spec.hints.has(Hints::DRIVEN_QUERY) => item.iter().find_map(|i| match i.kind {
                ParamKind::Query { filter, .. } => Some((path.clone(), filter, None)),
                _ => None,
            }),
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
        let shown = self.visible_selection_fields();
        self.set_value_auto(path, v, cx);
        self.focus_new_empty_field(&shown);
    }

    /// The selection fields (and selection-driven arrays) shown now, in
    /// dialog order.
    fn visible_selection_fields(&self) -> Vec<&'static str> {
        let params = &self.feature.params;
        self.ty
            .params
            .iter()
            .filter(|s| s.is_visible(params) && !s.hints.has(Hints::ALWAYS_HIDDEN))
            .filter(|s| matches!(s.kind, ParamKind::Query { .. }) || s.hints.has(Hints::DRIVEN_QUERY) || s.hints.has(Hints::ALLOW_ARRAY_FOCUS))
            .map(|s| s.id)
            .collect()
    }

    fn selection_empty(&self, path: &str) -> bool {
        match self.get_value(path) {
            Some(ParamValue::Array(items)) => items.is_empty(),
            Some(ParamValue::Query(q)) => q.is_empty(),
            _ => true,
        }
    }

    /// Onshape (updateParameterVisibility / setNewQLVFocus): after an edit,
    /// the first newly shown selection field that is empty becomes the
    /// active one (ticking Extrude's Direction: live dialogs/extrude-23),
    /// unless the active field is itself empty or in error.
    fn focus_new_empty_field(&mut self, shown_before: &[&'static str]) {
        // FOCUS_ON_VISIBLE (live p1audit loft-guides / -path / -connections):
        // such a field takes the focus even from an empty active one.
        let eager = self.visible_selection_fields().into_iter().find(|id| !shown_before.contains(id) && self.ty.params.iter().any(|s| s.id == *id && s.hints.has(Hints::FOCUS_ON_VISIBLE)));
        if let Some(id) = eager {
            self.active_query = Some(id.to_string());
            return;
        }
        let keep = self.active_query.as_deref().is_some_and(|a| self.selection_empty(a) || self.field_error(a).is_some());
        if keep {
            return;
        }
        if let Some(id) = self.visible_selection_fields().into_iter().find(|id| !shown_before.contains(id) && self.selection_empty(id)) {
            self.active_query = Some(id.to_string());
        }
    }

    /// Set a value on the user's behalf (automatic defaults): the field is
    /// not marked as user-set.
    pub fn set_value_auto(&mut self, path: &str, v: ParamValue, cx: &mut Context<Self>) {
        if let Some((vals, id)) = self.values_for_path_mut(path) {
            vals.set(&id, v);
        }
        self.refocus_hidden_query();
        // Hole: switching units or style shows another Standard; copy its values.
        if self.feature.type_id == "hole" && !noshape_model::custom::std_feature::has_editing_logic("hole") && matches!(path, "unitsSystem" | "styleV2") && noshape_model::features::hole_tables::sync_hole_standard(&mut self.feature.params) {
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

    /// A dialog without selection fields starts typing in its first text or
    /// number field (Onshape: the Variable dialog opens with Name focused).
    pub fn focus_first_field(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        // An edited feature's INITIAL_FOCUS_ON_EDIT field (`focus_for_edit`)
        // takes the focus instead of the first list.
        if let Some(id) = self.active_query.clone()
            && self.ty.params.iter().any(|s| s.id == id && s.hints.has(Hints::INITIAL_FOCUS_ON_EDIT))
        {
            let h = self.tab_stop(&format!("q:{id}"), cx);
            window.focus(&h, cx);
            return true;
        }
        let params = &self.feature.params;
        let mut first: Option<(&'static str, String)> = None;
        for s in self.ty.params.iter().filter(|s| s.is_visible(params) && !s.hints.has(Hints::ALWAYS_HIDDEN)) {
            match &s.kind {
                // Live (psb 10-19 h): a dialog opens with the focus on its
                // first selection list.
                ParamKind::Query { max_picks, .. } if first.is_none() => {
                    // A single pick the preselection filled has let go
                    // (`release_prefilled_single_pick`): it doesn't take the
                    // focus back, which would make it active again (live
                    // sketch/01: no field active).
                    if *max_picks == Some(1) && !params.query(s.id).is_empty() && self.active_query.as_deref() != Some(s.id) {
                        return false;
                    }
                    let h = self.tab_stop(&format!("q:{}", s.id), cx);
                    window.focus(&h, cx);
                    return true;
                }
                ParamKind::Query { .. } => return false,
                ParamKind::Text { .. } if first.is_none() => first = Some((s.id, params.text(s.id).to_string())),
                ParamKind::Num(_) if first.is_none() => first = Some((s.id, params.expr(s.id).map(|e| e.text.clone()).unwrap_or_default())),
                _ => {}
            }
        }
        let Some((id, text)) = first else { return false };
        let st = self.ensure_input(id, &text, window, cx);
        st.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        true
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
        if let Some(spec) = self.spec_for_path(&path)
            && let ParamKind::Array { item, .. } = &spec.kind
        {
            self.toggle_driven_item(spec.id, item_slice(item), r);
            self.changed(cx);
            cx.notify();
            return true;
        }
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
        // A single-pick field that got its pick passes the focus on
        // (Onshape: autoAdvanceToNextControl when the maximum is selected).
        if max == Some(1) && !q.is_empty() {
            self.advance_query_focus(&path);
        }
        self.changed(cx);
        cx.notify();
        true
    }

    /// The active field when it is an array taking the feature's toggle
    /// points (`ALLOW_ARRAY_FOCUS`: pattern "Instances to skip").
    pub fn toggle_points_field(&self) -> Option<&'static str> {
        let path = self.active_query.as_deref()?;
        self.ty.params.iter().find(|s| s.id == path && s.hints.has(Hints::ALLOW_ARRAY_FOCUS) && s.is_visible(&self.feature.params)).map(|s| s.id)
    }

    /// A toggle point clicked (linearPatternPointChange): the item for
    /// `index` goes away if there is one, else a new item takes it.
    pub fn toggle_instance(&mut self, index: (i64, i64), cx: &mut Context<Self>) {
        let Some(arr) = self.toggle_points_field() else { return };
        let Some(ParamKind::Array { item, .. }) = self.ty.params.iter().find(|s| s.id == arr).map(|s| &s.kind) else { return };
        let item_specs = item_slice(item);
        let two = item_specs.iter().any(|p| p.id == "index1");
        let int = |it: &ParamValues, id: &str| it.expr(id).and_then(|e| e.text.trim().parse::<i64>().ok());
        let mut v = ParamValues::default();
        fill_defaults(item_specs, &mut v, &self.units);
        let entry = self.feature.params.0.entry(arr.to_string()).or_insert_with(|| ParamValue::Array(vec![]));
        let ParamValue::Array(items) = entry else { return };
        let at = items.iter().position(|it| if two { int(it, "index1") == Some(index.0) && int(it, "index2") == Some(index.1) } else { int(it, "index") == Some(index.0) });
        match at {
            Some(k) => {
                items.remove(k);
            }
            None => {
                if two {
                    v.set("index1", ParamValue::Expr(noshape_units::ExprValue::new(index.0.to_string())));
                    v.set("index2", ParamValue::Expr(noshape_units::ExprValue::new(index.1.to_string())));
                } else {
                    v.set("index", ParamValue::Expr(noshape_units::ExprValue::new(index.0.to_string())));
                }
                items.push(v);
            }
        }
        self.inputs.retain(|p, _| !p.starts_with(&format!("{arr}.")));
        self.changed(cx);
        cx.notify();
    }

    /// A pick in a driven array ("Vertices"): the item holding `r` goes
    /// away, otherwise a new item takes it, its "match last item" values
    /// copied from the previous item (Onshape).
    fn toggle_driven_item(&mut self, arr: &'static str, item_specs: &'static [ParamSpec], r: Reference) {
        let Some(q) = item_specs.iter().find(|p| matches!(p.kind, ParamKind::Query { .. })) else { return };
        let mut v = ParamValues::default();
        fill_defaults(item_specs, &mut v, &self.units);
        let entry = self.feature.params.0.entry(arr.to_string()).or_insert_with(|| ParamValue::Array(vec![]));
        let ParamValue::Array(items) = entry else { return };
        if let Some(k) = items.iter().position(|it| it.query(q.id).contains(&r)) {
            items.remove(k);
            self.inputs.retain(|p, _| !p.starts_with(&format!("{arr}.")));
            return;
        }
        if let Some(last) = items.last() {
            for spec in item_specs.iter().filter(|p| p.hints.has(Hints::MATCH_LAST_ARRAY_ITEM)) {
                if let Some(val) = last.get(spec.id) {
                    v.set(spec.id, val.clone());
                }
            }
        }
        v.set(q.id, ParamValue::Query(vec![r]));
        items.push(v);
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
        self.changed(cx);
        cx.notify();
    }

    /// Tolerance options under a tolerant quantity (live,
    /// dom/dialog-hole-tolerance*.txt, dialogs/hole-31–35): 20 px in, past a
    /// tree line ending in a short tick, a 148 px column of 28 px rows: the
    /// precision select ("Workspace precision"), the tolerance type select,
    /// then the type's values; the Fit types add 22 px Standard / Fit type /
    /// Hole class / Shaft class rows (labels 11 px, right-aligned in 60 px).
    fn render_tolerance(&mut self, key: &str, t: Tolerance, diameter: bool, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        // The stored options (value inputs write straight into the params).
        fn stored(this: &FeatureDialog, key: &str) -> Tolerance {
            Tolerance::parse(this.get_value(key)).unwrap_or_default()
        }
        let mut col = v_flex().w(px(148.)).flex_none();
        // Precision, then type: 148 px selects with their name as tooltip.
        let k = key.to_string();
        let items = Tolerance::PRECISIONS.iter().map(|(_, l)| (SharedString::from(*l), SharedString::from(*l))).collect();
        let prec = self.os_select(&format!("{key}#precision"), items, Tolerance::precision_label(t.precision), move |this, id, cx| {
            let mut nt = stored(this, &k);
            nt.precision = Tolerance::PRECISIONS.iter().find(|(_, l)| *l == &*id).and_then(|(v, _)| *v);
            this.set_tolerance(&k, Some(nt), cx);
        }, cx);
        let k = key.to_string();
        let items = Tolerance::kinds(diameter).iter().map(|l| (SharedString::from(*l), SharedString::from(*l))).collect();
        let kind = self.os_select(&format!("{key}#type"), items, &t.kind, move |this, id, cx| {
            let mut nt = stored(this, &k);
            nt.kind = id.to_string();
            if nt.is_fit() {
                nt.fit = Some(nt.fit_spec());
            }
            this.set_tolerance(&k, Some(nt), cx);
        }, cx);
        for (name, sel) in [("Precision", prec), ("Tolerance type", kind)] {
            col = col.child(div().id(SharedString::from(format!("{key}-{name}"))).h(px(28.)).tooltip(crate::ui::tooltip(name)).child(sel));
        }
        // Values of Symmetrical / Deviation / Limits.
        for (slot, label) in t.value_labels() {
            let text = if slot == 0 { t.upper.clone() } else { t.lower.clone() };
            let ik = format!("{key}.{slot}");
            let st = match self.inputs.get(&ik) {
                Some(st) => st.clone(),
                None => {
                    let st = cx.new(|cx| InputState::new(window, cx).default_value(text));
                    let k = key.to_string();
                    let sub = cx.subscribe_in(&st, window, move |this: &mut Self, st, ev: &InputEvent, _, cx| {
                        if matches!(ev, InputEvent::Change) {
                            let mut nt = stored(this, &k);
                            let v = st.read(cx).value().to_string();
                            if slot == 0 {
                                nt.upper = v;
                            } else {
                                nt.lower = v;
                            }
                            this.feature.params.set(&k, ParamValue::Text(nt.encode()));
                            this.changed(cx);
                        }
                    });
                    self.subs.push(sub);
                    self.inputs.insert(ik.clone(), st.clone());
                    st
                }
            };
            col = col.child(
                h_flex()
                    .h(px(28.))
                    .child(div().px(px(5.)).flex_none().text_size(px(12.)).text_color(pal.text_muted).child(label))
                    .child(div().flex_1().min_w_0().child(Input::new(&st).text_size(px(13.33)).xsmall())),
            );
        }
        // Fit: Standard ▸ Fit type ▸ Hole class ▸ Shaft class.
        if t.is_fit() {
            let spec = t.fit_spec();
            let levels: [(&'static [&'static str], Vec<&'static str>, &str); 4] = [
                (&["Standard"], noshape_model::fits::STANDARDS.to_vec(), spec.standard.as_str()),
                (&["Fit type"], noshape_model::fits::FIT_TYPES.to_vec(), spec.fit_type.as_str()),
                (&["Hole", "class"], noshape_model::fits::hole_classes(&spec.standard, &spec.fit_type), spec.hole.as_str()),
                (&["Shaft", "class"], noshape_model::fits::shaft_classes(&spec.standard, &spec.fit_type, &spec.hole), spec.shaft.as_str()),
            ];
            for (level, (lines, options, current)) in levels.into_iter().enumerate() {
                let k = key.to_string();
                let items = options.iter().map(|o| (SharedString::from(*o), SharedString::from(*o))).collect();
                let sel = self.os_select(&format!("{key}#fit{level}"), items, current, move |this, id, cx| {
                    let mut nt = stored(this, &k);
                    let mut f = nt.fit_spec();
                    match level {
                        0 => f.standard = id.to_string(),
                        1 => f.fit_type = id.to_string(),
                        2 => f.hole = id.to_string(),
                        _ => f.shaft = id.to_string(),
                    }
                    nt.fit = Some(f.normalized());
                    this.set_tolerance(&k, Some(nt), cx);
                }, cx);
                sel.update(cx, |s, _| s.set_height(22.));
                // Live: the label cell is 11 px #333 at 50 % (line height 9.9 px,
                // "Hole class" on two lines), the select ≈ 88 px.
                let label = v_flex()
                    .w(px(60.))
                    .flex_none()
                    .h_full()
                    .px(px(5.))
                    .pb(px(2.))
                    .justify_center()
                    .items_end()
                    .text_size(px(11.))
                    .line_height(px(9.9))
                    .text_color(pal.text)
                    .opacity(0.5)
                    .children(lines.iter().map(|l| div().child(*l)));
                col = col.child(h_flex().h(px(22.)).child(label).child(div().flex_1().min_w_0().child(sel)));
            }
        }
        // Live: div.quantity-tolerance-indent.node-indent-line-end (20 px): a
        // 1 px line down the middle ending in a tick toward the options.
        let indent = div()
            .w(px(20.))
            .flex_none()
            .relative()
            .child(div().absolute().left(px(10.)).top_0().bottom_0().w(px(1.)).bg(pal.border))
            .child(div().absolute().left(px(10.)).bottom_0().w(px(5.)).h(px(1.)).bg(pal.border));
        h_flex().w_full().items_stretch().child(indent).child(col).into_any_element()
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
        self.changed(cx);
        cx.notify();
    }

    /// Move item `from` of array `arr` to position `to` (a driven-query
    /// array's picks, e.g. Loft profiles).
    fn move_array_item(&mut self, arr: &str, from: usize, to: usize, cx: &mut Context<Self>) {
        let Some(ParamValue::Array(items)) = self.feature.params.0.get_mut(arr) else { return };
        if from >= items.len() || to >= items.len() || from == to {
            return;
        }
        let it = items.remove(from);
        items.insert(to, it);
        self.user_set.insert(arr.to_string());
        self.changed(cx);
        cx.notify();
    }

    /// The selection list's "Reorder items" button (live behaviours.md
    /// "Selection-list reorder mode": 20 x 24 ↑↓ at the right of the caption,
    /// for lists that may be ordered with 2+ entries), "DONE" ("Done
    /// reordering") while the mode is on. Only DONE ends the mode.
    fn reorder_button(&self, path: &str, cx: &mut Context<Self>) -> AnyElement {
        let on = self.reordering.contains(path);
        let p = path.to_string();
        div()
            .id(SharedString::from(format!("reorder-{path}")))
            .absolute()
            .top_0()
            .right(px(2.))
            .h(px(24.))
            .w(px(if on { 29. } else { 20. }))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .tooltip(crate::ui::tooltip(if on { "Done reordering" } else { "Reorder items" }))
            .child(if on {
                div().text_size(px(11.)).font_weight(FontWeight::BOLD).text_color(gpui_kit::rgb(0xaaaaaa)).child("DONE").into_any_element()
            } else {
                crate::ui::glyph("reorder-items-button", 16., gpui_kit::rgb(0x888888)).into_any_element()
            })
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                cx.stop_propagation();
                if !this.reordering.remove(&p) {
                    this.reordering.insert(p.clone());
                }
                cx.notify();
            }))
            .into_any_element()
    }

    /// A row's ≡ grip in reorder mode (live: 20 x 20, `ns-resize`, in place
    /// of the ×; three 1 px lines 3 px apart).
    fn reorder_grip(&self, path: &str, index: usize, label: &str, array: bool) -> AnyElement {
        let line = || div().w(px(10.)).h(px(1.)).bg(gpui_kit::rgb(0x777777));
        let drag = PickDrag { path: path.to_string(), index, label: SharedString::from(label.to_string()), array, offset: Point::default() };
        v_flex()
            .id(SharedString::from(format!("grip-{path}-{index}")))
            .size(px(20.))
            .flex_none()
            .items_center()
            .justify_center()
            .gap(px(2.))
            .cursor(CursorStyle::ResizeUpDown)
            .child(line())
            .child(line())
            .child(line())
            .on_drag(drag, |d, offset, _, cx| {
                let mut d = d.clone();
                d.offset = offset;
                cx.new(|_| d)
            })
            .into_any_element()
    }

    /// A row of a list in reorder mode as a place to drop a dragged row: the
    /// drop line goes after it when the dragged row is above, else before.
    fn reorder_target(&self, row: Stateful<Div>, path: &str, index: usize, cx: &mut Context<Self>) -> Stateful<Div> {
        let line = self.pick_drop.as_ref().filter(|(p, i, _)| p == path && *i == index).map(|(_, _, after)| *after);
        let (p1, p2) = (path.to_string(), path.to_string());
        row.relative()
            .when_some(line, |d, after| d.child(if after { crate::ui::drop_line().bottom(px(-3.)) } else { crate::ui::drop_line().top(px(-3.)) }))
            .on_drag_move(cx.listener(move |this, e: &DragMoveEvent<PickDrag>, _, cx| {
                if !e.bounds.contains(&e.event.position) {
                    return;
                }
                let d = e.drag(cx);
                let drop = (d.path == p1 && d.index != index).then(|| (p1.clone(), index, index > d.index));
                if this.pick_drop != drop {
                    this.pick_drop = drop;
                    cx.notify();
                }
            }))
            .on_drop(cx.listener(move |this, d: &PickDrag, _, cx| {
                this.pick_drop = None;
                if d.path != p2 {
                    return;
                }
                if d.array {
                    this.move_array_item(&p2, d.index, index, cx);
                } else {
                    this.move_pick(&p2, d.index, index, cx);
                }
            }))
    }

    fn advance_query_focus(&mut self, from: &str) {
        // Onshape (getNextFocusableParameter): the next visible selection
        // field or selection-driven array, filled or not (live qa4 r5 s7:
        // Extrude direction picked → Merge scope "Part 1" active).
        let params = &self.feature.params;
        let next = self
            .ty
            .params
            .iter()
            .filter(|s| s.is_visible(params) && !s.hints.has(Hints::ALWAYS_HIDDEN))
            .skip_while(|s| s.id != from)
            .skip(1)
            .find(|s| matches!(s.kind, ParamKind::Query { .. }) || s.hints.has(Hints::DRIVEN_QUERY));
        match next {
            Some(s) => self.active_query = Some(s.id.to_string()),
            // Sketch lets its filled plane field go (defocusFilledSketchPlaneQLV;
            // live sketch/01: "Top plane" picked, the field no longer active).
            None if self.feature.type_id == "sketch" => self.active_query = None,
            // Elsewhere the focus stays where it is.
            None => {}
        }
    }

    /// An array item's label from its template (FeatureScript "Item label
    /// template"): `#param` = the item's value of that parameter (a pick's
    /// name, an option's label, a value as typed), `#index` = its position.
    fn item_label(&self, template: &str, item: &ParamValues, specs: &[ParamSpec], index: usize) -> String {
        let mut out = String::new();
        let mut rest = template;
        while let Some(at) = rest.find('#') {
            out.push_str(&rest[..at]);
            let tail = &rest[at + 1..];
            let end = tail.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(tail.len());
            let name = &tail[..end];
            let value = match (name, specs.iter().find(|p| p.id == name)) {
                ("index", None) => (index + 1).to_string(),
                (_, Some(spec)) => match (&spec.kind, item.get(name)) {
                    (ParamKind::Query { .. }, _) => item.query(name).first().map(|r| (self.labeler)(r)).unwrap_or_default(),
                    (ParamKind::Enum { options, default }, _) => options.iter().find(|(id, _)| item.is(name, id)).or_else(|| options.iter().find(|(id, _)| id == default)).map(|(_, l)| l.to_string()).unwrap_or_default(),
                    (_, Some(ParamValue::Expr(e))) => e.text.clone(),
                    (_, Some(ParamValue::Bool(b))) => b.to_string(),
                    _ => String::new(),
                },
                _ => format!("#{name}"),
            };
            out.push_str(&value);
            rest = &tail[end..];
        }
        out.push_str(rest);
        out.trim().to_string()
    }

    /// Show the model's values of read-only parameters.
    pub fn set_computed(&mut self, computed: HashMap<String, String>, cx: &mut Context<Self>) {
        if self.computed != computed {
            self.computed = computed;
            cx.notify();
        }
    }

    /// The Onshape select for `key` showing `items` with `current` chosen;
    /// `action` runs when an item is picked.
    fn os_select(&mut self, key: &str, items: Vec<(SharedString, SharedString)>, current: &str, action: impl Fn(&mut Self, SharedString, &mut Context<Self>) + 'static, cx: &mut Context<Self>) -> Entity<crate::os_select::OsSelect> {
        self.select_actions.insert(key.to_string(), std::rc::Rc::new(action));
        let sel = match self.selects.get(key) {
            Some(s) => s.clone(),
            None => {
                // Selects are Tab stops (live psb 10-19 h: the End type focusser).
                let s = cx.new(|cx| {
                    let mut s = crate::os_select::OsSelect::new(SharedString::from(format!("sel-{key}")));
                    s.focus_handle(cx);
                    s
                });
                let k = key.to_string();
                let sub = cx.subscribe(&s, move |this, _, ev: &crate::os_select::OsSelectEvent, cx| {
                    let crate::os_select::OsSelectEvent::Picked(id) = ev;
                    if let Some(f) = this.select_actions.get(&k).cloned() {
                        f(this, id.clone(), cx);
                    }
                });
                self.subs.push(sub);
                self.selects.insert(key.to_string(), s.clone());
                s
            }
        };
        let current = current.to_string();
        // The options as the feature specs have them in the UI language.
        let items = items.into_iter().map(|(id, l)| (id, SharedString::from(crate::i18n::spec(&l)))).collect();
        sel.update(cx, |s, _| {
            s.set_translated(true);
            s.set_items(items, current)
        });
        sel
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

    /// Refs of the active field: the only picks the dialog highlights
    /// (live psui/m-03, m-04: Part 1 in Entities to mirror isn't outlined
    /// while Mirror plane is active). A selection-driven array (Loft's
    /// Profiles, Fillet's Vertices) gives every item's picks.
    pub fn active_refs(&self) -> Vec<Reference> {
        match self.active_query.as_deref().and_then(|p| self.get_value(p)) {
            Some(ParamValue::Query(q)) => q.clone(),
            Some(ParamValue::Array(items)) => items.iter().flat_map(|it| it.0.values()).filter_map(|v| if let ParamValue::Query(q) = v { Some(q.iter().cloned()) } else { None }).flatten().collect(),
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
        self.changed(cx);
        cx.notify();
    }

    /// A user edit: the preview re-runs and ✓ may enable (live
    /// `canSubmit`: the dialog is dirty and no value fails validation).
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.dirty = true;
        // The preview runs with every value, typed ones included: a commit
        // (Tab) that rewrote the text doesn't schedule a second run.
        self.typed_pending = false;
        cx.emit(DialogEvent::Changed);
    }

    /// Live: a regeneration without an error marks the dialog dirty too.
    pub fn note_regenerated(&mut self, ok: bool) {
        self.dirty |= ok;
    }

    pub fn set_status(&mut self, s: Option<(FeatureStatus, String)>, cx: &mut Context<Self>) {
        if self.status != s {
            self.status = s;
            cx.notify();
        }
    }

    pub fn set_faulty(&mut self, faulty: Vec<String>, cx: &mut Context<Self>) {
        if self.faulty != faulty {
            self.faulty = faulty;
            cx.notify();
        }
    }

    /// The error a field shows: the feature's error when the regeneration
    /// names this parameter.
    pub(crate) fn field_error(&self, id: &str) -> Option<String> {
        let (FeatureStatus::Error, msg) = self.status.as_ref()? else { return None };
        if !self.faulty.is_empty() {
            return self.faulty.iter().any(|f| f == id).then(|| msg.clone());
        }
        // Features that don't name the parameter: the first empty
        // selection field is the one Onshape marks.
        let first_empty = self.ty.params.iter().find(|s| s.is_visible(&self.feature.params) && matches!(s.kind, ParamKind::Query { .. }) && self.feature.params.query(s.id).is_empty());
        first_empty.is_some_and(|s| s.id == id).then(|| msg.clone())
    }

    // ------------------------------------------------------------ inputs

    fn ensure_input(&mut self, path: &str, initial: &str, window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        if let Some(i) = self.inputs.get(path) {
            return i.clone();
        }
        let state = cx.new(|cx| InputState::new(window, cx).default_value(initial.to_string()));
        // A text field marked for initial focus (Variable's Name) takes the
        // keyboard when the dialog opens (live).
        if !self.initial_focus_done && self.spec_for_path(path).is_some_and(|s| matches!(s.kind, ParamKind::Text { .. }) && s.hints.has(Hints::INITIAL_FOCUS)) {
            self.initial_focus_done = true;
            state.update(cx, |s, cx| s.focus(window, cx));
        }
        let path_s = path.to_string();
        let sub = cx.subscribe_in(&state, window, move |this: &mut FeatureDialog, state, ev: &InputEvent, window, cx| match ev {
            InputEvent::Change => {
                let text = state.read(cx).value().to_string();
                // Number fields wait for their commit before the preview
                // re-runs (live s2; as qa4's 66563c00); other text applies.
                if this.spec_for_path(&path_s).is_some_and(|s| matches!(s.kind, ParamKind::Num(_))) {
                    if this.set_text_value(&path_s, text) {
                        this.typed_pending = true;
                        cx.notify();
                    }
                } else {
                    this.on_text_changed(&path_s, text, cx);
                }
                // Number fields: units of the field's kind are offered too.
                let dim = this.spec_for_path(&path_s).and_then(|s| match &s.kind {
                    ParamKind::Num(n) if n.kind == noshape_model::params::NumKind::Integer => Some(noshape_units::Dim::UNITLESS),
                    ParamKind::Num(n) => Some(n.kind.dim()),
                    _ => None,
                });
                this.completion = dim.and_then(|d| crate::var_complete::VarCompletion::compute(&path_s, state.read(cx), &this.variables, Some(d)));
            }
            InputEvent::PressEnter { shift, .. } => {
                // Onshape: Enter in a changed field applies the value (the
                // text stays selected); the next Enter commits the feature.
                if *shift {
                    cx.emit(DialogEvent::Accept { repeat: true });
                } else if this.apply_on_enter(&path_s, state, window, cx) {
                    this.flush_typed(cx);
                } else if !this.refuse_invalid_enter(&path_s, state, window, cx) {
                    cx.emit(DialogEvent::Accept { repeat: false });
                }
            }
            // Onshape keeps the selection field active while a number field
            // has keyboard focus ("two fields are active"), so picks still
            // go to it.
            InputEvent::Focus => cx.notify(),
            InputEvent::Blur => {
                // Tab / click-away commits: the value gets its unit (live).
                this.completion = None;
                this.commit_number(&path_s, state, window, cx);
                this.flush_typed(cx);
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
        // Live ("Input behaviour", qa4 F30): Enter in a count (Instance
        // count) accepts the whole dialog at once; only quantities (Depth,
        // a Variable's value: "8" → "8 mm") apply first.
        if matches!(self.spec_for_path(path).map(|s| &s.kind), Some(ParamKind::Num(n)) if n.kind == noshape_model::params::NumKind::Integer) {
            self.commit_number(path, state, window, cx);
            return false;
        }
        // Live: "30" → "30 mm", "10+5" → "(10+5) mm", the text re-selected.
        self.commit_number(path, state, window, cx);
        let text = state.read(cx).value().to_string();
        self.baselines.insert(path.to_string(), text);
        state.update(cx, |s, cx| s.select_all(window, cx));
        cx.notify();
        true
    }

    /// One ↑ / ↓ / wheel step of a number field (`quantity_field::step_text`):
    /// only a plain value unchanged since its commit steps; the new value is
    /// committed and selected. False when it can't step.
    fn step_number(&mut self, path: &str, state: &Entity<InputState>, up: bool, mods: crate::quantity_field::StepMods, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.completion.as_ref().is_some_and(|c| c.key == path) {
            return false;
        }
        let Some(ParamKind::Num(n)) = self.spec_for_path(path).map(|s| s.kind.clone()) else { return false };
        let text = state.read(cx).value().to_string();
        if self.baselines.get(path).is_some_and(|b| *b != text) {
            return false;
        }
        let integer = n.kind == noshape_model::params::NumKind::Integer;
        let dim = if integer { noshape_units::Dim::UNITLESS } else { n.kind.dim() };
        let f = self.units.si_factor(dim);
        let stepping = crate::quantity_field::Stepping { dim, prefs: &self.units, scope: &self.variables, integer, range: (Some(n.min / f), Some(n.max / f)) };
        let Some(next) = crate::quantity_field::step_text(&text, up, mods, &stepping) else { return false };
        state.update(cx, |s, cx| {
            s.set_value(next.clone(), window, cx);
            s.select_all(window, cx);
        });
        self.baselines.insert(path.to_string(), next.clone());
        self.on_text_changed(path, next, cx);
        true
    }

    /// A number field's commit (Enter, Tab, blur): its text rewritten by
    /// Onshape's rules (`quantity_field::commit_text`). An empty field keeps
    /// its last text (live).
    fn commit_number(&mut self, path: &str, state: &Entity<InputState>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ParamKind::Num(n)) = self.spec_for_path(path).map(|s| &s.kind) else { return };
        let dim = if n.kind == noshape_model::params::NumKind::Integer { noshape_units::Dim::UNITLESS } else { n.kind.dim() };
        let text = state.read(cx).value().to_string();
        // Live (DEPTH-RANGE): a value out of the field's range stays as typed.
        if self.out_of_range(path, &text) {
            return;
        }
        let next = if text.trim().is_empty() { self.baselines.get(path).cloned().unwrap_or_default() } else { crate::quantity_field::commit_text(&text, dim, &self.units, &self.variables) };
        if next != text {
            state.update(cx, |s, cx| s.set_value(next.clone(), window, cx));
            self.on_text_changed(path, next, cx);
        }
    }

    /// A number field's text evaluating outside its parameter's range (live
    /// "qa (2026-10-26, P2) Extrude Depth out of range": above 500 m the
    /// field is pink and keeps the text as typed; 500 m itself is fine).
    fn out_of_range(&self, path: &str, text: &str) -> bool {
        let Some(ParamKind::Num(n)) = self.spec_for_path(path).map(|s| &s.kind) else { return false };
        let dim = if n.kind == noshape_model::params::NumKind::Integer { noshape_units::Dim::UNITLESS } else { n.kind.dim() };
        let slack = |v: f64| v.abs() * 1e-12 + 1e-15;
        noshape_units::evaluate_as(text, dim, &self.units, &self.variables).is_ok_and(|q| q.value < n.min - slack(n.min) || q.value > n.max + slack(n.max))
    }

    /// Insert autofill entry `index` (the selected one when `None`) into the
    /// field and close the list.
    fn accept_completion(&mut self, index: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = self.completion.take() else { return };
        // "New variable…" opens the Create variable popup (live 153).
        if c.picks_new(index)
            && let Some(name) = c.new_variable.clone()
        {
            self.open_create_variable(c.key.clone(), name, window, cx);
            cx.notify();
            return;
        }
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
        let (p1, p2, p3, p4, p5) = (path.to_string(), path.to_string(), path.to_string(), path.to_string(), path.to_string());
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
        // Tab accepts too (live 152: "Tab/Enter").
        .capture_action(cx.listener(move |this, _: &crate::actions::DialogTab, window, cx| {
            if open(this, &p5) {
                cx.stop_propagation();
                this.accept_completion(None, window, cx);
            }
        }))
    }

    /// The "Create variable" popup for field `path` (live 10-20 o, 577–579;
    /// bundle CreateVariable): Name `name`; Value / Default = the field's
    /// previous value, selected; the type tab matching the field;
    /// Configuration adds Minimum 0 and Maximum 100000 in the workspace
    /// unit.
    fn open_create_variable(&mut self, path: String, name: String, window: &mut Window, cx: &mut Context<Self>) {
        use noshape_model::params::NumKind;
        let kind = self.spec_for_path(&path).and_then(|s| if let ParamKind::Num(n) = &s.kind { Some(n.kind) } else { None }).or_else(|| self.external_field.as_ref().filter(|f| f.0 == path).and_then(|f| f.2));
        let (k, ck, unit) = match kind {
            Some(NumKind::Angle) => (1, 1, Some(self.units.angle.symbol())),
            Some(NumKind::Integer) => (2, 2, None),
            Some(NumKind::Real) => (2, 3, None),
            _ => (0, 0, Some(self.units.length.symbol())),
        };
        let fallback = noshape_model::params::default_expr(&noshape_model::params::NumSpec::length(25.0, 1.0), &self.units).text;
        let value_text = self.baselines.get(&path).filter(|b| !b.trim().starts_with('#')).cloned().unwrap_or(fallback);
        let bound = |n: &str| unit.map(|u| format!("{n} {u}")).unwrap_or_else(|| n.to_string());
        let (lo, hi) = (bound("0"), bound("100000"));
        let initial = name.clone();
        let name = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let value = cx.new(|cx| InputState::new(window, cx).default_value(value_text));
        let description = cx.new(|cx| InputState::new(window, cx));
        let min = cx.new(|cx| InputState::new(window, cx).default_value(lo));
        let max = cx.new(|cx| InputState::new(window, cx).default_value(hi));
        value.update(cx, |s, cx| {
            s.focus(window, cx);
            s.select_all(window, cx);
        });
        self.create_var = Some(CreateVar { path, initial, configuration: false, kind: k, config_kind: ck, name, value, description, min, max });
    }

    /// A field's input: the dialog's own, or the outside one that opened
    /// the Create variable popup.
    fn field_state(&self, path: &str) -> Option<Entity<InputState>> {
        self.inputs.get(path).cloned().or_else(|| self.external_field.as_ref().filter(|f| f.0 == path).map(|f| f.1.clone()))
    }

    /// The Create variable popup for a field outside the dialog (the
    /// sketch's dimension box, live: an unknown `#w` + Enter opens it with
    /// the name prefilled and the field's previous value). The caller draws
    /// it ([`Self::external_create_variable_popup`]).
    pub(crate) fn open_external_create_variable(&mut self, f: ExternalField, window: &mut Window, cx: &mut Context<Self>) {
        self.external_field = Some((f.path.to_string(), f.state, f.kind));
        self.baselines.insert(f.path.to_string(), f.previous);
        self.open_create_variable(f.path.to_string(), f.name, window, cx);
        cx.notify();
    }

    /// Drop the Create variable popup of outside field `path` (its box was
    /// closed: a new box starts without it, qa5 F32).
    pub(crate) fn close_external_create_variable(&mut self, path: &str, cx: &mut Context<Self>) {
        if self.create_var.as_ref().is_some_and(|c| c.path == path) {
            self.create_var = None;
        }
        if self.external_field.as_ref().is_some_and(|f| f.0 == path) {
            self.external_field = None;
        }
        cx.notify();
    }

    /// The open Create variable popup of outside field `path`, if any.
    pub(crate) fn external_create_variable_popup(&mut self, path: &str, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.create_var.as_ref().is_some_and(|c| c.path == path && self.external_field.as_ref().is_some_and(|f| f.0 == path)).then(|| self.render_create_variable(cx))
    }

    /// The "Create variable" popup (live part-studio/153, dom/dialog-create-
    /// variable-popup.txt): 220 px, right of the field and level with it;
    /// title + ✓ ✕; tabs Feature | Configuration and Length | Angle |
    /// Number | Any; Name, Value, Description.
    fn render_create_variable(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let pal = p();
        let Some(c) = &self.create_var else { return div().into_any_element() };
        let blue = gpui_kit::rgb(0x1651b0);
        let tab = |id: SharedString, label: &'static str, on: bool, w: f32| {
            div()
                .id(id)
                .w(px(w))
                .h(px(26.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .text_size(px(12.))
                .when(on, |d| d.font_weight(FontWeight::MEDIUM).text_color(blue).border_b_2().border_color(blue))
                .when(!on, |d| d.text_color(pal.text).border_b_1().border_color(gpui_kit::rgb(0xd7d7d7)))
                .child(label)
        };
        let target = h_flex()
            .child(tab("cv-feature".into(), "Feature", !c.configuration, 95.).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                if let Some(c) = &mut this.create_var {
                    c.configuration = false;
                }
                cx.notify();
            })))
            .child(tab("cv-config".into(), "Configuration", c.configuration, 125.).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                if let Some(c) = &mut this.create_var {
                    c.configuration = true;
                }
                cx.notify();
            })));
        let mut kinds = h_flex();
        if c.configuration {
            for (i, (_, label)) in CREATE_CONFIG_KINDS.iter().enumerate() {
                kinds = kinds.child(tab(SharedString::from(format!("cv-ckind-{i}")), label, c.config_kind == i, 44.).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(c) = &mut this.create_var {
                        c.config_kind = i;
                    }
                    cx.notify();
                })));
            }
        } else {
            let widths = [60., 53., 65., 42.];
            for (i, (_, label)) in CREATE_VAR_KINDS.iter().enumerate() {
                kinds = kinds.child(tab(SharedString::from(format!("cv-kind-{i}")), label, c.kind == i, widths[i]).on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if let Some(c) = &mut this.create_var {
                        c.kind = i;
                    }
                    cx.notify();
                })));
            }
        }
        // A field in error is outlined red with the reason as tooltip (the
        // bundle's parameter errors).
        let row = |label: &'static str, st: &Entity<InputState>, err: Option<&'static str>| {
            h_flex()
                .id(SharedString::from(format!("cv-row-{label}")))
                .h(px(26.))
                .w(px(194.))
                .child(div().flex_none().pl(px(5.)).pr(px(5.)).text_size(px(12.)).text_color(gpui_kit::rgb(0x666666)).child(label))
                .child(gpui_kit::base::input::input_frame(
                    div()
                        .flex_1()
                        .min_w_0()
                        .border_b_1()
                        .border_color(if err.is_some() { pal.error_outline } else { gpui_kit::rgb(0xd7d7d7) })
                        .when(err.is_some(), |d| d.bg(pal.error_fill))
                        .child(Input::new(st).small().appearance(false).text_size(px(13.33))),
                ))
                .when_some(err, |d, e| d.tooltip(crate::ui::tooltip(e)))
        };
        let name_err = c.name_error(cx);
        let range = c.range_error(&self.units, cx);
        let invalid = c.value_errors(&self.units, &self.variables, cx);
        let marked = |i: usize| if invalid[i] { Some("Enter a valid expression.") } else { range.and_then(|(e, m)| m[i].then_some(e)) };
        // Save is disabled while a field is in error (bundle `disableSave`),
        // pale like a feature dialog's ✓.
        let blocked = name_err.is_some() || range.is_some() || invalid.contains(&true);
        let text = c.configuration && CREATE_CONFIG_KINDS[c.config_kind].0 == "TEXT";
        let fields = if c.configuration {
            v_flex().child(row("Name", &c.name, name_err)).child(row("Default", &c.value, marked(0))).when(!text, |d| d.child(row("Minimum", &c.min, marked(1))).child(row("Maximum", &c.max, marked(2))))
        } else {
            v_flex().child(row("Name", &c.name, name_err)).child(row("Value", &c.value, marked(0))).child(row("Description", &c.description, None))
        };
        let popup = v_flex()
            .id("create-variable")
            .occlude()
            .w(px(220.))
            .bg(gpui_kit::rgb(0xffffff))
            .shadow_md()
            .text_color(pal.text)
            // Enter commits, Esc cancels (like a feature dialog).
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| match e.keystroke.key.as_str() {
                "enter" => {
                    cx.stop_propagation();
                    this.commit_create_variable(window, cx);
                }
                "escape" => {
                    cx.stop_propagation();
                    this.cancel_create_variable(window, cx);
                }
                _ => {}
            }))
            .child(
                h_flex()
                    .h(px(29.))
                    .border_b_1()
                    .border_color(gpui_kit::rgb(0xd7d7d7))
                    .child(div().flex_1().pl(px(6.)).text_size(px(13.)).font_weight(FontWeight::MEDIUM).child("Create variable"))
                    .child(div().id("cv-ok").size(px(28.)).flex().items_center().justify_center().bg(pal.accept).when(blocked, crate::ui::disabled_accept).cursor_pointer().child(crate::ui::glyph("check", 16., gpui_kit::rgb_static(0xffffff))).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.commit_create_variable(window, cx))))
                    .child(div().id("cv-cancel").size(px(28.)).flex().items_center().justify_center().cursor_pointer().child(crate::ui::glyph("close", 14., gpui_kit::rgb(0xd52b1e))).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.cancel_create_variable(window, cx)))),
            )
            .child(target)
            .child(kinds)
            .child(fields);
        // Anchored at a zero-size box under the field: up beside it.
        div().relative().w_full().h_0().child(deferred(anchored().offset(point(px(107.), px(-40.))).snap_to_window().child(popup)).with_priority(5)).into_any_element()
    }

    /// ✓: create the variable; the field then resolves.
    /// ✕ / Esc: back to the field with its text selected (bundle
    /// `focusControlAndSelectValue`).
    fn cancel_create_variable(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = self.create_var.take() else { return };
        if let Some(st) = self.field_state(&c.path) {
            st.update(cx, |s, cx| {
                s.focus(window, cx);
                s.select_all(window, cx);
            });
        }
        cx.notify();
    }

    fn commit_create_variable(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // ✓ does nothing while a field is in error (bundle `disableSave`).
        if self.create_var.as_ref().is_some_and(|c| c.name_error(cx).is_some() || c.range_error(&self.units, cx).is_some() || c.value_errors(&self.units, &self.variables, cx).contains(&true)) {
            return;
        }
        let Some(c) = self.create_var.take() else { return };
        let name = c.name.read(cx).value().trim().trim_start_matches('#').to_string();
        let value = c.value.read(cx).value().to_string();
        let kind = if c.configuration { CREATE_CONFIG_KINDS[c.config_kind].0 } else { CREATE_VAR_KINDS[c.kind].0 };
        if let Some(dim) = match kind {
            "LENGTH" => Some(noshape_units::Dim::LENGTH),
            "ANGLE" => Some(noshape_units::Dim::ANGLE),
            "NUMBER" | "INTEGER" | "REAL" => Some(noshape_units::Dim::UNITLESS),
            _ => None,
        } && let Ok(q) = noshape_units::evaluate_as(&value, dim, &self.units, &self.variables)
        {
            self.variables.insert(name.clone(), q);
        }
        let range = (c.configuration && kind != "TEXT").then(|| (c.min.read(cx).value().to_string(), c.max.read(cx).value().to_string()));
        // The field keeps `#name` (renamed when the popup's name changed) and
        // gets the focus back (bundle `insertVariableName`).
        if let Some(st) = self.field_state(&c.path) {
            let text = st.read(cx).value().to_string();
            let text = noshape_model::variable_table::rename_in_expr(&text, &c.initial, &name);
            st.update(cx, |s, cx| {
                s.set_value(text.clone(), window, cx);
                s.focus(window, cx);
            });
            self.on_text_changed(&c.path, text, cx);
        }
        cx.emit(DialogEvent::CreateVariable(NewVariable { name, kind, value, description: c.description.read(cx).value().to_string(), configuration: c.configuration, range }));
        self.changed(cx);
        cx.notify();
    }

    fn on_text_changed(&mut self, path: &str, text: String, cx: &mut Context<Self>) {
        if self.set_text_value(path, text) {
            self.changed(cx);
            cx.notify();
        }
    }

    /// The preview re-runs for typed text only when the field commits (Tab,
    /// a click away, Enter): live s2 "Typing does not update the preview".
    fn flush_typed(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.typed_pending) {
            self.changed(cx);
        }
    }

    /// A shown number field whose value doesn't evaluate as its type (a
    /// wrong type, a bad expression, an unknown variable): its id and live's
    /// message for it.
    pub fn value_error(&self, _cx: &App) -> Option<(&'static str, String)> {
        self.value_errors().next()
    }

    fn value_errors(&self) -> impl Iterator<Item = (&'static str, String)> + '_ {
        self.eval_errors(|_, typed| typed.to_string())
    }

    /// Each shown number field's `text` that doesn't evaluate as its type,
    /// with live's message for it.
    fn eval_errors<'a>(&'a self, text: impl Fn(&str, &str) -> String + 'a) -> impl Iterator<Item = (&'static str, String)> + 'a {
        self.ty.params.iter().filter(|s| s.is_visible(&self.feature.params)).filter_map(move |s| {
            let ParamKind::Num(n) = &s.kind else { return None };
            let Some(ParamValue::Expr(e)) = self.feature.params.get(s.id) else { return None };
            let text = text(s.id, &e.text);
            if text.trim().is_empty() {
                return None;
            }
            noshape_units::evaluate_as(&text, n.kind.dim(), &self.units, &self.variables).err().map(|err| (s.id, crate::quantity_field::eval_message(&err)))
        })
    }

    /// A number field's applied text: what Enter or leaving the field last
    /// committed. Live (ui-var2/02-07): the title, the row, the pink field
    /// and ✓ follow it, never the text being typed.
    fn applied_text(&self, id: &str, typed: &str) -> String {
        if self.inputs.contains_key(id) { self.baselines.get(id).cloned().unwrap_or_else(|| typed.to_string()) } else { typed.to_string() }
    }

    fn applied_value_error(&self) -> Option<(&'static str, String)> {
        self.eval_errors(|id, typed| self.applied_text(id, typed)).next()
    }

    /// The feature as its title shows it: each number field's applied
    /// value, or its last valid one while that is of the wrong type (live
    /// ui-var/03: "#d = 0 mm").
    fn shown_feature(&self) -> Feature {
        let mut f = self.feature.clone();
        for s in self.ty.params.iter().filter(|s| matches!(s.kind, ParamKind::Num(_))) {
            let Some(ParamValue::Expr(e)) = self.feature.params.get(s.id) else { continue };
            let applied = self.applied_text(s.id, &e.text);
            let text = if self.eval_errors(|_, _| applied.clone()).any(|(id, _)| id == s.id) { self.valid_texts.get(s.id).cloned().unwrap_or(applied) } else { applied };
            if text != e.text {
                let mut e = e.clone();
                e.text = text;
                f.params.set(s.id, ParamValue::Expr(e));
            }
        }
        f
    }

    /// The feature as the list shows its row while the dialog is open.
    pub fn row_feature(&self) -> Feature {
        self.shown_feature()
    }

    /// A second Enter on a value of the wrong type keeps the dialog open
    /// and puts the field's last valid value back, still pink (live
    /// ui-var/09). False when `path` holds no such value.
    fn refuse_invalid_enter(&mut self, path: &str, state: &Entity<InputState>, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some((id, _)) = self.value_errors().find(|(id, _)| *id == path) else { return false };
        let Some(text) = self.valid_texts.get(id).cloned() else { return true };
        state.update(cx, |s, cx| {
            s.set_value(text, window, cx);
            s.select_all(window, cx);
        });
        cx.notify();
        true
    }

    /// A field's text into the feature's value (no preview update); false
    /// when the field isn't a number, checkbox expression or text.
    fn set_text_value(&mut self, path: &str, text: String) -> bool {
        let Some(spec) = self.spec_for_path(path) else { return false };
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
            _ => return false,
        };
        if let Some((vals, id)) = self.values_for_path_mut(path) {
            vals.set(&id, v);
        }
        true
    }

    // ------------------------------------------------------------ render helpers

    /// The pixel height of a row live lays out `h` px tall. Chrome keeps
    /// fractional rows (17.33 px label lines: 26.33 px empty selection
    /// fields, 27.33 px checkbox rows) and snaps each top to the pixel grid;
    /// gpui rounds an authored length first. Placing every fractional row
    /// from the running unrounded position gives live's 27/28 px steps
    /// (dom/dialog-extrude-new: checkbox rows at 254.3, 281.7, 309, 336.3,
    /// 363.7 → tops 254, 282, 309, 336, 364). Whole-pixel rows don't move the
    /// phase, so only fractional rows call this, in render order.
    /// Heights in Chrome's layout units (1/64 px): 17.33 px lines floor to
    /// 17.328125.
    fn live_h(&self, h: f32) -> f32 {
        let y = self.live_y.get();
        self.live_y.set(y + h);
        (y + h).round() - y.round()
    }

    fn render_params(&mut self, specs: &'static [ParamSpec], prefix: Option<(&'static str, usize)>, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        // Array items see the feature's values too (FeatureScript reads
        // `definition.isAsymmetric` inside its item loops); theirs win.
        let values: ParamValues = match prefix {
            None => self.feature.params.clone(),
            Some((arr, i)) => {
                let mut v = self.feature.params.clone();
                v.0.shift_remove(arr);
                for (k, x) in self.feature.params.array(arr).get(i).map(|it| it.0.clone()).unwrap_or_default() {
                    v.0.insert(k, x);
                }
                v
            }
        };
        // Items of a driven array show their query in the array's field;
        // an opened COLLAPSE_ARRAY_ITEMS entry shows it again inside (live
        // psui-loft-01: Loft's profile, "Faces and sketch regions").
        let driven = prefix.is_some_and(|(arr, _)| self.ty.params.iter().any(|p| p.id == arr && p.hints.has(Hints::DRIVEN_QUERY) && !p.hints.has(Hints::COLLAPSE_ARRAY_ITEMS)));
        // Rows with their group depth (indented once the next row is known).
        let mut rows: Vec<(AnyElement, usize)> = Vec::new();
        let mut i = 0;
        // Open groups, outermost first: the header and whether it is expanded.
        // Groups nest (Sheet metal model's Selections holds Extrude's
        // Second end position group); a param belongs to a group while
        // turning the header off would hide it.
        let mut groups: Vec<(&'static str, bool)> = Vec::new();
        let (mut prev_query, mut prev_depth) = (false, 0);
        while i < specs.len() {
            let s = &specs[i];
            i += 1;
            if !s.is_visible(&values) || (driven && matches!(s.kind, ParamKind::Query { .. })) {
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
            let depth = groups.len();
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
                } else if !n.is_visible(&values) && specs[i + 1..].iter().find(|x| x.is_visible(&values)).is_some_and(|x| x.hints.has(Hints::OPPOSITE_DIRECTION) || x.hints.has(Hints::OPPOSITE_DIRECTION_CIRCULAR)) {
                    // A hidden param between a row and its flip (Plane: Angle
                    // between Offset distance and Opposite direction).
                    i += 1;
                } else {
                    break;
                }
            }
            let row = if s.hints.has(Hints::PRIMARY_AXIS) {
                // Live (dom/dialog-mate-connector.txt): Flip primary axis and
                // the next "Reorient secondary axis" are one row of 26 px icon
                // buttons.
                let next_axis = specs.get(i).filter(|n| n.hints.has(Hints::MATE_CONNECTOR_AXIS_TYPE) && n.is_visible(&values));
                if next_axis.is_some() {
                    i += 1;
                }
                Some(self.render_axis_buttons(s, &path, next_axis, &values, prefix, cx))
            } else {
                self.render_param(s, &path, &values, companion, &trailing, prefix, window, cx)
            };
            // Configure features mode: a click on a top-level number,
            // checkbox, choice or selection configures it at once (live psb
            // 10-21 g: no outline before; the dialog stays open).
            let can_configure = prefix.is_none() && matches!(s.kind, ParamKind::Num(_) | ParamKind::Bool { .. } | ParamKind::Enum { .. } | ParamKind::Query { .. }) && !s.hints.has(Hints::UNCONFIGURABLE);
            let configurable = self.configure_mode && can_configure;
            let is_configured = prefix.is_none() && self.configured.contains(s.id);
            // Right-click (live): numbers and choices "Configure" /
            // "Unconfigure"; selections "Clear all" first (their pick rows
            // have it too); checkboxes keep "Convert to expression" (+
            // "Unconfigure"), in their own menu.
            let menu = !self.configure_mode && self.configurable_inputs && can_configure && !matches!(s.kind, ParamKind::Bool { .. });
            let row = row.map(|r| {
                let pid = s.id.to_string();
                let mut el = div().id(SharedString::from(format!("cfg-{pid}"))).child(r);
                if is_configured {
                    // Live: no "Configured" tag on a checkbox row.
                    el = el.border_2().border_dashed().border_color(gpui_kit::rgb(0xfab601));
                    if !matches!(s.kind, ParamKind::Bool { .. }) {
                        el = el.tooltip(crate::ui::tooltip("Configured"));
                    }
                }
                if configurable {
                    // Capture phase: the control under the click (a checkbox,
                    // a select) must not change its own value (qa5 C2).
                    let pid = pid.clone();
                    el = el.cursor_pointer().capture_any_mouse_down(cx.listener(move |_, e: &MouseDownEvent, _, cx| {
                        if e.button == MouseButton::Left {
                            cx.stop_propagation();
                            cx.emit(DialogEvent::ConfigureParam(pid.clone()));
                        }
                    }));
                }
                if menu {
                    let view = cx.entity();
                    let query = matches!(s.kind, ParamKind::Query { .. });
                    let path = path.clone();
                    let build = move |m: crate::os_menu::OsMenu, _: &mut Window, _: &mut Context<crate::os_menu::OsMenu>| {
                        let mut m = m;
                        if query {
                            let (v, p) = (view.clone(), path.clone());
                            m = m.item(crate::os_menu::OsMenuItem::new("Clear all").on_click(move |_, _, cx| {
                                v.update(cx, |this, cx| {
                                    this.set_value(&p, ParamValue::Query(Vec::new()), cx);
                                    cx.emit(DialogEvent::HoverRef(None));
                                })
                            }));
                        }
                        configure_items(m, &view, &pid, is_configured)
                    };
                    // Over a number field the menu wins over the input's own
                    // Cut / Copy / Paste (live: none); pick rows keep theirs.
                    el = if query { el.os_context_menu(build) } else { el.os_context_menu_over_inputs(build) };
                }
                el.into_any_element()
            });
            if s.hints.has(Hints::GROUP_HEADER) {
                groups.push((s.id, values.bool(s.id) && !self.collapsed_groups.contains(&path)));
            }
            if let Some(row) = row {
                let is_query = matches!(s.kind, ParamKind::Query { .. }) || s.hints.has(Hints::DRIVEN_QUERY);
                if prev_query && !is_query {
                    // Live (`os-param-requires-margin`): 5 px under the last
                    // field, inside its group.
                    rows.push((div().h(px(5.)).flex_none().into_any_element(), prev_depth));
                }
                (prev_query, prev_depth) = (is_query, depth);
                rows.push((row, depth));
            }
        }
        // Live (dialogs/mirror, delete-part): a query field ending the
        // dialog keeps its 5 px bottom margin.
        if prev_query && prefix.is_none() {
            rows.push((div().h(px(5.)).flex_none().into_any_element(), prev_depth));
        }
        // Live (extrude-24, dom/dialog-fillet-partial `os-param-group-indent`):
        // each group level indents its params 20 px with a 1 px `#d7d7d7`
        // guide 8 px in, under the expander; on the group's last row the
        // guide ends in a 10 px foot (`node-indent-line-end`).
        let depths: Vec<usize> = rows.iter().map(|r| r.1).collect();
        rows.into_iter()
            .enumerate()
            .map(|(k, (row, depth))| {
                let next = depths.get(k + 1).copied().unwrap_or(0);
                (0..depth).rev().fold(row, |r, level| {
                    div()
                        .relative()
                        .pl(px(20.))
                        .child(div().absolute().left(px(8.)).top_0().bottom_0().w(px(1.)).bg(p().border))
                        .when(level >= next, |d| d.child(div().absolute().left(px(3.)).bottom_0().w(px(10.)).h(px(1.)).bg(p().border)))
                        .child(r)
                        .into_any_element()
                })
            })
            .collect()
    }

    /// Mate connector's axis row: the primary axis flip (its arrow shows the
    /// current direction) and the secondary axis button, which steps
    /// through the options.
    fn render_axis_buttons(&mut self, flip: &'static ParamSpec, path: &str, axis: Option<&'static ParamSpec>, values: &ParamValues, prefix: Option<(&'static str, usize)>, cx: &mut Context<Self>) -> AnyElement {
        let on = values.bool(flip.id);
        let fpath = path.to_string();
        let button = |id: String, icon: &'static str, tip: &'static str, cx: &mut Context<Self>| {
            div()
                .id(SharedString::from(id))
                .size(px(26.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .border_b_1()
                .border_color(hsla(0., 0., 0., 0.))
                .hover(|d| d.bg(gpui_kit::rgb(0xeaeaea)))
                // Live (dom dialog-*.txt): flip icons are 20 x 19 svgs, the sprite at 19 px.
                .child(crate::assets::icon(icon, 19., cx))
                .tooltip(crate::ui::tooltip(tip))
        };
        // Live: the flip toggle is a Tab stop (Space / Enter press it).
        let flip_focus = self.tab_stop(&format!("axisflip:{path}"), cx);
        let toggle = move |this: &mut Self, _: &mut Window, cx: &mut Context<Self>| {
            let cur = matches!(this.get_value(&fpath), Some(ParamValue::Bool(true)));
            this.set_value(&fpath, ParamValue::Bool(!cur), cx);
        };
        let toggle = std::rc::Rc::new(toggle);
        let t1 = toggle.clone();
        let flip_btn = button(format!("axisflip-{path}"), if on { "flip-direction" } else { "flip-direction-opposite" }, flip.label, cx).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| t1(this, window, cx)));
        let mut row = h_flex().ml(px(-2.)).h(px(28.)).items_end().child(Self::tab_stop_element(flip_btn, &flip_focus, Focus::Underline, move |this, window, cx| toggle(this, window, cx), cx));
        if let Some(a) = axis {
            let apath = match prefix {
                None => a.id.to_string(),
                Some((arr, idx)) => item_path(arr, idx, a.id),
            };
            let options: Vec<&'static str> = match &a.kind {
                ParamKind::Enum { options, .. } => options.iter().map(|(id, _)| *id).collect(),
                _ => Vec::new(),
            };
            let cur = values.enum_(a.id).to_string();
            let step = move |this: &mut Self, _: &mut Window, cx: &mut Context<Self>| {
                let at = options.iter().position(|o| *o == cur).unwrap_or(0);
                if let Some(next) = options.get((at + 1) % options.len().max(1)) {
                    this.set_value(&apath, ParamValue::Enum(next.to_string()), cx);
                }
            };
            let step = std::rc::Rc::new(step);
            let s1 = step.clone();
            let turn_focus = self.tab_stop(&format!("axisturn:{path}"), cx);
            let turn = button(format!("axisturn-{path}"), "realign-mate-dialog", a.label, cx).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| s1(this, window, cx)));
            row = row.child(Self::tab_stop_element(turn, &turn_focus, Focus::Underline, move |this, window, cx| step(this, window, cx), cx));
        }
        row.into_any_element()
    }

    /// A Tab stop for a dialog control. Onshape's Tab order (live
    /// behaviours.md "Keyboard, full Tab cycle"): faces list (focused on
    /// open) → End type select → flip → Depth → tolerance (±) → the
    /// checkboxes → Merge scope list → `?` help, then out of the dialog;
    /// the tab rows and ✓ / ✕ are skipped. Stops follow the paint (DOM) order.
    fn tab_stop(&mut self, key: &str, cx: &mut Context<Self>) -> FocusHandle {
        self.tab_stops.entry(key.to_string()).or_insert_with(|| cx.focus_handle().tab_stop(true)).clone()
    }

    /// Make `d` a Tab stop; Space (and Enter for buttons) activates it.
    /// Focus looks (bundle CSS): icon buttons (`.os-param-icon-button:focus`)
    /// get a 2 px `#1651b0` bottom line (1 px border + 1 px inset shadow);
    /// checkboxes draw it on their box (`os_checkbox_focused`, so `Focus::Box`
    /// adds nothing here); the `?` link gets a ring.
    fn tab_stop_element(d: Stateful<Div>, h: &FocusHandle, look: Focus, activate: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static, cx: &mut Context<Self>) -> Stateful<Div> {
        let enter = look != Focus::Box;
        let blue: Hsla = gpui_kit::rgb(0x1651b0).into();
        let d = d.track_focus(h);
        let d = match look {
            Focus::Underline => d.focus(move |s| s.shadow(vec![BoxShadow { color: blue, offset: point(px(0.), px(-2.)), blur_radius: px(0.), spread_radius: px(0.), inset: true }])),
            Focus::Ring => d.focus(move |s| s.shadow(vec![BoxShadow { color: blue, offset: point(px(0.), px(0.)), blur_radius: px(0.), spread_radius: px(1.5), inset: false }])),
            Focus::Box => d,
        };
        d.on_key_down(cx.listener(move |this, ev: &KeyDownEvent, window, cx| {
            let k = ev.keystroke.key.as_str();
            if k == "space" || (enter && k == "enter") {
                activate(this, window, cx);
                cx.stop_propagation();
            }
        }))
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
        let flips: Vec<AnyElement> = trailing
            .iter()
            .map(|f| {
                let fpath = match prefix {
                    None => f.id.to_string(),
                    Some((arr, idx)) => item_path(arr, idx, f.id),
                };
                let on = values.bool(f.id);
                let circular = f.hints.has(Hints::OPPOSITE_DIRECTION_CIRCULAR);
                // Live (dialogs/extrude-01, rib-01, draft-01): the button
                // shows the current direction (the arrow turns round), with
                // no pressed background. Unflipped, Onshape shows the art of
                // its *-opposite symbols (big arrow down-left; circular arrow
                // clockwise, dialogs/circular-pattern-01).
                let icon = match (circular, on) {
                    (true, false) => "flip-circular-opposite",
                    (true, true) => "flip-circular",
                    (false, false) => "flip-direction-opposite",
                    (false, true) => "flip-direction",
                };
                // A Tab stop like Onshape's `os-param-icon-button` (Space /
                // Enter press it; focused: the 2 px blue bottom line).
                let focus = self.tab_stop(&format!("flip:{fpath}"), cx);
                let (p1, p2) = (fpath.clone(), fpath.clone());
                let toggle = move |this: &mut Self, path: &str, cx: &mut Context<Self>| {
                    let cur = matches!(this.get_value(path), Some(ParamValue::Bool(true)));
                    this.set_value(path, ParamValue::Bool(!cur), cx);
                };
                let el = div()
                    .id(SharedString::from(format!("flip-{fpath}")))
                    .w(px(26.))
                    .h(px(26.))
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|d| d.bg(p().panel_hover))
                    // Live (dom dialog-*.txt): flip icons are 20 x 19 svgs, the sprite at 19 px.
                    .child(crate::assets::icon(icon, 19., cx))
                    .tooltip(crate::ui::tooltip(f.label))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| toggle(this, &p1, cx)));
                Self::tab_stop_element(el, &focus, Focus::Underline, move |this, _, cx| toggle(this, &p2, cx), cx).into_any_element()
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
                    .child(div().w(px(96.)).flex_none().text_xs().text_color(pal.text_muted).child(crate::ui::spec_text(s.label)))
                    .child(div().flex_1().child(e))
                    .os_context_menu(move |m, _, _| {
                        let v = view.clone();
                        let p = p2.clone();
                        m.item(crate::os_menu::OsMenuItem::new("Convert to checkbox").on_click(move |_, _, cx| {
                            v.update(cx, |this, cx| {
                                this.inputs.remove(&p);
                                this.parse_errors.remove(&p);
                                this.set_value(&p, ParamValue::Bool(false), cx);
                            })
                        }))
                    })
                    .into_any_element()
            }
            ParamKind::Bool { .. } if s.hints.has(Hints::GROUP_LABEL) => {
                // A plain collapsible group title (live variable.fs "Axis
                // measurements"): chevron + 12 px #666 name; click folds.
                let on = values.bool(s.id);
                let path_s = path.to_string();
                // Live (dom/dialog-sheet-metal-model.txt): the group name is
                // 13.3 px #666 (padding 5 4); it turns red (#aa252d, error
                // expander) when a field inside it is in error.
                let error = self.ty.params.iter().any(|q| {
                    let (mut open, mut off) = (values.clone(), values.clone());
                    open.set(s.id, ParamValue::Bool(true));
                    off.set(s.id, ParamValue::Bool(false));
                    q.is_visible(&open) && !q.is_visible(&off) && self.field_error(q.id).is_some()
                });
                let color = if error { gpui_kit::rgb(0xaa252d) } else { gpui_kit::rgb(0x666666) };
                h_flex()
                    .id(SharedString::from(format!("grp-{path}")))
                    .w_full()
                    .ml(px(-2.))
                    // Live: 17.33 px line + 5 px padding each side.
                    .min_h(px(self.live_h(LIVE_CHECKBOX_ROW)))
                    .cursor_pointer()
                    .child(div().w(px(20.)).flex_none().flex().justify_center().child(crate::ui::glyph(if on { "expander-down" } else { "expander-right" }, 14., if error { color } else { gpui_kit::rgb(0x555555) })))
                    .child(div().px(px(4.)).py(px(5.)).text_size(px(13.33)).line_height(px(17.33)).text_color(color).child(crate::ui::spec_text(s.label)))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_value(&path_s, ParamValue::Bool(!on), cx)))
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
                // Onshape (os-param-group-expander is-error =
                // getIsAnyParameterInErrorState): a ticked group whose shown
                // fields include one in error (Direction ticked, nothing
                // picked: live dialogs/extrude-23, #bd3039) has a red chevron;
                // otherwise #666 when ticked (extrude-22), #d7d7d7 when not.
                let error = header && on && {
                    let mut off = values.clone();
                    off.set(s.id, ParamValue::Bool(false));
                    self.ty.params.iter().any(|q| q.is_visible(values) && !q.is_visible(&off) && self.field_error(q.id).is_some())
                };
                let chevron = header.then(|| {
                    div()
                        .id(SharedString::from(format!("chev-{path}")))
                        .w(px(20.))
                        .flex_none()
                        .flex()
                        .justify_center()
                        .cursor_pointer()
                        .child(crate::ui::glyph(if on && !collapsed { "expander-down" } else { "expander-right" }, 14., if error { pal.error } else if on { gpui_kit::rgb(0x666666) } else { pal.border }))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if !this.collapsed_groups.remove(&path_c) {
                                this.collapsed_groups.insert(path_c.clone());
                            }
                            cx.notify();
                        }))
                });
                // Onshape: a 20 px expander column before group checkboxes; the
                // checkbox itself is inset 4 px.
                // Live (qa5 DER1, part-studio/400): the label clips 28 px
                // short of the dialog's right edge (174 px of text).
                let mut row = h_flex()
                    .w_full()
                    .ml(px(-2.))
                    .pr(px(12.))
                    // Live (dom/dialog-extrude-new): 27.33 px rows (17.33 px
                    // label line + 5 px padding each side).
                    .min_h(px(self.live_h(LIVE_CHECKBOX_ROW)))
                    .children(chevron)
                    .child({
                        let view = cx.entity();
                        let p2 = path.to_string();
                        let toggle = move |this: &mut Self, cx: &mut Context<Self>| {
                            // Checking a group unfolds it.
                            if header && !on {
                                this.collapsed_groups.remove(&path_s);
                            }
                            this.set_value(&path_s, ParamValue::Bool(!on), cx)
                        };
                        let toggle = std::rc::Rc::new(toggle);
                        let (t1, t2) = (toggle.clone(), toggle);
                        let focus = self.tab_stop(&format!("cb:{path}"), cx);
                        let bool_unconfigure = (prefix.is_none() && self.configured.contains(s.id)).then(|| s.id.to_string());
                        let cb = crate::ui::os_checkbox_focused(SharedString::from(format!("cb-{path}")), "", on, focus.is_focused(window))
                            // Live (psb 10-20 l, part-studio/402): a long label stays
                            // on one line, clipped with an ellipsis ("Preserve active
                            // sheet meta…"), no tooltip.
                            .child(div().flex_1().min_w_0().ml(px(-4.)).truncate().child(crate::ui::spec_text(s.label)))
                            .rounded(px(2.))
                            .flex_shrink(1.)
                            .min_w_0()
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| t1(this, cx)));
                        div()
                            .id(SharedString::from(format!("cbw-{path}")))
                            .min_w_0()
                            .child(Self::tab_stop_element(cb, &focus, Focus::Box, move |this, _, cx| t2(this, cx), cx))
                            // Right-click > Convert to expression (e.g. `#Length > 3`),
                            // and Unconfigure when configured (live psb 10-21 g).
                            .os_context_menu(move |m, _, _| {
                                let v = view.clone();
                                let p = p2.clone();
                                let m = m.item(crate::os_menu::OsMenuItem::new("Convert to expression").on_click(move |_, window, cx| {
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
                                }));
                                match &bool_unconfigure {
                                    Some(pid) => configure_items(m, &view, pid, true),
                                    None => m,
                                }
                            })
                    });
                if let Some(c) = companion {
                    let cpath = match prefix {
                        None => c.id.to_string(),
                        Some((arr, idx)) => item_path(arr, idx, c.id),
                    };
                    if let Some(e) = self.render_inline_value(c, &cpath, values, window, cx) {
                        // Live (extrude-21 "Draft  3 deg ↻"): the value runs to
                        // the 194 px column like a quantity row; its flip sits
                        // in the 26 px right column.
                        row = row.pr(px(if flips.is_empty() { 26. } else { 0. })).child(div().flex_1().child(e));
                    }
                }
                row.children(flips).into_any_element()
            }
            ParamKind::Enum { options, .. } => {
                let cur = values.enum_(s.id).to_string();
                if s.hints.has(Hints::HORIZONTAL_ENUM) {
                    // Onshape underline tabs (Solid / Surface / Thin, New / Add / ...).
                    let group = h_flex().w_full().flex_wrap().children(options.iter().map(|(id, label)| {
                        let selected = cur == *id;
                        let path_s = path.to_string();
                        let id_s = id.to_string();
                        // Onshape (live DOM, behaviours "ui2 (10-24 09:10)"): items
                        // `flex: 1 0 auto` with 2 px padding, so each is its text +
                        // 4 px plus an equal share of the slack; a row too wide
                        // wraps, a lone item too wide ellipsizes. 12 px text, the
                        // selected one 500 blue with a 2 px blue underline, the
                        // others a 1 px #d7d7d7 underline.
                        div()
                            .id(SharedString::from(format!("seg-{path}-{id}")))
                            .flex_grow(1.)
                            .flex_shrink_0()
                            .min_w_0()
                            .max_w_full()
                            .overflow_hidden()
                            .px(px(2.))
                            .h(px(26.))
                            .relative()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(12.))
                            .cursor_pointer()
                            .when(selected, |d| d.text_color(pal.emphasis).font_weight(FontWeight::MEDIUM).child(div().absolute().left_0().right_0().bottom_0().h(px(2.)).bg(pal.accent)))
                            .when(!selected, |d| d.text_color(pal.text).child(div().absolute().left_0().right_0().bottom_0().h(px(1.)).bg(pal.border)).hover(|d| d.text_color(pal.text)))
                            .child(div().min_w_0().truncate().child(crate::ui::spec_text(label)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_value(&path_s, ParamValue::Enum(id_s.clone()), cx)))
                    }));
                    h_flex().mx(px(-2.)).mb(px(2.)).child(group).children(flips).into_any_element()
                } else {
                    let path_s = path.to_string();
                    let items = options.iter().map(|(id, l)| (SharedString::from(*id), SharedString::from(*l))).collect();
                    // Live: Tab skips the dropdowns (Shift+Tab from Depth lands on the flip button).
                    let dd = self.os_select(path, items, &cur, move |this, id, cx| this.set_value(&path_s, ParamValue::Enum(id.to_string()), cx), cx);
                    // Live (div.select-input tip=…): the select's tooltip is its label.
                    // Live (dom/dialog-hole.txt "Tip angle"): next to a 26 px icon
                    // the select sits 3 px down in a 31 px row.
                    let icon_row = s.hints.has(Hints::SHOW_LABEL) && param_icon(self.ty.id, s.id).is_some();
                    let dd = div().id(SharedString::from(format!("sel-{path}"))).flex_1().min_w_0().when(icon_row, |d| d.mt(px(3.))).tooltip(crate::ui::user_tooltip(crate::i18n::spec(s.label))).child(dd);
                    // Live (dom/dialog-extrude-new.txt): the select fills the dialog's
                    // first 194 px column flush with its left edge (the body has 2 px
                    // padding); flips sit in the 26 px right column.
                    // Live: a 28 px row, the 26 px select on its top line.
                    let mut row = h_flex().mx(px(-2.)).min_h(px(if icon_row { 31. } else { 28. })).items_start().when(flips.is_empty(), |d| d.pr(px(26.)));
                    // Live (dom/dialog-chamfer.txt): label and select wrap like
                    // inline boxes. When the label (12 px, 5 px padding) and
                    // the current value (14 px, 6 + 20 px padding) don't fit
                    // the 194 px column, the label takes its own row and the
                    // select the full width ("Chamfer type" / "Equal distance").
                    let cur_label = options.iter().find(|(id, _)| *id == cur.as_str()).map(|(_, l)| *l).unwrap_or("");
                    // The select is its value plus 22 px (padding, caret) and at
                    // least ~90 px (live: Loft's "End profile condition / None" and
                    // "Chamfer type / Equal distance" wrap; Hole's "Start plane /
                    // Start from part" and Helix's "Start condition / Start angle"
                    // don't).
                    let fits = 10.0 + crate::ui::text_width(window, &crate::i18n::spec(s.label), 12.0) + (22.0 + crate::ui::text_width(window, &crate::i18n::spec(cur_label), 14.0)).max(90.0) <= 194.0;
                    if s.hints.has(Hints::SHOW_LABEL) && param_icon(self.ty.id, s.id).is_none() && !fits {
                        return Some(
                            v_flex()
                                .w_full()
                                .child(div().mx(px(-2.)).px(px(5.)).pt(px(7.)).text_size(px(12.)).line_height(px(16.)).text_color(pal.text_muted).child(crate::ui::spec_text(s.label)))
                                // Live: label 23 px + the select's 25 px box (48; the
                                // 26 px select overflows it by 1).
                                .child(row.min_h(px(25.)).h(px(25.)).child(dd).children(flips))
                                .into_any_element(),
                        );
                    }
                    if s.hints.has(Hints::SHOW_LABEL) {
                        // Live: "Measurement ▾ Radius": the label 12 px #666 with 5 px
                        // padding, the select takes the rest of the column.
                        row = row.child(match param_icon(self.ty.id, s.id) {
                            Some(icon) => div().flex_none().child(crate::assets::icon(icon, 26., cx)).into_any_element(),
                            // Live: the label's baseline is the value's (label 7 px down).
                            None => div().px(px(5.)).pt(px(7.)).max_w(px(110.)).flex_none().text_size(px(12.)).text_color(pal.text_muted).truncate().child(crate::ui::spec_text(s.label)).into_any_element(),
                        });
                    }
                    row.child(dd).children(flips).into_any_element()
                }
            }
            ParamKind::Text { .. } if s.id == "partStudio" && self.ty.id == "importDerived" => self.render_derived_picker(cx),
            ParamKind::Num(_) | ParamKind::Text { .. } => {
                let e = self.render_inline_value(s, path, values, window, cx)?;
                // Tolerance options on tolerant quantities (Onshape ± icon).
                let tolerant = matches!(s.kind, ParamKind::Num(_)) && s.hints.has(Hints::CAN_BE_TOLERANT) && !path.contains('.');
                // Live (dom/dialog-hole.txt): a row with a 26 px icon for its
                // label is 29 px and its controls sit 1 px lower.
                let icon_dy = if param_icon(self.ty.id, s.id).is_some() { 1. } else { 0. };
                let tol_key = format!("{path}__tolerance");
                let tol = tolerant.then(|| Tolerance::parse(self.get_value(&tol_key))).flatten();
                let tol_focus = if tolerant { Some(self.tab_stop(&format!("tol:{path}"), cx)) } else { None };
                let on = tol.is_some();
                let press = move |this: &mut Self, key: &str, cx: &mut Context<Self>| {
                    let next = if on { None } else { Some(Tolerance::default()) };
                    this.tol_open.remove(key);
                    this.set_tolerance(key, next, cx);
                };
                let key2 = tol_key.clone();
                let tol_btn = tolerant.then(|| {
                    let key = tol_key.clone();
                    // Onshape (live, dom/dialog-hole-tolerance.txt): a 20 px
                    // "Tolerance options" toggle 5 px after the field, pressed
                    // `#eaeaea`; turning it on enables the row's chevron (the
                    // options stay collapsed until it is clicked).
                    div()
                        .id(SharedString::from(format!("tolbtn-{path}")))
                        // Live (extrude-01): the icon's ink 8 px under the row top.
                        .mt(px(6. + icon_dy))
                        .ml(px(5.))
                        // Live (dom/dialog-extrude-new): 5 px after the toggle
                        // before the 26 px right column.
                        .mr(px(4.))
                        .size(px(20.))
                        .flex_none()
                        .rounded(px(3.))
                        .cursor_pointer()
                        .when(on, |d| d.bg(gpui_kit::rgb(0xeaeaea)))
                        .when(!on, |d| d.hover(|d| d.bg(pal.panel_hover)))
                        .child(crate::assets::icon("tolerance", 20., cx))
                        .tooltip(crate::ui::tooltip("Tolerance options"))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| press(this, &key, cx)))
                });
                let tol_btn = tol_btn.zip(tol_focus).map(|(b, f)| Self::tab_stop_element(b, &f, Focus::Underline, move |this, _, cx| press(this, &key2, cx), cx));
                // Onshape (live DOM): tolerant quantities start with a 20 px
                // expander: `›` `#d7d7d7` and inert without tolerances, `#666`
                // and clickable with them, `⌄` while the options show; the
                // label is 12 px `#666` with 5 px padding, the field fills the
                // rest of the first column.
                let tol_shown = tol.is_some() && self.tol_open.contains(&tol_key);
                let expander = tolerant.then(|| {
                    let enabled = tol.is_some();
                    let key = tol_key.clone();
                    div()
                        .id(SharedString::from(format!("tolexp-{path}")))
                        .w(px(20.))
                        .h(px(26.))
                        // Live (extrude-01): the › 1 px under the row's middle.
                        .pt(px(1. + icon_dy))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(crate::ui::glyph(if tol_shown { "expander-down" } else { "expander-right" }, 14., if enabled { gpui_kit::rgb(0x666666) } else { pal.border }))
                        .when(enabled, |d| {
                            d.cursor_pointer().on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                if !this.tol_open.remove(&key) {
                                    this.tol_open.insert(key.clone());
                                }
                                cx.notify();
                            }))
                        })
                });
                let diameter = s.hints.has(Hints::CAN_BE_TOLERANT_DIAMETER);
                // Live (dom/dialog-variable, dialog-thicken, dialog-extrude-new):
                // the row starts at the dialog edge (expander, then the label
                // with 5 px padding each side) and the field runs from the
                // label to 1 px past the 194 px column.
                let row = h_flex()
                    .mr(px(-1.))
                    .ml(px(-2.))
                    .min_h(px(26. + 3. * icon_dy))
                    .items_start()
                    .children(expander)
                    // Live (dom/dialog-hole.txt): hole sizes show a 26 px icon
                    // instead of their label.
                    .child(match param_icon(self.ty.id, s.id) {
                        Some(icon) => div().flex_none().child(crate::assets::icon(icon, 26., cx)).into_any_element(),
                        None => div().pl(px(5.)).pr(px(5.)).pt(px(6.)).max_w(px(125.)).flex_none().text_size(px(12.)).text_color(pal.text_muted).truncate().child(crate::ui::spec_text(s.label)).into_any_element(),
                    })
                    // Live: the value's baseline 1 px under the label's, the
                    // underline on the row's bottom line.
                    // (Rows are 26 px like live: tolerance rows are set by
                    // their 26 px expander, the others by the field.)
                    .child(div().flex_1().min_w_0().pt(px(4. + icon_dy)).child(e))
                    .children(tol_btn)
                    // The first column is 194 px; flips sit in the 26 px right one.
                    .when(flips.is_empty(), |d| d.pr(px(26.)))
                    .children(flips);
                match tol {
                    Some(t) if tol_shown => v_flex().w_full().child(row).child(self.render_tolerance(&tol_key, t, diameter, window, cx)).into_any_element(),
                    _ => row.into_any_element(),
                }
            }
            ParamKind::Query { max_picks, filter } => {
                // Live (dom/dialog-*-pattern, mirror, transform): fields that
                // pick parts take existing mate connectors but have no button.
                let takes_mc = filter.intersects(QueryFilter::MATE_CONNECTOR) && !filter.intersects(QueryFilter::SOLID_BODY) && !s.hints.has(Hints::NO_MATE_CONNECTOR_BUTTON);
                let refs = values.query(s.id).to_vec();
                let cs_open = self.cs_target.as_deref() == Some(path);
                let active = self.active_query.as_deref() == Some(path) && !cs_open;
                let takes_cs = s.hints.has(Hints::SHOW_CREATE_SELECTION);
                let path_s = path.to_string();
                let any_missing = refs.iter().any(|r| (self.missing)(r));
                // Right-click on a pick: Configure (live psb 10-21 g) after
                // Clear all, with a configuration input.
                let pick_configure = (self.configurable_inputs && !self.configure_mode && prefix.is_none() && !s.hints.has(Hints::UNCONFIGURABLE)).then(|| (s.id.to_string(), self.configured.contains(s.id)));
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
                    if let Reference::SketchEntity { sketch, .. } = r {
                        let vertices = (self.sketch_vertices)(*sketch);
                        let all: Vec<usize> = refs.iter().enumerate().filter(|(_, x)| vertices.contains(x)).map(|(i, _)| i).collect();
                        if groups_vertices(self.grouped_sketches.contains(sketch), vertices.len(), all.len()) && all.contains(&k) {
                            if all[0] == k {
                                rows.push((format!("Vertices of {}", (self.labeler)(&Reference::Sketch(*sketch))), all, Reference::Sketch(*sketch)));
                            }
                            continue;
                        }
                    }
                    // A whole sketch in a curve field (its row clicked) reads
                    // "Edges of Sketch 2" (live Rib: "Edges of Rib sketch").
                    if let (Reference::Sketch(_), ParamKind::Query { filter, .. }) = (r, &s.kind)
                        && filter.intersects(QueryFilter::SKETCH_EDGE)
                        && !filter.intersects(QueryFilter::SKETCH)
                    {
                        rows.push((format!("Edges of {}", (self.labeler)(r)), vec![k], r.clone()));
                        continue;
                    }
                    rows.push(((self.labeler)(r), vec![k], r.clone()));
                }
                // Lists that may be ordered (`ALLOW_QUERY_ORDER`) with 2+ rows get
                // the reorder button; in the mode × gives way to the grip.
                let can_reorder = s.hints.has(Hints::ALLOW_QUERY_ORDER) && rows.len() >= 2;
                if !can_reorder {
                    self.reordering.remove(path);
                }
                let reorder_on = can_reorder && self.reordering.contains(path);
                if !cx.has_active_drag() && self.pick_drop.is_some() {
                    self.pick_drop = None;
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
                        // Onshape selection rows: 24 px plain text, the entity name as
                        // tooltip; every second row under a 35 % white overlay
                        // (woolsthorpe.css `.os-param-selection-list-entry:nth-of-type(2n)`;
                        // no hover background).
                        h_flex()
                            .id(SharedString::from(format!("chip-{path}-{row}")))
                            .on_hover(cx.listener(move |_, h: &bool, _, cx| cx.emit(DialogEvent::HoverRef(h.then(|| hover_ref.clone())))))
                            // Chips name entities: user text in every language.
                            .tooltip(crate::ui::user_tooltip(tip))
                            .w_full()
                            .h(px(24.))
                            .pl(px(6.))
                            .pr(px(4.))
                            .when(row % 2 == 1, |d| d.bg(with_alpha(gpui_kit::rgb(0xffffff), 0.35)))
                            .text_size(px(13.33))
                            .text_color(if missing { pal.error } else if active { gpui_kit::rgb(0x555555) } else { pal.text_muted })
                            .child(div().flex_1().truncate().child(crate::ui::entity_text(label)))
                            .when(reorder_on && single, |d| d.child(self.reorder_grip(path, k, &label_for_drag, false)))
                            .when(!reorder_on, |d| d.child(
                                div()
                                    .id(SharedString::from(format!("chipx-{path}-{row}")))
                                    // Live (woolsthorpe.css .os-param-selection-list-entry-delete:
                                    // a 20 px cell, 5 px padding, 5 px in): the ink 12 px from
                                    // the field's border (extrude-03, sketch/01).
                                    .w(px(20.))
                                    .h(px(24.))
                                    .mr(px(1.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    // Live (dialogs/extrude-13, fillet-02): the × is always shown.
                                    .group("field-x")
                                    .cursor_pointer()
                                    // Live (fillet-04, extrude-03): centred in a 24 px box
                                    // (ink 12–19 px in from the row's right end), 1 px
                                    // under the row's middle.
                                    .w(px(24.))
                                    .pl(px(2.))
                                    .flex()
                                    .justify_center()
                                    .relative()
                                    .top(px(1.))
                                    // Live (dialogs/extrude-03): Onshape's × glyph, 8 x 7 px ink, #666.
                                    .child(crate::ui::glyph("ui-clear", 8., crate::theme::p().icon_tertiary).group_hover("field-x", |s| s.text_color(pal.error)))
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
                                        this.changed(cx);
                                        cx.notify();
                                    })),
                            ))
                            // Reorder mode: rows take dropped rows (by their grip).
                            .map(|d| if reorder_on && single { self.reorder_target(d, path, k, cx) } else { d })
                            .os_context_menu({
                                // Onshape: right-click a pick > Clear all (and
                                // Clear missing references when some are).
                                let view = cx.entity();
                                let p3 = path_s.clone();
                                let pick_configure = pick_configure.clone();
                                move |menu, _, _| {
                                    let v2 = view.clone();
                                    let pb = p3.clone();
                                    let mut m = menu.item(crate::os_menu::OsMenuItem::new("Clear all").on_click(move |_, _, cx| {
                                        v2.update(cx, |this, cx| {
                                            this.set_value(&pb, ParamValue::Query(Vec::new()), cx);
                                            cx.emit(DialogEvent::HoverRef(None));
                                        })
                                    }));
                                    if any_missing {
                                        let v = view.clone();
                                        let pc = p3.clone();
                                        m = m.item(crate::os_menu::OsMenuItem::new("Clear missing references").on_click(move |_, _, cx| {
                                            v.update(cx, |this, cx| {
                                                let missing = this.missing.clone();
                                                if let Some((vals, id)) = this.values_for_path_mut(&pc) {
                                                    vals.query_mut(&id).retain(|r| !missing(r));
                                                }
                                                this.user_set.insert(pc.clone());
                                                this.changed(cx);
                                                cx.notify();
                                            })
                                        }));
                                    }
                                    match &pick_configure {
                                        Some((pid, on)) => configure_items(m, &view, pid, *on),
                                        None => m,
                                    }
                                }
                            })
                            .into_any_element()
                    })
                    .collect();
                // Onshape: the field label is the placeholder inside the box; once
                // entities are picked it shrinks to the top-left corner (§3.9).
                let empty = refs.is_empty();
                // Live (dialogs/*-error): a field the error points at is
                // pink with a red border; hovering it shows the message.
                let error = if active || cs_open { None } else { self.field_error(s.id) };
                let caption = div()
                    .px(px(6.))
                    // Live (dom/dialog-extrude-new): the empty field is 26 px
                    // with its border (label 24 px), #555 while active, #666
                    // otherwise (dom/dialog-circular-pattern "Axis of pattern").
                    .when(empty, |d| d.pt(px(7.)).pb(px(4.)).text_size(px(12.)).line_height(px(13.)).text_color(gpui_kit::rgb(if active { 0x555555 } else { 0x666666 })))
                    .when(!empty, |d| d.pt(px(7.)).text_size(px(10.)).line_height(px(9.)).text_color(pal.text_muted))
                    .truncate()
                    .child(crate::ui::spec_text(s.label));
                // Live (dom/dialog-fillet.txt): fields are 3 px in from the dialog
                // edges with 5 px above (`os-param-requires-margin`); the row
                // after the last field also starts 5 px below it (render_params).
                // Live (psb 10-19 h): selection lists are Tab stops, and
                // focusing one makes it the active field (it stays active when
                // the focus moves on).
                let q_focus = self.tab_stop(&format!("q:{path}"), cx);
                if q_focus.is_focused(window) && self.active_query.as_deref() != Some(path) {
                    self.active_query = Some(path.to_string());
                    cx.emit(DialogEvent::QueryFocused);
                    cx.notify();
                }
                let q_focus2 = q_focus.clone();
                h_flex()
                    .w_full()
                    .items_start()
                    .px(px(1.))
                    .pt(px(5.))
                    .child(
                        div()
                            .id(SharedString::from(format!("q-{path}")))
                            .track_focus(&q_focus)
                            .flex_1()
                            .min_w_0()
                            // Live: empty = 13.33 px label line + 6/5 px padding + border.
                            // A filled field (its caption line) carries the same
                            // fraction (extrude-03: the rows after it step as
                            // after an empty one).
                            .min_h(px({
                                let h = self.live_h(LIVE_EMPTY_FIELD);
                                if empty { h } else { 26. }
                            }))
                            .max_h(px(160.))
                            .overflow_y_scroll()
                            .rounded(px(2.))
                            .border_1()
                            .border_color(if error.is_some() { pal.error } else if active || cs_open { pal.accent } else { pal.border })
                            .bg(if active { pal.query_active } else if error.is_some() { gpui_kit::rgb(0xf9ebec) } else { pal.panel })
                            // A parameter with a description shows Onshape's
                            // expanding tooltip (live Derived › Locations: the
                            // label, then "Select or create mate connectors…").
                            .map(|d| match (&error, s.description.is_empty()) {
                                (None, false) => {
                                    let anchor: std::rc::Rc<std::cell::Cell<Bounds<Pixels>>> = Default::default();
                                    let a2 = anchor.clone();
                                    d.child(canvas(move |b, _, _| a2.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full()).tooltip(crate::ui::rich_tooltip(s.label, s.description, anchor))
                                }
                                _ => d.tooltip(crate::ui::tooltip(error.clone().unwrap_or_else(|| s.label.to_string()))),
                            })
                            .cursor_pointer()
                            .flex()
                            .flex_col()
                            .relative()
                            .child(caption)
                            .children(chips)
                            .when(can_reorder, |d| d.child(self.reorder_button(path, cx)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                this.active_query = Some(path_s.clone());
                                window.focus(&q_focus2, cx);
                                cx.emit(DialogEvent::QueryFocused);
                                cx.notify();
                            })),
                        // Right-click > Clear all lives on the pick rows (a
                        // field-level menu would open a second menu).
                    )
                    // Onshape: fields that accept a mate connector have a 26 px
                    // "Select mate connector" button at their right.
                    .when(takes_mc, |d| {
                        let p_mc = path.to_string();
                        d.child(
                            div()
                                .id(SharedString::from(format!("mc-{path}")))
                                .size(px(26.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(3.))
                                .cursor_pointer()
                                .hover(|d| d.bg(pal.panel_hover))
                                .child(crate::assets::icon("mc-select", 20., cx))
                                // Live (dom/dialog-draft.txt, dialog-hole.txt).
                                .tooltip(crate::ui::tooltip(if *max_picks == Some(1) { "Select mate connector (ctrl+m)" } else { "Select mate connectors (ctrl+m)" }))
                                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    this.active_query = Some(p_mc.clone());
                                    window.focus(&this.focus, cx);
                                    cx.emit(DialogEvent::QueryFocused);
                                    cx.notify();
                                })),
                        )
                    })
                    // Live (dom/delete-face-field-with-button.txt): a 26 px
                    // "Create selection" button (20 px icon, 3 px padding).
                    .when(takes_cs, |d| {
                        let p_cs = path.to_string();
                        d.child(
                            div()
                                .id(SharedString::from(format!("cs-{path}")))
                                .size(px(26.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .border_b_1()
                                .border_color(hsla(0., 0., 0., 0.))
                                .cursor_pointer()
                                .when(cs_open, |d| d.bg(gpui_kit::rgb(0xeaeaea)))
                                .hover(|d| d.bg(gpui_kit::rgb(0xeaeaea)).border_color(pal.accent))
                                .child(crate::assets::icon("create-selection", 20., cx))
                                .tooltip(crate::ui::tooltip("Create selection"))
                                .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    this.active_query = Some(p_cs.clone());
                                    window.focus(&this.focus, cx);
                                    cx.emit(DialogEvent::CreateSelection(p_cs.clone()));
                                    cx.notify();
                                })),
                        )
                    })
                    .children(flips)
                    .into_any_element()
            }
            ParamKind::Array { item, item_label } => {
                let items = values.array(s.id).len();
                let arr_id = s.id;
                let item_specs: &'static [ParamSpec] = item_slice(item);
                let remove = move |k: usize| {
                    move |this: &mut Self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>| {
                        if let Some(ParamValue::Array(a)) = this.feature.params.0.get_mut(arr_id)
                            && k < a.len()
                        {
                            a.remove(k);
                        }
                        this.inputs.retain(|p, _| !p.starts_with(&format!("{arr_id}.")));
                        this.changed(cx);
                        cx.notify();
                    }
                };
                // ALLOW_ARRAY_FOCUS (live p1audit skip-04): the same field, its
                // items' values never shown, a CLEAR button in its header.
                let focus_only = s.hints.has(Hints::ALLOW_ARRAY_FOCUS);
                if s.hints.has(Hints::DRIVEN_QUERY) || focus_only {
                    // Live (dialogs/fillet-12): one selection field ("Vertices",
                    // active blue); each pick is a row "Vertex of Extrude 1 ×"
                    // and its item's values follow under it, indented.
                    let q = item_specs.iter().find(|p| matches!(p.kind, ParamKind::Query { .. }));
                    let active = self.active_query.as_deref() == Some(path);
                    // An opened entry's own field active: the array's border
                    // stays blue around the white field (live psui-loft-01).
                    let inner_active = self.active_query.as_deref().is_some_and(|a| a.starts_with(&format!("{path}.")));
                    let mut field = v_flex()
                        .id(SharedString::from(format!("q-{path}")))
                        .flex_1()
                        .min_h(px({
                            let h = self.live_h(LIVE_EMPTY_FIELD);
                            if items == 0 { h } else { 26. }
                        }))
                        .rounded(px(2.))
                        .border_1()
                        .border_color(if active || inner_active { pal.accent } else { pal.border })
                        .bg(if active { pal.query_active } else { pal.panel })
                        .cursor_pointer()
                        .child(div().px(px(6.)).when(items == 0, |d| d.pt(px(6.)).pb(px(5.)).text_size(px(12.)).line_height(px(13.33))).when(items > 0, |d| d.pt(px(7.)).text_size(px(10.)).line_height(px(9.))).text_color(pal.text_muted).child(crate::ui::spec_text(s.label)));
                    let mut details: Vec<AnyElement> = Vec::new();
                    // Arrays may be reordered (live: unless PREVENT_ARRAY_REORDER)
                    // once they have 2+ items: the reorder button and mode.
                    let can_reorder = items >= 2 && !s.hints.has(Hints::PREVENT_ARRAY_REORDER);
                    if !can_reorder {
                        self.reordering.remove(arr_id);
                    }
                    let reorder_on = can_reorder && self.reordering.contains(arr_id);
                    // COLLAPSE_ARRAY_ITEMS (live psui-loft-00..03, Loft's Profiles
                    // and Guides): each entry has a › expander and nothing follows
                    // under the field; an opened entry shows its fields inside the
                    // field under its row (its query field then the active one);
                    // a 1 px rgba(0,0,0,0.2) divider between entries.
                    let collapse = s.hints.has(Hints::COLLAPSE_ARRAY_ITEMS);
                    for k in 0..items {
                        let r = q.and_then(|q| values.array(arr_id).get(k).and_then(|it| it.query(q.id).first().cloned()));
                        let label = match (s.item_template, values.array(arr_id).get(k)) {
                            (Some(t), Some(it)) => self.item_label(t, it, item_specs, k),
                            _ => r.as_ref().map(|r| (self.labeler)(r)).unwrap_or_default(),
                        };
                        let item_key = format!("{arr_id}.{k}");
                        let open = collapse && self.open_items.contains(&item_key);
                        let expander = collapse.then(|| {
                            let q_id = q.map(|q| q.id);
                            div()
                                .id(SharedString::from(format!("dqe-{path}-{k}")))
                                .size(px(20.))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .child(crate::ui::glyph(if open { "expander-down" } else { "expander-right" }, 14., gpui_kit::rgb(0x555555)))
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    cx.stop_propagation();
                                    if !this.open_items.remove(&item_key) {
                                        this.open_items.insert(item_key.clone());
                                        if let Some(q) = q_id {
                                            this.active_query = Some(item_path(arr_id, k, q));
                                        }
                                    }
                                    cx.notify();
                                }))
                        });
                        // Live (dom/psui-loft-entries.json): 24 px entries 30 px apart.
                        if collapse {
                            if k > 0 {
                                field = field.child(div().h(px(1.)).bg(gpui_kit::rgba(0x00000033)));
                            }
                            field = field.child(div().h(px(5.)));
                        }
                        let row = h_flex()
                            .id(SharedString::from(format!("dq-{path}-{k}")))
                            .h(px(24.))
                            .pl(px(6.))
                            .pr(px(4.))
                            .text_size(px(13.33))
                            .text_color(if active { gpui_kit::rgb(0x555555) } else { pal.text_muted })
                            .children(expander)
                            .child(div().flex_1().truncate().child(label.clone()))
                            .when(reorder_on, |d| d.child(self.reorder_grip(arr_id, k, &label, true)))
                            .when(!reorder_on, |d| {
                                d.child(
                                    div()
                                        .id(SharedString::from(format!("dqx-{path}-{k}")))
                                    // Live (woolsthorpe.css .os-param-selection-list-entry-delete:
                                    // a 20 px cell, 5 px padding, 5 px in): the ink 12 px from
                                    // the field's border (extrude-03, sketch/01).
                                    .w(px(20.))
                                    .h(px(24.))
                                    .mr(px(1.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                        .cursor_pointer()
                                        .group("field-x")
                                        // As a selection row's × (fillet-04).
                                        .w(px(24.))
                                        .pl(px(2.))
                                        .flex()
                                        .justify_center()
                                        .relative()
                                        .top(px(1.))
                                        // Live (dialogs/extrude-03): Onshape's × glyph, 8 x 7 px ink, #666.
                                        .child(crate::ui::glyph("ui-clear", 8., crate::theme::p().icon_tertiary).group_hover("field-x", |s| s.text_color(pal.error)))
                                        .on_click(cx.listener(remove(k))),
                                )
                            });
                        field = field.child(if reorder_on { self.reorder_target(row, arr_id, k, cx) } else { row });
                        if focus_only {
                            continue;
                        }
                        if collapse {
                            if open {
                                let inner = self.render_params(item_specs, Some((arr_id, k)), window, cx);
                                // Clicks inside stay with the entry's own fields.
                                field = field.child(v_flex().id(SharedString::from(format!("dqo-{path}-{k}"))).pl(px(20.)).pr(px(4.)).pb(px(4.)).children(inner).on_click(|_, _, cx| cx.stop_propagation()));
                            }
                            continue;
                        }
                        let inner = self.render_params(item_specs, Some((arr_id, k)), window, cx);
                        details.push(
                            v_flex()
                                .relative()
                                .pl(px(24.))
                                .child(div().absolute().left(px(15.)).top_0().bottom_0().w(px(1.)).bg(pal.border))
                                .child(div().h(px(24.)).flex().items_center().text_size(px(13.33)).text_color(pal.text_muted).truncate().child(label))
                                .children(inner)
                                .into_any_element(),
                        );
                    }
                    let path_s = path.to_string();
                    // Live (os-param-array-clear-button): "CLEAR", 10.5 px
                    // semibold black at 50 %, a 34 x 24 button 5 px in from
                    // the header's right; "Clear all items".
                    let clear = (focus_only && items > 0).then(|| {
                        div()
                            .id(SharedString::from(format!("clr-{path}")))
                            .absolute()
                            .top_0()
                            .right(px(4.))
                            .w(px(34.))
                            .h(px(24.))
                            .pt(px(4.))
                            .flex()
                            .justify_center()
                            .text_size(px(10.5))
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .text_color(gpui_kit::rgba(0x00000080))
                            .cursor_pointer()
                            .child("CLEAR")
                            .tooltip(crate::ui::tooltip("Clear all items"))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.feature.params.set(arr_id, ParamValue::Array(Vec::new()));
                                this.inputs.retain(|p, _| !p.starts_with(&format!("{arr_id}.")));
                                this.changed(cx);
                                cx.notify();
                            }))
                    });
                    let field = field.relative().children(clear).when(can_reorder, |d| d.child(self.reorder_button(arr_id, cx))).on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.active_query = Some(path_s.clone());
                        window.focus(&this.focus, cx);
                        cx.emit(DialogEvent::QueryFocused);
                        cx.notify();
                    }));
                    let cs = s.hints.has(Hints::SHOW_CREATE_SELECTION).then(|| {
                        let p_cs = path.to_string();
                        div()
                            .id(SharedString::from(format!("cs-{path}")))
                            .size(px(26.))
                            .flex_none()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .when(self.cs_target.as_deref() == Some(path), |d| d.bg(gpui_kit::rgb(0xeaeaea)))
                            .hover(|d| d.bg(gpui_kit::rgb(0xeaeaea)))
                            .child(crate::assets::icon("create-selection", 20., cx))
                            .tooltip(crate::ui::tooltip("Create selection"))
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                this.active_query = Some(p_cs.clone());
                                window.focus(&this.focus, cx);
                                cx.emit(DialogEvent::CreateSelection(p_cs.clone()));
                                cx.notify();
                            }))
                    });
                    return Some(v_flex().w_full().child(h_flex().w_full().items_start().gap(px(2.)).px(px(1.)).pt(px(5.)).child(field).children(cs)).children(details).into_any_element());
                }
                // Live (dialogs/fillet-12): a bordered box captioned with the
                // array's name, its items, and a full-width "Add <item>" button.
                let mut col = v_flex()
                    .mx(px(1.))
                    .mt(px(5.))
                    .mb(px(5.))
                    .p(px(4.))
                    .gap(px(4.))
                    .rounded(px(2.))
                    .border_1()
                    .border_color(pal.border)
                    .child(div().px(px(2.)).text_size(px(12.)).text_color(gpui_kit::rgb(0x555555)).child(crate::ui::spec_text(s.label)));
                for k in 0..items {
                    let inner = self.render_params(item_specs, Some((arr_id, k)), window, cx);
                    col = col.child(
                        v_flex()
                            .relative()
                            .pl(px(6.))
                            .border_l_1()
                            .border_color(pal.border)
                            .child(
                                h_flex()
                                    .justify_between()
                                    .child(div().text_size(px(13.33)).text_color(pal.text_muted).child(match (s.item_template, values.array(arr_id).get(k)) {
                                        (Some(t), Some(it)) => self.item_label(t, it, item_specs, k),
                                        _ => format!("{} {}", capitalize(item_label), k + 1),
                                    }))
                                    .child(
                                        div()
                                            .id(SharedString::from(format!("rm-{path}-{k}")))
                                            .cursor_pointer()
                                            .group("field-x")
                                            // Live (dialogs/extrude-03): Onshape's × glyph, 7 px ink, #666.
                                            .child(crate::ui::glyph("ui-clear", 9., crate::theme::p().icon_tertiary).group_hover("field-x", |s| s.text_color(pal.error)))
                                            .on_click(cx.listener(remove(k))),
                                    ),
                            )
                            .children(inner),
                    );
                }
                col = col.child(
                    div()
                        .id(SharedString::from(format!("add-{path}")))
                        .h(px(30.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(3.))
                        .border_1()
                        .border_color(gpui_kit::rgb(0xaaaaaa))
                        .bg(pal.panel)
                        .hover(|d| d.bg(pal.panel_hover))
                        .cursor_pointer()
                        .text_size(px(14.))
                        .text_color(pal.text)
                        .child(format!("Add {item_label}"))
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            let mut v = ParamValues::default();
                            fill_defaults(item_specs, &mut v, &this.units);
                            let arr = this.feature.params.0.entry(arr_id.to_string()).or_insert_with(|| ParamValue::Array(vec![]));
                            if let ParamValue::Array(a) = arr {
                                // "Match last item" values follow the previous item.
                                if let Some(last) = a.last() {
                                    for spec in item_specs.iter().filter(|p| p.hints.has(Hints::MATCH_LAST_ARRAY_ITEM)) {
                                        if let Some(val) = last.get(spec.id) {
                                            v.set(spec.id, val.clone());
                                        }
                                    }
                                }
                                a.push(v);
                                // New items take focus on their first selection field.
                                if let Some(q) = item_specs.iter().find(|p| matches!(p.kind, ParamKind::Query { .. })) {
                                    this.active_query = Some(item_path(arr_id, a.len() - 1, q.id));
                                }
                            }
                            this.changed(cx);
                            cx.notify();
                        })),
                );
                col.into_any_element()
            }
            // Reference parameters (TableData, ImageData, PartStudioData, …):
            // the labelled field and its "Select …" panel (fs_reference.rs).
            ParamKind::Reference { kind } => {
                let chosen = values.text(s.id).to_string();
                self.render_reference_field(s.label, path, *kind, &chosen, cx)
            }
            ParamKind::Lookup { table } => {
                let cur = values.lookup(s.id).to_vec();
                let path_s = path.to_string();
                let table: &'static str = table;
                // Live (dialogs/hole-01): no header row; each level is a 22 px row
                // with its name right-aligned (12 px #999) in an 88 px column and an
                // underline select ("Hole type ▾ Drilled", "Drill size ▾ 1/4"); the
                // first underline 27 px under the field above.
                let mut row = v_flex().w_full();
                let mut prefix_path: Vec<String> = Vec::new();
                let mut level = 0;
                loop {
                    let opts = noshape_model::features::hole_tables::table_levels(table, &prefix_path);
                    if opts.is_empty() || level > 6 {
                        break;
                    }
                    let name = noshape_model::features::hole_tables::table_level_name(table, &prefix_path).unwrap_or("");
                    let chosen = cur.get(level).cloned().or_else(|| noshape_model::features::hole_tables::table_level_default(table, &prefix_path)).unwrap_or_default();
                    let lvl = level;
                    let ps2 = path_s.clone();
                    let base_path = prefix_path.clone();
                    let items = opts.iter().map(|o| (SharedString::from(o.0.to_string()), SharedString::from(o.1.to_string()))).collect();
                    let sel = self.os_select(
                        &format!("{path}#{level}"),
                        items,
                        &chosen,
                        move |this, id, cx| {
                            let mut np = base_path.clone();
                            np.truncate(lvl);
                            np.push(id.to_string());
                            this.set_lookup(&ps2, np, cx);
                        },
                        cx,
                    );
                    sel.update(cx, |s, _| s.set_height(22.));
                    row = row.child(
                        h_flex()
                            .w_full()
                            .pr(px(4.))
                            .child(div().w(px(88.)).flex_none().pr(px(6.)).flex().justify_end().text_size(px(12.)).text_color(gpui_kit::rgb(0x999999)).truncate().child(name))
                            .child(div().flex_1().min_w_0().child(sel)),
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
        let external = crate::derived::ExternalRef::parse(&current);
        if let Some(r) = &external {
            self.resolve_external(r.clone(), cx);
        }
        let ext_info = self.ext_info.clone().filter(|i| i.key == current);
        // Live (part-studio/403–418): the "Part Studio" field opens the
        // Select Part Studio panel (shared with FeatureScript references),
        // where the studio or one of its parts is picked and a configured
        // studio's inputs are set (Generate); none of it shows in the dialog.
        let value = self.derived_reference_value();
        let field = self.render_reference_field("Part Studio", "partStudio", noshape_model::params::ReferenceKind::PartStudio, &value, cx);
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
        // Live (qa5 DER1): 5 px under the field, as under a selection field.
        v_flex().w_full().pb(px(5.)).child(field).children(update).into_any_element()
    }

    /// The Derived source as a reference value for the panel: `tab[#part]`,
    /// then the source configuration after a tab (`input=value;…`).
    pub(crate) fn derived_reference_value(&self) -> String {
        let studio = self.feature.params.text("partStudio").to_string();
        let parts: Vec<String> = self.feature.params.query("parts").iter().filter_map(|r| match r {
            Reference::Body(b) => Some(b.to_string()),
            _ => None,
        }).collect();
        let base = match (parts.as_slice(), studio.strip_prefix("ext:")) {
            ([one], Some(rest)) => {
                let mut it = rest.splitn(3, '|');
                match (it.next(), it.next(), it.next()) {
                    (Some(mv), Some(tab), Some(path)) => format!("ext:{mv}|{tab}#{one}|{path}"),
                    _ => studio.clone(),
                }
            }
            ([one], None) => format!("{studio}#{one}"),
            _ => studio.clone(),
        };
        let cfg = noshape_model::regen::features::importing::derived_configuration(&self.feature.params);
        if cfg.0.is_empty() {
            return base;
        }
        let pairs: Vec<String> = cfg.0.iter().map(|(k, v)| format!("{k}={v}")).collect();
        format!("{base}\t{}", pairs.join(";"))
    }

    /// A pick in the Select Part Studio panel: the studio (or one of its
    /// parts) and its configuration become the Derived parameters.
    pub(crate) fn pick_derived_source(&mut self, value: &str, cx: &mut Context<Self>) {
        use noshape_model::regen::features::importing::DERIVED_CONFIG_PREFIX;
        let (base, cfg) = value.split_once('\t').unwrap_or((value, ""));
        let (key, part) = split_part_value(base);
        let body = part.and_then(|p| {
            let local = self.derived_sources.iter().find(|s| s.tab == key).and_then(|s| s.parts.iter().find(|(b, _)| b.to_string() == p).map(|(b, _)| b.clone()));
            local.or_else(|| self.ext_info.as_ref().filter(|i| i.key == key).and_then(|i| i.parts.iter().find(|(b, _)| b.to_string() == p).map(|(b, _)| b.clone())))
        });
        self.feature.params.0.retain(|k, _| !k.starts_with(DERIVED_CONFIG_PREFIX));
        for kv in cfg.split(';').filter(|s| !s.is_empty()) {
            if let Some((k, v)) = kv.split_once('=') {
                self.feature.params.set(&format!("{DERIVED_CONFIG_PREFIX}{k}"), ParamValue::Text(v.to_string()));
            }
        }
        self.feature.params.set("parts", ParamValue::Query(body.into_iter().map(Reference::Body).collect()));
        // Clicking the selected studio again clears the field (live).
        let key = if key.is_empty() { String::new() } else { key };
        self.set_value("partStudio", ParamValue::Text(key), cx);
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
                    let parts = crate::derived::external_context(&r2).map(|c| c.bodies.iter().map(|b| (b.id.clone(), b.name.clone())).collect()).unwrap_or_default();
                    let newer = crate::derived::newer_version(&r2);
                    ExtInfo { key: r2.key(), parts, newer }
                })
                .await;
            let _ = this.update(cx, |this: &mut Self, cx| {
                this.ext_info = Some(info);
                this.ext_task = None;
                cx.notify();
            });
        }));
    }

    /// Input element for a numeric or text value.
    fn render_inline_value(&mut self, s: &ParamSpec, path: &str, values: &ParamValues, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        // Read-only outputs (Variable "Distance", "Along X"…): the value the
        // model computed, gray, not editable.
        if s.hints.has(Hints::READ_ONLY) {
            let pal = p();
            let text = self.computed.get(path).cloned().or_else(|| values.expr(s.id).map(|e| e.text.clone())).unwrap_or_default();
            return Some(div().w_full().h(px(26.)).flex().items_center().justify_end().pr(px(4.)).border_b_1().border_color(gpui_kit::rgb(0xeeeeee)).text_size(px(13.33)).text_color(pal.text_faint).child(text).into_any_element());
        }
        let text = match (&s.kind, values.get(s.id)) {
            (ParamKind::Num(_) | ParamKind::Bool { .. }, Some(ParamValue::Expr(e))) => e.text.clone(),
            (ParamKind::Text { .. }, Some(ParamValue::Text(t))) => t.clone(),
            (ParamKind::Num(_), _) | (ParamKind::Text { .. }, _) => String::new(),
            _ => return None,
        };
        let state = self.ensure_input(path, &text, window, cx);
        // Live (ui-var2/04, 07): the field shows the applied text's error,
        // not the typed one's: "10 d" being typed isn't pink yet, and an
        // applied bad value stays pink while the next one is typed.
        let editing = state.read(cx).focus_handle(cx).is_focused(window) && self.baselines.get(path).is_some_and(|b| *b != *state.read(cx).value());
        let err = if editing { self.baselines.get(path).and_then(|b| noshape_units::parse(b).err()).map(|e| e.to_string()) } else { self.parse_errors.get(path).cloned() };
        let pal = p();
        let num = match &s.kind {
            ParamKind::Num(n) => Some(*n),
            _ => None,
        };
        // Onshape: an accepted value displays converted to the document
        // units (and an expression its result); focusing shows the text.
        let focused = state.read(cx).focus_handle(cx).is_focused(window);
        // The text before editing started: Esc reverts to it (see `baselines`).
        if !focused {
            self.baselines.insert(path.to_string(), text.clone());
        }
        let converted = num
            // An out-of-range value shows as typed (live DEPTH-RANGE: "1e6 mm").
            .filter(|_| !focused && err.is_none() && self.completion.is_none() && !self.out_of_range(path, &text))
            .and_then(|n| noshape_units::evaluate_as(&text, n.kind.dim(), &self.units, &self.variables).ok())
            .map(|q| noshape_units::format_quantity(q, &self.units))
            // Dialog fields write angles "3 deg" (live dialogs/draft-01, hole).
            .map(|c| c.replace('°', " deg"))
            .filter(|c| c.replace(' ', "") != text.trim().replace(' ', ""));
        let path_s = path.to_string();
        let st2 = state.clone();
        let st3 = state.clone();
        let create_var_popup = self.create_var.as_ref().is_some_and(|c| c.path == path).then(|| self.render_create_variable(cx));
        // Live (part-studio/155): a value that doesn't evaluate (bad syntax,
        // an unknown #variable) is pink with a red underline and the hover
        // tooltip "Enter a valid expression.". Live (ui-var/03): Enter
        // shows it in the still focused field; typing hides it again.
        let applied = self.baselines.get(path).cloned().unwrap_or_else(|| text.clone());
        let eval_err = num.filter(|_| !applied.trim().is_empty()).and_then(|n| noshape_units::evaluate_as(&applied, n.kind.dim(), &self.units, &self.variables).err());
        let invalid = err.is_some() || eval_err.is_some() || (num.is_some() && self.out_of_range(path, &applied));
        // Live (ui (10-26 c)): the hover says why ("Cannot use angle as length.").
        let invalid_tip = eval_err.as_ref().map(crate::quantity_field::eval_message).unwrap_or_else(|| crate::quantity_field::INVALID_TIP.to_string());
        let field = v_flex().id(SharedString::from(format!("num-{path}")));
        let field = self.completion_keys(field, path, cx);
        // Enter in a changed field applies it; the next Enter commits.
        let (p_ent, st_ent) = (path.to_string(), state.clone());
        let field = field.capture_action(cx.listener(move |this, _: &crate::actions::Accept, window, cx| {
            if this.apply_on_enter(&p_ent, &st_ent, window, cx) {
                cx.stop_propagation();
            }
        }));
        // ↑ / ↓ step a plain, committed value and commit it (live); with the
        // autofill list open they move in the list (completion_keys first).
        let field = if num.is_some() {
            use gpui_kit::base::actions::{SelectDown, SelectUp};
            use gpui_kit::component::input::{MoveDown, MoveUp};
            let (p_up, st_up, p_dn, st_dn) = (path.to_string(), state.clone(), path.to_string(), state.clone());
            let (p_su, st_su, p_sd, st_sd, p_k, st_k) = (path.to_string(), state.clone(), path.to_string(), state.clone(), path.to_string(), state.clone());
            let shift = crate::quantity_field::StepMods { shift: true, ctrl: false };
            field
                // Shift+↑ / ↓ (the input's select-up / -down): ×10.
                .capture_action(cx.listener(move |this, _: &SelectUp, window, cx| {
                    if this.step_number(&p_su, &st_su, true, shift, window, cx) {
                        cx.stop_propagation();
                    }
                }))
                .capture_action(cx.listener(move |this, _: &SelectDown, window, cx| {
                    if this.step_number(&p_sd, &st_sd, false, shift, window, cx) {
                        cx.stop_propagation();
                    }
                }))
                // Ctrl+↑ / ↓ (no input binding): ×0.1.
                .capture_key_down(cx.listener(move |this, e: &KeyDownEvent, window, cx| {
                    let k = &e.keystroke;
                    if (k.key == "up" || k.key == "down") && k.modifiers.control && !k.modifiers.alt {
                        let mods = crate::quantity_field::StepMods { shift: k.modifiers.shift, ctrl: true };
                        if this.step_number(&p_k, &st_k, k.key == "up", mods, window, cx) {
                            cx.stop_propagation();
                        }
                    }
                }))
                .capture_action(cx.listener(move |this, _: &MoveUp, window, cx| {
                    let mods = window.modifiers();
                    if this.step_number(&p_up, &st_up, true, crate::quantity_field::StepMods { shift: mods.shift, ctrl: mods.control || mods.platform }, window, cx) {
                        cx.stop_propagation();
                    }
                }))
                .capture_action(cx.listener(move |this, _: &MoveDown, window, cx| {
                    let mods = window.modifiers();
                    if this.step_number(&p_dn, &st_dn, false, crate::quantity_field::StepMods { shift: mods.shift, ctrl: mods.control || mods.platform }, window, cx) {
                        cx.stop_propagation();
                    }
                }))
        } else {
            field
        };
        // Esc in a changed field reverts it; the next Esc cancels the dialog.
        let (p_esc, st_esc) = (path.to_string(), state.clone());
        // (The kit's revert: the focus leaves the field too.)
        let field = field.capture_action(cx.listener(move |this, _: &crate::actions::Cancel, window, cx| {
            if !st_esc.update(cx, |s, cx| s.revert_escape(window, cx)) {
                return;
            }
            cx.stop_propagation();
            let base = st_esc.read(cx).value().to_string();
            this.on_text_changed(&p_esc, base, cx);
            cx.notify();
        }));
        Some(
            field
                .gap_0p5()
                // Onshape underline field: no box, 1 px #d7d7d7 underline (blue on hover/focus),
                // invalid values get a pink fill and red underline (§3.9, §4.2).
                // The whole field box takes clicks and shows the I-beam.
                .child(gpui_kit::base::input::input_frame(
                    div()
                        .id(SharedString::from(format!("numf-{path}")))
                        .border_b_1()
                        .border_color(if invalid { pal.error } else { pal.border })
                        .map(|d| crate::quantity_field::invalid_field_tip(d, invalid, invalid_tip))
                        .hover(|d| d.border_color(pal.accent))
                        // Live (dialogs/variable-01): the focused field has a 2 px blue underline.
                        // (A 1 px border plus a 1 px inset line above it, so
                        // the underline thickens upward like live's inset shadow.)
                        .when(focused && !invalid, |d| d.border_color(pal.accent).child(div().absolute().left_0().right_0().bottom_0().h(px(1.)).bg(pal.accent)))
                        // The input stays in the tree (so clicks and Tab can
                        // focus it); unfocused, the converted value covers it.
                        .relative()
                        // Live (dom/caret-inputs.json): `.os-param-number` / `.os-param-text` pad 3 px
                        // at the sides (numbers end 3 px from the right edge).
                        .child(div().when(converted.is_some(), |d| d.opacity(0.)).child(Input::new(&state).small().appearance(false).text_size(px(13.33)).px(px(3.)).when(num.is_some(), |i| i.text_align(TextAlign::Right))))
                        .when_some(converted, |d, c| {
                            // Live: hovering shows "Depth: 32.1 mm".
                            let tip = format!("{}: {c}", s.label);
                            d.child(
                                div()
                                    .id(SharedString::from(format!("numv-{path}")))
                                    .tooltip(crate::ui::tooltip(tip))
                                    .absolute()
                                    .inset_0()
                                    .px(px(3.))
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
                ))
                .when_some(self.completion.as_ref().filter(|c| c.key == path), |d, c| {
                    let view = cx.entity();
                    d.child(c.render(&self.variables, &self.units, move |i, window, cx| view.update(cx, |this, cx| this.accept_completion(Some(i), window, cx))))
                })
                .children(create_var_popup)
                .when_some(num, |d, _| {
                    // The wheel steps a focused field like ↑ / ↓ (live: only
                    // while focused; unfocused it does nothing).
                    let p_wheel = path_s.clone();
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
                        let mods = crate::quantity_field::StepMods { shift: e.modifiers.shift, ctrl: e.modifiers.control || e.modifiers.platform };
                        this.step_number(&p_wheel, &st2, dy > 0., mods, window, cx);
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
        if self.feature.type_id == "hole" && !noshape_model::custom::std_feature::has_editing_logic("hole") {
            // Copy the standard's sizes into the numeric fields (Onshape behavior)
            // and rebuild the inputs so they show the new values (std's
            // holeEditLogic does it where it runs).
            noshape_model::features::hole_tables::apply_table_to_params(&mut self.feature.params);
            self.inputs.clear();
            self.changed(cx);
        }
    }
}


/// A length parameter as its field shows it: converted to the workspace
/// units ("6.35 mm" for "0.25 in").
pub fn shown_length(f: &Feature, id: &str, units: &UnitPrefs, vars: &std::collections::BTreeMap<String, noshape_units::Quantity>) -> Option<String> {
    let e = f.params.expr(id)?;
    let q = noshape_units::evaluate_as(&e.text, noshape_units::Dim::LENGTH, units, vars).ok()?;
    Some(noshape_units::format_quantity(q, units))
}

/// Feature schemas are static (registry), so array item specs live forever.
/// Parameters shown with an icon instead of their label (live Hole dialog).
fn param_icon(feature: &str, param: &str) -> Option<&'static str> {
    match (feature, param) {
        ("hole", "holeDiameterV3") => Some("hole-diameter"),
        ("hole", "holeDepthV3" | "holeDepthComputedV3") => Some("hole-depth"),
        ("hole", "tipAngleStyle" | "tipAngleV3") => Some("hole-drill-angle"),
        // Counterbore, countersink and tap rows too (qa5 ADV4: text labels
        // truncated the values); our own drawings of Onshape's ideas.
        ("hole", "cBoreDiameterV3") => Some("hole-cbore-diameter"),
        ("hole", "cBoreDepthV3") => Some("hole-cbore-depth"),
        ("hole", "cSinkDiameterV3") => Some("hole-csink-diameter"),
        ("hole", "cSinkAngleV3") => Some("hole-csink-angle"),
        // Live (ADV4): the tap drill row shows the plain diameter icon.
        ("hole", "tapDrillDiameterV3") => Some("hole-diameter"),
        ("hole", "tapClearance") => Some("hole-tap-clearance"),
        ("hole", "tappedDepthV3") => Some("hole-tapped-depth"),
        // Live (part-studio/102, dom/dialog-copy-here.txt): Translate by
        // XYZ rows start with an axis icon instead of a label.
        ("transform", "dx") => Some("xyz-along-x"),
        ("transform", "dy") => Some("xyz-along-y"),
        ("transform", "dz") => Some("xyz-along-z"),
        _ => None,
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

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
        let help_focus = self.tab_stop("help", cx);
        let params: &'static [ParamSpec] = self.ty.params.as_slice();
        self.live_y.set(0.);
        let mut rows = self.render_params(params, None, window, cx);
        let footer_pad = if self.feature.type_id == "sketch" { 0. } else { self.live_h(24.5) - 24. };
        let (extra_rows, footer_extra) = match self.extra.clone() {
            Some(f) => f(window, cx),
            None => (Vec::new(), None),
        };
        rows.extend(extra_rows);
        // The Sketch dialog has no preview slider (live).
        let sketch = self.feature.type_id == "sketch";
        let status = self.status.clone();
        // Onshape: the title turns red while the feature is invalid.
        // Live L27: a warning turns the title red too (has-regen-error).
        // Live (ui (10-26 c), VAR-INVALID): a value that doesn't evaluate as
        // its field's type turns the title red with "A parameter value is
        // not valid." and disables ✓ (Enter doesn't commit either).
        // Only applied values count: "10" on the way to "10 deg" doesn't.
        for spec in self.ty.params.iter() {
            if let Some(ParamValue::Expr(e)) = self.feature.params.get(spec.id)
                && let ParamKind::Num(n) = &spec.kind
                && let applied = self.applied_text(spec.id, &e.text)
                && noshape_units::evaluate_as(&applied, n.kind.dim(), &self.units, &self.variables).is_ok()
            {
                self.valid_texts.insert(spec.id, applied);
            }
        }
        let value_invalid = self.applied_value_error().is_some();
        let status = if value_invalid { Some((FeatureStatus::Error, VALUE_NOT_VALID.to_string())) } else { status };
        let invalid = matches!(status.as_ref().map(|s| s.0), Some(FeatureStatus::Error | FeatureStatus::Warning));
        // Live (`canSubmit`, woolsthorpe): ✓ waits for the dialog to be
        // dirty (an edit, or a regeneration without an error) and for every
        // applied value to be valid; regeneration errors and empty
        // selections don't disable it (a new Loft with no profiles isn't
        // dirty yet, icons2; an Add with no merge scope is, qa5 BOOL4b).
        let blocked = value_invalid || !self.dirty;
        let title_color = if invalid { pal.text_invalid } else { pal.text };
        let shown = self.shown_feature();
        let name = noshape_model::interaction::display_name_computed(&shown, &|k| self.computed.get(k).cloned().or_else(|| shown_length(&shown, k, &self.units, &self.variables)));
        // Live (dom/dialog-hole.txt, dialog-variable.txt): a name built from
        // the parameters ("Ø 0.25 in ↧ 0.5 in", "#? = 0 in") is followed
        // by the feature type in brackets.
        let tip_name = if name == self.feature.name { crate::part_studio::typed_name(&self.feature) } else { format!("{name} [{}]", self.ty.name) };
        // The sketch's red title has no tooltip: the bubble asks for a plane.
        let error_tip: Option<String> = status.as_ref().filter(|s| matches!(s.0, FeatureStatus::Error | FeatureStatus::Warning) && !sketch).map(|s| format!("{tip_name} did not regenerate properly: {}", s.1));
        let ref_panel = self.render_reference_panel(window, cx);
        let renaming = self.renaming.clone();
        v_flex()
            .id("feature-dialog")
            // Clicks on the dialog never reach the graphics area beneath.
            .occlude()
            .key_context("FeatureDialog")
            .track_focus(&self.focus)
            // Onshape feature dialog frame (§3.9): 220 px, white at 95%, no
            // border or radius, shadow 0 5px 10px rgba(0,0,0,.2) (GPUI blurs
            // twice as far as CSS: half the radius).
            .relative()
            // Live (behaviours "ui2 (10-24 09:10)"): 220 px in every language
            // until the user drags the right edge (remembered per type).
            .w(self.width.unwrap_or(px(220.)))
            .max_h(px(720.))
            .bg(pal.dialog_bg)
            .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.2), offset: point(px(0.), px(5.)), blur_radius: px(5.), spread_radius: px(0.), inset: false }])
            .text_color(pal.text)
            // Live registers feature dialogs with its panel layout manager:
            // moved panels snap to their edges too.
            .child(crate::ui::panel_probe("feature-dialog"))
            .on_action(cx.listener(|_, _: &crate::actions::Accept, _, cx| cx.emit(DialogEvent::Accept { repeat: false })))
            // Tab / Shift+Tab walk the controls in paint (DOM) order; past
            // either end the focus leaves the dialog (live psb 10-19 h).
            .on_action(cx.listener(|this, _: &crate::actions::DialogTab, window, cx| {
                window.focus_next(cx);
                if !this.focus.contains_focused(window, cx) {
                    cx.emit(DialogEvent::LeaveFocus { forward: true });
                }
            }))
            .on_action(cx.listener(|this, _: &crate::actions::DialogTabPrev, window, cx| {
                window.focus_prev(cx);
                if !this.focus.contains_focused(window, cx) {
                    cx.emit(DialogEvent::LeaveFocus { forward: false });
                }
            }))
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
                // Live: 28 px plus the 1 px bottom border.
                h_flex()
                    .h(px(29.))
                    .flex_none()
                    .pl(px(6.))
                    .bg(pal.panel)
                    .border_b_1()
                    .border_color(pal.border)
                    .child(match renaming {
                        Some(state) => div().flex_1().child(Input::new(&state).xsmall()).into_any_element(),
                        None => div()
                            .id("dialog-title")
                            // Drag the title to move the dialog (Onshape; its
                            // drag area shows `grab`, docs/parity/cursors.md).
                            .cursor(CursorStyle::OpenHand)
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
                            .child(crate::ui::user_text(name))
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
                                            this.changed(cx);
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
                    // Live (psb 10-21 g): a dialog in configure mode has only ✕.
                    .when(!self.configure_mode, |d| d.child(
                        Button::new("dlg-ok")
                            // Live: the ✓ / ✕ are not Tab stops.
                            .tab_stop(false)
                            .custom(
                                gpui_kit::component::button::ButtonCustomVariant::new(cx)
                                    .color(pal.accept.into())
                                    .hover(gpui_kit::rgb(0x74ce3a).into())
                                    .active(pal.accept.into())
                                    .shadow(false),
                            )
                            // Live (dom adv4-a-dialog): ✓ and ✗ are 24 px sprites centred in 28 x 24.
                            .child(crate::assets::icon("dialog-ok", 24., cx))
                            .w(px(28.))
                            .h(px(28.))
                            .p_0()
                            .rounded(px(0.))
                            .bg(pal.accept).when(blocked, crate::ui::disabled_accept)
                            // Live (dom/cursors.txt): ✓ and ✗ show the default arrow
                            // (`.ns-dialog-button-ok/-cancel`), enabled or not.
                            .cursor(CursorStyle::Arrow)
                            // Live (psb 10-21 e): ✓ and ✕ have no tooltip in
                            // any dialog (no title either).
                            // Onshape: the ✓ is disabled while the feature errors
                            // (Enter still commits it).
                            .on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                                if !blocked {
                                    cx.emit(DialogEvent::Accept { repeat: false })
                                }
                            })),
                    ))
                    .child(
                        Button::new("dlg-cancel")
                            .tab_stop(false)
                            .custom(dialog_icon_variant(cx, false))
                            .child(crate::assets::icon("dialog-cancel", 24., cx))
                            .w(px(28.))
                            .h(px(28.))
                            .p_0()
                            .rounded(px(0.))
                            .cursor(CursorStyle::Arrow)
                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| cx.emit(DialogEvent::Cancel))),
                    ),
            )
            .child(v_flex().id("dialog-body").px(px(2.)).pb(px(4.)).overflow_y_scroll().children(rows).when(self.configure_mode, |d| {
                // Live (psb 10-21 g): configure mode ends with ☑ "Unsuppressed",
                // so suppression can be configured too.
                let key = noshape_model::configuration::SUPPRESSED_PARAM;
                let mut el = div().id("cfg-unsuppressed").cursor_pointer().child(crate::ui::os_checkbox("cfg-unsuppressed-box", "Unsuppressed", !self.feature.suppressed)).capture_any_mouse_down(cx.listener(move |_, e: &MouseDownEvent, _, cx| {
                    if e.button == MouseButton::Left {
                        cx.stop_propagation();
                        cx.emit(DialogEvent::ConfigureParam(key.to_string()));
                    }
                }));
                if self.configured.contains(key) {
                    el = el.border_2().border_dashed().border_color(gpui_kit::rgb(0xfab601));
                }
                d.child(el)
            }))
            // Live (`osc-resize-handle.osc-x`, dom/cursor-rows.json): a 3 px
            // handle inside the right edge resizes the width only; there is
            // no corner grip.
            .child(div().id("dialog-grip").absolute().top_0().bottom_0().right_0().w(px(3.)).cursor(CursorStyle::ResizeLeftRight).on_drag(DialogResize, |d, _, _, cx| cx.new(|_| d.clone())))
            .child(
                // Footer (live): no band; a 112 px preview slider at the
                // left and the round ? help button at the right. Live
                // (dom/dialog-extrude-new): the frame ends 24.5 px under the
                // list (22 px slider row + 2.5): 20 px after our 4 px body
                // padding, its controls on the same line.
                h_flex()
                    .h(px(if sketch { 25. } else { 20. }))
                    .when(!sketch, |d| d.pt(px(2.)))
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
                                this.changed(cx);
                                cx.notify();
                            })),
                    ))
                    // Preview slider: before (left) to after (right) the feature.
                    // Features without a preview (Sketch; FeatureScript
                    // NO_PREVIEW_PROVIDED: Variable) have no slider (live).
                    .when(!sketch && self.feature.type_id != "assignVariable", |d| d.child(self.render_preview_slider(cx)))
                    .child(div().flex_1())
                    .children(footer_extra)
                    .child(
                        Self::tab_stop_element(
                            div().id("dlg-help"),
                            &help_focus,
                            Focus::Ring,
                            |this, _, cx| {
                                this.show_help = !this.show_help;
                                cx.notify();
                            },
                            cx,
                        )
                            // Live: a 12 px `#a0a0a0` disc, its centre 1 px
                            // above the slider track, 11 px from the frame's
                            // right edge (dialogs/*-01-open: disc x 449–461).
                            .size(px(12.))
                            .relative()
                            // Live (dom/dialog-sketch-empty): in the Sketch
                            // dialog's 25 px footer the help sits 10 px down.
                            .top(px(if sketch { 1. } else { -2. }))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            // Live: the help-button sprite (dom svg-icon-help-button),
                            // its "?" cut out.
                            .child(crate::ui::glyph("help-dot", 13.3, crate::theme::help_dot(0xa0a0a0)))
                            .cursor_pointer()
                            .tooltip(crate::ui::tooltip(format!("{} help", self.ty.name)))
                            // Help: the feature's description (offline help).
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.show_help = !this.show_help;
                                cx.notify();
                            })),
                    ),
            )
            // Live: the frame ends 24.5 px under the list's unrounded end
            // (one more pixel when that rounds up, e.g. Fillet).
            .when(footer_pad > 0., |d| d.child(div().flex_none().h(px(footer_pad))))
            .when(self.show_help && !self.ty.description.is_empty(), |d| {
                d.child(div().mx_3().mb_2().p_2().rounded_md().bg(pal.app_bg).text_xs().text_color(pal.text_muted).child(format!("{}: {}.", self.ty.name, self.ty.description)))
            })
            // Onshape shows regeneration errors as the red title (with the
            // message as its tooltip), not as an inline alert; warnings get the alert.
            // Warnings are a yellow bubble over the graphics (live L27).
            .when_some(status.filter(|s| !matches!(s.0, FeatureStatus::Ok | FeatureStatus::Error | FeatureStatus::Warning)), |d, (st, msg)| {
                let kind = match st {
                    FeatureStatus::Error => crate::ui::AlertKind::Error,
                    FeatureStatus::Warning => crate::ui::AlertKind::Warning,
                    _ => crate::ui::AlertKind::Info,
                };
                d.child(div().p(px(4.)).child(crate::ui::alert(kind).text_size(px(12.)).child(div().flex_1().min_w_0().whitespace_normal().child(msg))))
            })
            .children(ref_panel)
    }
}

/// How a Tab stop shows its focus ([`FeatureDialog::tab_stop_element`]).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    /// Icon buttons: a 2 px blue bottom line.
    Underline,
    /// Checkboxes: their box draws it.
    Box,
    /// The `?` help link.
    Ring,
}

/// Live row heights in Chrome layout units (1/64 px, see `live_h`): a
/// 17.33 px label line + 10 px padding; an empty selection field's 13.33 px
/// line + 11 px padding + 2 px border.
const LIVE_CHECKBOX_ROW: f32 = 27.328125;
const LIVE_EMPTY_FIELD: f32 = 26.328125;

/// Where Onshape's preview slider handle rests (live: 69% of the track).
/// From there to the right end the feature shows fully; to the left it fades.
pub(crate) const PREVIEW_REST: f32 = 0.7;

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
                    .shadow(vec![BoxShadow { color: hsla(0., 0., 0., 0.6), offset: point(px(0.), px(0.)), blur_radius: px(1.5), spread_radius: px(0.), inset: false }]),
            )
            .child(canvas(move |b, _, _| track.set(b), |_, _, _, _| {}).absolute().top_0().left_0().size_full())
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, e: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
                set(this, e.position.x, cx);
            }))
            .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, _, cx| {
                if e.pressed_button == Some(MouseButton::Left) && !cx.has_active_drag() {
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

/// A selection row dragged by its grip in reorder mode.
#[derive(Clone)]
struct PickDrag {
    path: String,
    index: usize,
    label: SharedString,
    /// A driven-query array's item (else a pick in a query).
    array: bool,
    offset: Point<Pixels>,
}

impl Render for PickDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // Live ui/966: the same pill as the Configurations headers.
        crate::ui::name_pill(self.label.clone(), self.offset)
    }
}

/// The dialog's resize grip being dragged.
#[derive(Clone)]
pub struct DialogResize;

impl Render for DialogResize {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
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

/// Tolerance options of a tolerant quantity, stored as `<param>__tolerance`
/// text (the model reads them for hole callouts and fits).
pub use noshape_model::fits::Tolerance;

/// A Part Studio reference value split into the studio key and the picked
/// part: `tab#part`, or `ext:<microversion>|<tab>#<part>|<path>` (as the
/// model's FeatureScript references store it).
fn split_part_value(value: &str) -> (String, Option<String>) {
    if let Some(rest) = value.strip_prefix("ext:") {
        let mut it = rest.splitn(3, '|');
        if let (Some(mv), Some(tab), Some(path)) = (it.next(), it.next(), it.next()) {
            let (tab, part) = match tab.split_once('#') {
                Some((t, p)) => (t, Some(p.to_string())),
                None => (tab, None),
            };
            return (format!("ext:{mv}|{tab}|{path}"), part);
        }
    }
    match value.split_once('#') {
        Some((t, p)) => (t.to_string(), Some(p.to_string())),
        None => (value.to_string(), None),
    }
}

/// A field outside the dialog asking for the Create variable popup
/// ([`FeatureDialog::open_external_create_variable`]).
pub(crate) struct ExternalField {
    /// The field's path in the dialog (any unique key).
    pub path: &'static str,
    pub state: Entity<InputState>,
    /// The variable's kind (the popup's Length / Angle / Number tab).
    pub kind: Option<noshape_model::params::NumKind>,
    /// The popup's Value: the field's previous value.
    pub previous: String,
    /// The name typed after `#`.
    pub name: String,
}

/// A parameter's Configure / Unconfigure context-menu item (live psb 10-21 g).
fn configure_items(m: crate::os_menu::OsMenu, view: &Entity<FeatureDialog>, param: &str, configured: bool) -> crate::os_menu::OsMenu {
    let (v, p) = (view.clone(), param.to_string());
    m.item(crate::os_menu::OsMenuItem::new(if configured { "Unconfigure" } else { "Configure" }).on_click(move |_, _, cx| {
        v.update(cx, |_, cx| cx.emit(if configured { DialogEvent::UnconfigureParam(p.clone()) } else { DialogEvent::ConfigureParam(p.clone()) }))
    }))
}

/// Whether a sketch's picked vertices show as one "Vertices of Sketch N"
/// row: the sketch was picked as a whole (its row) and every vertex is
/// still there. Points clicked one by one stay separate chips (live qa5
/// (10-25 p1) (2): two clicks give two "Vertex of Sketch 1" chips).
fn groups_vertices(picked_as_whole: bool, vertices: usize, picked: usize) -> bool {
    picked_as_whole && vertices >= 2 && picked == vertices
}

#[cfg(test)]
mod vertex_group_tests {
    use super::groups_vertices;

    #[::core::prelude::v1::test]
    fn only_a_sketch_picked_whole_shows_as_vertices_of() {
        // Two points clicked one by one: two chips (qa5 P1-8).
        assert!(!groups_vertices(false, 2, 2));
        // The sketch's row: one "Vertices of Sketch 3" row.
        assert!(groups_vertices(true, 2, 2));
        // One of them removed again: single chips.
        assert!(!groups_vertices(true, 2, 1));
    }
}
