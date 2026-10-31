//! This machine's setup (Help › Install command line tool…, Help › Connect
//! to coding agents…, the first-launch prompt and the installers): the
//! `noshape` command on PATH, and noshape's MCP server registered with the
//! coding agents installed. `noshape agents connect` from a terminal.

use crate::error::OpError;
use crate::install::{self, Agent, Env};
use crate::registry::{Args, Op, OpOutput, opt};
use crate::Ctx;
use serde_json::{Value, json};
use std::path::PathBuf;

fn agents_field(what: &'static str) -> crate::registry::Field {
    opt("agents", json!({ "type": "array", "items": { "type": "string", "enum": ["claude_code", "codex", "claude_desktop"] } }), what)
}

fn command_field() -> crate::registry::Field {
    opt("command", json!({ "type": "string" }), "The noshape binary agents start (`<command> mcp`). Default: this one (the app, or noshape-cli).")
}

pub fn ops() -> Vec<Op> {
    vec![
        Op {
            group: "agents",
            verb: "status",
            summary: "The coding agents on this computer and whether noshape is connected to them",
            description: "Claude Code (`claude` on PATH or where its installers put it), the Codex CLI (`codex`) and Claude Desktop (its folder: macOS ~/Library/Application Support/Claude, Windows %APPDATA%\\Claude): for each, `found`, its configuration file and `state`: missing (not installed), not_connected, connected (noshape's MCP server starts this binary) or elsewhere (`connected_command`: another noshape binary). `found_names` lists the ones installed.",
            fields: vec![command_field()],
            mutates: false,
            destructive: false,
            run: agents_status,
        },
        Op {
            group: "agents",
            verb: "connect",
            summary: "Register noshape's MCP server with the coding agents installed (Help › Connect to coding agents…)",
            description: "Adds `noshape` (`<command> mcp`) to each agent's MCP servers, user-wide: Claude Code with `claude mcp add --scope user`, the Codex CLI with `codex mcp add` (older versions: a [mcp_servers.noshape] table in ~/.codex/config.toml), Claude Desktop in claude_desktop_config.json (other servers kept). A file is backed up first (`<file>.noshape-backup`). An entry already starting this binary is left as it is; one starting another binary is updated. Restart Claude Desktop to load it.",
            fields: vec![agents_field("Agents to connect (claude_code, codex, claude_desktop). Default: every one installed."), command_field()],
            mutates: false,
            destructive: false,
            run: agents_connect,
        },
        Op {
            group: "agents",
            verb: "disconnect",
            summary: "Remove noshape's MCP server from the coding agents",
            description: "Removes the `noshape` entry from each agent's MCP servers (with its own command where it has one, else from its configuration file, backed up first). Other servers are kept.",
            fields: vec![agents_field("Agents to disconnect. Default: every one connected.")],
            mutates: false,
            destructive: true,
            run: agents_disconnect,
        },
        Op {
            group: "cli",
            verb: "status",
            summary: "Whether the `noshape` command is on PATH",
            description: "`installed` and `path`: a `noshape` on PATH (or in /usr/local/bin, ~/.local/bin) that starts this binary; `on_path`: its folder is on PATH; `other`: a `noshape` on PATH that starts another binary.",
            fields: vec![command_field()],
            mutates: false,
            destructive: false,
            run: cli_status,
        },
        Op {
            group: "cli",
            verb: "install",
            summary: "Put the `noshape` command on PATH (Help › Install command line tool…)",
            description: "macOS and Linux: a link `noshape` to this binary in `dir`, default /usr/local/bin (macOS asks for an administrator's password through the system prompt when needed), else ~/.local/bin, with a note when that folder isn't on PATH. An existing link is replaced. Windows: the app's folder is added to the user's PATH (the installer does this already).",
            fields: vec![opt("dir", json!({ "type": "string" }), "Folder for the link (macOS, Linux). Default: /usr/local/bin, else ~/.local/bin."), command_field()],
            mutates: false,
            destructive: false,
            run: cli_install,
        },
    ]
}

fn command(a: &Args) -> Result<PathBuf, OpError> {
    match a.opt_str("command")? {
        Some(c) => Ok(PathBuf::from(c)),
        None => install::this_binary().ok_or_else(|| OpError::new("io", "could not tell this program's path; pass `command`")),
    }
}

/// `agents` as ids: an array, or a comma-separated string (the CLI's
/// `--agents claude_code,codex`).
fn agents(a: &Args) -> Result<Option<Vec<Agent>>, OpError> {
    let ids: Vec<String> = match a.get("agents") {
        None => return Ok(None),
        Some(Value::String(s)) => s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
        Some(Value::Array(v)) => v.iter().map(|x| x.as_str().map(str::to_string).ok_or_else(|| OpError::param("agents", "agent ids (strings)"))).collect::<Result<_, _>>()?,
        Some(_) => return Err(OpError::param("agents", "an array of agent ids")),
    };
    ids.iter().map(|id| Agent::from_id(id).ok_or_else(|| OpError::param("agents", &format!("`{id}` isn't an agent (claude_code, codex, claude_desktop)")))).collect::<Result<Vec<_>, _>>().map(Some)
}

fn agents_status(_: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let cmd = command(a)?;
    let all = install::statuses(&Env::current(), &cmd);
    let found: Vec<&str> = all.iter().filter(|s| s.found.is_some()).map(|s| s.agent.name()).collect();
    Ok(json!({ "command": cmd.display().to_string(), "agents": all.iter().map(install::AgentStatus::to_json).collect::<Vec<_>>(), "found_names": found.join(", ") }).into())
}

/// Each change, or the first failure with what was done before it.
fn changes(list: Vec<Agent>, mut f: impl FnMut(Agent) -> Result<install::Change, String>) -> Result<OpOutput, OpError> {
    let mut done = Vec::new();
    for agent in list {
        match f(agent) {
            Ok(c) => done.push(c.to_json()),
            Err(e) => return Err(OpError::new("agent", format!("{}: {e}", agent.name())).with_param("agents").with_detail(json!({ "done": done }))),
        }
    }
    Ok(json!({ "agents": done }).into())
}

fn agents_connect(_: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let cmd = command(a)?;
    let env = Env::current();
    let list = match agents(a)? {
        Some(l) => l,
        None => install::statuses(&env, &cmd).into_iter().filter(|s| s.found.is_some()).map(|s| s.agent).collect(),
    };
    if list.is_empty() {
        return Ok(json!({ "agents": [], "note": "no coding agent found (Claude Code, Codex CLI, Claude Desktop)" }).into());
    }
    changes(list, |agent| install::connect(&env, agent, &cmd))
}

fn agents_disconnect(_: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let env = Env::current();
    let list = match agents(a)? {
        Some(l) => l,
        None => install::statuses(&env, std::path::Path::new("")).into_iter().filter(|s| matches!(s.state, install::State::Connected | install::State::Elsewhere(_))).map(|s| s.agent).collect(),
    };
    changes(list, |agent| install::disconnect(&env, agent))
}

fn cli_status(_: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let cmd = command(a)?;
    Ok(install::cli_status(&Env::current(), &cmd).to_json().into())
}

fn cli_install(_: &mut Ctx, a: &Args) -> Result<OpOutput, OpError> {
    let cmd = command(a)?;
    let env = Env::current();
    let dir = a.opt_str("dir")?.map(|d| match d.strip_prefix("~/") {
        Some(rest) => env.home.join(rest),
        None => PathBuf::from(d),
    });
    install::cli_install(&env, &cmd, dir.as_deref()).map(Into::into).map_err(|e| OpError::new("io", e))
}
