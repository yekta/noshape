# studios: current state

Owner: studios. Areas:
- documents, tabs, start page, Preferences
- versions, branches, history, release
- Variable Studio, Feature Studio (the editor and UI; the language and host stay dx's)
- the drawing UI (from dx, 11-12)
- the assembly UI's feel (11-13; asm keeps the mate solver)

History: docs/handoff/archive/studios.md (dated entries up to 11-15 18:03). Rulings: docs/DECISIONS.md.

Updated 2026-11-22 04:58 UTC.

## In flight

Worktree /root/projects/noshape-wt/studios, branch studios-0811a. Land: `NOSHAPE_RB_HOST=vibex7
NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh`; read `git diff --stat origin/main...HEAD` first. Builds and crate
tests on vibex7 (rb-hosts 6a36d3b1f; off it 02:00-04:00 UTC, qa's quiet window: vibex8 then). Scenarios on vibex8 via
~/bin/slot (<=3 apps): build on vibex7, `scp -3 vibex7:tmp/studios/noshape-uip vibex8:tmp/studios/noshape-uip`, sync
vibex8 with `rb.sh bash -c true`, then `ssh vibex8 "bash -s" < script`. Never vibe2. Every run under `timeout`;
`touch -c` edited files before rb.sh; two rb.sh runs on one host share a target dir (one waits). Full ops e2e before a
crates/drawing land. Never `git checkout <file>` over uncommitted work.
- QUEUED 60f6ad8d (01:14 11-22; carries e41a7227: Esc rework, custom table listing, max/min edge, note colour (j),
  DRW-BORDER-DRAG, APP-VULKAN-SEGV). Still "re-checked apart from a red batch" at 04:20 (an assembly-ui test, not mine).
- COMMITTED, not landed: 8419bdc8e (live drwm-1..6 hover/select: palette tan + auto-close at 100 px, hover wins on a
  selected dimension, BOM header/rows/cell looks, title-block click keeps the view, custom-table properties keep the
  selection, 3 matrix scenarios) and 641159b57 (qa walk 9: note hover/selected look, palette arrowheads, FS stray-name
  error). x184 (vibex7 04:20): drawing-ui 163/163, FS parser 8/8, clippy clean, app built. Shots (vibex8, shots1122d):
  note matrix 32/32, edge/view 41/44 (was 20), dimension 28/31.
- UNCOMMITTED (04:5x), x185 (drawing-ui tests + clippy) / x186 (variable-studio-ui) running on vibex7:
  - FEEL-DRW-VIEW-HOVER-AFTER-PLACE: Insert view's just-placed view drawn picked (orange edges + orange frame, not
    selected: `lit_parent`, from `projection_parent_lit`); while placing projected views the placed view under the
    pointer gets the tan hover frame, edges plain (`projecting_hover` in update_hover; view_frames). Test
    a_just_placed_view_is_lit_under_the_pointer (passes).
  - FEEL-DRW-DIM-PLACE-DOTS: drawing_related push_related_highlights put snap discs on every vertex of a just-placed
    dimension; now none (a 2 point one keeps its discs through dim_kept_points). The ±0.1 launcher's delay counts as
    work still to show (`launcher_pending` in ui_busy). Test a_dimension_placed_on_an_edge_leaves_no_dots_on_its_ends.
  - dimension matrix d7 (3 states): a click / double-click on the palette's own dimension keeps the palette
    (select_click); assert in a_dimension_dragged_from_unselected_ends_unselected.
  - edge/view matrix v5 (3 states): View properties open, a selected view under the pointer keeps the selection frame
    (VIEW); assert in a_title_block_click_keeps_a_selected_view...
  - BOM-CELL-TOOLBAR (P2): a BOM cell's panel row 1 = size icons, the cell's text, Override (moved off row 2, whose
    overflow squeezed the text height to "0.‹"); anchored at the table's left for any cell (drawing_cell_panel).
  - VS-NAME-FOCUS-BOX (qa P1, workflow 15): the focused empty Name cell outlined 2 px #1651b0 (`add_name_focused`);
    test the_focused_add_row_name_is_outlined.
  - x184b (vibex8, binary of the two commits): all drw doc docs ver vs fs rel -> ~/agents/studios-keep/uip1122h, log
    x184b.log.
  NEXT: x185/x186 green -> commit, rebase, mark the walk-9 / workflow 09 / 15 tickets in TICKETS, land (no ops e2e:
  no crates/drawing change). Then a fresh app build and the 3 matrices again.
- Waiting on live: (k) exact-centre Ø8 click (REQUESTS 02:47), VIEW-FRAME-PAD rule (03:38), HISTORY-BRANCH-FILTER
  menu (04:39; qa P2 workflow 16: a funnel ▾ right of Search history once a branch exists; not built until answered).
- Left P2s: VS-CHIP-ON-CLICK, FS-DRAG-SELECT-BAND (need vendored GPUI tooltip / editor patches, 3 platform copies).
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
