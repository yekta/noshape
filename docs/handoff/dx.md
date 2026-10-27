# dx handoff: agent API + drawings + data exchange

Area (PLAN.md 2.3): crates/ops, crates/cli, docs/AGENT_API.md, crates/model/src/drawing, crates/app/src/drawing_*.rs,
import/export (kernel and model import/export files), configurations, Variable Studio, Derived, start window and
document visuals. Older notes: agent-api.md (api), ui2.md + theme-studios.md (ui2), drawing-behaviour.md (asb),
drawing-verify.md (drw2), qa5-routing.md (api-owned rows).

Setup: worktree /root/projects/noshape-wt/dx (branch agent/dx), env in /root/dx-scratch/env.sh (CARGO_TARGET_DIR
/root/target-dx, OPENNURBS_ROOT=/root/opennurbs so Rhino export is built), detached jobs with
/root/dx-scratch/bg.sh <log> "<cmd>". Live: `cd /root/live-pw && node dxr.mjs -e "…"` drives dx's own Chrome tab
(target id in /root/dx-scratch/live/.dx-tab).

## Build times (load ~25–30)
- 10-26 after c9540ae6: first build ~25 min; edit ops/src/values.rs → `cargo test -p noshape-ops --test all
  e2e::hole_standard` 74 s.
- 10-27 after the model split: first build after the rebase ~8 min (+ the kernel's C++ once for ccache); edit
  model-core regen + ops session → `cargo test -p noshape-model-core --lib regen::crash_guard::` then `-p noshape-ops
  --lib session::`: ~6 min for both (model-core, ops rebuild under load); ops `e2e::` run 11 s.

## Landed on main (2026-10-26/27)
- aa563bcd ui2's DL12 piece; c63d2bf5 drawings / data exchange (ANSI thread arcs, PDF FlateDecode, material density
  types, silhouettes in drawing_get, rhino bound); 5ef64eb8 + 829a8627 agent API batch 1 (Variable Studios, version
  properties / publish, Hole Standard, sketch_create plane query, variable_set name / new_name, Derived by tab name…);
  f0da1ca3 batch 2 (singular list args, legacy hidden params refused, VS configuration rules as live, one VERSION).
- b29afb6f fixed my own mistake: 36e4da66 had pushed TICKETS.md mid-rebase with conflict markers (qa's tickets restored).
- (this commit) P0 DOC-POISON: crash markers around each feature evaluation (model-core `regen::crash_guard`, enabled by
  `ops::session::Session::open/create`); Session::open rolls a Part Studio back above a feature whose evaluation
  stopped a process ("Roll back above <name>: it stopped noshape while regenerating"), notes in `Session::recovered`
  and document_open / document_info `recovered`. Checked on qa's cli-fillet-segv.noshape: open 1 aborts in the kernel
  (Shell 4), opens 2 and 3 work. The app needs nothing new (it opens through Session::open); a toast with the note
  would be nicer (app side, after the app split).

## Next (in order, after landing)
1. Apply the two pending patches; test (model drawing:: export tests, ops e2e drawing tests).
2. qa tickets: OPEN-ERROR-SILENT (start window: show why a file didn't open), DRAWING-SCALE second half (auto centre
   marks on full circles only, not partial arcs).
3. In-context: context_update of a Part Studio created in context (no primary) can't refresh, app and API alike
   (ops::in_context::refreshed); live question filed (REQUESTS dx "P2 Modeling in context").
4. API: F19 width mate e2e (skasm landed the fix in cdde1c06); re-time F4; benchmarks sheet metal (in-context done:
   /root/dx-scratch/bench/incontext.py; Variable Studio done: led to the Variable Studio ops).
5. Drawings (P2): PDF text as filled outlines (Normal) and a subset NotoSans (Selectable), 1/600 in units; ui2's
   Tables-end and thread tiles re-screenshot vs live; flat pattern Counterbore / Countersink / Form styles.
6. Re-verify old rows on the green build: workflow-configurations.md C1–C8, workflow-derived.md DER2–DER14 (DER8 works
   through the API; check the app's Insert panel), workflow-drawing.md D8/D14/D16.

## Requests to the lead
- RHINO-GREEN: please build the watcher / green app with `OPENNURBS_ROOT=/root/opennurbs` (release.yml builds with it,
  dx's builds too). Hiding RHINO in builds without it needs a kernel build.rs cfg, i.e. a C++ rebuild for everyone; not
  worth it unless you want it.
