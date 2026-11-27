# psui (Part Studio UI): current state
Updated 2026-11-27 11:2x UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Builds, crate tests and scenario runs on vibe2, one rb.sh at a time: `NOSHAPE_RB_HOST=vibe2 tools/tiers/rb.sh bash -c "$(cat script)"`
  (the script exports OCCT_ROOT=/opt/occt CARGO_PROFILE_DEV_DEBUG=line-tables-only TMPDIR=$HOME/tmp/psui); `git diff --name-only origin/main | xargs touch -c` first. ~/bin/slot
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
  Run: `NOSHAPE_RB_HOST=vibex7 tools/tiers/rb.sh bash -c "$(cat /root/psui-scratch/uip-some.sh)" uip-some <out> <names…>`
  (`~` in remote paths expands locally: pass /home/ngyekta/… paths). Output buffered: the log fills at the end.
- `psui-shards.sh <out> <names file>`: the same in 3 shards, then one `--compare-only` score. The names file must be on
  the host (scp it first). psui-names.txt: the 1021 psui-area scenarios with live takes.
- Compare without rerunning: `python3 tools/ui-parity/run.py --compare-only --out <dir> <names…>` (through rb.sh).
- Runner env: NOSHAPE_UIP_LOGS=1 (app stderr), NOSHAPE_UI_SHOTS=<dir> (a screenshot per step), NOSHAPE_PERF=1,
  NOSHAPE_CACHE_DIR (fresh: cold runs).

## IN FLIGHT 11:2x (after compaction: start here)
- LAND 538d28d1 (08:5x) FAILED: model-data hole_tables tests still expected the old text; fixed e4cab5e73 (0.2010 in,
  5.000 mm, 1/4 in; model-data 125 ok on vibex8). Re-land with everything below (tree must be clean: stash the reach
  change + trace with `git stash push -u -m psui-reach`, land, apply by sha, drop).
- COMMITTED, to land: 8894046f3 Windows click fix (LEAD 09:1x: v0.4.73 Windows CI lost a rim-test click: GPUI
  simulate_click runs background work between down and up, the viewport times the press on the real clock and drops a
  press >= 520 ms as a long press; Pointer::click now sends down+up in one update; ps14: part-studio-ui 661 ok);
  38da04e1a ASM-DIALOG-FOCUS (ps16: Limits x5, mc rotate, pattern directions match; assembly-ui 233 ok) +
  TICKETS ASM-MATE-ROTATION-SIGN (skasm: our 45 deg mate rotation is live's transpose). Tell qa the subjects of
  486ff6e1e ("assembly dialogs name their Limits fields, …") and 38da04e1a after the land; asm-mate-connector-realign
  needs a live retake (old take: Realign keeps the keyboard; realign-edge: the axis box takes it).
- LEAD 11:1x (core uipdep3): Sweep's Thin tab keeps the keyboard on "Faces and sketch regions to sweep": 788387b0e
  (in the failed land) already does it; test the_sweep_thin_tab_keeps_the_keyboard_on_its_profiles_field (ps17 ok),
  committed; in this land.
- Edge reach (PICK-FACE-OVER-NEAR-EDGE), held, uncommitted: renderer.rs pick_reaching `body`, builder.rs body_fn,
  viewport.rs pick_body, behaviors.rs, offscreen.rs, feel_tests.rs (2 edge tests, both pass in GPUI). The app replay
  (same 0.275 mm/px camera as live) still takes Part 1's edge at split-face-ok s7. TEMP trace (never commit):
  renderer.rs NOSHAPE_PICK_TRACE eprintln in pick_reaching. ps18 (resp/ps18.log) = replicate + traced split-face-ok.
  LEAD: land only when the replay matches AND live answers REQUESTS "psui 11-27 08:46". Backup resp/reach-staged.patch.
- live 11-2x: "Release all configurations…" takes queued (6362d352, start config-ps); "Disable imported appearances"
  is the Appearances panel row's item (needs recorder work: not asked for).
- Still differing (ps5): fillet-variable-two-vertices-ok s7 (fields[8] kind number/other); sketch-image-place/edit s3
  (Libraries search field: needs a live capture); mistake-split-trim-to-face-partial s7 (live error / ours ok).
- ps4 rescored: live's focus retakes disagree with the focus rule (a checkbox click focuses the checkbox in live):
  edit-curve-control-points-details s5, fillet-overflow-smooth-toggles s7, frame-corner-coped-limit s5,
  linear-pattern-centered-skip s12, loft-connections-isocurves s3; loft-surface-thin-tabs s3, mistake-hole-counterbore-
  diameter-5 s11, query-variable-created-owned-by s3, curve-pattern-feature-distance s3.

## NEXT
- qa's diffs from live's option-combination batches as filed; interaction-feel rows to verify (archive 11-27).

## Agents

- skasm owns the hover matrix's sk/* cells (lead 11-22); psui the 3D / Part Studio cells.
- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
