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
- 5e62371f the release commit (release.yml, installers, install.sh, .deb, CLI-only downloads, README, AGENT_API quick
  start, this file), landed 10-31 after dry run 41239498082 on `agent/cliapp-dry` passed on all three platforms.
- dfb5c385 help and usage name the command (`noshape`, `noshape-cli`) via clap's `bin_name`, not argv[0]'s file
  name: Windows CI run 41243150421 failed `cli_mode::version_and_help_name_the_app` ("Usage: noshape.exe"); the
  cli unit test `usage_names_the_command_not_the_file` reproduces it on any OS. CI run 41254317096 on dfb5c385:
  T1 green on Linux, macOS and Windows (cli_mode tests included).
- 90f9c7d0 (not cliapp's code) the model test probe `custom_features::dump_stacks` (core d77728ea) only on Linux:
  its `syscall` / /proc didn't link on Windows, so scheduled CI 41258265400 built no Windows tests. CI 41267393153
  on that fix: T1 green on all three (70 Windows test binaries ok).

Dry runs: 41228366250 (Windows: `Out` is reserved in Inno's Pascal Script), 41234738521 (Windows and macOS checks
green; Linux: a clean Ubuntu lacks libxkbcommon-x11, so install.sh now names missing libraries and the check tests
the CLI-only archive and the .deb on the clean runner first), 41239498082.

Seen while landing: ops `live::calls_for_a_served_document_run_in_the_server` failed once on vibe3 under load (the
discovery probe is a 300 ms localhost connect) and passed on the re-run and locally; not touched here.

## Open

- ~~Windows terminal prompt doesn't wait~~ done 11-01 (cde0386e): noshape.com (crates/launcher, std only) beside
  noshape.exe in the installer and the zip; terminals run it for `noshape` (.com before .exe), it runs noshape.exe with
  the terminal's handles and waits (exit code passed on); a document / --demo / no argument opens the app without
  waiting (and without passing its handles on, else a pipe reader waits for the window). noshape.exe joins the
  parent's console first. Dry run 41289774607 checks cmd, pwsh and Windows PowerShell (.github/release/check_terminal.ps1).
- ~~Uninstall leaves agents connected~~ done 11-01 (43b1ccdd): `agents disconnect --command <binary>` removes only
  that binary's entries; the Windows uninstaller ([UninstallRun]) and `install.sh --uninstall` run it.
- ~~ops live test flaky under load~~ done 11-01 (7a8d1dd4): discovery files are renamed into place and stale only when
  the app's process is gone or its port refuses; T1 `live::servers_starting_together_stay_discoverable_and_stale_files_go`
  failed 3 of 6 runs on the old code.
- macOS and Windows builds aren't notarized/signed beyond ad hoc.

## 11-02: the setup dialogs answer a click at once (P1, macOS v0.4.20: Help items showed nothing)

The macOS app menu's items are actions dispatched while their window is busy; the dialog was opened with an update of
that same window, which fails inside its own dispatch, and the error was dropped. `in_some_window` now defers to after
the dispatch and logs when no window can take the dialog. The dialog opens within the click with every row
"checking…"; each check runs on the background executor with a 5 s timeout ("couldn't check" after that); Install /
Connect run in the dialog, each row "installing…" then the path installed to or the exact error. T1 (app, GPUI test
platform, runs on macOS CI too): agent_setup::tests (dialog open before any frame with a 30 s-sleeping `claude` on
PATH, a never-answering check times out, the menu action opens the dialog: it failed without the deferral).


## In-app updates (11-02)

- **What a user sees.** At launch (after 3 s) and every 5 minutes the app asks GitHub's `releases/latest` with the
  last answer's ETag (a 304 doesn't count against the 60-an-hour limit; the cache is `<config>/updates/feed.json`; a
  403/429 waits as long as GitHub says). A newer release is downloaded in the background; a card floating at the
  bottom centre of every window (start window and documents alike, clear of the navbar and tab bar; app-core
  `os_menu::set_floating`, the message bubble's look; user 11-03: the navbar slot covered the start window's search)
  says "Update available: v… — downloading n%", then "Updated — restart to finish" with Restart and What's new (the
  release notes, a link to the release page); its ✕ hides it until the next launch.
  Restart lists the open documents (`<config>/updates/reopen.json`), starts the helper and quits; the new version
  starts with `--reopen <list>`. Preferences › Updates turns it off. Code: crates/update (no GUI), crates/app
  `updates.rs`.
- **Trust: keyless Sigstore, no signing key anywhere.** release.yml's `sign` job writes SHA256SUMS over every download
  and signs it with `cosign sign-blob` (cosign 2.5.2) under the workflow's GitHub OIDC identity:
  SHA256SUMS.sigstore.json (a Fulcio certificate, a Rekor v1 entry with its signed entry timestamp). The app
  (crates/update `sigstore.rs`, a small verifier on webpki/ring against Sigstore's trust root embedded as
  `src/trusted_root.json`) accepts only: a certificate chaining to Fulcio at the time Rekor logged it, for code signing,
  whose only subject is exactly `https://github.com/yekta/noshape/.github/workflows/release.yml@refs/tags/<the
  release's own tag>` with GitHub Actions' issuer; the signature over SHA256SUMS; Rekor's promise for this
  certificate, signature and digest. Then the downloaded file must have its SHA256SUMS checksum, and on macOS the
  unpacked app must satisfy the Developer ID requirement pinned to team 44R7KF7645 (`codesign --verify -R`). A release
  without the bundle (older ones) only gets a Download link. Not checked: SCTs, the inclusion proof (cosign's offline
  verification accepts the signed entry timestamp the same way).
  - When Sigstore rotates its Fulcio CA or Rekor key, refresh `src/trusted_root.json` (update-test-fixtures.yml saves
    the one cosign fetched through TUF). When cosign 2.x stops working (Rekor v1 retired), the verifier needs Rekor v2
    (inclusion proof + checkpoint, a timestamp authority) before release.yml moves to cosign 3.
- **Applying (crates/update `apply`, a helper script written next to the download, run after the app quits):** macOS
  swaps noshape.app (the `.app.zip` asset; the system's password prompt when Applications isn't writable) and reopens
  it with `open -n`; Windows installed runs the new setup.exe with `/VERYSILENT` (elevated; the install folder, its
  PATH entry and agent connections stay); the portable zip copies noshape.exe and noshape.com over; Linux tar.gz
  (install.sh) swaps the binary (pkexec when not writable); the .deb runs `pkexec apt-get install` (no pkexec: a
  Download link).
- **Windows across UAC (11-06, P0).** A user's v0.4.28 updated to nothing: UAC's prompt, then no installer, no
  noshape, still v0.4.28. Reproduced on a runner (`update-uac.yml` on agent/updater-uac, run 42313888625): with
  Claude Desktop's `noshape mcp` running from the install folder, the silent installer exits 5 (its files in use)
  and v0.4.28-29's helper exits without a word. v0.4.30-33's helper closes copies it can (not an elevated one) first.
  The installer now closes every copy in its folder itself when silent (`PrepareToInstall`, `CloseApplications=force`),
  so the old helpers, which run each user's next update, install it. Today's helper also waits on the installer's
  handle with a 15-minute limit, judges by the version the registry says was installed (not the exit code alone),
  writes the installer's log beside the download (`update-install.log`), and on any error (a `trap`) starts the old
  noshape with the reason. Runners' own account is the built-in administrator (no UAC split): the test logs a second
  admin account on runas-style to get UAC's Medium token (`.github/release/uac`, in release.yml's dry run).
- **No Restart Manager (11-07).** Restart pressed at once lost GitHub's Windows runner 7 times in 10: the installer's
  Restart Manager (`/FORCECLOSEAPPLICATIONS`, `CloseApplications=force`) closed a scanner of GitHub's that had
  noshape.exe loaded, and with it its console, which the runner shared (a user's terminal could go the same way).
  The installer now has `CloseApplications=no`; `packaging/windows/prepare.ps1` (PrepareToInstall) ends noshape's own
  copies by process and waits up to 120 s for the files, else fails with the reason, which the helper passes on.
  Shown with an image-loaded reader sharing a console with a sentinel (update_across_uac.ps1 `-Reader`): v0.4.33's
  installer closed the sentinel, the new one waited (agent/updater-uac run 42355757410). release.yml's restart job
  presses Restart at once (after 30 s for target releases up to v0.4.34).
- **Tests.** crates/update/tests/update.rs (T1): real Sigstore fixtures (tests/fixtures, made by update-test-fixtures.yml
  on a runner; `identity.txt` is that run's workflow identity) verify, and a changed SHA256SUMS, signature, digest,
  certificate, signed entry timestamp or log time, another signer (a release tag's, a fork's, a prefix), a trust root
  without the CA or the log are each refused; a local server stands in for GitHub (ETag, rate limit, download,
  tampered download, wrong signer, missing bundle); the Linux helper swaps a binary. crates/app updates_tests: the
  card's phases draw in wide and narrow windows, its ✕ hides it for the session; the reopen list is read once.
- **The release dry run updates every install** (`.github/release/check_update.py` in release.yml's verify jobs, dry
  runs only: a release build ignores the test feed): the installed app (Linux install.sh under Xvfb, macOS
  /Applications, Windows installed and portable) started with a document and pointed at a local feed serving this
  build as a test release (NOSHAPE_UPDATE_FEED / NOSHAPE_UPDATE_SELF_VERSION / NOSHAPE_UPDATE_TEST_IDENTITY /
  NOSHAPE_UPDATE_AUTORESTART / NOSHAPE_UPDATE_LOG, all ignored by release builds) finds, downloads, verifies,
  restarts, the helper swaps the install, and the new process reopens the document; then the same release with a
  changed download and with another expected signer are refused. Not covered there: the .deb (pkexec needs a
  desktop).

## Apple signing (11-02)

release.yml (macOS bundle job), from repo secrets: APPLE_CERT_P12 (Developer ID Application, base64) with
APPLE_DEVID_CERT_PASSWORD imported into a temporary keychain (deleted at the end of the job, `always()`); the app
signed with the hardened runtime and a timestamp, zipped and notarized with `notarytool` and the App Store Connect API
key (ASC_KEY_P8 / ASC_KEY_ID / ASC_ISSUER_ID), stapled; the dmg signed, notarized and stapled too; the CLI-only binary
signed. Without the certificate it falls back to the ad-hoc signature (a notice). Bundle ID `app.noshape.noshape`
(packaging/macos/Info.plist). The App Store profiles (APPLE_MAC_APPSTORE_PROFILE, APPLE_IOS_APPSTORE_PROFILE) are only
read to print their application identifier, team and expiry (a warning when they don't name the bundle ID). The
verify job checks the signature's identifier, `spctl` and both staples.

Dry runs of the update cycle: 41441100003 (macOS signed, notarized, stapled; every install verified the update, then:
Linux and macOS waited on the old app as a zombie, fixed 2654a117; the check script reused a folder), 41459697171
(Windows portable: `\\?\` paths broke the helper's Copy-Item, fixed c0b067d1; `sign` failed twice on GitHub's OIDC
token endpoint, transient: re-run), 41469689688 green on all three (installed, portable, .app, install.sh; tampered
and other-signer refused). Windows CI 41453979037: the fixtures are `-text` in .gitattributes (CRLF broke the signed
SHA256SUMS, dc2e9f46).
