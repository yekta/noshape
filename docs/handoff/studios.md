# studios: current state

Owner: studios. Areas:
- documents, tabs, start page, Preferences
- versions, branches, history, release
- Variable Studio, Feature Studio (the editor and UI; the language and host stay dx's)
- the drawing UI (from dx, 11-12)
- the assembly UI's feel (11-13; asm keeps the mate solver)

History: docs/handoff/archive/studios.md (dated entries up to 11-15 18:03). Rulings: docs/DECISIONS.md.

Updated 2026-11-20 10:49 UTC.

## In flight

Worktree /root/projects/noshape-wt/studios, branch studios-0811a. Land: `NOSHAPE_LAND_QUEUE_HOST=vibe2
tools/tiers/land.sh` (`--no-precheck` only when the code was clippy-checked unchanged); read `git diff --stat
origin/main` first. App builds for shots on vibex7 / vibex6, never vibe2 (its one slot is the land queue's, lead 11-20 08:50; `scp -3 vibex7:tmp/studios/noshape vibe2:tmp/studios/noshape`, shots on vibe2); crate tests on the Mac or the lowest 1-min load of vibex7 / vibex8 (`NOSHAPE_RB_HOST=… tools/tiers/rb.sh`;
read the load with `awk`, `cut` is aliased on the hosts). rb.sh skips a file edited during a running sync: `touch` edited files before
the next run. Run the full ops e2e before any drawing land. vibex8 is qa's quiet speed window 02:00-04:00 (lead 11-20):
nothing of ours there then; shots elsewhere with /root/studios-scratch/bin/pdshots-host.sh (PD_HOST=vibe2 PD_DISP=395: first free display, never skasm's :391-399). A few app runs don't take ~/bin/slot (DECISIONS 11-15).
- Landed 09:57: 782f4c47 as f29e6eb2 (the Windows CI wheel-notch fix and the 18 commits with it).
- Queued 10:05 as fc8e6c91 (11 commits): a8b7e4c61 / 2d0b3a87e matrix doc (b21 merged; live's visually-hidden
  labels dropped); 856e293d8 BOM / cut list / custom cell menu = live's bom-table-cell (Size ▸, Resize… | Copy), the
  MENUDUMP test lists BOM header / row / cell, a revision's row, the sheet tree's reference row; 0cf22ebd0 Datum target
  X / Y flips = flip-direction icon, Length / Angle dropdown 13.33 px #555, distance text 12 px; bc65e1c09 Origin field
  stays active once picked (live s5; test); 5b8590995 Targets box ≤400 px with a scrollbar, #1b5faa borders. Mac:
  drawing-ui 126, clippy clean.
- Local after fc8e6c91 (to land once it does): 8305af19d coverage MENUS "drawing:Resize..." (856e293d8 renamed the cell
  menu's item; without it ops coverage::every_menu_item_is_listed fails, so fc8e6c91 may fail on it: fix = this);
  404ed8a14 DRW-DATUM-TARGET-OP: op `drawing_datum_target` (ops drawing_tools.rs): area / size / area precision,
  style, origin, Length (x_edge / y_edge, distance_x / _y, flip_x / _y) or Angle (reference_edge, diameter_distance,
  angle, flip, rotate, instances, radial_lines), precision_1 / _2; e2e
  drawing_datum_target_takes_area_style_origin_and_distances; shared with the dialog via scene opts.rs
  `datum_target_at`, `datum_target_distances`, SheetPick `datum_target_custom`, `straight_edge_dir_at`. Mac: ops all
  (450 + coverage 11) ok, drawing-ui 126, clippy clean; AGENT_API regenerated.
- Local, uncommitted (10:50): live's panel-lists s21-24 (uip 10:30) + a34-1699 10-15: an empty distance's Precision
  ▾ opens no list (test an_empty_target_distances_precision_opens_no_list); Angle's Instance count an underlined field
  with its Opposite direction button (flip-circular-opposite; on with an origin and ≥2 instances; new
  DatumTargetOpts::instances_reverse, op `opposite_direction`, drawing test); Flip / Rotate icon buttons
  (flip-direction / flip-circular-opposite), disabled (grey) with no origin. Feature Studio bracket box on the glyph
  cell (vendor gpui-base inset_frame_corners 0.5 px; qa measured ours 11-12x17 vs live 8x16). TICKETS: FS-WHEEL /
  BRACKET / OUTDENT / TRIPLE rows closed as qa's verified FEEL-FS-* duplicates. Needs: Mac run dto3 green, AGENT_API
  scp back (mac:noshape-host/agents/studios/src/docs/AGENT_API.md), full ops e2e (model-data changed), land.
- revision-table-delete retake (REQUESTS 09:55 item 2): not in at 10:48.
- Matrix 09:30: 1232 = + 558 shown of 1841, 0 ≠, 9 present-only, all conditional rows that wait on live's takes
  (REQUESTS "studios (2026-11-20 08:08 UTC)", not captured at 09:30): Section Depth / Offset distance, Datum "This
  field cannot be empty" (live's hidden Instance count row), Custom table Cell / Split custom table below, Inspection
  Diameter dimension, View properties Multiple values, Sheet properties A4 Portrait, Styles Show dimension line.
- b22 DONE 09:40 (vibex6 build): hide-dim 12/12, s5 now as live (Origin active, flips, Length, distance text);
  panel-lists 25/27: s22-23 our click at (285, 447) opened Diameter distance's ▾ list; live's Targets box is 400 px
  with a 15 px scrollbar inside (▾ 15 px further left), fixed 5b8590995 (+ #1b5faa borders). Local after 782f4c47 also:
  5b8590995, and the commits above.
- b23 DONE 09:52 (vibex7 build): hide-dim 12/12, inspection-table-iso-scrolled 10/10, panel-lists 25/27 (still the
  (285, 447) ▾: ours' Targets box in Angle mode is 370 px, under live's 400, so no scrollbar), revision-table-delete
  7/10 (live's s7-s10 "EDGE Top" is the deleted table's stale paint). Both asked of live: REQUESTS "studios
  (2026-11-20 09:55 UTC)".
- qa 11-20: 9 of 10 to-verify rows verified (6c3c0a8fe); FEEL-DRW-WHEEL-ZOOM-SLOW stays open (re-timed 02:00-04:00).

## Next, in order
1. Lead 11-20: the drawing matrix's items not yet matching, by use (dialogs, menus, placement, rarer panels): check
   each against our build (screenshots: /root/studios-scratch/bin/pdshots.sh <tag> <takes> on vibex8 :394 2560x1440,
   NOSHAPE_UI_SHOTS per step; compare with live/drawing/*.png and dom dumps), fix real differences in batches, report
   the count per land. Regenerate: `drawing_matrix.py --run <uip dir> --ours-menus <MENUDUMP>` (merged dir
   /root/studios-scratch/uip-merged). Left at 11-20 00:23: 174 present-only (Create Drawing custom template tab,
   Move to sheet / Replicate page chrome, small rests). b4 compared (above); next: rescreenshot b4 with the new
   build, then the remaining panels (inspection / revision tables, BOM properties, section / detail view panels).
2. Live cfg-choosers-1120 left open: whether a custom table's cells take the picked configuration (live's studio
   has Small = Large). Insert view takes the default configuration, as live's (iv-10: "Size: Small").
3. Speed pass 11-19 done (no gap >=100 ms and >=2x); P1 parity rows for drawings / Feature Studio: none open.

## Feel rows to take (gaps only; check ours against each, fix with a feel test)
- live2 drawing tables: done: the hover / select look (ink black, rows #f1f1f1, header #ebebeb, hovered row / cell
  #e2e2e2, tan frame; 37ecb60c); the BOM row's part tan in the views and its callouts blue were ours already. Local:
  one click on a BOM cell selects it and opens the Cell panel (sizes + formats + a greyed Override), a double-click the
  same (live cferr-bomclk-1118 (b)); Split BOM table below puts the second part 3 mm under the first, same x and width.
  A placed BOM stays selected (live2 bomsel-1118; the recorder can't read tables, and reads no EDGE under a just-placed
  one, so ours doesn't count it until the next press, `placed_table`): local. live2 is adding a TABLE kind to the
  recorder and retaking the seven drw-bom-* + drw-note-editor (type → ✓): then drop `placed_table`'s reader rule and
  report the table as live's new kind. The "︿" is the isometric part's corner, not a control
  (live2). Update from
  this workspace enabling ~3 s late is live's server latency, not a target. Custom feature ✓ with a field error:
  live (a) confirms ours (pale, Enter no-op; no field named: ✓ commits).
- live2 drawing tables (behaviours "studios (lead 11-18 ~03:36, feel): drawing tables", live/studios/tables-1118/):
  (1) hovering a BOM row shades it darker grey and draws its part tan in every view; a selected table has a small "︿"
  chevron centred under it. (2) the BOM tool places on one click (no ✓), panel closes, table stays selected. (3)
  double-click a BOM cell: blue cell outline + floating Cell panel (Override), focus on its Font select, no in-place
  typing. (4) Split below (row 1): header + row 1 stay; a second table, header repeated + row 2, ~12 px below, same x
  and width. (5) the assembly's BOM changed: the sheet waits; ~3.1 s later "Update from this workspace (ctrl+q)"
  turns enabled (yellow); a click updates (qty 1 -> 2), keeps the split, disables it again; no toast.
- live VS / FS (live/studios/vs-fs-feel-1117/): VS add-row focus to the next Name, duplicate -> "w_0"; FS
  autocomplete Enter inserts, Tab closes + inserts a tab, Esc keeps text; FS errors while typing (<0.5 s) red x at
  gutter line 1 + squiggle, Ctrl/Cmd+S commits, Commit pales, a fixed error greys ~150 ms, gone ~500 ms. VSFEEL bubble:
  centred over the cell (done 11-18).
- qa HISTFEEL "sticky hover" (mm.png): the tint there is #b2ddf6, the viewed change's selection (live selects it too),
  not the #def1fb hover: no change.
- live documents / Import / Derived + Insert (live/studios/docs-import-feel-1118/): search on Enter only; sort A-Z,
  Z-A, default; row hover icons; double-click opens; inline Rename; Delete -> "Send to trash" (Enter nothing, Esc
  closes) + toast with Restore; New document Enter creates. Import: "CAD Imports" tab ~1.8 s, the Part Studio tab
  ~6.9 s, active tab unchanged, blue bar, toast. Derived / Insert search ~350 ms after the last key (live's server debounce, not a
  target; ours filters at once), Enter nothing; Derived Enter / Esc 1 close the panel, 2 commit / cancel (done 11-18).
- live2 document level (live/feel/doc-level-1117/): tab switch order (URL, old list / canvas gone at once, toolbar,
  list, 3D last; no spinner); undo / redo as ours; regen spinner is psui's.

## Lead's queue (11-17, restated after compaction)
1. Land what is green. 2. User P0: hover / select parity in Assemblies (instances, mate connectors, mates list <->
viewport, parts vs connectors) and Drawings (views, edges, dimensions, annotations: hover, click, box, drag), each
against live with a feel scenario; share psui's pick code (`Viewport::pick_priority`, `AsmPickMap::priority_fn`,
`PickKind::priority`; psui owns viewport.rs' hover path). 3. Versions / branches / release queue.
- Drawing feel rows all have a GPUI test now (feel.rs, 11) except the double-click palette (its own test).
- Assembly already as live (checked 11-17): box select rule, glyph click selects the mate (related rows, mated faces
  amber, dashed connector line), graphics hover lights no list row, list hover outlines the parts.

## Agents
Ids live only in /root/lead-scratch/roster.md (lead 11-19): read it before every message_agent; never copy ids
into notes.

## Waiting on others
- live: REQUESTS 09:55 (panel-lists s20-24 shots; revision-table-delete ending on a repaint) and 08:08 (9 rows).
- Left to others under BRAND-IN-UI-TEXT: Category values "Onshape Part"… and "Import to Onshape" (dx), "Onshape
  default tolerances" (psui), "Onshape images" / "Onshape form library" (skasm).

## Open (no owner action possible yet)
- live's drw-hole-callout-palette retake (23:03, q-hcp-shots/record.json) has the same states as the take: kept;
  the take matches.
- Assembly "?" rows: a list row's tooltip (live2 ~70 ms), a selected instance's outline through other parts in
  explode mode, a Revolute-constrained drag's grab point.
- Release gap list: all done (Release all configurations…, the outdated associated drawing: 11-18).

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
