# Working on noshape (guide for coding agents)

Read `docs/ARCHITECTURE.md` first, then the parts of these that matter for your task:
`docs/kernel-api.md` (OCCT kernel wrapper), `docs/gpui-kit-cheatsheet.md` (UI toolkit),
and the Onshape research specs in `research/sections/` (what each feature must do).

## Build and test
* OCCT is prebuilt at `/root/occt`. Every build that touches the kernel needs `OCCT_ROOT=/root/occt`.
* Use the target directory your task assigns (e.g. `CARGO_TARGET_DIR=/root/target-<name>`);
  several agents build in parallel, and the machine has 8 cores and limited disk.
* Model-only work: `OCCT_ROOT=/root/occt CARGO_TARGET_DIR=... cargo test -p noshape-model`.
* App work: `OCCT_ROOT=/root/occt CARGO_TARGET_DIR=... cargo build -p noshape`.
* Run the app headless for screenshots: an X server is running on `DISPLAY=:99` (Xvfb, 1600x1000).
  `DISPLAY=:99 timeout 25 <target>/debug/noshape --demo /tmp/<you>/x.noshape &`, wait ~15 s,
  then `DISPLAY=:99 import -window root /tmp/<you>/shot.png` and look at the PNG with the Read tool.
  `xdotool` is available to click/type (`DISPLAY=:99 xdotool mousemove X Y click 1`, `key shift+e`).
  Kill your instance afterwards (`pkill -f <your path>`). Use your own document path.
* Do not commit or push; the lead integrates. Never edit files outside your assigned area
  unless your task says so; if you need a change elsewhere, describe it in your final message.

## Conventions
* Units: SI internally (meters, radians). User-facing text uses `noshape_units::format_quantity`.
* Features: schema in `crates/model/src/features/*.rs` (parameter ids follow FeatureScript names),
  evaluator in `crates/model/src/regen/features/*.rs`, dispatched in `regen/features/mod.rs::evaluator`.
  Look at `extrude.rs`, `detail.rs` and `basic.rs` for the evaluator pattern: resolve references
  via `EvalCtx` (`resolve_faces/edges/bodies/plane/axis/direction/vertex_point/region`, `profiles`),
  call kernel ops, name result faces with `name_result`/`sweep_names` (persistent naming!), and
  merge with `apply_operation` (New/Add/Remove/Intersect) or `replace_body` / `boolean_into`.
  Return `err("message")` for user-facing failures; never panic.
  Get points/planes/axes/directions/connectors through the `resolve_*` helpers (not raw lookups):
  feature patterns re-run evaluators with an instance transform (`EvalCtx::xform`) that those
  helpers apply. Query parameters may contain `Reference::QueryVariable` (expanded before evaluation).
* Persistent naming (`crates/model/src/naming.rs`, `reference.rs`): every face a feature creates
  must get a deterministic name from (feature id, role, source); edges/vertices derive from faces.
  Choose roles so that names stay stable when unrelated upstream geometry changes.
* UI: GPUI via `gpui-kit`; colors from `crates/app/src/theme.rs`; icons are SVGs in
  `crates/app/assets/icons/` referenced with `assets::icon_path(name)`.
* Performance matters: no blocking work on the UI thread (use `cx.background_spawn`), cache
  derived data, avoid per-frame allocation.

## Live Onshape reference
A real Onshape account is available through the environment variables `ONSHAPE_EMAIL` and `ONSHAPE_PASSWORD`
(sourced from `/root/.config/noshape/onshape.env`). Use it with headless Chrome (`/usr/bin/google-chrome`) to look
at and screenshot the real app, and to check how features behave. Never commit, log or print the values.
Keep screenshots outside the repo (`/root/onshape-research/live/`).
