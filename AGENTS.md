## What is this?

noshape is a local clone of Onshape's CAD side, written in Rust with GPUI: it must look and behave exactly like
Onshape (no deviations; only cloud features such as sign-in, sharing, comments, notifications, and simulation/render
are left out), and be faster than Onshape wherever the work is local. It runs on macOS, Linux and Windows, fully
offline. Part Studios, sketches, assemblies, drawings, Variable and Feature Studios (FeatureScript), versions and
branches, import/export. This is the monorepo.

## Repo Structure

A Rust workspace of layered crates (docs/CRATES.md has sizes, rules and measured build times; docs/APP-ARCHITECTURE.md
the app's target shape). A crate never depends on a crate above it.

### Engine
- `crates/units`, `crates/sketch` (the sketch solver), `crates/featurescript` (the FeatureScript language).
- `crates/brep`: noshape's own B-rep in pure Rust (no OCCT types): topology arenas and a Boolean for solids bounded
  by planes, cylinders and cones (and a line's or faces' common), which `op_boolean`, Shell and Thicken try before
  OCCT (docs/BOOLEAN-SPEEDUP.md phase 3, docs/handoff/bool.md); psui's display mesher (`brep::mesh`).
- `crates/kernel`: the OpenCascade (OCCT) bridge: C++ in `crates/kernel/cpp`, patched OCCT at `/root/occt`
  (patches in `patches/occt/`). CI compiles the C++ with GCC 13 and MSVC: direct-initialise OCCT handle conversions,
  no GCC-only headers, guard platform code with `_WIN32` / `_MSC_VER`.

### Model
- `crates/model-data`: the document data model (documents, tabs, feature lists, schemas, params, references,
  configurations, the data of assemblies, drawings, Feature Studios). Serde types, no evaluation.
- `crates/model-core`: regeneration, features, sheet metal, in-context, analysis.
- `crates/custom`: the FeatureScript host (Feature Studios, custom features, std).
- `crates/assembly`: assembly evaluation, the mate solver, BOM, standard content.
- `crates/drawing`: drawing views, sheets, tables, PDF/DXF/DWG export.
- `crates/model`: a facade re-exporting all of the above under their old paths; the model integration tests.

### Scene, rendering, persistence
- `crates/scene` (scene building, sketch mode visuals), `crates/render` (the wgpu renderer), `crates/store`.

### Agent API
- `crates/ops` (operations), `crates/cli` (CLI + MCP server).

### App (GPUI)
- `crates/app-core`: theme, assets, base widgets, menus, the 3D viewport and navigation, shared dialogs. Never
  references a studio.
- One crate per studio / area: `part-studio-ui`, `assembly-ui`, `drawing-ui`, `variable-studio-ui`,
  `feature-studio-ui`, `history-ui`, `release-ui`, `start-ui`, `preferences-ui`.
- `crates/app`: the binary (`noshape`): `main.rs`, the workspace, wiring; registers each crate's hooks at startup.

## Agent API (CLI + MCP)

Everything a user can do in the app must be doable through `crates/ops` (the CLI and the MCP server use the same
registry), with the same options and results as the app's dialogs.
- A new feature or command isn't done until its op exists, with an e2e test (`crates/ops/tests/e2e.rs`).
- Every menu item must be listed in `crates/ops/src/coverage.rs`; `cargo test -p noshape-ops --test all coverage::`
  fails otherwise.
- `docs/AGENT_API.md` is generated: `UPDATE_DOCS=1 cargo test -p noshape-ops --test all coverage::`. Never edit it by
  hand.
- Errors name the parameter at fault and say what is wrong, in Onshape's words.

## General Rules

- Do not start editing code, committing or changing anything in response to a question. Answer the question. We'll
  tell you when to change something.
- Onshape parity is the requirement: check how live Onshape looks and behaves before you implement or change
  something, and match it exactly. Don't propose deviations.
- Speed is a feature: no blocking work on the UI thread (`cx.background_spawn`), cache derived data, no per-frame
  allocation; the targets are in docs/PLAN.md 2.7. A speed regression is a P0.
- Keep it simple. Follow the crate layering (docs/CRATES.md): data down, logic up; logic methods on data types are
  extension traits (`noshape_model::prelude`); upward calls are hooks or actions, never a dependency on a higher crate.
- Do not leave paragraphs of comments on top of the code. Prefer clear names; keep necessary comments short; comments
  move with the code they describe.
- Use guard statements (early returns) rather than deep nesting. Return user-facing errors (`err("…")`); never panic
  on user input.
- Tests must be meaningful and follow docs/TESTS.md: T1 tests are deterministic and platform-independent (geometry
  compared within live's tolerance band, never against a kernel's last digits; unique scratch files; GPU only under
  the GPU lock). Slow or fragile tests are T2 (`#[ignore = "T2: <reason>"]`).
- Zero compiler and clippy warnings. Never hide one with `#[allow]`.
- Never run `cargo fmt`/rustfmt on the tree: it isn't rustfmt-formatted; match the surrounding style by hand.
- Do not edit generated code: `docs/AGENT_API.md`, the workspace-hack crate (`cargo hakari generate`), `Cargo.lock` by
  hand.
- No Onshape assets or brand marks in the repo: icons are our own drawings on Onshape's 20-unit grid.
- Do not insert yourself into our code, commits or PRs in any way: no AI attribution lines.
- If we are missing a glaring issue when we ask you to do something, say so.
- Reinvent the wheel but not the car: no library for a simple problem; a well-maintained library for a complex,
  common one.
- Before starting work, check for uncommitted or unpushed local changes; if there are none, pull the latest main.

## Commit Messages

Commit as `git -c user.name=yekta -c user.email=ngyekta@gmail.com commit`. The subject starts with the agent or area
that did the work, followed by a short plain-language explanation of what changed:

    core: Shell keeps the outer walls of a filleted plate with holes
    psui: hovered edges drawn as live (no dark border)
    lead: CI runs every 3 hours on last-green

Describe the work in the body only when needed, concisely, in simple language.

## Build, test, land

- Env for every build: `OCCT_ROOT=/root/occt CARGO_TARGET_DIR=/root/target-<yours> CC="ccache gcc-13"
  CXX="ccache g++-13"`. Use your own target dir; never share one with another agent.
- Incremental compilation is on (machine config); mold links. Test the crate you changed:
  `cargo test -p <crate> --test all <file>::<name>` or `--lib <module>::`. Build the app only to look at it.
- Tiers (docs/TESTS.md): T0 `tools/tiers/t0.sh`, T1 `tools/tiers/t1.sh`, T2 `tools/tiers/t2.sh`.
- Land only with `tools/tiers/land.sh`: it rebases, runs clippy and the full tests of every crate your change touches
  plus every crate that depends on them (T0 when the app, ops or cli is affected), and pushes only if all of it is
  green. Docs-only changes push without a build.
- A new `crates/*/tests/*.rs` file: run `python3 tools/tiers/merge-tests.py .` (each crate's tests are one binary).
- Main is tested by the lead's watcher on every commit (T0 + T1 + clippy); `cat /tmp/lead/main-status` after
  pushing. If your commit turns main red, fixing it comes first.

## Conventions

- Units: SI internally (meters, radians); user-facing text through `noshape_units::format_quantity`.
- Features: schema in `crates/model-data/src/features/*.rs` (parameter ids follow FeatureScript names), evaluator in
  `crates/model-core/src/regen/features/*.rs`. Resolve references through `EvalCtx`'s `resolve_*` helpers, name result
  faces deterministically (`name_result` / `sweep_names`: persistent naming, `naming.rs`, `reference.rs`), merge with
  `apply_operation`.
- UI: GPUI via `gpui-kit`; colors from `crates/app-core/src/theme.rs`; icons are SVGs in `crates/app/assets/icons/`.

## Running the app

Each agent has its own Xvfb display (see its prompt; e.g. `:101`):
`NOSHAPE_NO_NATIVE_DIALOGS=1 DISPLAY=:101 timeout 300 $CARGO_TARGET_DIR/debug/noshape <file.noshape>`; screenshot with
`DISPLAY=:101 import -window root out.png`; input with `xdotool`. Kill your instance by its PID when done. The newest
green build is `/root/green/noshape`. Keep scratch files and screenshots in `/root/<agent>-scratch/`, outside the repo.

## Live Onshape reference

A real Onshape account is in `ONSHAPE_EMAIL` / `ONSHAPE_PASSWORD` (from `/root/.config/noshape/onshape.env`; never
print or commit them). Chrome runs on `DISPLAY=:107` with CDP on `127.0.0.1:9333`; helpers in `/root/live-pw/`; read
`/root/onshape-research/live/HANDOFF-live.md` first and use your own tab. Recorded behaviour and timings:
`/root/onshape-research/live/behaviours.md` (index: `SPECS.md`); ask for a capture in `live/REQUESTS.md`.

## Docs to read

docs/PLAN.md (how we work, speed targets), docs/PRIORITIES.md (P0–P3), docs/TESTS.md, docs/CRATES.md,
docs/APP-ARCHITECTURE.md, docs/TICKETS.md, docs/handoff/<area>.md, and the checklists in docs/parity/.
