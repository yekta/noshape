# studios: current state

Owner: studios. Areas:
- documents, tabs, start page, Preferences
- versions, branches, history, release
- Variable Studio, Feature Studio (the editor and UI; the language and host stay dx's)
- the drawing UI (from dx, 11-12)
- the assembly UI's feel (11-13; asm keeps the mate solver)

History: docs/handoff/archive/studios.md (dated entries up to 11-15 18:03). Rulings: docs/DECISIONS.md.

Updated 2026-11-22 19:26 UTC.

## In flight

Worktree /root/projects/noshape-wt/studios, branch studios-0811a. Land: `NOSHAPE_RB_HOST=vibex7
NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh`; read `git diff --stat origin/main...HEAD` first. Builds and crate
tests on vibex7 (rb-hosts 6a36d3b1f; off it 02:00-04:00 UTC, qa's quiet window: vibex8 then). Scenarios on vibex8 via
~/bin/slot (<=3 apps): build on vibex7, `scp -3 vibex7:tmp/studios/noshape-uip vibex8:tmp/studios/noshape-uip`, sync
vibex8 with `rb.sh bash -c true`, then `ssh vibex8 "bash -s" < script` (x184b.sh: all drw doc ver vs fs rel scenarios,
score line "score: N/978"; x184d.sh: drw only + the three hover matrices). Never vibe2. Every run under `timeout`;
`touch -c` edited files before rb.sh; two rb.sh runs on one host share a target dir (one waits). Full ops e2e before a
crates/drawing land (drawing-ui alone: not needed).
- LANDED 155f85da as f4dda516 (11-22 13:07): the drawing P0 stack (hover/select, click_still, balloon leader, Change to
  version, Esc rework, qa walk 9, workflow 09 / 26 batches).
- LANDED d1452c9a as d425fa05 (14:40): palette halo, FS-DRAG-SELECT-BAND, view box rule, insert-row selection clear.
- LANDED cd7b7401 as 634aa1f1 (15:21): Select line reach 8 px (lit at 7, not 8), SELECT_FORMAT_REACH_PX 9 (sheet border
  lit at 8, not 9). vibex8 drw run on it (uip1122k): 371/374, same 3 diffs; hover matrices 3/3.
- LANDED dd49dc0f as 1a9112ed (15:47): TICKETS stale rows.
- COMMITTED 3806ff953 (not queued): dx's 3 cells from drw-hover-1122 (text reach 8 px from live's tan box: glyphs +
  0.5 h every side, `font::text_hover_box` / `text_hover_distance`, SELECT_TEXT_REACH_PX 8; no-tool centerline 7 px;
  Note tool lights a centerline from 8 px, NOTE_CENTERLINE_REACH_PX 9, leader attaches there, ops too, AGENT_API
  regenerated). Tests + full ops e2e green (x202).
  REGRESSION it causes: drw-title-block-dblclick s1: our dblclick at sheet (240, 20) (8 px under the DWG NO. row's
  top line) SELECTS a title-block line (orange, 3 grips, x 1422-1726 y 957 in shots) instead of opening the field's
  Note editor (live: dialog "Note"). In `BuiltSheet::hit_reach` (crates/drawing/src/drawing/build.rs ~283) a `best`
  that is not TitleBlock/FormatLine now beats `title_cells`; likely a text (field note / empty field text) reached
  by the bigger hover box. Probe test ADDED (uncommitted) at the end of drawing-ui feel.rs:
  `a_double_click_in_a_title_cell_by_its_line_edits_the_field` (zoom 1084/279.4, asserts Panel::Note field Some, its
  message prints the lit pick). RUNNING at the cap: x212.sh on vibex7 (task busyfkfva, waiter b2at92r0z): that probe,
  variable_feel_tests::, clippy vs-ui/app-core/gpui-pre. Fix so the cell wins, then rerun the 3 scenarios
  (x210.sh: build + x210r.sh on vibex8).
- COMMITTED c1b5a6a07 (not queued): live's table re-record (0a3ac2eee): Insert ▸ keeps the Cell panel on its cell; a
  left press beside the cell's open menu ends the edit, a right press only closes it. App CHECKED 18:24 on vibex8:
  drw-table-cells-insert-row 12/12, drw-table-cell-menu 31/31 (the earlier 370/374 run had a stale binary). The
  debug eprintln is removed.
- UNCOMMITTED, untested beyond the build error fix: FEEL-VS-CHIP-ON-CLICK (P2; live vsm-1 v10: a value cell's chip
  stays on a click). vendor/gpui-pre div.rs: `Interactivity::tooltip_stays_on_click` (+ fluent), its mouse-down
  skips the press gate inside the element and the hover checks ignore pending_mouse_down; text.rs passes `false`
  (that missing arg failed the first build). app-core ui.rs `with_tip_kept_on_click` (tip_with gains `stays`);
  variable_studio.rs value cell uses it. Test `a_value_cells_tooltip_stays_on_a_click`.
- UNCOMMITTED, untested: live vs-rowdrag-1122 (row drag): drag_threshold(5.5) (starts at 6 px), ghost = #1651b0 pill
  with the full name, 31 px, 10 px right / 5 px below the pointer (VsRowDrag gets name + offset; debug "vs-row-ghost");
  no drop line outside the table; Esc in grid_key stops the drag (cx.stop_active_drag). Live's ~550 ms drop delay
  is server slowness, not copied. Test `a_row_drags_from_6_px_as_a_named_pill_and_esc_cancels` (if Esc doesn't
  reach grid_key, focus isn't in the grid during the drag: move the check to a window-level handler).
- TICKETS on main: VS-NAME-FOCUS-BOX (studios) is there twice (line ~445 "[ ]", ~451 "[x]"; fixed in c2e9546c3):
  drop the "[ ]" copy in the land. No open studios P0s.
- Then land all (fetch, rebase, diff --stat, TICKETS duplicates; full ops e2e ran for 3806ff953; rerun it if the
  title-block fix touches crates/drawing).
- drw-centerline-circle-3-point-hole-centres: live takes nothing on an exact Ø8 centre, Ø5 centres take; ours takes
  it. Decision needed from the lead (copy or record as live's failure); not changed.
- QUEUE after the land (lead / live 11-22, all mine; pages under /root/onshape-research/live/studios/):
  1. drw-drag-feel-1122 (dimension text drag: black while dragged, arrows flip outside and the dimension line runs on
     to the text, red + / mark on the other dimensions and red ✕ at the dragged text's centre, pink dotted guide
     rgb(222,142,222) snapping the text to another dimension's line height within 3 px; radial text inside the hole:
     leader from outside; leader arrow-end drag: target edge #CC925C, released on empty sheet stays free, note stays
     selected; press-drag on an unselected note: not moved, selected after (one run only: don't change ours on it);
     view drag colours, Esc mid-drag puts the view back). Our copy of live's kit: /root/studios-scratch/drag/
     drw-drag-feel.json, runner script x213r.sh (vibex8; shots ~/agents/studios-keep/shots-drw-drag-feel, local copies
     drag/s/, montage drag/m1.png). First run: our hover tan + grips match; the Right view's dimension was NOT placed
     by the setup (check its step), the 2nd text drag started a box select (the annotation locator after a drag
     points off the text?), ".197" not found by the annotation locator (ours "R.197": check the locator's match).
  2. vs-rowdrag-1122: done above (uncommitted).
  3. fs-editor-feel-1122: no hover docs; error tooltip at once on the underlined char (1 char off: nothing); AC band
     #D0DBE5, typed letters darker, mouse hover doesn't move the highlight, wheel scrolls; Find prefills the
     selection and searches as you type (~75 ms); Ctrl+H keeps focus in find; no box on an unmatched bracket.
  4. history-gaps-1122: change-row tooltip (one stage, fade from ~50 ms), change-row menu (no Restore to Main…),
     Restore dialog (Esc closes, Enter does nothing).
  5. docs-page-gaps-1122: folder tile hover, "Click to open …" tooltips, thumbnail popover, menu look; ↓ closes the
     menu and moves the list selection.
- Left: ui_doc_state doesn't read HistoryPanel::ui_branch_filter() yet; Esc doesn't close the filter menu. FS snippet
  rounded corners, Esc.
## Next, in order
1. Drawing matrix (11-21 05:21): 1419 = / 381 shown / 0 differs / 0 present-only / 0 missing of 1842 (05:21: the Note
   panel's nested reading counted; its Border size gained Tight fit and 5 Characters). Every panel
   was shot in pd/b1-b35 (tables, inspection / revision, section / detail included). The 456 shown are mostly
   dropdown options our takes don't list in order (fonts, balloon shapes, standards: Drawing properties 101, Create Drawing 57, Title block 54,
   Hole/thread callout 53, Note 52, Hole table 45, Styles 43): only a take opening each list would make them =.
   Regenerate (copy new take outputs into uip-merged first): `drawing_matrix.py --run /root/studios-scratch/uip-merged --ours-menus /root/studios-scratch/menudump.json`

## Feel rows to take (gaps only; check ours against each, fix with a feel test)
- live2 drawing tables (tables-1118 rows 1-5): checked 11-22 01:50, all in our code and tests (row 5's ~3.1 s wait is
  live's server, not a target).
- live VS / FS (live/studios/vs-fs-feel-1117/): VS add-row focus to the next Name, duplicate -> "w_0"; FS
  autocomplete Enter inserts, Tab closes + inserts a tab, Esc keeps text; FS errors while typing (<0.5 s) red x at
  gutter line 1 + squiggle, Ctrl/Cmd+S commits, Commit pales, a fixed error greys ~150 ms, gone ~500 ms. VSFEEL bubble:
  centred over the cell (done 11-18).
- live documents / Import / Derived + Insert (live/studios/docs-import-feel-1118/): search on Enter only; sort A-Z,
  Z-A, default; row hover icons; double-click opens; inline Rename; Delete -> "Send to trash" (Enter nothing, Esc
  closes) + toast with Restore; New document Enter creates. Import: "CAD Imports" tab ~1.8 s, the Part Studio tab
  ~6.9 s, active tab unchanged, blue bar, toast. Derived / Insert search ~350 ms after the last key (live's server debounce, not a
  target; ours filters at once), Enter nothing; Derived Enter / Esc 1 close the panel, 2 commit / cancel (done 11-18).

## Lead's queue (11-17, restated after compaction)
1. Land what is green. 2. User P0: hover / select parity in Assemblies (instances, mate connectors, mates list <->
viewport, parts vs connectors) and Drawings (views, edges, dimensions, annotations: hover, click, box, drag), each
against live with a feel scenario; share psui's pick code (`Viewport::pick_priority`, `AsmPickMap::priority_fn`,
`PickKind::priority`; psui owns viewport.rs' hover path). 3. Versions / branches / release queue.

Agent ids: only /root/lead-scratch/roster.md, read before every message_agent; never copied into notes.
## Waiting on others
- Left to others under BRAND-IN-UI-TEXT: Category values "Onshape Part"… and "Import to Onshape" (dx), "Onshape
  default tolerances" (psui), "Onshape images" / "Onshape form library" (skasm).

## Open (no owner action possible yet)
- P0 FEEL-DRW-WHEEL-ZOOM-SLOW waits on qa's quiet re-time (the bench's zoom frame is under a frame; qa's hover in the
  same app was as slow: the host).

## How to run
Builds and crate tests on the Mac:
- `NOSHAPE_RB_HOST=mac tools/tiers/rb.sh cargo test -p <crate> --lib <filter> > log 2>&1`, then grep the log for
  `^test |panicked|test result|^error|  left|  right`. Never pipe into `head`: the kernel's and vendor's warnings
  fill it.
- The Mac target (~/noshape-host/agents/studios/target there; env ~/noshape-host/env.sh, lead 11-19) must stay under 20 GB (lead 11-15: the Mac folder's cap is 180 GB).
  tools/tiers/prune-target.py reads /proc and fails on macOS, so between builds delete target/debug/deps
  and target/debug/incremental (16 of 24 GB on 11-16) once `du -sh` passes ~15 GB.

App build on vibex6:
```
NOSHAPE_RB_HOST=vibex6 tools/tiers/rb.sh bash -c 'cargo build -p noshape; cp $CARGO_TARGET_DIR/debug/noshape ~/tmp/studios/noshape'
```
An rb.sh build to vibex6 deletes untracked takes there: put them back after it.

ui-parity (lead 11-18: vibex6 is main's watcher, runs go to vibe2 or vibex7 through `~/bin/slot`, one run per 8-core
host; vibex7 has ~/tmp/studios/uip-mine.sh <out> <shards> <display> <prefix>… and the app at ~/tmp/studios/noshape-uip,
built by /root/studios-scratch/r28.sh). Older notes for vibex6:
- Sync: `rsync -a tools/ui-parity/ tests/ui-parity/` to `vibex6:agents/studios/src/…`. The app binary is
  `~/tmp/studios/noshape` there (build above).
- Run: `setsid nohup ~/bin/slot bash ~/agents/studios-keep/shards-names.sh ~/tmp/studios/noshape <out> <names file>
  2 381 > <log> 2>&1 &`; outputs that must outlive 6-12 h go in ~/agents/studios-keep.
- One take: `Xvfb :389 … &`, then `NOSHAPE_UIP_LOGS=1 python3 tools/ui-parity/run.py --app ~/tmp/studios/noshape
  --display :389 --out <out> <name>` in ~/agents/studios/src.
- Compare again without a run: `python3 tools/ui-parity/run.py --compare-only --out <out> <names…>`.
- The script's master copy is /root/studios-scratch/bin/shards-names.sh.

Land:
```
NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh [--no-precheck]
```
It runs a clippy -D warnings precheck (or run clippy on the least-loaded vibexN and pass --no-precheck). Status: `tools/tiers/land.sh --status`.

Scratch tools (/root/studios-scratch):
- cmp.py <take> [out dir on vibex6]: a take's drawing state, live against ours, step by step
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
  - a live EDGE read while a list a drawing panel opened lies over a view (`panel_list_over_view`)
  - a panel live shows a step late after a failed drawing read (`late_dialog`)
  - a take's centermarks where a wait changes live's count (`centermarks_unread`: dx 11-16, drw-centermark[-align])
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
