//! The MCP server's default tool list stays small enough to load (the
//! common tools in short), loses nothing (every tool reachable through
//! ops_list / ops_describe / ops_call), and `--all-tools` still lists
//! everything in full.

use noshape_cli::core_tools::CORE;
use noshape_cli::mcp;
use noshape_ops::Ctx;
use noshape_ops::registry;
use serde_json::{Value, json};

fn rpc(ctx: &mut Ctx, method: &str, params: Value, all: bool) -> Value {
    mcp::handle(ctx, &json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }), all).expect("reply")["result"].clone()
}

#[test]
fn default_tool_list_fits_an_agents_context() {
    let mut ctx = Ctx::new();
    let init = rpc(&mut ctx, "initialize", json!({ "protocolVersion": "2025-06-18" }), false);
    let tools = rpc(&mut ctx, "tools/list", json!({}), false);
    let up_front = tools.to_string().len() + init["instructions"].as_str().unwrap().len();
    assert!(up_front < 40_000, "tool list + instructions: {up_front} characters");
    assert_eq!(tools["tools"].as_array().unwrap().len(), CORE.len());
}

#[test]
fn core_tools_show_their_required_fields() {
    for c in CORE {
        let op = registry::by_tool_name(c.tool).unwrap_or_else(|| panic!("no tool {}", c.tool));
        for (name, _) in c.fields {
            assert!(op.fields.iter().any(|f| f.name == *name), "{}: no field {name}", c.tool);
        }
        for f in op.fields.iter().filter(|f| f.required) {
            assert!(c.fields.iter().any(|(n, _)| *n == f.name), "{}: required field {} not listed", c.tool, f.name);
        }
    }
}

#[test]
fn every_tool_stays_reachable() {
    let mut ctx = Ctx::new();
    let all = rpc(&mut ctx, "tools/list", json!({}), true);
    assert_eq!(all["tools"].as_array().unwrap().len(), registry::all().len());
    // ops_list names every tool; ops_describe documents any; ops_call runs any.
    let listed = rpc(&mut ctx, "tools/call", json!({ "name": "ops_list", "arguments": {} }), false);
    assert_eq!(listed["structuredContent"]["operations"].as_array().unwrap().len(), registry::all().len());
    let d = rpc(&mut ctx, "tools/call", json!({ "name": "ops_describe", "arguments": { "tools": ["drawing_section_view"] } }), false);
    let full = &d["structuredContent"]["tools"][0];
    assert_eq!(full["input_schema"], registry::by_tool_name("drawing_section_view").unwrap().input_schema(), "{d}");
    let r = rpc(&mut ctx, "tools/call", json!({ "name": "ops_call", "arguments": { "tool": "ops_list", "args": { "group": "ops" } } }), false);
    assert_eq!(r["structuredContent"]["operations"].as_array().unwrap().len(), 3, "{r}");
    // A tool the short list doesn't show is still callable directly too.
    let direct = rpc(&mut ctx, "tools/call", json!({ "name": "drawing_templates", "arguments": {} }), false);
    assert!(!direct.to_string().contains("unknown tool"), "{direct}");
}
