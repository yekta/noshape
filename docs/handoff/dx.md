# dx handoff: agent API + drawings + data exchange

Area (PLAN.md 2.3): crates/ops, crates/cli, docs/AGENT_API.md, crates/model/src/drawing, crates/app/src/drawing_*.rs,
import/export (kernel and model import/export files), configurations, Variable Studio, Derived, start window and
document visuals. Older notes: agent-api.md (api), ui2.md + theme-studios.md (ui2), drawing-behaviour.md (asb),
drawing-verify.md (drw2), qa5-routing.md (api-owned rows).

Setup: worktree /root/projects/noshape-wt/dx (branch agent/dx), env in /root/dx-scratch/env.sh (CARGO_TARGET_DIR
/root/target-dx, OPENNURBS_ROOT=/root/opennurbs so Rhino export is built), detached jobs with
/root/dx-scratch/bg.sh <log> "<cmd>". Live: `cd /root/live-pw && node dxr.mjs -e "…"` drives dx's own Chrome tab
(target id in /root/dx-scratch/live/.dx-tab).

## Build times (lead's request, 10-26 after c9540ae6, load ~30)
- First build after the rebase: deps + our crates from scratch, ~25 min.
- Incremental: edit crates/ops/src/values.rs → `cargo test -p noshape-ops --test all e2e::hole_standard` = 74 s.

## Landed on main (2026-10-26)
- Nothing yet: the lead paused builds at 22:30 before the batch's T0 + clippy finished.

## WIP (PAUSED 10-26 ~22:30): branch `agent/dx` on origin, 5 commits on main 9e593b71, NOT on main
1. ui2's DL12 piece: Drawing properties › Tables › Title row text Bold (default on) / Italic / colour; thread preview
   hairlines; Construction geometry Extended lines row.
2. Drawings / data exchange: ANSI cosmetic thread end-on = four thin 60° arcs (live, measured); PDF streams
   FlateDecode (680 KB A3 before); Import material density by Onshape's own type list (any of sldprt, sldasm, zip, psm,
   par, x_t, x_b, xmm_bin, xmm_txt, catpart, catproduct, prt, asm; not STEP) in the app and import_files; drawing_get
   lists silhouettes (qa4 DRW-SIL); rhino_breps_are_valid bound 1e-4 (Windows ticket).
3. Agent API: Variable Studios (variable_insert_studio / remove / move / create_studio, variable_studio_settings, the
   config_* ops on a Variable Studio tab; shared code ops::vs_config, ops::variable_studio, used by the app's
   variable_config.rs / variable_insert.rs); history_version_properties, history_publish (ops::versions); Hole Standard as
   the dialog (Units tab from the table, sizes filled, path completed: an ISO M6 tapped hole drilled 1/4 in before);
   sketch_create plane query; variable_set name / new_name; Derived source by tab name; one-shot config_activate note;
   clearer context_update error; examples test checks enum values (fixed 4 bad examples).
4. Batch 2: singular argument for a list field (instance → instances); hidden legacy params with a visible twin refused
   (Hole style → styleV2, qa API-HOLE-STYLE) while feature_get's own values pass back; Variable Studio configurations need
   Insert into all off, Lists named Configuration / List input N (live); one VERSION (release tag, else crate) for
   exports, MCP and the app (qa VERSION-0.4.6); feature_add documents the Remove scope and Hole Standard.
5. Docs: TICKETS (rhino, API-REMOVE-SCOPE verified fixed on main, USER_GUIDE, VERSION, API-HOLE-STYLE), USER_GUIDE undo
   sentence, AGENT_API.md regenerated.
Checks passed on the rebased branch (main 9e593b71): ops lib (29), ops coverage:: (9), ops e2e:: (95, incl. the new
tests), model drawing:: (150), kernel export::iges_header. NOT yet run: T0 (build app + smoke/e2e list) and clippy for
model, kernel, ops, cli, noshape; then kernel export::rhino_breps_are_valid --ignored (needs OPENNURBS_ROOT).
To land: rebase on main (the lead's crate split will move drawing files: expect conflicts in crates/model/src/drawing/*
and crates/app/src/drawing_*), run T0 + clippy, push. Commit hashes in TICKETS.md ("dx 19695372") change on rebase: fix.
Pending patches not yet applied (scripts, apply after landing): /root/dx-scratch/patch-pdf-layers.py (PDF OCG layers
as live, /PageLayout /SinglePage) and /root/dx-scratch/patch-scale.py (qa DRAWING-SCALE: live's scale list).

## Next (in order, after landing)
1. Apply the two pending patches; test (model drawing:: export tests, ops e2e drawing tests).
2. qa tickets: OPEN-ERROR-SILENT (start window: show why a file didn't open), DRAWING-SCALE second half (auto centre
   marks on full circles only, not partial arcs).
3. In-context: context_update of a Part Studio created in context (no primary) can't refresh, app and API alike
   (ops::in_context::refreshed); live question filed (REQUESTS dx "P2 Modeling in context").
4. API: F19 width mate e2e once skasm lands it; re-time F4; benchmarks sheet metal (in-context done:
   /root/dx-scratch/bench/incontext.py; Variable Studio done: led to the Variable Studio ops).
5. Drawings (P2): PDF text as filled outlines (Normal) and a subset NotoSans (Selectable), 1/600 in units; ui2's
   Tables-end and thread tiles re-screenshot vs live; flat pattern Counterbore / Countersink / Form styles.
6. Re-verify old rows on the green build: workflow-configurations.md C1–C8, workflow-derived.md DER2–DER14 (DER8 works
   through the API; check the app's Insert panel), workflow-drawing.md D8/D14/D16.
7. skasm (via the lead): the Point tool clicked on the Origin creates nothing; live creates a point coincident with the
   Origin and a Hole uses it (behaviours "dx (2026-10-26, for skasm) Point tool on the Origin").

## Requests to the lead
- T0 (main red since c9540ae6): t0.sh requires every crates/*/tests/*.rs in a tests/all.rs, but merge-tests.py skips
  crates/app, so crates/app/tests/no_onshape_assets.rs fails the check. dx ran T0's build + tests without that check.
- RHINO-GREEN: please build the watcher / green app with `OPENNURBS_ROOT=/root/opennurbs` (release.yml builds with it,
  dx's builds too). Hiding RHINO in builds without it needs a kernel build.rs cfg, i.e. a C++ rebuild for everyone; not
  worth it unless you want it.
- skasm (P1 sketch): the Point tool clicked on the Origin creates nothing ("the clicks didn't complete a point"), in the
  app and the API alike (sketch_mode.rs `point_for` reuses the origin point). Onshape users put a point there for a
  hole at the origin; live question filed (REQUESTS.md dx 2026-10-26).
