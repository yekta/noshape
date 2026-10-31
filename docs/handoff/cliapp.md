# cliapp: one binary for the app and the CLI, installed on PATH and connected to coding agents

Started 2026-10-31 (agent cliapp, branch `agent/cliapp`).

## What a user gets

- **One binary.** `noshape` is the app, the CLI and the MCP server. `noshape mcp` and `noshape <command> …` (every
  command noshape-cli takes, plus `--version` / `--help`) run before GPUI starts (crates/app/src/cli_mode.rs →
  `noshape_cli::run`, the same library noshape-cli's 16-line main calls); no command, a file path or a `--demo*`
  flag opens the app as before. `noshape_cli::is_command` decides (the fixed commands, then the registry's groups;
  a path or a flag never builds the registry).
  - Windows: the release app stays a GUI-subsystem exe (no console flash). In CLI mode the standard handles a caller
    passes (an MCP client's pipes, a redirect) are kept; the missing ones are attached to the parent terminal's
    console (`AttachConsole(ATTACH_PARENT_PROCESS)` + `CONOUT$`/`CONIN$`). Known limit of any GUI-subsystem exe:
    an interactive cmd/PowerShell prompt doesn't wait for it, so `noshape --version` typed in a terminal prints after
    the prompt comes back; pipes, redirects and MCP clients are unaffected (release.yml checks both).
  - macOS: `noshape file.noshape` through the /usr/local/bin link reopens the .app bundle with `open -n -a` (paths made
    absolute), so it runs as a regular app.
- **noshape-cli** still builds alone without GUI dependencies (agents' builds, the CLI-only download).
- **Agents.** `noshape agents status|connect|disconnect` (ops `agents_*`, crates/ops/src/install.rs): Claude Code
  (`claude` on PATH or ~/.claude/local, ~/.local/bin, npm/bun/volta folders, /opt/homebrew/bin, /usr/local/bin;
  `claude mcp add --scope user noshape -- <bin> mcp`, falling back to ~/.claude.json when the command fails), Codex
  CLI (`codex mcp add` when `codex mcp add --help` works, else a `[mcp_servers.noshape]` table in
  $CODEX_HOME/config.toml edited as text), Claude Desktop (its folder; JSON merge of `mcpServers.noshape`). Files
  are backed up to `<file>.noshape-backup` before an edit; an entry for this binary is left alone, one for another
  binary is updated. Status only reads files (no agent command runs), so the app's dialog and the installer are fast.
- **Command line tool.** `noshape cli status|install` (ops `cli_*`): a link in /usr/local/bin (macOS: the system's
  administrator prompt through `osascript … with administrator privileges` when needed), else ~/.local/bin with a
  note when that isn't on PATH; Windows: the exe's folder on the user PATH.
- **In the app:** Help ▾ › Install command line tool… / Connect to coding agents… (crates/app/src/agent_setup.rs, the
  menu coverage area "app"), the same two in the macOS app menu, and a one-time "Set up noshape" prompt on the first
  launch of a release build (flag file `setup-offered` in the config dir; skipped when nothing is left to offer, in
  dev builds, and with NOSHAPE_NO_SETUP_PROMPT / NOSHAPE_NO_NATIVE_DIALOGS / NOSHAPE_SMOKE; perf.yml sets the first).
  Each row has a checkbox and the current state; what's chosen runs in the background, then a prompt says what
  happened.

## How each platform installs (release.yml)

| Platform | Downloads | Install |
|---|---|---|
| macOS arm64 | `noshape-<v>-macos-arm64.dmg` (noshape.app only), `noshape-cli-<v>-macos-arm64.tar.gz` | drag to Applications; the first launch / Help menu links `/usr/local/bin/noshape` |
| Windows x86_64 | `noshape-<v>-windows-x86_64-setup.exe` (Inno Setup, packaging/windows/noshape.iss), the portable `.zip`, `noshape-cli-<v>-windows-x86_64.zip` | Program Files, Start menu entry, install folder on the user PATH (removed on uninstall); the finish page offers "Connect noshape to <agents found>" (runs `noshape agents connect` as the original user) |
| Linux x86_64 | `noshape-<v>-linux-x86_64.tar.gz` (with `install.sh`), `noshape-<v>-linux-x86_64.deb`, `noshape-cli-<v>-linux-x86_64.tar.gz` | `./install.sh` (~/.local; `--system` for /usr/local with sudo; `--agents` / `--no-agents` / asks when interactive; `--uninstall`), or `apt install ./….deb` |

The app bundles no longer carry noshape-cli. `.github/actions/noshape-binary` downloads `noshape-[0-9]*-<asset>` so
the CLI-only archives don't match.

## Dry-running the release

`gh workflow run release.yml --ref <branch>` (the `dry_run` input defaults to true): the three bundle jobs, then a
`verify` job per platform on a fresh runner, no GitHub release. verify installs each download as a user would and
runs `.github/release/check_binary.py` (`--version`, then `mcp` initialize + tools/list over pipes) through the PATH
entry: Linux install.sh (with a fake `claude` on PATH for `--agents`), uninstall, the .deb via apt, the CLI tarball;
macOS the dmg (no loose CLI), `cli install --dir /usr/local/bin` and the link; Windows the silent installer (Program
Files, Start menu, user PATH, `Get-Command noshape`, cmd redirect and PowerShell pipe), the silent uninstall (files
and PATH entry gone), the zip and the CLI zip. `publish` needs `verify` and runs only on a tag push (or a dispatch on
a tag with dry_run=false).

## Tests

- crates/app/tests/cli_mode.rs (T1): `noshape mcp` answers initialize + tools/list over stdio with no display;
  `tools`, `feature types`, an error and `document info` print the same through the app binary as through
  `noshape_cli::run_to`; `--version` / `--help` name the app.
- crates/ops/tests/install.rs (T1): detection, Claude Desktop JSON merge/update/remove with backups (macOS and Windows
  layouts on any OS), Codex config.toml as text, and with fake `claude` / `codex` scripts on PATH (unix): own
  commands, not twice, update on another path, fallback to the files, usual install folders, the PATH link.
- crates/ops/tests/e2e.rs `agents_and_command_line_tool_ops`.

## Commits

- 7f319ae9 the app binary is the CLI and the MCP server (landed 10-31).
- 94403ef6 `agents_*` / `cli_*` ops (landed 10-31).
- d7df1ce4 Help › Install command line tool… / Connect to coding agents…, macOS app menu, first-launch prompt (landed
  10-31).
- The release commit (release.yml, installers, install.sh, .deb, CLI-only downloads, README, AGENT_API quick start,
  this file): landed after the dry run on `agent/cliapp-dry` passed on all three platforms.

Dry runs: 41228366250 (Windows: `Out` is reserved in Inno's Pascal Script), 41234738521 (Windows and macOS checks
green; Linux: a clean Ubuntu lacks libxkbcommon-x11, so install.sh now names missing libraries and the check tests
the CLI-only archive and the .deb on the clean runner first), 41239498082.

Seen while landing: ops `live::calls_for_a_served_document_run_in_the_server` failed once on vibe3 under load (the
discovery probe is a 300 ms localhost connect) and passed on the re-run and locally; not touched here.

## Open

- Windows: a terminal prompt doesn't wait for the GUI-subsystem exe (see above). A `noshape.com` console shim next to
  the exe would fix it at the cost of a second (tiny) binary; not done (one binary was the requirement).
- The installer doesn't disconnect agents on uninstall (an entry may then point at a removed exe; `noshape agents
  connect` from a new install updates it).
- macOS and Windows builds aren't notarized/signed beyond ad hoc.
