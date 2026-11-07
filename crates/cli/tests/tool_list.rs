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
fn instructions_name_the_apps_documents_folder() {
    let mut ctx = Ctx::new();
    for all in [false, true] {
        let text = rpc(&mut ctx, "initialize", json!({ "protocolVersion": "2025-06-18" }), all)["instructions"].as_str().unwrap().to_string();
        assert!(!text.contains("{documents}"), "placeholder left in the instructions");
        let line = text.lines().find(|l| l.contains("the documents folder is")).expect("the documents folder line");
        assert!(line.contains("noshape") && line.contains(std::path::MAIN_SEPARATOR), "{line}");
    }
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

/// A document the MCP server (kept running by the agent's app) touched has nothing beside it once a call returns
/// (user 11-07: a friend's folder held a 3.7 MB Panda.noshape-wal): its `.noshape` alone, copied or sent, has every
/// edit, and the server goes on with the document at the next call.
#[test]
fn a_document_the_server_touched_has_no_side_files_between_calls() {
    let dir = std::env::temp_dir().join(format!("noshape-cli-mcp-files-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("Panda.noshape");
    let side = |suffix: &str| dir.join(format!("Panda.noshape{suffix}"));
    let mut ctx = Ctx::new();
    let call = |ctx: &mut Ctx, tool: &str, args: Value| {
        let r = rpc(ctx, "tools/call", json!({ "name": tool, "arguments": args }), false);
        assert_eq!(r["isError"], false, "{tool}: {r}");
        r["structuredContent"].clone()
    };
    call(&mut ctx, "document_create", json!({ "path": path.to_str().unwrap() }));
    let sketch = call(&mut ctx, "sketch_create", json!({ "plane": "plane:top" }))["sketch"].as_str().unwrap().to_string();
    for suffix in ["-wal", "-shm"] {
        assert!(!side(suffix).exists(), "{suffix} left after a call");
    }
    call(&mut ctx, "sketch_draw", json!({ "sketch": sketch, "tool": "corner_rectangle", "points": [[0, 0], [40, 30]] }));
    for suffix in ["-wal", "-shm"] {
        assert!(!side(suffix).exists(), "{suffix} left after a call");
    }
    // The file alone, elsewhere, has the sketch.
    let copy = dir.join("copy");
    std::fs::create_dir_all(&copy).unwrap();
    std::fs::copy(&path, copy.join("Panda.noshape")).unwrap();
    let mut other = Ctx::new();
    let opened = registry::run(&mut other, registry::by_tool_name("document_open").unwrap(), &json!({ "path": copy.join("Panda.noshape").to_str().unwrap() })).unwrap().value;
    assert!(opened["tabs"].is_array(), "{opened}");
    let list = registry::run(&mut other, registry::by_tool_name("feature_list").unwrap(), &json!({})).unwrap().value;
    assert!(list.to_string().contains("Sketch 1"), "{list}");
    drop(other);
    drop(ctx);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The instructions are documentation of the tools: nothing in them reads as orders to the model or as a message
/// about the conversation (user 11-08: a session paused on "Confirming a reminder's authenticity" after our text).
#[test]
fn instructions_describe_the_tools_and_address_no_one() {
    let mut ctx = Ctx::new();
    let text = rpc(&mut ctx, "initialize", json!({ "protocolVersion": "2025-06-18" }), false)["instructions"].as_str().unwrap().to_lowercase();
    for phrase in ["you ", "you'", "your ", "the user", "tell ", "remind", "must ", "important", "system", "assistant", "always ", "never "] {
        assert!(!text.contains(phrase), "the instructions say {phrase:?}");
    }
}

/// An agent's usual first calls (user 11-08: a real session started with four failures) either work or fail with
/// one error that says what to do next: a bare name, "~/…", an absolute path, a file already there (made here
/// moments ago: a retry; made before: the error names document_open), and document_info before any document.
#[test]
fn an_agents_first_calls_work_or_say_what_to_do() {
    let scratch = std::env::temp_dir().join(format!("noshape-cli-first-calls-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();
    let documents = scratch.join("documents");
    let mut ctx = Ctx::new();
    ctx.documents = Some(documents.clone());
    let call = |ctx: &mut Ctx, tool: &str, args: Value| rpc(ctx, "tools/call", json!({ "name": tool, "arguments": args }), false);
    let ok = |r: &Value| -> std::path::PathBuf {
        assert_eq!(r["isError"], false, "{r}");
        std::path::PathBuf::from(r["structuredContent"]["path"].as_str().unwrap())
    };
    let says_what_to_do = |r: &Value, next: &[&str]| {
        assert_eq!(r["isError"], true, "{r}");
        let text = r["content"][0]["text"].as_str().unwrap();
        assert!(next.iter().any(|n| text.contains(n)), "the error doesn't say what to do: {text}");
    };
    // Before any document.
    says_what_to_do(&call(&mut ctx, "document_info", json!({})), &["document_open", "document_create"]);
    // A bare name: the documents folder.
    let made = ok(&call(&mut ctx, "document_create", json!({ "path": "Paper Towel Dispenser - Queen.noshape", "length_unit": "mm" })));
    assert_eq!(made, documents.join("Paper Towel Dispenser - Queen.noshape"));
    // The same again at once (a retry): the same document.
    let again = call(&mut ctx, "document_create", json!({ "path": "Paper Towel Dispenser - Queen.noshape", "length_unit": "mm" }));
    assert_eq!(ok(&again), made);
    assert_eq!(again["structuredContent"]["existed"], true);
    // A file made before (another session): the error says what to do; `if_exists: "open"` opens it.
    let mut other = Ctx::new();
    other.documents = Some(documents.clone());
    says_what_to_do(&call(&mut other, "document_create", json!({ "path": made.to_str().unwrap() })), &["document_open"]);
    ok(&call(&mut other, "document_create", json!({ "path": made.to_str().unwrap(), "if_exists": "open" })));
    drop(other);
    // An absolute path of this OS.
    let absolute = scratch.join("elsewhere").join("Bracket.noshape");
    assert_eq!(ok(&call(&mut ctx, "document_create", json!({ "path": absolute.to_str().unwrap() }))), absolute);
    // "~/…": the home folder's.
    let home = dirs::home_dir().expect("a home folder");
    let rel = format!("noshape-cli-first-calls-{}", std::process::id());
    let in_home = ok(&call(&mut ctx, "document_create", json!({ "path": format!("~/{rel}/Documents/x.noshape") })));
    assert_eq!(in_home, home.join(&rel).join("Documents").join("x.noshape"));
    assert!(in_home.exists());
    // document_open by the bare name: the documents folder's.
    ok(&call(&mut ctx, "document_open", json!({ "path": "Paper Towel Dispenser - Queen" })));
    drop(ctx);
    let _ = std::fs::remove_dir_all(home.join(&rel));
    let _ = std::fs::remove_dir_all(&scratch);
}
