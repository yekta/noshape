# studios: current state

Owner: studios. Areas:
- documents, tabs, start page, Preferences
- versions, branches, history, release
- Variable Studio, Feature Studio (the editor and UI; the language and host stay dx's)
- the drawing UI (from dx, 11-12)
- the assembly UI's feel (11-13; asm keeps the mate solver)

History: docs/handoff/archive/studios.md (dated entries up to 11-15 18:03). Rulings: docs/DECISIONS.md.

Updated 2026-11-15 19:25 UTC.

## In flight

Worktree /root/projects/noshape-wt/studios, branch studios-0811a, on main 482d52e7 plus the uncommitted batch below.
Nothing is queued to land; every earlier commit is on main (last: 53a5e6d0 → 482d52e7, 18:35).

Uncommitted batch (one commit when green):
- Feature Studio completion (fs_complete.rs, ace_order.txt; live2 feel/fs-ac-ties): the score is live's completer's:
  one per matched letter among the name's first 32, −1 when the match doesn't start the name, −1 per extra run. Ties
  keep live's completer list order (ace_order.txt, 1038 names). Constants are listed ("constant"). Only modules
  common.fs re-exports (`export import`) are offered. T1 `ties_keep_lives_list_order`; all 6 fs_complete tests pass.
- Notices count (feature_studio.rs `notice_lines`): the marked gutter lines plus 1 while a squiggle's tooltip shows.
  Live's tooltip reuses the gutter cell's class (live2 fs-squiggle-hover-gutter).
- Angular dimension (model-data sheet.rs), ANSI only: the value's centre sits on the arc and the arc is broken
  around it, as ANSI linear values. Live's DXF puts the text centre at the arc's radius (0.3215 in); ours was 0.1 in
  out. ISO keeps the value above the arc. drw-dimension-angular matches with it (vibe6 19:20).
  - Open: T1 `a_tight_angular_dimension_puts_its_arrows_outside` fails its "value on the arc" assert. The message
    was being read on the Mac (task bb2xa4foi): either the test's box centre or a real offset.
- ISO A4 Zoom to fit (camera_ops `drawing_fit_sheet`, used by drawing_view and ops camera): the frame fills the
  canvas less 19 px, centred: 3.448 px/mm portrait, 5.02 landscape, frame y 85..1039 (live2 A4P/A4L-empty-fit).
  Other sizes fit the paper (926 px, 14.75 px low). T1 `an_a4_sheet_fits_its_frame`, not run yet.
- 3 point angular (drawing_ui_state.rs): after the third point the selection reads EDGE of the view. Live shows
  the points' orange markers and the preview (live2 retake 11-15 19:27). Not built yet.

Last run (vibe6 19:20, fs-* and drw-dimension-*, before the 3-point rule): 127/132.
- drw-dimension-3-point-angular: fixed in the batch.
- drw-dimension-foreshorten: live2's retake (now EDGE like ours) is in their land queue.
- fs-key-forward-reference, fs-ps-added-tool-menu, fs-tb-narrow-import: not run (app start / X flake). Rerun them.

## Queued to land
None.

## Waiting on others
- live2: drw-dimension-foreshorten and drw-dimension-3-point-angular retakes (queued 19:27); the five fs-ac-tie-*
  scenarios and takes (queued 18:21). Rerun all once on main.

## Open (no owner action possible yet)
- fs-ac-fn-std-list's tie: solved by the batch (NewBodyOperationType 917 before BooleanOperationType 925 in live's list).
- drw-insert-view-top-1to1: live reads VIEW where five other Insert view takes read EDGE (the reader's pixels).
- drw-hole-callout-palette steps 17-18: our callout's leader is lit inside Top's box after the palette closes; live
  reads []. Near the reader's 60 px threshold.
- A table's Bring to front greyed when just placed (REQUESTS 12:38; ours greys it only for a table alone).
- Projected view preview: ours snaps at 22.5° both ways. Live2's coarse sweep put the side/iso boundary at 25-30°
  (250 px) and 20-25° (400 px), and top at 20-25° from vertical. Left as is until a finer sweep.
- Assembly "?" rows: a list row's tooltip (live2 ~70 ms), a selected instance's outline through other parts in
  explode mode, a Revolute-constrained drag's grab point.
- context-create-version-update: an old difference, not new.
- Coverage items that need live takes:
  - documents: the Print… pop-up toast, the quick tab switcher's hover + mouse-up, shortcuts with focus on the top bar
  - Feature Studio: the pointer cursor over an identifier with Ctrl held
- Release: the gap list in the archive (11-12 07:33 entry) is partly stale. Submit already makes the version, and
  approvals and the release_* ops (approve, reject, comment, drafts, obsolete, settings) exist. Re-audit it against
  the code before taking an item from it.

## Next, in order
1. Read the angular test's message, fix it, run noshape-drawing, drawing-ui, scene, feature-studio-ui, model-data on
   the Mac, rebuild on vibe6. Rerun fs-*, drw-dimension-* and the 3 not-run takes, then land the batch.
2. When live2's retakes are on main: rerun drw-dimension-foreshorten, -3-point-angular and fs-ac-tie-*.
3. Refresh docs/parity/drawing-matrix.md (last 11-15 14:30: 949 of 1868 live items compared and matching).
4. interaction-feel.md's remaining "?" rows in Drawings / Documents / Assembly (ask live2 for the missing captures).
5. A full run of my areas' takes (655 non-drawing, about 290 drawing) to catch regressions since 15:05.

## How to run

Builds and crate tests on the Mac:
- `NOSHAPE_RB_HOST=mac tools/tiers/rb.sh cargo test -p <crate> --lib <filter> > log 2>&1`, then grep the log for
  `^test |panicked|test result|^error|  left|  right`. Never pipe into `head`: the kernel's and vendor's warnings
  fill it.

App build on vibe6:
```
NOSHAPE_RB_HOST=vibe6 tools/tiers/rb.sh bash -c 'cargo build -p noshape; cp $CARGO_TARGET_DIR/debug/noshape ~/tmp/studios/noshape'
```
An rb.sh build to vibe6 deletes untracked takes there: put them back after it.

ui-parity on vibe6 (≤2 apps while psui runs there; heavy runs through `~/bin/slot <cmd>`, lead 11-15):
- Sync: `rsync -a tools/ui-parity/ tests/ui-parity/` to `vibe6:agents/studios/src/…`.
- Run: `~/bin/slot bash ~/agents/studios-keep/shards-names.sh ~/tmp/studios/noshape <out> <names file> 2 381`.
- Compare again without a run: `python3 tools/ui-parity/run.py --compare-only --out <out> <names…>` in
  ~/agents/studios/src.
- The script's master copy is /root/studios-scratch/bin/shards-names.sh. vibe6's ~/tmp is pruned after 6-12 h, so
  keep the script in ~/agents/studios-keep.
- NOSHAPE_UIP_LOGS=1 keeps the app's stderr per take.

Land:
```
NOSHAPE_LAND_QUEUE_HOST=vibe5 NOSHAPE_LAND_HOSTS="vibe5 vibe3" tools/tiers/land.sh
```
It runs a clippy -D warnings precheck. Status: `tools/tiers/land.sh --status`.

Scratch tools (/root/studios-scratch):
- cmp.py <take> [out dir on vibe6]: a take's drawing state, live against ours, step by step
- ac_ties.py, ac_cand.py: Ace tie pairs and candidate needles
- dxfdim.py <dxf…>: a DXF's DIMENSION / MTEXT / ARC entities

## Standing facts (still true)

How the areas are measured:
- Each area scores `match × coverage` from the interaction suite (tests/ui-parity, docs/PARITY-SUITES.md §2).
- Universes are in tools/ui-parity/universe/<area>.json, with cloud items marked `"cloud": true`.
- Coverage: `python3 tools/ui-parity/coverage_universe.py [--uncovered] <area>`. 11-15: documents 310/313, versions
  167/167, Variable Studio 98/98, Feature Studio 118/119.

| Area | Weight | Code |
|---|---|---|
| Documents, tabs, start page, preferences | 7 | crates/start-ui, crates/preferences-ui, crates/app (tab bar, document menu) |
| Versions, branches, history, release | 5 | crates/history-ui, crates/release-ui |
| Variable Studio | 3 | crates/variable-studio-ui |
| Feature Studio and the FeatureScript editor | 5 | crates/feature-studio-ui |

Runner (tools/ui-parity/run.py), what it knows about live's takes:
- Page height per take (`window_heights`), passed to the app as NOSHAPE_UI_WINDOW_HEIGHT. It reads, in order:
  - app.viewport (recorder 2026-11-15.2 on)
  - the camera (height_mm / mmPerPixel + 106)
  - a drawing's sheet_point click against the ANSI A fit
  - the nearest take of the same lane within 6 h

  Live has recorded at 1919x992 and at 1919x1079 (canvas 973). The app's default stays 992.
- Recorder 2026-11-15.1+ reads panels as controls (`cell-format`, `palette`) and drawing context menus as "RmbMenu"
  (`drawing: true` in ours, dropped for older takes by `recorder_form`).
- Not compared:
  - a list's `name`
  - a field's list live didn't read (`select_unread`)
  - a document read before it loaded (`document_loading`)
  - a view's frame drawn late (`view_frame_late`)
  - texts written late (`texts_late`)
  - a hole callout's export text: count and diameter only (`hole_callout_export_text`)
  - a take's `live_defects` steps (DECISIONS 11-15)
- Live's selection reader: more than 60 px of selection colour inside a view's box reads EDGE. A finished line to
  line angular or max/min dimension stays selected after Esc; other dimensions don't.

Drawings:
- Zoom to fit: ANSI A/B and ISO A3 fit the paper 926 px tall in a 973 px canvas, 14.75 px below its centre. A4 fits
  its frame (above).
- Dimension palette: its main row starts at the text's centre line (top = text mid − 42 px). Its halo (40 px left, 20
  right, 30 above and below) closes a palette not yet clicked into and swallows clicks once it has been.
- Hole callout text: Noto Sans advances at 0.12 in. A selected callout is an orange box behind its text, its holes
  light blue.
- Profile tools (Crop, Broken-out, Detail) snap a click to the nearest view edge within 6 px on screen, inside the
  material and out (live2 crop-snap-inside, ff244f19).

Feature Studio:
- The completion list shows 250 ms after the key (`fs_complete::LIST_DELAY`), at once while it is open. Ace draws 9
  rows (`ACE_DRAWN_ROWS`).
- The notices count: the notices pane's rows when it is open, else the marked gutter lines (+ the squiggle tooltip).

Agent API Hole: DECISIONS 11-15. e2e `a_hole_size_given_is_used_as_given`, `hole_standard_sets_the_table_sizes`.

Speed (lead 11-14): five T2 UI benches (`profile ui-bench`): documents page, version graph, Variable Studio, Feature
Studio, drawing. The before / after table is in the archive's 11-14 13:24 entry.
