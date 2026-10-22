//! User export rules (Preferences › User export rules; live behaviours
//! "Preferences › User export rules", "ui2 10-22 05:00", prefs/er-900…931,
//! 1876–1883, dom/preferences-export-rules*): per object type (Scope) and file
//! format, a naming convention with `${property}` placeholders. With no rule
//! an export keeps its default name ("<Part Studio> - <part>").

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What is exported (the rule dialog's Scope ▾).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportElement {
    PartStudio,
    Part,
    Assembly,
    Drawing,
    /// An imported file tab.
    File,
}

impl ExportElement {
    /// The Scope ▾ options, in live's order.
    pub const ALL: [ExportElement; 5] = [ExportElement::Part, ExportElement::Assembly, ExportElement::Drawing, ExportElement::PartStudio, ExportElement::File];

    pub fn label(self) -> &'static str {
        match self {
            ExportElement::Part => "Part",
            ExportElement::Assembly => "Assembly",
            ExportElement::Drawing => "Drawing",
            ExportElement::PartStudio => "Part Studio",
            ExportElement::File => "File",
        }
    }

    /// The Format ▾ options ("All file formats" first).
    pub fn formats(self) -> &'static [&'static str] {
        match self {
            ExportElement::Part => &["All file formats", "PARASOLID", "ACIS", "STEP", "IGES", "PVZ", "JT", "RHINO", "GLTF", "GLB", "OBJ", "3MF", "STL", "DXF", "DWG"],
            ExportElement::Assembly => &["All file formats", "PARASOLID", "ACIS", "STEP", "IGES", "PVZ", "JT", "GLTF", "GLB", "OBJ", "3MF", "URDF", "STL", "CSV_BOM"],
            ExportElement::Drawing => &["All file formats", "Inspection list", "Drawing definition list", "DRAWING_JSON", "PDF", "DWG", "DXF", "DWT", "PNG", "JPEG", "TIFF", "SVG"],
            ExportElement::PartStudio => &["All file formats", "PARASOLID", "ACIS", "STEP", "IGES", "PVZ", "JT", "RHINO", "GLTF", "GLB", "OBJ", "3MF", "STL"],
            ExportElement::File => &["All file formats", "PARASOLID", "ACIS", "STEP", "IGES", "PVZ", "JT", "RHINO", "GLTF", "GLB", "OBJ", "3MF"],
        }
    }

    /// The placeholders valid for this scope: (name, autocomplete meta).
    pub fn variables(self) -> &'static [(&'static str, &'static str)] {
        match self {
            ExportElement::Part => &[("appearance", "Object property"), ("name", "Object property"), ("description", "Object property"), ("category", "Object property"), ("partNumber", "Object property"), ("revision", "Object property"), ("state", "Object property"), ("vendor", "Object property"), ("project", "Object property"), ("productLine", "Object property"), ("material", "Object property"), ("title1", "Object property"), ("title2", "Object property"), ("title3", "Object property"), ("notRevisionManaged", "Object property"), ("excludeFromAllBoms", "Object property"), ("unitOfMeasure", "Object property"), ("tessellationQuality", "Object property"), ("workspaceOrVersion.name", ""), ("workspaceOrVersion.description", ""), ("workspaceOrVersion.category", ""), ("document.name", "Document property"), ("export.format", "Export option"), ("export.resolution", "Export option"), ("export.units", "Export option"), ("export.stlFormat", "Export option"), ("export.timestamp", "Export option"), ("partStudio.name", "Object property"), ("partStudio.description", "Object property"), ("partStudio.category", "Object property"), ("partStudio.title1", "Object property"), ("partStudio.title2", "Object property"), ("partStudio.title3", "Object property"), ("partStudio.notRevisionManaged", "Object property")],
            ExportElement::Assembly => &[("name", "Object property"), ("description", "Object property"), ("category", "Object property"), ("partNumber", "Object property"), ("revision", "Object property"), ("state", "Object property"), ("vendor", "Object property"), ("project", "Object property"), ("productLine", "Object property"), ("title1", "Object property"), ("title2", "Object property"), ("title3", "Object property"), ("notRevisionManaged", "Object property"), ("excludeFromAllBoms", "Object property"), ("unitOfMeasure", "Object property"), ("subassemblyBomBehavior", "Object property"), ("workspaceOrVersion.name", ""), ("workspaceOrVersion.description", ""), ("workspaceOrVersion.category", ""), ("document.name", "Document property"), ("export.format", "Export option"), ("export.resolution", "Export option"), ("export.units", "Export option"), ("export.stlFormat", "Export option"), ("export.timestamp", "Export option")],
            ExportElement::Drawing => &[("name", "Object property"), ("description", "Object property"), ("category", "Object property"), ("partNumber", "Object property"), ("revision", "Object property"), ("state", "Object property"), ("title1", "Object property"), ("title2", "Object property"), ("title3", "Object property"), ("drawnBy", "Object property"), ("approver", "Object property"), ("dateDrawn", "Object property"), ("dateApproved", "Object property"), ("notRevisionManaged", "Object property"), ("lastChangedDate", "Object property"), ("lastChangedBy", "Object property"), ("revisionDescription", "Object property"), ("inspectionCount", "Object property"), ("workspaceOrVersion.name", ""), ("workspaceOrVersion.description", ""), ("workspaceOrVersion.category", ""), ("document.name", "Document property"), ("export.format", "Export option"), ("export.timestamp", "Export option"), ("reference.name", "Object property"), ("reference.description", "Object property"), ("reference.category", "Object property"), ("reference.partNumber", "Object property"), ("reference.revision", "Object property"), ("reference.state", "Object property"), ("reference.vendor", "Object property"), ("reference.project", "Object property"), ("reference.productLine", "Object property"), ("reference.title1", "Object property"), ("reference.title2", "Object property"), ("reference.title3", "Object property"), ("reference.notRevisionManaged", "Object property"), ("reference.excludeFromAllBoms", "Object property"), ("reference.unitOfMeasure", "Object property"), ("reference.subassemblyBomBehavior", "Object property"), ("reference.appearance", "Object property"), ("reference.material", "Object property"), ("reference.tessellationQuality", "Object property")],
            ExportElement::PartStudio => &[("name", "Object property"), ("description", "Object property"), ("category", "Object property"), ("title1", "Object property"), ("title2", "Object property"), ("title3", "Object property"), ("notRevisionManaged", "Object property"), ("workspaceOrVersion.name", ""), ("workspaceOrVersion.description", ""), ("workspaceOrVersion.category", ""), ("document.name", "Document property"), ("export.format", "Export option"), ("export.resolution", "Export option"), ("export.units", "Export option"), ("export.stlFormat", "Export option"), ("export.timestamp", "Export option")],
            ExportElement::File => &[("name", "Object property"), ("description", "Object property"), ("category", "Object property"), ("partNumber", "Object property"), ("revision", "Object property"), ("state", "Object property"), ("vendor", "Object property"), ("project", "Object property"), ("productLine", "Object property"), ("title1", "Object property"), ("title2", "Object property"), ("title3", "Object property"), ("notRevisionManaged", "Object property"), ("workspaceOrVersion.name", ""), ("workspaceOrVersion.description", ""), ("workspaceOrVersion.category", ""), ("document.name", "Document property"), ("export.format", "Export option"), ("export.timestamp", "Export option")],
        }
    }
}

/// "All file formats".
pub const ALL_FORMATS: &str = "All file formats";

/// Most characters a convention may have.
pub const MAX_TEMPLATE: usize = 256;

/// A `${…}` placeholder of a template: its byte range (with `${` `}`) and
/// whether it is a property of the scope (live: blue, else red).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placeholder {
    pub range: std::ops::Range<usize>,
    pub name: String,
    pub valid: bool,
}

/// The placeholders of `template` for `element`.
pub fn placeholders(template: &str, element: ExportElement) -> Vec<Placeholder> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(i) = template[at..].find("${") {
        let start = at + i;
        let Some(j) = template[start + 2..].find('}') else { break };
        let end = start + 2 + j + 1;
        let name = template[start + 2..end - 1].trim().to_string();
        let valid = element.variables().iter().any(|(n, _)| *n == name);
        out.push(Placeholder { range: start..end, name, valid });
        at = end;
    }
    out
}

/// The rule dialog's error (Add / Save disabled), if any (live texts).
pub fn validate(template: &str, element: ExportElement) -> Option<&'static str> {
    if template.trim().is_empty() {
        return Some("Convention is blank.");
    }
    if placeholders(template, element).iter().any(|p| !p.valid) {
        return Some("Convention contains unknown or inactive properties.");
    }
    None
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
    /// The rule for exporting `element` as `format`, if any: the format's own
    /// rule, else the element's "All file formats" one (live: the specific
    /// rule wins whatever the creation order).
    pub fn rule_for(&self, element: ExportElement, format: &str) -> Option<&ExportRule> {
        self.rules
            .iter()
            .find(|r| r.element == element && r.format.eq_ignore_ascii_case(format))
            .or_else(|| self.rules.iter().find(|r| r.element == element && r.format == ALL_FORMATS))
    }

    /// Whether another rule (not `except`) has this element and format (live:
    /// "Unable to save export rule. Another export rule already exists …").
    pub fn conflicts(&self, element: ExportElement, format: &str, except: Option<usize>) -> bool {
        self.rules.iter().enumerate().any(|(i, r)| Some(i) != except && r.element == element && r.format == format)
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
        [("name", "Bracket"), ("partNumber", "PRT-0042"), ("revision", "B")].into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn templates_expand_properties() {
        assert_eq!(expand("${partNumber}_${revision} ${ name }", &values()), "PRT-0042_B Bracket");
        assert_eq!(expand("${Unknown}x", &values()), "x");
        assert_eq!(expand("a/b:${name}", &values()), "a_b_Bracket");
        assert_eq!(expand("open ${name", &values()), "open ${name");
    }

    #[test]
    fn rules_match_element_and_format() {
        let rules = ExportRules { rules: vec![ExportRule { element: ExportElement::Part, format: "STEP".into(), template: "${partNumber}-${revision}".into() }] };
        assert_eq!(rules.file_name(ExportElement::Part, "step", &values(), "Studio - Bracket"), "PRT-0042-B");
        assert_eq!(rules.file_name(ExportElement::Part, "IGES", &values(), "Studio - Bracket"), "Studio - Bracket");
        assert_eq!(rules.file_name(ExportElement::PartStudio, "STEP", &values(), "Studio"), "Studio");
        // A template that expands to nothing keeps the default.
        let empty = ExportRules { rules: vec![ExportRule { element: ExportElement::Drawing, format: "PDF".into(), template: "${Unknown}".into() }] };
        assert_eq!(empty.file_name(ExportElement::Drawing, "PDF", &values(), "Drawing 1"), "Drawing 1");
    }

    #[test]
    fn format_rule_wins_over_all_formats() {
        // Live 1881–1882: STEP → "S_Part 1", IGES → "A_Part 1", whatever the order.
        for rules in [
            vec![ExportRule { element: ExportElement::Part, format: ALL_FORMATS.into(), template: "A_${name}".into() }, ExportRule { element: ExportElement::Part, format: "STEP".into(), template: "S_${name}".into() }],
            vec![ExportRule { element: ExportElement::Part, format: "STEP".into(), template: "S_${name}".into() }, ExportRule { element: ExportElement::Part, format: ALL_FORMATS.into(), template: "A_${name}".into() }],
        ] {
            let r = ExportRules { rules };
            assert_eq!(r.file_name(ExportElement::Part, "STEP", &values(), "x"), "S_Bracket");
            assert_eq!(r.file_name(ExportElement::Part, "IGES", &values(), "x"), "A_Bracket");
            assert!(r.conflicts(ExportElement::Part, "STEP", None));
            assert!(!r.conflicts(ExportElement::Part, "STEP", Some(if r.rules[0].format == "STEP" { 0 } else { 1 })));
        }
    }

    #[test]
    fn placeholders_validate_per_scope() {
        assert_eq!(ExportElement::Part.variables().len(), 34);
        assert_eq!(validate("", ExportElement::Part), Some("Convention is blank."));
        assert_eq!(validate("P_${name}", ExportElement::Part), None);
        assert_eq!(validate("${partNumber}", ExportElement::PartStudio), Some("Convention contains unknown or inactive properties."));
        assert_eq!(validate("a/b:${name}", ExportElement::Part), None);
    }
}
