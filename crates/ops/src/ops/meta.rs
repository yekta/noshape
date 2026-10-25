//! Discovering the API itself.

use crate::Ctx;
use crate::error::OpError;
use crate::registry::{self, Args, Op, OpOutput, opt, req};
use serde_json::json;

pub fn ops() -> Vec<Op> {
    vec![
        Op {
            group: "ops",
            verb: "list",
            summary: "List every operation (tool) of the noshape API",
            description: "Returns each operation's tool name, CLI command and summary, by group. Use `feature_types` and `feature_schema` for the Part Studio features and their parameters.",
            fields: vec![opt("group", json!({ "type": "string" }), "Only this group (document, tab, feature, sketch, variable, config, part, material, query, analysis, render, export, import, featurestudio, history, assembly, context, drawing).")],
            mutates: false,
            destructive: false,
            run: list,
        },
        Op {
            group: "ops",
            verb: "describe",
            summary: "Full documentation of tools: description, examples and input schema",
            description: "Everything the CLI's --help says about each tool: its whole description with examples, and its JSON input schema with every field. The MCP server's tool list carries only short descriptions and the main fields; this gives the rest (any tool, listed or not; run unlisted ones with ops_call).",
            fields: vec![req("tools", json!({ "type": "array", "items": { "type": "string" }, "minItems": 1 }), "Tool names, e.g. [\"drawing_section_view\", \"config_add_input\"].")],
            mutates: false,
            destructive: false,
            run: describe,
        },
        Op {
            group: "ops",
            verb: "call",
            summary: "Run any tool by name with its arguments",
            description: "For tools the MCP tool list doesn't show (see ops_list): runs `tool` with `args` exactly as calling it directly would, and returns its reply.",
            fields: vec![req("tool", json!({ "type": "string" }), "Tool name, e.g. \"drawing_section_view\"."), opt("args", json!({ "type": "object" }), "Its arguments (see ops_describe).")],
            mutates: true,
            destructive: true,
            run: call,
        },
    ]
}

fn describe(_: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let mut out = Vec::new();
    for (i, name) in a.array("tools")?.iter().enumerate() {
        let name = name.as_str().ok_or_else(|| OpError::param(&format!("tools[{i}]"), "a tool name"))?;
        let op = registry::by_tool_name(name).ok_or_else(|| OpError::param(&format!("tools[{i}]"), &format!("unknown tool `{name}`; see ops_list")))?;
        out.push(json!({ "tool": name, "cli": format!("noshape-cli {} {}", op.group, op.verb), "description": op.full_description(), "input_schema": op.input_schema(), "mutates": op.mutates }));
    }
    Ok(json!({ "tools": out }).into())
}

fn call(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let name = a.str("tool")?;
    let op = registry::by_tool_name(name).ok_or_else(|| OpError::param("tool", &format!("unknown tool `{name}`; see ops_list")))?;
    let args = a.get("args").cloned().unwrap_or_else(|| json!({}));
    registry::run(ctx, op, &args)
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
