//! Onshape's metadata "Properties" modal (parts and assemblies; live
//! assembly/100–102, dom/dialog-assembly-part-properties.txt,
//! dom/dialog-assembly-top-properties.txt) and the small "Instance
//! properties" modal (dom/dialog-assembly-instance-properties.txt).
//!
//! An 800 px Bootstrap modal 15 px from the top: a 57 px header with the
//! 24 px title, a sub-header with the item's name (18 px, `#ccc` rule), a
//! scrolling form of bold 14 px labels over full-width 34 px fields, and a
//! footer with form undo / redo at the left and Apply, Save (disabled until
//! edited) and Close at the right.

use crate::theme::p;
use gpui_kit::component::WindowExt as _;
use gpui_kit::component::input::{IndentInline, Input, InputEvent, InputState, OutdentInline, Textarea, TextareaState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use noshape_model::partstudio::{ItemMeta, RELEASE_STATES};
use std::rc::Rc;

/// Properties › Unit of measure, in Onshape's order (live assembly/131;
/// the first option is blank).
pub const UNITS_OF_MEASURE: [&str; 18] = [
    "", "Centimeter", "Foot", "Inch", "Meter", "Millimeter", "Yard", "Gram", "Kilogram", "Ounce", "Pound", "Liter", "Gallon", "Each", "Fluid ounce", "Milliliter", "Centiliter", "Package",
];
/// Properties › Mass override: "2.5 kg", "2.5", "5.5 lb" → kg.
pub fn parse_mass(text: &str) -> Option<f64> {
    let t = text.trim();
    let (num, unit) = t.split_at(t.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(t.len()));
    let v: f64 = num.trim().parse().ok()?;
    let k = match unit.trim() {
        "" | "kg" => 1.0,
        "g" => 1e-3,
        "lb" | "lbm" => 0.453_592_37,
        "oz" => 0.028_349_523_125,
        _ => return None,
    };
    (v > 0.0).then_some(v * k)
}

/// The user's mass unit (Workspace units › Mass: Kilogram, Gram, Pound,
/// Ounce): (symbol, per kilogram).
pub fn mass_unit(units: &noshape_units::UnitPrefs) -> (&'static str, f64) {
    match units.other[2].0 {
        1 => ("g", 1000.0),
        2 => ("lb", 2.204_622_621_848_776),
        3 => ("oz", 35.273_961_949_580_41),
        _ => ("kg", 1.0),
    }
}

/// A Mass and section properties value as Onshape writes it (live 738–739):
/// three decimals without trailing zeros ("5.77", "11000"), and values that
/// would read 0.000 in exponent form ("-3.307e-4").
pub fn mass_number(x: f64) -> String {
    let fixed = noshape_units::to_fixed(x, 3);
    if x != 0.0 && fixed.trim_start_matches('-') == "0.000" {
        return format!("{x:.3e}");
    }
    let t = fixed.trim_end_matches('0').trim_end_matches('.');
    if t == "-0" { "0".into() } else { t.to_string() }
}

/// A drawing's own fields, after Title 3 (live doc-tab-properties-drawing).
const DRAWING_FIELDS: [&str; 4] = ["Drawn by", "Approver", "Date drawn", "Date approved"];
/// A drawing's shown fields, after Not revision managed.
const DRAWING_INFO: [&str; 3] = ["Last changed date", "Last changed by", "Inspection count"];

/// A new item's unit of measure.
pub const DEFAULT_UNIT: &str = "Each";
/// A field whose values differ among the parts shown (live q2962: italic grey, the field empty).
const MULTIPLE_VALUES: &str = "Multiple values";

/// The values the Properties modal edits.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MetaValues {
    pub name: String,
    pub description: String,
    pub part_number: String,
    pub revision: String,
    pub meta: ItemMeta,
    /// Mass › Override (text in the document's mass unit), `None` = computed.
    pub mass_override: Option<String>,
    /// Subassembly BOM behavior (assemblies), index into the options.
    pub bom: usize,
    /// A drawing's Drawn by, Approver, Date drawn and Date approved (dates MM/DD/YYYY as shown).
    pub drawing: Option<[String; 4]>,
}

/// What the modal shows besides the values.
pub struct MetaSpec {
    /// "Onshape Part" / "Onshape Assembly".
    pub category: &'static str,
    /// Parts: the assigned material ("Unassigned") and the computed mass.
    pub material: Option<String>,
    pub computed_mass: Option<String>,
    /// Assemblies: the "Subassembly BOM behavior" options.
    pub bom_options: Option<Vec<String>>,
    /// Drawings: Last changed date, Last changed by and Inspection count (shown, not edited).
    pub drawing_info: Option<[String; 3]>,
    pub values: MetaValues,
}

type SaveFn = Rc<dyn Fn(MetaValues, &mut Window, &mut App)>;

thread_local! {
    /// The Properties form last opened (tests drive it).
    static OPEN_FORM: std::cell::RefCell<WeakEntity<MetaForm>> = std::cell::RefCell::new(WeakEntity::new_invalid());
}

/// The open Properties modal's form.
pub fn open_form() -> Option<Entity<MetaForm>> {
    OPEN_FORM.with(|f| f.borrow().upgrade())
}
/// Several parts' Save: each written part (its index in the list) with its new values.
type SaveManyFn = Rc<dyn Fn(Vec<(usize, MetaValues)>, &mut Window, &mut App)>;

/// Several parts in one modal (Parts list › Properties… on 2 or more, live q2962): a left list of the parts, the form
/// showing the common values of the rows selected in it ("Multiple values" where they differ), Save writing only the
/// edited fields to each of them.
struct Multi {
    names: Vec<String>,
    thumbs: Vec<Option<std::sync::Arc<RenderImage>>>,
    values: Vec<MetaValues>,
    /// What Reset all restores, per part.
    reset_to: Vec<MetaValues>,
    /// The list's selected rows (the form's parts) and the Shift+click anchor.
    shown: Vec<usize>,
    anchor: usize,
    /// Fields whose values differ among the shown parts when the form was filled, and those still differing (a
    /// mixed checkbox clicked stops being mixed).
    mixed_at_fill: Vec<String>,
    mixed: Vec<String>,
    /// Reset all was clicked: Save writes each part's reset values before the edits.
    reset: bool,
    on_save: SaveManyFn,
}

/// The modal's fields as text, by label (checkboxes "true" / "false", Override "" when off), for comparing parts.
const MULTI_FIELDS: [&str; 14] = ["Name", "Description", "Part number", "Revision", "Vendor", "Project", "Product line", "Title 1", "Title 2", "Title 3", "Not revision managed", "Exclude from all BOMs", "Unit of measure", "Override"];

fn field_of(v: &MetaValues, f: &str) -> Option<String> {
    let o = |x: &Option<String>| x.clone().unwrap_or_default();
    let m = &v.meta;
    Some(match f {
        "Name" => v.name.clone(),
        "Description" => v.description.clone(),
        "Part number" => v.part_number.clone(),
        "Revision" => v.revision.clone(),
        "Vendor" => o(&m.vendor),
        "Project" => o(&m.project),
        "Product line" => o(&m.product_line),
        "Title 1" => o(&m.title1),
        "Title 2" => o(&m.title2),
        "Title 3" => o(&m.title3),
        "Not revision managed" => m.not_revision_managed.to_string(),
        "Exclude from all BOMs" => m.exclude_from_bom.to_string(),
        "Unit of measure" => o(&m.unit_of_measure),
        "Override" => o(&v.mass_override),
        _ => m.custom.get(f.strip_prefix("custom:")?).cloned().unwrap_or_default(),
    })
}

fn set_field(v: &mut MetaValues, f: &str, x: String) {
    let some = |x: String| Some(x).filter(|s| !s.is_empty());
    let m = &mut v.meta;
    match f {
        "Name" => v.name = x,
        "Description" => v.description = x,
        "Part number" => v.part_number = x,
        "Revision" => v.revision = x,
        "Vendor" => m.vendor = some(x),
        "Project" => m.project = some(x),
        "Product line" => m.product_line = some(x),
        "Title 1" => m.title1 = some(x),
        "Title 2" => m.title2 = some(x),
        "Title 3" => m.title3 = some(x),
        "Not revision managed" => m.not_revision_managed = x == "true",
        "Exclude from all BOMs" => m.exclude_from_bom = x == "true",
        "Unit of measure" => m.unit_of_measure = some(x),
        "Override" => v.mass_override = some(x),
        _ => {
            let Some(k) = f.strip_prefix("custom:") else { return };
            match some(x) {
                Some(x) => m.custom.insert(k.to_string(), x),
                None => m.custom.remove(k),
            };
        }
    }
}

/// Every field of these values (the custom ones as "custom:<name>").
fn fields_of<'a>(vs: impl Iterator<Item = &'a MetaValues>) -> Vec<String> {
    let mut out: Vec<String> = MULTI_FIELDS.iter().map(|f| f.to_string()).collect();
    for v in vs {
        for k in v.meta.custom.keys() {
            let f = format!("custom:{k}");
            if !out.contains(&f) {
                out.push(f);
            }
        }
    }
    out
}

/// The values the form shows for `parts`: each field's common value, empty (off) where they differ; and those fields.
fn common_values(parts: &[&MetaValues]) -> (MetaValues, Vec<String>) {
    let Some(first) = parts.first() else { return (MetaValues::default(), Vec::new()) };
    let mut v = (*first).clone();
    let mut mixed = Vec::new();
    for f in fields_of(parts.iter().copied()) {
        let x = field_of(first, &f);
        if parts.iter().any(|p| field_of(p, &f) != x) {
            set_field(&mut v, &f, String::new());
            mixed.push(f);
        }
    }
    (v, mixed)
}

/// A custom property's field: text (String, numbers, dates), a checkbox
/// (Boolean) or a select (Enum).
enum CustomInput {
    Text(Entity<InputState>),
    Bool(bool),
    List(Option<String>),
}
/// Opens the Create Release candidate dialog for the item.
type ReleaseFn = Rc<dyn Fn(&mut Window, &mut App)>;

/// A Part Studio tab's card in the modal's left tree (live 1738: a 300 px
/// panel with the studio's ▸, 70×40 picture, name and grey type).
#[derive(Clone)]
pub struct ItemCard {
    pub kind: &'static str,
    pub thumb: Option<std::sync::Arc<RenderImage>>,
}

/// The form inside the modal.
pub struct MetaForm {
    /// The left tree's card (Part Studio tabs).
    card: Option<ItemCard>,
    /// Several parts at once.
    multi: Option<Multi>,
    /// The modal's own focus: it opens with no field focused (live feel/dialogs.json "propertiesDialog": typing does
    /// nothing until a field is clicked; the document's shortcuts are off behind it).
    focus: FocusHandle,
    /// Not revision managed's and Exclude from all BOMs' Tab stops (live: after Title 3, in that order).
    check_focus: [FocusHandle; 2],
    /// The fields edited since the form was filled or saved (by label), worked out once per frame.
    edited: Vec<String>,
    /// Release management: the release-create icon at the top left
    /// (Release__typical_release_workflow, "From any Properties dialog").
    on_release: Option<ReleaseFn>,
    /// Part Studio parts: "Reset all" restores these values (live
    /// dom/dialog-partstudio-part-properties.txt; absent for assembly parts).
    reset_to: Option<MetaValues>,
    spec_category: &'static str,
    /// Part Studio parts: fields with a configured value in the active
    /// configuration ("Name", "Part number"…; Configured properties).
    configured: Vec<&'static str>,
    /// Fields shown but not edited (a standard content part's Name, live skb 10-20).
    read_only: Vec<&'static str>,
    material: Option<String>,
    computed_mass: Option<String>,
    bom_options: Option<Vec<String>>,
    name: Entity<InputState>,
    description: Entity<TextareaState>,
    part_number: Entity<InputState>,
    revision: Entity<InputState>,
    vendor: Entity<InputState>,
    project: Entity<InputState>,
    product_line: Entity<InputState>,
    titles: [Entity<InputState>; 3],
    /// Drawings: Drawn by, Approver, Date drawn, Date approved; and the shown Last changed date / by, Inspection count.
    drawing: Option<[Entity<InputState>; 4]>,
    drawing_info: Option<[String; 3]>,
    mass: Entity<InputState>,
    /// Values not held by text inputs.
    state: Option<String>,
    not_revision_managed: bool,
    exclude_from_bom: bool,
    unit: Option<String>,
    mass_on: bool,
    /// Calculate value was clicked (the button goes away) / it failed.
    mass_computed: bool,
    mass_failed: bool,
    /// The computed mass is still being worked out off the UI thread (an assembly's, from its tab's menu).
    mass_pending: bool,
    bom: usize,
    /// Custom properties (Company settings › Custom properties): the active
    /// ones for the item's category, at the bottom of the dialog, and the
    /// item's values of the others (kept as they are).
    custom: Vec<(noshape_ops::custom_properties::CustomProperty, CustomInput)>,
    custom_rest: std::collections::BTreeMap<String, String>,
    /// Saved values (Save / Apply enabled while the form differs).
    saved: MetaValues,
    /// Form undo history and the current position in it.
    history: Vec<MetaValues>,
    at: usize,
    restoring: bool,
    on_save: SaveFn,
    scroll: ScrollHandle,
    _subs: Vec<Subscription>,
}

impl MetaForm {
    fn new(spec: MetaSpec, on_save: SaveFn, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let me = cx.entity().downgrade();
        OPEN_FORM.with(|f| *f.borrow_mut() = me);
        let v = spec.values.clone();
        let text = |s: &str, window: &mut Window, cx: &mut Context<Self>| {
            let s = s.to_string();
            cx.new(|cx| InputState::new(window, cx).default_value(s))
        };
        let opt = |s: &Option<String>| s.clone().unwrap_or_default();
        let name = text(&v.name, window, cx);
        let description = {
            let d = v.description.clone();
            cx.new(|cx| TextareaState::new(window, cx).rows(2).default_value(d))
        };
        let part_number = text(&v.part_number, window, cx);
        let revision = text(&v.revision, window, cx);
        let vendor = text(&opt(&v.meta.vendor), window, cx);
        let project = text(&opt(&v.meta.project), window, cx);
        let product_line = text(&opt(&v.meta.product_line), window, cx);
        let titles = [text(&opt(&v.meta.title1), window, cx), text(&opt(&v.meta.title2), window, cx), text(&opt(&v.meta.title3), window, cx)];
        let drawing = v.drawing.as_ref().map(|d| [text(&d[0], window, cx), text(&d[1], window, cx), text(&d[2], window, cx), text(&d[3], window, cx)]);
        let mass = {
            let m = v.mass_override.clone().unwrap_or_default();
            cx.new(|cx| InputState::new(window, cx).placeholder("Computed value").default_value(m))
        };
        let mut subs = Vec::new();
        for st in [&name, &part_number, &revision, &vendor, &project, &product_line, &titles[0], &titles[1], &titles[2], &mass].into_iter().chain(drawing.iter().flatten()) {
            subs.push(cx.subscribe(st, |this: &mut Self, _, ev: &InputEvent, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.changed(cx);
                }
            }));
        }
        subs.push(cx.subscribe(&description, |this: &mut Self, _, ev: &InputEvent, cx| {
            if matches!(ev, InputEvent::Change) {
                this.changed(cx);
            }
        }));
        let mass_on = v.mass_override.is_some();
        if !mass_on {
            mass.update(cx, |s, cx| s.set_disabled(true, cx));
        }
        use noshape_ops::custom_properties::{self as cp, CustomKind};
        let defs = cp::CustomTarget::of_category(spec.category).map(cp::active_for).unwrap_or_default();
        let mut custom_rest = v.meta.custom.clone();
        let mut custom = Vec::new();
        for d in defs {
            let value = custom_rest.remove(&d.name).unwrap_or_else(|| d.default.clone());
            let input = match &d.kind {
                CustomKind::Boolean => CustomInput::Bool(value.eq_ignore_ascii_case("true")),
                CustomKind::List(_) => CustomInput::List(Some(value).filter(|s| !s.is_empty())),
                _ => {
                    let st = text(&value, window, cx);
                    subs.push(cx.subscribe(&st, |this: &mut Self, _, ev: &InputEvent, cx| {
                        if matches!(ev, InputEvent::Change) {
                            this.changed(cx);
                        }
                    }));
                    CustomInput::Text(st)
                }
            };
            custom.push((d, input));
        }
        Self {
            on_release: None,
            reset_to: None,
            configured: Vec::new(),
            read_only: Vec::new(),
            spec_category: spec.category,
            card: None,
            multi: None,
            focus: cx.focus_handle(),
            check_focus: [cx.focus_handle().tab_stop(true), cx.focus_handle().tab_stop(true)],
            edited: Vec::new(),
            material: spec.material,
            computed_mass: spec.computed_mass,
            bom_options: spec.bom_options,
            name,
            description,
            part_number,
            revision,
            vendor,
            project,
            product_line,
            titles,
            drawing,
            drawing_info: spec.drawing_info,
            mass,
            state: v.meta.state.clone(),
            not_revision_managed: v.meta.not_revision_managed,
            exclude_from_bom: v.meta.exclude_from_bom,
            unit: v.meta.unit_of_measure.clone(),
            mass_on,
            mass_computed: false,
            mass_failed: false,
            mass_pending: false,
            bom: v.bom,
            custom,
            custom_rest,
            saved: v.clone(),
            history: vec![v],
            at: 0,
            restoring: false,
            on_save,
            scroll: ScrollHandle::new(),
            _subs: subs,
        }
    }

    /// The form's current values.
    pub fn values(&self, cx: &App) -> MetaValues {
        let t = |s: &Entity<InputState>| s.read(cx).value().trim().to_string();
        let o = |s: &Entity<InputState>| Some(s.read(cx).value().trim().to_string()).filter(|x| !x.is_empty());
        MetaValues {
            name: t(&self.name),
            description: self.description.read(cx).value().to_string(),
            part_number: t(&self.part_number),
            revision: t(&self.revision),
            meta: ItemMeta {
                state: self.state.clone(),
                vendor: o(&self.vendor),
                project: o(&self.project),
                product_line: o(&self.product_line),
                title1: o(&self.titles[0]),
                title2: o(&self.titles[1]),
                title3: o(&self.titles[2]),
                not_revision_managed: self.not_revision_managed,
                exclude_from_bom: self.exclude_from_bom,
                unit_of_measure: self.unit.clone(),
                custom: {
                    let mut m = self.custom_rest.clone();
                    for (d, input) in &self.custom {
                        let v = match input {
                            CustomInput::Text(st) => st.read(cx).value().trim().to_string(),
                            CustomInput::Bool(on) => if *on { "True".into() } else { "False".into() },
                            CustomInput::List(v) => v.clone().unwrap_or_default(),
                        };
                        if !v.is_empty() {
                            m.insert(d.name.clone(), v);
                        }
                    }
                    m
                },
            },
            mass_override: self.mass_on.then(|| t(&self.mass)),
            drawing: self.drawing.as_ref().map(|d| d.each_ref().map(|st| st.read(cx).value().trim().to_string())),
            bom: self.bom,
        }
    }

    /// Record an edit in the form's undo history.
    fn changed(&mut self, cx: &mut Context<Self>) {
        if self.restoring {
            return;
        }
        let v = self.values(cx);
        if self.history.get(self.at) != Some(&v) {
            self.history.truncate(self.at + 1);
            self.history.push(v);
            self.at = self.history.len() - 1;
        }
        cx.notify();
    }

    fn restore(&mut self, v: MetaValues, window: &mut Window, cx: &mut Context<Self>) {
        self.restoring = true;
        let set = |st: &Entity<InputState>, s: String, window: &mut Window, cx: &mut Context<Self>| st.update(cx, |st, cx| st.set_value(s, window, cx));
        set(&self.name, v.name.clone(), window, cx);
        self.description.update(cx, |st, cx| st.set_value(v.description.clone(), window, cx));
        set(&self.part_number, v.part_number.clone(), window, cx);
        set(&self.revision, v.revision.clone(), window, cx);
        set(&self.vendor, v.meta.vendor.clone().unwrap_or_default(), window, cx);
        set(&self.project, v.meta.project.clone().unwrap_or_default(), window, cx);
        set(&self.product_line, v.meta.product_line.clone().unwrap_or_default(), window, cx);
        set(&self.titles[0], v.meta.title1.clone().unwrap_or_default(), window, cx);
        set(&self.titles[1], v.meta.title2.clone().unwrap_or_default(), window, cx);
        set(&self.titles[2], v.meta.title3.clone().unwrap_or_default(), window, cx);
        if let (Some(d), Some(dv)) = (&self.drawing, &v.drawing) {
            for (st, val) in d.iter().zip(dv.iter()) {
                set(st, val.clone(), window, cx);
            }
        }
        set(&self.mass, v.mass_override.clone().unwrap_or_default(), window, cx);
        self.state = v.meta.state.clone();
        self.not_revision_managed = v.meta.not_revision_managed;
        self.exclude_from_bom = v.meta.exclude_from_bom;
        self.unit = v.meta.unit_of_measure.clone();
        self.mass_on = v.mass_override.is_some();
        let on = self.mass_on;
        self.mass.update(cx, |s, cx| s.set_disabled(!on, cx));
        self.bom = v.bom;
        for (d, input) in &mut self.custom {
            let value = v.meta.custom.get(&d.name).cloned().unwrap_or_default();
            match input {
                CustomInput::Text(st) => st.update(cx, |st, cx| st.set_value(value, window, cx)),
                CustomInput::Bool(on) => *on = value.eq_ignore_ascii_case("true"),
                CustomInput::List(cur) => *cur = Some(value).filter(|s| !s.is_empty()),
            }
        }
        self.restoring = false;
        cx.notify();
    }

    fn undo(&mut self, redo: bool, window: &mut Window, cx: &mut Context<Self>) {
        let to = if redo { self.at + 1 } else { self.at.wrapping_sub(1) };
        let Some(v) = self.history.get(to).cloned() else { return };
        self.at = to;
        self.restore(v, window, cx);
    }

    fn dirty(&self, cx: &App) -> bool {
        let v = self.values(cx);
        if let Some(m) = &self.multi {
            return (m.reset || v != self.saved || m.mixed != m.mixed_at_fill) && (!v.name.is_empty() || m.mixed.iter().any(|f| f == "Name"));
        }
        v != self.saved && !v.name.is_empty()
    }

    /// The fields edited since the form was filled (a field left at "Multiple values" isn't).
    fn multi_edits(&self, cx: &App) -> Vec<(String, String)> {
        let Some(m) = &self.multi else { return Vec::new() };
        let v = self.values(cx);
        fields_of([&v, &self.saved].into_iter())
            .into_iter()
            .filter(|f| field_of(&v, f) != field_of(&self.saved, f) || (m.mixed_at_fill.contains(f) && !m.mixed.contains(f)))
            .filter_map(|f| Some((f.clone(), field_of(&v, &f)?)))
            .collect()
    }

    /// Fills the form with the common values of the list's selected rows (one row: that part's own).
    fn fill_multi(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(m) = &mut self.multi else { return };
        let parts: Vec<&MetaValues> = m.shown.iter().filter_map(|i| m.values.get(*i)).collect();
        let (v, mixed) = common_values(&parts);
        m.mixed_at_fill = mixed.clone();
        m.mixed = mixed;
        m.reset = false;
        self.saved = v.clone();
        self.history = vec![v.clone()];
        self.at = 0;
        self.restore(v, window, cx);
        self.set_hints(window, cx);
    }

    /// "Multiple values" in the empty fields whose values differ.
    fn set_hints(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mixed = self.multi.as_ref().map(|m| m.mixed.clone()).unwrap_or_default();
        let hint = |f: &str| if mixed.iter().any(|x| x == f) { MULTIPLE_VALUES } else { "" };
        for (f, st) in [("Name", &self.name), ("Part number", &self.part_number), ("Revision", &self.revision), ("Vendor", &self.vendor), ("Project", &self.project), ("Product line", &self.product_line), ("Title 1", &self.titles[0]), ("Title 2", &self.titles[1]), ("Title 3", &self.titles[2])] {
            st.update(cx, |s, cx| s.set_placeholder(hint(f), window, cx));
        }
        self.description.update(cx, |s, cx| s.set_placeholder(hint("Description"), window, cx));
        for (d, input) in &self.custom {
            if let CustomInput::Text(st) = input {
                let h = hint(&format!("custom:{}", d.name));
                st.update(cx, |s, cx| s.set_placeholder(h, window, cx));
            }
        }
        cx.notify();
    }

    /// A click on a row of the left list: that part alone; Ctrl (Cmd) toggles it, Shift selects the range from the
    /// last clicked row (live q2962 (d)). The Parts list's selection stays.
    fn click_row(&mut self, i: usize, modifiers: Modifiers, window: &mut Window, cx: &mut Context<Self>) {
        let Some(m) = &mut self.multi else { return };
        if modifiers.shift {
            let (a, b) = (m.anchor.min(i), m.anchor.max(i));
            m.shown = (a..=b).collect();
        } else if modifiers.secondary() {
            if let Some(k) = m.shown.iter().position(|x| *x == i) {
                if m.shown.len() > 1 {
                    m.shown.remove(k);
                }
            } else {
                m.shown.push(i);
                m.shown.sort_unstable();
            }
            m.anchor = i;
        } else {
            m.shown = vec![i];
            m.anchor = i;
        }
        self.fill_multi(window, cx);
    }

    /// Several parts' Reset all: the form shows the shown parts' reset values; Save writes them.
    fn reset_multi(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(m) = &mut self.multi else { return };
        let parts: Vec<&MetaValues> = m.shown.iter().filter_map(|i| m.reset_to.get(*i)).collect();
        let (v, mixed) = common_values(&parts);
        m.mixed = mixed;
        m.reset = true;
        self.restore(v, window, cx);
        self.set_hints(window, cx);
        self.changed(cx);
    }

    /// Reset all is enabled: some value isn't the one it would restore (live dk-999, part-studio/1840; several parts: a
    /// part has an edited property, q2962).
    fn can_reset(&self, cx: &App) -> bool {
        match &self.multi {
            Some(m) => m.reset || m.shown.iter().any(|&i| m.values.get(i) != m.reset_to.get(i)),
            None => self.reset_to.as_ref().is_some_and(|r| *r != self.values(cx)),
        }
    }

    /// The fields whose value isn't the saved one (a mixed checkbox clicked counts): they get the orange corner.
    fn edited_fields(&self, cx: &App) -> Vec<String> {
        let v = self.values(cx);
        let unmixed = |f: &str| self.multi.as_ref().is_some_and(|m| m.mixed_at_fill.iter().any(|x| x == f) && !m.mixed.iter().any(|x| x == f));
        fields_of([&v, &self.saved].into_iter()).into_iter().filter(|f| field_of(&v, f) != field_of(&self.saved, f) || unmixed(f)).collect()
    }

    /// The fields marked edited as last drawn.
    pub fn edited_marks(&self) -> Vec<String> {
        self.edited.clone()
    }

    fn is_edited(&self, f: &str) -> bool {
        self.edited.iter().any(|x| x == f)
    }

    /// Whether a field (any text input) has the keyboard, not the modal itself.
    pub fn a_field_has_focus(&self, window: &Window, cx: &App) -> bool {
        let fields = [&self.name, &self.part_number, &self.revision, &self.vendor, &self.project, &self.product_line, &self.titles[0], &self.titles[1], &self.titles[2]];
        fields.iter().any(|f| f.read(cx).focus_handle(cx).is_focused(window)) || self.description.read(cx).focus_handle(cx).is_focused(window)
    }

    /// The text field with the keyboard, by its label (tests: the Tab order).
    pub fn focused_field(&self, window: &Window, cx: &App) -> Option<&'static str> {
        let fields = [("Name", &self.name), ("Part number", &self.part_number), ("Revision", &self.revision), ("Vendor", &self.vendor), ("Project", &self.project), ("Product line", &self.product_line), ("Title 1", &self.titles[0]), ("Title 2", &self.titles[1]), ("Title 3", &self.titles[2])];
        if self.description.read(cx).focus_handle(cx).is_focused(window) {
            return Some("Description");
        }
        if let Some(i) = self.check_focus.iter().position(|h| h.is_focused(window)) {
            return Some(["Not revision managed", "Exclude from all BOMs"][i]);
        }
        fields.iter().find(|(_, f)| f.read(cx).focus_handle(cx).is_focused(window)).map(|(l, _)| *l)
    }

    /// Whether the modal itself has the keyboard.
    pub fn has_focus(&self, window: &Window) -> bool {
        self.focus.is_focused(window)
    }

    /// Fields whose parts differ, shown "Multiple values" (several parts).
    pub fn mixed_fields(&self) -> Vec<String> {
        self.multi.as_ref().map(|m| m.mixed.clone()).unwrap_or_default()
    }

    /// The left list's rows and whether each is selected (several parts).
    pub fn list_rows(&self) -> Vec<(String, bool)> {
        self.multi.as_ref().map(|m| m.names.iter().enumerate().map(|(i, n)| (n.clone(), m.shown.contains(&i))).collect()).unwrap_or_default()
    }

    /// Puts the keyboard in a text field by its label ("Description", "Vendor").
    pub fn focus_field(&self, label: &str, window: &mut Window, cx: &mut App) {
        if label == "Description" {
            self.description.update(cx, |s, cx| s.focus(window, cx));
            return;
        }
        let st = match label {
            "Name" => &self.name,
            "Part number" => &self.part_number,
            "Revision" => &self.revision,
            "Vendor" => &self.vendor,
            "Project" => &self.project,
            "Product line" => &self.product_line,
            _ => return,
        };
        st.update(cx, |s, cx| s.focus(window, cx));
    }

    /// The footer's Apply (`close` false) or Save.
    pub fn press_save(&mut self, close: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.save(close, window, cx);
    }

    /// A click on the left list's row `i` with `modifiers`.
    pub fn press_row(&mut self, i: usize, modifiers: Modifiers, window: &mut Window, cx: &mut Context<Self>) {
        self.click_row(i, modifiers, window, cx);
    }

    /// The header over the form: the part's name, or "2 selected".
    pub fn header(&self) -> String {
        match &self.multi {
            Some(m) if m.shown.len() > 1 => format!("{} selected", m.shown.len()),
            _ => self.saved.name.clone(),
        }
    }

    /// Several parts' Apply / Save: the edited fields onto each shown part's values (after its reset values when
    /// Reset all was clicked).
    fn save_multi(&mut self, close: bool, window: &mut Window, cx: &mut Context<Self>) {
        let edits = self.multi_edits(cx);
        let Some(m) = &mut self.multi else { return };
        let mut out = Vec::new();
        for &i in &m.shown {
            let (Some(old), Some(reset)) = (m.values.get(i), m.reset_to.get(i)) else { continue };
            let mut v = if m.reset { reset.clone() } else { old.clone() };
            for (f, x) in &edits {
                set_field(&mut v, f, x.clone());
            }
            m.values[i] = v.clone();
            out.push((i, v));
        }
        let on_save = m.on_save.clone();
        on_save(out, window, cx);
        if close {
            window.close_dialog(cx);
            return;
        }
        self.fill_multi(window, cx);
    }

    /// Apply (keep the modal open) or Save (close it).
    fn save(&mut self, close: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dirty(cx) {
            return;
        }
        if self.multi.is_some() {
            self.save_multi(close, window, cx);
            return;
        }
        let v = self.values(cx);
        self.saved = v.clone();
        (self.on_save)(v, window, cx);
        if close {
            window.close_dialog(cx);
        }
        cx.notify();
    }
}

/// A bold 14 px label, with a red "*" for required fields.
pub fn label(text: impl Into<SharedString>, required: bool) -> Div {
    let text: SharedString = text.into();
    h_flex()
        .h(px(21.))
        .gap(px(3.))
        .text_size(px(14.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(0x333333))
        .child(text)
        .when(required, |d| d.child(div().text_color(rgb(0xaa252d)).child("*")))
}

/// A disabled-looking field (`#eee`, `#ccc` border) with an optional caret.
pub fn gray_box(text: String, caret: bool) -> Div {
    h_flex()
        .h(px(34.))
        .w_full()
        .pl(px(10.))
        .pr(px(10.))
        .rounded(px(3.))
        .border_1()
        .border_color(rgb(0xcccccc))
        .bg(rgb(0xeeeeee))
        .text_size(px(14.))
        .text_color(rgb(0x333333))
        .child(div().flex_1().truncate().child(text))
        .when(caret, |d| d.child(crate::os_ui::glyph(crate::os_ui::GLYPH_CARET, 8., rgb(0x666666))))
}

fn text_field(st: &Entity<InputState>) -> AnyElement {
    Input::new(st).h(px(34.)).text_size(px(14.)).into_any_element()
}

/// A tab's Properties fields after Name, Description and Category, in the modal's order, as (label, kind, value) with kind
/// "input", "select" or "checkbox": what a version's Properties tree shows, read-only, for a tab picked in it (live
/// ver-version-properties-tree). The same fields by category as the tab's own modal.
pub fn tab_fields(spec: &MetaSpec) -> Vec<(&'static str, &'static str, String)> {
    let kind = spec.category;
    let (v, m) = (&spec.values, &spec.values.meta);
    let text = |s: &Option<String>| s.clone().unwrap_or_default();
    let mut out = Vec::new();
    if !matches!(kind, "Onshape Part Studio" | "Onshape Feature Studio") {
        let state = m.state.clone().unwrap_or_else(|| RELEASE_STATES[0].to_string());
        out.extend([("Part number", "input", v.part_number.clone()), ("Revision", "input", v.revision.clone()), ("State *", "select", state)]);
    }
    if !matches!(kind, "Onshape Part Studio" | "Onshape Feature Studio" | "Onshape Variable Studio" | "Onshape Drawing") {
        out.extend([("Vendor", "input", text(&m.vendor)), ("Project", "input", text(&m.project)), ("Product line", "input", text(&m.product_line))]);
    }
    if kind != "Onshape Feature Studio" {
        out.extend([("Title 1", "input", text(&m.title1)), ("Title 2", "input", text(&m.title2)), ("Title 3", "input", text(&m.title3))]);
        if let Some(d) = &v.drawing {
            out.extend(DRAWING_FIELDS.iter().zip(d.iter()).map(|(l, x)| (*l, "input", x.clone())));
        }
        out.push(("Not revision managed", "checkbox", m.not_revision_managed.to_string()));
        if let Some(info) = &spec.drawing_info {
            out.extend(DRAWING_INFO.iter().zip(info.iter()).map(|(l, x)| (*l, "input", x.clone())));
        }
    }
    if !matches!(kind, "Onshape Part Studio" | "Onshape Feature Studio" | "Onshape Variable Studio" | "Onshape File" | "Onshape Drawing") {
        out.push(("Exclude from all BOMs", "checkbox", m.exclude_from_bom.to_string()));
        out.push(("Unit of measure", "select", m.unit_of_measure.clone().unwrap_or_else(|| DEFAULT_UNIT.to_string())));
        if let Some(options) = &spec.bom_options {
            out.push(("Override", "checkbox", v.mass_override.is_some().to_string()));
            out.push(("Mass", "input", v.mass_override.clone().unwrap_or_default()));
            out.push(("Subassembly BOM behavior *", "select", options.get(v.bom).cloned().unwrap_or_default()));
        }
    }
    out
}

/// A read-only checkbox row (a version's Properties tree).
pub fn readonly_check(text: impl Into<SharedString>, on: bool) -> AnyElement {
    let text: SharedString = text.into();
    check_row(SharedString::from(format!("ro-check-{text}")), text, on).cursor_default().opacity(0.6).into_any_element()
}

/// A configured field: disabled, grey, with the hover note (live psb
/// 10-20 q, 645–646).
fn configured_box(field: &'static str, value: String) -> AnyElement {
    div().id(SharedString::from(format!("meta-configured-{field}"))).w_full().child(gray_box(value, false)).tooltip(crate::ui::tooltip(format!("{field} is configured. Its configured values can be viewed in the configured properties table."))).into_any_element()
}

impl MetaForm {
    /// The interaction suite's `state.dialog` for this modal, in live's recorder's form: its inputs, selects and checkboxes
    /// by label (required ones with " *"), in order. Live's reader takes Category's typeahead input (empty) and leaves the
    /// Description textarea out.
    fn note_ui_state(&self, numbered: bool, sourced: bool, titled: bool, item: bool, cx: &App) {
        crate::ui_script::note_dialog(|| {
            let text = |st: &Entity<InputState>| st.read(cx).value().to_string();
            // Live's recorder (2026-11-13.1): a field's placeholder while it is empty ("Multiple values"), greyed fields
            // `disabled`, a mixed checkbox `indeterminate`.
            let input = |label: &str, value: String| serde_json::json!({ "label": label, "kind": "input", "value": value });
            let typed = |label: &str, f: &str, st: &Entity<InputState>| {
                let mut v = input(label, text(st));
                if text(st).is_empty() && self.mixed(f) {
                    v["placeholder"] = MULTIPLE_VALUES.into();
                }
                v
            };
            let greyed = |mut v: serde_json::Value| {
                v["disabled"] = true.into();
                v
            };
            let check = |label: &str, on: bool| {
                let mut v = serde_json::json!({ "label": label, "kind": "checkbox", "value": on });
                if self.mixed(label) {
                    v["indeterminate"] = true.into();
                }
                v
            };
            let mut fields = vec![typed("Name *", "Name", &self.name), greyed(input("Category", String::new()))];
            if numbered {
                let state = self.state.clone().unwrap_or_else(|| RELEASE_STATES[0].to_string());
                fields.extend([typed("Part number", "Part number", &self.part_number), typed("Revision", "Revision", &self.revision), greyed(serde_json::json!({ "label": "State *", "kind": "select", "value": state }))]);
            }
            if sourced {
                fields.extend([typed("Vendor", "Vendor", &self.vendor), typed("Project", "Project", &self.project), typed("Product line", "Product line", &self.product_line)]);
            }
            if let Some(m) = &self.material {
                fields.push(greyed(input("Material", m.clone())));
            }
            if titled {
                fields.extend([typed("Title 1", "Title 1", &self.titles[0]), typed("Title 2", "Title 2", &self.titles[1]), typed("Title 3", "Title 3", &self.titles[2])]);
                if let Some(d) = &self.drawing {
                    fields.extend(DRAWING_FIELDS.iter().zip(d.iter()).map(|(l, st)| input(l, text(st))));
                }
                fields.push(check("Not revision managed", self.not_revision_managed));
                if let Some(info) = &self.drawing_info {
                    fields.extend(DRAWING_INFO.iter().zip(info.iter()).map(|(l, v)| input(l, v.clone())));
                }
            }
            if item {
                fields.push(check("Exclude from all BOMs", self.exclude_from_bom));
                fields.push(serde_json::json!({ "label": "Unit of measure", "kind": "select", "value": self.unit.clone().unwrap_or_else(|| DEFAULT_UNIT.to_string()) }));
                if let Some(options) = &self.bom_options {
                    // An assembly (live doc-dialog-properties-assembly…, 11-08 recorder): Override, Mass, then
                    // Subassembly BOM behavior.
                    fields.push(check("Override", self.mass_on));
                    fields.push(input("Mass", if self.mass_on { text(&self.mass) } else { String::new() }));
                    fields.push(serde_json::json!({ "label": "Subassembly BOM behavior *", "kind": "select", "value": options.get(self.bom).cloned().unwrap_or_default() }));
                } else if self.material.is_some() || self.computed_mass.is_some() {
                    let mut v = check("Mass Override", self.mass_on);
                    if self.mixed("Override") {
                        v["indeterminate"] = true.into();
                    }
                    fields.push(v);
                }
            }
            // Undo and Redo are icon buttons (no text).
            let mut buttons = vec!["", ""];
            let mut disabled: Vec<&str> = [(self.at == 0, ""), (self.at + 1 >= self.history.len(), "")].into_iter().filter(|(off, _)| *off).map(|(_, b)| b).collect();
            if self.reset_to.is_some() {
                buttons.push("Reset all");
                if !self.can_reset(cx) {
                    disabled.push("Reset all");
                }
            }
            buttons.extend(["Apply", "Save", "Close"]);
            if !self.dirty(cx) {
                disabled.extend(["Apply", "Save"]);
            }
            serde_json::json!({ "kind": "modal", "title": "Properties", "fields": fields, "buttons": buttons, "disabledButtons": disabled })
        });
    }

    /// A text field, or its configured box.
    /// Whether a checkbox's parts differ (shown indeterminate).
    fn mixed(&self, f: &str) -> bool {
        self.multi.as_ref().is_some_and(|m| m.mixed.iter().any(|x| x == f))
    }

    /// A checkbox clicked: a mixed one turns on for every part, otherwise it toggles.
    fn toggle_check(&mut self, f: &str, on: bool) -> bool {
        match self.multi.as_mut().and_then(|m| m.mixed.iter().position(|x| x == f).map(|k| (m, k))) {
            Some((m, k)) => {
                m.mixed.remove(k);
                true
            }
            None => !on,
        }
    }

    fn field(&self, name: &'static str, st: &Entity<InputState>, cx: &App) -> AnyElement {
        if self.configured.contains(&name) {
            return configured_box(name, st.read(cx).value().to_string());
        }
        if self.read_only.contains(&name) {
            return gray_box(st.read(cx).value().to_string(), false).into_any_element();
        }
        // The scenario runner's `{"field": "<label>"}`.
        edited_corner(text_field(st), self.is_edited(name)).map(|d| crate::ui_script::mark(d, || format!("field:{name}"))).into_any_element()
    }
}

/// `el` with the orange corner of an edited field or row at its top left when `on` (live q2962: 15 px, 2 px in).
fn edited_corner(el: impl IntoElement, on: bool) -> Div {
    div()
        .relative()
        .w_full()
        .child(el)
        .when(on, |d| d.child(div().absolute().top(px(2.)).left(px(2.)).child(crate::os_ui::glyph(crate::os_ui::GLYPH_DIRTY_CORNER, 15., rgb(0xfab603)))))
}

/// A Bootstrap checkbox row ("☐ Not revision managed").
fn check_row(id: impl Into<ElementId>, text: impl Into<SharedString>, on: bool) -> Stateful<Div> {
    check_row_mixed(id, text, on, false)
}

/// [`check_row`], indeterminate (a blue box with a white dash) when `mixed`.
fn check_row_mixed(id: impl Into<ElementId>, text: impl Into<SharedString>, on: bool, mixed: bool) -> Stateful<Div> {
    let on = on || mixed;
    let text: SharedString = text.into();
    h_flex()
        .id(id)
        .h(px(21.))
        .gap(px(6.))
        .cursor_pointer()
        .text_size(px(14.))
        .text_color(rgb(0x333333))
        .child(
            div()
                .size(px(14.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.5))
                .border_1()
                .border_color(if on { rgb(0x1651b0) } else { rgb(0xaaaaaa) })
                .bg(if on { rgb(0x1651b0) } else { rgb(0xffffff) })
                .when(mixed, |d| d.child(div().w(px(8.)).h(px(2.)).bg(rgb(0xffffff))))
                .when(on && !mixed, |d| d.child(gpui_kit::component::Icon::new(gpui_kit::component::IconName::Check).size(px(11.)).text_color(gpui_kit::rgb_static(0xffffff)))),
        )
        .child(text)
}

/// A footer button: Apply / Save (`#1651b0`, 40 % until edited) or Close (`#d7d7d7`).
pub fn footer_button(id: &'static str, text: &'static str, primary: bool, enabled: bool) -> Stateful<Div> {
    div()
        .id(id)
        .map(|d| crate::ui_script::mark(d, || format!("button:{text}")))
        .h(px(34.))
        .px(px(10.5))
        .flex()
        .items_center()
        .rounded(px(5.25))
        .text_size(px(14.))
        .when(primary, |d| d.bg(rgb(0x1651b0)).text_color(gpui_kit::rgb_static(0xffffff)))
        // Close stays light in the dark theme (live dark/dk-999).
        .when(!primary, |d| d.bg(gpui_kit::rgb_static(0xd7d7d7)).text_color(gpui_kit::rgb_static(0x333333)))
        .when(!enabled, |d| d.opacity(0.4))
        .when(enabled, |d| d.cursor_pointer())
        .child(text)
}

impl Render for MetaForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pal = p();
        let dirty = self.dirty(cx);
        self.edited = self.edited_fields(cx);
        let view = cx.entity();
        let group = |l: Div, f: AnyElement| v_flex().w_full().gap(px(0.)).child(l).child(div().p(px(1.)).child(f));
        let state_ix = self.state.as_deref().and_then(|s| RELEASE_STATES.iter().position(|x| *x == s)).unwrap_or(0);
        let unit = self.unit.clone().unwrap_or_else(|| DEFAULT_UNIT.to_string());
        let unit_ix = UNITS_OF_MEASURE.iter().position(|u| *u == unit).unwrap_or(0);
        let release = self.on_release.clone().map(|f| {
            h_flex().child(
                div()
                    .id("props-release")
                    .size(px(26.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(3.))
                    .cursor_pointer()
                    .hover(|d| d.bg(gpui_kit::rgb(0xeaeaea)))
                    .child(crate::assets::icon("release-create", 20., cx))
                    .tooltip(crate::ui::tooltip("Create release candidate"))
                    .on_click(move |_, window, cx| {
                        window.close_dialog(cx);
                        f(window, cx)
                    }),
            )
        });
        // Live 1738 (Tab › Properties…): the fields a tab kind has, by its
        // category. Parts, assemblies and drawings have them all.
        let kind = self.spec_category;
        let numbered = !matches!(kind, "Onshape Part Studio" | "Onshape Feature Studio");
        // A drawing has its own fields in their place (live doc-tab-properties-drawing).
        let sourced = !matches!(kind, "Onshape Part Studio" | "Onshape Feature Studio" | "Onshape Variable Studio" | "Onshape Drawing");
        let titled = kind != "Onshape Feature Studio";
        let item = !matches!(kind, "Onshape Part Studio" | "Onshape Feature Studio" | "Onshape Variable Studio" | "Onshape File" | "Onshape Drawing");
        self.note_ui_state(numbered, sourced, titled, item, cx);
        let mut form = v_flex()
            .w_full()
            .gap(px(15.))
            .children(release)
            .child(group(label("Name", true), self.field("Name", &self.name, cx)))
            .child(group(
                label("Description", false),
                if self.configured.contains(&"Description") {
                    configured_box("Description", self.description.read(cx).value().to_string())
                } else if self.read_only.contains(&"Description") {
                    gray_box(self.description.read(cx).value().to_string(), false).h(px(56.)).into_any_element()
                } else {
                    // Tab moves on as from a browser's textarea (live: Name, Description, Part number …), not indents.
                    div()
                        .capture_action(|_: &IndentInline, window, cx| {
                            window.focus_next(cx);
                            cx.stop_propagation();
                        })
                        .capture_action(|_: &OutdentInline, window, cx| {
                            window.focus_prev(cx);
                            cx.stop_propagation();
                        })
                        .child(edited_corner(Textarea::new(&self.description).h(px(56.)).text_size(px(14.)), self.is_edited("Description")).map(|d| crate::ui_script::mark(d, || "field:Description".into())))
                        .into_any_element()
                },
            ))
            .child(group(label("Category", false), gray_box(self.spec_category.to_string(), true).into_any_element()));
        if numbered {
            form = form
                .child(group(label("Part number", false), self.field("Part number", &self.part_number, cx)))
                .child(group(label("Revision", false), self.field("Revision", &self.revision, cx)))
                // State is set by the release workflow (a greyed select).
                .child(group(label("State", true), gray_box(RELEASE_STATES[state_ix].to_string(), true).into_any_element()));
        }
        if sourced {
            form = form
                .child(group(label("Vendor", false), self.field("Vendor", &self.vendor, cx)))
                .child(group(label("Project", false), self.field("Project", &self.project, cx)))
                .child(group(label("Product line", false), self.field("Product line", &self.product_line, cx)));
        }
        if let Some(m) = self.material.clone() {
            form = form.child(group(label("Material", false), gray_box(m, false).into_any_element()));
        }
        if titled {
            form = form
                .child(group(label("Title 1", false), self.field("Title 1", &self.titles[0], cx)))
                .child(group(label("Title 2", false), self.field("Title 2", &self.titles[1], cx)))
                .child(group(label("Title 3", false), self.field("Title 3", &self.titles[2], cx)));
            if let Some(d) = self.drawing.clone() {
                for (l, st) in DRAWING_FIELDS.iter().zip(d.iter()) {
                    form = form.child(group(label(*l, false), self.field(l, st, cx)));
                }
            }
            form = form.child(if self.read_only.contains(&"Not revision managed") {
                readonly_check("Not revision managed", self.not_revision_managed)
            } else {
                edited_corner(
                    check_row_mixed("meta-nrm", "Not revision managed", self.not_revision_managed, self.mixed("Not revision managed")).track_focus(&self.check_focus[0]).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.not_revision_managed = this.toggle_check("Not revision managed", this.not_revision_managed);
                        this.changed(cx);
                    })),
                    self.is_edited("Not revision managed"),
                )
                .into_any_element()
            });
            if let Some(info) = self.drawing_info.clone() {
                for (l, v) in DRAWING_INFO.iter().zip(info) {
                    form = form.child(group(label(*l, false), gray_box(v, false).into_any_element()));
                }
            }
        }
        if item {
        form = form
            .child(edited_corner(
                check_row_mixed("meta-bom", "Exclude from all BOMs", self.exclude_from_bom, self.mixed("Exclude from all BOMs")).track_focus(&self.check_focus[1]).on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.exclude_from_bom = this.toggle_check("Exclude from all BOMs", this.exclude_from_bom);
                    this.changed(cx);
                })),
                self.is_edited("Exclude from all BOMs"),
            ))
            .child(group(
                label("Unit of measure", false),
                crate::os_ui::select_field("meta-unit", UNITS_OF_MEASURE.iter().map(|s| s.to_string()).collect(), unit_ix, {
                    let v = view.clone();
                    move |i, _, cx| {
                        v.update(cx, |this, cx| {
                            this.unit = Some(UNITS_OF_MEASURE[i].to_string());
                            this.changed(cx);
                        })
                    }
                })
                .into_any_element(),
            ));
        // Parts (with their material) and assemblies (a computed mass, no
        // material; live assembly/726; from the tab's menu too) have the Mass row.
        if self.material.is_some() || self.computed_mass.is_some() || self.bom_options.is_some() {
            let mass_on = self.mass_on;
            let computed = self.computed_mass.clone();
            form = form.child(
                v_flex()
                    .w_full()
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .child(label("Mass", false))
                            .child(svg().path(crate::assets::icon_path("function")).size(px(14.)).text_color(rgb(0x333333)))
                            .when(self.mass_failed, |d| d.child(div().text_size(px(14.)).text_color(rgb(0xaa252d)).child("Computation failed")))
                            .child(div().w(px(10.)))
                            .child(if self.read_only.contains(&"Override") {
                                readonly_check("Override", mass_on)
                            } else {
                                check_row_mixed("meta-mass", "Override", mass_on, self.mixed("Override"))
                                    .font_weight(FontWeight::NORMAL)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.mass_on = this.toggle_check("Override", this.mass_on);
                                        let on = this.mass_on;
                                        this.mass.update(cx, |s, cx| s.set_disabled(!on, cx));
                                        this.changed(cx);
                                    }))
                                    .into_any_element()
                            }),
                    )
                    // Live (assembly/132): one 34 px group, the grey field
                    // (#ccc outline, square right corners) running into the
                    // 36 px calculator button, whose #aaa outline divides them;
                    // not overridden, the field is #eee with a #ccc outline.
                    .child(
                        h_flex()
                            .p(px(1.))
                            .child({
                                let joined = !self.mass_computed;
                                let input = Input::new(&self.mass)
                                    .h(px(34.))
                                    .text_size(px(14.))
                                    .when(joined, |i| i.rounded_r(px(0.)))
                                    .when(!mass_on, |i| i.bg(rgb(0xeeeeee)).border_color(rgb(0xcccccc)));
                                div().flex_1().child(input)
                            })
                            .when(!self.mass_computed, |d| d.child(
                                div()
                                    .id("meta-calc")
                                    .ml(px(-1.))
                                    .w(px(36.))
                                    .h(px(34.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .border_1()
                                    .border_color(rgb(0xaaaaaa))
                                    .rounded_r(px(3.5))
                                    .bg(rgb(0xffffff))
                                    .cursor_pointer()
                                    .tooltip(crate::ui::tooltip("Calculate value"))
                                    .map(|d| crate::ui_script::mark(d, || "button:Calculate value".into()))
                                    .child(svg().path(crate::assets::icon_path("calculate-button")).size(px(20.)).text_color(rgb(0x333333)))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                        // Without a material Onshape reports "Computation
                                        // failed" / "No material assigned." and hides the button.
                                        this.mass_computed = true;
                                        if this.mass_pending {
                                            this.mass.update(cx, |s, cx| s.set_placeholder("Calculating…", window, cx));
                                            cx.notify();
                                            return;
                                        }
                                        let text = computed.clone().unwrap_or_else(|| "No material assigned.".into());
                                        this.mass_failed = computed.is_none();
                                        this.mass.update(cx, |s, cx| s.set_placeholder(text, window, cx));
                                        cx.notify();
                                    })),
                            )),
                    ),
            );
        }
        if let Some(options) = self.bom_options.clone() {
            let v = view.clone();
            form = form.child(group(
                label("Subassembly BOM behavior", true),
                crate::os_ui::select_field("meta-bom-behavior", options, self.bom, move |i, _, cx| {
                    v.update(cx, |this, cx| {
                        this.bom = i;
                        this.changed(cx);
                    })
                })
                .into_any_element(),
            ));
        }
        }
        // Custom properties at the bottom (help Properties: "Custom
        // properties are shown at the bottom of the dialog").
        for (i, (d, input)) in self.custom.iter().enumerate() {
            let field: AnyElement = match input {
                CustomInput::Text(st) => text_field(st),
                CustomInput::Bool(on) => check_row(("meta-custom-check", i), d.name.clone(), *on)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if let Some((_, CustomInput::Bool(on))) = this.custom.get_mut(i) {
                            *on = !*on;
                        }
                        this.changed(cx);
                    }))
                    .into_any_element(),
                CustomInput::List(cur) => {
                    let noshape_ops::custom_properties::CustomKind::List(options) = &d.kind else { continue };
                    let mut items = vec![String::new()];
                    items.extend(options.iter().cloned());
                    let at = cur.as_ref().and_then(|c| items.iter().position(|o| o == c)).unwrap_or(0);
                    let v = view.clone();
                    let opts = items.clone();
                    crate::os_ui::select_field(("meta-custom-list", i), items, at, move |k, _, cx| {
                        let chosen = Some(opts[k].clone()).filter(|s| !s.is_empty());
                        v.update(cx, |this, cx| {
                            if let Some((_, CustomInput::List(c))) = this.custom.get_mut(i) {
                                *c = chosen.clone();
                            }
                            this.changed(cx);
                        })
                    })
                    .into_any_element()
                }
            };
            form = match input {
                CustomInput::Bool(_) => form.child(field),
                _ => form.child(group(label(d.name.clone(), false), field)),
            };
        }
        let (can_undo, can_redo) = (self.at > 0, self.at + 1 < self.history.len());
        let undo_button = |id: &'static str, icon: &'static str, tip: &'static str, on: bool| {
            div()
                .id(id)
                .size(px(30.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .when(!on, |d| d.opacity(0.25))
                .when(on, |d| d.cursor_pointer().hover(|d| d.bg(rgb(0xeeeeee))))
                .tooltip(crate::ui::tooltip(tip))
                .child(svg().path(crate::assets::icon_path(icon)).size(px(20.)).text_color(rgb(0x333333)))
        };
        let title = self.header();
        // (Beside several parts' list: 41 px from the rule above, the name in its middle, live q2962.)
        let header_row = move |beside_list: bool| {
            div()
                .mt(px(if beside_list { 0. } else { 10. }))
                .h(px(if beside_list { 41. } else { 31. }))
                .when(beside_list, |d| d.flex().items_center())
                .px(px(15.))
                .border_b_1()
                .border_color(rgb(0xcccccc))
                .child(div().pl(px(4.)).text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(rgb(0x333333)).truncate().child(title.clone()))
        };
        let (header, form_header) = (header_row(false), header_row(true));
        v_flex()
            .w_full()
            .track_focus(&self.focus)
            // The kit dialog puts 8 px between title and body (several parts: their title is 8 px shorter instead).
            .when(self.multi.is_none(), |d| d.mt(px(-8.)))
            .text_color(pal.text)
            // The item's name under the title, over a #ccc rule (several parts: over the form, beside their list).
            .when(self.multi.is_none(), |d| d.child(header))
            .child({
                let scroll = div().id("meta-scroll").h(px(381. + props_size().1 - PROPS_MIN.1)).overflow_y_scroll().track_scroll(&self.scroll).p(px(15.)).child(form);
                match (&self.multi, &self.card) {
                    (Some(m), _) => v_flex()
                        .w_full()
                        .child(div().w_full().h(px(1.)).bg(rgb(0xdee2e6)))
                        .child(parts_list(m, dirty, v_flex().w_full().child(form_header).child(scroll), cx))
                        .into_any_element(),
                    (None, None) => scroll.into_any_element(),
                    (None, Some(card)) => {
                        let picture = match &card.thumb {
                            Some(image) => img(ImageSource::Render(image.clone())).w(px(70.)).h(px(40.)).object_fit(ObjectFit::Contain).into_any_element(),
                            None => crate::assets::icon("part-studio", 30., cx),
                        };
                        let row = h_flex()
                            .w_full()
                            .h(px(55.))
                            .pl(px(12.))
                            .pr(px(8.))
                            .border_b_1()
                            .border_color(rgb(0xdddddd))
                            .bg(rgb(0xb2ddf6))
                            .child(div().w(px(12.)).flex_none().child(crate::os_ui::glyph(crate::os_ui::GLYPH_COLLAPSED, 12., rgb(0x333333))))
                            .child(div().w(px(70.)).h(px(40.)).flex_none().flex().items_center().justify_center().child(picture))
                            .child(
                                v_flex()
                                    .pl(px(4.))
                                    .min_w_0()
                                    .flex_1()
                                    .child(div().text_size(px(13.33)).font_weight(FontWeight::MEDIUM).text_color(rgb(0x333333)).truncate().child(self.saved.name.clone()))
                                    .child(div().text_size(px(12.)).text_color(rgb(0x666666)).truncate().child(card.kind)),
                            );
                        h_flex()
                            .w_full()
                            .items_start()
                            .child(div().w(px(300.)).h(px(381. + props_size().1 - PROPS_MIN.1)).flex_none().bg(rgb(0xf5f5f5)).border_r_1().border_color(rgb(0xcccccc)).child(row))
                            .child(div().flex_1().min_w_0().child(scroll))
                            .into_any_element()
                    }
                }
            })
            .child(
                h_flex()
                    .h(px(62.))
                    .px(px(10.))
                    .border_t_1()
                    .border_color(rgb(0xdee2e6))
                    .child(
                        h_flex()
                            .gap(px(9.))
                            .pl(px(3.))
                            .child(undo_button("meta-undo", "undo", "Undo", can_undo).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.undo(false, window, cx))))
                            .child(undo_button("meta-redo", "redo", "Redo", can_redo).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.undo(true, window, cx))))
                            .when(self.reset_to.is_some(), |d| {
                                // Live (dk-999, part-studio/1840): disabled while
                                // every value is the one it would restore.
                                let can_reset = self.can_reset(cx);
                                d.child(
                                    footer_button("meta-reset", "Reset all", true, can_reset)
                                        .tooltip(crate::ui::tooltip("Reset all properties"))
                                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                            if can_reset && this.multi.is_some() {
                                                this.reset_multi(window, cx);
                                                return;
                                            }
                                            if let Some(v) = this.reset_to.clone().filter(|_| can_reset) {
                                                this.restore(v, window, cx);
                                                cx.notify();
                                            }
                                        })),
                                )
                            }),
                    )
                    .child(div().flex_1())
                    .child(
                        h_flex()
                            .gap(px(5.))
                            .pr(px(4.))
                            .child(footer_button("meta-apply", "Apply", true, dirty).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.save(false, window, cx))))
                            .child(footer_button("meta-save", "Save", true, dirty).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.save(true, window, cx))))
                            .child(footer_button("meta-close", "Close", false, true).on_click(|_, window, cx| window.close_dialog(cx))),
                    ),
            )
    }
}

/// Several parts' left list (live q2962): 300 px, a 55 px row per part with its picture, its name and "Part" in
/// italics; the form's rows highlighted.
fn parts_list(m: &Multi, dirty: bool, form: impl IntoElement, cx: &mut Context<MetaForm>) -> AnyElement {
    // The form's height and its 41 px header.
    let h = px(381. + props_size().1 - PROPS_MIN.1 + 41.);
    let rows = m.names.iter().enumerate().map(|(i, name)| {
        let picture = match m.thumbs.get(i).cloned().flatten() {
            Some(image) => img(ImageSource::Render(image)).w(px(70.)).h(px(40.)).object_fit(ObjectFit::Contain).into_any_element(),
            None => crate::assets::icon("part", 30., cx),
        };
        let on = m.shown.contains(&i);
        let row = h_flex()
            .id(("meta-part-row", i))
            .map(|d| crate::ui_script::mark(d, || format!("row:{name}")))
            .w_full()
            .h(px(55.))
            .pl(px(24.))
            .pr(px(8.))
            .border_b_1()
            .border_color(rgb(0xdddddd))
            .cursor_pointer()
            .when(on, |d| d.bg(rgb(0xb2ddf6)))
            .when(!on, |d| d.hover(|d| d.bg(rgb(0xdef1fb))))
            .child(div().w(px(70.)).h(px(40.)).flex_none().flex().items_center().justify_center().child(picture))
            .child(
                v_flex()
                    .pl(px(4.))
                    .min_w_0()
                    .flex_1()
                    .child(div().text_size(px(13.33)).font_weight(FontWeight::MEDIUM).text_color(rgb(0x333333)).truncate().child(name.clone()))
                    .child(div().text_size(px(12.)).italic().text_color(rgb(0x666666)).child("Part")),
            )
            .on_click(cx.listener(move |this, e: &ClickEvent, window, cx| this.click_row(i, e.modifiers(), window, cx)));
        // An edit marks every part it will be written to.
        edited_corner(row, dirty && on)
    });
    h_flex()
        .w_full()
        .items_start()
        .child(div().id("meta-parts").w(px(300.)).h(h).flex_none().overflow_y_scroll().bg(rgb(0xf5f5f5)).border_r_1().border_color(rgb(0xcccccc)).children(rows))
        .child(div().flex_1().min_w_0().child(form))
        .into_any_element()
}

/// The 57 px Bootstrap modal header with a 24 px / 500 title.
pub fn modal_header(title: &'static str, width: f32) -> Div {
    h_flex()
        .w(px(width - 2.))
        .h(px(57.))
        .px(px(10.))
        .border_b_1()
        .border_color(rgb(0xdee2e6))
        .child(div().text_size(px(24.)).font_weight(FontWeight::MEDIUM).text_color(rgb(0x333333)).child(title))
}

/// [`modal_header`] without its rule and 8 px shorter (the title where it was): the body draws the rule, so what it
/// shows can start right at it (the kit puts 8 px between title and body; several parts' list, live q2962).
fn modal_header_ruled_by_body(title: &'static str, width: f32) -> Div {
    h_flex()
        .w(px(width - 2.))
        .h(px(49.))
        .pt(px(8.))
        .px(px(10.))
        .child(div().text_size(px(24.)).font_weight(FontWeight::MEDIUM).text_color(rgb(0x333333)).child(title))
}

pub fn bootstrap_modal(dialog: gpui_kit::component::dialog::Dialog, title: &'static str, width: f32, top: f32) -> gpui_kit::component::dialog::Dialog {
    dialog
        .w(px(width))
        .margin_top(px(top))
        .close_button(false)
        .p_0()
        .gap_0()
        .rounded(px(6.))
        .border_color(rgb(0xdee2e6))
        .title(modal_header(title, width))
}

/// Open the Properties modal; `on_save` runs on Apply and Save.
pub fn open_properties(spec: MetaSpec, on_save: impl Fn(MetaValues, &mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) {
    let on_save: SaveFn = Rc::new(on_save);
    let form = cx.new(|cx| MetaForm::new(spec, on_save, window, cx));
    let focus = form.read(cx).focus.clone();
    window.open_dialog(cx, move |dialog, _, _| resizable_props(bootstrap_modal(dialog, "Properties", props_size().0, 15.).keyboard(false).child(form.clone())));
    window.focus(&focus, cx);
}

/// [`open_properties`] with the Release icon at the top left (`on_release` opens the Create Release candidate dialog;
/// Onshape help "Typical Release Workflow": any Properties dialog).
pub fn open_properties_with_release(spec: MetaSpec, on_save: impl Fn(MetaValues, &mut Window, &mut App) + 'static, on_release: impl Fn(&mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) {
    let on_save: SaveFn = Rc::new(on_save);
    let on_release: ReleaseFn = Rc::new(on_release);
    let form = cx.new(|cx| {
        let mut f = MetaForm::new(spec, on_save, window, cx);
        f.on_release = Some(on_release);
        f
    });
    let focus = form.read(cx).focus.clone();
    window.open_dialog(cx, move |dialog, _, _| resizable_props(bootstrap_modal(dialog, "Properties", props_size().0, 15.).keyboard(false).child(form.clone())));
    window.focus(&focus, cx);
}

/// [`open_properties_with_release`] whose computed mass arrives later (`mass`, as [`open_properties_mass_later`]):
/// an Assembly's Properties.
pub fn open_properties_mass_later_with_release(spec: MetaSpec, mass: Task<Option<String>>, on_save: impl Fn(MetaValues, &mut Window, &mut App) + 'static, on_release: impl Fn(&mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) {
    let on_save: SaveFn = Rc::new(on_save);
    let on_release: ReleaseFn = Rc::new(on_release);
    let form = cx.new(|cx| {
        let mut f = MetaForm::new(spec, on_save, window, cx);
        f.on_release = Some(on_release);
        f
    });
    mass_arrives_later(&form, mass, window, cx);
    let focus = form.read(cx).focus.clone();
    window.open_dialog(cx, move |dialog, _, _| resizable_props(bootstrap_modal(dialog, "Properties", props_size().0, 15.).keyboard(false).child(form.clone())));
    window.focus(&focus, cx);
}

/// [`open_properties`] whose computed mass arrives later (`mass`, worked out off the UI thread: an assembly's from
/// its tab's menu): Calculate value says "Calculating…" until then, and shows it when it comes.
pub fn open_properties_mass_later(spec: MetaSpec, mass: Task<Option<String>>, on_save: impl Fn(MetaValues, &mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) {
    let on_save: SaveFn = Rc::new(on_save);
    let form = cx.new(|cx| MetaForm::new(spec, on_save, window, cx));
    mass_arrives_later(&form, mass, window, cx);
    let focus = form.read(cx).focus.clone();
    window.open_dialog(cx, move |dialog, _, _| resizable_props(bootstrap_modal(dialog, "Properties", props_size().0, 15.).keyboard(false).child(form.clone())));
    window.focus(&focus, cx);
}

/// The form's computed mass, worked out off the UI thread (`mass`): "Calculating…" until it arrives.
fn mass_arrives_later(form: &Entity<MetaForm>, mass: Task<Option<String>>, window: &mut Window, cx: &mut App) {
    form.update(cx, |f, _| f.mass_pending = true);
    let weak = form.downgrade();
    let handle = window.window_handle();
    cx.spawn(async move |cx| {
        let computed = mass.await;
        let _ = handle.update(cx, |_, window, cx| {
            let _ = weak.update(cx, |f, cx| {
                f.mass_pending = false;
                f.computed_mass = computed.clone();
                if f.mass_computed {
                    f.mass_failed = computed.is_none();
                    let text = computed.unwrap_or_else(|| "No material assigned.".into());
                    f.mass.update(cx, |s, cx| s.set_placeholder(text, window, cx));
                }
                cx.notify();
            });
        });
    })
    .detach();
}

/// [`open_properties`] with the item's card in a left tree (Part Studio
/// tabs).
pub fn open_properties_with_card(spec: MetaSpec, card: ItemCard, on_save: impl Fn(MetaValues, &mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) {
    let on_save: SaveFn = Rc::new(on_save);
    let form = cx.new(|cx| {
        let mut f = MetaForm::new(spec, on_save, window, cx);
        f.card = Some(card);
        f
    });
    let focus = form.read(cx).focus.clone();
    window.open_dialog(cx, move |dialog, _, _| resizable_props(bootstrap_modal(dialog, "Properties", props_size().0, 15.).keyboard(false).child(form.clone())));
    window.focus(&focus, cx);
}

/// [`open_properties`] for a Part Studio part, with the release-create icon
/// (`on_release` opens the Create Release candidate dialog) and Reset all
/// (`reset_to`). Its `configured` fields have configured values in the
/// active configuration: they show those values, disabled (Configured
/// properties).
pub fn open_part_properties_configured(spec: MetaSpec, part: PartExtras, on_save: impl Fn(MetaValues, &mut Window, &mut App) + 'static, on_release: impl Fn(&mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) {
    let PartExtras { mass, reset_to, configured } = part;
    let on_save: SaveFn = Rc::new(on_save);
    let on_release: ReleaseFn = Rc::new(on_release);
    let form = cx.new(|cx| {
        let mut f = MetaForm::new(spec, on_save, window, cx);
        f.on_release = Some(on_release);
        f.reset_to = reset_to;
        f.configured = configured;
        f
    });
    if let Some(mass) = mass {
        mass_arrives_later(&form, mass, window, cx);
    }
    let focus = form.read(cx).focus.clone();
    window.open_dialog(cx, move |dialog, _, _| resizable_props(bootstrap_modal(dialog, "Properties", props_size().0, 15.).keyboard(false).child(form.clone())));
    window.focus(&focus, cx);
}

/// Several parts' Properties: each part's name, values and what Reset all restores (in the list's order), and their
/// pictures when they come.
pub struct PartsProperties {
    pub names: Vec<String>,
    pub values: Vec<MetaValues>,
    pub reset_to: Vec<MetaValues>,
    pub thumbs: Task<Vec<Option<std::sync::Arc<RenderImage>>>>,
}

/// Parts list › Properties… on several parts (live q2962): one modal for all of them; `on_save` gets the parts written.
pub fn open_parts_properties(spec: MetaSpec, parts: PartsProperties, on_save: impl Fn(Vec<(usize, MetaValues)>, &mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) {
    let PartsProperties { names, values, reset_to, thumbs } = parts;
    let n = names.len();
    let form = cx.new(|cx| {
        let mut f = MetaForm::new(spec, Rc::new(|_, _, _| {}), window, cx);
        f.reset_to = Some(MetaValues::default());
        f.multi = Some(Multi { names, thumbs: vec![None; n], values, reset_to, shown: (0..n).collect(), anchor: 0, mixed_at_fill: Vec::new(), mixed: Vec::new(), reset: false, on_save: Rc::new(on_save) });
        f.fill_multi(window, cx);
        f
    });
    let weak = form.downgrade();
    cx.spawn(async move |cx| {
        let made = thumbs.await;
        let _ = weak.update(cx, |f, cx| {
            if let Some(m) = &mut f.multi {
                m.thumbs = made;
                cx.notify();
            }
        });
    })
    .detach();
    let focus = form.read(cx).focus.clone();
    window.open_dialog(cx, move |dialog, _, _| {
        let width = props_size().0;
        resizable_props(bootstrap_modal(dialog, "Properties", width, 15.).title(modal_header_ruled_by_body("Properties", width)).keyboard(false).child(form.clone()))
    });
    window.focus(&focus, cx);
}

/// [`open_properties`] with `read_only` fields shown greyed, not edited (a standard content part's Name, live skb
/// 10-20); the first editable field has the focus.
pub fn open_properties_read_only(spec: MetaSpec, read_only: Vec<&'static str>, on_save: impl Fn(MetaValues, &mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) {
    let on_save: SaveFn = Rc::new(on_save);
    let (name_locked, number_locked) = (read_only.contains(&"Name"), read_only.contains(&"Part number"));
    let form = cx.new(|cx| {
        let mut f = MetaForm::new(spec, on_save, window, cx);
        f.read_only = read_only;
        f
    });
    let first = match (name_locked, number_locked) {
        (false, _) => Some(form.read(cx).name.clone()),
        (true, false) => Some(form.read(cx).part_number.clone()),
        (true, true) => None,
    };
    window.open_dialog(cx, move |dialog, _, _| resizable_props(bootstrap_modal(dialog, "Properties", props_size().0, 15.).keyboard(false).child(form.clone())));
    if let Some(f) = first {
        f.update(cx, |s, cx| s.focus(window, cx));
    }
}

/// A Part Studio part's Properties beyond [`MetaSpec`]: its computed mass worked out off the UI thread
/// ("Calculating…" until it arrives), the values Reset all restores, the fields configured in the active configuration.
pub struct PartExtras {
    pub mass: Option<Task<Option<String>>>,
    pub reset_to: Option<MetaValues>,
    pub configured: Vec<&'static str>,
}

/// Instance properties…: a 600 px modal with the instance's name and its
/// "Reference designator" (focused); Save (disabled until edited), Close.
pub fn open_instance_properties(instance: String, designator: String, on_save: impl Fn(String, &mut Window, &mut App) + 'static, window: &mut Window, cx: &mut App) {
    let st = cx.new(|cx| InputState::new(window, cx).default_value(designator.clone()));
    let on_save = Rc::new(on_save);
    let focus = st.clone();
    // Save turns on as soon as the field differs.
    cx.subscribe(&st, |_, ev: &InputEvent, cx| {
        if matches!(ev, InputEvent::Change) {
            cx.refresh_windows();
        }
    })
    .detach();
    window.open_dialog(cx, move |dialog, _, cx| {
        let value = st.read(cx).value().trim().to_string();
        let dirty = value != designator;
        let (st2, on_save) = (st.clone(), on_save.clone());
        crate::ui_script::note_dialog(|| {
            // Its one input has the keyboard (the focus' text and selection); live's recorder reads it as the focus, no
            // fields (asm-menu-instance-properties).
            let focus = crate::ui_script::field_focus("", st.read(cx));
            serde_json::json!({ "kind": "modal", "title": "Instance properties", "fields": [], "buttons": ["Save", "Close"], "focus": focus })
        });
        bootstrap_modal(dialog, "Instance properties", 600., 25.).child(
            v_flex()
                .w_full()
                .mt(px(-8.))
                .child(
                    v_flex()
                        .py(px(10.))
                        .gap(px(11.))
                        .child(div().px(px(16.)).pb(px(8.)).border_b_1().border_color(rgb(0xcccccc)).text_size(px(18.)).font_weight(FontWeight::MEDIUM).text_color(rgb(0x333333)).child(instance.clone()))
                        .child(
                            v_flex()
                                .px(px(16.))
                                .gap(px(5.))
                                .child(div().text_size(px(14.)).font_weight(FontWeight::BOLD).text_color(rgb(0x333333)).child("Reference designator"))
                                .child(Input::new(&st).h(px(34.)).text_size(px(14.))),
                        ),
                )
                .child(
                    h_flex()
                        .h(px(58.))
                        .px(px(14.))
                        .gap(px(8.))
                        .justify_end()
                        .border_t_1()
                        .border_color(rgb(0xdee2e6))
                        .child(footer_button("ip-save", "Save", true, dirty).on_click(move |_, window, cx| {
                            if dirty {
                                let v = st2.read(cx).value().trim().to_string();
                                on_save(v, window, cx);
                                window.close_dialog(cx);
                            }
                        }))
                        .child(footer_button("ip-close", "Close", false, true).on_click(|_, window, cx| window.close_dialog(cx))),
                ),
        )
    });
    window.defer(cx, move |window, cx| focus.update(cx, |s, cx| s.focus(window, cx)));
}

/// The Properties modal's least (and default) size; live `resizableModal`
/// (`metadata-dialog`, resizeX + resizeY), kept across sessions as Onshape
/// keeps it in localStorage.
pub const PROPS_MIN: (f32, f32) = (800., 542.);

thread_local! {
    static PROPS_SIZE: std::cell::Cell<(f32, f32)> = std::cell::Cell::new(
        std::fs::read_to_string(crate::paths::config_dir().join("properties_modal_size.txt"))
            .ok()
            .and_then(|t| {
                let (w, h) = t.trim().split_once(' ')?;
                Some((w.parse::<f32>().ok()?.max(PROPS_MIN.0), h.parse::<f32>().ok()?.max(PROPS_MIN.1)))
            })
            .unwrap_or(PROPS_MIN),
    );
}

pub fn props_size() -> (f32, f32) {
    PROPS_SIZE.with(|s| s.get())
}

/// A drag of the modal's right edge (`x`), bottom edge (`y`) or corner.
#[derive(Clone)]
struct ModalResize {
    x: bool,
    y: bool,
}

impl Render for ModalResize {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// The modal's resize handles (live `.modal-resizable-ns` 5 px at the bottom,
/// `.modal-resizable-ew` 5 px at the right, `.modal-resizeable-se` 10 px in
/// the corner). The modal stays centred horizontally, so its width grows
/// on both sides; its top stays.
pub fn resizable_props(dialog: gpui_kit::component::dialog::Dialog) -> gpui_kit::component::dialog::Dialog {
    let handle = |id: &'static str, x: bool, y: bool, cursor: CursorStyle| {
        div().id(id).absolute().cursor(cursor).on_drag(ModalResize { x, y }, |d, _, _, cx| cx.new(|_| d.clone()))
    };
    dialog.child(
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(handle("props-resize-ew", true, false, CursorStyle::ResizeLeftRight).top_0().bottom_0().right_0().w(px(5.)))
            .child(handle("props-resize-ns", false, true, CursorStyle::ResizeUpDown).left_0().right_0().bottom_0().h(px(5.)))
            .child(handle("props-resize-se", true, true, CursorStyle::ResizeUpLeftDownRight).right_0().bottom_0().size(px(10.)))
            .on_drag_move(|e: &DragMoveEvent<ModalResize>, window, cx| {
                let d = e.drag(cx);
                let (mut w, mut h) = props_size();
                let view = window.viewport_size();
                if d.x {
                    w = (2. * f32::from(e.event.position.x - e.bounds.center().x)).round().clamp(PROPS_MIN.0, f32::from(view.width).max(PROPS_MIN.0));
                }
                if d.y {
                    // The modal's top stays 15 px under the window's (the handles
                    // sit in its body, under the header).
                    h = (f32::from(e.event.position.y) - 15.).round().clamp(PROPS_MIN.1, (f32::from(view.height) - 15.).max(PROPS_MIN.1));
                }
                if (w, h) != props_size() {
                    PROPS_SIZE.with(|s| s.set((w, h)));
                    let p = crate::paths::config_dir().join("properties_modal_size.txt");
                    if let Some(dir) = p.parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    let _ = std::fs::write(p, format!("{w} {h}"));
                    window.refresh();
                }
            }),
    )
}


#[cfg(test)]
mod tests {
    use super::{MetaSpec, MetaValues, tab_fields};

    fn spec(category: &'static str, bom_options: Option<Vec<String>>) -> MetaSpec {
        let mut values = MetaValues { name: "T".into(), ..Default::default() };
        values.meta.title2 = Some("t2".into());
        MetaSpec { category, material: None, computed_mass: None, bom_options, drawing_info: None, values }
    }

    fn labels(s: &MetaSpec) -> Vec<&'static str> {
        tab_fields(s).into_iter().map(|f| f.0).collect()
    }

    /// Live ver-version-properties-tree: a Part Studio picked in a version's tree shows Title 1–3 and Not revision
    /// managed after Category, as its own Properties modal does; a Feature Studio has none; an assembly has them all.
    #[test]
    fn a_tabs_fields_follow_its_category() {
        let ps = spec("Onshape Part Studio", None);
        assert_eq!(labels(&ps), ["Title 1", "Title 2", "Title 3", "Not revision managed"]);
        assert_eq!(tab_fields(&ps)[1], ("Title 2", "input", "t2".to_string()));
        assert_eq!(tab_fields(&ps)[3], ("Not revision managed", "checkbox", "false".to_string()));
        assert!(labels(&spec("Onshape Feature Studio", None)).is_empty());
        let asm = spec("Onshape Assembly", Some(vec!["Detailed".into()]));
        assert_eq!(
            labels(&asm),
            ["Part number", "Revision", "State *", "Vendor", "Project", "Product line", "Title 1", "Title 2", "Title 3", "Not revision managed", "Exclude from all BOMs", "Unit of measure", "Override", "Mass", "Subassembly BOM behavior *"]
        );
    }
}
