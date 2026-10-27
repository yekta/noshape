# dx handoff: agent API + drawings + data exchange

Area (PLAN.md 2.3): crates/ops, crates/cli, docs/AGENT_API.md, crates/model/src/drawing, crates/app/src/drawing_*.rs,
import/export (kernel and model import/export files), configurations, Variable Studio, Derived, start window and
document visuals. Older notes: agent-api.md (api), ui2.md + theme-studios.md (ui2), drawing-behaviour.md (asb),
drawing-verify.md (drw2), qa5-routing.md (api-owned rows).

Setup: worktree /root/projects/noshape-wt/dx (branch agent/dx), env in /root/dx-scratch/env.sh (CARGO_TARGET_DIR
/root/target-dx, OPENNURBS_ROOT=/root/opennurbs so Rhino export is built), detached jobs with
/root/dx-scratch/bg.sh <log> "<cmd>". Live: `cd /root/live-pw && node dxr.mjs -e "…"` drives dx's own Chrome tab
(target id in /root/dx-scratch/live/.dx-tab).

## Landed (2026-10-26)
- (pending) ui2's WIP a6b4baa7: Tables › Title row text Bold (default on) / Italic / colour; thread preview hairlines;
  Construction geometry Extended lines row.
- (pending) API: variable_set takes `name` alone as the variable (what agents try first) and `new_name`; sketch_create
  `plane` takes `{"query": {...}}` (one planar face); config_activate says when the session is one CLI call (qa4
  CFG-MASS: mass properties do follow the active configuration within a session); feature_add / feature_edit resolve
  Derived's `partStudio` by tab name and refuse non-Part-Studio tabs; drawing_get lists silhouettes (qa4 DRW-SIL).
- (pending) kernel export test rhino_breps_are_valid: volume bound 1e-4 (Windows 4e-5 off; live's spline bands ±0.1 %).
- (pending) Hole Standard through the API (P1, wrong geometry): feature_add / feature_edit run the dialog's Standard
  logic (Units tab from the table given; the table's sizes into the fields not given). Before: an ISO M6 tapped hole
  kept the new hole's 1/4 in drill.
- (pending) IMP-DENS: "Import material density" from Onshape's own type list (any file of sldprt, sldasm, zip, psm,
  par, x_t, x_b, xmm_bin, xmm_txt, catpart, catproduct, prt, asm; not STEP), app dialog and import_files alike.
- (pending) Variable Studios through the API (P1 coverage; the app's code moved to ops so both share it):
  variable_insert_studio (whole / some variables / in a configuration / another document at a version),
  variable_remove_studio, variable_move_studio, variable_create_studio, variable_studio_settings (Insert into all);
  variable_list shows `studio_tables` and copies a studio's table; the config_* ops work on a Variable Studio `tab`
  (its Configuration panel: inputs, rows and the row menu, configured Value / Description cells, Set to #variable,
  Unconfigure, the shown configuration, paste inputs, internal IDs, visibility). Shared code: ops::vs_config,
  ops::variable_studio (references), app variable_config.rs / variable_insert.rs call it.
- (pending) history_version_properties (version / workspace Properties…) and history_publish (Publish FeatureScript…,
  checklist shared in ops::versions). Coverage: only Release (P3), the history filter "Any" and collapse / expand view
  state remain Planned.
- (pending) ANSI cosmetic thread end-on: four thin 60° arcs (30–90°, 120–180°, 210–270°, 300–360°) as live, not a
  dashed circle. Examples test now checks enum values (variable_create's example had kind INTEGER).

## In progress

## Next (in order)
1. API (P1): F19 width mate e2e once skb lands its fix; re-time F4 (Extrude / Hole add) after core's add-path work.
2. API benchmarks (P1, agent usability): sheet metal, Variable Studio, in-context; render_image of the window camera
   through MCP (image size).
3. Drawings (P2, ui2's list): re-screenshot Tables end vs live drawing/ui2-ah-tab-Tables-end.png and the thread
   tiles; flat pattern Counterbore / Countersink / Form styles drawn.
4. Drawing PDF size (680 KB for one A3 sheet): compare with live's export.

## Requests to the lead
- RHINO-GREEN: please build the watcher / green app with `OPENNURBS_ROOT=/root/opennurbs` (release.yml builds with it,
  dx's builds too). Hiding RHINO in builds without it needs a kernel build.rs cfg, i.e. a C++ rebuild for everyone; not
  worth it unless you want it.
- skasm (P1 sketch): the Point tool clicked on the Origin creates nothing ("the clicks didn't complete a point"), in the
  app and the API alike (sketch_mode.rs `point_for` reuses the origin point). Onshape users put a point there for a
  hole at the origin; live question filed (REQUESTS.md dx 2026-10-26).
