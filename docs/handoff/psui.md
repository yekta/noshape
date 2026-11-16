# psui (Part Studio UI): current state

Updated 2026-11-16 13:31 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

- Landed 11-16 (to 07:42): flat bend dash / hover / click, pattern labels, 2D manipulator and its boxes, snaps spline
  rows, woken model edges, failed-feature part highlight, circular pattern handle + clockwise, view cube hover map,
  tooltip fade-out (vendored gpui div.rs / window.rs, 150 ms). Details: archive.
- Landed 08:35: 6741b373, the user's toolbar P1 (lead 07:13). Icons' light faces opaque white (`data-face` white
  underlays; `data-face="themed"` turn #333 in dark; assets.rs `dark_icon`; test `icons_with_faces_are_opaque_inside`,
  `SEE_THROUGH` allowlist of 18 where live is see-through too). Buttons as live2's toolbar-states + live's addendum:
  pressed #EAEAEA + inset, active hovered flat #DFDFDF (sketch group #CECECE), ⌄ hover tool #EAEAEA / arrow #D7D7D7,
  ⌄ open #D7D7D7 + inset, search highlight border 2 px. Vendored gpui-component Button: `pressed_inset`, `flat_hover`.
- Landed 12:42 (99dea19f): box shadows blur as CSS (`BoxShadow::blur_sigma`, vendor/gpui-pre/NOSHAPE.md, test
  `ui::tests::box_shadow_blur_is_css`), halved call sites back to CSS, toolbar lists and tooltips with live's shadows;
  hole-table / drawing-hole-table, configuration, import-foreign icons. Crops /root/psui-scratch/shadows.
- Queued: 547b4db1 (ui-parity: free-click readouts within one screen pixel on the sketch plane, doubled for two-click
  measures, lead 11-16; `pixel_on_plane`; README), 04e40652 (toolbar dropdowns: 180 px with border, icon cell 2 px
  in, rows with chips 35 px; measured equal to live menus/ps-dropdown-01 within 1 px).
- Running 13:20: full psui shards on vibe6 (/home/ngyekta/tmp/psui/full1116, psui-names.txt): take the next miss.
- Paused until ~10:15 UTC 11-17: asm, skasm, bool2, studios, live, live2. Don't message them (it wakes them): write
  into onshape-research/live/REQUESTS.md or their handoffs. Waiting there: live/live2's tool-list box model request
  (now measured from menus/ps-dropdown-*; can be dropped).
- skasm's sheet metal colour fix (`Context::reuse_colours_from`) lands after their suite: then recheck our flat view
  colours with fb5 sm-click.json.
- tab-new-edit-reedit: decision asked of the lead (11-15 23:50; click ~1.1 px from Flange 1's wall).
- Rerun 11-16: boundary-surface, end-caps, circular click-commit, context-create-version-update, routing match.
  Still apart: gusset-rectangle-aligned step 3 (title error, skasm's), sketch-linear-pattern-click-commit readout
  9.618 vs 10.110.
- Mac target ~20 GB: prune-target.py, and drop debug/incremental dirs idle > 90 min.
- Scratch: /root/psui-scratch/fb5/run.sh runs one one-off scenario (--scenarios ~/tmp/psui/fb5s --all, shots per
  step); scenario files in fb5/ (tb.json, sm-click.json, …; copy one to vibe6 fb5s/ and set its name in run.sh).

## Queued to land

- 6b16ed19 (shadows, icons); this handoff.

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

1. Recheck the flat view's colours after skasm's colour fix; the tab pick per the lead's answer.
2. Older open items (11-06 to 11-08 "Next, in order" in the archive): check against a rerun before picking any up.

## Open decisions

- None pending (11-16: free-click tolerance answered and in the runner).
