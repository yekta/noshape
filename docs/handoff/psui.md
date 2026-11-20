# psui (Part Studio UI): current state

Updated 2026-11-20 02:34 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Builds and crate tests: plain `NOSHAPE_RB_HOST=vibe2 tools/tiers/rb.sh bash -c "$(cat script)"` (DECISIONS 9e5b481d4;
  ~/bin/slot only for suites, gates, timing pairs). Scripts in /root/psui-scratch/run*.sh. The GPUI dialog matrix is
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
- sbs.py: side-by-side crops (ours offset (242, 90)); mirror-sxs/ holds the Mirror side by sides with README.
- vibe ~/tmp is pruned after 6-12 h: keep results in ~/agents/psui-keep.

## Now (updated 02:34 UTC 11-20; this thread replaced 4973e295 at 13:27 11-19, Ubeyd's weekly limit)

1. LANDED: 00389dbd as ffd85455 (00:50, STALE-SKETCH-HOVER; qa verified (a)(b)(c) on green a933e1b7).
2. IN THE LAND QUEUE: 7c2d7313 (re-queued 01:22 after a handoff conflict): ERROR-CHIP-RED-NO-FIELD (FeatureError /
   FeatureState `invalid_input`, Thicken's both-zero; core told), the 712 px Configurations panel test, the
   dialog-query-rclick-clear-all seed. On green tell qa ERROR-CHIP-RED-NO-FIELD is fixed.
3. NEXT TO LAND (after 7c2d7313): qa P1 EDGE-MIDPOINT-REST-LOOK (TICKETS 5296e9f9c; behaviors `midpoint_rest_edge`:
   rested on the edge, the edge stays lit and the midpoint is a 6 px #ffd3a0 fill in a 7 px dark ring; test
   a_rested_edge_midpoint_keeps_the_edge_lit_with_a_filled_ring; tell qa on green, they re-walk it). Feel-row tests:
   the_working_spinner_shows_900_ms_into_a_regeneration_centred_low (pending.rs "pending-spinner"; the slow case
   drives begin_pending / end_pending: a held Regenerator stalls GPUI's single-threaded test executor),
   switching_configurations_greys_nothing_and_spins_only_past_900_ms, parts_list_properties_put_the_state_pill_on_each_part
   (mod.rs "part-state-{id}"), dialog_field_lists_variables_by_their_word_prefixes (Tab closes the list).
   run100 (vibex8): perspective, mirror, linear pattern, editing matrix pass; fixed since and in run102 (vibe2):
   the skip ring (focus "Instances to skip" after the first draw), the Hole tabs (its value rows have no label: read
   in order), the Hole depth (only "no regeneration" checked), Isolate 198 → 196 px (app-core isolate_ui: the last row
   18 px, no bottom margin, live dom dialog-assembly-isolate; the assembly's too). New in run102:
   dialogs_feel_tests::the_query_variable_dialog_shows_no_count (row 1331), feel_tests::
   a_lit_face_goes_dark_when_the_pointer_leaves_the_window (qa HOVER-LEAVES-WINDOW; GPU, skips without one).
   On landing also tick in TICKETS (fixed earlier, never ticked): TRANSFORM-GHOST-LOOK (row 1148), SECTION-PREVIEW-
   COLOUR (1162), HOLE-CALLOUT-PLACE (1464), FLFEEL-BAR-MENU (list matrix "rollback bar / its menu while rolled back");
   FEEL-FAST-OFF-CLEAR: the clear is on the second fast move, the rest 0-40 ms frame latency, under DECISIONS'
   100 ms bar (11-18): ask qa to close. vibex8 is qa's until 04:00 UTC 11-20 (nightly suite): no builds there.
   Restored: perspective_and_render_modes_switch_in_one_step (Window::dispatch_action is deferred:
   read the state in the next update), a_skip_ring_lights_and_a_click_skips_its_instance_then_keeps_it ("Instances to
   skip" clicked first: an edit dialog opens on Entities). New P0 rows: dialogs_feel_tests
   a_mirror_across_a_side_face_adds_into_its_seed, a_linear_patterns_instance_previews_once_its_direction_is_picked,
   the_hole_tabs_bring_their_rows_and_values, a_typed_hole_depth_waits_for_enter; editing matrix "Shift+Y on a hidden
   part's selected row", "empty-space menu with a part hidden"; list matrix "menu / one sketch"; dialogs matrix row
   click + Delete / Backspace, row hover tooltip text; Isolate's place (behaviors "ps-isolate" selector); the configure
   toast; the section value box + typing (run101). Rows 1208 / 1210 / 1211 now cite existing app-core unit tests.
   Skipped: 1199 (device px needs an app-core refactor), 1221 (no live inch readouts), 835, the speed rows.
4. fillet-chip-clear-missing / fillet-edit-cancel-keeps retaken by live (e4ff71a8): both match on green 08525ecd
   (20/20 states, vibex6 02:4x). Waiting on others: ver-compare-hover-part (history-ui) with the lead.
5. Interaction-feel rows that match live with no test (grep "missing" in the Test column; P0 sections first).
   Skipped: the two-block box counts (live's camera: iso + 10 wheel steps about an unknown pointer), the rollback
   bar's hover colour (GPUI group_hover, no test hook); [skasm]-tagged rows; qa's speed-suite rows.
   Asked of live2: the diagonal submenu move's timing (REQUESTS 16:37).
6. Not to build: 3D Sketch, Fault analysis. Cleanup: vibex7 ~/tmp/psui/priv/dock.noshape when the tab work is over;
   vibe2 ~/tmp/psui/hv binaries. Branch psui-backup-1119 (all landed; delete).

## Agents

- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
