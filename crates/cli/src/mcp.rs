//! MCP server (Model Context Protocol, stdio transport): JSON-RPC 2.0, one
//! message per line. Tools are generated from the operation registry
//! (`noshape_ops::registry`), so the CLI and MCP can't drift.

use base64::Engine as _;
use noshape_ops::registry::{self, OpOutput};
use noshape_ops::Ctx;
use serde_json::{Value, json};
use std::io::{BufRead, Write};

/// Protocol revisions we speak (newest first). We answer with the client's
/// revision when we know it, else our newest.
const PROTOCOL_VERSIONS: [&str; 4] = ["2025-06-18", "2025-03-26", "2024-11-05", "2025-11-25"];

const INSTRUCTIONS: &str = include_str!("mcp_instructions.md");

/// Run the server on stdin/stdout until stdin closes.
pub fn serve(mut ctx: Ctx, all_tools: bool) -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(Value::Array(batch)) => {
                let replies: Vec<Value> = batch.iter().filter_map(|m| handle(&mut ctx, m, all_tools)).collect();
                (!replies.is_empty()).then_some(Value::Array(replies))
            }
            Ok(msg) => handle(&mut ctx, &msg, all_tools),
            Err(e) => Some(error_reply(Value::Null, -32700, &format!("parse error: {e}"))),
        };
        if let Some(r) = reply {
            serde_json::to_writer(&mut out, &r)?;
            out.write_all(b"\n")?;
            out.flush()?;
        }
    }
    Ok(())
}

/// Handle one JSON-RPC message; `None` for notifications. `all_tools`:
/// list every tool in full (else the common ones in short, [`tool_list`]).
pub fn handle(ctx: &mut Ctx, msg: &Value, all_tools: bool) -> Option<Value> {
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let id = msg.get("id").cloned();
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    // Notifications (no id) get no reply.
    let id = id?;
    let result = match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
            let version = PROTOCOL_VERSIONS.iter().find(|v| **v == asked).copied().unwrap_or(PROTOCOL_VERSIONS[0]);
            Ok(json!({
                "protocolVersion": version,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "noshape", "title": "noshape CAD", "version": env!("CARGO_PKG_VERSION") },
                "instructions": if all_tools { INSTRUCTIONS.replace(LISTED_NOTE, ALL_NOTE) } else { INSTRUCTIONS.to_string() },
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_list(all_tools) })),
        "tools/call" => Ok(call_tool(ctx, &params)),
        "resources/list" => Ok(json!({ "resources": [] })),
        "resources/templates/list" => Ok(json!({ "resourceTemplates": [] })),
        "prompts/list" => Ok(json!({ "prompts": [] })),
        _ => Err((-32601, format!("method not found: {method}"))),
    };
    Some(match result {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err((code, m)) => error_reply(id, code, &m),
    })
}

fn error_reply(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// The instructions' paragraph on the abridged tool list, and what it says
/// with every tool listed.
const LISTED_NOTE: &str = "The tool list shows the common tools with short descriptions and their main fields (they accept every field ops_describe lists).";
const ALL_NOTE: &str = "The tool list shows every tool in full.";

/// The MCP tools: every operation in full (`all`), else the common ones
/// ([`crate::core_tools::CORE`]) with short descriptions and main fields.
pub fn tool_list(all: bool) -> Vec<Value> {
    if all {
        return registry::all()
            .iter()
            .map(|op| {
                json!({
                    "name": op.tool_name(),
                    "title": op.summary,
                    "description": op.full_description(),
                    "inputSchema": op.input_schema(),
                    "annotations": {
                        "readOnlyHint": !op.mutates,
                        "destructiveHint": op.destructive,
                        "idempotentHint": !op.mutates,
                        "openWorldHint": false,
                    },
                })
            })
            .collect();
    }
    crate::core_tools::CORE.iter().filter_map(|c| registry::by_tool_name(c.tool).map(|op| compact_tool(op, c))).collect()
}

/// A field's description in short: its first sentence, at most about 100
/// characters.
fn first_sentence(d: &str) -> String {
    let s = d.split_once(". ").map_or(d, |(a, _)| a).trim_end_matches('.');
    if s.chars().count() <= 100 {
        return s.to_string();
    }
    let cut: String = s.chars().take(100).collect();
    format!("{}…", cut.rsplit_once(' ').map_or(cut.as_str(), |(a, _)| a))
}

/// One tool of the abridged list: the short description, `doc` and the
/// listed fields (long nested schemas reduced to their type), the required
/// ones required; other fields stay accepted (no additionalProperties: false).
fn compact_tool(op: &registry::Op, c: &crate::core_tools::Core) -> Value {
    let mut props = serde_json::Map::new();
    if op.fields.iter().any(|f| f.name == "doc") {
        props.insert("doc".into(), json!({ "type": "string" }));
    }
    for (name, brief) in c.fields {
        let Some(f) = op.fields.iter().find(|f| f.name == *name) else { continue };
        let mut schema = f.schema.clone();
        if schema.to_string().len() > 200 {
            let mut short = json!({ "type": schema["type"].clone() });
            if let Some(t) = schema.pointer("/items/type") {
                short["items"] = json!({ "type": t });
            }
            schema = short;
        }
        let d = if brief.is_empty() { first_sentence(f.description) } else { brief.to_string() };
        if !d.is_empty() {
            schema["description"] = Value::String(d);
        }
        props.insert((*name).into(), schema);
    }
    let required: Vec<&str> = op.fields.iter().filter(|f| f.required).map(|f| f.name).collect();
    let annotations = if op.mutates { json!({ "destructiveHint": op.destructive, "openWorldHint": false }) } else { json!({ "readOnlyHint": true, "openWorldHint": false }) };
    json!({ "name": op.tool_name(), "description": c.brief, "inputSchema": { "type": "object", "properties": props, "required": required }, "annotations": annotations })
}

fn call_tool(ctx: &mut Ctx, params: &Value) -> Value {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let Some(op) = registry::by_tool_name(name) else {
        return tool_error(&registry::unknown_tool_message(name));
    };
    match registry::run(ctx, op, &args) {
        Ok(out) => tool_result(out),
        Err(e) => tool_error(&serde_json::to_string(&e.to_json()).unwrap_or_else(|_| e.to_string())),
    }
}

fn tool_result(out: OpOutput) -> Value {
    // Compact JSON: a third fewer tokens than pretty-printed.
    let mut content = vec![json!({ "type": "text", "text": serde_json::to_string(&out.value).unwrap_or_default() })];
    for img in &out.images {
        content.push(json!({ "type": "image", "data": base64::engine::general_purpose::STANDARD.encode(&img.png), "mimeType": "image/png" }));
    }
    let mut r = json!({ "content": content, "isError": false });
    if out.value.is_object() {
        r["structuredContent"] = out.value;
    }
    r
}

fn tool_error(text: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": true })
}
