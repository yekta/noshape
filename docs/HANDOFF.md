# noshape: handoff for the next lead

Read this first, then `docs/handoff/*.md` (one file per workstream, written by the agents),
`docs/AGENTS.md`, `docs/ARCHITECTURE.md`, `docs/ONSHAPE_UI_SPEC.md` and `docs/parity/*.md`.

## What the user wants

- **A clone of Onshape** that looks and works exactly like Onshape: light theme, Onshape's colors,
  fonts, layout, buttons, icons and behaviours. The only exceptions are cloud features (sign-in,
  sharing, notifications).
- **CAD side only:** sketching, part modeling, assemblies, drawings, documents/versions,
  variables/configurations, import/export. No simulation, render studio, etc.
- **Stack and platforms:** Rust + GPUI (gpui-kit 0.7.3), extremely performant, macOS, Linux and
  Windows, fully local (no cloud).
- **Every Onshape behaviour, shortcut and interaction** reproduced. Example from the user: clicking a sketch while an Extrude
  region field is active selects all its regions. Agents build checklists from Onshape's docs in
  `docs/parity/` and work through them.
- **Zero compiler and clippy warnings,** enforced in CI. Don't hide warnings with `#[allow]`.
- **Keep the machine's disk clean.** Delete build dirs of finished work and incremental caches.
- **Send the user UI screenshots from time to time.**
- **Repo:** github.com/yekta/noshape (public). Commit as
  `git -c user.name="yekta" -c user.email="ngyekta@gmail.com"` and push to `main`.

## Constraints already decided

- **No account workarounds.** Onshape blocks disposable emails at sign-up; we do not evade that.
  If the user supplies a real account or inbox, use Chrome (`/usr/bin/google-chrome`, Node 24,
  Playwright in `/tmp/pw`) to study the live app.
- **No copied Onshape assets.** Don't put Onshape's own SVG or image assets in the public repo. The icons in
  `crates/app/assets/icons/` are redrawn on Onshape's 20-unit grid. Reference material lives outside the repo in
  `/root/onshape-research/` (help pages, 5,148 help images in `ui-ref/`, the public CSS tokens, and the
  icon sprite rendered for visual reference).
- **Licensing is undecided.** `Cargo.toml` says Apache-2.0 but there is no LICENSE file, and OCCT is LGPL-2.1
  with an exception, statically linked. This is the user's decision; we flagged it.

## State of `main` (last commit: see `git log`)

- **Features:** full CAD feature set, parity with Onshape's FeatureScript feature list (every
  feature has an evaluator), sheet metal, frames, custom features (Rhai Feature Studios), assemblies, drawings,
  versions/branches/merge/compare, derived parts across documents, start window, STL/OBJ import.
- **Icons:** the Onshape-style icon set is committed (`44cdcd5`), and so is `docs/ONSHAPE_UI_SPEC.md`. The generator scripts that produced the icons were in /tmp and are gone, so edit the SVGs directly. Conventions: 24x24 viewBox, a `scale(1.2)` group on a 20-unit grid, colors #333333/#1651B0/#009400/#BD3039/#FAB601, mask-safe. See crates/app/assets/icons/README.md.
- **Release:** v0.1.0 is published (macOS arm64 dmg, Windows zip, Linux tar.gz) at
  https://github.com/yekta/noshape/releases/tag/v0.1.0. It still has the old dark theme.
- **CI** (`.github/workflows/ci.yml`): build, test and library clippy on all three OSes, with Node 24 actions.
  - `release.yml` builds bundles on `v*` tags.
  - To re-release, move the tag: `git push origin :refs/tags/vX && git tag -a vX -m ... && git push origin vX`. Bump the version instead
    when appropriate.

## Uncommitted work in `/root/noshape` (backed up on branch `wip`)

The working tree holds the work of the agents below, much of it finished but not yet on `main`.
Branch `wip` on GitHub is a snapshot of the whole working tree. Refresh it with:

```sh
cd /root/noshape && export GIT_INDEX_FILE=/tmp/wip-index && cp .git/index $GIT_INDEX_FILE && git add -A \
 && tree=$(git write-tree) && unset GIT_INDEX_FILE \
 && c=$(git -c user.name=yekta -c user.email=ngyekta@gmail.com commit-tree $tree -p HEAD -m "WIP snapshot") \
 && git push -f origin $c:refs/heads/wip
```

**Finished, not yet on main:**
- Drawing polish round 2: broken-out and partial sections, ordinate groups, templates, cell formatting, moving views
  between sheets.
- Performance pass: lazy picking, readback pipeline, `NOSHAPE_PERF=1` timing logs, faster
  startup. See the ARCHITECTURE.md performance section.
- QA round 1: 12 bug fixes, undo rebuilt across sessions, parameter bounds checks, hole defaults.
  Regression tests are in `crates/model/tests/qa_regressions.rs`.
- Warning cleanup in most app files.
- The stricter `ci.yml` is staged in the working tree:
  - `RUSTFLAGS=-D warnings`;
  - `cargo clippy --workspace --all-targets -- -D warnings`.

  Commit it only once the whole workspace is warning-free.

**In progress** (each agent keeps `docs/handoff/<name>.md` current):

| Workstream | Handoff file | Status |
|---|---|---|
| Theme and layout restyle to Onshape (light theme, layout, dialogs, icons via `img()`) | `docs/handoff/theme-layout.md` | in progress |
| Part Studio behaviour parity, pass 2 | `docs/handoff/part-studio-behaviour.md`, `docs/parity/part-studio.md` | 207 items: 164 done, 43 remaining |
| Sketch behaviour parity, pass 2 | `docs/handoff/sketch-behaviour.md`, `docs/parity/sketch.md` | 200 items: 161 done, 39 remaining |
| Assembly behaviour parity, pass 2 | `docs/handoff/assembly-behaviour.md`, `docs/parity/assembly.md` | 222 items: 175 done; next are flexible subassemblies, live mate values, standard content |
| QA round 2 + renderer bugs | `docs/handoff/qa-round-2.md` | Fixing: pick depth bias for sketch regions on faces, dotted edges (z-fighting), stale frame after edits, zoom-to-fit ignoring planes, `#` autocomplete. Testing: configurations, assemblies, drawings, versions, import/export. |

Not started yet:
- **Drawing behaviour parity:** same method as the others, writing `docs/parity/drawing.md`.
- **Release v0.2.0:** once the restyle and parity work land on main and CI is green with strict
  warnings.

## How to integrate (the lead's job)

1. **Wait for a stable tree.** Wait until agents pause, or let them finish. Check
   `cargo build -p noshape`, `cargo test --workspace --exclude noshape` and
   `cargo clippy --workspace --all-targets -- -D warnings`.
2. **If only part of the work is finished:** copy the finished files into a git worktree of HEAD, build and test
   there, then commit only those files (see the `git worktree add` pattern in git history).
3. **Commit, push and watch CI** with `gh run list -R yekta/noshape`. Each push to main cancels the previous
   run. OCCT is cached per OS. A full CI run takes about 20 minutes, mostly Windows.
4. **Check Cargo.lock.** A lock change that belongs to an uncommitted manifest will break `--locked` in CI.
5. **Clean up:** delete finished agents' `/root/target-*` dirs and private copies (`/root/noshape-*`).

## Build and test environment

- **OCCT:** a static OCCT 8.0.1 is at `/root/occt`. Every build needs `OCCT_ROOT=/root/occt`. Each agent uses its own
  `CARGO_TARGET_DIR=/root/target-<name>`. The dev profile uses line-tables-only debug info to save
  disk.
- **Displays:** Xvfb displays `:101`–`:106` (1600x1000) are used by agents, one display per agent.
  - Run the app: `NOSHAPE_NO_NATIVE_DIALOGS=1 DISPLAY=:10x <target>/debug/noshape --demo /tmp/x.noshape`.
  - Other demo flags: `--demo-drawing-full`.
  - With no arguments the start window opens (set `HOME` to a scratch dir).
- **Screenshots:** `DISPLAY=:10x import -window root out.png`. xdotool and xclip are installed.
- **Pasting test content:** features can be pasted into a Part Studio from the clipboard as JSON
  `{"noshape_features":[...]}`. Generators: `crates/model/src/demo_fab.rs` and the `write_*_clip` ignored tests.
- **Never use broad `pkill -f` patterns.** They kill other agents' builds and even your own shell. Use
  `pkill -f "[t]arget-<yours>/debug/noshape"`, or kill by PID.
- **Disk:** about 150 GB total. Keep more than 15 GB free.
  - Delete `target-*/debug/incremental`.
  - Delete finished agents' target dirs.
  - Remove `/tmp` screenshot folders of finished work.

## Next steps, in order

1. Integrate everything once the agents finish, and enable strict CI.
2. Send the user screenshots of the Onshape-styled UI (Part Studio, dialog, sketch, assembly,
   drawing, start window) next to Onshape references.
3. Close the remaining parity gaps (all the `docs/parity/*.md` items) and start the drawing parity
   checklist.
4. Release v0.2.0.
5. Continue QA from a blank document across all workflows on all three platforms.
