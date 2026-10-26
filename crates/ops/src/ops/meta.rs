//! Discovering the API itself.

use crate::Ctx;
use crate::error::OpError;
use crate::registry::{self, Args, Op, OpOutput, opt, req};
use serde_json::{Value, json};

pub fn ops() -> Vec<Op> {
    vec![
        Op {
            group: "ops",
            verb: "list",
            summary: "List every operation (tool) of the noshape API",
            description: "Without filters: every tool's name and the groups with their sizes (short). With `group` (a group's name, its start or a word in it: \"configuration\" finds config) or `search` (words all found in a tool's name or summary): each tool's name, CLI command, summary and examples. Use `feature_types` and `feature_schema` for the Part Studio features and their parameters, ops_describe for a tool's full documentation.",
            fields: vec![
                opt("group", json!({ "type": "string" }), "Only this group (document, tab, feature, sketch, variable, config, part, material, query, analysis, render, export, import, featurestudio, history, assembly, context, drawing, view, ui...)."),
                opt("search", json!({ "type": "string" }), "Only tools whose name or summary holds all these words (\"pattern assembly\")."),
            ],
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
        let op = registry::by_tool_name(name).ok_or_else(|| OpError::param(&format!("tools[{i}]"), &registry::unknown_tool_message(name)))?;
        out.push(json!({ "tool": name, "cli": format!("noshape-cli {} {}", op.group, op.verb), "description": op.full_description(), "input_schema": op.input_schema(), "mutates": op.mutates }));
    }
    Ok(json!({ "tools": out }).into())
}

fn call(ctx: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let name = a.str("tool")?;
    let op = registry::by_tool_name(name).ok_or_else(|| OpError::param("tool", &registry::unknown_tool_message(name)))?;
    let args = a.get("args").cloned().unwrap_or_else(|| json!({}));
    registry::run(ctx, op, &args)
}

fn list(_: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let all = registry::all();
    let mut groups: Vec<&str> = Vec::new();
    for o in all.iter() {
        if !groups.contains(&o.group) {
            groups.push(o.group);
        }
    }
    // A group by its name, its start ("draw") or a name it starts ("configuration").
    let group = match a.opt_str("group")?.map(|g| g.trim().to_lowercase()) {
        None => None,
        Some(g) => Some(
            groups
                .iter()
                .find(|x| **x == g)
                .or_else(|| groups.iter().find(|x| x.starts_with(g.as_str()) || g.starts_with(**x)))
                .copied()
                .ok_or_else(|| OpError::param("group", &format!("no group `{g}`; groups: {}", groups.join(", "))))?,
        ),
    };
    let words: Vec<String> = a.opt_str("search")?.map(|s| s.split_whitespace().map(str::to_lowercase).collect()).unwrap_or_default();
    let filtered = group.is_some() || !words.is_empty();
    let ops: Vec<_> = all
        .iter()
        .filter(|o| group.is_none_or(|g| o.group == g))
        .filter(|o| {
            let text = format!("{} {}", o.tool_name(), o.summary).to_lowercase();
            words.iter().all(|w| text.contains(w.as_str()))
        })
        .map(|o| {
            if !filtered {
                return json!({ "tool": o.tool_name() });
            }
            let mut v = json!({ "tool": o.tool_name(), "cli": format!("noshape-cli {} {}", o.group, o.verb), "summary": o.summary, "mutates": o.mutates });
            let examples = crate::examples::of(&o.tool_name());
            if !examples.is_empty() {
                v["examples"] = examples.iter().filter_map(|e| serde_json::from_str::<serde_json::Value>(e).ok()).collect();
            }
            v
        })
        .collect();
    if filtered {
        return Ok(json!({ "operations": ops }).into());
    }
    let sizes: Vec<Value> = groups.iter().map(|g| json!({ "group": g, "tools": all.iter().filter(|o| o.group == *g).count() })).collect();
    Ok(json!({ "operations": ops, "groups": sizes, "hint": "ops_list with `group` or `search` gives summaries and examples; ops_describe a tool's full documentation" }).into())
}
