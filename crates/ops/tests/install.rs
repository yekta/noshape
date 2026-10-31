//! Connecting noshape to coding agents and putting `noshape` on PATH
//! (`noshape_ops::install`), against fake homes and fake `claude` / `codex`
//! commands: no real install is read or touched.

use noshape_ops::install::{self, Agent, Env, Os, State};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn scratch(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let d = std::env::temp_dir().join(format!("noshape-install-{name}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

/// A fake home with nothing installed and nothing on PATH.
fn env(home: &Path, os: Os) -> Env {
    Env { os, home: home.to_path_buf(), path: Default::default(), appdata: (os == Os::Windows).then(|| home.join("AppData/Roaming")), codex_home: Some(home.join(".codex")), system_dirs: Vec::new() }
}

fn read_json(p: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(p).expect("read")).expect("JSON")
}

const APP: &str = "/Applications/noshape.app/Contents/MacOS/noshape";

#[test]
fn only_installed_agents_are_found() {
    let home = scratch("none");
    for os in [Os::Mac, Os::Windows, Os::Linux] {
        let all = install::statuses(&env(&home, os), Path::new(APP));
        assert_eq!(all.len(), 3);
        assert!(all.iter().all(|s| s.found.is_none() && s.state == State::Missing), "{os:?}: {all:?}");
    }
    // Claude Desktop: its folder (macOS: Application Support; Windows: %APPDATA%); none on Linux.
    std::fs::create_dir_all(home.join("Library/Application Support/Claude")).unwrap();
    std::fs::create_dir_all(home.join("AppData/Roaming/Claude")).unwrap();
    for (os, found) in [(Os::Mac, true), (Os::Windows, true), (Os::Linux, false)] {
        let s = install::status(&env(&home, os), Agent::ClaudeDesktop, Path::new(APP));
        assert_eq!(s.found.is_some(), found, "{os:?}");
    }
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn claude_desktop_entry_is_merged_kept_once_updated_and_removed() {
    let home = scratch("desktop");
    let dir = home.join("Library/Application Support/Claude");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("claude_desktop_config.json");
    let before = json!({ "globalShortcut": "Alt+Space", "mcpServers": { "files": { "command": "npx", "args": ["-y", "files"] } } });
    std::fs::write(&file, serde_json::to_string_pretty(&before).unwrap()).unwrap();
    let e = env(&home, Os::Mac);
    assert_eq!(install::status(&e, Agent::ClaudeDesktop, Path::new(APP)).state, State::NotConnected);

    let c = install::connect(&e, Agent::ClaudeDesktop, Path::new(APP)).expect("connect");
    assert_eq!(c.result, "connected");
    assert_eq!(read_json(c.backup.as_ref().expect("backup")), before, "the backup is the file from before");
    let v = read_json(&file);
    assert_eq!(v["mcpServers"]["noshape"], json!({ "command": APP, "args": ["mcp"] }));
    assert_eq!(v["mcpServers"]["files"], before["mcpServers"]["files"], "other servers kept");
    assert_eq!(v["globalShortcut"], "Alt+Space");
    assert_eq!(install::status(&e, Agent::ClaudeDesktop, Path::new(APP)).state, State::Connected);

    // Not twice; another binary's entry is updated.
    assert_eq!(install::connect(&e, Agent::ClaudeDesktop, Path::new(APP)).unwrap().result, "already connected");
    let other = "/opt/noshape/noshape";
    assert_eq!(install::status(&e, Agent::ClaudeDesktop, Path::new(other)).state, State::Elsewhere(APP.into()));
    assert_eq!(install::connect(&e, Agent::ClaudeDesktop, Path::new(other)).unwrap().result, "updated");
    assert_eq!(read_json(&file)["mcpServers"]["noshape"]["command"], other);
    assert_eq!(read_json(&file)["mcpServers"].as_object().unwrap().len(), 2);

    let d = install::disconnect(&e, Agent::ClaudeDesktop).expect("disconnect");
    assert_eq!(d.result, "disconnected");
    let v = read_json(&file);
    assert!(v["mcpServers"].get("noshape").is_none());
    assert_eq!(v["mcpServers"]["files"], before["mcpServers"]["files"]);
    assert_eq!(install::disconnect(&e, Agent::ClaudeDesktop).unwrap().result, "not connected");
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn claude_desktop_without_a_config_file_gets_one() {
    let home = scratch("desktop-new");
    std::fs::create_dir_all(home.join("AppData/Roaming/Claude")).unwrap();
    let e = env(&home, Os::Windows);
    let exe = r"C:\Program Files\noshape\noshape.exe";
    let c = install::connect(&e, Agent::ClaudeDesktop, Path::new(exe)).expect("connect");
    assert!(c.backup.is_none());
    let v = read_json(&home.join("AppData/Roaming/Claude/claude_desktop_config.json"));
    assert_eq!(v, json!({ "mcpServers": { "noshape": { "command": exe, "args": ["mcp"] } } }));
    // Windows paths compare without case.
    assert_eq!(install::status(&e, Agent::ClaudeDesktop, Path::new(&exe.to_lowercase())).state, State::Connected);
    let _ = std::fs::remove_dir_all(home);
}

#[test]
fn codex_config_toml_table_is_added_replaced_and_removed_as_text() {
    let text = "model = \"o4\"\n\n[mcp_servers.files]\ncommand = \"npx\"\nargs = [\"-y\", \"files\"]\n";
    let added = install::toml_with_server(text, Some(APP));
    assert_eq!(added, format!("{text}\n[mcp_servers.noshape]\ncommand = '{APP}'\nargs = [\"mcp\"]\n"));
    // Replaced in place of the old table (with its subtables); the rest kept.
    let old = "[mcp_servers.noshape]\ncommand = \"/old/noshape\"\nargs = [\"mcp\"]\n\n[mcp_servers.noshape.env]\nX = \"1\"\n\n[profiles.a]\nmodel = \"x\"\n";
    let replaced = install::toml_with_server(old, Some(r"C:\it's\noshape.exe"));
    assert_eq!(replaced, "[profiles.a]\nmodel = \"x\"\n\n[mcp_servers.noshape]\ncommand = \"C:\\\\it's\\\\noshape.exe\"\nargs = [\"mcp\"]\n");
    assert_eq!(install::toml_with_server(&added, None).trim_end(), text.trim_end());
    assert!(install::toml_section("[mcp_servers.\"noshape\"]\ncommand = 'x'\n").is_some());
    assert!(install::toml_section("[mcp_servers.noshape_dev]\n").is_none());
}

#[cfg(unix)]
mod fake_commands {
    use super::*;

    /// An executable shell script `dir/name`.
    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(dir).unwrap();
        let p = dir.join(name);
        std::fs::write(&p, format!("#!/bin/sh\n{body}")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    /// `claude`: logs its arguments; `mcp add` / `mcp remove` write
    /// ~/.claude.json as Claude Code does (fails when ~/fail exists).
    const CLAUDE: &str = r#"echo "$*" >> "$HOME/claude-calls"
[ -f "$HOME/fail" ] && exit 1
if [ "$1 $2" = "mcp add" ]; then printf '{"mcpServers":{"noshape":{"type":"stdio","command":"%s","args":["mcp"]}}}' "$7" > "$HOME/.claude.json"; fi
if [ "$1 $2" = "mcp remove" ]; then printf '{"mcpServers":{}}' > "$HOME/.claude.json"; fi
exit 0
"#;

    fn calls(home: &Path, name: &str) -> Vec<String> {
        std::fs::read_to_string(home.join(name)).unwrap_or_default().lines().map(str::to_string).collect()
    }

    #[test]
    fn claude_code_through_its_own_command() {
        let home = scratch("claude");
        let bin = home.join("bin");
        script(&bin, "claude", CLAUDE);
        let mut e = env(&home, Os::Linux);
        e.path = bin.clone().into_os_string();
        let s = install::status(&e, Agent::ClaudeCode, Path::new(APP));
        assert_eq!((s.found.as_deref(), &s.state), (Some(bin.join("claude").as_path()), &State::NotConnected));

        let c = install::connect(&e, Agent::ClaudeCode, Path::new(APP)).expect("connect");
        assert_eq!((c.result, c.via.as_str()), ("connected", "claude mcp add --scope user"));
        assert_eq!(calls(&home, "claude-calls"), [format!("mcp add --scope user noshape -- {APP} mcp")]);
        // Not twice.
        assert_eq!(install::connect(&e, Agent::ClaudeCode, Path::new(APP)).unwrap().result, "already connected");
        assert_eq!(calls(&home, "claude-calls").len(), 1);
        // Another binary: removed, then added with the new path.
        let other = home.join("noshape-2/noshape");
        assert_eq!(install::connect(&e, Agent::ClaudeCode, &other).unwrap().result, "updated");
        assert_eq!(calls(&home, "claude-calls")[1..], ["mcp remove --scope user noshape".to_string(), format!("mcp add --scope user noshape -- {} mcp", other.display())]);
        assert_eq!(install::disconnect(&e, Agent::ClaudeCode).unwrap().result, "disconnected");
        assert_eq!(install::status(&e, Agent::ClaudeCode, &other).state, State::NotConnected);
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn claude_code_in_its_usual_place_and_its_file_when_the_command_fails() {
        let home = scratch("claude-fail");
        // Not on PATH: Claude Code's own install folder.
        script(&home.join(".claude/local"), "claude", CLAUDE);
        std::fs::write(home.join("fail"), "").unwrap();
        std::fs::write(home.join(".claude.json"), r#"{"numStartups": 3, "mcpServers": {"web": {"type": "stdio", "command": "web", "args": []}}}"#).unwrap();
        let e = env(&home, Os::Mac);
        let c = install::connect(&e, Agent::ClaudeCode, Path::new(APP)).expect("connect");
        assert!(c.via.starts_with("edited ") && c.via.contains("claude mcp add --scope user noshape"), "{}", c.via);
        assert!(c.backup.is_some());
        let v = read_json(&home.join(".claude.json"));
        assert_eq!(v["mcpServers"]["noshape"], json!({ "type": "stdio", "command": APP, "args": ["mcp"] }));
        assert_eq!(v["mcpServers"]["web"]["command"], "web");
        assert_eq!(v["numStartups"], 3);
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn codex_with_and_without_mcp_add() {
        // A Codex with `codex mcp add`.
        let home = scratch("codex");
        let bin = home.join("bin");
        script(&bin, "codex", r#"echo "$*" >> "$HOME/codex-calls"
case "$1 $2 $3" in "mcp add --help"|"mcp remove --help") exit 0;; esac
if [ "$1 $2" = "mcp add" ]; then printf '[mcp_servers.noshape]\ncommand = "%s"\nargs = ["mcp"]\n' "$5" >> "$CODEX_HOME/config.toml"; fi
if [ "$1 $2" = "mcp remove" ]; then : > "$CODEX_HOME/config.toml"; fi
exit 0
"#);
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        let mut e = env(&home, Os::Linux);
        e.path = bin.into_os_string();
        let c = install::connect(&e, Agent::Codex, Path::new(APP)).expect("connect");
        assert_eq!((c.result, c.via.as_str()), ("connected", "codex mcp add"));
        assert_eq!(calls(&home, "codex-calls"), ["mcp add --help".to_string(), format!("mcp add noshape -- {APP} mcp")]);
        assert_eq!(install::status(&e, Agent::Codex, Path::new(APP)).state, State::Connected);
        assert_eq!(install::connect(&e, Agent::Codex, Path::new(APP)).unwrap().result, "already connected");
        assert_eq!(install::disconnect(&e, Agent::Codex).unwrap().via, "codex mcp remove");
        let _ = std::fs::remove_dir_all(home);

        // An older Codex: its config.toml, other tables kept.
        let home = scratch("codex-old");
        script(&home.join(".local/bin"), "codex", "echo \"$*\" >> \"$HOME/codex-calls\"\nexit 2\n");
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        let cfg = home.join(".codex/config.toml");
        std::fs::write(&cfg, "model = \"o4\"\n\n[mcp_servers.noshape]\ncommand = \"/old/noshape\"\nargs = [\"mcp\"]\n").unwrap();
        let e = env(&home, Os::Linux);
        assert_eq!(install::status(&e, Agent::Codex, Path::new(APP)).state, State::Elsewhere("/old/noshape".into()));
        let c = install::connect(&e, Agent::Codex, Path::new(APP)).expect("connect");
        assert_eq!(c.result, "updated");
        assert!(c.via.starts_with("edited "), "{}", c.via);
        assert_eq!(std::fs::read_to_string(&cfg).unwrap(), format!("model = \"o4\"\n\n[mcp_servers.noshape]\ncommand = '{APP}'\nargs = [\"mcp\"]\n"));
        assert!(std::fs::read_to_string(c.backup.unwrap()).unwrap().contains("/old/noshape"));
        install::disconnect(&e, Agent::Codex).expect("disconnect");
        assert_eq!(std::fs::read_to_string(&cfg).unwrap().trim_end(), "model = \"o4\"");
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn command_line_tool_link() {
        let home = scratch("link");
        let app = script(&home.join("noshape.app/Contents/MacOS"), "noshape", "exit 0\n");
        let other = script(&home.join("old"), "noshape", "exit 0\n");
        let dir = home.join("bin");
        let mut e = env(&home, Os::Linux);
        e.path = dir.clone().into_os_string();
        assert!(install::cli_status(&e, &app).installed.is_none());
        // An old link is replaced; the folder is on PATH.
        std::fs::create_dir_all(&dir).unwrap();
        std::os::unix::fs::symlink(&other, dir.join("noshape")).unwrap();
        assert_eq!(install::cli_status(&e, &app).other.as_deref(), Some(dir.join("noshape").as_path()));
        let r = install::cli_install(&e, &app, Some(&dir)).expect("install");
        assert_eq!(r["on_path"], true);
        assert_eq!(std::fs::read_link(dir.join("noshape")).unwrap(), app);
        let st = install::cli_status(&e, &app);
        assert_eq!((st.installed.as_deref(), st.on_path), (Some(dir.join("noshape").as_path()), true));
        // A folder off PATH says so; a file that isn't a link is left alone.
        let off = home.join("off");
        let r = install::cli_install(&e, &app, Some(&off)).expect("install");
        assert!(r["note"].as_str().unwrap().contains("isn't on your PATH"), "{r}");
        std::fs::remove_file(off.join("noshape")).unwrap();
        std::fs::write(off.join("noshape"), "").unwrap();
        assert!(install::cli_install(&e, &app, Some(&off)).unwrap_err().contains("isn't a link"));
        let _ = std::fs::remove_dir_all(home);
    }
}
