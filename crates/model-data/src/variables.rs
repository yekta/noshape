//! Variable Studios: document-level variable tables. As in Onshape, a
//! studio's variables are visible only in the Part Studios and Assemblies it
//! is inserted into: every element when its "Insert into all Part Studios and
//! Assemblies" box is checked (the default), plus elements that inserted it
//! from their Variable table ("Insert Variable Studio"). A studio can also
//! insert other studios' tables into itself.

use noshape_units::{Dim, ExprValue};
use crate::document::TabId;
use noshape_units::Quantity;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VariableDef {
    pub name: String,
    pub expr: ExprValue,
    /// Expected dimension; None = any.
    pub kind: Option<VarKind>,
    #[serde(default)]
    pub description: String,
    /// Description right-click › Convert to expression: `description` is an
    /// expression (text is the string literal `"text"`), as in the Part
    /// Studio Variable table (live 652–656).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub description_expr: bool,
    /// "From import": the CSV selection (kind `FromImport`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import: Option<CsvImport>,
}

impl VariableDef {
    /// Description right-click › Convert to expression / Convert to text:
    /// the text becomes the string literal `"text"` and back.
    pub fn toggle_description_expr(&mut self) {
        if self.description_expr {
            if let Some(t) = crate::variable_table::string_literal(&self.description) {
                self.description = t.to_string();
            }
        } else if !self.description.trim().is_empty() {
            self.description = format!("\"{}\"", self.description);
        }
        self.description_expr = !self.description_expr;
    }

    /// The Description cell's text, typed: an expression description keeps
    /// plain words as a string literal.
    pub fn set_description(&mut self, text: &str) {
        self.description = if self.description_expr { crate::variable_table::as_expression(text) } else { text.to_string() };
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VarKind {
    Length,
    Angle,
    Number,
    Any,
    /// A value read from a CSV file tab (help: "Importing a variable from a
    /// CSV table"): a single value, an array or a map.
    FromImport,
}

impl VarKind {
    pub fn dim(self) -> Option<Dim> {
        match self {
            VarKind::Length => Some(Dim::LENGTH),
            VarKind::Angle => Some(Dim::ANGLE),
            VarKind::Number => Some(Dim::UNITLESS),
            VarKind::Any | VarKind::FromImport => None,
        }
    }
}

/// How rows (or columns) of the CSV are picked: Access type and options.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CsvAxis {
    pub access: CsvAccess,
    /// By index (0-based).
    pub index: usize,
    /// By index range (inclusive, 0-based).
    pub min: usize,
    pub max: usize,
    /// By label: the label to match ("Value").
    pub label: String,
    /// By label: match with a regular expression.
    pub regex: bool,
    pub result: CsvResult,
    /// Map results: the column (for rows) / row (for columns) holding labels.
    pub label_index: usize,
}

impl Default for CsvAxis {
    fn default() -> Self {
        Self { access: CsvAccess::ByIndex, index: 0, min: 0, max: 0, label: String::new(), regex: false, result: CsvResult::Single, label_index: 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CsvAccess {
    ByIndex,
    ByIndexRange,
    ByLabel,
    All,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CsvResult {
    Single,
    Array,
    Map,
}

/// A "From import" variable's CSV selection.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CsvImport {
    /// The CSV file tab.
    pub file: Option<TabId>,
    pub row: CsvAxis,
    pub column: CsvAxis,
    /// "Conversion factor" (checked: an expression with units, e.g. `1 in`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversion: Option<String>,
}

/// The value of a "From import" variable.
#[derive(Clone, Debug, PartialEq)]
pub enum ImportValue {
    Single(String),
    Array(Vec<ImportValue>),
    Map(Vec<(String, ImportValue)>),
}

impl std::fmt::Display for ImportValue {
    /// Onshape's display: `[A : [1, 10], B : [2, 20]]`, `[1, 2]`, `10`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportValue::Single(s) => write!(f, "{s}"),
            ImportValue::Array(a) => write!(f, "[{}]", a.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ")),
            ImportValue::Map(m) => write!(f, "[{}]", m.iter().map(|(k, v)| format!("{k} : {v}")).collect::<Vec<_>>().join(", ")),
        }
    }
}

/// Pick the entries of `items` (rows, or the cells of a row) by `axis`;
/// `label_of(i)` is an entry's label (By label, Map keys).
fn select<T: Clone>(items: &[T], axis: &CsvAxis, label_of: impl Fn(usize) -> String, value: impl Fn(&T) -> ImportValue) -> Result<ImportValue, String> {
    let idx: Vec<usize> = match axis.access {
        CsvAccess::ByIndex => vec![axis.index],
        CsvAccess::ByIndexRange => (axis.min..=axis.max.max(axis.min)).collect(),
        CsvAccess::All => (0..items.len()).collect(),
        CsvAccess::ByLabel => {
            let re = if axis.regex { Some(regex_lite_match(&axis.label)?) } else { None };
            (0..items.len()).filter(|i| {
                let l = label_of(*i);
                match &re {
                    Some(m) => m(&l),
                    None => l == axis.label,
                }
            }).collect()
        }
    };
    let picked: Vec<usize> = idx.into_iter().filter(|i| *i < items.len()).collect();
    match axis.result {
        CsvResult::Single => picked.first().map(|i| value(&items[*i])).ok_or_else(|| "No matching value in the CSV file".to_string()),
        CsvResult::Array => Ok(ImportValue::Array(picked.iter().map(|i| value(&items[*i])).collect())),
        CsvResult::Map => Ok(ImportValue::Map(picked.iter().map(|i| (label_of(*i), value(&items[*i]))).collect())),
    }
}

/// A small regular-expression matcher (`^`, `$`, `.`, `*`, literal text) for
/// "By label" with "Regular expression": anchored at both ends like Onshape.
fn regex_lite_match(pat: &str) -> Result<impl Fn(&str) -> bool, String> {
    fn m(p: &[char], t: &[char]) -> bool {
        match p {
            [] => t.is_empty(),
            [c, '*', rest @ ..] => {
                let mut i = 0;
                loop {
                    if m(rest, &t[i..]) {
                        return true;
                    }
                    if i < t.len() && (*c == '.' || t[i] == *c) {
                        i += 1;
                    } else {
                        return false;
                    }
                }
            }
            [c, rest @ ..] => !t.is_empty() && (*c == '.' || t[0] == *c) && m(rest, &t[1..]),
        }
    }
    let p: Vec<char> = pat.trim_start_matches('^').trim_end_matches('$').chars().collect();
    Ok(move |t: &str| m(&p, &t.chars().collect::<Vec<_>>()))
}

/// Evaluate a "From import" selection on the document's CSV file tab.
pub fn import_value(doc: &crate::Document, imp: &CsvImport) -> Result<ImportValue, String> {
    let file = imp.file.and_then(|t| doc.tab(t)).ok_or("Select a CSV file")?;
    let crate::TabContent::File(f) = &file.content else { return Err("Select a CSV file".into()) };
    let text = f.bytes().map(|b| String::from_utf8_lossy(&b).into_owned()).ok_or("The CSV file can't be read")?;
    let rows: Vec<Vec<String>> = text.trim_start_matches('\u{feff}').lines().filter(|l| !l.trim().is_empty()).map(crate::material_library::split_csv).collect();
    let col = &imp.column;
    let row_label = |i: usize| rows.get(i).and_then(|r| r.get(imp.row.label_index)).cloned().unwrap_or_default();
    select(&rows, &imp.row, row_label, |r: &Vec<String>| {
        let cell_label = |j: usize| rows.get(col.label_index).and_then(|h| h.get(j)).cloned().unwrap_or_default();
        select(r, col, cell_label, |c: &String| ImportValue::Single(c.clone())).unwrap_or(ImportValue::Array(Vec::new()))
    })
}

/// The expression a single-value "From import" variable evaluates to (the
/// cell, times the conversion factor); None for arrays and maps (FeatureScript values).
pub fn import_expression(doc: &crate::Document, imp: &CsvImport) -> Option<String> {
    match import_value(doc, imp).ok()? {
        ImportValue::Single(s) => Some(match &imp.conversion {
            Some(c) if !c.trim().is_empty() => format!("({s}) * ({c})"),
            _ => s,
        }),
        _ => None,
    }
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VariableStudio {
    pub variables: Vec<VariableDef>,
    /// "Insert into all Part Studios and Assemblies" (the footer checkbox).
    /// On for new studios; documents saved before it existed load with it
    /// on, which keeps their variables visible everywhere.
    #[serde(default = "default_true")]
    pub insert_into_all: bool,
    /// Variable Studios inserted into this one ("Insert Variable Studio"),
    /// shown as read-only tables above its own; its own variables can use
    /// theirs.
    #[serde(default)]
    pub inserted: Vec<StudioRef>,
    /// Its configurations (help "Variable Studio Configurations"; the right
    /// strip's Configuration panel).
    #[serde(default, skip_serializing_if = "VsConfiguration::is_empty")]
    pub configuration: VsConfiguration,
}

/// A Variable Studio's configuration inputs (as a Part Studio's) and the
/// variable cells they drive: a configured Value or Description takes the
/// text of the chosen list option / checkbox state, or a configuration
/// variable's value ("Set to").
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct VsConfiguration {
    pub inputs: Vec<crate::configuration::ConfigInput>,
    #[serde(default)]
    pub configured: Vec<VsConfigured>,
    /// The configuration the studio itself shows (the panel's selected rows).
    #[serde(default)]
    pub active: crate::configuration::ConfigValues,
}

/// A configured cell of a Variable Studio.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VsConfigured {
    pub input: String,
    pub variable: String,
    pub field: VsField,
    /// Row key (list option id, "true" / "false") → the cell's text; empty
    /// for a configuration variable ("Set to").
    #[serde(default)]
    pub values: BTreeMap<String, String>,
}

/// Which cell of a variable row is configured.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VsField {
    Value,
    Description,
}

impl VsConfiguration {
    pub fn is_empty(&self) -> bool {
        self.inputs.is_empty()
    }

    fn inputs_only(&self) -> crate::configuration::Configuration {
        crate::configuration::Configuration { inputs: self.inputs.clone(), configured: Vec::new() }
    }

    /// An input's chosen value (option id, "true" / "false", the
    /// variable's expression), its default when not chosen.
    pub fn value_of(&self, values: &crate::configuration::ConfigValues, input: &str) -> Option<String> {
        self.inputs_only().value_of(values, input)
    }

    /// The row keys of an input: list option ids, or "true" / "false".
    pub fn row_keys(&self, input: &str) -> Vec<String> {
        use crate::configuration::ConfigInputKind;
        match self.inputs.iter().find(|i| i.id == input).map(|i| &i.kind) {
            Some(ConfigInputKind::List { options, .. }) => options.iter().map(|o| o.id.clone()).collect(),
            Some(ConfigInputKind::Checkbox { .. }) => vec!["true".into(), "false".into()],
            _ => Vec::new(),
        }
    }

    /// The configured entry of a cell, if any.
    pub fn configured_cell(&self, variable: &str, field: VsField) -> Option<&VsConfigured> {
        self.configured.iter().find(|c| c.variable == variable && c.field == field)
    }

    /// Configure a cell with `input`, every row starting from `current`
    /// (help: "added to the table with their default values").
    pub fn configure(&mut self, input: &str, variable: &str, field: VsField, current: &str) {
        self.configured.retain(|c| !(c.variable == variable && c.field == field));
        let values = self.row_keys(input).into_iter().map(|k| (k, current.to_string())).collect();
        self.configured.push(VsConfigured { input: input.to_string(), variable: variable.to_string(), field, values });
    }

    /// Edit internal IDs: the input `input` becomes `new_id`, its rows
    /// renamed by `rows` (old id, new id); visibility rules, configured
    /// cells and the shown configuration follow.
    pub fn change_ids(&mut self, input: &str, new_id: &str, rows: &[(String, String)]) {
        let mut cfg = self.inputs_only();
        cfg.change_ids(input, new_id, rows);
        self.inputs = cfg.inputs;
        let row = |k: &String| rows.iter().find(|(a, _)| a == k).map(|(_, b)| b.clone()).unwrap_or_else(|| k.clone());
        for c in self.configured.iter_mut().filter(|c| c.input == input) {
            c.input = new_id.to_string();
            c.values = std::mem::take(&mut c.values).into_iter().map(|(k, v)| (row(&k), v)).collect();
        }
        if let Some(v) = self.active.0.remove(input) {
            self.active.0.insert(new_id.to_string(), row(&v));
        }
    }

    /// Unconfigure a cell.
    pub fn unconfigure(&mut self, variable: &str, field: VsField) {
        self.configured.retain(|c| !(c.variable == variable && c.field == field));
    }

    /// A configured cell's text in a configuration.
    pub fn cell_text(&self, c: &VsConfigured, values: &crate::configuration::ConfigValues) -> Option<String> {
        use crate::configuration::ConfigInputKind;
        let input = self.inputs.iter().find(|i| i.id == c.input)?;
        let chosen = self.value_of(values, &c.input)?;
        match &input.kind {
            ConfigInputKind::Variable { .. } | ConfigInputKind::Text { .. } => Some(chosen),
            _ => c.values.get(&chosen).cloned(),
        }
    }
}

impl VariableStudio {
    /// Typing in a configured cell changes the shown configuration's row
    /// (None: the cell isn't configured by a list or checkbox input).
    pub fn configured_text_mut(&mut self, variable: &str, field: VsField) -> Option<&mut String> {
        let c = self.configuration.configured.iter().position(|c| c.variable == variable && c.field == field)?;
        let key = self.configuration.value_of(&self.configuration.active, &self.configuration.configured[c].input)?;
        self.configuration.configured[c].values.get_mut(&key)
    }

    /// A variable renamed: its configured cells follow it.
    pub fn rename_configured(&mut self, old: &str, new: &str) {
        for c in self.configuration.configured.iter_mut().filter(|c| c.variable == old) {
            c.variable = new.to_string();
        }
    }

    /// The studio in a configuration: configured cells take that
    /// configuration's text.
    pub fn configured(&self, values: &crate::configuration::ConfigValues) -> std::borrow::Cow<'_, VariableStudio> {
        if self.configuration.configured.is_empty() {
            return std::borrow::Cow::Borrowed(self);
        }
        let mut out = self.clone();
        for c in &self.configuration.configured {
            let Some(text) = self.configuration.cell_text(c, values) else { continue };
            if let Some(v) = out.variables.iter_mut().find(|v| v.name == c.variable) {
                match c.field {
                    VsField::Value => v.expr = ExprValue::new(text),
                    VsField::Description => v.description = text,
                }
            }
        }
        std::borrow::Cow::Owned(out)
    }
}

/// An inserted Variable Studio table: the whole studio, or only some of its
/// variables (Onshape's Insert Variable Studio dialog allows both).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudioRef {
    /// The studio's tab (in `external`'s document when set).
    pub studio: TabId,
    /// None: every variable.
    #[serde(default)]
    pub variables: Option<Vec<String>>,
    /// A studio of another document, at a version (read-only here).
    #[serde(default)]
    pub external: Option<ExternalStudio>,
    /// A configured studio is inserted in one configuration (the Insert
    /// Variable Studio dialog's Configuration choice).
    #[serde(default, skip_serializing_if = "crate::configuration::ConfigValues::is_empty")]
    pub config: crate::configuration::ConfigValues,
}

/// Where an inserted studio of another document comes from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalStudio {
    pub path: std::path::PathBuf,
    /// Microversion of the version (store id, as text).
    pub microversion: String,
    /// Version name shown after the table title ("Variable Studio 1 V1").
    pub version: String,
}

/// Reads another document's state at a microversion; registered by the app
/// (the model doesn't open stores itself).
pub type ExternalLoader = fn(&std::path::Path, &str) -> Option<std::sync::Arc<crate::Document>>;

static EXTERNAL_LOADER: std::sync::OnceLock<ExternalLoader> = std::sync::OnceLock::new();

/// Register how [`ExternalStudio`] references are read.
pub fn set_external_loader(f: ExternalLoader) {
    let _ = EXTERNAL_LOADER.set(f);
}

/// The document an external reference points at.
pub fn external_document(e: &ExternalStudio) -> Option<std::sync::Arc<crate::Document>> {
    EXTERNAL_LOADER.get().and_then(|f| f(&e.path, &e.microversion))
}

impl StudioRef {
    pub fn whole(studio: TabId) -> Self {
        Self { studio, variables: None, external: None, config: Default::default() }
    }
    fn keep(&self, name: &str) -> bool {
        self.variables.as_ref().is_none_or(|v| v.iter().any(|n| n == name))
    }
}

impl Default for VariableStudio {
    fn default() -> Self {
        Self { variables: Vec::new(), insert_into_all: true, inserted: Vec::new(), configuration: VsConfiguration::default() }
    }
}

/// A new Variable Studio for `doc`: "Insert into all Part Studios and
/// Assemblies" is ticked only for the workspace's first one, later ones come
/// up unticked, even when no other studio is auto-inserted (live, lead 10-21).
pub fn new_studio(doc: &crate::document::Document) -> VariableStudio {
    let first = !doc.tabs.iter().any(|t| matches!(t.content, crate::document::TabContent::VariableStudio(_)));
    VariableStudio { insert_into_all: first, ..VariableStudio::default() }
}

/// The Variable Studio tables of a Part Studio or Assembly (its Variable
/// table panel): studios inserted explicitly, and auto-inserted ones
/// ("Insert into all…") whose reference was removed here.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariableStudioRefs {
    #[serde(default)]
    pub inserted: Vec<StudioRef>,
    #[serde(default)]
    pub removed: Vec<TabId>,
}

impl VariableStudio {
    /// Evaluate in dependency order (Variable Studios allow any order),
    /// without the tables inserted into it (see [`studio_evaluation`]).
    pub fn evaluate(&self, units: &noshape_units::UnitPrefs) -> noshape_units::TableEvaluation {
        self.evaluate_in(&noshape_units::EmptyScope, units)
    }

    /// Evaluate with `outer` (the inserted tables' variables) in scope.
    pub fn evaluate_in(&self, outer: &dyn noshape_units::VariableScope, units: &noshape_units::UnitPrefs) -> noshape_units::TableEvaluation {
        let table = noshape_units::VariableTable {
            order: noshape_units::EvalOrder::Dependency,
            entries: self
                .variables
                .iter()
                .map(|v| noshape_units::VariableDef { name: v.name.clone(), expr: v.expr.text.clone(), kind: v.kind.and_then(|k| k.dim()), description: v.description.clone() })
                .collect(),
        };
        table.evaluate(&outer, units)
    }
}

/// Names of Part Studio variables that are not yet defined where feature
/// `index` evaluates: assigned only by (unsuppressed) Variable features at or
/// after `index`. Onshape offers only variables "that exist in the current
/// context" in numeric fields; callers remove these from the final values.
pub fn variables_defined_later(studio: &crate::partstudio::PartStudio, index: usize) -> std::collections::BTreeSet<String> {
    let name_of = |f: &crate::partstudio::Feature| (f.type_id == "assignVariable" && !f.suppressed).then(|| f.params.text("name").trim().to_string()).filter(|n| !n.is_empty());
    let before: std::collections::BTreeSet<String> = studio.features.iter().take(index).filter_map(name_of).collect();
    studio.features.iter().skip(index).filter_map(name_of).filter(|n| !before.contains(n)).collect()
}

/// `vs` with its "From import" variables turned into expressions (single
/// values; arrays and maps are FeatureScript values and get no scalar).
fn resolved<'a>(vs: &'a VariableStudio, doc: &crate::Document) -> std::borrow::Cow<'a, VariableStudio> {
    if !vs.variables.iter().any(|v| v.kind == Some(VarKind::FromImport)) {
        return std::borrow::Cow::Borrowed(vs);
    }
    let mut out = vs.clone();
    // Rows stay in place (results are index-aligned with the table); an
    // array / map variable gets no scalar value (empty expression).
    for v in out.variables.iter_mut().filter(|v| v.kind == Some(VarKind::FromImport)) {
        v.expr = ExprValue::new(v.import.as_ref().and_then(|i| import_expression(doc, i)).unwrap_or_default());
        v.kind = Some(VarKind::Any);
    }
    std::borrow::Cow::Owned(out)
}

fn studio_of(doc: &crate::Document, tab: TabId) -> Option<&VariableStudio> {
    match &doc.tab(tab)?.content {
        crate::TabContent::VariableStudio(v) => Some(v),
        _ => None,
    }
}

fn refs_of(doc: &crate::Document, element: TabId) -> Option<&VariableStudioRefs> {
    match &doc.tab(element)?.content {
        crate::TabContent::PartStudio(ps) => Some(&ps.variable_studios),
        crate::TabContent::Assembly(a) => Some(&a.variable_studios),
        _ => None,
    }
}

/// The Variable Studios whose tables a Part Studio or Assembly shows, in
/// panel order: the auto-inserted ones ("Insert into all…", document order)
/// not removed there, then the ones inserted there. Deleted tabs drop out.
pub fn element_studios(doc: &crate::Document, element: TabId) -> Vec<StudioRef> {
    match refs_of(doc, element) {
        Some(refs) => refs_studios(doc, refs),
        None => Vec::new(),
    }
}

/// [`element_studios`] for an element's references.
pub fn refs_studios(doc: &crate::Document, refs: &VariableStudioRefs) -> Vec<StudioRef> {
    let mut out: Vec<StudioRef> = Vec::new();
    for t in doc.ordered_tabs() {
        if let crate::TabContent::VariableStudio(vs) = &t.content
            && vs.insert_into_all
            && !refs.removed.contains(&t.id)
            && !refs.inserted.iter().any(|r| r.studio == t.id)
        {
            out.push(StudioRef::whole(t.id));
        }
    }
    for r in &refs.inserted {
        if (r.external.is_some() || studio_of(doc, r.studio).is_some()) && !out.iter().any(|o| o.studio == r.studio) {
            out.push(r.clone());
        }
    }
    out
}

/// Variables of a Variable Studio table: the tables inserted into it first
/// (later ones override earlier ones), then its own variables evaluated
/// with those in scope.
pub fn studio_variables(doc: &crate::Document, studio: TabId) -> BTreeMap<String, Quantity> {
    studio_variables_at(doc, studio, None, &mut Vec::new())
}

fn studio_variables_at(doc: &crate::Document, studio: TabId, config: Option<&crate::configuration::ConfigValues>, visiting: &mut Vec<TabId>) -> BTreeMap<String, Quantity> {
    let Some(vs) = studio_of(doc, studio) else { return BTreeMap::new() };
    // A reference's configuration, else the one the studio shows.
    let vs = vs.configured(config.unwrap_or(&vs.configuration.active));
    if visiting.contains(&studio) {
        return BTreeMap::new();
    }
    visiting.push(studio);
    let mut out = BTreeMap::new();
    for r in &vs.inserted {
        out.extend(ref_variables(doc, r, visiting));
    }
    let own = resolved(&vs, doc).evaluate_in(&out, &doc.settings.units);
    out.extend(own.values);
    visiting.pop();
    out
}

/// The variables an inserted table contributes (all, or the chosen ones);
/// an external table is evaluated in its own document's version.
fn ref_variables(doc: &crate::Document, r: &StudioRef, visiting: &mut Vec<TabId>) -> BTreeMap<String, Quantity> {
    let mut vars = match &r.external {
        Some(e) => match external_document(e) {
            Some(ext) => studio_variables_at(&ext, r.studio, Some(&r.config), &mut Vec::new()),
            None => BTreeMap::new(),
        },
        None => studio_variables_at(doc, r.studio, Some(&r.config), visiting),
    };
    vars.retain(|k, _| r.keep(k));
    vars
}

/// The evaluation of a Variable Studio's own table (results per row), with
/// the tables inserted into it in scope.
pub fn studio_evaluation(doc: &crate::Document, studio: TabId) -> noshape_units::TableEvaluation {
    let Some(vs) = studio_of(doc, studio) else { return VariableStudio::default().evaluate(&doc.settings.units) };
    let vs = vs.configured(&vs.configuration.active);
    let mut outer = BTreeMap::new();
    let mut visiting = vec![studio];
    for r in &vs.inserted {
        outer.extend(ref_variables(doc, r, &mut visiting));
    }
    resolved(&vs, doc).evaluate_in(&outer, &doc.settings.units)
}

/// Variables visible in a Part Studio or Assembly: those of the Variable
/// Studio tables it shows ([`element_studios`]); with the same name in two
/// tables, the later table wins (Onshape).
pub fn element_variables(doc: &crate::Document, element: TabId) -> BTreeMap<String, Quantity> {
    match refs_of(doc, element) {
        Some(refs) => refs_variables(doc, refs),
        None => BTreeMap::new(),
    }
}

/// [`element_variables`] for an element's references (an Assembly being
/// evaluated, which may be a subassembly without a tab of its own here).
pub fn refs_variables(doc: &crate::Document, refs: &VariableStudioRefs) -> BTreeMap<String, Quantity> {
    let mut out = BTreeMap::new();
    for r in refs_studios(doc, refs) {
        out.extend(ref_variables(doc, &r, &mut Vec::new()));
    }
    out
}

/// Names in `studio`'s table that another of `element`'s tables overrides:
/// a lower inserted table, or one of `own` (the element's own variables).
/// Their rows get an ⓘ "Overwritten in another variable table." (live psb
/// 10-20 t).
pub fn overwritten_names(doc: &crate::Document, element: TabId, studio: TabId, own: &[String]) -> std::collections::BTreeSet<String> {
    let list = element_studios(doc, element);
    let Some(k) = list.iter().position(|r| r.studio == studio) else { return Default::default() };
    let mine = ref_variables(doc, &list[k], &mut Vec::new());
    let mut later: std::collections::BTreeSet<String> = own.iter().cloned().collect();
    for r in &list[k + 1..] {
        later.extend(ref_variables(doc, r, &mut Vec::new()).into_keys());
    }
    mine.into_keys().filter(|n| later.contains(n)).collect()
}

/// [`overwritten_names`] with the element's own variables (a Part Studio's
/// Variable features with valid names).
pub fn overwritten(doc: &crate::Document, element: TabId, studio: TabId) -> std::collections::BTreeSet<String> {
    let own: Vec<String> = match doc.tabs.iter().find(|t| t.id == element).map(|t| &t.content) {
        Some(crate::TabContent::PartStudio(ps)) => crate::variable_table::rows(ps).into_iter().map(|r| r.name).collect(),
        _ => Vec::new(),
    };
    overwritten_names(doc, element, studio, &own)
}

/// Whether `studio` shows in `element`'s Variable table.
pub fn is_inserted(doc: &crate::Document, element: TabId, studio: TabId) -> bool {
    element_studios(doc, element).iter().any(|r| r.studio == studio)
}

/// `studio`'s table is in `element`'s Variable table only through "Insert
/// into all Part Studios and Assemblies" (live: the table header's
/// "Automatically referenced Variable Studio" icon).
pub fn is_auto_referenced(doc: &crate::Document, element: TabId, studio: TabId) -> bool {
    let Some(refs) = refs_of(doc, element) else { return false };
    studio_of(doc, studio).is_some_and(|vs| vs.insert_into_all) && !refs.removed.contains(&studio) && !refs.inserted.iter().any(|r| r.studio == studio)
}

/// Insert a table into `refs` (an element's Variable table); replaces an
/// earlier reference to the same studio.
pub fn insert_studio(refs: &mut VariableStudioRefs, r: StudioRef) {
    refs.removed.retain(|t| *t != r.studio);
    match refs.inserted.iter_mut().find(|x| x.studio == r.studio) {
        Some(x) => *x = r,
        None => refs.inserted.push(r),
    }
}

/// Remove `studio`'s table from `refs` ("Remove reference"), including an
/// auto-inserted one.
pub fn remove_studio(refs: &mut VariableStudioRefs, studio: TabId) {
    refs.inserted.retain(|r| r.studio != studio);
    if !refs.removed.contains(&studio) {
        refs.removed.push(studio);
    }
}

/// Insert a table into a Variable Studio (its own "Insert Variable Studio").
pub fn insert_into_studio(vs: &mut VariableStudio, r: StudioRef) {
    match vs.inserted.iter_mut().find(|x| x.studio == r.studio) {
        Some(x) => *x = r,
        None => vs.inserted.push(r),
    }
}

#[cfg(test)]
mod studio_scope_tests {
    use super::*;
    use crate::document::{Tab, TabContent};

    #[test]
    fn description_converts_to_expression_and_back() {
        let mut v = VariableDef { name: "w".into(), expr: ExprValue::new("10 mm"), kind: Some(VarKind::Length), description: "half of 10".into(), description_expr: false, import: None };
        v.toggle_description_expr();
        assert_eq!((v.description.as_str(), v.description_expr), ("\"half of 10\"", true));
        v.set_description("plate width");
        assert_eq!(v.description, "\"plate width\"");
        v.set_description("#w * 2");
        assert_eq!(v.description, "#w * 2");
        v.set_description("plate width");
        v.toggle_description_expr();
        assert_eq!((v.description.as_str(), v.description_expr), ("plate width", false));
    }

    fn vs(vars: &[(&str, &str)], all: bool) -> VariableStudio {
        VariableStudio { variables: vars.iter().map(|(n, e)| VariableDef { name: n.to_string(), expr: ExprValue::new(*e), kind: None, description: String::new(), description_expr: false, import: None }).collect(), insert_into_all: all, inserted: Vec::new(), configuration: VsConfiguration::default() }
    }

    fn doc_with(studios: Vec<VariableStudio>) -> (crate::Document, TabId, Vec<TabId>) {
        let mut doc = crate::Document { settings: Default::default(), tabs: Vec::new() };
        let ps = TabId(1);
        doc.tabs.push(Tab { id: ps, name: "Part Studio 1".into(), content: TabContent::PartStudio(crate::partstudio::PartStudio::new()) });
        let mut ids = Vec::new();
        for (i, s) in studios.into_iter().enumerate() {
            let id = TabId(10 + i as u64);
            doc.tabs.push(Tab { id, name: format!("Variable Studio {}", i + 1), content: TabContent::VariableStudio(s) });
            ids.push(id);
        }
        (doc, ps, ids)
    }

    fn mm(q: Option<&Quantity>) -> f64 {
        q.map(|q| q.value * 1000.0).unwrap_or(f64::NAN)
    }

    #[test]
    fn configured_studio_evaluates_per_configuration() {
        use crate::configuration::{ConfigInput, ConfigInputKind, ConfigValues, ListOption};
        let mut s = vs(&[("L1", "72 in"), ("W1", "#L1 / 2")], false);
        let opts = vec![ListOption { id: "large".into(), name: "Large".into() }, ListOption { id: "small".into(), name: "Small".into() }];
        s.configuration.inputs.push(ConfigInput { id: "c".into(), name: "Configuration".into(), kind: ConfigInputKind::List { options: opts, default: "large".into() }, visible_when: None, excluded: false });
        s.configuration.configure("c", "L1", VsField::Value, "72 in");
        s.configuration.configured[0].values.insert("small".into(), "10 in".into());
        let (mut doc, ps, ids) = doc_with(vec![s]);
        // The studio shows its active (default) configuration.
        assert!((studio_variables(&doc, ids[0])["W1"].value - 0.9144).abs() < 1e-9);
        // An insert in "Small".
        let small = ConfigValues([("c".to_string(), "small".to_string())].into_iter().collect());
        if let TabContent::PartStudio(p) = &mut doc.tab_mut(ps).unwrap().content {
            insert_studio(&mut p.variable_studios, StudioRef { config: small, ..StudioRef::whole(ids[0]) });
        }
        assert!((element_variables(&doc, ps)["W1"].value - 0.127).abs() < 1e-9);
    }

    #[test]
    fn internal_ids_change_with_their_cells() {
        use crate::configuration::{ConfigInput, ConfigInputKind, ListOption};
        let mut s = vs(&[("L1", "72 in")], false);
        let opts = vec![ListOption { id: "Default".into(), name: "Default".into() }, ListOption { id: "Big".into(), name: "Big".into() }];
        s.configuration.inputs.push(ConfigInput { id: "List_a".into(), name: "Configuration".into(), kind: ConfigInputKind::List { options: opts, default: "Default".into() }, visible_when: None, excluded: false });
        s.configuration.configure("List_a", "L1", VsField::Value, "72 in");
        s.configuration.configured[0].values.insert("Big".into(), "80 in".into());
        s.configuration.active.0.insert("List_a".into(), "Big".into());
        s.configuration.change_ids("List_a", "size", &[("Big".into(), "Big2".into())]);
        let c = &s.configuration;
        assert_eq!(c.inputs[0].id, "size");
        assert_eq!(c.configured[0].input, "size");
        assert_eq!(c.configured[0].values.get("Big2").map(String::as_str), Some("80 in"));
        assert_eq!(c.active.0.get("size").map(String::as_str), Some("Big2"));
        assert_eq!(s.configured(&c.active).variables[0].expr.text, "80 in");
    }

    #[test]
    fn lowest_table_wins_and_shadowed_rows_are_known() {
        // Live psb 10-20 t: qa4 (all) → vs1 → vs2; the lowest `w` wins.
        let (mut doc, ps, ids) = doc_with(vec![vs(&[("w", "10 mm")], true), vs(&[("w", "15 mm")], false), vs(&[("w", "20 mm")], false)]);
        if let TabContent::PartStudio(p) = &mut doc.tab_mut(ps).unwrap().content {
            insert_studio(&mut p.variable_studios, StudioRef::whole(ids[1]));
            insert_studio(&mut p.variable_studios, StudioRef::whole(ids[2]));
        }
        assert_eq!(mm(element_variables(&doc, ps).get("w")), 20.0);
        assert_eq!(overwritten_names(&doc, ps, ids[1], &[]).into_iter().collect::<Vec<_>>(), ["w"]);
        assert!(overwritten_names(&doc, ps, ids[2], &[]).is_empty());
        // The Part Studio's own `w` shadows every table.
        assert_eq!(overwritten_names(&doc, ps, ids[2], &["w".to_string()]).len(), 1);
        // Reordered (vs2 above vs1): vs1 wins.
        if let TabContent::PartStudio(p) = &mut doc.tab_mut(ps).unwrap().content {
            p.variable_studios.inserted = vec![StudioRef::whole(ids[0]), StudioRef::whole(ids[2]), StudioRef::whole(ids[1])];
        }
        assert_eq!(mm(element_variables(&doc, ps).get("w")), 15.0);
    }

    #[test]
    fn insert_into_all_scopes_variables() {
        let (mut doc, ps, ids) = doc_with(vec![vs(&[("width", "2 mm")], true), vs(&[("height", "3 mm")], false)]);
        let vars = element_variables(&doc, ps);
        assert_eq!(mm(vars.get("width")), 2.0);
        assert!(!vars.contains_key("height"), "unchecked studio is not visible until inserted");
        // Insert the second one from the Part Studio's Variable table.
        if let TabContent::PartStudio(p) = &mut doc.tab_mut(ps).unwrap().content {
            insert_studio(&mut p.variable_studios, StudioRef::whole(ids[1]));
        }
        assert_eq!(element_studios(&doc, ps).iter().map(|r| r.studio).collect::<Vec<_>>(), vec![ids[0], ids[1]]);
        assert_eq!(mm(element_variables(&doc, ps).get("height")), 3.0);
        // Remove the auto-inserted one there.
        if let TabContent::PartStudio(p) = &mut doc.tab_mut(ps).unwrap().content {
            remove_studio(&mut p.variable_studios, ids[0]);
        }
        assert!(!element_variables(&doc, ps).contains_key("width"));
        assert!(!is_inserted(&doc, ps, ids[0]));
    }

    #[test]
    fn later_tables_win_and_studios_insert_studios() {
        let (mut doc, ps, ids) = doc_with(vec![vs(&[("a", "1 mm")], true), vs(&[("a", "5 mm"), ("b", "#a * 2")], true)]);
        assert_eq!(mm(element_variables(&doc, ps).get("a")), 5.0);
        // Studio 2 inserts studio 1 and uses its variable; its own `a` still wins.
        if let TabContent::VariableStudio(v) = &mut doc.tab_mut(ids[1]).unwrap().content {
            v.variables = vec![VariableDef { name: "c".into(), expr: ExprValue::new("#a * 3"), kind: None, description: String::new(), description_expr: false, import: None }];
            v.inserted = vec![StudioRef::whole(ids[0])];
            v.insert_into_all = false;
        }
        assert_eq!(mm(studio_variables(&doc, ids[1]).get("c")), 3.0);
        assert!(studio_evaluation(&doc, ids[1]).results.iter().all(|r| r.is_ok()));
        // Cycles don't recurse forever.
        if let TabContent::VariableStudio(v) = &mut doc.tab_mut(ids[0]).unwrap().content {
            v.inserted = vec![StudioRef::whole(ids[1])];
        }
        let _ = element_variables(&doc, ps);
    }

    #[test]
    fn single_variables_can_be_inserted() {
        let (mut doc, ps, ids) = doc_with(vec![vs(&[("a", "1 mm"), ("b", "2 mm")], false)]);
        if let TabContent::PartStudio(p) = &mut doc.tab_mut(ps).unwrap().content {
            insert_studio(&mut p.variable_studios, StudioRef { studio: ids[0], variables: Some(vec!["b".into()]), external: None, config: Default::default() });
        }
        let vars = element_variables(&doc, ps);
        assert!(!vars.contains_key("a"));
        assert_eq!(mm(vars.get("b")), 2.0);
    }

    #[test]
    fn only_the_first_studio_is_inserted_into_all() {
        let mut doc = crate::document::Document::default();
        let first = new_studio(&doc);
        assert!(first.insert_into_all);
        doc.add_tab("Variable Studio 1", crate::document::TabContent::VariableStudio(first));
        assert!(!new_studio(&doc).insert_into_all);
    }

    #[test]
    fn old_documents_load_with_insert_into_all() {
        let v: VariableStudio = serde_json::from_str(r#"{"variables":[]}"#).unwrap();
        assert!(v.insert_into_all && v.inserted.is_empty());
    }
}

#[cfg(test)]
mod scope_tests {
    use super::variables_defined_later;
    use crate::params::{ParamValue, ParamValues};
    use crate::partstudio::{Feature, PartStudio};
    use crate::reference::FeatureId;

    fn var(id: u64, name: &str, suppressed: bool) -> Feature {
        let mut p = ParamValues::default();
        p.set("name", ParamValue::Text(name.into()));
        Feature { id: FeatureId(id), type_id: "assignVariable".into(), name: format!("Variable {id}"), suppressed, params: p, folder: None, custom_name: false, hidden: false, suppression_expr: None }
    }

    #[test]
    fn only_variables_defined_before_a_feature_are_in_scope() {
        let mut ps = PartStudio::new();
        let ext = Feature { id: FeatureId(9), type_id: "extrude".into(), name: "Extrude 1".into(), suppressed: false, params: ParamValues::default(), folder: None, custom_name: false, hidden: false, suppression_expr: None };
        ps.features = vec![var(1, "a", false), var(2, "b", true), ext, var(3, "width", false), var(4, "a", false), var(5, "b", false)];
        // At the extrude (index 2): `a` exists, `width` and `b` (suppressed before) do not.
        let later = variables_defined_later(&ps, 2);
        assert_eq!(later.into_iter().collect::<Vec<_>>(), ["b", "width"]);
        // At the end everything is defined.
        assert!(variables_defined_later(&ps, ps.features.len()).is_empty());
        // Editing the first Variable: nothing is defined yet.
        assert_eq!(variables_defined_later(&ps, 0).len(), 3);
    }
}

#[cfg(test)]
mod import_tests {
    use super::*;
    use crate::document::{Document, TabContent};

    fn doc_with_csv(csv: &str) -> (Document, TabId) {
        let mut d = Document::new();
        let id = d.add_tab("t.csv", TabContent::File(crate::file_tab::FileTab::new("t.csv", csv.as_bytes(), 0)));
        (d, id)
    }

    #[test]
    fn rows_indexed_by_first_column_and_single_values() {
        // Help: "Variable: From table - Rows indexed by first column example".
        let (d, id) = doc_with_csv("A,1,10\nB,2,20\nC,3,30\n");
        let imp = CsvImport {
            file: Some(id),
            row: CsvAxis { access: CsvAccess::All, result: CsvResult::Map, label_index: 0, ..Default::default() },
            column: CsvAxis { access: CsvAccess::ByIndexRange, min: 1, max: 2, result: CsvResult::Array, ..Default::default() },
            conversion: None,
        };
        assert_eq!(import_value(&d, &imp).unwrap().to_string(), "[A : [1, 10], B : [2, 20], C : [3, 30]]");
        // One cell with a conversion factor becomes a scalar variable.
        let one = CsvImport { file: Some(id), row: CsvAxis { index: 1, ..Default::default() }, column: CsvAxis { index: 2, ..Default::default() }, conversion: Some("1 mm".into()) };
        assert_eq!(import_expression(&d, &one).as_deref(), Some("(20) * (1 mm)"));
        // By label with a regular expression.
        let lab = CsvImport { file: Some(id), row: CsvAxis { access: CsvAccess::ByLabel, label: "^[BC]*$".replace("[BC]", "C"), regex: true, result: CsvResult::Array, ..Default::default() }, column: CsvAxis { index: 1, ..Default::default() }, conversion: None };
        assert_eq!(import_value(&d, &lab).unwrap().to_string(), "[3]");
        assert!(import_value(&d, &CsvImport::default()).is_err());
    }
}
