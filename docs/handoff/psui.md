# psui (Part Studio UI): current state

Updated 2026-11-17 10:45 UTC (new thread on this account since 10:35). History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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
- Landed: d75fd4d1 (free-click readouts within one screen pixel on the sketch plane, doubled for two-click
  measures; `pixel_on_plane`; README), b752d3ad (toolbar dropdowns 180 px, icon 2 px in, rows with chips 35 px).
- Landed df0f2ae0: the manipulator's arrows grabbed along their shafts (qa's sketch-entity-copy-paste regression).
- Full psui run 11-16 16:00 (vibe6 full1116): 1009 / 1021. Mine fixed with this land: sketch readouts of two points /
  a point and a line ("Min dist") and two circles (Center / Max / Min dist), sketch-key-symmetric / -midpoint /
  -concentric-two-circles match. Left, others': bend / jog / tab "Face of …" labels and bend-relief related rows
  (skasm), gusset title error (skasm), face-rclick-export-dxf (dx).
- Speed (lead 11-16, PLAN 2.7), live's 30-feature part on the Mac, release, real frames, 120 Hz tick: T2
  `latency::everyday_interactions_beat_live_on_the_30_feature_part` (`cargo test --release -p noshape-part-studio-ui
  --lib latency:: -- --ignored --nocapture`). Median of 3 Mac runs (11-16 20:54 Mac time, load 4–7, all three alike): hover after live's 8-move approach 16.9 ms (target 30; live
  10–50 + pick), click → selection drawn 8.5 (16.7), Extrude 1 dialog on its preview 17 (100; live 200–300), a depth
  step's preview 17 (100; live ~400), ✓ → redrawn 43 (155; live 310–350). None slower than live. A whole-frame pick
  prefetch takes hover to 8.5 (/root/psui-scratch/ab/hover-prefetch.patch) but reads back 15–60 MB per scene change:
  not landed. Live never timed click → selection or a dialog's open: asked in REQUESTS.
- tab-new-edit-reedit: ours clicks the middle of Flange 1's wall (`ours_on`, lead 11-16); matches 22/22.
- Idle cost (lead 11-16), the Mac, release, the 30-feature part: 5 min idle 0 frames presented (NOSHAPE_FRAME_LOG),
  CPU 0.1 %, RSS flat 313.7 MB (/root/psui-scratch/idle/idle.sh). Open/close: `NOSHAPE_SOAK=<n>` (crates/app soak.rs:
  opens the command line's document, waits until drawn, closes; logs RSS and whether the workspace, document, Part
  Studio view and viewport were freed; soak*.sh in idle/ add vmmap / heap / leaks --traceTree snapshots). Two leaks
  fixed: a closed document's live server thread + port (ops live.rs, test `a_dropped_server_ends_its_thread`); the
  kernel's display mesh cache kept every face it meshed (mesh.cpp `drop_unreferenced`: keys holding a face's only
  references go once a set doubles, across sets: each face is a key at the view's and a thumbnail's deflections; test
  `rust_mesh::the_mesh_cache_drops_faces_no_shape_keeps`). 30 cycles: +2.3 MB / cycle before, ~0.2 after.
- Agents back 11-17 09:25 as fresh threads (full ids): asm d8d597e0-675c-497b-a804-544ceab99bbd, skasm
  18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6, bool2 e98465f5-5cc9-4d4f-b67e-aae1d5e6d2e0, studios
  58e9bca8-346c-4847-8012-e15006ae61ef, live e8549719-ef9a-42de-960c-e24b29f142a6, live2
  3b9b1e66-33a0-4dfe-9e0a-eee216e354b2, bool 0f798808-e854-4e83-bbd0-22ae5a1a0578, qa
  132756b3-e48d-4b9f-8a0e-3d4f23629c8b. Old ids stopped.
- skasm's sheet metal colour fix (`Context::reuse_colours_from`) lands after their suite: then recheck our flat view
  colours with fb5 sm-click.json.
- Rerun 11-16: boundary-surface, end-caps, circular click-commit, context-create-version-update, routing match.
  Still apart: gusset-rectangle-aligned step 3 (title error, skasm's), sketch-linear-pattern-click-commit readout
  9.618 vs 10.110.
- Mac target ~20 GB: prune-target.py, and drop debug/incremental dirs idle > 90 min.
- Scratch: /root/psui-scratch/fb5/run.sh runs one one-off scenario (--scenarios ~/tmp/psui/fb5s --all, shots per
  step); scenario files in fb5/ (tb.json, sm-click.json, …; copy one to vibe6 fb5s/ and set its name in run.sh).

## Queued to land

- Landed 11-17 10:28 as a14ef99e (queued as 15a48786): Search tools' toolbar highlight as live (psui/toolbar-highlight: `.is-highlighted` on the
  button, #fcda7f + `inset 0 0 0 2px #fab601`, no border, nothing moves; a dropdown member lights the ⌄ alone).
  ui.rs `search_highlight`, part_studio/mod.rs `found` / `in_menu`, T1 `toolbar_highlight::` (fails with the old
  border: Sketch 99 → 103 px).
- live's timings (11-17, live/psui/timings-1117, SwiftShader lanes): click → selected face drawn 57–67 ms (ours 8.5),
  "Area" readout 167–185 ms; Extrude 1 double-click → dialog 322–448 ms (ours 17), preview 0.73–2.2 s. Put into the
  latency test's TARGETS comments when next touched.
- Landed 11-17 09:54: 498b96a8 (leak fixes + NOSHAPE_SOAK + spawn_guard allow).
- The Mac's one build slot can hold a test 10+ min: for a quick single test use NOSHAPE_RB_HOST=vibe6.

## Current task (lead 11-17 09:25)

1. Rerun the full ui-parity suite (`tools/ui-parity/shards.sh`: every scenario with a live take, 2146; the psui list
   alone is psui-names.txt, 1021) and the interaction-feel suites (the `*_feel_tests` in part-studio-ui) on vibe6
   (≤4 apps, through ~/bin/slot, under `timeout`) against current green; list the failing scenarios by owner; fix the
   psui-owned ones, P0/P1 first (look, hover/pick, timing). Report before/after pass counts in ≤5 lines. Not started
   (the move to Ubeyd came first). Owner mapping: no owner field in scenarios; by area (sketch / part studio / chrome /
   viewport → psui; sheet metal → skasm; assemblies → asm; drawings → dx/studios; …) as the 11-16 run's notes.
2. Toolbar dropdown box model (live2 11-17 09:26, live/feel/shadows/README.md "Toolbar dropdown list box model",
   tooldd-*.json/png): rows 1 px margin + 6 px padding, icon at list.x + 8, name text at + 32 (margin 0 4px), row 32
   (34.5 with chips), chips min-width 17, padding 0 2px, 1 px #AAA, radius 2, 11 px on 16.5, no gap, no background;
   chips end at list right − 8. Our edit, unverified, saved as /root/psui-scratch/toolbar-boxmodel.patch (`git apply`
   it: ui.rs `shortcut_chips` / `menu_tool_item`, `shortcut_chip` removed as unused; popup_menu.rs tool_list item
   pr 7 → 6). Next: build, screenshot a Fillet ⌄ / Sheet metal ⌄ list (fb5/tb.json), compare with live's
   tooldd-fillet-page.png / tooldd-sheetmetal-page.png, then land.
3. Sketch selection readouts: done, landed e1ce5e41.

## In progress (11-17, new thread)

- Full ui-parity run, split (lead 11-17 11:55: a run deep in one host's queue moves): green 082bd57e staged as
  ~/agents/psui-keep/noshape, scenarios snapshot ~/tmp/psui/src-1117, script ~/agents/psui-keep/full-half.sh
  (/root/psui-scratch/full-half.sh). Half 0 on vibe7 (4 apps, :386-389) -> ~/agents/psui-keep/full1117-a, half 1 on
  vibe5 (3 apps, :386-388) -> full1117-b; units psui-full1117 through each host's ~/bin/slot, logs
  ~/agents/psui-keep/full1117-{a,b}.log. 12:00: vibe7 third in its queue, vibe5 second. When both are done: copy
  both into one dir and `run.py --compare-only` over all names for the score; split failures by owner (area field).
  vibe6's run cancelled.
- Feel matrices (Mac, 11-17 10:50): 18 / 18 pass. Command: `cargo test -p noshape-part-studio-ui --lib --
  ui_tests::feel:: ui_tests::snaps:: ui_tests::list:: ui_tests::measure:: ui_tests::nav:: ui_tests::chrome::
  ui_tests::toolbar_highlight:: ui_tests::sm:: ui_tests::sketchtools:: ui_tests::editing:: ui_tests::dialogs::
  ui_tests::fields:: ui_tests::feedback::` (module path part_studio::sketch::ui_tests::<m>; one test per matrix).
- 11-17 12:25: interim vibe5 half 142/144 (asm-cylindrical-limits: asm; bend-relief-new-edit-reedit related row
  Flange 2 vs live's Flange 1: skasm's vertex label item).
- Landed 01fb280e: toolbar box model. Queued: Query variable Adjacent (std qAdjacent; T1
  features_b::query_variable_adjacent_as_std; audit A3). Asked live2 (REQUESTS 12:21) for Fillets Less / Greater and
  Matching: std says all fillets of the body, ours Equal is tangent-connected only; no change until live answers.
- Checked, nothing to do: flat view colours after 0bb94eede (ours 126,165,189 vs live 124,164,187; our ui-parity
  window's right 242 px are off-screen, so the flat looks cut); tab-new-edit-reedit 22/22 (live's ray 0.3 mm = 1.1 px
  off the edge: sub-pixel camera, `ours_on` stays); S-TOOLBAR-OVER-TABS already fixed; audit D134 (Frame profile) is
  only the oracle's type, the picker is there.
- 11-17 12:00: sketch-linear- / -circular-pattern-click-commit match (28/28 states, build e1ce5e41 + toolbar).
- Queued 22349e4f (toolbar box model, chips y 11-29 vs live 11-30).
- Landed e1ce5e41 (11:12): sketch pair readouts as live's (arcs as arcs, Center to the infinite line, order by kinds,
  point on line Min only), lone circle / arc icon, values as measure_value; latency TARGETS note live's timings.
- Toolbar box model: patch applied in the worktree (uncommitted) + chips on the foot of a 22.5 px line
  (`shortcut_chips` h 22.5 items_end). First screenshot (fb5-tooldd, before the chip fix): Fillet list 180 x 71
  (live 71.5), icon art x 9-26, text from 33, chips 127-171 all equal to live's; chips were 2 px high (ours y 9-27,
  live 11-30). Rebuild + rerun: vibe6 unit psui-tooldd (`~/agents/psui-keep/run1.sh tooldd`, log
  ~/tmp/psui/tooldd.log, shots ~/tmp/psui/fb5-tooldd/shots 02 = Fillet list, 06 = Sheet metal list). Measure
  script: /root/psui-scratch/tooldd (chip #aaa extents). Then commit + land.
- qa P2 DOCK-TAB-SWITCH-TRAILING-FRAMES: repro ~/tmp/psui/tabab (vibe6; live-30-features + duplicated tab;
  ab.sh rounds apps). Same session 0.4.53 and 0.4.55 both ~850 ms at load 36-43; 0.4.55 208 ms at load 29: load /
  software GL, not a regression. Told qa (11:50). qa agrees (vibe7 load 17: 0.4.53 818 ms too); qa's nightly (vibe7 alone, 3 rounds) -> /root/qa-scratch/nightly/tabab-1118.txt: close if both settle ~100-200 ms there.

## Waiting on others

- skasm: bend-relief-* "Vertex of Flange 1" (label after the feature that first created the definition entity: theirs,
  in progress). tab-new-edit-reedit is a pick: live's ray passes ~0.3 mm above Flange 1's top edge and hovers its outer
  wall; ours the plate's top. Mine: queue after the P1.
- skasm: gusset-rectangle-aligned step 7, end-cap-profile-internal step 5 (their validation); sketch circular pattern
  Equal (sketch-circular-pattern-click-commit constraints[2]).
- live2: analysis-thickness ×3 (our list lacks the Pro rows?), context-create-version-update retake on main.
- dx: face-rclick-export-dxf. core: boundary-surface (with my 11620bb3).

## Known, left as they are

- sketch-mirror-preselected-batch: live's lost first click after a box select is a live defect (`live_defects` from
  step 20, DECISIONS 11-15); the rest matches.

## Next, in order

1. Recheck the flat view's colours: skasm's fix landed 0bb94eede (11:23); fb5 sm-click.json. Then the tab pick per the lead's answer.
2. Older open items (11-06 to 11-08 "Next, in order" in the archive): check against a rerun before picking any up.

## Open decisions

- None pending (11-16: free-click tolerance answered and in the runner).
