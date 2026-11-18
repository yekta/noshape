# studios: current state

Owner: studios. Areas:
- documents, tabs, start page, Preferences
- versions, branches, history, release
- Variable Studio, Feature Studio (the editor and UI; the language and host stay dx's)
- the drawing UI (from dx, 11-12)
- the assembly UI's feel (11-13; asm keeps the mate solver)

History: docs/handoff/archive/studios.md (dated entries up to 11-15 18:03). Rulings: docs/DECISIONS.md.

Updated 2026-11-18 10:52 UTC. Restarted 11-17 09:20 after the lead's pause; compacted ~14:45, ~18:15, ~23:30, ~01:40.

## In flight

Worktree /root/projects/noshape-wt/studios, branch studios-0811a. Land: `NOSHAPE_LAND_QUEUE_HOST=vibe2
tools/tiers/land.sh --no-precheck`; builds `NOSHAPE_RB_HOST=vibex6 tools/tiers/rb.sh`; drawing runs vibex8.
- Landed 11-18: 882a3ecd, 781eac7c, 37ecb60c, d2eb5c15, b5c4ccc4 (duplicate workspace names, BOM cell panel, BOM
  split), 4beed1be (ISO value on the pointer, placed BOM selected, Reset ordinate, section Exclude checkbox, linestyle).
- Landed dfe045ad 09:11: Variable Studio invalid name pink 526 ms after Enter (live fsvs-feel-1118); Edit section…
  Exclude from cut (op drawing_edit_section + e2e, drawing_get `exclude_from_cut`); section / detail menus without
  Explode/Position, Display state; Measure button bottom right; Sheets tab tooltip; Replicate region reader; panel
  readers (Break view, Sheet properties, View properties); a selected table read as TABLE for recorders ≥ 2026-11-18.1
  (when live2's BOM retakes land: check TABLE's order beside EDGE Isometric, drop `placed_table`'s reader rule if
  they read TABLE from the placing click).
- Landed 49cfe99b 09:12: the Release dialog ignores Esc (real-app check on vibex6 Xvfb: xdotool "abc" Return "xyz"
  lands as two lines, Esc leaves it open; the unit test's dropped letters are the harness), Feature Studio error /
  completion timings.
- Queued (10:2x): Preferences (live 11-18): shortcut "Create release" (Unassigned; a key opens the active tab's
  Release…), Length Display decimals per unit (list to the last decimal ≥ 1e-8 m: mm 0.12345, in 0.123456; Workspace
  units too), the Versions flyout's open state carries to the next document (thread-local flag); live2's BOM retakes:
  the BOM panel read as fields + options, a placed table reads TABLE + EDGE <view> (recorder ≥ 2026-11-18.1) until
  right-clicked; Add to ordinate dimension group reads nothing until its first click; tooltips: Adjust linestyle's on
  the Edges label + field, Select a DWT through ours (the kit's Button tooltip isn't read); Sheet properties / Adjust
  linestyle selects marked `dropdown:<value>`; runner: Graebert `*_Menu` lists' hover not compared (never read), an
  unnamed menu's "" separators, "start X asm Copy n" field values. run12: BOM / ordinate / linestyle / gtol 12/13;
  run10: rel, ver, prefs, docs-pref all match.
- Queued (10:4x): Sheet properties › Size › Custom read as live (Zones, Horizontal / Vertical zones, Include border),
  zones shown only with the border, Custom starts at 4 x 4 zones (live A: template 2 x 2; other sizes asked live2);
  op drawing_sheet the same, e2e. custom-zones 12/12.
- Local, to land: VS value tooltip centred over the cell (qa VSFEEL, test); Derived's Select Part Studio panel: the
  first Esc / Enter close only the panel (live docs-import-feel-1118; dialogs matrix row).
- Open: drw-bom-table-properties s7 (Esc in BOM table properties: today's retake deselects, the old take kept it;
  asked live2 10:24); drw-hatch-panel s31 (Pattern list covers its field; asked live2 09:23).
- Checked against live 08:4x, already as live: documents search on Enter only; version row menu (View in repair,
  Restore to Main…, Releases only with candidates; Copy link web-only); Restore strategy dialog; Create version (V<n>
  focused, Enter creates, Esc closes); Branch (B<n> selected, Enter creates and opens).
- Drawing items left: section / detail label position (live's distance below the view centre differs per view:
  11.7 / 10.1, 22.4 / 20.1, 19.6 / 20.1, detail 50.0 / 47.6; DXFs asked of live2, REQUESTS 09:03); toolbar help popovers
  (Update, Projected view read "title / title description" in live; asked in REQUESTS 08:56);
  drw-gtol-type-list s5 / drw-hatch-panel s29 (list highlight). Not targets: replicate-region-place s21 (live's
  toast comes ~9.75 s after ✓, server time; ours at once, gone by the 12 s read), drw-revision-table-delete s8,
  drw-inspection-table-iso-scrolled s2 (reader count), drw-note-editor (retake asked).
- Full drawing run drw1118b on vibex8 (~/agents/studios-keep/drw1118b): 317/341; its fixes landed (d2eb5c15,
  b5c4ccc4, 4beed1be). Next full run on main after the local batch.

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
- live near-snap retake (bm 4092, live/studios/near-snap-1118/): nearer vertex wins at the midpoint, no stickiness,
  reach 6.8 px: ours the same (row stays "=").
- live drawing constants + number formats (behaviours "… drawing pick/snap constants", "server-side number formats by
  experiment" §3; live/psui/numfmt-1118/). Done locally: tools pick within PICKBOX 7 px (hover_at, hovered_edge_sel,
  centermark_point, section edge), grips GRIPSIZE 6 px (detail circle resize, aux indicator); fractional inches round
  to the precision (digits = power of two, drawing 0.123 = 1/8": "2 3/4"), feet and inches `1'-2 1/4"`, the fractional
  precision list 0" … 1/256", an explicit unit's button bare ("ft in"). Left: the fraction drawn stacked (ours
  inline); jog / leader / hole-table / attach picks still 5 px (not named by live: ask if they are getIntersected too);
  24 h time H:mm done (history rows keep the time on older days).
- live2 workspaces (behaviours "studios (lead 11-18 ~02:23, feel): workspaces", live/studios/ws-1118/). Done locally:
  Delete on every non-default workspace row (current too, none on the default), deleting the one you're in lands
  viewing its branched-from version ("…and navigate you to V1."), notice "Deleted workspace B2." + Restore, ops
  history_delete_branch / history_restore_branch; Branch dialog's "Name is required and cannot exceed 256
  characters." with Create off, default name from the live workspaces; a Merge deletes what the source deleted even if
  the target edited it (model-core merge.rs); navbar workspace name's tooltip. Duplicate names (lead 11-18,
  DECISIONS): local, the store's unique index dropped on a writable open (no version bump), ops' `branch` by id or a
  unique name, an ambiguous name lists the ids. Left: merge preview "Edited" tooltip
  "Last change by <user> at <time>" (needs the last microversion per tab per side); switch's centred "Loading
  workspace…" spinner (ours switches at once: not a target unless slow); Create workspace and edit properties ->
  Properties modal fields (Name *, Description, Category "Onshape Workspace" greyed, Apply / Save greyed, Close):
  ours already (version_properties).
- live2 drawing creation (behaviours "studios (lead 11-18 ~02:07, feel): drawing creation", live/studios/
  drw-create-1118/): Create Drawing shows in 237 ms on "Recently opened" with "Retrieving templates…", OK disabled
  while it loads, no focus, Options Suggested / No views remembered; Enter after the list loaded creates (tab 1.9 s).
  First view (No views): the preview's centre on the pointer, no snap to the sheet centre. No Auto-dimension tool.
- live2 custom features (behaviours "studios (lead 11-18 ~01:53, feel): custom features", live/studios/
  custom-feature-1118/): dialog rows, conditional field, red title / pink field: ours already (test
  a_custom_features_dialog_is_generated_and_fails_as_live). Left: ✓ is pale in both while the error names a number
  field, live2 says a click still commits (ours: disabled; the hem capture said off): ask live2 to confirm. FS save ->
  Part Studio regenerates by itself ~1.1 s, an open dialog does NOT relabel: check ours.
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

## Agents (lead's roster 11-17 10:30, core / blend 11-18; only these ids)
psui 4973e295-9e92-463f-a13f-2ec5258bc5f9, core e8af89fe-d460-49b3-b0bb-9572da230fed,
blend 0672fa20-964d-4fde-a19c-c5b4363729ba (11-18; 6692ae35 / 0e6e9942 stopped), bool 7fa8ceab-4549-499f-9b01-3a289c2c95b4,
dx 2108c2b7-2f48-43a7-87a0-3f652e3cda6d (11-17 19:49; f93a1292 stopped), qa 132756b3-e48d-4b9f-8a0e-3d4f23629c8b,
asm d8d597e0-675c-497b-a804-544ceab99bbd, skasm 18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6,
bool2 e98465f5-5cc9-4d4f-b67e-aae1d5e6d2e0, studios 58e9bca8-346c-4847-8012-e15006ae61ef,
live e8549719-ef9a-42de-960c-e24b29f142a6, live2 79ae5cdb-42a4-4025-b43a-da61a4f92ad7 (fresh id 11-17).
Paused agents are not messaged (DECISIONS 11-17).

## Waiting on others
- live2 (79ae5cdb): the weld retake (metric positions). live (e8549719): the 8
  drw-revision-* and drw-styles-dimension retakes (failed: no Revision table tool, .6 dropdowns).

## Open (no owner action possible yet)
- live's drw-hole-callout-palette retake (23:03, q-hcp-shots/record.json) has the same states as the take: kept;
  the take matches.
- Projected view preview: ours snaps at 22.5° both ways. Live2's coarse sweep put the side/iso boundary at 25-30°
  (250 px) and 20-25° (400 px), and top at 20-25° from vertical. Left as is until a finer sweep.
- Assembly "?" rows: a list row's tooltip (live2 ~70 ms), a selected instance's outline through other parts in
  explode mode, a Revolute-constrained drag's grab point.
- Release gap list (archive 11-12 07:33), re-audited 11-17: done 1 (approvals), 2 (drafts, approve / reject, comment,
  obsolete, settings, require_note), 8 (Go back), 9 (revision table Copy), 10 (unreleased suffix), 11 (part number in
  another pending candidate). Still absent: "Release all configurations", the outdated associated drawing warning.
  Neither is in the versions universe (167/167): only with a live capture.

## Next, in order
See "In flight" and "Lead's queue (11-17)".

## How to run

Builds and crate tests on the Mac:
- `NOSHAPE_RB_HOST=mac tools/tiers/rb.sh cargo test -p <crate> --lib <filter> > log 2>&1`, then grep the log for
  `^test |panicked|test result|^error|  left|  right`. Never pipe into `head`: the kernel's and vendor's warnings
  fill it.
- The Mac target (~/agents/studios/target there) must stay under 20 GB (lead 11-15: the Mac folder's cap is 180 GB).
  tools/tiers/prune-target.py reads /proc and fails on macOS, so between builds delete target/debug/deps
  and target/debug/incremental (16 of 24 GB on 11-16) once `du -sh` passes ~15 GB.

App build on vibex6:
```
NOSHAPE_RB_HOST=vibex6 tools/tiers/rb.sh bash -c 'cargo build -p noshape; cp $CARGO_TARGET_DIR/debug/noshape ~/tmp/studios/noshape'
```
An rb.sh build to vibex6 deletes untracked takes there: put them back after it.

ui-parity on vibex6 (≤2 apps while its load is ≥16; heavy runs always through `~/bin/slot <cmd>`):
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
NOSHAPE_LAND_QUEUE_HOST=vibex7 tools/tiers/land.sh [--no-precheck]
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
