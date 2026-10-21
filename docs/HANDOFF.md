# noshape: handoff for the next lead

Read this first (new lead: start with `docs/handoff/lead.md`), then `docs/handoff/*.md` (one file per workstream, written by the agents),
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

- **Onshape account (provided by the user):** the credentials are in the environment variables
  `ONSHAPE_EMAIL` and `ONSHAPE_PASSWORD`, set in `/root/.config/noshape/onshape.env` (mode 600) and
  sourced from `~/.bashrc` and `~/.profile`. Never commit or print the values. Use them with Chrome
  (`/usr/bin/google-chrome`, Node 24, Playwright in `/tmp/pw`) to study and screenshot the live Onshape app
  and to verify behaviours. Don't create other accounts.
- **No copied Onshape assets.** Don't put Onshape's own SVG or image assets in the public repo. The icons in
  `crates/app/assets/icons/` are redrawn on Onshape's 20-unit grid. Reference material lives outside the repo in
  `/root/onshape-research/` (help pages, 5,148 help images in `ui-ref/`, the public CSS tokens, and the
  icon sprite rendered for visual reference).
- **Licensing is undecided.** `Cargo.toml` says Apache-2.0 but there is no LICENSE file, and OCCT is LGPL-2.1
  with an exception, statically linked. This is the user's decision; we flagged it.

## State of `main` (last commit: see `git log`)

- **Features:** full CAD feature set, parity with Onshape's FeatureScript feature list (every
  feature has an evaluator), sheet metal, frames, custom features (FeatureScript Feature Studios), assemblies, drawings,
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

**In progress** (started 2026-10-18 by the second lead; rules for all agents: `docs/handoff/TEAM-RULES.md`).
Agents run on different accounts through Motile (`list_accounts` / `start_agent`); reach them with `message_agent`.

| Agent | Motile id | Account / model | Display, target | Handoff |
|---|---|---|---|---|
| ui: Part Studio chrome, dialogs, viewport | 407f0ff2-4b45-4c0c-8a62-11a33f3e05b8 | Motile / Opus 5.5 (moved 10-21 19:30 from Ngyekta 3f25493a) | :101, target-shared | `docs/handoff/theme-layout.md` |
| ui2: sketch/assembly/drawing/start-window/document visuals | e197d9fc-d255-4e6d-8004-e99ab4a60c25 | Stablecog / Opus 5.5 (moved 06:55 from Unbind db9e1762) | :105, target-shared | `docs/handoff/theme-studios.md` |
| nav: view navigation (FINISHED; only real-hardware touch checks remain, see docs/handoff/navigation.md) | 98da8819-1a7a-4be9-a814-e167275fcc08 | Motile / Opus 5.5 | :109, target-shared | `docs/handoff/navigation.md`, `docs/parity/navigation.md` |
| icons: redrew 21 icons copied from Onshape + added no_onshape_assets guard test (FINISHED) | ddea0979-7067-4399-9ade-2a18dbe32300 | Yekta / Opus 5.5 | - | report to lead |
| qa4: QA round 4 done (see qa-round-4.md); now Part Studio keystroke latency profiling + fix | c4f2a534-3006-49f6-a930-7bd2102a858c | Motile / Opus 5.5 (moved 10-21 19:30 from Ngyekta fee4041d) | :110, target-shared | `docs/handoff/qa-round-4.md` |
| drw2: verifies/finishes the 🔧 rows of docs/parity/drawing.md | 2a0c08de-2087-4fea-8be7-2b0dd812e3dc | Ngyekta / Opus 5.5 (moved 21:20 from Motile 0885a73c) | :111, target-behave | `docs/handoff/drawing-verify.md` |
| live: answers `live/REQUESTS.md` | 6ea36fb4-0912-49b5-b7ba-350760f91b49 | Ngyekta / Opus 5.5 | Chrome on :107, CDP 9333 | `live/behaviours.md`, `live/REQUESTS.md` |
| psb: Part Studio behaviour (+ release workflow, materials) | 4a7b49af-e9f1-42af-8052-e63ca4303709 | Yekta / Opus 5.5 (moved 10-21 19:00 from Motile 64e2935c) | :102, target-behave | `docs/handoff/part-studio-behaviour.md` |
| skb: assembly behaviour (+ sketch leftovers) | 870c0d67-ce63-4414-beb3-ecd4d7077f2d | Stablecog / Opus 5.5 (moved 06:55 from Unbind 9d432d37) | :103, target-behave | `docs/handoff/assembly-behaviour.md`, `sketch-behaviour.md` |
| asb: drawing behaviour | 4ade3a23-24dd-4151-a02a-8a43b8e83456 | Yekta / Opus 5.5 (moved 06:55 from Unbind 60ec2ad1) | :104, target-behave | `docs/handoff/drawing-behaviour.md` |
| qa3: user-level QA after the restyle (finished; findings forwarded) | 212b596b-0639-475a-87aa-543ce8f4d12d | Unbind / Opus 5.5 | - | `docs/handoff/qa-round-3.md` |
| kern: kernel robustness + import/export | 1a2e857a-2972-4d11-a1c4-34b29cebd3c6 | Unbind / Opus 5.5 (moved 10-21 19:00 from Motile 3eeb6d7c) | :106, target-engine | `docs/handoff/kernel-robustness.md` |
| api: agent API — CLI + MCP server (all app features usable by agents) | f3af292a-68f0-44ec-ba3f-74af834c9dac | Stablecog / Opus 5.5 (moved 10-21 19:00 from Motile 795820ea) | target-engine | `docs/handoff/agent-api.md` |
| icons2: icon parity, every icon matching Onshape at 1×/2× as our own redrawn artwork | ecf0a581-d0a5-4e7b-8338-02a760b07175 | Unbind / Opus 5.5 (restarted 10-21 19:58: 084c1884 hung on a pkill approval prompt) | :115, target-shared | `docs/handoff/icons2.md` |
| qa5: workflow QA vs live Onshape (sheet metal, assembly, drawing, import, surfaces, configurations), no building | 7f57fb0b-3f98-4ab1-81ed-006728e3b180 | Yekta / Opus 5.5 (moved 10-21 19:00 from Motile f741a4b8) | :120, copies target-shared binary | `docs/handoff/qa5.md` |
| perf: viewport performance (60/120 fps at Retina; zero-copy presentation) | 1e01a3a3-b95e-4ce6-8772-6efc3767fc9f | Unbind / Opus 5.5 (moved 14:55 from Yekta ee9f7d90) | :113, target-engine | `docs/handoff/viewport-performance.md` |
| fs: FeatureScript language + std library (resumed 08:20; first: green tests + model clippy) | 1d3b2881-712d-4762-9dd5-6b315fae3774 | Motile / Opus 5.5 (moved 10-21 19:30 from Ngyekta 80461700) | :108, target-engine | `docs/handoff/featurescript.md` |

Use Opus 5.5 for all agents (user, 2026-10-18; not Fable). Unbind and Codex hit session limits on 2026-10-18; their agents were moved.
Motile bug: an agent in "waiting_for_usage_limit" ignores stop_agent; send it a message (that wakes it), then stop_agent while it shows "running".
Unbind is signed in again (2026-10-21).
Accounts (10-21): Ngyekta is the lead's Max account, weekly 73% used at 10-21 19:00 (resets 10-26 04:00) — keep agents off it except live; Team accounts (Yekta, Unbind, Stablecog, Motile) have 5-hour session windows, ~3 active agents each.
Integration: `/tmp/lead/integrate.sh "msg"` snapshots the tree, builds/tests/clippies it in /root/noshape-lead and commits that exact tree (the shared tree rarely compiles at any instant).

Not started yet:
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
