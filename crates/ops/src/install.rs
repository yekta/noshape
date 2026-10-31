//! Setting noshape up on the user's machine: the `noshape` command on PATH
//! and noshape's MCP server registered with the coding agents installed
//! (Claude Code, the Codex CLI, Claude Desktop). Shared by the app (its
//! first-launch prompt and Help menu), the `agents_*` / `cli_*` operations
//! and the installers. Every function takes an [`Env`], so tests run against
//! a fake home and fake `claude` / `codex` commands.

use serde_json::{Map, Value, json};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The MCP server's name in every agent's configuration.
pub const SERVER: &str = "noshape";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Mac,
    Windows,
    Linux,
}

/// Where to look: the user's home, PATH and the per-OS folders.
#[derive(Clone, Debug)]
pub struct Env {
    pub os: Os,
    pub home: PathBuf,
    /// PATH.
    pub path: OsString,
    /// %APPDATA% (Windows: Claude Desktop's folder, npm's commands).
    pub appdata: Option<PathBuf>,
    /// CODEX_HOME (default `~/.codex`).
    pub codex_home: Option<PathBuf>,
    /// Folders outside the home where installers put commands; an app
    /// started from the Finder doesn't have them on PATH.
    pub system_dirs: Vec<PathBuf>,
}

impl Env {
    /// This process's environment.
    pub fn current() -> Env {
        let os = if cfg!(target_os = "macos") {
            Os::Mac
        } else if cfg!(windows) {
            Os::Windows
        } else {
            Os::Linux
        };
        let var = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
        let home = if os == Os::Windows { var("USERPROFILE").or_else(|| var("HOME")) } else { var("HOME") }.unwrap_or_default();
        let system_dirs = match os {
            Os::Windows => Vec::new(),
            _ => ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"].into_iter().map(PathBuf::from).collect(),
        };
        Env { os, home, path: std::env::var_os("PATH").unwrap_or_default(), appdata: var("APPDATA"), codex_home: var("CODEX_HOME"), system_dirs }
    }

    fn path_dirs(&self) -> Vec<PathBuf> {
        std::env::split_paths(&self.path).filter(|d| !d.as_os_str().is_empty()).collect()
    }

    /// A command by name: on PATH, then where its installers put it.
    pub fn find_command(&self, name: &str, usual: &[PathBuf]) -> Option<PathBuf> {
        let exts: &[&str] = if self.os == Os::Windows { &[".exe", ".cmd", ".bat"] } else { &[""] };
        let dirs = self.path_dirs().into_iter().chain(usual.iter().cloned()).chain(self.system_dirs.iter().cloned());
        for d in dirs {
            for e in exts {
                let p = d.join(format!("{name}{e}"));
                if is_executable(&p) {
                    return Some(p);
                }
            }
        }
        None
    }

    fn codex_dir(&self) -> PathBuf {
        self.codex_home.clone().unwrap_or_else(|| self.home.join(".codex"))
    }

    /// Claude Desktop's folder (it has no Linux version).
    fn claude_desktop_dir(&self) -> Option<PathBuf> {
        match self.os {
            Os::Mac => Some(self.home.join("Library/Application Support/Claude")),
            Os::Windows => Some(self.appdata.clone().unwrap_or_else(|| self.home.join("AppData/Roaming")).join("Claude")),
            Os::Linux => None,
        }
    }
}

fn is_executable(p: &Path) -> bool {
    let Ok(m) = std::fs::metadata(p) else { return false };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        m.is_file() && m.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        m.is_file()
    }
}

/// This binary's path, links resolved (what agents should start).
pub fn this_binary() -> Option<PathBuf> {
    let p = std::env::current_exe().ok()?;
    let p = p.canonicalize().unwrap_or(p);
    // Windows' canonical form is `\\?\C:\…`, which agents' configs don't take.
    let s = p.to_string_lossy();
    Some(match s.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => p,
    })
}

// ------------------------------------------------------------------ agents

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agent {
    ClaudeCode,
    Codex,
    ClaudeDesktop,
}

impl Agent {
    pub const ALL: [Agent; 3] = [Agent::ClaudeCode, Agent::Codex, Agent::ClaudeDesktop];

    pub fn id(self) -> &'static str {
        match self {
            Agent::ClaudeCode => "claude_code",
            Agent::Codex => "codex",
            Agent::ClaudeDesktop => "claude_desktop",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Agent::ClaudeCode => "Claude Code",
            Agent::Codex => "Codex CLI",
            Agent::ClaudeDesktop => "Claude Desktop",
        }
    }

    pub fn from_id(id: &str) -> Option<Agent> {
        Agent::ALL.into_iter().find(|a| a.id() == id)
    }

    /// Where the agent's command usually is when it isn't on PATH.
    fn usual_dirs(self, env: &Env) -> Vec<PathBuf> {
        let h = &env.home;
        let mut v = vec![h.join(".local/bin"), h.join(".npm-global/bin"), h.join(".bun/bin"), h.join(".volta/bin")];
        if self == Agent::ClaudeCode {
            v.insert(0, h.join(".claude/local"));
        }
        if let Some(a) = &env.appdata {
            v.push(a.join("npm"));
        }
        v
    }
}

/// An agent's noshape entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    /// The agent isn't installed.
    Missing,
    NotConnected,
    /// noshape's entry starts this binary.
    Connected,
    /// noshape's entry starts another binary (this path).
    Elsewhere(String),
}

impl State {
    pub fn label(&self) -> String {
        match self {
            State::Missing => "not installed".into(),
            State::NotConnected => "not connected".into(),
            State::Connected => "connected".into(),
            State::Elsewhere(p) => format!("connected to another noshape ({p})"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AgentStatus {
    pub agent: Agent,
    /// The agent's command (Claude Code, Codex) or folder (Claude Desktop).
    pub found: Option<PathBuf>,
    /// Its configuration file holding MCP servers.
    pub config: Option<PathBuf>,
    pub state: State,
}

impl AgentStatus {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.agent.id(),
            "name": self.agent.name(),
            "found": self.found.is_some(),
            "path": self.found.as_ref().map(|p| p.display().to_string()),
            "config": self.config.as_ref().map(|p| p.display().to_string()),
            "state": match self.state { State::Missing => "missing", State::NotConnected => "not_connected", State::Connected => "connected", State::Elsewhere(_) => "elsewhere" },
            "connected_command": match &self.state { State::Elsewhere(p) => Some(p.clone()), _ => None },
            "label": self.state.label(),
        })
    }
}

fn same_command(a: &str, b: &Path, os: Os) -> bool {
    let b = b.to_string_lossy();
    if os == Os::Windows { a.eq_ignore_ascii_case(&b) } else { a == b }
}

/// The state of an entry `{command, args}` (JSON) for `command`.
fn entry_state(entry: Option<&Value>, command: &Path, os: Os) -> State {
    let Some(e) = entry else { return State::NotConnected };
    let cmd = e.get("command").and_then(Value::as_str).unwrap_or("");
    let mcp = e.get("args").and_then(Value::as_array).and_then(|a| a.first()).and_then(Value::as_str) == Some("mcp");
    if mcp && same_command(cmd, command, os) { State::Connected } else { State::Elsewhere(cmd.to_string()) }
}

fn claude_code_config(env: &Env) -> PathBuf {
    env.home.join(".claude.json")
}

fn codex_config(env: &Env) -> PathBuf {
    env.codex_dir().join("config.toml")
}

fn read_json(p: &Path) -> Result<Value, String> {
    match std::fs::read_to_string(p) {
        Ok(t) if t.trim().is_empty() => Ok(json!({})),
        Ok(t) => serde_json::from_str(&t).map_err(|e| format!("{} isn't valid JSON ({e}); fix or remove it first", p.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(format!("{}: {e}", p.display())),
    }
}

/// Whether noshape is connected to `agent`, starting `command`.
pub fn status(env: &Env, agent: Agent, command: &Path) -> AgentStatus {
    let (found, config) = match agent {
        Agent::ClaudeCode => (env.find_command("claude", &agent.usual_dirs(env)), Some(claude_code_config(env))),
        Agent::Codex => (env.find_command("codex", &agent.usual_dirs(env)), Some(codex_config(env))),
        Agent::ClaudeDesktop => {
            let dir = env.claude_desktop_dir();
            (dir.clone().filter(|d| d.is_dir()), dir.map(|d| d.join("claude_desktop_config.json")))
        }
    };
    let state = if found.is_none() {
        State::Missing
    } else {
        let cfg = config.as_deref().unwrap_or(Path::new(""));
        match agent {
            Agent::ClaudeCode | Agent::ClaudeDesktop => {
                let v = read_json(cfg).unwrap_or(Value::Null);
                entry_state(v.get("mcpServers").and_then(|s| s.get(SERVER)), command, env.os)
            }
            Agent::Codex => {
                let text = std::fs::read_to_string(cfg).unwrap_or_default();
                let entry = toml_section(&text).map(|(a, b)| toml_entry(&text[a..b]));
                entry_state(entry.as_ref(), command, env.os)
            }
        }
    };
    AgentStatus { agent, found, config, state }
}

/// Every agent's status.
pub fn statuses(env: &Env, command: &Path) -> Vec<AgentStatus> {
    Agent::ALL.into_iter().map(|a| status(env, a, command)).collect()
}

/// What connecting or disconnecting did.
#[derive(Clone, Debug)]
pub struct Change {
    pub agent: Agent,
    /// "connected", "updated", "already connected", "disconnected", "not connected".
    pub result: &'static str,
    /// How: the agent's own command or the configuration file.
    pub via: String,
    /// The configuration file's copy from before the edit.
    pub backup: Option<PathBuf>,
}

impl Change {
    pub fn to_json(&self) -> Value {
        json!({ "id": self.agent.id(), "name": self.agent.name(), "result": self.result, "via": self.via, "backup": self.backup.as_ref().map(|p| p.display().to_string()) })
    }
}

/// Register noshape's MCP server (`<command> mcp`) with `agent`: with its
/// own command where it has one, else in its configuration file (backed
/// up first). An entry starting another binary is updated; one starting
/// this binary is left as it is.
pub fn connect(env: &Env, agent: Agent, command: &Path) -> Result<Change, String> {
    let st = status(env, agent, command);
    let Some(found) = st.found.clone() else { return Err(format!("{} isn't installed", agent.name())) };
    let config = st.config.clone().unwrap_or_default();
    let result = match st.state {
        State::Connected => return Ok(Change { agent, result: "already connected", via: String::new(), backup: None }),
        State::Elsewhere(_) => "updated",
        _ => "connected",
    };
    let cmd = command.to_string_lossy().to_string();
    let mut tried = String::new();
    match agent {
        Agent::ClaudeCode => {
            if st.state != State::NotConnected {
                let _ = run(env, &found, &["mcp", "remove", "--scope", "user", SERVER]);
            }
            match run(env, &found, &["mcp", "add", "--scope", "user", SERVER, "--", &cmd, "mcp"]) {
                Ok(()) if status(env, agent, command).state == State::Connected => return Ok(Change { agent, result, via: "claude mcp add --scope user".into(), backup: None }),
                Ok(()) => tried = "claude mcp add left no entry".into(),
                Err(e) => tried = e,
            }
        }
        Agent::Codex => {
            if run(env, &found, &["mcp", "add", "--help"]).is_ok() {
                if st.state != State::NotConnected {
                    let _ = run(env, &found, &["mcp", "remove", SERVER]);
                }
                match run(env, &found, &["mcp", "add", SERVER, "--", &cmd, "mcp"]) {
                    Ok(()) if status(env, agent, command).state == State::Connected => return Ok(Change { agent, result, via: "codex mcp add".into(), backup: None }),
                    Ok(()) => tried = "codex mcp add left no entry".into(),
                    Err(e) => tried = e,
                }
            }
            let text = std::fs::read_to_string(&config).unwrap_or_default();
            let backup = backup(&config)?;
            let new = toml_with_server(&text, Some(&cmd));
            write(&config, &new)?;
            return Ok(Change { agent, result, via: via_file(&config, &tried), backup });
        }
        Agent::ClaudeDesktop => {}
    }
    // Claude Desktop, or Claude Code when its command failed: the JSON file.
    let mut v = read_json(&config)?;
    let backup = backup(&config)?;
    let mut entry = Map::new();
    if agent == Agent::ClaudeCode {
        entry.insert("type".into(), json!("stdio"));
    }
    entry.insert("command".into(), json!(cmd));
    entry.insert("args".into(), json!(["mcp"]));
    if !v.is_object() {
        v = json!({});
    }
    let servers = v.as_object_mut().expect("object").entry("mcpServers").or_insert_with(|| json!({}));
    if !servers.is_object() {
        *servers = json!({});
    }
    servers.as_object_mut().expect("object").insert(SERVER.into(), Value::Object(entry));
    write(&config, &format!("{}\n", serde_json::to_string_pretty(&v).unwrap_or_default()))?;
    Ok(Change { agent, result, via: via_file(&config, &tried), backup })
}

/// Remove noshape's MCP server from `agent` (whatever binary it starts).
pub fn disconnect(env: &Env, agent: Agent) -> Result<Change, String> {
    let st = status(env, agent, Path::new(""));
    let Some(found) = st.found.clone() else { return Err(format!("{} isn't installed", agent.name())) };
    if st.state == State::NotConnected {
        return Ok(Change { agent, result: "not connected", via: String::new(), backup: None });
    }
    let config = st.config.clone().unwrap_or_default();
    let mut tried = String::new();
    let removed = |env: &Env| status(env, agent, Path::new("")).state == State::NotConnected;
    match agent {
        Agent::ClaudeCode => match run(env, &found, &["mcp", "remove", "--scope", "user", SERVER]) {
            Ok(()) if removed(env) => return Ok(Change { agent, result: "disconnected", via: "claude mcp remove --scope user".into(), backup: None }),
            Ok(()) => tried = "claude mcp remove left the entry".into(),
            Err(e) => tried = e,
        },
        Agent::Codex => {
            if run(env, &found, &["mcp", "remove", "--help"]).is_ok() {
                match run(env, &found, &["mcp", "remove", SERVER]) {
                    Ok(()) if removed(env) => return Ok(Change { agent, result: "disconnected", via: "codex mcp remove".into(), backup: None }),
                    Ok(()) => tried = "codex mcp remove left the entry".into(),
                    Err(e) => tried = e,
                }
            }
            let text = std::fs::read_to_string(&config).unwrap_or_default();
            let backup = backup(&config)?;
            write(&config, &toml_with_server(&text, None))?;
            return Ok(Change { agent, result: "disconnected", via: via_file(&config, &tried), backup });
        }
        Agent::ClaudeDesktop => {}
    }
    let mut v = read_json(&config)?;
    let backup = backup(&config)?;
    if let Some(s) = v.get_mut("mcpServers").and_then(Value::as_object_mut) {
        s.remove(SERVER);
    }
    write(&config, &format!("{}\n", serde_json::to_string_pretty(&v).unwrap_or_default()))?;
    Ok(Change { agent, result: "disconnected", via: via_file(&config, &tried), backup })
}

fn via_file(config: &Path, tried: &str) -> String {
    if tried.is_empty() { format!("edited {}", config.display()) } else { format!("edited {} ({tried})", config.display()) }
}

/// A copy of `file` from before an edit (`<file>.noshape-backup`); none
/// when there is no file yet.
fn backup(file: &Path) -> Result<Option<PathBuf>, String> {
    if !file.is_file() {
        return Ok(None);
    }
    let mut name = file.file_name().unwrap_or_default().to_os_string();
    name.push(".noshape-backup");
    let b = file.with_file_name(name);
    std::fs::copy(file, &b).map_err(|e| format!("could not back up {}: {e}", file.display()))?;
    Ok(Some(b))
}

fn write(file: &Path, text: &str) -> Result<(), String> {
    if let Some(d) = file.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    std::fs::write(file, text).map_err(|e| format!("could not write {}: {e}", file.display()))
}

/// Run an agent's command (at most a minute), with its folder and the
/// usual ones on PATH (an npm-installed `claude` needs `node`).
fn run(env: &Env, program: &Path, args: &[&str]) -> Result<(), String> {
    let mut path: Vec<PathBuf> = program.parent().map(Path::to_path_buf).into_iter().collect();
    path.extend(env.path_dirs());
    path.extend(env.system_dirs.iter().cloned());
    let mut c = Command::new(program);
    c.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    if let Ok(p) = std::env::join_paths(path) {
        c.env("PATH", p);
    }
    if env.os != Os::Windows {
        c.env("HOME", &env.home);
    }
    if let Some(h) = &env.codex_home {
        c.env("CODEX_HOME", h);
    }
    let shown = format!("{} {}", program.file_stem().unwrap_or_default().to_string_lossy(), args.join(" "));
    // A script written a moment ago can still be open in a process another
    // thread is starting ("text file busy").
    let mut tries = 0;
    let mut child = loop {
        match c.spawn() {
            Err(e) if e.raw_os_error() == Some(26) && cfg!(unix) && tries < 20 => {
                tries += 1;
                std::thread::sleep(Duration::from_millis(25));
            }
            r => break r.map_err(|e| format!("{shown}: {e}"))?,
        }
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(s)) if s.success() => return Ok(()),
            Ok(Some(s)) => {
                let mut err = String::new();
                if let Some(mut e) = child.stderr.take() {
                    let _ = std::io::Read::read_to_string(&mut e, &mut err);
                }
                let err = err.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string();
                return Err(format!("{shown} failed ({s}){}", if err.is_empty() { String::new() } else { format!(": {err}") }));
            }
            Ok(None) if start.elapsed() > Duration::from_secs(60) => {
                let _ = child.kill();
                return Err(format!("{shown} took over a minute"));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => return Err(format!("{shown}: {e}")),
        }
    }
}

// --------------------------------------------- Codex's config.toml, as text

/// Whether a line is a table header for noshape's server (or one of its
/// subtables, `[mcp_servers.noshape.env]`).
fn is_our_header(line: &str) -> bool {
    let t = line.trim();
    let Some(inner) = t.strip_prefix('[').and_then(|r| r.split(']').next()) else { return false };
    let key: String = inner.chars().filter(|c| !c.is_whitespace() && *c != '"' && *c != '\'').collect();
    key == format!("mcp_servers.{SERVER}") || key.starts_with(&format!("mcp_servers.{SERVER}."))
}

/// The byte range of noshape's `[mcp_servers.noshape]` table (and its
/// subtables) in `text`.
pub fn toml_section(text: &str) -> Option<(usize, usize)> {
    let mut start = None;
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        let header = line.trim_start().starts_with('[');
        match start {
            None if header && is_our_header(line) => start = Some(at),
            Some(s) if header && !is_our_header(line) => return Some((s, at)),
            _ => {}
        }
        at += line.len();
    }
    start.map(|s| (s, text.len()))
}

/// A TOML string value (`"…"` with escapes or `'…'`).
fn toml_string(v: &str) -> Option<String> {
    let v = v.trim();
    if let Some(r) = v.strip_prefix('\'') {
        return r.split('\'').next().map(str::to_string);
    }
    let r = v.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = r.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                c => out.push(c),
            },
            c => out.push(c),
        }
    }
    None
}

/// The table's `command` and `args`, as JSON.
fn toml_entry(table: &str) -> Value {
    let mut e = Map::new();
    for line in table.lines() {
        let Some((k, v)) = line.split_once('=') else { continue };
        match k.trim() {
            "command" => {
                e.insert("command".into(), json!(toml_string(v).unwrap_or_default()));
            }
            "args" => {
                let items: Vec<String> = v.trim().trim_start_matches('[').trim_end_matches(']').split(',').filter_map(toml_string).collect();
                e.insert("args".into(), json!(items));
            }
            _ => {}
        }
    }
    Value::Object(e)
}

fn toml_quote(s: &str) -> String {
    if !s.contains('\'') && !s.contains('\n') { format!("'{s}'") } else { format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")) }
}

/// `text` with noshape's table set to start `command` (or removed: None);
/// the rest of the file as it was.
pub fn toml_with_server(text: &str, command: Option<&str>) -> String {
    let mut out = match toml_section(text) {
        Some((a, b)) => format!("{}{}", &text[..a], &text[b..]),
        None => text.to_string(),
    };
    if let Some(cmd) = command {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        if !out.is_empty() && !out.ends_with("\n\n") {
            out.push('\n');
        }
        out.push_str(&format!("[mcp_servers.{SERVER}]\ncommand = {}\nargs = [\"mcp\"]\n", toml_quote(cmd)));
    }
    out
}

// ------------------------------------------------- the command line tool

/// Where a `noshape` command that starts `command` is: on PATH, or in the
/// folders the install puts it. `on_path`: that folder is on PATH.
#[derive(Clone, Debug, Default)]
pub struct CliStatus {
    pub installed: Option<PathBuf>,
    pub on_path: bool,
    /// A `noshape` on PATH that starts another binary.
    pub other: Option<PathBuf>,
}

impl CliStatus {
    pub fn to_json(&self) -> Value {
        json!({ "installed": self.installed.is_some(), "path": self.installed.as_ref().map(|p| p.display().to_string()), "on_path": self.on_path, "other": self.other.as_ref().map(|p| p.display().to_string()) })
    }
}

fn exe_name(os: Os) -> &'static str {
    if os == Os::Windows { "noshape.exe" } else { "noshape" }
}

/// The default link folders: /usr/local/bin, then ~/.local/bin.
fn link_dirs(env: &Env) -> Vec<PathBuf> {
    vec![PathBuf::from("/usr/local/bin"), env.home.join(".local/bin")]
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

pub fn cli_status(env: &Env, command: &Path) -> CliStatus {
    let path_dirs = env.path_dirs();
    let mut st = CliStatus::default();
    for d in &path_dirs {
        let p = d.join(exe_name(env.os));
        if !p.exists() {
            continue;
        }
        if same_file(&p, command) {
            st.installed = Some(p);
            st.on_path = true;
            return st;
        }
        if st.other.is_none() {
            st.other = Some(p);
        }
    }
    if env.os != Os::Windows {
        st.installed = link_dirs(env).into_iter().map(|d| d.join("noshape")).find(|p| same_file(p, command));
    }
    st
}

/// Put `noshape` on PATH. macOS and Linux: a link to `command` in `dir`
/// (default /usr/local/bin, asking for an administrator's password on
/// macOS when needed, else ~/.local/bin). Windows: `command`'s folder on
/// the user's PATH.
pub fn cli_install(env: &Env, command: &Path, dir: Option<&Path>) -> Result<Value, String> {
    if env.os == Os::Windows {
        if dir.is_some() {
            return Err("on Windows the app's folder goes on PATH; `dir` is for macOS and Linux".into());
        }
        return windows_add_to_path(command);
    }
    if let Some(d) = dir {
        let link = link(command, d)?;
        return Ok(link_reply(env, &link, None));
    }
    let [system, user] = <[PathBuf; 2]>::try_from(link_dirs(env)).expect("two folders");
    let first = match link(command, &system) {
        Ok(l) => return Ok(link_reply(env, &l, None)),
        Err(e) => e,
    };
    if env.os == Os::Mac {
        match mac_admin_link(command, &system) {
            Ok(()) => return Ok(link_reply(env, &system.join("noshape"), None)),
            Err(e) => log::info!("install command line tool: {e}"),
        }
    }
    let l = link(command, &user)?;
    Ok(link_reply(env, &l, Some(first)))
}

fn link_reply(env: &Env, link: &Path, fallback_because: Option<String>) -> Value {
    let dir = link.parent().unwrap_or(Path::new(""));
    let on_path = env.path_dirs().iter().any(|d| d == dir);
    let note = match (&fallback_because, on_path) {
        (_, false) => Some(format!("{} isn't on your PATH: add `export PATH=\"{}:$PATH\"` to your shell's profile", dir.display(), dir.display())),
        (Some(why), true) => Some(format!("installed for you only ({why})")),
        (None, true) => None,
    };
    json!({ "path": link.display().to_string(), "on_path": on_path, "note": note })
}

/// `dir/noshape` → `command`. An existing link is replaced; a file that
/// isn't one is left alone.
fn link(command: &Path, dir: &Path) -> Result<PathBuf, String> {
    let l = dir.join("noshape");
    if let Ok(m) = std::fs::symlink_metadata(&l) {
        if same_file(&l, command) && m.file_type().is_symlink() {
            return Ok(l);
        }
        if !m.file_type().is_symlink() {
            return Err(format!("{} exists and isn't a link: remove it or choose another folder", l.display()));
        }
        std::fs::remove_file(&l).map_err(|e| format!("{}: {e}", l.display()))?;
    }
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    #[cfg(unix)]
    std::os::unix::fs::symlink(command, &l).map_err(|e| format!("{}: {e}", l.display()))?;
    #[cfg(not(unix))]
    std::fs::copy(command, &l).map(|_| ()).map_err(|e| format!("{}: {e}", l.display()))?;
    Ok(l)
}

/// macOS: the link made with an administrator's rights, through the
/// system's password prompt.
fn mac_admin_link(command: &Path, dir: &Path) -> Result<(), String> {
    let q = |p: &Path| format!("'{}'", p.to_string_lossy().replace('\'', "'\\''"));
    let shell = format!("mkdir -p {d} && ln -sfn {c} {l}", d = q(dir), c = q(command), l = q(&dir.join("noshape")));
    let script = format!("do shell script \"{}\" with prompt \"noshape wants to install the noshape command in {}.\" with administrator privileges", shell.replace('\\', "\\\\").replace('"', "\\\""), dir.display());
    let out = Command::new("/usr/bin/osascript").arg("-e").arg(script).output().map_err(|e| format!("osascript: {e}"))?;
    if out.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&out.stderr).trim().to_string()) }
}

/// Windows: `command`'s folder on the user's PATH (HKCU\Environment, which
/// new terminals read).
fn windows_add_to_path(command: &Path) -> Result<Value, String> {
    let dir = command.parent().ok_or("no folder")?.to_string_lossy().to_string();
    let script = "$d = $env:NOSHAPE_DIR; $p = [Environment]::GetEnvironmentVariable('Path', 'User'); $parts = @($p -split ';' | Where-Object { $_ -ne '' }); if ($parts -notcontains $d) { [Environment]::SetEnvironmentVariable('Path', (($parts + $d) -join ';'), 'User'); 'added' } else { 'present' }";
    let out = Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", script]).env("NOSHAPE_DIR", &dir).stdin(Stdio::null()).output().map_err(|e| format!("powershell: {e}"))?;
    if !out.status.success() {
        return Err(format!("could not change the user PATH: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(json!({ "path": dir, "on_path": true, "note": "new terminals find `noshape`" }))
}
