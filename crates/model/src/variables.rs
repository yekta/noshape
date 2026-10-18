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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VarKind {
    Length,
    Angle,
    Number,
    Any,
}

impl VarKind {
    pub fn dim(self) -> Option<Dim> {
        match self {
            VarKind::Length => Some(Dim::LENGTH),
            VarKind::Angle => Some(Dim::ANGLE),
            VarKind::Number => Some(Dim::UNITLESS),
            VarKind::Any => None,
        }
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
        Self { studio, variables: None, external: None }
    }
    fn keep(&self, name: &str) -> bool {
        self.variables.as_ref().is_none_or(|v| v.iter().any(|n| n == name))
    }
}

impl Default for VariableStudio {
    fn default() -> Self {
        Self { variables: Vec::new(), insert_into_all: true, inserted: Vec::new() }
    }
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
    studio_variables_at(doc, studio, &mut Vec::new())
}

fn studio_variables_at(doc: &crate::Document, studio: TabId, visiting: &mut Vec<TabId>) -> BTreeMap<String, Quantity> {
    let Some(vs) = studio_of(doc, studio) else { return BTreeMap::new() };
    if visiting.contains(&studio) {
        return BTreeMap::new();
    }
    visiting.push(studio);
    let mut out = BTreeMap::new();
    for r in &vs.inserted {
        out.extend(ref_variables(doc, r, visiting));
    }
    let own = vs.evaluate_in(&out, &doc.settings.units);
    out.extend(own.values);
    visiting.pop();
    out
}

/// The variables an inserted table contributes (all, or the chosen ones);
/// an external table is evaluated in its own document's version.
fn ref_variables(doc: &crate::Document, r: &StudioRef, visiting: &mut Vec<TabId>) -> BTreeMap<String, Quantity> {
    let mut vars = match &r.external {
        Some(e) => match external_document(e) {
            Some(ext) => studio_variables_at(&ext, r.studio, &mut Vec::new()),
            None => BTreeMap::new(),
        },
        None => studio_variables_at(doc, r.studio, visiting),
    };
    vars.retain(|k, _| r.keep(k));
    vars
}

/// The evaluation of a Variable Studio's own table (results per row), with
/// the tables inserted into it in scope.
pub fn studio_evaluation(doc: &crate::Document, studio: TabId) -> noshape_units::TableEvaluation {
    let Some(vs) = studio_of(doc, studio) else { return VariableStudio::default().evaluate(&doc.settings.units) };
    let mut outer = BTreeMap::new();
    let mut visiting = vec![studio];
    for r in &vs.inserted {
        outer.extend(ref_variables(doc, r, &mut visiting));
    }
    vs.evaluate_in(&outer, &doc.settings.units)
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

/// Whether `studio` shows in `element`'s Variable table.
pub fn is_inserted(doc: &crate::Document, element: TabId, studio: TabId) -> bool {
    element_studios(doc, element).iter().any(|r| r.studio == studio)
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

    fn vs(vars: &[(&str, &str)], all: bool) -> VariableStudio {
        VariableStudio { variables: vars.iter().map(|(n, e)| VariableDef { name: n.to_string(), expr: ExprValue::new(*e), kind: None, description: String::new() }).collect(), insert_into_all: all, inserted: Vec::new() }
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
            v.variables = vec![VariableDef { name: "c".into(), expr: ExprValue::new("#a * 3"), kind: None, description: String::new() }];
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
            insert_studio(&mut p.variable_studios, StudioRef { studio: ids[0], variables: Some(vec!["b".into()]), external: None });
        }
        let vars = element_variables(&doc, ps);
        assert!(!vars.contains_key("a"));
        assert_eq!(mm(vars.get("b")), 2.0);
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
