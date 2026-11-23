# studios: current state

Owner: studios. Areas:
- documents, tabs, start page, Preferences
- versions, branches, history, release
- Variable Studio, Feature Studio (the editor and UI; the language and host stay dx's)
- the drawing UI (from dx, 11-12)
- the assembly UI's feel (11-13; asm keeps the mate solver)

History: docs/handoff/archive/studios.md (dated entries up to 11-15 18:03). Rulings: docs/DECISIONS.md.

Updated 2026-11-23 03:58 UTC.

## In flight

Worktree /root/projects/noshape-wt/studios, branch studios-0811a. Land: `NOSHAPE_RB_HOST=vibex7
NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh`; read `git diff --stat origin/main...HEAD` first. Builds and crate
tests on vibex7 (off it 02:00-04:00 UTC: vibex8 then). Scenarios on vibex8 via ~/bin/slot (<=3 apps): build on vibex7,
`scp -3 vibex7:tmp/studios/noshape-uip vibex8:tmp/studios/noshape-uip`, sync vibex8 with `rb.sh bash -c true`, then
`ssh vibex8 "bash -s" < script` (x184b.sh all, x184d.sh drw + hover matrices). Never vibe2. Every run under `timeout`;
`touch -c` edited files before rb.sh. Full ops e2e before a crates/drawing or store land. Read logs with tail/grep only.
- STATE 07:46: NOT yet on main (queued: 7a12ac31 batch c5264d17d..aa2bc79cc, e6667368 = 3f9e8e994 drw-dims-1123,
  c1a6b8de3, d607d23ad TICKETS). drw-dims-1123 / DRW-FLOW-* all done. Told dx at 07:2x raise_noted = Info, corrected:
  only once landed.
- Lead 07:2x: skasm did SKT-MENU-HEAD / PATTERN-DESELECT / MIRROR-AGAIN / TANGENT-ARC-DOT (mine reverted); studios
  keeps SKT-USE-AGAIN: ops sketch_use external_entity reuses an external only for constraint/dimension picks
  (construction) or a reference_only one; a 2nd Use of an edge makes a new copy (DECISIONS 11-23). e2e
  sketch_use_intersection_and_model_references asserts 4 new lines on a 2nd top-face Use.
- 07:5x: ops e2e (x278) caught 2 reds in queued c1a6b8de/3f9e8e994: Diameter hover refused a
  cylinder silhouette (now lit: pick.rs silhouette_line_of + smart.rs silhouette_axis_offset), tapped hole table cell
  dropped ISO's thread line (now ANSI only, drawing test covers both). x281 green: ops e2e 300, scene, clippy. 28462984f-ish
  tip (with USE-AGAIN) queued on top of the stack.
- 07:58 (lead 07:5x): centerline on an exact hole centre = live's failure, ours takes it (behaviours, parity A52, the
  take's about). The 3 P1s (MOVED-VIEW-PROPS, DETAIL-PREVIEW-LOOK, SECTION-PREVIEW-IN-PARENT) were done in the queued
  stack. live drw-hover-matrix-1123 (r23n) paused after its setup probe (no hover samples); audited drw-hover-1122's
  matrix instead: FEEL-DRW-NOTE-HOURGLASS done (x282 drawing-ui 180, clippy), FEEL-DRW-DIM-FRAME-LINE open (REQUESTS
  studios 11-23 07:58). Rest of the 1122 matrix already in feel tests (reach, colours, table, centerline).
- 08:1x (lead: my open P2s): all on main's list are fixed in the queued stack (qa's "STILL OPEN" 02:5x reads predate
  it). Leftovers done: HIST-LEGEND pin glyphs + Change glyph, HIST-GRAPH version dot r 5.5 (VERSION_DOT_R).
- FEEL-VS-CHIP-ON-CLICK ticked (0cfd58165 on main). live import-into-doc-1123: ours matches; rest is dx's.
- Mac check: watcher bd5jb96zs polls v0.4.68's mac asset (not published at 06:28); then bin/mac-copycheck.sh 0.4.68
  (idle >= 600 s only), report <= 3 lines to the lead. Background runs/watchers die at session end: re-arm.
- Shortcuts (live shortcuts-1123, lead's ask 04:2x): Drawing context: Mac ⌃Z/⌃Y NoAction, ⌘Y redo; elsewhere
  Ctrl+Shift+Z redo (app-core actions.rs, test drawing_key_tests). FS: Commit/CommitAll ⌘ only on a Mac; ⇧⌘F search
  also in the editor (input's own was Replace); Mac ⌃B/⌃P/⌃H/⌃D/⌃O (SplitLine, fs_lines::split_line). FS Mac keys
  untested on Linux (cfg!(macos) in bind_keys: refactor to fs_bindings(mac) + test). VS: a typed key opens the value
  list at once (value_completion; test a_letter_in_a_focused_value_cell_opens_the_list). Run x264 (vibex7).
- Lead's extra: after v0.4.68 (05:00 release) is published, on the Mac (idle >= 10 min only): Finder-copy a scratch
  .noshape in ~/noshape-host/studios-keep, rename the copy, open it with the release app: new name, not a copy.
  Report <= 3 lines.
- USER P0 (lead 11-22 23:45, macOS ~/Documents/noshape): DOC-COPY-IDENTITY + DOC-SIDE-FILES:
  - No per-document id exists: windows (OpenDocs, canonical path), recents, last-viewed are keyed by path, so a Finder
    copy is already its own document; only its name was the original's. store `Document::meta().name` =
    `shown_name(path, stored, name_file)`: the stored name while the file is the one it was set in (`set_name` records
    meta `name_file`; the navbar rename keeps the file) or named after it ("Name" / "Name 2"), else the file stem.
    Legacy docs renamed in the navbar before this (no name_file) show their file name.
  - Quit: `DocModel::close_on_quit` (files::build_workspace, versions_page, and on the first deferred edit) flushes
    and `store.suspend()`s, since quit ends via kernel exit_now without dropping documents -> no -wal/-shm left.
  - `noshape_store::fold_log(path)`: a closed doc's leftover -wal/-shm folded in via Document::open (foreign logs
    moved aside); skipped for docs this process has open (WRITERS registry). The documents page calls it on read.
  - Finder copy of an open doc: the idle checkpointer (1 s) keeps the .noshape whole; a copy within 1 s of an edit
    misses it (store test an_idle_open_document_is_whole_in_its_file). Leftover logs from old quits are now folded.
  - Tests: store a_document_copied_or_renamed_outside_is_named_after_its_file, a_leftover_log_is_folded_into_its_document;
    app-core start_library a_finder_copy_is_named_after_its_file_and_leftover_side_files_go; app quit_tests (2).
  - File association / open-file events are dx's (open_requests.rs); dx keys "already open" on canonical(path) and its
    second start never opens the store. Not yet verified on the Mac's release build.


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
