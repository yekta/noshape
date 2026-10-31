//! The app binary is the CLI: `noshape mcp` serves MCP over stdio and
//! `noshape <command>` prints what noshape-cli prints, without a window (no
//! display here: a window would fail to start).

use serde_json::{Value, json};
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn scratch(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let d = std::env::temp_dir().join(format!("noshape-cli-mode-{name}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

/// The app binary, headless: no display, its own config dir.
fn app(config: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_noshape"));
    c.env_remove("DISPLAY").env_remove("WAYLAND_DISPLAY").env("NOSHAPE_CONFIG_DIR", config);
    c
}

#[test]
fn mcp_answers_initialize_and_tools_list_over_stdio() {
    let dir = scratch("mcp");
    let mut child = app(&dir).arg("mcp").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().expect("start noshape mcp");
    let mut stdin = child.stdin.take().expect("stdin");
    for m in [
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "1" } } }),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
    ] {
        writeln!(stdin, "{m}").expect("write");
    }
    drop(stdin);
    let replies: Vec<Value> = BufReader::new(child.stdout.take().expect("stdout")).lines().map(|l| serde_json::from_str(&l.expect("line")).expect("JSON reply")).collect();
    assert!(child.wait().expect("exit").success());
    assert_eq!(replies.len(), 2, "{replies:?}");
    assert_eq!(replies[0]["id"], 1);
    assert_eq!(replies[0]["result"]["serverInfo"]["name"], "noshape");
    assert_eq!(replies[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(replies[1]["id"], 2);
    let names: Vec<&str> = replies[1]["result"]["tools"].as_array().expect("tools").iter().filter_map(|t| t["name"].as_str()).collect();
    let expected: Vec<String> = noshape_cli::mcp::tool_list(false).iter().filter_map(|t| t["name"].as_str().map(String::from)).collect();
    assert_eq!(names, expected);
    assert!(names.contains(&"feature_add") && names.contains(&"ops_call"), "{names:?}");
    let _ = std::fs::remove_dir_all(dir);
}

/// One command through the app binary and through noshape-cli's code: the
/// same output and exit code.
fn same_as_cli(dir: &Path, args: &[&str]) -> String {
    let out = app(dir).args(args).output().expect("run noshape");
    let mut cli = Vec::new();
    let code = noshape_cli::run_to(std::iter::once("noshape-cli").chain(args.iter().copied()).map(OsString::from).collect(), &mut cli);
    let text = String::from_utf8(out.stdout).expect("utf-8");
    assert_eq!(out.status.code(), Some(code), "{args:?}: {text}");
    assert_eq!(text, String::from_utf8(cli).expect("utf-8"), "{args:?}");
    text
}

#[test]
fn a_cli_command_through_the_app_prints_what_noshape_cli_prints() {
    let dir = scratch("cmd");
    let tools = same_as_cli(&dir, &["tools"]);
    assert!(tools.contains("\"feature_add\""));
    let types = same_as_cli(&dir, &["feature", "types", "--search", "fillet"]);
    assert!(types.contains("fillet"), "{types}");
    let failed = same_as_cli(&dir, &["call", "no_such_tool", "{}"]);
    assert!(failed.contains("not_found"), "{failed}");
    // A document made by the app binary reads back the same through both.
    let doc = dir.join("plate.noshape");
    let doc = doc.to_str().expect("utf-8 path");
    let made = app(&dir).args(["document", "create", "--path", doc]).output().expect("create");
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stdout));
    let info = same_as_cli(&dir, &["document", "info", "--doc", doc]);
    assert!(info.contains("Part Studio 1"), "{info}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn version_and_help_name_the_app() {
    let dir = scratch("version");
    let out = app(&dir).arg("--version").output().expect("run");
    assert!(out.status.success());
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), format!("noshape {}", noshape_ops::VERSION));
    let help = app(&dir).arg("--help").output().expect("run");
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("Usage: noshape ") && help.contains("opens the app") && help.contains("mcp"), "{help}");
    let _ = std::fs::remove_dir_all(dir);
}
