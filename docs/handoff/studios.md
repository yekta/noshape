# studios: current state

Owner: studios. Areas:
- documents, tabs, start page, Preferences
- versions, branches, history, release
- Variable Studio, Feature Studio (the editor and UI; the language and host stay dx's)
- the drawing UI (from dx, 11-12)
- the assembly UI's feel (11-13; asm keeps the mate solver)

History: docs/handoff/archive/studios.md (dated entries up to 11-15 18:03). Rulings: docs/DECISIONS.md.

Updated 2026-11-15 23:37 UTC.

## In flight

Worktree /root/projects/noshape-wt/studios, branch studios-0811a. Nothing uncommitted.

## Queued to land
- aaa63901d (committed, not queued: waits for the full drawing run on vibe5, ~/tmp/studios/drw288, task queued behind
  ~/bin/slot; don't rebuild ~/tmp/studios/noshape there until it ends):
  - a projected view's View orientation ▸ (Top, Left, Right, Front, Back, Bottom, Isometric; its own greyed, no
    checks) turns its base so that it shows the pick, the family following (`views::reoriented_base`,
    `set_base_frame`; op drawing_view_display orientation on a projected view; e2e drawing_view_menu_edits; take
    drw-view-orientation-projected from live2's vo-probe)
  - the sheet's right-click menu opens with its top 35 px above the pointer, left edge 1 px right (live2 rmb-probe);
    panel and list menus keep +2 px below
  - what a menu row leaves hovered under the pointer, and Align views' view and target, read lit with the `new`
    marker (only the 11-15 recorder reads them): drw-view-align-horizontally / -vertically match
- Checked on 18 menu takes (vibe5): no regressions.

Landed 23:11: af25fdb0 as 0876332a; c1e1a6e5 (drawing matrix + handoff) queued 23:13.

## Waiting on others
- live2 (asked 23:20): which points near the title block's TITLE field highlight it (x 230, y 36-48 mm). live2's
  rmb-probe: (230, 40) is the TITLE field; a right-click there adds it to the selection and the menu is the field's
  plus the selection's (datum: Copy, Paste as table, Add leader | Clear selection, Zoom to fit, Delete; note: + Move
  to; dimension: no Copy; view: as datum). Ours treats (230, 40) as empty sheet: our TITLE cell is its row only.
- live2: panel takes for Drawing properties, Hole table, Title block (asked 23:15).
- drw-note-move-to-title-block s8: decision asked of the lead. Live's note menu is one row and a separator taller
  (Add comment, cloud), so when it shifts up to fit the window its Move to ▸ Title block row sits ~30 px higher than
  ours and the pointer ends on Front's edge in live, 31 px under it in ours.

## Open (no owner action possible yet)
- live's drw-hole-callout-palette retake (23:03, q-hcp-shots/record.json) has the same states as the take: kept;
  the take matches.
- Full run of my areas (vibe5, 11-15 ~22:00, before the batch in flight): 992 of 1004 takes match. Besides the
  takes above: asm-cylindrical-limits (old).
- drw-insert-view-top-1to1: live reads VIEW where five other Insert view takes read EDGE (the reader's pixels).
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
1. drw-datum-copy-paste: Add leader on the empty sheet's menu and the menu's placement, from live2's answer.
2. View orientation on a projected view's menu, from live2's answer.
3. Ask live2 for takes of the drawing panels with most items uncompared (docs/parity/drawing-matrix.md, refreshed
   23:11: 964 of 1874 compared and matching, none missing): Drawing properties, Hole table, Title block, Note, Hatch
   region, Hole/thread callout.
4. interaction-feel.md's remaining "?" rows in Drawings / Documents / Assembly (ask live2 for the missing captures).

## How to run

Builds and crate tests on the Mac:
- `NOSHAPE_RB_HOST=mac tools/tiers/rb.sh cargo test -p <crate> --lib <filter> > log 2>&1`, then grep the log for
  `^test |panicked|test result|^error|  left|  right`. Never pipe into `head`: the kernel's and vendor's warnings
  fill it.
- The Mac target (~/agents/studios/target there) must stay under 20 GB (lead 11-15: the Mac folder's cap is 180 GB).
  tools/tiers/prune-target.py reads /proc and fails on macOS, so between builds delete target/debug/deps
  (`ssh mac 'rm -rf ~/agents/studios/target/debug/deps'`) once `du -sh` passes ~15 GB.

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
