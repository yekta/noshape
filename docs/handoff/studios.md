# studios: current state

Owner: studios. Areas:
- documents, tabs, start page, Preferences
- versions, branches, history, release
- Variable Studio, Feature Studio (the editor and UI; the language and host stay dx's)
- the drawing UI (from dx, 11-12)
- the assembly UI's feel (11-13; asm keeps the mate solver)

History: docs/handoff/archive/studios.md (dated entries up to 11-15 18:03). Rulings: docs/DECISIONS.md.

Updated 2026-11-16 06:12 UTC.

## In flight

Worktree /root/projects/noshape-wt/studios, branch studios-0811a. Nothing uncommitted.

## Queued to land
- Landed today: 547a769e (TITLE split, Drawing properties, title-field picks, 05:02), 79a35df0 (box past 7 px,
  05:17).
- Queued 06:12: 2e6aa396d hole tables as live2's drw-hole-table-* (Split Hole table below: the part goes under the first,
  3 mm apart, left selected; Esc keeps a selected hole table; Bring to front / Send to back offered; a cell's menu
  Size ▸ / Resize... | Copy, Rows Equally over a merged cell, Esc closes the Cell panel too (`OsMenu::after_escape`);
  run.py strips live's "NoteContextMenus" separators). Table takes on vibe6: 49/50; drw-hole-table-cell-bold s12 is
  live's state read before its Cell panel showed (it shows at s12 in -cell-rmb), left.
- Live's reader reads a selected hole table's lines inside a view's box as EDGE <view> (live2 11-16,
  live/uip/drw-hole-table-split-14.png); ours' reader does the same, so no runner rule is needed.
- Full drawing run (306 takes) queued in vibe6's slot at 06:14 (~/tmp/studios/drwall, log
  ~/agents/studios-keep/drwall.log): don't rebuild vibe6's binary until it starts. Then the matrix:
  drawing_matrix.py --run ~/tmp/studios/drwall --ours-menus (MENUDUMP of drawing_context_menus_are_lives).

Areas (11-16 01:30-02:30, vibe5): Feature Studio 116/116 takes, versions/history/release 154/154, documents/tabs/
start/prefs 285/285 (SCORE.md's 97.8 / 98.2 / 98.4 are from 11-11..13).

## Waiting on others
- live2: Hatch region / Note panel / Hole callout explorations (asked 03:30), then their takes.

## Open (no owner action possible yet)
- live's drw-hole-callout-palette retake (23:03, q-hcp-shots/record.json) has the same states as the take: kept;
  the take matches.
- Full run of my areas (vibe5, 11-15 ~22:00, before the batch in flight): 992 of 1004 takes match. Besides the
  takes above: asm-cylindrical-limits (old).
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
1. Refresh docs/parity/drawing-matrix.md (drawing_matrix.py --run on a fresh full drawing run + our menus).
2. Hatch region, Note panel, Hole/thread callout takes as live2 records them.
3. interaction-feel.md's remaining "?" rows (Drawings: a projected view's drag axis, asked; the centerline case).

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
