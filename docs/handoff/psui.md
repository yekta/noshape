# psui (Part Studio UI): current state

Updated 2026-11-16 04:44 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Builds and crate tests: the Mac slot only, `NOSHAPE_RB_HOST=mac tools/tiers/rb.sh cargo …` (one slot, rb.sh waits).
  Never cargo on this machine. Test the crate you changed (`--lib <module>::` or `--test all <file>::`).
- Scenario / app runs: vibe6, at most 3 app instances (shared with studios). Heavy runs (scenario shards) through
  `~/bin/slot <cmd>` there (one per host, waits for 10 GB free). The app build there OOMs under load: scripts build
  with `-j 4`.
- Land only with `tools/tiers/land.sh` (`--status` for results). Start an rb.sh run, wait until it builds (its sync
  done), then land.sh: land.sh's rebase during an rb.sh sync breaks that build.
- Live: Chrome on :107, CDP 127.0.0.1:9333, helpers /root/live-pw; captures are asked of live2 in
  onshape-research/live/REQUESTS.md (dated `date -u`). Recorded behaviour: onshape-research/live/behaviours.md.
- Private documents: work on a mode-700 copy in ~/tmp, delete it after.
- GPUI tests: platform keys as "secondary-…"; marks (`ui_script::mark`) only record while a scenario runs, so tests
  find controls by `debug_selector`.
- Xvfb app runs: `xdotool windowfocus <w>` after start (no WM: no focus, no blur commits).
- UI tests: `NOSHAPE_CONFIG_DIR=$(mktemp -d)` keeps them off the shared ~/.config/noshape.
- psui owns docs/parity/interaction-feel.md (the feel matrices; all matched as of 11-15): keep it current.
- A new menu label needs its line in crates/ops/src/coverage.rs (`cargo test -p noshape-ops --test all coverage::`).

## Tools (/root/psui-scratch)

- `uip-some.sh <out> <names…>`: release build on the rb host, then the named ui-parity scenarios on one Xvfb (:392).
  Run: `NOSHAPE_RB_HOST=vibe6 tools/tiers/rb.sh bash -c "$(cat /root/psui-scratch/uip-some.sh)" uip-some <out> <names…>`
  (`~` in remote paths expands locally: pass /home/ngyekta/… paths). Output buffered: the log fills at the end.
- `psui-shards.sh <out> <names file>`: the same in 3 shards, then one `--compare-only` score. The names file must be on
  the host (scp it first). psui-names.txt: the 1021 psui-area scenarios with live takes.
- Compare without rerunning: `python3 tools/ui-parity/run.py --compare-only --out <dir> <names…>` (through rb.sh).
- Runner env: NOSHAPE_UIP_LOGS=1 (app stderr), NOSHAPE_UI_SHOTS=<dir> (a screenshot per step), NOSHAPE_PERF=1,
  NOSHAPE_CACHE_DIR (fresh: cold runs).
- sbs.py: side-by-side crops (ours offset (242, 90)); mirror-sxs/ holds the Mirror side by sides with README.
- vibe6 can't reach vibe5: `scp -3`. vibe ~/tmp is pruned after 6-12 h: keep results in ~/agents/psui-keep.

## In flight

- Landed 11-16: c525abd8 (flat bend dash and hover, pattern labels), f2029e2a (the sketch 2D manipulator as live
  editing-gaps3: `transform_anchor`, `PatternHandle::MoveX` / `MoveY`, `XF_*`; `sketch_transform`'s default pivot),
  876f6e61 (snaps: spline rows, 2 gaps: live reaches 8–12 px across a spline, its shape unrecorded).
- Queued 04:25 (dd13425c): model edges wake after a 60 ms rest and give PERPENDICULAR through a line's start (live
  sketch-gaps 1c; `woken_edge`, `WOKEN_EDGE` sentinel retargeted by `link_woken_edge` when the view uses the edge;
  `model_lines` filled in `model_snap`); snaps matrix 63 / 65 with a triangle prism bench (`triangle_part`,
  `Pt::SlantMid` / `SlantPerp`, `Lit::Slant`). A woken model edge's parallel isn't measured live: not offered.
- Queued with this handoff: a failed feature's parts of the result (core's `FeatureState::error_parts`, 601df0ac)
  drawn as live feedback-gaps 5: their shading under rgb(189,48,57) at 25 %, edges 5 px (`error_geometry(…,
  ghost: false)`, `ERROR_PART_ALPHA`); vibe6 shot /root/psui-scratch/fb5/sf2/08.png: top (162,156,176) = live,
  front (124,107,120) vs (125,107,120), edges 5 px.
- Open feel gaps: the tooltip fade-out (vendored gpui's tooltip lifecycle; gpui's tooltip tests expect an instant
  hide; left), the view cube knob size vs live's 3 px hover map, the flat bend click colours (zone rgb(214,151,30),
  outline rgb(255,169,10); needs a flat-view click target in the runner), an open circular pattern's angle label
  (live has none), where live's triad angle / factor boxes sit.
- end-cap-profile-circle step 3: matches on a quiet host; under load our Faces field keeps focus a moment. Low.
- tab-new-edit-reedit: decision asked of the lead (11-15 23:50): the click lands ~1.1 px from Flange 1's wall;
  known (sub-pixel camera) or retarget onto the wall live hit.
- Mac target ~20 GB: prune-target.py, and drop debug/incremental dirs idle > 90 min.
- Scratch: /root/psui-scratch/fb5/run.sh runs one one-off scenario (--scenarios ~/tmp/psui/fb5s --all, shots per
  step); scenario files there: shell-fail.json, sm-flat.json, triad.json.

## Queued to land

- dd13425c (model edge waking); the error-parts highlight with this handoff.

## Waiting on others

- skasm: bend-relief-* "Vertex of Flange 1" (label after the feature that first created the definition entity: theirs,
  in progress). tab-new-edit-reedit is a pick: live's ray passes ~0.3 mm above Flange 1's top edge and hovers its outer
  wall; ours the plate's top. Mine: queue after the P1.
- skasm: gusset-rectangle-aligned step 7, end-cap-profile-internal step 5 (their validation); sketch circular pattern
  Equal (sketch-circular-pattern-click-commit constraints[2]).
- live2: analysis-thickness ×3 (our list lacks the Pro rows?), context-create-version-update retake on main.
- dx: face-rclick-export-dxf. core: boundary-surface (with my 11620bb3).

## Known, left as they are

- Sketch readouts of free clicks beyond ±1 px (DECISIONS 11-15): sketch-linear-pattern-click-commit (1.8 px),
  sketch-key-symmetric (1.7), sketch-key-concentric-two-circles (2.0), sketch-key-midpoint (1.0, live's own point
  1.67 px off in y). Reported to the lead.
- sketch-mirror-preselected-batch: live's lost first click after a box select is a live defect (`live_defects` from
  step 20, DECISIONS 11-15); the rest matches.

## Next, in order

1. The flat bend click colours (a flat-view click target in the runner first); the tab pick per the lead's answer;
   live's triad angle / factor boxes (ask live where they sit).
2. Rerun after the others' fixes land: boundary-surface-new-edit-reedit (core 658bef9d), the gussets and end-caps
   (skasm), sketch pattern takes (skasm 58a0bd16 + linear), context-create-version-update (live2's retake).
3. Older open items (11-06 to 11-08 "Next, in order" in the archive): doc-tab-rename-escape (5 / 5) and
   sketch-constraint-refusals (91 / 91) match on 11-16 03:10; check the rest against a rerun before picking any up.

## Open decisions

- None pending. (11-15: ±1 px on free-click readouts, granted; Mirror lost click as live defect, granted.)
