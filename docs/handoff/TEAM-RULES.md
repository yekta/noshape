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
- **Build:** `OCCT_ROOT=/root/occt CARGO_TARGET_DIR=/root/target-<you> nice cargo build -p noshape`. Use only your
  own target dir and your own Xvfb display (given in your prompt). Delete `<target>/debug/incremental` when it
  grows past a few GB. Disk is shared (keep more than 15 GB free).
- **Disk:** the disk hit 100% once. After each build, if `<target>/debug/incremental` is over 2 GB, delete it.
  Each test binary is ~130 MB (OCCT is linked in), so old ones pile up. When you're not building, delete the stale
  executables: `find <target>/debug/deps -maxdepth 1 -type f -executable ! -name '*.so' -mmin +60 -delete`.
  Never delete `debug/deps` files while a build might be running.
- **Memory:** 15 GB RAM for everyone; earlyoom kills compilers when it runs out ("could not compile" with no
  error = killed). `~/.cargo/config.toml` sets a rustc wrapper (`~/.local/bin/rustc-throttle`) that lets at most 3
  compiles of noshape's big crates run at once machine-wide, so a build may wait a little for a slot. Don't bypass it.
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
