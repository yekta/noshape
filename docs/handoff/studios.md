# studios: current state

Owner: studios. Areas:
- documents, tabs, start page, Preferences
- versions, branches, history, release
- Variable Studio, Feature Studio (the editor and UI; the language and host stay dx's)
- the drawing UI (from dx, 11-12)
- the assembly UI's feel (11-13; asm keeps the mate solver)

History: docs/handoff/archive/studios.md (dated entries up to 11-15 18:03). Rulings: docs/DECISIONS.md.

Updated 2026-11-21 22:56 UTC.

## In flight

Worktree /root/projects/noshape-wt/studios, branch studios-0811a. Land: `NOSHAPE_RB_HOST=vibex8
NOSHAPE_LAND_QUEUE_HOST=vibe2 tools/tiers/land.sh`; read `git diff --stat origin/main...HEAD` first. App builds for
shots on vibex7 / vibex6 (never vibe2: its slot is the land queue's), shots on vibe2 (rsync tests/ui-parity +
tools/ui-parity to vibe2:agents/studios/src first). Crate tests on vibex7 / vibex8, every run under `timeout` (lead
11-20 21:xx: a hung test held a vibex8 slot 43 min). `touch -c` edited files before rb.sh. Full ops e2e before a
drawing land. vibex7 02:00-04:00 UTC is qa's quiet window (lead 11-22): use vibex8 then. Never `git checkout <file>` over uncommitted work: WIP-commit
or save a patch first (land.sh needs a clean tree: `git diff > patch`, land, `git apply --3way`).
- LANDED 11-21: eabacbef, 420d7f79, a2074bb2 (documents page, versions page, merge strategies).
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
- LANDED cf9f4cc6 as a2a3382d (16:30): live rel-1121b (release dialog 1092 px, Compare revisions only with an earlier
  revision, `earlier_revision` / `comparable`) and search relevance (`noshape_ops::doc_library::search_order`, store
  `document_created`, `DocEntry::created`; bold whole words only).
- LANDED 6ac15022 as 610fc78d (17:42): qa's interaction run 1121-0935 (uip.log, ours in uip/*.json), my areas:
  - inspection bubble (red-orange fill) reads EDGE of the view box holding it, whatever is selected (`area_inside`);
    Esc keeps a selected custom table (as a hole table); centerline picks and a hovered dimension's text read from
    recorder 2026-11-14.1 (`^`; 11-12.1 reads neither); a picked circle lights only what the view draws of it
    (max/min on a fillet arc); os_menu's first-paint hover ignores rows scrolled out of view (Note panel lists);
    Edge to edge centerline treats a cylinder's seam as a silhouette (app + ops); an arc length's value sits on its
    dimension arc (model-data sheet.rs); run.py: CDP's Esc under a native <select> closes live's panel (ours keeps it
    one Esc longer: dialog blanked); drw-centermark-align's expect from live retake 996d6b8a (mark stays);
    ver-compare-hover-part (qa P2 UIP-COMPARE-HOVER-PART): the app reader's Compare override (ui_script.rs
    `comparing_state`) put the panel's row FEATURE over a graphics pick; now the row only when nothing is picked.
  - LANDED e591ac9a as 21da2808 (20:32; picked edges per view, Import mapping, wheel_tries, esc_kept/tab_left). COMMITTED 7038164b0 (rebased, ops e2e 277/277 on vibex8,
    not landed): Esc leaves what a tool made hovered, not selected (`update_hover` in `cancel`; `esc_kept` keeps only
    its grips for live's reader until another hover, a press, a tool or leaving the tab); a placed custom table listed;
    max/min to an edge: `MaxMinDimension.b_edge` (resolved, not saved) + sheet.rs `ext_from_edge` start that extension
    line at the edge's end (drw-max-min-fillet-arc s6); Note colour panel opens with its hex focused + selected
    (`note_color_focus`, note-format-lists s14); run.py drops Change to version… in pre-Pro takes.
  - COMMITTED 8b1d10552 on top of 7038164b0 (both unlanded): a hovered annotation reads lit after Esc only if Esc left
    it (`esc_kept`); Esc after a 2 point centerline keeps it (EDGE^); the Note color panel over its swatch (top 46),
    focus/Esc per live (j). x160/x161/x162 (vibex7): 23/26, 19/20, 147-scenario run 139/147.
  - QUEUED e41a7227 (00:31 11-22; 4 commits: the Esc rework, custom table listing, max/min edge, note colour (j),
    P0 DRW-BORDER-DRAG + TICKETS): x168 (vibex7) drawing-ui 159/159, clippy clean, 147 drw-* 140/147 (left: custom-table
    x3 = live re-takes now on main, centerline-3pt = (k), table-cell-menu s3 = live timing, 2 flaky "no recording");
    ops e2e 277/277 (x166 vibex8). Also in it: `pointer_off` (drawing_view): a rebuild under a palette/menu hovers
    nothing at the last sheet position (hcallout-palette-dual s24 / -tips s40).
  - QUEUED 60f6ad8d (01:14 11-22, #8; carries e41a7227): APP-VULKAN-SEGV + the DRW-BORDER-DRAG batch.
  - Uncommitted (01:19): live drwm-2 dimension take as scenario drw-dimension-hover-matrix (+ live/ copy); a selected
    dimension's text drag opens its palette on release (drawing_tools MoveAnnotation, test extended); hover wins on a
    selected dimension (drawing_view `hovered_dim`, test in a_selected_dimension_keeps_its_text_black...);
    drw-export-formats seeds config_files export_drawing.json (PDF, DWG 2018 = index 8: live's account remembers it);
    drw-custom-table-properties s11: a menu action cleared the selection because `dialog_or_mode_open` missed
    `custom_tables.dlg` (added; feel test a_custom_table_stays_read_selected_through_its_properties on the real
    block-drawing-table start; drawing() now applies SETUP after a START too). x174 (vibex7, 01:23) runs: all
    drawing-ui tests, clippy, app, matrix shots -> ~/agents/studios-keep/shots1122, all drw-* -> uip1122d.
    FS (live fsm-1 f4): bracket boxes at the caret with a selection too (update_bracket_box uses `cursor()`; test in
    ace_feel_outdent_...); x176 (vibex8) FS tests + clippy. VS matrix rows (vsm-1) already match.
    Shots (x174 shots1122): ours matched d1/d3/d4; s10 `{"empty": true}` wasn't a drawing target (added: right margin,
    [w - 4.6, 19] mm); with the palette open the orange end dots / diamonds now hide (push_annotation_look `editing`).
    x174 stopped 01:42 (vibex7 off 02:00-04:00 for qa's speed window: use vibex8 then): 144/146 drw run, both known
    (dimension-hover-matrix s10 = fixed above, centerline-3pt = (k)). x177/x175b/x179 stopped 02:25 (32 min
    waiting for a vibex8 build slot, both held by blend): one run instead, x180 (vibex8, 02:26: tests, clippy, app,
    shots1122c of the 3 matrices) then x181 (all drw doc docs ver vs fs rel -> uip1122g).
    After x177 started : live drwm-1 / drwm-3 takes as
    drw-edge-view-hover-matrix / drw-note-hover-matrix (+ live/ copies); BOM look per live drwm-4b (shots only): a
    hovered header lit #e2e2e2 like a row (b1), a selected table away from the pointer has no fills (b0, b5), a selected
    cell hides the table's frame and grips until hovered (b3, b4) (`hovered_table`, lit_tables, table_grip_overlay;
    test a_hovered_bom_header_shades_like_a_row). FS: Tab closes the completion list and inserts whitespace, Esc
    keeps the text (test in the_completion_list_shows_230_ms_after_the_key; x178). Live drwm-6 (answer to 01:51): a selected
    table's frame is tan (ours already); cells move with each click (ours); an empty click keeps a selected cell (now:
    table_cell_click; assert in a_click_on_a_bom_cell_opens...).
    x180 shots1122c: note matrix 32/32; dimension matrix 28/31 (s19-27 palette fields, old); edge/view 20/44: a click in
    the title block kept the view in live (now: select_click, title_block_rect) and live's reader says EDGE for a
    selected view under the pointer (tan frame; drawing_ui_state); test a_title_block_click_keeps_a_selected_view...
    x182 (vibex8): drawing-ui tests + clippy with all of it; x181 runs on x180's binary (before the title block fix). Live drwm-5 (answer to 01:24): the palette's dimension tan (hover look, no
    grips / diamonds) wherever the pointer is; more than PALETTE_REACH_PX (100) off the palette it closes by itself,
    keeping changes, dimension unselected (drawing_tools on_mouse_move, `palette_gap`; test extended in
    a_dimension_dragged_from_unselected_ends_unselected). Live's blue dots left after that = stale paint (its reader reads
    no selection), not drawn. All of it is in x180/x181.
  - P0 APP-VULKAN-SEGV fixed (committed after e41a7227, landing 01:1x 11-22): ui-parity "no recording" = app segfault
    at start in libvulkan (kernel log: 17 vibex7 / 96 vibex8 since 11-18). gdb (installed on vibex7 with sudo): GPUI
    create_pipelines (set_object_name) on main vs our `shared_device` second instance's vkEnumeratePhysicalDevices
    dlclose on noshape-gpu-init. Fix: `renderer::share_adapter` (render) from app-core gpu.rs spawn_worker for a
    Vulkan GPUI ctx read back. x172 (vibex7): clippy clean, gdb stress 0 segv / 42 starts (old 4 / 17); scenarios 5/7
    (custom-table-properties s11 selection 0 vs 1; export-formats s4 DWG version "2000" vs live "2018").
  - table-cell-menu s3 is live's own timing. (k) centerline-3pt s4: live (behaviours "studios (k)") never creates the
    circle: its Ø8 centre clicks (recorder px ~4 px off centre) don't take. REQUESTS 02:47 asks for an exact-centre click.
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
