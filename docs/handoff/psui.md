# psui (Part Studio UI): current state

Updated 2026-11-20 19:00 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Mac off limits until 21:35 UTC 11-20 (lead; tunnel shut): build on vibex7/vibex8. Builds and crate tests: plain `NOSHAPE_RB_HOST=vibex8 tools/tiers/rb.sh bash -c "$(cat script)"` (DECISIONS 9e5b481d4;
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

## Now (18:55 UTC 11-20)

1. LANDED: 6f48184a (FS notices compile off the UI thread; A/B 6 max prepaint 534 -> 28 ms on 850efe, 549 -> 37 on
   de4eff), 18809b4c (spinner on the executor clock), 1c6e56e4 + 5c3f3bb7 (revert of a5505f50). Worktrees psui-fsn,
   psui-spin removed.
2. LANDED 87ffadda 17:15 (b3c497b9): a5505f50's parts again + alone-only hover-wait averaging + white/blue flange handle +
   Configurations box pane (FEEL-CONFIG-BOX-WIDTH). run156 green (clippy, part-studio-ui 354, app-core 115, offscreen).
   A/B 7/8 (vibex6, load 5-10): select the same at low load (5-7 ms both); under load both slip (main 64-171, re-land
   96-154), waiting for the first window present after the click. Not a re-land regression.
3. Heavy-doc hover. LANDED 51c01c9c 18:35 (0808250d): after 500 ms with no window frame the viewport took itself as
   off screen, so every hover after a rest redrew the whole window (present 100-180 ms on llvmpipe) before its frame;
   now vendor gpui `App::entity_drawn` (NOSHAPE PATCH) decides on screen. Test
   a_hover_after_a_rest_submits_its_frame_without_a_window_redraw asserts `Viewport::last_layout()` unchanged (GPUI's
   test mode draws dirty windows in flush_effects). Land check flaked once on clicks_in_3d_…ctrl_opens_the_menu and
   feature-studio a_double_click_…: 5/5 and 3/3 alone on vibex8 at load 13.
   A/B 11 (vibex6 load 8-11, same-tree baseline): hover medians de4eff 380/237 -> 283/253, impeller 207/180 -> 164/222,
   block 187/183 -> 82/232: mixed under load, clear only at low load.
   DONE (lead 18:5x: software-only frame cost is below the open feel rows): scratch bench /root/psui-scratch/pick_bench.rs
   (never land; run162.sh copies it in on the host), 151k tris + 17k segments 1600x1000 llvmpipe, load 11-13: frame
   71-106 ms with nothing skipped; skipping ao / silhouettes / backsil (alone or together) 63-142, in the noise; meshes
   skipped 28-38; pick 0.1-2 ms. The shaded mesh pass is the cost; no cheap pass win that also helps GPU frames.
   Tools vibex6 ~/agents/psui-keep: ab9/10/11.sh, hf3t.sh, hfeel-t.py, hovtrace.py, hovtrace2.py (+ DIAG lines from
   the trace patch /root/psui-scratch/diag2.py: NOSHAPE_NOTIFY_TRACE, never land); docs in ~/tmp/psui (850efe
   bb49db4ceba0762c, de4eff 874aef1a8b008b1b, impeller 844c39ce20497196, block cc4a172465de10c6).
4. NOW (lead 18:5x): every Part Studio uip scenario that still differs (feature dialogs, feature list, sketch chrome,
   view cube, filters), most-used first; report the count before / after. Sources: qa 14:47 run on 0a0b9a49
   (/root/qa-scratch/suites/1120-1447-0a0b9a49/uip.log; it has a5505f50's select regression, none of the fixes) and
   the night runs (1120-0407-81d23c9b, 1120-night-ui-a06ec927). The 33 Part Studio names: /root/psui-scratch/ps-names.txt;
   rerun on main via uip-some.sh on vibex8 -> ~/tmp/psui/ps1 (log /root/psui-scratch/ps1.log) = the "before":
   4/33 differ on eb8aae4fb (rollback-bar-drag, sketch-dimension-variable, sketch-entity-copy-paste,
   variable-type-angle); the 14:47 run's other 12 + 5 not run all match now. "After": rerun once this batch lands.
   - P0 (lead 20:0x): macOS CI failed a_cleared_hover_shows_the_frame_drawn_before_without_drawing (reused 0
     frames): T2'd from worktree /root/projects/noshape-wt/psui-p0 (branch agent/psui-p0), QUEUED 81b0fa91 20:11.
     Diagnose on the Mac after 21:35 (frame_key / in_flight / readback); remove psui-p0 once landed.
   - variable-type-angle: title "0°" vs live "0 deg": fixed (uncommitted) in feature_dialog `shown_length`
     (dialog_angle_text), test angle_text_tests::an_angle_variables_name_reads_deg (run163).
   - sketch-dimension-variable: live's take is broken (no dimension placed; "#width" typed into shortcuts): retake
     asked of live in REQUESTS 19:00.
   - FEEL-ROW-MENU-ROLL-TO-HERE: live answered 19:0x (live/dm/rollto-menus.json): Roll to here on every row but the
     last, after Show dependencies… = ours already (roll_items). New from it: "Unconfigure suppression" for a feature
     whose suppression is configured: added (uncommitted) in the row menu + coverage.rs line + list matrix row
     "menu / configured suppression" (run163). Open: "Section view…" on Extrude 1's row but not Extrude 2's: asked of
     live in REQUESTS 19:11.
   - Sheet metal titleState errors (bend-alignment-hold-line, hem-in-place-closed, bend/jog-new-edit-reedit,
     bend-angle-align-to-geometry) are skasm's features.
   - rollback-bar-drag: after a drop the bar's tip is off (FEEL-ROLLBACK-DROP-ROW), and with it its driver name
     "button:Rollback bar": now marked while the tip is off too (uncommitted, mod.rs render_rollback_bar).
   - sketch-entity-copy-paste: a menu paste no longer follows the pointer (Ctrl+V still does, R6); Transform keeps a
     picked entity clicked again (inferred from this take only). Test sketch_mode a_menu_paste_stays_... (run163).
   - qa 19:2x: FEEL-CONFIG-BOX-WIDTH reopened (5 px): cfg-box pr(5) (title split 10 px from the border, rows to 220);
     test the_configurations_box_stays_one_input_tall_with_two. FEEL-FLANGE-EDGE-BEHIND-WALL: a preview's picked edge
     lines drawn again (selected_only line instances, compare::POST; line shader bit 2); tests offscreen
     a_previews_picked_edge_stays_whole_over_the_mix + the linear-pattern preview test (run163).
4b. Flange handle vs live flange-handle-1120: the orange inside the heads is the selected edge seen through the hollow
   heads: nothing more to draw.
5. Cleanup done 18:56 (vibex6 A/B binaries and hf/, vibex8 target/release).
   Not to build: 3D Sketch, Fault analysis.

## Agents

- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
