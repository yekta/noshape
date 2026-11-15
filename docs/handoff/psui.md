# psui (Part Studio UI): current state

Updated 2026-11-15 19:21 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

- Full psui-area rerun (1021 scenarios), vibe6, 3 shards, started 19:02 UTC (not through slot: started before the
  rule): out /home/ngyekta/tmp/psui/full1115e, log /root/psui-scratch/full1115e.log. Lead 11-15: fix what it shows.
  Baseline: the 11-15 morning run, /root/psui-scratch/full1115-score.log (972 / 1021).

## Queued to land

- Nothing (all landed as of 2a525af7, 19:15 UTC).

## Waiting on others

- live2 (REQUESTS): 13:57 does Chrome's own menu show on a right-clicked Depth (dialog-number-rclick-menu: ours opens
  the input's Cut / Copy / Paste, which takes live's dialog-closing Escape); 16:04 Boundary surface's selection drop
  after the second pick, 3 runs (boundary-surface-new-edit-reedit; every other dialog keeps its picks); 16:04
  analysis-thickness ×3 retaken on Pro (ours has Thickness analysis; the free plan didn't); 17:2x the navbar's
  workspace label while editing another document's part in context (context-create-version-update step 3: live null,
  ours "Main").
- skasm: gusset-rectangle-aligned step 7 and end-cap-profile-internal step 5 (live titleState error, ours ok: their
  validation); tab / bend-relief owner labels ("Face of Flange 1" vs "Sheet metal model 1"; their REQUESTS 16:27);
  sketch Linear pattern without Equal + its construction direction line (after 58a0bd16, the circular one).
- studios: docs-account-* (live2 retakes).
- dx: face-rclick-export-dxf.

## Known, left as they are

- Sketch readouts of free clicks beyond ±1 px (DECISIONS 11-15): sketch-linear-pattern-click-commit (1.8 px),
  sketch-key-symmetric (1.7), sketch-key-concentric-two-circles (2.0), sketch-key-midpoint (1.0, live's own point
  1.67 px off in y). Reported to the lead.
- sketch-mirror-preselected-batch: live's lost first click after a box select is a live defect (`live_defects` from
  step 20, DECISIONS 11-15); the rest matches.

## Next, in order

1. When full1115e finishes: `grep -v ': match'` its score.log, compare with the morning baseline, fix psui misses,
   route the others (owners: core model/regen, skasm sketch solver / sheet metal / frames, blend fillets, asm assembly
   menus, studios drawings / documents / account, dx exchange).
2. Rerun after the others' fixes land: boundary-surface-new-edit-reedit (core 658bef9d), the gussets and end-caps
   (skasm), sketch pattern takes (skasm 58a0bd16 + linear), context-create-version-update (live2's answer).
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
