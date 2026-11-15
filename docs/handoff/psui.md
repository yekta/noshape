# psui (Part Studio UI): current state

Updated 2026-11-15 23:48 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

- qa P0 (dock tab B first open 7-10 s): cause found and fixed (f8e38653, queued with this handoff): the tab's hover
  card (app tab_thumbs.rs) regenerated B in full with fine meshes beside the view's own opening (the pointer rests on
  the tab before the click). Now it takes the open view's display, else regenerates with coarse meshes, stopped by any
  view regeneration of the document (`start_thumbnail::picture_job_started`). A/B on vibe6, 3 alternating pairs, load
  3-9 (candidates): tabToB settle base 6240 / 5568 / 6486 ms, fix 3535 / 5777 / 3650. Left: B's preview regeneration
  (2.2-2.6 s, 75 features) and the refine (Part 1's 285k-triangle fine mesh, 0.3 s idle, up to 2 s under load) before
  "settled"; the coarse model shows at the preview. Repro: tabab.sh / tabab-ab.sh (scratch) via rb.sh on vibe6 with a
  mode-700 dock copy (scp -3 from vibe5 ~/lead-private/dock.noshape to /home/ngyekta/tmp/psui/priv; the scripts' EXIT
  trap deletes it). Gotchas: start Xvfb by PID (a pgrep -f pattern matches the script's own command line); a file put
  back by rb.sh's rsync keeps its older mtime, so touch it or cargo keeps the stale build.
- end-cap-profile-circle step 3: ours keeps the Faces field focused after the face click until the regeneration;
  live drops focus at the click (Mirror's Faces drops it at the click in both). Not found yet: next, a logged run.
- tab-new-edit-reedit: live's click at (30, 35, 2) lands ~1.1 px (0.3 mm at 0.275 mm/px) on Flange 1's wall; ours on
  the plate's top. Decision asked of the lead: known (sub-pixel camera) or retarget the click onto the wall live hit.
- live (b89af730) asked 11-15 23:10 for the feel rows without live numbers (snaps, Measure midpoint window, Fields
  wheel / Tab / New variable, list insertion line, Feedback, tooltip fade, view cube reach): fold its
  live/feel/<area>-gaps.json into the matrices when it answers.

## Queued to land

- f8e38653 hover card fix, 4195ec4d prune-target objects, this handoff (23:48).
- Landed 11-15: c5b7e380 (dropdown P1: lists out of the flow, live's corners; fields matrix 11 / 11), acdf09d2.
- Mac target 42 -> 14 GB (lead 11-15): prune-target.py now runs on macOS and removes stale codegen-unit objects.

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

1. end-cap-profile-circle focus (logged run); the tab pick per the lead's answer; fold live's feel numbers in.
2. Rerun after the others' fixes land: boundary-surface-new-edit-reedit (core 658bef9d), the gussets and end-caps
   (skasm), sketch pattern takes (skasm 58a0bd16 + linear), context-create-version-update (live2's retake).
3. Older open items (11-06 to 11-08 "Next, in order" in the archive) were not re-verified this session; check them
   against the rerun before picking any up (e.g. S-TOOLBAR-OVER-TABS: the S toolbar over the tab bar; qa's 11-12
   sketch-constraint-refusals step 59, not in the psui take list).

## Open decisions

- None pending. (11-15: ±1 px on free-click readouts, granted; Mirror lost click as live defect, granted.)

## What changed today (11-15), for orientation

- Mirror: 24 / 25 Mirror scenarios; side by sides for the user in /root/psui-scratch/mirror-sxs.
- Tab with a number field's unit list open applies the field (qa's hat15 fillet edit); a sketch edit's regeneration
  clears the stuck working overlay.
- Analysis tools as live: Zebra / Reflection keep their face selected on close; Curve/surface from the toolbar or Shift+C
  drops a preselection (`ui.analysis_menu_target` for the menu path).
- A blamed empty field stops reading red at its first pick (fixed plane-line-angle, ruled-surface-extended-cubic,
  offset-surface-new-edit-reedit, revolve-add-tab).
- Sketch Text dialog as live's 3 takes; a text placed on a point is Coincident with it; scenarios read a text as one
  "other" entity.
- Part Properties: Reset all goes by saved values; P0 fixed: saved metadata survives Undo / Redo of earlier steps
  (`keep_part_metadata`, ops/session.rs), Save is no undo step unless it renames or changes the mass override.
- Coincident edges pick the first drawn (line-pick depth Greater): the earlier part, as live at a mitred frame corner.
- Sketch patterns: two constraints each in scenarios; a white-space accept ends the tool.
- Runner: `live_defects`, `click_placed_readouts` (±1 px), per-take window heights (studios).
