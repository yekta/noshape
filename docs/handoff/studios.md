# studios: current state

Owner: studios. Areas:
- documents, tabs, start page, Preferences
- versions, branches, history, release
- Variable Studio, Feature Studio (the editor and UI; the language and host stay dx's)
- the drawing UI (from dx, 11-12)
- the assembly UI's feel (11-13; asm keeps the mate solver)

History: docs/handoff/archive/studios.md (dated entries up to 11-15 18:03). Rulings: docs/DECISIONS.md.

Updated 2026-11-18 00:56 UTC. Restarted 11-17 09:20 after the lead's pause; compacted ~14:45, ~18:15, ~23:30.

## In flight

Worktree /root/projects/noshape-wt/studios, branch studios-0811a. Land: `NOSHAPE_LAND_QUEUE_HOST=vibe2
tools/tiers/land.sh --no-precheck`; builds `NOSHAPE_RB_HOST=vibex6 tools/tiers/rb.sh`; drawing runs vibex8.
- Landed 11-17: ec42bd22 22:23, a4502066 23:37 (Edge condition symbols, weld centered joints, hatch Esc, hole callout
  dual row), 4014c1fd 23:43 (mate glyph press = its click; deterministic test).
- To land (committed, takes checking on vibex6): 4ea1aa5d (leader right-click selects the note, Add / Remove node keep
  it; Cell panel for ranges / merged cells as live; revision Cell panel sizes only; tables' Send to back; reader
  emulation: lines clipped to the view box at 2 px, a hovered note not counted), 740891f7 (documents list Ctrl or Cmd
  toggles), 7077f4c2 (VS: invalid name red / Enter nothing / Esc drops; foreign length shown in document unit; error
  tooltip "Error regenerating." + red border), b91c30b4 (view drag alone + aligned follow on release, Esc mid-drag puts
  back, 200 ms double-click on views, Esc mid-box nothing, Add revision callout unlit until placed).
- Full drawing run 11-18 00:00 on vibex8 (~/agents/studios-keep/drw1118a, build before the cell / VS / feel fixes;
  memguard pgid verified): its selection diffs came from a wider hover rule since narrowed to notes.
- Still differing (next, in order): drw-revision-table-delete s8 (after Delete live reads Top lit, ours Front: the
  hover-after-menu rule), ordinate add-to-group / Reset ordinate, note copy-paste menu, note editor closing, sheet
  properties zones, view adjust linestyle rows, replicate region panel, detail-id label position, section
  exclude-from-cut, synchronize-breaks Gap distance, simplification-absolute BOM row name, toolbar Undo tip take,
  drw-gtol-type-list (new take: pending annotation), drw-hatch-panel s29 list highlight, vs-cfg-delete-input s4 message
  (old take, likely pre-existing).
- Run when landed (vibex8): live2's drw-hole-table-empty-tips, drw-view-properties-projected-from-section,
  drw-view-replicate-region-place.

## Feel rows to take (11-17 23:3x, gaps only; check ours against each, fix with a feel test)
- live, VS / FS editing (behaviours "Variable Studio and Feature Studio editing", live/studios/vs-fs-feel-1117/):
  VS add: name + Enter makes "w | Length | 0 mm", focus to the next add row's Name; an invalid name red while typing,
  Enter does nothing; a duplicate becomes "w_0". VS value errors only after Enter: pink cell, red border selected,
  grey "Error regenerating." bubble over the Value header; "2 in" shows 50.8 mm, the expression in the bubble when
  selected. FS autocomplete: Enter inserts the snippet, Tab closes the list and inserts a tab, Esc keeps the typed
  text. FS errors while typing (<0.5 s, before saving): red x at gutter line 1 with a tooltip, squiggle on the token;
  Ctrl/Cmd+S commits, the Commit button pales; a fixed error greys ~150 ms, gone ~500 ms.
- qa HISTFEEL (11-18, TICKETS 16b1c4255; sheet vibex6 ~/agents/qa-keep/history-feel): P1 CHANGE-CLICK (a click on a
  change row opens nothing; live opens it with the bar "History changes are view only. Viewing <change>" [Restore to
  Main] [Return to Main]); P1 CHANGE-BAR (Open shows an orange VIEWING HISTORY badge, no bar; live: navbar "V3", the
  panel header only compare and x); P2 EXPANDED ("Hide changes..." vs live "15 changes"; rows 26 px white with a time vs
  30 px grey, no time); P2 SCROLL (search / toggles / legend fixed, lines over the legend; live scrolls all but the
  Name | Modified header, legend at the end); P2 CHANGE-TOOLTIP (none on change rows).
- live2, Configurations (behaviours "psui + studios (lead 11-18 ~00:30, feel): Configurations", live/feel/cfg-1118/;
  also sent to psui, who owns the Part Studio panel): row menu Switch to "X" / Set as default / Duplicate row / Move
  up / Move down / Delete row, a plain row click doesn't switch; duplicate row names accepted (id A0, shown "A"); bad
  cells pink + bold only once their row is active; no greyed rows on a switch, spinner after ~1.2 s. Mine: check the
  Variable Studio configuration panel against it.
- live, documents / Import / Derived + Insert (behaviours "studios + dx (lead 11-18, P0 feel): documents list,
  Import into a document, Derived / Insert panels", live/studios/docs-import-feel-1118/): search on Enter only (Esc
  keeps it); header sort A-Z, Z-A, default; row hover Share / Trash / Label icons; double-click opens; inline Rename
  (Enter / Esc); Delete or trash icon -> "Send to trash" (Enter nothing, Esc closes) + toast with Restore; New
  document Enter creates and opens. Import: "CAD Imports" folder tab ~1.8 s, the Part Studio tab before it ~6.9 s,
  active tab unchanged, blue bar under the bell, toast "<file> has been imported. Open document". Derived / Insert
  searches filter ~350 ms after the last key, Enter nothing; Derived: Enter 1 closes the panel, 2 commits; Esc 1
  closes, 2 cancels.
- live, history panel (behaviours "Versions and history panel: hover, a change row, scrolling a long history",
  live/studios/history-feel-1117/): hover never previews (row tints #def1fb + tooltip; a change row's "<tab> :: Update
  feature : Extrude 1: Change by <user> at <time>"; graphics / navbar stay the workspace's). Change rows 30 px, only
  under an expanded "⌄ N changes"; a click opens that microversion with the bar "History changes are view only.
  Viewing <change>" [Restore to Main] [Return to Main]. The whole panel scrolls as one (search box and toggles too),
  "Name | Modified" header pinned, all rows built up front, the graph continuous (amber branch line V1 -> hollow circle).
- live2, macOS keys: documents list toggles on Ctrl or Cmd on every OS (done, start_window.rs `toggles`); drawing
  Ctrl+M = Max/min checked: drw-max-min-key matches in the app (the workspace binding has no handler there).
- live, drawing view feel (behaviours "studios (lead 11-18, P0 feel): drawing view drag preview…",
  live/studios/drawing-feel-1118/): (1) a dragged view moves alone (orange, old place blank), aligned views follow
  only after release, no guide line, ends unselected, Ctrl+Z undoes; (2) Esc mid-drag: preview stays while held, the
  view goes back on release; (3) double-click anywhere a click takes a view (its empty inside too) opens View
  properties within 200 ms (not 250); on a dimension, its editor; (4) a dimension in a view lights alone, no dwell;
  (5) box select: no pre-highlight while dragging, shown on release; Esc mid-box nothing; empty box keeps selection.
- qa VSFEEL (11-18, d65a06570): name red / Esc (done), units 50.8 mm + error tooltip + red border (done); the bubble
  over the header is the hover tooltip drawn above the cell (live vs-value-decimals s2: none on a click): position
  still ours (at the pointer).
- live2, document level (behaviours "studios + psui (11-17, interaction feel)", live/feel/doc-level-1117/): tab switch:
  the URL changes at once, the old list and canvas vanish at once, then the toolbar swaps and the list fills, the 3D
  canvas last; no spinner. Undo / redo: Ctrl+Y redoes, Ctrl+Shift+Z doesn't (Linux; ours already: actions.rs), no
  toast; Rename and part Hide one step each; view changes / selections none; Ctrl+Z in a dialog's value field does
  nothing. Regen (psui's pending.rs spinner): old model stays drawn, rows grey; after ~0.75 s a 50x50 box at the
  viewport's bottom centre, blue spinner pulsing 30-42 px; rows update when done, spinner lingers, no toast.

## Lead's queue (11-17, restated after compaction)
1. Land what is green. 2. User P0: hover / select parity in Assemblies (instances, mate connectors, mates list <->
viewport, parts vs connectors) and Drawings (views, edges, dimensions, annotations: hover, click, box, drag), each
against live with a feel scenario; share psui's pick code (`Viewport::pick_priority`, `AsmPickMap::priority_fn`,
`PickKind::priority`; psui owns viewport.rs' hover path). 3. Versions / branches / release queue.
- Drawing feel rows all have a GPUI test now (feel.rs, 11) except the double-click palette (its own test).
- Assembly already as live (checked 11-17): box select rule, glyph click selects the mate (related rows, mated faces
  amber, dashed connector line), graphics hover lights no list row, list hover outlines the parts.

## Agents (lead's roster 11-17 10:30; only these ids)
psui 4973e295-9e92-463f-a13f-2ec5258bc5f9, core 6692ae35-758a-4442-81c8-29f67be2e21a,
blend 0e6e9942-0713-46c1-ae55-132fbbb4a14b, bool 7fa8ceab-4549-499f-9b01-3a289c2c95b4,
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
