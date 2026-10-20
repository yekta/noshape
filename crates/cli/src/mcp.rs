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
pub fn serve(mut ctx: Ctx) -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(Value::Array(batch)) => {
                let replies: Vec<Value> = batch.iter().filter_map(|m| handle(&mut ctx, m)).collect();
                (!replies.is_empty()).then_some(Value::Array(replies))
            }
            Ok(msg) => handle(&mut ctx, &msg),
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

/// Handle one JSON-RPC message; `None` for notifications.
pub fn handle(ctx: &mut Ctx, msg: &Value) -> Option<Value> {
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
                "instructions": INSTRUCTIONS,
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tool_list() })),
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

/// Every operation as an MCP tool.
pub fn tool_list() -> Vec<Value> {
    registry::all()
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
        .collect()
}

fn call_tool(ctx: &mut Ctx, params: &Value) -> Value {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let Some(op) = registry::by_tool_name(name) else {
        return tool_error(&format!("unknown tool `{name}`; call `ops_list` for the list"));
    };
    match registry::run(ctx, op, &args) {
        Ok(out) => tool_result(out),
        Err(e) => tool_error(&serde_json::to_string_pretty(&e.to_json()).unwrap_or_else(|_| e.to_string())),
    }
}

fn tool_result(out: OpOutput) -> Value {
    let mut content = vec![json!({ "type": "text", "text": serde_json::to_string_pretty(&out.value).unwrap_or_default() })];
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
