# studios: current state

Owner: studios. Areas:
- documents, tabs, start page, Preferences
- versions, branches, history, release
- Variable Studio, Feature Studio (the editor and UI; the language and host stay dx's)
- the drawing UI (from dx, 11-12)
- the assembly UI's feel (11-13; asm keeps the mate solver)

History: docs/handoff/archive/studios.md (dated entries up to 11-15 18:03). Rulings: docs/DECISIONS.md.

Updated 2026-11-15 20:10 UTC.

## In flight

Worktree /root/projects/noshape-wt/studios, branch studios-0811a. Nothing uncommitted.

## Queued to land
- The 20:10 batch (one commit):
  - Feature Studio completion: live's completer score (one per matched letter among the first 32, −1 off the
    name's start, −1 per extra run), ties in live's list order (ace_order.txt, live2 feel/fs-ac-ties), constants
    listed, only what common.fs re-exports. The kind column has two more letters in a list short enough to show
    without a scrollbar (8 rows or fewer).
  - Notices: the squiggle's tooltip counts.
  - ANSI angular value on its arc, the arc broken around it.
  - ISO A4 Zoom to fit fills the frame.
  - 3 point angular reads EDGE from its first point to after Esc.
  - Checked on vibe5 20:05: drw-dimension-*, fs-ac-*, fs-ed-error-squiggle and the 3 earlier not-run takes all
    match except drw-dimension-foreshorten (next item 1).

## Waiting on others
- live2 (asked 19:58): retakes with screenshots for drw-insert-view-top-1to1, drw-hole-callout-palette,
  drw-note-move-to-title-block, drw-view-align-horizontally, drw-datum-target-hide-dim,
  drw-auxiliary-view-no-indicator, drw-datum-copy-paste, drw-dimension-dblclick-precision /
  -tolerance-symmetrical (live's "(in)" panel), context-create-version-update, drw-centermark(-align).

## Open (no owner action possible yet)
- drw-insert-view-top-1to1: live reads VIEW where five other Insert view takes read EDGE (the reader's pixels).
- drw-hole-callout-palette steps 17-18: our callout's leader is lit inside Top's box after the palette closes; live
  reads []. Near the reader's 60 px threshold.
- A table's Bring to front greyed when just placed (REQUESTS 12:38; ours greys it only for a table alone).
- Projected view preview: ours snaps at 22.5° both ways. Live2's coarse sweep put the side/iso boundary at 25-30°
  (250 px) and 20-25° (400 px), and top at 20-25° from vertical. Left as is until a finer sweep.
- Assembly "?" rows: a list row's tooltip (live2 ~70 ms), a selected instance's outline through other parts in
  explode mode, a Revolute-constrained drag's grab point.
- context-create-version-update: an old difference, not new.
- Coverage items that need live takes:
  - documents: the Print… pop-up toast, the quick tab switcher's hover + mouse-up, shortcuts with focus on the top bar
  - Feature Studio: the pointer cursor over an identifier with Ctrl held
- Release: the gap list in the archive (11-12 07:33 entry) is partly stale. Submit already makes the version, and
  approvals and the release_* ops (approve, reject, comment, drafts, obsolete, settings) exist. Re-audit it against
  the code before taking an item from it.

## Next, in order
1. drw-dimension-foreshorten (live2's 11-15 retake, recorder 2026-11-15.3; screenshots
   live/feel/dim-retakes-1115/):
   - the radial's menu in live's order: Edit..., Paste as table, Foreshorten, Unbreak dimension | Clear selection,
     Zoom to fit, Delete; no "Show as diameter" (not in live's)
   - foreshortened: Paste as table, Remove foreshorten, Unbreak dimension | the tail (no Edit...)
   - Foreshorten and Remove foreshorten leave the value where it was (live 83.5, 160.8 throughout; ours moves it,
     and Remove puts it at 109.7)
   - the dropped value's centre is 2.6 mm right of the hole's centre in live (ours on it)
   - live reads the radial selected after Esc (its value is lit inside Top's box)
2. Rerun the takes live2 retakes (list above) as they land.
3. Refresh docs/parity/drawing-matrix.md (last 11-15 14:30: 949 of 1868 live items compared and matching).
4. interaction-feel.md's remaining "?" rows in Drawings / Documents / Assembly (ask live2 for the missing captures).
5. A full run of my areas' takes (655 non-drawing, about 290 drawing) on vibe5 to catch regressions since 15:05.

## How to run

Builds and crate tests on the Mac:
- `NOSHAPE_RB_HOST=mac tools/tiers/rb.sh cargo test -p <crate> --lib <filter> > log 2>&1`, then grep the log for
  `^test |panicked|test result|^error|  left|  right`. Never pipe into `head`: the kernel's and vendor's warnings
  fill it.

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
