//! noshape-cli: the command line and the MCP server, both generated from
//! the operation registry of `noshape-ops` (see `docs/AGENT_API.md`). The
//! app binary forwards `noshape mcp` / `noshape <group> <verb>` here.

pub mod mcp;

use clap::{Arg, ArgAction, Command};
use noshape_ops::Ctx;
use noshape_ops::registry::{self, OpOutput};
use serde_json::{Value, json};
use std::ffi::OsString;
use std::io::{BufRead, Read};

/// The clap command tree: one subcommand per group, one per operation.
pub fn command() -> Command {
    let mut cmd = Command::new("noshape-cli")
        .about("noshape CAD for scripts and agents: every operation of the app, JSON in and out")
        .after_help("Values are JSON when they parse (numbers, arrays, objects, true/false), strings otherwise.\nExamples:\n  noshape-cli document create --path /tmp/a.noshape\n  noshape-cli sketch create --doc /tmp/a.noshape --plane plane:top\n  noshape-cli call feature_add '{\"doc\":\"/tmp/a.noshape\",\"type\":\"extrude\",\"params\":{\"depth\":\"10 mm\"}}'\n  noshape-cli mcp   # MCP server on stdio")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(Command::new("mcp").about("Run the MCP server (stdio, JSON-RPC 2.0, one message per line)"))
        .subcommand(Command::new("call").about("Run an operation by MCP tool name with a JSON object of arguments (stdin when omitted)").arg(Arg::new("tool").required(true)).arg(Arg::new("json")))
        .subcommand(Command::new("batch").about("Run JSON lines {\"tool\": ..., \"args\": {...}} from a file (or stdin) in one session; prints one JSON result per line").arg(Arg::new("file")))
        .subcommand(Command::new("tools").about("Print the MCP tool list (names, descriptions, input schemas) as JSON"));
    let mut groups: Vec<&str> = registry::all().iter().map(|o| o.group).collect();
    groups.dedup();
    for g in groups {
        let mut gc = Command::new(g).subcommand_required(true).arg_required_else_help(true).about(format!("{g} operations"));
        for op in registry::all().iter().filter(|o| o.group == g) {
            let mut oc = Command::new(op.verb).about(op.summary).long_about(op.full_description());
            for f in &op.fields {
                oc = oc.arg(Arg::new(f.name).long(f.name).value_name(f.name.to_uppercase()).help(f.description.to_string()).action(ArgAction::Set));
            }
            oc = oc.arg(Arg::new("__json").long("json").value_name("OBJECT").help("All arguments as one JSON object (fields given separately override it)"));
            gc = gc.subcommand(oc);
        }
        cmd = cmd.subcommand(gc);
    }
    cmd
}

fn print(v: &Value) {
    println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
}

fn output_json(out: &OpOutput) -> Value {
    let mut v = out.value.clone();
    if !out.images.is_empty() && v.get("file").is_none() {
        v["note"] = json!("pass --file to save the rendered image");
    }
    v
}

/// Parse a command line value: JSON when it parses, else a string.
fn value(s: &str) -> Value {
    serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.to_string()))
}

fn run_tool(ctx: &mut Ctx, tool: &str, args: &Value) -> Result<Value, Value> {
    let op = registry::by_tool_name(tool).ok_or_else(|| json!({ "error": "not_found", "message": format!("unknown tool `{tool}`; run `noshape-cli tools`") }))?;
    registry::run(ctx, op, args).map(|o| output_json(&o)).map_err(|e| e.to_json())
}

/// Run the CLI; returns the exit code.
pub fn run(args: Vec<OsString>) -> i32 {
    let m = match command().try_get_matches_from(args) {
        Ok(m) => m,
        Err(e) => {
            let _ = e.print();
            return if e.use_stderr() { 1 } else { 0 };
        }
    };
    let mut ctx = Ctx::new();
    let result = match m.subcommand() {
        Some(("mcp", _)) => {
            return match mcp::serve(ctx) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("mcp: {e:#}");
                    1
                }
            };
        }
        Some(("tools", _)) => Ok(json!({ "tools": mcp::tool_list() })),
        Some(("call", sm)) => {
            let tool = sm.get_one::<String>("tool").cloned().unwrap_or_default();
            let text = match sm.get_one::<String>("json") {
                Some(t) => t.clone(),
                None => {
                    let mut s = String::new();
                    let _ = std::io::stdin().read_to_string(&mut s);
                    s
                }
            };
            match serde_json::from_str::<Value>(if text.trim().is_empty() { "{}" } else { &text }) {
                Ok(args) => run_tool(&mut ctx, &tool, &args),
                Err(e) => Err(json!({ "error": "invalid_argument", "message": format!("arguments are not JSON: {e}") })),
            }
        }
        Some(("batch", sm)) => {
            let reader: Box<dyn BufRead> = match sm.get_one::<String>("file") {
                Some(f) if f != "-" => match std::fs::File::open(f) {
                    Ok(file) => Box::new(std::io::BufReader::new(file)),
                    Err(e) => {
                        print(&json!({ "error": "io", "message": format!("{f}: {e}") }));
                        return 2;
                    }
                },
                _ => Box::new(std::io::BufReader::new(std::io::stdin())),
            };
            let mut failed = false;
            for line in reader.lines().map_while(Result::ok) {
                if line.trim().is_empty() || line.trim_start().starts_with('#') {
                    continue;
                }
                let r = match serde_json::from_str::<Value>(&line) {
                    Ok(v) => run_tool(&mut ctx, v["tool"].as_str().unwrap_or(""), v.get("args").unwrap_or(&json!({}))),
                    Err(e) => Err(json!({ "error": "invalid_argument", "message": format!("not JSON: {e}") })),
                };
                failed |= r.is_err();
                println!("{}", serde_json::to_string(&r.unwrap_or_else(|e| e)).unwrap_or_default());
            }
            return if failed { 2 } else { 0 };
        }
        Some((group, gm)) => {
            let Some((verb, om)) = gm.subcommand() else { return 1 };
            let Some(op) = registry::by_command(group, verb) else { return 1 };
            let mut args = match om.get_one::<String>("__json") {
                Some(j) => match serde_json::from_str::<Value>(j) {
                    Ok(v @ Value::Object(_)) => v,
                    _ => {
                        print(&json!({ "error": "invalid_argument", "param": "json", "message": "--json takes a JSON object" }));
                        return 2;
                    }
                },
                None => json!({}),
            };
            for f in &op.fields {
                if let Some(v) = om.get_one::<String>(f.name) {
                    let is_string = f.schema.get("type").and_then(Value::as_str) == Some("string");
                    args[f.name] = if is_string { Value::String(v.clone()) } else { value(v) };
                }
            }
            registry::run(&mut ctx, op, &args).map(|o| output_json(&o)).map_err(|e| e.to_json())
        }
        None => return 1,
    };
    match result {
        Ok(v) => {
            print(&v);
            0
        }
        Err(e) => {
            print(&e);
            2
        }
    }
}
