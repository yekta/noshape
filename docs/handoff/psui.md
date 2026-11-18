# psui (Part Studio UI): current state

Updated 2026-11-17 23:26 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Builds and crate tests: the Mac slot only, `NOSHAPE_RB_HOST=mac tools/tiers/rb.sh cargo …` (one slot, rb.sh waits).
  Never cargo on this machine. Test the crate you changed (`--lib <module>::` or `--test all <file>::`).
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
- Xvfb app runs: `xdotool windowfocus <w>` after start (no WM: no focus, no blur commits).
- UI tests: `NOSHAPE_CONFIG_DIR=$(mktemp -d)` keeps them off the shared ~/.config/noshape.
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
- vibex6 can't reach vibe5: `scp -3`. vibe ~/tmp is pruned after 6-12 h: keep results in ~/agents/psui-keep.

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
- Full psui run 11-16 16:00 (vibex6 full1116): 1009 / 1021. Mine fixed with this land: sketch readouts of two points /
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
  79ae5cdb-42a4-4025-b43a-da61a4f92ad7 (from 11-17; 3b9b1e66 stopped), bool 0f798808-e854-4e83-bbd0-22ae5a1a0578, qa
  132756b3-e48d-4b9f-8a0e-3d4f23629c8b. Old ids stopped.
- skasm's sheet metal colour fix (`Context::reuse_colours_from`) lands after their suite: then recheck our flat view
  colours with fb5 sm-click.json.
- Rerun 11-16: boundary-surface, end-caps, circular click-commit, context-create-version-update, routing match.
  Still apart: gusset-rectangle-aligned step 3 (title error, skasm's), sketch-linear-pattern-click-commit readout
  9.618 vs 10.110.
- Mac target ~20 GB: prune-target.py, and drop debug/incremental dirs idle > 90 min.
- Scratch: /root/psui-scratch/fb5/run.sh runs one one-off scenario (--scenarios ~/tmp/psui/fb5s --all, shots per
  step); scenario files in fb5/ (tb.json, sm-click.json, …; copy one to vibex6 fb5s/ and set its name in run.sh).

## Queued to land

- Landed 11-17 10:28 as a14ef99e (queued as 15a48786): Search tools' toolbar highlight as live (psui/toolbar-highlight: `.is-highlighted` on the
  button, #fcda7f + `inset 0 0 0 2px #fab601`, no border, nothing moves; a dropdown member lights the ⌄ alone).
  ui.rs `search_highlight`, part_studio/mod.rs `found` / `in_menu`, T1 `toolbar_highlight::` (fails with the old
  border: Sketch 99 → 103 px).
- live's timings (11-17, live/psui/timings-1117, SwiftShader lanes): click → selected face drawn 57–67 ms (ours 8.5),
  "Area" readout 167–185 ms; Extrude 1 double-click → dialog 322–448 ms (ours 17), preview 0.73–2.2 s. Put into the
  latency test's TARGETS comments when next touched.
- Landed 11-17 09:54: 498b96a8 (leak fixes + NOSHAPE_SOAK + spawn_guard allow).
- The Mac's one build slot can hold a test 10+ min: for a quick single test use NOSHAPE_RB_HOST=vibex6.

## State 11-18 00:2x (lead's P0 queue after compaction)

- USER-HOVER-STAYS-LIT: latest-wins (5e498ffb) verified by qa on green 7187ac75 (uh3 ×3, 0.7 / 1.5 / 3 s shots: every
  1.5 / 3 s shot right, one 0.7 s shot trailed; qa-scratch/uh/g5e/lit.txt). Closed on my side.
- the_first_pick_at_rest… flaked under load (skasm): it now waits for the GPU's answer with the test clock held
  (landed c5959e26; 4 / 4 on vibex6 at load 24–36).
- Landed 0f7610c8 (23:51): below 20 fps a view jumps, "Where used for Part 1…" own group, handoff.
- Queued 2234f511 (00:2x): feature list as live2 featurelist-feel + qa's 3 P2s (rollback bar: stays over Default
  geometry rows, blue-grey only while rolled back, menu "Roll to end"); rename commits on any press outside; face + edge
  graphics menu = live ctx-menus-1117; list matrix 76 / 76 (new rows: bar held / released / above first / menu,
  refused reorder red, rename Enter / click / F2, renamed tooltip, suppress / Unsuppress, dialog place, undo steps
  (rename, Hide; selection and Shift+5 none), Cmd+click on parts); sketch_ui_tests graphics_menus_over_a_selection_as_live.
- Asked live2 (00:0x): the bar released over the collapsed Default geometry header (10-24 capture said: to the top; ours
  still does that for the header); confirm the blue-grey with the pointer away.
- Main RED 23:53 on asm's a_mate_glyph_wins… (assembly_feel_tests.rs:233, click → selected_mate None). It failed on
  core's land and mine before; no commit since the last green touches assembly-ui or app-core; passes 3 / 3 on the
  Mac. A load flake for asm (its settle() exits on hover_settled; the click may beat the overlay hover).
- Already live's (no change): undo / redo keys (app-core actions tests), Ctrl+Z in a dialog field
  (undo_does_nothing_while_a_dialog_control_has_the_keyboard), wheel 2^(1/12) = ×1.0595 a notch about the pointer,
  500 / 250 / 125 ms animations, part rows toggle on Ctrl or Cmd; live2's (b) Ctrl-only box deselect (ours Ctrl or
  Cmd; live's Cmd unverified): left.

## 11-18 00:4x
- Landed 591643fc / 403bbe0b: feature-list rows, graphics menus over a selection. Queued 8d2e0056: measure angles toward
  middles + Mass button / `[` (live2 measure-mass-1118), spinner as live (900 ms, 50 px box, 1.1 s turn), rollback bar
  back to live2's rb-followup (over Default geometry → top; blue-grey only hovered; qa told), viewport offset test.
- VIEWPORT OFFSET (skasm, lead P0): not reproduced on green. hat15b (copy on vibex6, deleted), 1440 × 900, list at its
  end, Sketch 1, n, 14 wheel steps: the line under the pointer lights and a click selects it. ±10 px is the reach (and the
  screw hole's rim). The perceived offset is latency: at load 27 on llvmpipe the highlight shows 1.2–2.2 s after the
  rest, so a moving pointer sees what it left. Test sketch_hover::the_pointer_lights_the_line_under_it_whatever_the_window_and_list.
  Rig: /root/psui-scratch/offset (run.sh / step*.sh, DOC=… for another document; vibex6 :665).

## Next queue
1. Lead 11-18: pixel parity of hover / select highlights vs live crops (face fill, edge colour / width hover vs select,
   silhouettes, hidden edges, selected part amber, sketch entity hover / select). live's numbers: behaviours
   "EDGE-HIGHLIGHT-LOOK" (10-27); scene tests/edge_looks.rs already checks the edge band and faces; next: selected part
   amber, silhouettes / hidden edges at hover, sketch entity hover / select widths; ask live2 for crops it lacks.
2. qa P2 HOVER-LEAVES-WINDOW: a face hovered just before the pointer leaves the window stays outlined.
3. live2's (b): box deselect with Ctrl only (ours Ctrl or Cmd; live's Cmd unverified): left.
4. Menus vs ctx-menus-1117 menus-sk.json (sketch mode).

## Current order (lead 11-17 ~14:05)

1. Interaction-feel P0 (DECISIONS 11-17): hover and select of points, lines, arcs, constraint glyphs, dimensions,
   planes, faces, edges, vertices; box select both ways; shift / ctrl click. A scenario (feel test) each, fixed to
   live. live2 (79ae5cdb-42a4-4025-b43a-da61a4f92ad7) sends groups (behaviours "USER P0, 11-17 12:2x", Part 2, 2b,
   "psui (11-17 14:1x, feel P0)"). skasm (18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6) owns sketch tools, constraints from a
   selection, sketch_hover / sketch_hover_id / in_sketch_reach; mine: dimension label drag and the feel rows.
2. Full ui-parity rerun split, failures by owner.
3. The tab pick (tab-new-edit-reedit).

## Landed today (11-17)

452838df (region beats planes, own plane never hovers, pick table resent on rebuild; sketch_hover_feel_tests),
1313b3c90 + 291285d6 (remembered values never carry a #variable, per metric / inch memory; legacy store ignored),
19b3b1f2 Adjacent, eb0ab032 Fillets / Matching (queued 14:05 with the handoff a57c59b4), 01fb280e toolbar box model,
e1ce5e41 readouts. qa verified both user P0s on green c330a760 (14:0x).

## Queued 11-17 ~15:00 (feel P0 batch)

- Dimension label drag (live2 2b + feel-1417: starts 5 px from the press, then the text centred on the pointer, no
  snap, never selected): overlay_labels.rs `row_box` keeps the box (hit area) when the text can't be rasterized (the
  GPUI test's text system); sketch_mode.rs `DragState::Label.dragging` (it froze within 5 px of the press).
- Glyph linger (qa: a ⫽ glyph stayed after the pointer left, /root/qa-scratch/uh/x-c330 02-off / 08-off):
  GLYPH_LINGER 550 ms (live2 0.4-0.7 s) and `ui.glyph_linger_task` redraws when it ends; a hovered glyph of the
  lingering row keeps the row.
- Feel tests (real pointer + GPU pick; Mac green): sketch_label (drag; hover tan / click selects / double-click box),
  sketch_glyph (linger end redraw), sketch_box (iso box over the planes: window 4 lines, crossing 3; clicks toggle,
  Shift the same, Ctrl the menu). Live's Esc doesn't clear a sketch selection (behaviours 189): tests clear directly.
- Then: a hovered line's glyph row (Show constraints off) at the pointer's spot on the line, not its middle (live2
  feel-1417); `SketchEditor::glyph_row_at`, kept after the hover ends; glyph boxes carry debug selectors
  `glyph-<constraint>-<entity>`; test sketch_glyph::a_hovered_lines_glyphs_wait_by_the_pointer. Asked live2 whether the
  row follows the pointer along the line (ours follows) or stays where it appeared.
- interaction-feel.md: woken model edge parallel = none (live2 wake-parallel), ours none.
- Show constraints remembered per user (live2 14:3x): skasm's (sketch dialog), sent to them.

## Queued 11-17 ~16:30

- 0a87d6b1: glyph row placed once at the hover start (live2 follow-up), re-lingers off any of its glyphs, a selected
  glyph keeps its row place, glyph click replaces (Shift adds), plane click in sketch mode selects it (editor
  `model_press`); test sketch_box::midpoint_glyph_and_plane_clicks_in_a_sketch_as_live. 170/170 sketch UI tests.
- Remembered values: stores keep lengths per unit ("length:Millimeter": older builds and the ui-parity runner's
  seeds); 1313b3c9 read only "length:metric" / "length:imperial", so every seeded scenario opened our defaults (half a:
  ~18 of 24 psui failures) and users' older memories were lost. apply_remembered reads the system's per-unit groups
  too (own unit last, then the system group). Test updated (older store in "length:Millimeter").

## Full ui-parity rerun (11-17; hosts renamed vibexN 17:3x, vibe5 dead)

Scripts: ~/agents/psui-keep/full-half.sh (half 0 / 1 of every scenario with a live take), list-run.sh (named list with
the staged green), on vibex7 (copies in /root/psui-scratch); scenarios ~/tmp/psui/src-1117 there.
- Half a (vibex6, green f6274d94): 1029 / 1073. Not psui: dx drw-* note dialogs x10 (text.length 30 vs 39) + 2
  (drawing), studios rel-flow-two-parts. psui 24: ~18 were the per-unit memory regression (582351aa); the rest
  bend / flange / helix / jog / plane title errors, intersection-curve and mate-connector focus, mirror-face-type and
  bend-relief-extend field state, hole-countersink-tab title.
- Half b: vibe5's results died with it (1062 / 1074 on 082bd57e). Rerunning on vibex7 (unit psui-full1117b, 4 apps,
  -> ~/agents/psui-keep/full1117-b).
- The 26 psui failures (both halves, /root/psui-scratch/full1117/psui-fails.txt): local unit psui-after-land
  (/root/psui-scratch/after-land.sh, log after-land.log) waits for 582351aa to land and reach green, stages that green
  on vibex7 (half b uses it too if not started) and queues unit psui-list26 (-> ~/agents/psui-keep/list26). Read its
  score.log, fix what is left.
- The 63 psui failures of both halves, scored on qa's 21:03 run (current tools, green 592b0fc9; 49 of 63 there so
  far): 41 / 49 pass. The ~50 "step 3" failures were my stale 11-15 scenario snapshot (old run.py), not the app.
  Left: bend-relief x3, end-cap-profile-rectangle, gusset, hem x2 (sheet metal / frames: skasm, core) and
  intersection-curve-new-edit-reedit (Group 2's red comes one state later than live's: our field blame waits for the
  regeneration; live shows it as the dialog opens with a preselected face). The psui-list63 rerun on vibex7 is still
  queued behind qa's run; it can be dropped once qa's run is complete.
- Before: 11-16 psui list 1009 / 1021.

## Waiting on others

- qa's nightly tab A/B /root/qa-scratch/nightly/tabab-1118.txt: close DOCK-TAB-SWITCH-TRAILING-FRAMES if both settle
  ~100-200 ms.
- skasm: bend-relief vertex label; gusset-rectangle-aligned step 7, end-cap-profile-internal step 5.
- dx: face-rclick-export-dxf. core: boundary-surface, gusset frame part order.

## Known, left as they are

- sketch-mirror-preselected-batch: live's lost first click after a box select is a live defect (DECISIONS 11-15).

## Next, in order

1. Feel rows still to write (live2 Part 2 / 2b): point hover 10 px and point-over-line, dimension hover tan / click
   orange / double-click box, glyph hover = secondary, box select L→R window / R→L crossing (lines only), shift /
   ctrl click, 3D faces / edges / vertices with planes behind (iso).
2. Older "Next" items in the archive: check against the rerun first.

## Open decisions

- None.
