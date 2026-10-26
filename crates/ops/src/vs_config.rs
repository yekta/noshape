//! Variable Studio configurations (the studio's Configuration panel), shared
//! by the app (`crates/app/src/variable_config.rs`) and the agent API: the
//! same edits with the same history text. Model:
//! `noshape_model::variables::VsConfiguration`.

use noshape_model::configuration::{ConfigInput, ConfigInputKind, ListOption};
use noshape_model::variables::{VsConfiguration, VsField};

pub const ADD_INPUT: &str = "Add configuration input";
pub const DELETE_INPUT: &str = "Delete configuration input";
pub const EDIT: &str = "Edit configuration";
pub const SWITCH: &str = "Change configuration";
pub const PASTE_INPUTS: &str = "Paste configuration inputs";
pub const VISIBILITY: &str = "Edit visibility conditions";
pub const EDIT_IDS: &str = "Edit internal IDs";
pub const CONFIGURE: &str = "Configure variable";
pub const UNCONFIGURE: &str = "Unconfigure variable";
pub const SET_DEFAULT: &str = "Set as default";
pub const DUPLICATE_ROW: &str = "Duplicate row";
pub const DELETE_ROW: &str = "Delete row";
pub const DELETE_COLUMN: &str = "Delete column";

/// An input's or row's Move up / Move down (also the history text).
pub fn move_description(up: bool) -> &'static str {
    if up { "Move up" } else { "Move down" }
}

/// A pasted input's new internal ID when its own is taken.
pub fn new_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let n = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("in{:x}", n & 0xffff_ffff_ffff)
}

/// A new list input's internal ID, as Onshape's "List_8iFesnytxapQpg".
pub fn list_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    const ALNUM: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut n = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0) ^ 0x9e37_79b9_7f4a_7c15;
    let mut out = String::from("List_");
    for _ in 0..14 {
        out.push(ALNUM[(n % 62) as usize] as char);
        n = (n / 62) ^ (n << 7);
    }
    out
}

/// A row's or a named input's internal ID: its name as a FeatureScript
/// identifier, unique among `taken` (live: row "Big" → ID "Big").
pub fn name_id(name: &str, taken: &[String]) -> String {
    noshape_model::configuration::fs_identifier(name, taken)
}

fn options_mut<'a>(c: &'a mut VsConfiguration, input: &str) -> Option<(&'a mut Vec<ListOption>, &'a mut String)> {
    match c.inputs.iter_mut().find(|i| i.id == input).map(|i| &mut i.kind) {
        Some(ConfigInputKind::List { options, default }) => Some((options, default)),
        _ => None,
    }
}

/// Configure Variable Studio ▾ / Add configuration input ▲ › List (made at
/// once): live ei-906 "List_…" named "Configuration" (the first) or
/// "List N", one row "Default". Returns its ID.
pub fn add_list_input(c: &mut VsConfiguration) -> String {
    let n = c.inputs.len() + 1;
    let id = list_id();
    c.inputs.push(ConfigInput {
        id: id.clone(),
        name: if n == 1 { "Configuration".into() } else { format!("List {n}") },
        kind: ConfigInputKind::List { options: vec![ListOption { id: "Default".into(), name: "Default".into() }], default: "Default".into() },
        visible_when: None,
        excluded: false,
    });
    id
}

/// … › Checkbox / Configuration variable, from its dialog: the ID is the
/// name as an identifier (live ei-911: a checkbox "CB" has the ID "CB").
pub fn add_named_input(c: &mut VsConfiguration, name: &str, kind: ConfigInputKind) -> String {
    let taken: Vec<String> = c.inputs.iter().map(|i| i.id.clone()).collect();
    let id = name_id(name, &taken);
    c.inputs.push(ConfigInput { id: id.clone(), name: name.to_string(), kind, visible_when: None, excluded: false });
    id
}

/// The input's name cell (or its ⋯ › Rename).
pub fn rename_input(c: &mut VsConfiguration, input: &str, name: &str) {
    if let Some(i) = c.inputs.iter_mut().find(|i| i.id == input) {
        i.name = name.trim().to_string();
    }
}

/// The input's ⋯ › Move up / Move down.
pub fn move_input(c: &mut VsConfiguration, input: &str, up: bool) {
    let n = c.inputs.len();
    if let Some(at) = c.inputs.iter().position(|i| i.id == input)
        && let Some(to) = if up { at.checked_sub(1) } else { (at + 1 < n).then_some(at + 1) }
    {
        c.inputs.swap(at, to);
    }
}

/// The input's ⋯ › Delete: its configured cells and shown value go too.
pub fn delete_input(c: &mut VsConfiguration, input: &str) {
    c.inputs.retain(|i| i.id != input);
    c.configured.retain(|x| x.input != input);
    c.active.0.remove(input);
}

/// A list's italic "Name" row: a new row named `name`, starting from the
/// default row's values. Returns its ID (None for an empty name or a
/// non-list input).
pub fn add_row(c: &mut VsConfiguration, input: &str, name: &str) -> Option<String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return None;
    }
    let (options, default) = options_mut(c, input)?;
    let taken: Vec<String> = options.iter().map(|o| o.id.clone()).collect();
    let oid = name_id(&name, &taken);
    let default = default.clone();
    options.push(ListOption { id: oid.clone(), name });
    for x in c.configured.iter_mut().filter(|x| x.input == input) {
        let v = x.values.get(&default).cloned().unwrap_or_default();
        x.values.insert(oid.clone(), v);
    }
    Some(oid)
}

/// A row's name cell.
pub fn rename_row(c: &mut VsConfiguration, input: &str, key: &str, name: &str) {
    if let Some((options, _)) = options_mut(c, input)
        && let Some(o) = options.iter_mut().find(|o| o.id == key)
    {
        o.name = name.trim().to_string();
    }
}

/// A row's menu › Set as default.
pub fn set_default_row(c: &mut VsConfiguration, input: &str, key: &str) {
    if let Some((_, default)) = options_mut(c, input) {
        *default = key.to_string();
    }
}

/// A row's menu › Duplicate row: "<name> copy" under it, with its values.
pub fn duplicate_row(c: &mut VsConfiguration, input: &str, key: &str) -> Option<String> {
    let (options, _) = options_mut(c, input)?;
    let at = options.iter().position(|o| o.id == key)?;
    let name = format!("{} copy", options[at].name);
    let taken: Vec<String> = options.iter().map(|o| o.id.clone()).collect();
    let oid = name_id(&name, &taken);
    options.insert(at + 1, ListOption { id: oid.clone(), name });
    for x in c.configured.iter_mut().filter(|x| x.input == input) {
        let v = x.values.get(key).cloned().unwrap_or_default();
        x.values.insert(oid.clone(), v);
    }
    Some(oid)
}

/// A row's menu › Move up / Move down.
pub fn move_row(c: &mut VsConfiguration, input: &str, key: &str, up: bool) {
    if let Some((options, _)) = options_mut(c, input)
        && let Some(at) = options.iter().position(|o| o.id == key)
        && let Some(to) = if up { at.checked_sub(1) } else { (at + 1 < options.len()).then_some(at + 1) }
    {
        options.swap(at, to);
    }
}

/// A row's menu › Delete row (not the last row): a deleted default row
/// passes the default to the first.
pub fn delete_row(c: &mut VsConfiguration, input: &str, key: &str) {
    if let Some((options, default)) = options_mut(c, input)
        && options.len() > 1
    {
        options.retain(|o| o.id != key);
        if *default == key {
            *default = options[0].id.clone();
        }
    }
    for x in c.configured.iter_mut() {
        x.values.remove(key);
    }
    if c.active.0.get(input).is_some_and(|k| k == key) {
        c.active.0.remove(input);
    }
}

/// A configured cell of the table: variable `variable`'s Value or
/// Description in row `key` ("false" / "true" for a checkbox).
pub fn set_cell(c: &mut VsConfiguration, variable: &str, field: VsField, key: &str, text: &str) {
    if let Some(x) = c.configured.iter_mut().find(|x| x.variable == variable && x.field == field) {
        x.values.insert(key.to_string(), text.trim().to_string());
    }
}

/// Switch to a row / tick a checkbox / type a configuration variable's
/// value: the configuration the studio shows.
pub fn switch(c: &mut VsConfiguration, input: &str, value: &str) {
    c.active.0.insert(input.to_string(), value.trim().to_string());
}

/// Paste inputs (copied with Copy input, here or in a Part Studio): an ID
/// already used here gets a new one.
pub fn paste_inputs(c: &mut VsConfiguration, mut inputs: Vec<ConfigInput>) {
    for i in &mut inputs {
        if c.inputs.iter().any(|x| x.id == i.id) {
            i.id = new_id();
        }
    }
    c.inputs.append(&mut inputs);
}

/// Copy table: the input's table as tab-separated text (header rows, then
/// a row per option).
pub fn table_text(cfg: &VsConfiguration, input: &str) -> String {
    let cols: Vec<&noshape_model::variables::VsConfigured> = cfg.configured.iter().filter(|c| c.input == input).collect();
    let mut out = String::from("Name");
    for c in &cols {
        out.push_str(&format!("\t#{} {}", c.variable, if c.field == VsField::Value { "Value" } else { "Description" }));
    }
    let names: Vec<(String, String)> = match cfg.inputs.iter().find(|i| i.id == input).map(|i| &i.kind) {
        Some(ConfigInputKind::List { options, .. }) => options.iter().map(|o| (o.id.clone(), o.name.clone())).collect(),
        Some(ConfigInputKind::Checkbox { .. }) => vec![("false".into(), "false".into()), ("true".into(), "true".into())],
        _ => Vec::new(),
    };
    for (key, name) in names {
        out.push('\n');
        out.push_str(&name);
        for c in &cols {
            out.push('\t');
            out.push_str(c.values.get(&key).map(String::as_str).unwrap_or(""));
        }
    }
    out
}

/// An internal ID's problem (Edit internal IDs…): empty, not a
/// FeatureScript identifier, or a reserved keyword.
pub fn id_error(id: &str) -> Option<&'static str> {
    if id.is_empty() {
        return Some("ID cannot be empty");
    }
    let mut chars = id.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    if !first_ok || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Some("ID must be a valid FeatureScript identifier");
    }
    noshape_model::configuration::FS_KEYWORDS.contains(&id).then_some("ID cannot be a reserved FeatureScript keyword")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rows: added from the default row's values, duplicated, moved and
    /// deleted as the table's menus do.
    #[test]
    fn rows_follow_the_tables_menus() {
        let mut c = VsConfiguration::default();
        let input = add_list_input(&mut c);
        c.configure(&input, "w", VsField::Value, "10 mm");
        let big = add_row(&mut c, &input, "Big").expect("a list");
        assert_eq!(big, "Big");
        assert_eq!(c.configured[0].values.get("Big").map(String::as_str), Some("10 mm"));
        set_cell(&mut c, "w", VsField::Value, "Big", "20 mm ");
        let copy = duplicate_row(&mut c, &input, "Big").expect("a row");
        assert_eq!(c.configured[0].values.get(&copy).map(String::as_str), Some("20 mm"));
        move_row(&mut c, &input, &copy, true);
        let ids = |c: &VsConfiguration| match &c.inputs[0].kind {
            ConfigInputKind::List { options, default } => (options.iter().map(|o| o.id.clone()).collect::<Vec<_>>(), default.clone()),
            _ => unreachable!(),
        };
        assert_eq!(ids(&c).0, ["Default", "Big_copy", "Big"]);
        switch(&mut c, &input, "Big");
        set_default_row(&mut c, &input, "Big");
        delete_row(&mut c, &input, "Big");
        assert_eq!(ids(&c), (vec!["Default".to_string(), "Big_copy".to_string()], "Default".to_string()));
        assert!(c.active.0.is_empty() && !c.configured[0].values.contains_key("Big"));
        assert_eq!(table_text(&c, &input), "Name\t#w Value\nDefault\t10 mm\nBig copy\t20 mm");
        delete_input(&mut c, &input);
        assert!(c.inputs.is_empty() && c.configured.is_empty());
    }
}
