//! Discovering the API itself.

use crate::Ctx;
use crate::error::OpError;
use crate::registry::{self, Args, Op, OpOutput, opt};
use serde_json::json;

pub fn ops() -> Vec<Op> {
    vec![Op {
        group: "ops",
        verb: "list",
        summary: "List every operation (tool) of the noshape API",
        description: "Returns each operation's tool name, CLI command and summary, by group. Use `feature_types` and `feature_schema` for the Part Studio features and their parameters.",
        fields: vec![opt("group", json!({ "type": "string" }), "Only this group (document, tab, feature, sketch, query, render, export, history...).")],
        mutates: false,
        destructive: false,
        run: list,
    }]
}

fn list(_: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let group = a.opt_str("group")?;
    let ops: Vec<_> = registry::all()
        .iter()
        .filter(|o| group.is_none_or(|g| o.group == g))
        .map(|o| {
            let mut v = json!({ "tool": o.tool_name(), "cli": format!("noshape-cli {} {}", o.group, o.verb), "summary": o.summary, "mutates": o.mutates });
            // With one group: each operation's examples too.
            let examples = crate::examples::of(&o.tool_name());
            if group.is_some() && !examples.is_empty() {
                v["examples"] = examples.iter().filter_map(|e| serde_json::from_str::<serde_json::Value>(e).ok()).collect();
            }
            v
        })
        .collect();
    Ok(json!({ "operations": ops }).into())
}
