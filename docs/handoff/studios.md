# studios: current state

Owner: studios. Areas:
- documents, tabs, start page, Preferences
- versions, branches, history, release
- Variable Studio, Feature Studio (the editor and UI; the language and host stay dx's)
- the drawing UI (from dx, 11-12)
- the assembly UI's feel (11-13; asm keeps the mate solver)

History: docs/handoff/archive/studios.md (dated entries up to 11-15 18:03). Rulings: docs/DECISIONS.md.

Updated 2026-11-17 22:09 UTC. Restarted 11-17 09:20 after the lead's pause; compacted ~14:45 and ~18:15.

## In flight

Worktree /root/projects/noshape-wt/studios, branch studios-0811a.
- Landed 11-17: 7abd9bdf, b5d11ad7, 01fb280e, 1db5e57e's fix; 19:31 as 539aeb9b: 09af37a6 (+358ab7a3, 66eee5f9,
  a457a82b): mate glyphs per type, part silhouette, instance row tooltips, drawing view frames, leader reach.
- Queued: on vibex7 b6c54cdd (carries ccab6c95 / c8d30fce, whose 20:42 failure was psui's real-pointer sketch test
  under load: it and the whole part-studio-ui suite pass on vibex6, 221/221); on vibe2 (`NOSHAPE_LAND_QUEUE_HOST=vibe2
  tools/tiers/land.sh --no-precheck`, lead 21:23) ec42bd22 = everything since, rebased on main 21:2x: Drawing
  properties footer / tab headers / hatch values, Styles gap fields (live2 confirmed s27-29), `{"px": [x, y]}` targets,
  Datum target panel (rows at live's heights, lists, Esc), weld captions, `MenuIcon::Glyph`, View properties' last
  lines, hole callout palette names / tooltips / count default, gtol type rows, inspection Lower limit, hatch tooltips,
  the runner's Drawing properties header rule, the matrix refresh.
- Full drawing run 11-17 (vibex8, outside the slot, 2 apps, memguard): 276/334 on the 20:08 build; the 58 differing +
  live2's 4 new takes rerun on the 21:17 build: 41 still differing then, many fixed since (checked on vibex6 with 1 app:
  datum x3, canvas, view properties, dprops, inspection, hcallout fit-tips). Results ~/agents/studios-keep/drw1117c and
  drwredo on vibex8; local copy /root/studios-scratch/m1117. Matrix: docs/parity/drawing-matrix.md (1046/1875 =).
- Still differing (next): weld Seam / Spot centered joints (new `WeldJoint`s, glyphs, export); Annotations tab's Edge
  condition symbols section (a new `AnnType`: shifts `wt_fields` indices); hcallout palette dual Units "(mm)", hatch
  Esc with a tooltip up, gtol pending annotation (asked live2 22:1x); table cell-format panel (drw-table-cells-*),
  table menus' disabled rows, leader add / remove node selection, ordinate add-to-group / Reset ordinate, note
  copy-paste menu, note editor closing, sheet properties zones, view adjust linestyle rows, replicate region panel,
  detail-id label position, section exclude-from-cut, synchronize-breaks Gap distance, simplification-absolute BOM row
  name, toolbar Undo tip take.
- live2's new takes queued on vibe2: drw-hole-table-empty-tips, drw-view-properties-projected-from-section,
  drw-view-replicate-region-place, the 8 drw-revision-* retakes: run when landed.
- Side-by-side crops for the lead: /root/studios-scratch/mate-glyphs-live-vs-ours.png, centermark-live-vs-ours.png,
  dimhover-sbs.png, view-sbs.png.

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
