# psui (Part Studio UI): current state
Updated 2026-11-22 21:41 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Builds and crate tests on vibe2 or vibex8 (lead 11-22: no waiting on vibe2's one slot): `NOSHAPE_RB_HOST=… tools/tiers/rb.sh bash -c "$(cat script)"`,
  CARGO_INCREMENTAL=0; `git diff --name-only origin/main | xargs touch -c` first. ~/bin/slot
  only for suites, gates, timing pairs (DECISIONS 9e5b481d4). Scripts in /root/psui-scratch/run*.sh. The GPUI dialog matrix is
  `dialogs::the_dialogs_matrix_matches_live` (rows filter: DIALOG_ROW=…).
- Scenario / app runs: vibex6, at most 3 app instances (shared with studios). Heavy runs (scenario shards) through
  `~/bin/slot <cmd>` there (one per host, waits for 10 GB free). The app build there OOMs under load: scripts build
  with `-j 4`.
- Land only with `tools/tiers/land.sh` (`--status` for results). Start an rb.sh run, wait until it builds (its sync
  done), then land.sh: land.sh's rebase during an rb.sh sync breaks that build.
- Live: Chrome on :107, CDP 127.0.0.1:9333, helpers /root/live-pw; captures are asked of live2 in
  onshape-research/live/REQUESTS.md (dated `date -u`). Recorded behaviour: onshape-research/live/behaviours.md.
- Private documents: work on a mode-700 copy in ~/tmp, delete it after.
- GPUI tests: platform keys as "secondary-…"; marks (`ui_script::mark`) only record while a scenario runs, so tests
  find controls by `debug_selector`.
- psui owns docs/parity/interaction-feel.md (the feel matrices; all matched as of 11-15): keep it current.
- A new menu label needs its line in crates/ops/src/coverage.rs (`cargo test -p noshape-ops --test all coverage::`).

## Tools (/root/psui-scratch)

- `uip-some.sh <out> <names…>`: release build on the rb host, then the named ui-parity scenarios on one Xvfb (:392).
  Run: `NOSHAPE_RB_HOST=vibex6 tools/tiers/rb.sh bash -c "$(cat /root/psui-scratch/uip-some.sh)" uip-some <out> <names…>`
  (`~` in remote paths expands locally: pass /home/ngyekta/… paths). Output buffered: the log fills at the end.
- `psui-shards.sh <out> <names file>`: the same in 3 shards, then one `--compare-only` score. The names file must be on
  the host (scp it first). psui-names.txt: the 1021 psui-area scenarios with live takes.
- Compare without rerunning: `python3 tools/ui-parity/run.py --compare-only --out <dir> <names…>` (through rb.sh).
- Runner env: NOSHAPE_UIP_LOGS=1 (app stderr), NOSHAPE_UI_SHOTS=<dir> (a screenshot per step), NOSHAPE_PERF=1,
  NOSHAPE_CACHE_DIR (fresh: cold runs).

## Now (21:41 UTC 11-22)
Batch 1 LANDED 613b06bb (19:59). TICKETS ticked in the tree (uncommitted, goes with batch 2): EXTRUDE-ARROW-ANCHOR,
PS-LONGPRESS-SELECTS, QUERY-PART-PICK-FILL, PICK-SKETCH-LINE-THROUGH-FACE, PLANE-LIT-AFTER-MENU, SECTION-PLANE-LABEL-LOOK.
Lead told the hover matrix score (ours 94/94; qa's last 100/125, ask qa to rescore after batch 2).

Batch 2: applied uncommitted in the tree (backup /root/psui-scratch/batch2.patch; full test script run-t60.sh:
clippy, features_a, app-core, scene, render, part-studio-ui). Covers qa P0s:
- nav-anim-1122: view changes are time-based at any fps (`view_change_duration` removed); nav matrix row "a view change is time-based".
- NAV-N-CENTRE: `Viewport::animate_to_direction_centred`; N row checks (20,15).
- PLANE-LABEL-NORMAL-VIEW: plane names 20 px on screen (client PLANE_TEXT_HEIGHT_PX). `Instance::pixel_sized`; render
  `upload_instances(scene, camera, h)` scales by `pixel_world`; builder PLANE_NAME_*. Test hover_matrix
  `a_plane_name_keeps_its_screen_size_at_any_zoom`; the section-plane-names test locates the label by px.
- MANIP-DRAG-DECIMALS (angles 7 decimals) + FILLET-DRAG-FOCUS (qa new P0: Radius focused during and after the drag).
- TRIAD-RELEASE-FOCUS, TRANSFORM-PLANE-HANDLE (qa new P0), TRANSFORM-TRIAD-LOOK:
  - Transform triad uses the client constants: shaft 60, head 30/r10, planes 20-45 px. Routing's full triad unchanged.
  - Second plane diagonals are hit segments (appended last). No focus on release. Moved part opaque in its own colour.
  - Tests: dialogs_feel `the_translate_triads_xy_square_drags_x_and_y_and_keeps_the_focus`; sketch_ui transform ghost test.
- MANIP-RELEASED-LOOK: `end_manip_drag` sets manip_hover + refresh. Dialogs row "an arrow drag lights nothing" checks it.
- TRANSFORM-CHIP-RED: Rotate's axis errors faulty ["axis"] (model-core pattern.rs). features_a transform test.
- ROLLBACK-CLAMP-HOVER: default rows take RollbackDrag moves (`ui.rollback_drop_dg`). List row "held over Top".
- interaction-feel.md rows for all of the above; this handoff.
t60 (vibex8): clippy clean, features_a 130, app-core 116, scene 212+6, render 17+68; part-studio-ui 490/494.
Live answered 19:58 (box-hover-1122, multi-part-menu-1122, sel-see-through-1122); added on top, uncommitted:
- PS-BOX-DRAG-HOVER: `Viewport::box_plane` (the plane hovered at the press stays lit through the box, out at release; new
  `PickClass::Plane`; box_ids never takes planes). Test feel `a_box_from_a_plane_keeps_that_plane_lit_to_the_release`: PASSES.
- PS-MULTI-PART-MENU: menus.rs several-parts branch (live's 26 rows less Add comment); coverage "partStudio:Check
  interference…". Test sketch_ui `two_parts_faces_give_the_two_parts_menu`: PASSES.
- PS-SELECTED-SEE-THROUGH: builder `selected_edge`: part edge lines drawn again `selected_only().on_top()`. Test hover_matrix
  `a_hidden_selected_faces_outline_shows_through_the_part`: PASSES. List × viewport matrix (p04) passes again (t62).
- interaction-feel.md rows for the three.
Fixed after t63: plane names weren't drawn because frustum culling tested the unscaled (pixel-unit) box; pixel-sized
instances now skip the cull (renderer.rs). Plane-name test passes (t64). Triad XY-square test passes (Isometric for the
drag). EDIT-BODY-SHADE: test dropped (ours top (118,127,133), live (132,147,155); preview mix 1 gives 97, 0 gives 166:
the gap is not the mix alone); ticket stays open. Probes removed. coverage "partStudio:Check interference…" added.
t65 (vibex8): clippy clean (+ops), features_a 130, app-core 116, scene 212, render 17+68, part-studio-ui 497/497, ops
coverage 11. Scene edge_looks failed: the see-through copy doubled a seen selected edge's band; the copy is now
`hidden_only` (drawn only where hidden). t66: scene 212+6, hover matrix 96/96 + list 4/4. Committed and landing batch 2.
Next: land result → tick PS-BOX-DRAG-HOVER, PS-MULTI-PART-MENU, PS-SELECTED-SEE-THROUGH + batch 2's tickets → hover
matrix score to lead (96/96), qa rescore.

Open qa P0s (TICKETS "psui, 2026-11-22 …"):
- TRANSFORM-ROTATE-EDGE-SENSE: the sense matches live (qa amended); only the preview look is left (batch 2's opaque moved
  part should cover it; the orange axis edge to check).
- CTX-MENU-FLIP-UP (P1): the vertical flip already exists (anchored SwitchAnchorOrSnap, nav-feel-1119). Ask qa to re-check.
skasm owns FEEL-MC-* Part Studio set (10) + SKETCH-DIM-PLACE-PICKS-PLANE (11-22 18:0x); mate connector inference code is
skasm's. Told them batch 1 carries the Shift lock, ConnectorLook::Placed and the with-layer hover mask.
Then: assembly column of sel-gestures (tell asm first); autocomplete list position. Lead rule: end turns at ~180k context.

## Agents

- skasm owns the hover matrix's sk/* cells (lead 11-22); psui the 3D / Part Studio cells.
- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
