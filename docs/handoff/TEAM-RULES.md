# Rules for every agent working in /root/noshape right now

Several agents (each on its own account) edit the same working tree, /root/noshape, at the same time.
The lead integrates: it builds, tests and commits the whole tree to `main` whenever it's green.

- **Goal:** noshape is a local Rust + GPUI (gpui-kit) clone of Onshape's CAD side. The user wants it to look and
  work *exactly* like Onshape (colors, fonts, sizes, icons, layout, dialogs, menus, every behaviour and shortcut),
  except for cloud features (sign-in, sharing, notifications). They are very unhappy with anything that "looks
  nothing like Onshape". Must be extremely performant, and work on macOS, Linux and Windows.
- **No deviations from Onshape. Ever.** Never skip an Onshape behaviour, never mark something a "deliberate
  difference", "local-only simplification" or "not needed in our model", and never settle for a workaround because
  GPUI or our data model makes it hard. Change the model or the platform code instead. If something seems
  impossible, find the closest way to make it look and behave identically (custom platform code, your own
  rendering, model changes) and keep going. If you really are stuck, message the lead, who decides; the user is
  never asked, so don't wait. The only exclusions
  are cloud-only features (sign-in, sharing, comments, notifications, subscriptions) and simulation/render tools.
- **Read first:** docs/HANDOFF.md, docs/AGENTS.md, docs/ARCHITECTURE.md, docs/ONSHAPE_UI_SPEC.md, plus your own
  docs/handoff/<you>.md and docs/parity/<area>.md if they exist.
- **Live Onshape reference:** screenshots, DOM dumps and measurements of the real app are in
  /root/onshape-research/live/ (see `behaviours.md` there; the live-capture agent keeps adding to it). Live captures
  beat help-center images (/root/onshape-research/ui-ref/, docs-raw/). The Onshape login is in env vars
  ONSHAPE_EMAIL / ONSHAPE_PASSWORD (from /root/.config/noshape/onshape.env). Never print, log or commit them.
  Only the live-capture agent drives the Chrome instance on DISPLAY=:107 (CDP port 9333). If you need something
  captured, write the request into /root/onshape-research/live/REQUESTS.md (one line per request, your name).
- **Build (changed 2026-10-20): three shared target dirs, one per group** (bigger machine: 16 cores, 30 GB):
  - UI group (ui, ui2, qa*): `CARGO_TARGET_DIR=/root/target-shared`
  - Behaviour group (psb, skb, asb, drw*): `CARGO_TARGET_DIR=/root/target-behave`
  - Engine group (kern, fs): `CARGO_TARGET_DIR=/root/target-engine`
  Use `OCCT_ROOT=/root/occt CARGO_TARGET_DIR=<your group's dir> nice cargo build -p noshape`. Cargo locks each dir, so builds
  within a group queue ("Blocking waiting for file lock" is normal) and reuse each other's output. Rules: never `cargo clean`;
  never delete anything in a shared target dir (the lead clears them); batch edits; prefer `cargo check` and single tests.
  **Right after your build, copy the binary** to /tmp/<you>/ and run that copy. Build in the shared tree /root/noshape.
  A private worktree is allowed only when the shared tree is broken outside your area, and it must still use your group's dir.
- **Disk:** never delete anything in /root/target-shared (including debug/incremental); the lead clears it at quiet moments.
  Only your own non-shared dirs (private worktrees, /tmp/<you>) are yours to clean.
  Each test binary is ~130 MB (OCCT is linked in), so old ones pile up. When you're not building, delete the stale
  executables: `find <target>/debug/deps -maxdepth 1 -type f -executable ! -name '*.so' -mmin +60 -delete`.
  Never delete `debug/deps` files while a build might be running.
- **Machine (upgraded 2026-10-19): 16 cores, 30 GB RAM, 301 GB disk.** After a reboot /tmp is wiped: restart your own Xvfb display
  (`Xvfb :10x -screen 0 1600x1000x24 &`) if it's missing and recreate your /tmp/<you> folder.
- **Memory:** 30 GB RAM for everyone; earlyoom kills compilers when it runs out ("could not compile" with no
  error = killed). `~/.cargo/config.toml` sets a rustc wrapper (`~/.local/bin/rustc-throttle`) that lets at most 6
  compiles of noshape's big crates run at once machine-wide, so a build may wait a little for a slot. Don't bypass it.
- **Xvfb screenshots:** with no window manager the window is never active, so carets and selections don't paint.
  Run `xdotool windowfocus <window>` before capturing.
- **New windows** must open with `os_menu::open_window` (not `gpui_kit::open_window`), so the window draws its own
  menus on top.
- **No time-limited background waits/watchers** (they end your turn). Run builds and tests in the foreground.
- **Never copy Onshape's files into the repo** (it's public): no SVG/PNG/font/CSS/JS from /root/onshape-research
  (ui-ref, bundle-raw, live, env-maps, …). Use them only as reference: redraw icons, re-implement behaviour,
  write our own data. Physical/engineering data (material values, ISO tables) is fine.
- **Never use broad `pkill -f` patterns.** Kill only your own processes (`pkill -f "[t]arget-<you>/debug/noshape"`).
- **Keep the shared tree compiling.** Make edits in small steps that build. If someone else's file breaks the
  build, wait a few minutes and retry instead of "fixing" their in-progress code. Stay in your area; if you need a
  change in another agent's area, make it minimal and surgical.
- **Zero warnings:** no new compiler or clippy warnings in your code (`cargo clippy -p noshape --all-targets
  -- -D warnings` must show nothing from your files). Don't hide warnings with `#[allow]`.
- **Don't commit or push.** The lead does that.
- **Verify in the app** (Xvfb screenshot + xdotool), side by side with the live Onshape reference, not just in tests.
- **Handoff:** keep docs/handoff/<you>.md current after every milestone: state, what's done, what's next, gotchas,
  so a fresh agent can continue if you run out of quota. Keep the parity checklist for your area current.
- **Final message:** say what you finished, what's left, and anything the lead must do (cross-area changes,
  Cargo.lock changes, files that are half done).

## C++ must build on GCC 13
The release runners use Ubuntu 24.04 (GCC 13); this machine has GCC 15, which accepts things GCC 13 rejects
(v0.4.1's release build failed on an implicit `handle<Derived>` -> `handle<Base>` return). Spell such
conversions out (`occ::handle<Base>(x)`). Before you finish C++ kernel changes, syntax-check them with
`g++-13 -std=c++17 -fsyntax-only -w -Icpp -I/root/occt/include/opencascade -I<out>/cxxbridge/include -I<out>/cxxbridge/crate <file>`
(`<out>` = your target dir's `debug/build/noshape-kernel-*/out`).

## Resource limits (2026-10-20, after the server ran out of memory and rebooted)
- Only the agents the lead lists as active may build or run the app. Paused agents run no cargo and no app at all.
- Compiles of the big crates are capped at 3 machine-wide (rustc-throttle), and every cargo run at `jobs = 6`
  (~/.cargo/config.toml). Don't override either one.
- At most **one** app instance per agent. Quit it as soon as your check is done; don't leave apps running.
- No extra target dirs. Use your group's dir. A cross-target check dir is OK only if you delete it right after.
- Before you start a build, check `free -g`. If "available" is under 8 GB, wait and retry in a few minutes.
- /tmp is wiped on reboot. Keep anything you need (plans, patches, scripts) under /root/<agent>-scratch or in
  your handoff file.

## Zero warnings, enforced (2026-10-20)
CI now builds with `RUSTFLAGS=-D warnings` and runs `cargo clippy --locked --workspace --all-targets -- -D warnings`
(the app crate and its tests included), and so does the lead's integration. A single warning anywhere blocks
integration for everyone, so run that clippy command on your crate before you report.

## Never build a private worktree into a shared target dir (2026-10-20)
Cargo gives the vendored crates the same build ID from every checkout, but rustc's crate ID depends on the
absolute path. So building /root/noshape-<you> into /root/target-shared (or target-behave or target-engine)
overwrites artifacts that the shared tree then treats as up to date. Symptoms: "required to be available in
rlib format", or missing gpui items. A private worktree needs its own CARGO_TARGET_DIR (ask the lead first,
since it costs disk) or no build at all. Use it only to read and diff.

## Keep the shared tree compiling (2026-10-21)
Twelve agents build from this one tree. A file left non-compiling, even for a few minutes while you build,
measure or think, blocks everyone's builds and the integration. Make each edit so the file still compiles:
add the import together with its use, and change the callers in the same step. Run `cargo check -p <crate>`
right after any multi-step change. If you have to stop mid-change, revert to compiling first.
