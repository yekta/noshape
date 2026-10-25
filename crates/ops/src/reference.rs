//! The operation reference of `docs/AGENT_API.md`, generated from the
//! registry (`noshape-cli docs`), so the documentation lists every
//! operation with its exact fields. `tests/coverage.rs` fails when the
//! document's generated section is out of date.

use crate::registry;
use serde_json::Value;

/// Markers around the generated section in `docs/AGENT_API.md`.
pub const BEGIN: &str = "<!-- BEGIN GENERATED REFERENCE (noshape-cli docs) -->";
pub const END: &str = "<!-- END GENERATED REFERENCE -->";

/// A field's JSON type in words: `string`, `number`, `[x, y]`, `one of: a, b`...
fn type_text(schema: &Value) -> String {
    if let Some(e) = schema.get("enum").and_then(Value::as_array) {
        let opts: Vec<String> = e.iter().map(|v| v.as_str().map(|s| format!("`{s}`")).unwrap_or_else(|| v.to_string())).collect();
        return format!("one of {}", opts.join(", "));
    }
    match schema.get("type") {
        Some(Value::String(t)) if t == "array" => match schema.get("items") {
            Some(items) => format!("array of {}", type_text(items)),
            None => "array".into(),
        },
        Some(Value::String(t)) => t.clone(),
        Some(Value::Array(ts)) => ts.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" or "),
        _ => "any".into(),
    }
}

/// Table cells can't hold `|` or line breaks.
fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

/// The generated reference: a table of contents by group, then every
/// operation with its description and fields.
pub fn markdown() -> String {
    let ops = registry::all();
    let groups = registry::groups();
    let mut out = String::new();
    // No totals here: branches adding operations would all change one line.
    out.push_str("The operations by group. Field types are JSON types; lengths and angles take expressions with units.\n\n");
    for g in &groups {
        let names: Vec<String> = ops.iter().filter(|o| o.group == *g).map(|o| format!("[`{}`](#{})", o.tool_name(), o.tool_name())).collect();
        out.push_str(&format!("- **{g}**: {}\n", names.join(", ")));
    }
    for g in &groups {
        out.push_str(&format!("\n### {g}\n"));
        for op in ops.iter().filter(|o| o.group == *g) {
            out.push_str(&format!("\n#### {}\n\n{}", op.tool_name(), op.summary));
            let mut flags = Vec::new();
            if op.mutates {
                flags.push("edits the document (one undoable history entry)");
            }
            if op.destructive {
                flags.push("deletes or overwrites");
            }
            if !flags.is_empty() {
                out.push_str(&format!(" ({})", flags.join("; ")));
            }
            out.push_str(".\n");
            if !op.description.is_empty() {
                out.push_str(&format!("\n{}\n", op.description.trim()));
            }
            let examples = crate::examples::of(&op.tool_name());
            if !examples.is_empty() {
                out.push_str("\nExamples:\n\n```json\n");
                for e in examples {
                    out.push_str(e);
                    out.push('\n');
                }
                out.push_str("```\n");
            }
            if !op.fields.is_empty() {
                out.push_str("\n| field | type | | description |\n|---|---|---|---|\n");
                for f in &op.fields {
                    out.push_str(&format!("| `{}` | {} | {} | {} |\n", f.name, cell(&type_text(&f.schema)), if f.required { "required" } else { "" }, cell(f.description)));
                }
            }
        }
    }
    out
}

/// The document with its generated section replaced (None: no markers).
pub fn splice(doc: &str) -> Option<String> {
    let a = doc.find(BEGIN)? + BEGIN.len();
    let b = doc.find(END)?;
    (a <= b).then(|| format!("{}\n\n{}\n{}", &doc[..a], markdown().trim_end(), &doc[b..]))
}
