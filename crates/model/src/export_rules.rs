//! User export rules (Preferences › User export rules): file name templates
//! for exports. This is the rule store and the name expansion the export
//! dialogs go through; with no rules every export keeps its default name.
//! The rule fields, the template tokens and which rule wins come from the
//! live capture (G39) and are filled in with the Preferences UI.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What is exported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportElement {
    PartStudio,
    Part,
    Assembly,
    Drawing,
}

/// One rule: exports of `element` in `format` (Onshape's format name, e.g.
/// "STEP", "PDF") are named by `template`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportRule {
    pub element: ExportElement,
    pub format: String,
    pub template: String,
}

/// The user's rules, in the order they were added.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportRules {
    #[serde(default)]
    pub rules: Vec<ExportRule>,
}

/// The values a template can use, keyed by property name ("Name",
/// "Part number", "Revision", …).
pub type NameValues = BTreeMap<String, String>;

impl ExportRules {
    /// The rule for exporting `element` as `format`, if any.
    pub fn rule_for(&self, element: ExportElement, format: &str) -> Option<&ExportRule> {
        self.rules.iter().find(|r| r.element == element && r.format.eq_ignore_ascii_case(format))
    }

    /// The file name (without extension) for an export: the matching rule's
    /// template filled with `values`, else `default`.
    pub fn file_name(&self, element: ExportElement, format: &str, values: &NameValues, default: &str) -> String {
        match self.rule_for(element, format).map(|r| expand(&r.template, values)) {
            Some(name) if !name.is_empty() => name,
            _ => default.to_string(),
        }
    }
}

/// Fill `${Property}` tokens (property names match ignoring case and
/// surrounding spaces; unknown ones expand to nothing) and replace the
/// characters file names can't hold.
pub fn expand(template: &str, values: &NameValues) -> String {
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let key = after[..end].trim();
        if let Some(v) = values.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v) {
            out.push_str(v);
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out.chars().map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() { '_' } else { c }).collect::<String>().trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> NameValues {
        [("Name", "Bracket"), ("Part number", "PRT-0042"), ("Revision", "B")].into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn templates_expand_properties() {
        assert_eq!(expand("${Part number}_${revision} ${ Name }", &values()), "PRT-0042_B Bracket");
        assert_eq!(expand("${Unknown}x", &values()), "x");
        assert_eq!(expand("a/b:${Name}", &values()), "a_b_Bracket");
        assert_eq!(expand("open ${Name", &values()), "open ${Name");
    }

    #[test]
    fn rules_match_element_and_format() {
        let rules = ExportRules { rules: vec![ExportRule { element: ExportElement::Part, format: "STEP".into(), template: "${Part number}-${Revision}".into() }] };
        assert_eq!(rules.file_name(ExportElement::Part, "step", &values(), "Studio - Bracket"), "PRT-0042-B");
        assert_eq!(rules.file_name(ExportElement::Part, "IGES", &values(), "Studio - Bracket"), "Studio - Bracket");
        assert_eq!(rules.file_name(ExportElement::PartStudio, "STEP", &values(), "Studio"), "Studio");
        // A template that expands to nothing keeps the default.
        let empty = ExportRules { rules: vec![ExportRule { element: ExportElement::Drawing, format: "PDF".into(), template: "${Unknown}".into() }] };
        assert_eq!(empty.file_name(ExportElement::Drawing, "PDF", &values(), "Drawing 1"), "Drawing 1");
    }
}
