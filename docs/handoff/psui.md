# psui (Part Studio UI): current state
Updated 2026-11-22 18:55 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

## Now (18:5x UTC 11-22)
P0 lit-end landed 93b1dfdd8 (cherry-picked to release/v0.4.67); psui-p0b worktree + vibex8 dir removed.

Batch 1 = commit 7af801429 (amended; rebased on main 17:5x). Rebase conflicts resolved: renderer band palettes use main's
4-colour `[border, instance, face, picked]`, with our body_outline_rgb as slot 0; builder sketch curves use main's lit/unlit
split plus our pick_plane. t58 (vibex8): part-studio-ui 491/492 (p13 of the list × viewport matrix failed: main's
fs_body_face took every BAND|SELECTED face as an instance's selected face, so it drew no silhouette), app-core 117, scene 212+6,
render 17+68, clippy (+assembly-ui) clean. Fix amended: shaders.wgsl fs_body_face needs INSTANCE|PICKED.
t59a green: part-studio-ui hover_matrix 94/94, assembly-ui hover 23/23, render 17+68.
NEXT:
- If t59a is green: commit this handoff into batch 1 or stash it (land.sh needs a clean tree), then land.sh.
- After the land, tick in TICKETS: PART-PICK-FILL, SECTION-PLANE-LABEL-LOOK, PICK-SKETCH-LINE-THROUGH-FACE,
  PLANE-LIT-AFTER-MENU, PS-LONGPRESS-SELECTS. Report the hover matrix score (qa 100/125) + list × viewport cells p01-p31.
- Then `git apply /root/psui-scratch/batch2.patch` (it holds no shaders.wgsl change).

Batch 2 (NOT built; patch /root/psui-scratch/batch2.patch, 1094 lines). Covers qa P0s:
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
After applying: touch -c, build + test part-studio-ui, app-core, scene, render, `noshape-model --test all features_a::`, clippy
(add assembly-ui) → commit "psui: …" → land.

Open qa P0s (TICKETS "psui, 2026-11-22 …"):
- Waiting on live (REQUESTS 18:15-18:3x):
  - PS-BOX-DRAG-HOVER (box-hover-1122; live2 11-17 vs 1122 conflict, code reverted).
  - PS-MULTI-PART-MENU (multi-part-menu-1122: the full 26 items).
  - PS-SELECTED-SEE-THROUGH (sel-see-through-1122: which rule).
- EXTRUDE-ARROW-ANCHOR: the anchor is the centroid in code (the entity is "Face of Sketch 1" → face_anchor). Run qa's cell
  /root/qa-scratch/feel1122/manipnav/cells/m01-ex-hover.json to see the 13/17 px offset.
- TRANSFORM-ROTATE-EDGE-SENSE: the sense matches live (qa amended); only the preview look is left (batch 2's opaque moved
  part should cover it; the orange axis edge to check).
- EDIT-BODY-SHADE (low conf).
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
