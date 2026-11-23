# psui (Part Studio UI): current state
Updated 2026-11-23 01:15 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Builds and crate tests on vibe2 or vibex8 (lead 11-22: no waiting on vibe2's one slot): `NOSHAPE_RB_HOST=… tools/tiers/rb.sh bash -c "$(cat script)"`,
  CARGO_INCREMENTAL=0; `git diff --name-only origin/main | xargs touch -c` first. ~/bin/slot
  only for suites, gates, timing pairs (DECISIONS 9e5b481d4). Scripts in /root/psui-scratch/run*.sh. The GPUI dialog matrix is
  `dialogs::the_dialogs_matrix_matches_live` (rows filter: DIALOG_ROW=…).
- Scenario / app runs: vibex6, at most 3 app instances (shared with studios). Heavy runs (scenario shards) through
  `~/bin/slot <cmd>` there (one per host, waits for 10 GB free). The app build there OOMs under load: scripts build
  with `-j 4`.
- Land only with `tools/tiers/land.sh` (`--status` for results). Start an rb.sh run, wait until it builds (its sync
  done), then land.sh: land.sh's rebase during an rb.sh sync breaks that build.
- Live: Chrome on :107, CDP 127.0.0.1:9333, helpers /root/live-pw; captures are asked of live2 in
  onshape-research/live/REQUESTS.md (dated `date -u`). Recorded behaviour: onshape-research/live/behaviours.md.
- Private documents: work on a mode-700 copy in ~/tmp, delete it after.
- GPUI tests: platform keys as "secondary-…"; marks (`ui_script::mark`) only record while a scenario runs, so tests
  find controls by `debug_selector`.
- psui owns docs/parity/interaction-feel.md (the feel matrices; all matched as of 11-15): keep it current.
- A new menu label needs its line in crates/ops/src/coverage.rs (`cargo test -p noshape-ops --test all coverage::`).

## Tools (/root/psui-scratch)

- `uip-some.sh <out> <names…>`: release build on the rb host, then the named ui-parity scenarios on one Xvfb (:392).
  Run: `NOSHAPE_RB_HOST=vibex6 tools/tiers/rb.sh bash -c "$(cat /root/psui-scratch/uip-some.sh)" uip-some <out> <names…>`
  (`~` in remote paths expands locally: pass /home/ngyekta/… paths). Output buffered: the log fills at the end.
- `psui-shards.sh <out> <names file>`: the same in 3 shards, then one `--compare-only` score. The names file must be on
  the host (scp it first). psui-names.txt: the 1021 psui-area scenarios with live takes.
- Compare without rerunning: `python3 tools/ui-parity/run.py --compare-only --out <dir> <names…>` (through rb.sh).
- Runner env: NOSHAPE_UIP_LOGS=1 (app stderr), NOSHAPE_UI_SHOTS=<dir> (a screenshot per step), NOSHAPE_PERF=1,
  NOSHAPE_CACHE_DIR (fresh: cold runs).

## Now (01:15 UTC 11-23)
Landing: b8a17535 QUEUED 01:2x (TRANSFORM-ROTATE-EDGE-SENSE axis edge, FEEL-EDIT-BODY-SHADE top via render `compare::POST_ONLY`,
CUSTOM-FEATURE-DIALOG-HELP, UIS-FIELD-NOT-SHOWN mark, Mac mini rename e2e, TICKETS: CTX-MENU-FLIP-UP closed (lead (a)),
6 union duplicates dropped). Earlier queue (f5ebb8b3, 02f48bdd, 975844f0) all on main. asm's c6ec3d1c (software settle):
once landed, rebase, re-run render + hover_matrix (end cap test tol 9: (131,140,147)/(138,148,154) vs live (132,146,155)).
NEXT (qa 11-23, TICKETS rows have the evidence):
- P0 FEEL-PS-SELECTED-EDGE-DARK-CORE: a selected visible edge's orange band has a dark 1 px line down its middle (old
  dc779 too, so not b6812's hidden-only copy alone). live/psui/sel-gestures-1122/ps-box-crossing-result.png vs
  qa-scratch/tickets/FEEL-PS-SELECTED-SEE-THROUGH-ver-1122.png. Suspects: renderer colour pass order (line_hidden
  before lines, renderer.rs ~1581), the selected band vs the base edge core. Probe with a hover_matrix frame first.
- P1 FEEL-PS-SELECTED-FRONT-FILL: selected front face (199,148,52) vs live (184,136,45); top/right match.
- TRANSFORM-ROTATE-EDGE-SENSE rest: radius lines + arc bold white, live thin grey.
- FEEL-EDIT-BODY-SHADE rest: sketch region (180,179,154) vs live (185,186,164).
- FEEL-DIM-SELECTED-REF-LINE-YELLOW PARTLY: line ok; leader/arrows ours ~2 px (106,79,6) big filled heads, live 1 px
  (131,111,53) small heads (tickets/FEEL-DIM-SELECTED-REF-LINE-YELLOW-ver-1123.png). Label in plane = skasm's.
- TRANSFORM-PLANE-HANDLE (verified) X/Y split: 12 px right gives 2.6/1.5 vs live 2.5/2.5; +40,+30 px 16.3/-8.1 vs
  19/-2.5 (feel1122/manipnav/r8).
- PARTLY rows (qa 11-22 notes in TICKETS): TRANSFORM-TRIAD-LOOK (dashed X arrow; orange outline at the original place
  while dragging), TRIAD-RELEASE-FOCUS (focus leaves the dropdown at drag start), PS-BOX-DRAG-HOVER (lit plane's edge
  hidden behind the part), ROLLBACK-CLAMP-HOVER (pill ~18 px, drop line has no dot).
- qa wf-1123 on 24f1566de (before 3cfb09978; 01-03/06/07 replay on its first green): FEEL-CORNER-BREAK-VERTEX-PICK
  (03 s16) no disc at all (live a saturated orange disc), arrow outline white dashed: recheck on b8a17535's green.
  New P2 PS-PARTS-ROW-TOOLTIP-PITCH (17 s11): live "Part 1" tooltip on the clicked row and 26 px pitch; ours none, 24.
- CTX-MENU-FLIP-UP: the flip is closed (lead (a)), but qa keeps the row open. The drawing view menu (wf40 s57) opened with
  "View orientation" hovered and its submenu showing, so the pointer was over the menu. Check our place (drawing_view.rs
  ~186: pointer + (0,2)) against step 57's pointer (qa hm2/out-wf-v/40-*/cmp-57.png, live 111-327 above it).
- qa errors-1123 set b (TICKETS "qa, 2026-11-23 01:2x", evidence qa-scratch/tickets/ERR-*-1123.png), 3 psui P0s:
  ERR-HOLE-INVALID-COMMIT (Hole depth 0: ours commits on the 2nd Enter, retitled "↧ 0 mm"; live's Enter does nothing),
  ERR-MISSING-SELECTION (live "Missing Face of Extrude 1" red + yellow icon overlay + "1 missing selection" tip; ours
  a plain entry; core owns the flag), ERR-INVALID-VALUE-CHROME (count 0: two spaces after "properly:", the row tip
  carries the message, 2 px red underline without the blue focus line). Do these FIRST (P0), before the dark core.
- ERR-FEATPATTERN-TITLE-SUFFIX (core hands it to psui): a faulty Bool field (std flags fullFeaturePattern on "Could
  not create all instances…") must not add " A parameter value is not valid." to the title tip (feature_dialog.rs
  ~4553, any non-Query faulty field today) nor draw the checkbox red; live shows neither.
- qa errors-1123 set c (TICKETS block 01:4x), 4 psui P0s: ERR-FIELD-RANGE-TIP-ENTER (sheet metal out-of-range: no
  "Enter a value between … Current value is …" tip, 2nd Enter commits; same root as ERR-HOLE-INVALID-COMMIT; skasm is
  adding the tip behind Hints::RANGE_TIP: coordinate, psui owns the Enter-doesn't-commit rule), ERR-SM-BAD-SELECTION-RED
  (failing entry grey, joint edge not red), ERR-MATE-HOVER-GLYPH (error mate row hover: live red glyph + orange edges;
  ours tints parts; assembly: tell asm first), ERR-LIST-TOOLTIP-CHROME (row tip drops unticked rows, no 400 px wrap,
  root red dot instead of the circled ⬇).
- t85 (vibex8, ~/tmp/psui/t85.log, script /root/psui-scratch/run-t85.sh) checks the batch below: clippy app-core/app/
  part-studio-ui/drawing-ui + dialog tests + chrome/feedback matrices + drawing menus. It started before the
  "two spaces" edit: rerun once, then land. b8a17535's batch failed a_chip_made_by_a_later_feature; it passes alone
  (t84), lead told. skasm dropped RANGE_TIP and re-adds their Flange test on top of mine. asm: mate hover/row tip
  are free (their root-row tip hunk ~5299 lands first: the circled ⬇ waits for it).
- qa 02:xx on green ed11f7c48: VERIFIED PARTS-ROW-OF-PICK-SHEET-METAL, PICKED-EDGE-DASHED-UNDER-PREVIEW,
  HOVER-DISC-AFTER-COMMIT. PARTLY: CORNER-BREAK-VERTEX-PICK (disc 70 px vs 73; white 167 px arrow outline x1090-1147
  y500-521, ~450 black px round the vertex vs live 18), CONFIG-FILTER-HELP (s3-4: the Supported filters popover 400x133
  at 1312,147 missing). Open: FEEL-ROW-TIP-AFTER-DBLCLICK (01 s17), UIS-LIST-ROW-MENU-ROWS. tickets/<T>-ver-1123.png.
- Landing (02:2x, error P0 batch + p04 flake fix; dx 2782ce84 waits on it): Enter refused on an out-of-range value (`range_error`, accept_edit), title keeps the last
  valid value, `title_status()` (row tip while open, count 0 "properly:  A…"), invalid field no blue hover, general
  range tip "Enter a value between 0.0100 and 500000.  Current value is 0." (two spaces, value whole; skasm: live/skasm/out-of-range-1123), Bool faulty no suffix, faulty query field's
  entries red, active empty box tip = message beside the dialog, menus flip at the tab bar's top (os_menu
  `set_menu_floor`, workspace canvas; anchored `flip_floor` patch), drawing canvas menu fit to that floor.
  Still open: ERR-SM-BAD-SELECTION-RED's red joint edge in the viewport (repro qa-scratch/err/c/cells/sm-fl-joint2.json),
  ERR-CHROME-TIP-PLACEMENT title + row tips (~5 px right, 7 low), ERR-TOOL-BODY-COLOUR (scene error_geometry: live tool
  body (113,76,79), ring (141,68,73), region outline only), ERR-MATE-HOVER-GLYPH, ERR-LIST-TOOLTIP-CHROME.
  ERR-MISSING-SELECTION: core's flag ~3-4 h after 02:00 (they message), then check.
- qa appearance-1123 (TICKETS 01:5x): P0 FEEL-APPEAR-FACE-SELECT-FILL (face/feature appearance dialog fills picked faces
  orange; live keeps their colour, orange outline only). P1 PANEL-FA-ROWS, PART-NO-RESET (drop "Remove appearance"),
  PART-FACE-MENU-2-PARTS, EMPTY-FACE-DIALOG (✓ off, 000000), DIALOG-UNDER-OVERLAYS. P2 DIALOG-OFFSET (5 px), PS-PROPERTIES-
  DIALOG-SIZE, UIS-APPEAR-TARGETS.
- Appearance: live/psui/appearance-1123/index.md (41 shots; behaviours "psui the Appearance panel in full"): part
  dialog 220x400 Palette|Mixer, ✓ off until a change, no Reset; Face appearance rows only in the Appearances panel;
  Boolean/split/pattern colour rules. Compare ours after the Mac mini P0.
- skasm (lead ruling): range-error field tip behind Hints::RANGE_TIP (sheet metal only, skasm, in feature_dialog.rs);
  if live's recapture shows it general, making it the default for non-integer fields is psui's.
USER P0 Mac mini: dx IMPORT-SOLID-COLOUR-ON-EVERY-FACE pending; core's union (6d3caf84) queued. Then
APPEARANCE-PART-UNDER-FACE-COLOURS: test + the user's doc on a mode-700 copy (~/tmp/psui/macmini-m6.noshape, same as
lead-scratch's), then delete it and /root/psui-scratch/Mac-mini-2024.step. Rename: not reproducible (TICKETS line).
Queue P2: FEEL-ROW-TIP-AFTER-DBLCLICK, UIS-LIST-ROW-MENU-ROWS, CUSTOM-FEATURE-ROW-ICON, UIS-FEATURE-INITIALS.
- FEEL-SKETCH-INF-END-DOT: lead deciding; sketch_mode is skasm's (tell skasm first).
- PS-RAIL-STOPWATCH-WARNING: the software-renderer performance badge (live's own); environment, not a bug.

skasm owns FEEL-MC-* Part Studio set (10) + SKETCH-DIM-PLACE-PICKS-PLANE (11-22 18:0x); mate connector inference code is
skasm's. Told them batch 1 carries the Shift lock, ConnectorLook::Placed and the with-layer hover mask.
Then: assembly column of sel-gestures (tell asm first); autocomplete list position. Lead rule: end turns at ~180k context.

## Agents

- skasm owns the hover matrix's sk/* cells (lead 11-22); psui the 3D / Part Studio cells.
- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
