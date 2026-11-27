# psui (Part Studio UI): current state
Updated 2026-11-27 08:5x UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

## Shard triage + focus rule + qa P2s (lead 11-27): LANDED 08:5x (queued), two items in flight
- Queued 08:5x: the focus rule, Hole table text, part menu, Boolean offset, Hole TAP_INCONSISTENT, Full round blame,
  Edit material, Where used, Insert image picker, spline points, sketch-region measure, Loft Thin regions + pick owner,
  tests, TICKETS (coverage-1 rows ~9284-9289, depth-3 rows 40-47 marked).
- IN STASH `psui-reach-0846` (`git stash list --format='%H %gs' | grep psui-reach`; apply by sha, then drop):
  1. Edge reach: over another part's face an edge reaches 5 px, not 6 (renderer.rs pick_reaching's `body`,
     builder.rs PickMap::body_fn, viewport.rs pick_body, behaviors.rs sets it; offscreen.rs callers). GPUI
     feel::an_edge_ending_in_another_part_does_not_reach_past_its_end passes with it (fails without); scene lib + render
     pick tests ok (ps7). LEAD: land only when the split-face-ok replay matches AND live answers REQUESTS "psui 11-27
     08:46" (edge reach over another part's face). Next: replay split-face-ok + split-part-front-face-ok with it.
  2. dialogs::a_part_face_swept_along_its_edge_adds_into_the_part (SWEEP-PART-FACE-PROFILE-ADD): passes with waits
     between profile and path (ps7); ps8 (08:4x) tries the path picked at once (the replay's pace). If ps8 passes, the
     app loses the run elsewhere: replay mistake-sweep-add-no-change with NOSHAPE_UI_SHOTS + a log line in
     std_edit_logic's result (changes dropped when params changed meanwhile?). App stderr has no logic lines.
- Still differing (ps5): fillet-variable-two-vertices-ok s7 (fields[8] kind live "number" / ours "other");
  sketch-image-place/edit s3 (live's Libraries tab: a search field; needs a live capture);
  mistake-split-trim-to-face-partial s7 (live error / ours ok).
- ps4 rescored on today's takes (vibe2 ~/tmp/psui/ps4-rescore.log): live's focus retakes disagree with the focus rule
  (a checkbox click focuses the checkbox in live): edit-curve-control-points-details s5, fillet-overflow-smooth-toggles
  s7, frame-corner-coped-limit s5, linear-pattern-centered-skip s12, loft-connections-isocurves s3;
  loft-surface-thin-tabs s3 (live none / ours field), mistake-hole-counterbore-diameter-5 s11 (live none),
  query-variable-created-owned-by s3, curve-pattern-feature-distance s3 (live null / ours the query). Next after the two
  items above.
- Full round's 3 parts: core's FULL-ROUND-SPLITS-PART (lead 08:4x). Left to skasm: spline points' constraints.
- core (ps3 triage): circular-pattern-face-boss-ok s11, circular-pattern-feature-boss-reapply-ok s14,
  mistake-linear-pattern-* titleState, gusset x2, mistake-rib-flipped-misses; sweep-thin trim-ends message.

## NEXT
- qa's diffs from live's option-combination batches as filed; interaction-feel rows to verify (archive 11-27).

## Agents

- skasm owns the hover matrix's sk/* cells (lead 11-22); psui the 3D / Part Studio cells.
- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
