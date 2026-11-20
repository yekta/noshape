# psui (Part Studio UI): current state

Updated 2026-11-19 22:06 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

## Now (updated 23:35 UTC 11-19; this thread replaced 4973e295 at 13:27, Ubeyd's weekly limit)

1. LANDED 22:04 (01969838): qa's P0 feel rows, untested-row tests, Routing curve Multiple with its value box.
2. QUEUED 22:48 b5fce1fd (6 commits): Mass CoM / inertia Override; TOOLBAR-TOOLTIP-AFTER-CLICK; Routing Multiple
   centre ball; sketch-vertical-key fix (BAND_HOLD_PX 8); row tests. On green: tell qa (TOOLTIP + vertical-key re-run).
3. STALE-SKETCH-HOVER fix (to land after run88: part-studio lib + app-core lib + clippy on vibex7):
   leave_sketch drops self.hover / hover_pick / hover_raw / sketch_gpu_hover (the disc is drawn at hover_pick.anchor,
   so a cancelled sketch's end stayed drawn: (c), and after ✓ (a)); viewport `hover_id_stale` (set by set_scene on a
   geometry change) makes set_hover re-emit an unchanged id; CameraChanged re-resolves a point's reach (mod.rs, sketch.rs);
   outside sketch mode no reach lights while the pointer is out of the graphics (shown_sketch_hover_id).
   Tests (GPU, sketch_hover_feel_tests): a_closed_sketchs_lit_end_goes_with_it (fails without the fix),
   a_shown_circles_centre_and_region_go_dark_with_the_move_away (view-key part fails without the CameraChanged re-check),
   a_lines_lit_end_goes_dark_with_one_move_away. Ball test centre now exact (µm-rounded, 1e-12).
   Real-app repro kit: /root/psui-scratch/stale/ (stale-app.sh + scenarios; run on vibex6 with ~/green/noshape):
   on green 2bcd6dd7a (b) in sketch mode did NOT reproduce (one xdotool move clears the end) -> qa to re-run on green.
4. P2 ERROR-CHIP-RED-NO-FIELD fixed (lead 11-20 ok'd the flag): FeatureError::invalid_input / FeatureState.invalid_input
   (model-core; core told), set at Thicken's both-zero; FeatureDialog.invalid_input skips entry_failing and the red
   inputs. Feedback matrix row "error / inputs refused". Other validation errors can take it the same way when live
   shows their inputs dark.
4c. live retakes eef639d7 (camera recorded, 11-20): check ours against the changed takes: face-menu-select-create-selection
   (face menu adds "Where used for Part 1…"), ver-compare-hover-part (version row menu without "Releases"),
   fillet-chip-clear-missing / row 1738 ("Unknown of": likely the start without q2224/fix.js; live asked 00:3x to re-run
   with it; ours keeps "Missing Edge"), face menu "Where used" already ours (ctx-menus-1117), dialog-query-rclick-clear-all (new Fillet at the account's last radius: scenario now seeds
   remembered 2 mm). ver-compare-hover-part (history-ui) flagged to the lead. 12 others unchanged apart from the camera.
5. Interaction-feel rows that match live with no test (grep "missing" in the Test column; P0 sections first).
   Skipped: the two-block box counts (live's camera: iso + 10 wheel steps about an unknown pointer), the rollback
   bar's hover colour (GPUI group_hover, no test hook), the spinner's 900 ms (`Instant`, not the test clock).
   Asked of live2: the diagonal submenu move's timing (REQUESTS 16:37).
6. Not to build: 3D Sketch, Fault analysis. Cleanup: vibex7 ~/tmp/psui/priv/dock.noshape when the tab work is over;
   vibe2 ~/tmp/psui/hv binaries. Branch psui-backup-1119 (all landed; delete).

## Agents

- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key (lead after compaction: "match live's default view orientation, re-baseline"): checked 21:1x:
  ours already opens in live's start view, Camera::default = Trimetric (-0.433, 0.75, -0.5) = live's recorded start
  camera direction (every live take with a camera); live's Isometric menu item is the true (-1, 1, -1)/√3, as ours
  (view-cube-iso). The (64, -45) vs (61, -42) gap is the zoom of the camera-less 11-06 take (~0.2885 mm/px implied;
  live's empty start is 0.2754 in 17 of 25 takes, 0.2507 in today's). live2 retook it with its camera (cf70e676):
  live shows no Vertical at 0.2507. NEXT: once cf70e676 lands, run sketch-vertical-key on ours (uip-shots.sh); if ours
  infers Vertical on live's camera, the gap is the reach measure (screen distance to the projected plumb line) — with
  skasm. No orientation change, no re-baseline unless that run shows one.

## Open decisions

- None.
