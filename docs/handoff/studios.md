# studios: current state

Owner: studios. Areas:
- documents, tabs, start page, Preferences
- versions, branches, history, release
- Variable Studio, Feature Studio (the editor and UI; the language and host stay dx's)
- the drawing UI (from dx, 11-12)
- the assembly UI's feel (11-13; asm keeps the mate solver)

History: docs/handoff/archive/studios.md (dated entries up to 11-15 18:03). Rulings: docs/DECISIONS.md.

Updated 2026-11-16 11:50 UTC. PAUSED by the lead (~24 h; restart ~11-17 10:15 UTC). Nothing running on any host.

## State at the pause

Worktree /root/projects/noshape-wt/studios, branch studios-0811a.
- Landed today: 547a769e, 79a35df0, c2d47005, db04b19b (08:12: panel fixes, Styles on a dimension, Create Drawing
  names, runner rules), 9c9366b1 (09:23: view drag + `drawing_view_move`, centermark reading + `expect` centermarks,
  Esc in Align centermark, the 4 flow scenarios, Add Items rows named, coverage "manual capture").
- Committed, NOT landed: bdab0ef9 (ui_script: a documents-page step that opens a document is timed until its active
  tab is built and drawn; clippy clean on the Mac). Land it first (tools/tiers/land.sh).

## Lead's queue (11-16 ~09:00), in order

1. Versions, branches and merge as full flows vs live; release management (Professional account).
   - 154 versions / release takes exist and matched (01:30-02:30); qa's 00:32 misses in my areas 16/16 on today's build.
   - New: ver-flow-branch-version-merge, ver-flow-merge-same-feature, rel-flow-revision-b, rel-flow-two-parts (on main);
     live2 asked (REQUESTS 09:06). Ours runs the first three end to end; rel-flow-two-parts stops at "Part 2": ours'
     Add Items tree opens Part Studio 1 collapsed (live2 to say how live's opens; then match).
   - Approve / reject: not capturable with one account (live2 11-14).
2. Speed in my areas vs PLAN 2.7 and live (behaviours.md "speed: studios" (1)-(4), "speed: studios, part 2",
   "speed: drawings"). vibe6 11-16 09:07-09:40, load 3-12 (hints, not the quiet window), medians of 5,
   ~/agents/studios-keep/perf-1116.log:
   | operation | ours frame / idle ms | live ms |
   |---|---|---|
   | drawing tab open (30-feature part, 4 views) | 11 / 37-41 | 3200-3700 (first 23000) |
   | tab switch PS / Asm / VS / FS | 13-29 / 11-79 | 1578-2705 |
   | Versions and history open (2000 entries) | 13-32 / 63-241 | 771 |
   | Compare V10 / Open B1 / Main / V40 | 26-33 / 11-149 | 7135 / 4551 / 4087 / 3668 |
   | Variable Studio 500 vars open / repeat | 37 / 85-87 | 5315 (200 vars) |
   None slower than live. Still to do: document open (perf-open-30 / perf-open-hist scenarios, needs bdab0ef9 in the
   app build: `python3 ~/agents/studios-keep/perfrun.py ~/tmp/studios/noshape :389 5 ~/tmp/studios/perf/perf-open-30.json`
   with an Xvfb on :389); Feature Studio open on a large studio. Fixtures: `python3 ~/agents/studios-keep/mkperf.py
   ~/tmp/studios/noshape ~/tmp/studios/perf` (vibe6 ~/tmp is pruned after 12 h; scripts + scenarios kept in
   ~/agents/studios-keep/{perf,perfrun.py,mkperf.py}, local masters /root/studios-scratch/{bin,perf}).
3. The 4 manual captures (lead 11-16: no recorder work): live2 asked (REQUESTS 08:47, live/feel/manual-4): FS Ctrl-hover
   cursor; quick tab switcher hover + mouse-up; Print… pop-up toast; shortcuts with focus on the top bar. When it lands:
   implement each, a GPUI test each, set `manual.test` in tools/ui-parity/universe/{documents,feature-studio}.json.
4. Box select (interaction-feel row 106): ANSWERED by live2 (live/feel/box-select-rule/README.md): L→R takes a view
   only with a whole model edge of it inside (part of an edge or the frame corner: nothing); R→L when the box meets its
   frame (geometry box + ~7 px); a dimension L→R when its whole text or an arrow end is inside (not an extension line's
   foot alone), R→L when touched; a press within ~7 px of a view's geometry drags the view, not a box. Implement in
   crates/drawing-ui/src/drawing_cells.rs `select_in_box` + the press rule, GPUI tests, update row 106.
5. Drawing matrix refresh: the full drawing run (306 takes) never got vibe6's slot (queue 2nd at 07:58); live2 landed
   15533efc (6 Move to sheet / Replicate retakes); Datum target / Weld / GTOL takes + 12 retakes after 18:50 UTC.
   Then drawing_matrix.py --run on the fresh run.

## Waiting on others
- live2 (paused to ~11-17 10:15): the 4 flow takes (09:06), the 4 manual captures (08:47), Datum target / Weld / GTOL
  panel takes and 12 retakes (after the translation quota, 18:50 UTC).

## Open (no owner action possible yet)
- live's drw-hole-callout-palette retake (23:03, q-hcp-shots/record.json) has the same states as the take: kept;
  the take matches.
- Full run of my areas (vibe5, 11-15 ~22:00, before the batch in flight): 992 of 1004 takes match. Besides the
  takes above: asm-cylindrical-limits (old).
- Projected view preview: ours snaps at 22.5° both ways. Live2's coarse sweep put the side/iso boundary at 25-30°
  (250 px) and 20-25° (400 px), and top at 20-25° from vertical. Left as is until a finer sweep.
- Assembly "?" rows: a list row's tooltip (live2 ~70 ms), a selected instance's outline through other parts in
  explode mode, a Revolute-constrained drag's grab point.
- Interaction-feel row 97 ≠: a centerline over a view edge, live's click takes the view, ours the centerline (GPU pick
  bias); P3.
- Release: the gap list in the archive (11-12 07:33 entry) is partly stale. Submit already makes the version, and
  approvals and the release_* ops (approve, reject, comment, drafts, obsolete, settings) exist. Re-audit it against
  the code before taking an item from it.

## Next, in order
See "Lead's queue" above: land bdab0ef9, then 1-5 there.

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
