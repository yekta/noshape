# psui (Part Studio UI): current state

Updated 2026-11-16 03:56 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

- Landed 11-16 03:41 (c525abd8): flat view bend centre lines dashed 10 / 10 / 5 / 10 px (`LineSegment::CENTER_LINE`
  = 512), the flat bend hover (`FlatViewEvent::BendHover`, `ui.sm_flat_hover`; sheet metal matrix 12 / 12), sketch
  pattern labels as live sketch-gaps2 / 3 (`pattern_label_wpp`: px at the tool's start zoom, then model-fixed).
- Queued 03:55 (03b36ca6): the sketch 2D manipulator as live editing-gaps3 (`transform_anchor`: the lowest end or
  centre; `PatternHandle::MoveX` / `MoveY`; `XF_*` px constants; checked in a vibe6 shot); `sketch_transform`'s pivot
  defaults to the same point (AGENT_API.md regenerated); snaps matrix 60 / 62 (spline rows: across the through-point
  and the curve live reaches 8–12 px, its spline's shape unrecorded: gaps, not chased).
- Failed preview (live feedback-gaps 5): with core's ce0e6ae8 ours is the whole part red, in the ghost style
  (`scene_builder::error_geometry`: flat rgb(123,30,36) at 60 %, edges 3.5 px). Live (fb5-failed3.png vs our
  /root/psui-scratch/fb5/sf/08.png): the part's own shading + rgb(189,48,57) at α 0.25 (solved per channel from the
  top and front faces), edges 5 px. Asked core 04:10 for FeatureState to list which error shapes are result
  bodies (error_parts beside error_mesh); then: those parts tinted 25 % + 5 px red edges, the rest stay ghosts.
- routing-curve-new-edit-reedit: 19 / 19 (11-16 03:30). doc-tab-rename-escape, sketch-constraint-refusals match.
- Open feel gaps: the tooltip fade-out (needs vendored gpui's tooltip lifecycle: a timed WaitingForHide with an
  opacity the tooltip view reads; gpui's tooltip tests expect an instant hide; left), the view cube knob size vs
  live's 3 px hover map, a woken model edge's perpendicular (live sketch-gaps 1c: needs a slanted model edge in the
  snaps bench), the flat bend click colours (zone rgb(214,151,30), outline rgb(255,169,10): ours the selection
  colour), an open circular pattern's angle label (live has none), live's angle / factor boxes' place on the triad.
- end-cap-profile-circle step 3: matches on a quiet host; under load our Faces field keeps focus a moment. Low.
- tab-new-edit-reedit: decision asked of the lead (11-15 23:50): the click lands ~1.1 px from Flange 1's wall;
  known (sub-pixel camera) or retarget onto the wall live hit.
- Mac target ~20 GB: prune-target.py, and drop debug/incremental dirs idle > 90 min (it was 11 GB).

## Queued to land

- 03b36ca6 (2D manipulator, spline snap rows). This handoff goes with the failed-preview result.

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

1. The failed preview's highlight once core lists the error parts; the flat bend click colours (needs a flat-view
   click target in the runner); a slanted model edge in the snaps
   bench for the woken edge's perpendicular; the tab pick per the lead's answer.
2. Rerun after the others' fixes land: boundary-surface-new-edit-reedit (core 658bef9d), the gussets and end-caps
   (skasm), sketch pattern takes (skasm 58a0bd16 + linear), context-create-version-update (live2's retake).
3. Older open items (11-06 to 11-08 "Next, in order" in the archive): doc-tab-rename-escape (5 / 5) and
   sketch-constraint-refusals (91 / 91) match on 11-16 03:10; check the rest against a rerun before picking any up.

## Open decisions

- None pending. (11-15: ±1 px on free-click readouts, granted; Mirror lost click as live defect, granted.)
