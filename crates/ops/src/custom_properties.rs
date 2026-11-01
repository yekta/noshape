//! Custom properties (Onshape: Company settings › Properties / Custom
//! properties, help enterprise_settings_custom_properties): definitions
//! kept per user in noshape's config directory, as the company's are kept
//! for every document it owns. Items keep their values by property name
//! (`ItemMeta::custom`); the Properties dialog shows the active ones that
//! apply to the item at its bottom, drawings insert them as
//! `{part.custom.<name>}` / `{drawing.custom.<name>}` (drawing.md A15).

use serde::{Deserialize, Serialize};

/// A custom property's value type.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomKind {
    #[default]
    Text,
    Integer,
    Number,
    Boolean,
    Date,
    /// One of the listed values.
    List(Vec<String>),
}

impl CustomKind {
    pub fn label(&self) -> &'static str {
        match self {
            CustomKind::Text => "String",
            CustomKind::Integer => "Integer",
            CustomKind::Number => "Double",
            CustomKind::Boolean => "Boolean",
            CustomKind::Date => "Date",
            CustomKind::List(_) => "Enum",
        }
    }
}

/// What a custom property applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomTarget {
    Part,
    Assembly,
    Drawing,
}

impl CustomTarget {
    pub const ALL: [CustomTarget; 3] = [CustomTarget::Part, CustomTarget::Assembly, CustomTarget::Drawing];
    pub fn label(self) -> &'static str {
        match self {
            CustomTarget::Part => "Part",
            CustomTarget::Assembly => "Assembly",
            CustomTarget::Drawing => "Drawing",
        }
    }
    /// The Properties dialog's category ("Onshape Part"…).
    pub fn of_category(category: &str) -> Option<CustomTarget> {
        match category {
            "Onshape Part" => Some(CustomTarget::Part),
            "Onshape Assembly" => Some(CustomTarget::Assembly),
            "Onshape Drawing" => Some(CustomTarget::Drawing),
            _ => None,
        }
    }
}

/// Publish state (live: Pending, Active or Inactive); only Active ones
/// show in Properties dialogs and property lists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomState {
    Pending,
    #[default]
    Active,
    Inactive,
}

impl CustomState {
    pub const ALL: [CustomState; 3] = [CustomState::Pending, CustomState::Active, CustomState::Inactive];
    pub fn label(self) -> &'static str {
        match self {
            CustomState::Pending => "Pending",
            CustomState::Active => "Active",
            CustomState::Inactive => "Inactive",
        }
    }
}

/// One custom property definition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomProperty {
    pub name: String,
    pub description: String,
    pub kind: CustomKind,
    pub applies: Vec<CustomTarget>,
    pub state: CustomState,
    /// The value new items show until one is set ("" none).
    pub default: String,
}

impl Default for CustomProperty {
    fn default() -> Self {
        Self { name: String::new(), description: String::new(), kind: CustomKind::Text, applies: vec![CustomTarget::Part], state: CustomState::Active, default: String::new() }
    }
}

impl CustomProperty {
    /// A typed value checked against the type ("" clears); Err(why).
    pub fn check(&self, value: &str) -> Result<String, String> {
        let v = value.trim();
        if v.is_empty() {
            return Ok(String::new());
        }
        match &self.kind {
            CustomKind::Text => Ok(value.to_string()),
            CustomKind::Integer => v.parse::<i64>().map(|n| n.to_string()).map_err(|_| format!("{}: not an integer", self.name)),
            CustomKind::Number => v.parse::<f64>().map(|_| v.to_string()).map_err(|_| format!("{}: not a number", self.name)),
            CustomKind::Boolean => match v.to_ascii_lowercase().as_str() {
                "true" | "yes" | "1" => Ok("True".into()),
                "false" | "no" | "0" => Ok("False".into()),
                _ => Err(format!("{}: true or false", self.name)),
            },
            CustomKind::Date => (v.len() == 10 && v.chars().enumerate().all(|(i, c)| if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() })).then(|| v.to_string()).ok_or_else(|| format!("{}: a date YYYY-MM-DD", self.name)),
            CustomKind::List(options) => options.iter().find(|o| o.eq_ignore_ascii_case(v)).cloned().ok_or_else(|| format!("{}: one of {}", self.name, options.join(", "))),
        }
    }
}

fn path() -> std::path::PathBuf {
    noshape_scene::config_dir().join("custom_properties.json")
}

/// Every definition, in the order created.
pub fn custom_properties() -> Vec<CustomProperty> {
    std::fs::read(path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn set_custom_properties(list: &[CustomProperty]) {
    let _ = std::fs::write(path(), serde_json::to_vec_pretty(list).unwrap_or_default());
}

/// The active properties that apply to `target`, by name (live: custom
/// properties follow the standard ones alphabetically).
pub fn active_for(target: CustomTarget) -> Vec<CustomProperty> {
    let mut v: Vec<CustomProperty> = custom_properties().into_iter().filter(|p| p.state == CustomState::Active && p.applies.contains(&target)).collect();
    v.sort_by_key(|p| p.name.to_lowercase());
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_checked_against_their_type() {
        let p = |kind| CustomProperty { name: "P".into(), kind, ..Default::default() };
        assert_eq!(p(CustomKind::Integer).check(" 12 "), Ok("12".into()));
        assert!(p(CustomKind::Integer).check("1.5").is_err());
        assert_eq!(p(CustomKind::Boolean).check("yes"), Ok("True".into()));
        assert_eq!(p(CustomKind::List(vec!["Steel".into(), "Brass".into()])).check("brass"), Ok("Brass".into()));
        assert!(p(CustomKind::Date).check("2026-1-1").is_err());
        assert_eq!(p(CustomKind::Text).check(""), Ok(String::new()));
    }
}
