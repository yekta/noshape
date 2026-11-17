# studios: current state

Owner: studios. Areas:
- documents, tabs, start page, Preferences
- versions, branches, history, release
- Variable Studio, Feature Studio (the editor and UI; the language and host stay dx's)
- the drawing UI (from dx, 11-12)
- the assembly UI's feel (11-13; asm keeps the mate solver)

History: docs/handoff/archive/studios.md (dated entries up to 11-15 18:03). Rulings: docs/DECISIONS.md.

Updated 2026-11-17 15:36 UTC. Restarted 11-17 09:20 after the lead's pause; compacted ~14:45.

## In flight

Worktree /root/projects/noshape-wt/studios, branch studios-0811a.
- Landed 11-17: 7abd9bdf (doc-open timing), b5d11ad7 (box select), 01fb280e (panel text form, versions flows,
  manual-4). 57dc3ed4 failed 13:06 (ops e2e: first angle's side view); fixed forward (drawing_get "isometric",
  e2e takes either side view; ops e2e 222/222 on vibe5).
- Failed twice (14:54, 15:44) on coverage reading the drawing_ui_tests/ folder as a file; fixed. 1db5e57e, 11 commits: section / detail / menu readings, first angle, the e2e fix, the
  drawing feel tests (drawing_ui_tests/feel.rs, 7), centerline_on_edge, Move to sheet / cell Note / Cell panel
  readings, the mate-row flyout (assembly_flyout.rs).
- Committed 15:36 (landing after 1db5e57e), from live2 asm-feel 13:05 and asm-feel2 15:0x (behaviours "studios
  (11-17 12:52, P0)" and "(11-17 15:0x, P0 asm feel follow-ups)"):
  - mate glyphs at screen size (asm_scene::MateGlyphs camera geometry, 26 px, a pick disc 2 px past it); a hovered
    glyph sits on a light backing square and outlines its part. Still ours: a square glyph for every mate type
    (live: per type, Slider = a ring and an arrow) - next look item;
  - a hovered face shows its part's silhouette in #FFC20E (Highlights::hover_part, render Scene::hover_outline_rgb),
    only that part inside a subassembly;
  - every instance row has its name as its tooltip (live2: the 11-14 "only when cut off" was wrong);
  - the unticked Angular section takes 3 clicks (section_free_angle); section takes 10/10 match;
  - tests: assembly-ui src/assembly_feel_tests.rs (glyph reach + click, box rule), drawing
    an_angular_section_without_a_cut_angle_takes_its_line_from_two_clicks, feel
    a_dragged_view_lands_where_released_with_no_alignment_snap (F4).
- vibe6 slot (queued behind psui's run): asm + section takes, 224 (~/tmp/studios/asmsec, log
  ~/agents/studios-keep/asmsec.log).
- live2 2fdb573f (queued): drw-datum-target-panel-tips, drw-hatch-panel-tips: implement once landed. Next from them:
  Custom table, Hole table.
- Not rerun yet: the drawing matrix refresh (`drawing_matrix.py --run` on ~/tmp/studios/drw1117), the speed run
  (speed1117.sh). The weld take waits on live2's metric-position fix. Live re-records the 52 panel takes (.5): then
  fill panel texts past line 30.

## Lead's queue (11-17, restated after compaction)
1. Land what is green. 2. User P0: hover / select parity in Assemblies (instances, mate connectors, mates list <->
viewport, parts vs connectors) and Drawings (views, edges, dimensions, annotations: hover, click, box, drag), each
against live with a feel scenario; share psui's pick code (`Viewport::pick_priority`, `AsmPickMap::priority_fn`,
`PickKind::priority`; psui owns viewport.rs' hover path). 3. Versions / branches / release queue.
- Drawings without a test yet: note leader reach (needs a leader op), snap markers, placement inference, view drag
  alignment.
- Assembly already as live (checked 11-17): box select rule, glyph click selects the mate (related rows, mated faces
  amber, dashed connector line), graphics hover lights no list row, list hover outlines the parts.

## Agents (lead's roster 11-17 10:30; only these ids)
psui 4973e295-9e92-463f-a13f-2ec5258bc5f9, core 6692ae35-758a-4442-81c8-29f67be2e21a,
blend 0e6e9942-0713-46c1-ae55-132fbbb4a14b, bool 7fa8ceab-4549-499f-9b01-3a289c2c95b4,
dx f93a1292-5bb7-406f-9a25-59767e01878e, qa 132756b3-e48d-4b9f-8a0e-3d4f23629c8b,
asm d8d597e0-675c-497b-a804-544ceab99bbd, skasm 18a8ce79-5f2e-4089-9ad8-cc9d1182f0e6,
bool2 e98465f5-5cc9-4d4f-b67e-aae1d5e6d2e0, studios 58e9bca8-346c-4847-8012-e15006ae61ef,
live e8549719-ef9a-42de-960c-e24b29f142a6, live2 79ae5cdb-42a4-4025-b43a-da61a4f92ad7 (fresh id 11-17).
Paused agents are not messaged (DECISIONS 11-17).

## Waiting on others
- live2 (79ae5cdb): the weld retake (metric positions), 2fdb573f (datum-target / hatch tips). live (e8549719): 43
  re-recorded drawing takes + 3 Hole/thread callout palette takes landing in one commit; the 8 drw-revision-* and
  drw-styles-dimension in ~40 min (from ~15:10): run them all when they land (then fill panel texts past
  line 30).

## Open (no owner action possible yet)
- live's drw-hole-callout-palette retake (23:03, q-hcp-shots/record.json) has the same states as the take: kept;
  the take matches.
- Full run of my areas (vibe5, 11-15 ~22:00, before the batch in flight): 992 of 1004 takes match. Besides the
  takes above: asm-cylindrical-limits (old).
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

App build on vibe6:
```
NOSHAPE_RB_HOST=vibe6 tools/tiers/rb.sh bash -c 'cargo build -p noshape; cp $CARGO_TARGET_DIR/debug/noshape ~/tmp/studios/noshape'
```
An rb.sh build to vibe6 deletes untracked takes there: put them back after it.

ui-parity on vibe5 (lead 11-15 19:50: vibe6 loaded; ≤3 apps, always through `~/bin/slot <cmd>`):
- Sync: `rsync -a tools/ui-parity/ tests/ui-parity/` to `vibe5:agents/studios/src/…`. The app binary is
  `~/tmp/studios/noshape` there: build with `NOSHAPE_RB_HOST=vibe5 rb.sh …` (as for vibe6 above), or relay a built
  one (`ssh vibe6 'zstd -c …' | ssh vibe5 'zstd -d > …'`; the hosts have no keys to each other).
- Run: `~/bin/slot bash ~/agents/studios-keep/shards-names.sh ~/tmp/studios/noshape <out> <names file> 3 381`.
- Compare again without a run: `python3 tools/ui-parity/run.py --compare-only --out <out> <names…>` in
  ~/agents/studios/src.
- The script's master copy is /root/studios-scratch/bin/shards-names.sh. The hosts' ~/tmp is pruned after 6-12 h,
  so keep the script in ~/agents/studios-keep (on vibe5 and vibe6).
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
