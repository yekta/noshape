//! Variable Studio table edits, shared by the app's Variable Studio
//! (`crates/app/src/variable_studio.rs`) and the agent API: the same edits
//! with the same history text. Rows are addressed by their index.

use noshape_model::variables::{VarKind, VariableDef, VariableStudio, VsField};
use noshape_units::{ExprValue, UnitPrefs};

/// The last row's "Name" cell: a Length variable set to 0 in the
/// document's length unit ("Add variable").
pub fn add_variable(s: &mut VariableStudio, name: &str, units: &UnitPrefs) {
    let name = name.trim().trim_start_matches('#').to_string();
    let unit = units.length.symbol();
    s.variables.push(VariableDef { name, expr: ExprValue::new(format!("0 {unit}")), kind: Some(VarKind::Length), description: String::new(), description_expr: false, import: None });
}

/// The Name cell ("Rename variable"): configured cells follow the name.
pub fn rename_variable(s: &mut VariableStudio, i: usize, name: &str) {
    if let Some(v) = s.variables.get_mut(i) {
        let (old, new) = (v.name.clone(), name.trim().trim_start_matches('#').to_string());
        v.name = new.clone();
        s.rename_configured(&old, &new);
    }
}

/// The Value cell ("Edit variable"): a bare number takes the workspace
/// unit of a Length / Angle variable; a configured value sets the shown
/// configuration's row.
pub fn set_value(s: &mut VariableStudio, i: usize, text: &str, units: &UnitPrefs) {
    let Some(v) = s.variables.get(i) else { return };
    let kind = match v.kind {
        Some(VarKind::Length) => "LENGTH",
        Some(VarKind::Angle) => "ANGLE",
        _ => "",
    };
    let text = ExprValue::new(noshape_model::variable_table::with_workspace_unit(kind, text, units)).text;
    let name = v.name.clone();
    if let Some(slot) = s.configured_text_mut(&name, VsField::Value) {
        *slot = text;
    } else if let Some(v) = s.variables.get_mut(i) {
        v.expr = ExprValue::new(text);
    }
}

/// The Description cell ("Edit variable description").
pub fn set_description(s: &mut VariableStudio, i: usize, text: &str) {
    let Some(name) = s.variables.get(i).map(|v| v.name.clone()) else { return };
    if let Some(slot) = s.configured_text_mut(&name, VsField::Description) {
        *slot = text.to_string();
    } else if let Some(v) = s.variables.get_mut(i) {
        v.set_description(text);
    }
}

/// The Type select ("Change variable type").
pub fn set_kind(s: &mut VariableStudio, i: usize, kind: VarKind) {
    if let Some(v) = s.variables.get_mut(i) {
        v.kind = Some(kind);
    }
}

/// "Move variable up" / "Move variable down".
pub fn move_description(down: bool) -> &'static str {
    if down { "Move variable down" } else { "Move variable up" }
}

/// The row menu's Move up / Move down.
pub fn move_variable(s: &mut VariableStudio, i: usize, down: bool) {
    let j = if down { i + 1 } else { i.wrapping_sub(1) };
    if i < s.variables.len() && j < s.variables.len() {
        s.variables.swap(i, j);
    }
}

/// The row menu's Delete ("Delete variable"): its configured cells go too.
pub fn delete_variable(s: &mut VariableStudio, i: usize) {
    if i < s.variables.len() {
        let v = s.variables.remove(i);
        s.configuration.configured.retain(|c| c.variable != v.name);
    }
}

/// The Type select's label.
pub fn kind_label(k: VarKind) -> &'static str {
    match k {
        VarKind::Length => "Length",
        VarKind::Angle => "Angle",
        VarKind::Number => "Number",
        VarKind::Any => "Any",
        VarKind::FromImport => "From import",
    }
}

/// A Part Studio variable table's Copy table (the Variables panel's menu):
/// a header row, then each active variable's name, type, value and
/// description, tab-separated. Shared by the panel and the agent API.
pub fn part_studio_table_text(ps: &noshape_model::partstudio::PartStudio) -> String {
    let kind = |k: &str| match k {
        "ANGLE" => "Angle",
        "NUMBER" => "Number",
        "ANY" => "Any",
        "MEASURED" => "Measured",
        "TABLE" => "From import",
        _ => "Length",
    };
    let mut lines = vec!["Name\tVariable type\tValue\tDescription".to_string()];
    for r in noshape_model::variable_table::active_rows(ps) {
        lines.push(format!("{}\t{}\t{}\t{}", r.name, kind(&r.kind), r.value, r.description));
    }
    lines.join("\n")
}

/// A Variable Studio's table ⋯ › Copy table: a header row, then name,
/// type, value and description per variable, tab-separated.
pub fn studio_table_text(s: &VariableStudio) -> String {
    let mut out = String::from("Name\tVariable type\tValue\tDescription\n");
    for v in &s.variables {
        out.push_str(&format!("{}\t{:?}\t{}\t{}\n", v.name, v.kind.unwrap_or(VarKind::Any), v.expr.text, v.description));
    }
    out
}

// ------------------------------------------------- inserted Variable Studios

use noshape_model::document::{Document, TabContent, TabId};
use noshape_model::variables::{self as vars, StudioRef};

/// The history text of the Insert Variable Studio dialog's edits: a table
/// (or some of its variables) inserted, or its reference removed.
pub fn reference_description(insert: bool) -> &'static str {
    if insert { "Insert Variable Studio" } else { "Remove reference" }
}

/// Insert `r` into a target's Variable table (a Part Studio's or Assembly's,
/// or another Variable Studio's), replacing an earlier reference to the same
/// studio; None removes `studio`'s reference ("Remove reference", also of an
/// auto-inserted table).
pub fn set_reference(c: &mut TabContent, studio: TabId, r: Option<StudioRef>) {
    match (c, r) {
        (TabContent::VariableStudio(v), Some(r)) => vars::insert_into_studio(v, r),
        (TabContent::VariableStudio(v), None) => v.inserted.retain(|x| x.studio != studio),
        (TabContent::PartStudio(ps), Some(r)) => vars::insert_studio(&mut ps.variable_studios, r),
        (TabContent::PartStudio(ps), None) => vars::remove_studio(&mut ps.variable_studios, studio),
        (TabContent::Assembly(a), Some(r)) => vars::insert_studio(&mut a.variable_studios, r),
        (TabContent::Assembly(a), None) => vars::remove_studio(&mut a.variable_studios, studio),
        _ => {}
    }
}

/// "Move table up" / "Move table down".
pub fn move_table_description(down: bool) -> &'static str {
    if down { "Move table down" } else { "Move table up" }
}

/// An edit of one tab's content.
pub type TabEdit = Box<dyn FnOnce(&mut TabContent)>;

/// An inserted table's Move up / Move down in `tab`'s Variable table: the
/// edit to apply to `tab`, or None when it can't move. A Part Studio's or
/// Assembly's whole order is pinned explicitly, so auto-inserted tables
/// become explicit references and the order survives.
pub fn move_reference(doc: &Document, tab: TabId, studio: TabId, down: bool) -> Option<TabEdit> {
    let step = |k: usize, n: usize| {
        let j = if down { k + 1 } else { k.wrapping_sub(1) };
        (j < n).then_some(j)
    };
    match &doc.tab(tab)?.content {
        TabContent::VariableStudio(v) => {
            let k = v.inserted.iter().position(|r| r.studio == studio)?;
            let j = step(k, v.inserted.len())?;
            Some(Box::new(move |c| {
                if let TabContent::VariableStudio(v) = c
                    && k.max(j) < v.inserted.len()
                {
                    v.inserted.swap(k, j);
                }
            }))
        }
        TabContent::PartStudio(_) | TabContent::Assembly(_) => {
            let mut order = vars::element_studios(doc, tab);
            let k = order.iter().position(|r| r.studio == studio)?;
            let j = step(k, order.len())?;
            order.swap(k, j);
            Some(Box::new(move |c| {
                let refs = match c {
                    TabContent::PartStudio(ps) => &mut ps.variable_studios,
                    TabContent::Assembly(a) => &mut a.variable_studios,
                    _ => return,
                };
                refs.removed.retain(|t| !order.iter().any(|r| r.studio == *t));
                refs.inserted = order;
            }))
        }
        _ => None,
    }
}

/// The history text of [`create_studio`].
pub const CREATE_STUDIO_DESCRIPTION: &str = "Insert tab : Variable Studio";

/// A Variable table's "Create Variable Studio": a new studio tab right
/// after `element`, shown in its Variable table at once (the document's
/// first studio is inserted into all; a later one, unticked, is referenced
/// by `element`). Returns the new tab.
pub fn create_studio(doc: &mut Document, element: TabId) -> TabId {
    let name = doc.next_tab_name("Variable Studio");
    let studio = vars::new_studio(doc);
    let all = studio.insert_into_all;
    let id = doc.add_tab(&name, TabContent::VariableStudio(studio));
    if !all && let Some(c) = doc.tab_mut(element).map(|t| &mut t.content) {
        set_reference(c, id, Some(StudioRef::whole(id)));
    }
    let order = &mut doc.settings.tab_order;
    order.retain(|t| *t != id);
    let at = order.iter().position(|t| *t == element).map(|i| i + 1).unwrap_or(order.len());
    order.insert(at, id);
    id
}

/// A Variable Studio's "Insert into all Part Studios and Assemblies"
/// checkbox: its history text for the new state.
pub fn insert_into_all_description(on: bool) -> &'static str {
    if on { "Insert into all Part Studios and Assemblies" } else { "Do not insert into all Part Studios and Assemblies" }
}
