# studios: current state

Owner: studios. Areas:
- documents, tabs, start page, Preferences
- versions, branches, history, release
- Variable Studio, Feature Studio (the editor and UI; the language and host stay dx's)
- the drawing UI (from dx, 11-12)
- the assembly UI's feel (11-13; asm keeps the mate solver)

History: docs/handoff/archive/studios.md (dated entries up to 11-15 18:03). Rulings: docs/DECISIONS.md.

Updated 2026-11-21 17:04 UTC.

## In flight

Worktree /root/projects/noshape-wt/studios, branch studios-0811a. Land: `NOSHAPE_RB_HOST=vibex8
NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh`; read `git diff --stat origin/main...HEAD` first. App builds for
shots on vibex7 / vibex6 (never vibe2: its slot is the land queue's), shots on vibe2 (rsync tests/ui-parity +
tools/ui-parity to vibe2:agents/studios/src first). Crate tests on vibex7 / vibex8, every run under `timeout` (lead
11-20 21:xx: a hung test held a vibex8 slot 43 min). `touch -c` edited files before rb.sh. Full ops e2e before a
drawing land. vibex8 02:00-04:00 is qa's quiet window. Never `git checkout <file>` over uncommitted work: WIP-commit
or save a patch first (land.sh needs a clean tree: `git diff > patch`, land, `git apply --3way`).
- LANDED eabacbef (09:53, as a2fe2c5c): the audit's first fixes. LANDED 420d7f79 (10:12, as f33264f9): the documents
  page's Versions and history page (live 2435: `HistoryPanel::page`, start-ui `StartView::page`, hook
  `start_hooks::versions_page`, app `files::versions_page`), the deleted-workspace bubble 30 s (`NoticeLink::stays`),
  documents_list Created by me / `search` / `documents_sort`.
- LANDED a2074bb2 as 301b1895 (11:19): history_merge strategies + merge_plan; history_create_version at/publish.
- LANDED 04361993 as 3f70f4f2 (16:08): speed pass + live docver-1121 rows + drawing takes + Start version (x129/x130 on vibex7):
  - Speed pass: NO gap >=100 ms and >=2x live. 1000-part drawing: open 19-31 ms, all views 2.7-4.9 s (live 3.7-4.0 s a
    view), Update click 62-85 ms (two sheet assemblies ~20 ms each on the UI thread; bench now holds it to <100 ms, not a
    frame; possible later cleanup: one assembly per click), views remade 47-83 ms (live 6.4 s); FS marker ~0.8 s (live
    0.86-0.95); VS steps 18-20 ms; history row click 17 ms. (Benches using run_until_parked count background work: don't.)
  - docver-1121: Create version 255 limit; search empty keeps results, ⊗ clears + blurs (vendor Input `clear_blurs`);
    State column; Delete/Backspace -> Send to trash (`TrashSelected`); rename while open (hook
    `start_hooks::rename_open_document` -> app `files::rename_open_document`: store name, file kept); row icons hover only;
    version bar (app-core version_bar.rs) in Part Studio / Assembly / Drawing; the assembly redraws on Reloaded (its
    cached toolbar showed the bar late); versions page ••• on hover; release BlankRevision etc.
  - Start is a version (live 13:11): store `start_version` / `start_name` / `update_start_version` (meta start_name,
    start_description); row, view-only bar, version lists read it; menu Properties… (`Item::Start`) + Publish
    FeatureScript… (`versions::publish_target`, key `published_featurescript:start`); ops version_properties / publish
    take it by name; "Document must contain a PDF" (live words, 8 tsv).
  - Drawing takes (8dc787fa + live 13:08 answers): no menu while Edit section is open; no Resize... for several cells;
    Opposite direction swaps its arrow (icon section-flip-back.svg), no tint; Esc in Edit section stays open after a
    checkbox click (`check_focused`, panel_focus; test `edit_sections_esc_keeps_the_panel_while_a_ticked_checkbox_has_the_focus`).
  - Alphabetical past Y (live alpha-1121): model-core `ReleaseLog::settle_revision`; the dialog settles a revision on
    Blur (`revision_event`, `Row::settled`, the yellow `Issue::Notice`), `items()` takes the proposal; no upper-casing;
    ops release_create returns `revision_notes`. Tests `a_revision_it_cant_take_becomes_the_proposed_one_with_a_note`,
    e2e release_parts_and_revision_history (A -> C with the note).
  - Compare revisions (live rel-compare-*.png): review rows' button between Edit properties and Export… (`toggle_compare`,
    `render_compare`, `CompareRow`, `compare_layout`); release_thumbs::compare renders A (the previous revision's
    package version) and B (the package's version, else the workspace), plain and flat #acdbf5 / #e67d5e for a changed
    part. Assembly / drawing rows show the same picture both ways (asked). Test
    `compare_revisions_opens_revision_a_and_b_under_the_row`.
  - Live revtools-1121: top-level assembly row Where used… (`AssemblyViewEvent::WhereUsedAssembly` -> the rail's panel)
    and, with workflows, Revision history… (hook `open_tab_revision_history`); Revision tools' Object type Part ·
    Assembly · Drawing · Blob, empty result shows nothing. "Revision table properties..." kept (live's take reads ASCII
    dots).
  - Assembly "Viewing V1" bar: Reloaded swaps in a new toolbar slice (`toolbar_slice`): a cached slice reused since
    its last paint takes no notify (GPUI tracks only entities drawn in the last frame). ui-test-support now puts each
    test binary's settings under the run's NOSHAPE_CONFIG_DIR (release-ui's managed workflows leaked into
    assembly-ui's menus in one rb.sh run). Release dialog: a revision it takes clears the last note.
- LANDED cf9f4cc6 as a2a3382d (16:30): live rel-1121b (release dialog 1092 px, Compare revisions only with an earlier
  revision, `earlier_revision` / `comparable`) and search relevance (`noshape_ops::doc_library::search_order`, store
  `document_created`, `DocEntry::created`; bold whole words only).
- NEXT LAND (x142 / x143 on vibex8): qa's interaction run 1121-0935 (uip.log, ours in uip/*.json), my areas:
  - inspection bubble (red-orange fill) reads EDGE of the view box holding it, whatever is selected (`area_inside`);
    Esc keeps a selected custom table (as a hole table); centerline picks and a hovered dimension's text read from
    recorder 2026-11-14.1 (`^`; 11-12.1 reads neither); a picked circle lights only what the view draws of it
    (max/min on a fillet arc); os_menu's first-paint hover ignores rows scrolled out of view (Note panel lists);
    Edge to edge centerline treats a cylinder's seam as a silhouette (app + ops); an arc length's value sits on its
    dimension arc (model-data sheet.rs); run.py: CDP's Esc under a native <select> closes live's panel (ours keeps it
    one Esc longer: dialog blanked); drw-centermark-align's expect from live retake 996d6b8a (mark stays);
    ver-compare-hover-part (qa P2 UIP-COMPARE-HOVER-PART): the app reader's Compare override (ui_script.rs
    `comparing_state`) put the panel's row FEATURE over a graphics pick; now the row only when nothing is picked.
  - Left: drw-styles-radial / drw-update-deleted-feature wait on REQUESTS 17:02 (dimension selected after Esc / tab
    switch?); drw-centerline-circle-3-point-hole-centres is live's reader gap (asked for a re-take); 7 "not run"
    (shard drop) to rerun on vibe2 with the r41 build.
- REQUESTS for live (11-21): 08:58 the drawing matrix's 40 most-used shown rows (six takes); 14:33 all answered.
- Audit: #7–#10 done. interaction-feel's FS F2/F3 rows marked = (fixed c249e0ed9, qa verified).
- Live fc-1121 drawing feel (behaviours "studios fc-1121"): (1) a dimension being placed shows the line to another's
  x / y without snapping (still held on a + mark, live2 rad-note); (3) caret at the click (`note_caret_at`), a
  double-click in the editor selects all; (5) a dragged box draws its picks selected (`box_preview`); (2), (4), (6)
  already as live (tests for (6)). Live's leftover grips after Esc and an empty window box kept after release are
  stale paint, not copied.
- Drawing matrix: options compared in live's order (drawing_matrix.py `live_option_lists` / `ours_sequences`):
  Drawing properties 101 → 22 shown, matrix 04:45: 1344 = / 456 shown of 1842; View simplification was None, Auto, …: now live's None, Absolute, Ratio to
  studio, Ratio to part, Auto (test `the_view_simplification_list_is_in_lives_order`). REQUESTS fc-1121 (6) added:
  a table hover-lit after Esc under a resting pointer (split s14-15).
- Asm feel rows: none left untested in interaction-feel's asm tables.

## Next, in order
1. Drawing matrix (11-21 05:21): 1419 = / 381 shown / 0 differs / 0 present-only / 0 missing of 1842 (05:21: the Note
   panel's nested reading counted; its Border size gained Tight fit and 5 Characters). Every panel
   was shot in pd/b1-b35 (tables, inspection / revision, section / detail included). The 456 shown are mostly
   dropdown options our takes don't list in order (fonts, balloon shapes, standards: Drawing properties 101, Create Drawing 57, Title block 54,
   Hole/thread callout 53, Note 52, Hole table 45, Styles 43): only a take opening each list would make them =.
   Regenerate: `drawing_matrix.py --run /root/studios-scratch/uip-merged --ours-menus /root/studios-scratch/menudump.json`
   (copy new take outputs into uip-merged first).
2. Speed pass 11-19 done (no gap >=100 ms and >=2x); P1 parity rows for drawings / Feature Studio: none open.

## Feel rows to take (gaps only; check ours against each, fix with a feel test)
- live2 drawing tables: done: the hover / select look (ink black, rows #f1f1f1, header #ebebeb, hovered row / cell
  #e2e2e2, tan frame; 37ecb60c); the BOM row's part tan in the views and its callouts blue were ours already. Local:
  one click on a BOM cell selects it and opens the Cell panel (sizes + formats + a greyed Override), a double-click the
  same (live cferr-bomclk-1118 (b)); Split BOM table below puts the second part 3 mm under the first, same x and width.
  A placed BOM stays selected (live2 bomsel-1118; the recorder can't read tables, and reads no EDGE under a just-placed
  one, so ours doesn't count it until the next press, `placed_table`): local. the recorder's TABLE kind is read (ours `TABLE` since 2026-11-18.1, drawing_ui_state.rs). The "︿" is the isometric part's corner, not a control
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
- P0 FEEL-DRW-WHEEL-ZOOM-SLOW waits on qa's quiet re-time (the bench's zoom frame is under a frame; qa's hover in the
  same app was as slow: the host).
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
