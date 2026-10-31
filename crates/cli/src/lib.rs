//! noshape-cli: the command line and the MCP server, both generated from
//! the operation registry of `noshape-ops` (see `docs/AGENT_API.md`). The
//! app binary runs them too: `noshape mcp` / `noshape <group> <verb>` come
//! here before any window starts ([`is_command`]).

pub mod core_tools;
pub mod mcp;

use clap::{Arg, ArgAction, Command};
use noshape_ops::Ctx;
use noshape_ops::registry::{self, OpOutput};
use serde_json::{Value, json};
use std::ffi::{OsStr, OsString};
use std::io::{BufRead, Read, Write};

/// `--all-tools`: the MCP tool list with every tool in full, instead of the
/// common ones with short descriptions (the rest through ops_list /
/// ops_describe / ops_call).
fn all_tools_arg() -> Arg {
    Arg::new("all-tools").long("all-tools").action(ArgAction::SetTrue).help("List every tool with its full description and schema (about 340k characters) instead of the common ones in short")
}

/// The commands besides the operation groups.
const COMMANDS: [&str; 5] = ["mcp", "call", "batch", "tools", "docs"];

/// Whether the app binary's first argument is a CLI command (or --help,
/// --version) rather than a document to open or a `--demo` flag.
pub fn is_command(arg: &OsStr) -> bool {
    let Some(a) = arg.to_str() else { return false };
    if COMMANDS.contains(&a) || matches!(a, "help" | "--help" | "-h" | "--version" | "-V") {
        return true;
    }
    // A path or a flag: the app (without building the registry).
    a.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') && registry::groups().contains(&a)
}

/// The clap command tree: one subcommand per group, one per operation.
pub fn command() -> Command {
    command_named("noshape-cli")
}

/// The command tree under the program's name: `noshape-cli`, or `noshape`
/// for the app binary (which opens the app when given no command).
pub fn command_named(name: &str) -> Command {
    let about = if name == "noshape" {
        "noshape CAD. With no command (or a .noshape file) it opens the app; with a command it runs it for scripts and agents, JSON in and out"
    } else {
        "noshape CAD for scripts and agents: every operation of the app, JSON in and out"
    };
    let mut cmd = Command::new(name.to_string())
        .version(noshape_ops::VERSION)
        .about(about)
        .after_help(format!("Values are JSON when they parse (numbers, arrays, objects, true/false), strings otherwise.\nExamples:\n  {name} document create --path /tmp/a.noshape\n  {name} sketch create --doc /tmp/a.noshape --plane plane:top\n  {name} call feature_add '{{\"doc\":\"/tmp/a.noshape\",\"type\":\"extrude\",\"params\":{{\"depth\":\"10 mm\"}}}}'\n  {name} mcp   # MCP server on stdio\n  {name} agents connect   # register it with Claude Code, Codex, Claude Desktop"))
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(Command::new("mcp").about("Run the MCP server (stdio, JSON-RPC 2.0, one message per line)").arg(all_tools_arg()))
        .subcommand(Command::new("call").about("Run an operation by MCP tool name with a JSON object of arguments (stdin when omitted)").arg(Arg::new("tool").required(true)).arg(Arg::new("json")))
        .subcommand(Command::new("batch").about("Run JSON lines {\"tool\": ..., \"args\": {...}} from a file (or stdin) in one session; prints one JSON result per line").arg(Arg::new("file")))
        .subcommand(Command::new("tools").about("Print the MCP tool list (names, descriptions, input schemas) as JSON").arg(all_tools_arg()))
        .subcommand(Command::new("docs").about("Print the operation reference of docs/AGENT_API.md (Markdown)"));
    for g in registry::groups() {
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

/// Write the output; a reader that went away (`| head`) ends it quietly
/// instead of a panic.
fn emit(out: &mut dyn Write, text: &str) {
    if out.write_all(text.as_bytes()).and_then(|_| out.flush()).is_err() {
        std::process::exit(0);
    }
}

fn print(out: &mut dyn Write, v: &Value) {
    emit(out, &format!("{}\n", serde_json::to_string_pretty(v).unwrap_or_default()));
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
    let op = registry::by_tool_name(tool).ok_or_else(|| json!({ "error": "not_found", "message": registry::unknown_tool_message(tool) }))?;
    registry::run(ctx, op, args).map(|o| output_json(&o)).map_err(|e| e.to_json())
}

/// Run the CLI (`args[0]` the program); returns the exit code.
pub fn run(args: Vec<OsString>) -> i32 {
    run_to(args, &mut std::io::stdout().lock())
}

/// [`run`] with the results written to `out` (help, usage errors and the
/// MCP server still use stdout / stderr).
pub fn run_to(args: Vec<OsString>, out: &mut dyn Write) -> i32 {
    let name = args.first().map(std::path::Path::new).and_then(|p| p.file_stem()).and_then(OsStr::to_str).filter(|n| *n == "noshape").unwrap_or("noshape-cli");
    let m = match command_named(name).try_get_matches_from(args) {
        Ok(m) => m,
        Err(e) => {
            let _ = e.print();
            return if e.use_stderr() { 1 } else { 0 };
        }
    };
    let mut ctx = Ctx::new();
    let result = match m.subcommand() {
        Some(("mcp", sub)) => {
            return match mcp::serve(ctx, sub.get_flag("all-tools")) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("mcp: {e:#}");
                    1
                }
            };
        }
        Some(("tools", sub)) => Ok(json!({ "tools": mcp::tool_list(sub.get_flag("all-tools")) })),
        Some(("docs", _)) => {
            emit(out, &noshape_ops::reference::markdown());
            return 0;
        }
        Some(("call", sm)) => {
            ctx.one_shot = true;
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
                        print(out, &json!({ "error": "io", "message": format!("{f}: {e}") }));
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
                emit(out, &format!("{}\n", serde_json::to_string(&r.unwrap_or_else(|e| e)).unwrap_or_default()));
            }
            return if failed { 2 } else { 0 };
        }
        Some((group, gm)) => {
            let Some((verb, om)) = gm.subcommand() else { return 1 };
            let Some(op) = registry::by_command(group, verb) else { return 1 };
            ctx.one_shot = true;
            let mut args = match om.get_one::<String>("__json") {
                Some(j) => match serde_json::from_str::<Value>(j) {
                    Ok(v @ Value::Object(_)) => v,
                    _ => {
                        print(out, &json!({ "error": "invalid_argument", "param": "json", "message": "--json takes a JSON object" }));
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
            print(out, &v);
            0
        }
        Err(e) => {
            print(out, &e);
            2
        }
    }
}

#[cfg(test)]
mod tests {
    /// clap's own checks (no duplicate group or verb, valid args): a
    /// broken tree panics on every call.
    #[test]
    fn command_tree_is_valid() {
        super::command().debug_assert();
        super::command_named("noshape").debug_assert();
    }

    /// The app binary hands these to the CLI; a document path or a --demo
    /// flag opens the app.
    #[test]
    fn app_arguments_that_are_commands() {
        use std::ffi::OsStr;
        for a in ["mcp", "tools", "docs", "call", "batch", "document", "feature", "--version", "--help", "-h"] {
            assert!(super::is_command(OsStr::new(a)), "{a}");
        }
        for a in ["/tmp/a.noshape", "a.noshape", "--demo", "--demo-drawing", "Documents"] {
            assert!(!super::is_command(OsStr::new(a)), "{a}");
        }
    }
}
