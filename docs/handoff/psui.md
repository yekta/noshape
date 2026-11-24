# psui (Part Studio UI): current state
Updated 2026-11-24 11:5x UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

## RESUME HERE (11-24 13:4x UTC; brief /root/lead-scratch/prompt-psui-1124.txt; lead 11-24 13:1x)
- QUEUED 11-24: studios' stack 6aca9828 (landed), my stack edb7329e (checking 11:14), skasm's 30cba9c5; the feel batch
  (this handoff's commit). Once landed: `git worktree remove` psui-skrebase / psui-strebase, rm ~/agents/psui-*rebase
  on vibex7/8.
- Feel batch (landing): FEEL-MC-LOOK (MC_RIM_PX 1, MC_Z_AXIS_PX 23), ERR-MATE-HOVER-GLYPH, FEEL-SKETCH-INF-END-DOT,
  ERR-CHROME-TIP-PLACEMENT (title tip -2), ERR-LIST-TOOLTIP-CHROME (tips > 386 px wrap), SKT-USE-HOVER (edge most across
  the screen, 5 px band + core). mc_hovered_other_selected: a lone blended pixel matched #ffa033 within 24; now measured
  as the silhouette test does (tol 12, >= 2 px). The wrap test needed the front layer (mated_blocks_fixed_on).
- Windows CI hardening (same batch): a_hover_after_a_rest_...: the viewport's layout_count (last_layout was also
  stamped by present-only frames, and the frozen clock hid real layouts); the pick wait delivers picks only
  (Gpu::deliver_picks), since GPUI's test mode draws any dirty window after each update and an earlier frame's present
  redrew it (traced). The triad test takes the ball's spot before the release (drag_with). a_cleared_hover_...: fails
  on Windows only (after != before with reuse > 0): message now says what differs (frame_diff); read the next Windows
  run. the_pointer_lights_the_line_...: no Windows log of it yet (release/v0.4.68 run 45375844269 still running 13:3x).
- FEEL-EDIT-BODY-SHADE region (live 2nd capture psui/edit-body-shade-1124): test held in
  /root/psui-scratch/region-test.rs (paste before "A selected edge of a selected face" in hover_matrix_tests.rs). Ours:
  the front matches; the right face is (3,7,10) darker missed and through the region. The preview light fits (k 1.41 vs
  1.435), so look at the model-before's 0.3 behind the right face (default planes' extent? ours vs live's doc).
- qa's newest hover-matrix score: /root/qa-scratch/hm2/score-a910/sc8.txt (11-24 10:37, green a9107b3a9); ascii maps
  with score-a910/bin/ascii.py; inf.py for the end dot. ps-arc/linemid-hovered and b1-mid-hovered are single-jump cells
  (not scored); b4-as-*-selhover blue = asm's FEEL-ASM-MOVE-RING-RIMS.
- Next: ERR-SM-BAD-SELECTION-RED (repro in a gpui test), ERR-MISSING-SELECTION (core's flag; core parked),
  PARTLY rows in TICKETS (CORNER-BREAK-VERTEX-PICK, TRANSFORM-*, PS-BOX-DRAG-HOVER), P2 queue (PS-PARTS-ROW-TOOLTIP-PITCH,
  CUSTOM-FEATURE-ROW-ICON).
- USER P0 Mac mini: APPEARANCE-PART-UNDER-FACE-COLOURS once dx lands; user's doc only as ~/tmp/psui/macmini-m6.noshape
  (700), delete it and /root/psui-scratch/Mac-mini-2024.step after.

## Agents

- skasm owns the hover matrix's sk/* cells (lead 11-22); psui the 3D / Part Studio cells.
- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
