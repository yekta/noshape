# psui (Part Studio UI): current state
Updated 2026-11-23 03:05 UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

## RESUME HERE (09:05 UTC 11-23): cc8570b3 batch LANDED. QUEUED 78be1090 = 825f6603f UIS-STATE-GEOMETRY + 7506baee8
(below; t114/t115 green, clippy clean). Live recorder side of the geometry state: live/REQUESTS 07:17.
In 7506baee8:
- ERR-TOOL-BODY-COLOUR: failing tool matte (preview_body_ids), alpha 0.8, TOOL_PREVIEW_FAIL 0x9b3d40 (scene palette):
  extrude (129,60,63) vs live (130,62,67), revolve (129,63,66) vs (135,63,69). Test a_failing_tool_body_is_lives_matte_dark_red.
- FEEL-TRANSFORM-TRIAD-LOOK drag: ghost_outline_only(.., &selection) marks a selected part's ghost faces; renderer.rs
  draw_lists body_mask also draws lists.transparent. Dragged orange 944 (live 1384), ghost #C2CDD5 1771 (live 2418).
- FEEL-ROLLBACK-CLAMP-HOVER pills/dot (list matrix green); clippy sort fixes; TICKETS notes for all + ERR-LIST-TOOLTIP-CHROME.
- FEEL-EDIT-BODY-SHADE region: studied (print_edit_body_shade T2); only the region through the right face is off by
  (4,7,10): the preview light's right face; left open in TICKETS.
Then: commit, land.sh (825f6603f + these), report lead <=5 lines.
Not mine now: ERR-SM-BAD-SELECTION-RED joint edge (skasm 11-23, lead's ok); dx adds an app-core toast stack (touches
mod.rs DocEvent::Info/Error + render_viewport_overlay toast).
Open next: ERR-MISSING-SELECTION (core flag), FEEL-MC-LOOK navy px (left: llvmpipe), P2 queue below.
Older notes (t101):
script /root/psui-scratch/run-t102.sh). t101: clippy clean, PS 37/39, HIST ok. Since t101: div.rs gate REVERTED (the
row tip "Extrude 1 <- Sketch 1" still showed after a quick double-click), list-matrix row re-gapped, TICKETS
FEEL-ROW-TIP-AFTER-DBLCLICK back to [ ]; custom new-row initials assert DROPPED (test finds no feature:new row; noted
in CUSTOM-FEATURE-ROW-ICON); Line end: the general cursor dot also skipped under a lit target (sketch_mode.rs ~6815);
Properties footer: body h -163.5 / -122.5, gap 5.25, pr 1 (test checks gaps + Close right/top/bottom); b3 field face
threshold #ffc685 > 100 (125 now). Mac mini DONE: green 31f1866d8 on the user's copy: part appearance turns 227472 of
229112 body px, the ~1500 face-coloured px stay (as live); copies deleted (local + hosts clean). Commit 2 = e2e.rs
test + fix TICKETS line ~23-25 (my 7e017ac17 split APPEARANCE-PART-UNDER-FACE-COLOURS with the rename "[x] not
reproducible" line; mark it [x] with the Mac mini result). live REQUESTS: plane border over a part (FEEL-PS-BOX-DRAG-HOVER rest).
qa 200ca055e (green 31f1866d8): PARTLY FEEL-EDIT-BODY-SHADE top (138,148,154) vs live (132,147,155); region tint
(180,179,154) vs (185,186,164); Rotate radius lines (207,222,236) vs live thin grey (compare). Evidence feel1122/manipnav/r9.
t99: clippy clean; app-core initials ok; PS 17/18 ok incl. the flange repro (ERR-SM-BAD-SELECTION-RED test green).
Then ONE batch commit (all but crates/ops/tests/e2e.rs) + land.sh; report ≤5 lines: matrix 113/125 (ask qa to re-score
the 12 cells: 3cfb09978 landed after their 22:56 scoring + this batch), keymap parity, ERR-MATE-HOVER-GLYPH → asm,
tooltips (title/field/row placed, dblclick none), SKT-POSTCOMMIT-HOVER + SKT-USE-HOVER done, UIS-STATE-GEOMETRY needs
the ui-parity comparer (decision/coordination). After the rebase, mark SKT-POSTCOMMIT-HOVER / SKT-USE-HOVER in TICKETS
(qa's rows exist only on main). Then commit 2 (e2e.rs Mac mini) + land; Mac mini doc check; delete the copies.
Added since compaction (uncommitted): edge_looks clippy fix; PS-PROPERTIES-DIALOG-SIZE (PROPS_MIN = 1173×687, body
heights from the dialog height in properties_dialog.rs + history-ui version_properties.rs, test
the_properties_modal_has_lives_size); hover cells: Line end dot under the lit target (sketch_mode.rs
end_on_lit_target / end_dot_under_target, drawn at HIGHLIGHT_LINES_LAYER-1 in sketch.rs; skasm told; test
the_line_end_dot_goes_under_a_lit_target), Fillet face hover: preview-made boundary edges stay plain (mod.rs after
ghost_outline_only; b3_field_face_hovered_edge_picked asserts no #ffc685 within 7 px), MC wedge sides stop at the
inner circle (builder.rs, no navy square). OVER_LIT_FACE / MC fixes of 3cfb09978 landed AFTER qa scored 113/125 on
5e634b35e: ask qa to re-score the 12 cells after this land. ERR-TOOL-BODY-COLOUR: colour within 10 of live (TICKETS
PARTLY), region outline only in this batch.
Also added (uncommitted, in t100): SKT-POSTCOMMIT-HOVER (sketch.rs: a click committing a shape holds the sketch hover,
hold clears the inference when not mid-draw; test sketchtools_feel a_committed_shape_is_plain_under_the_still_pointer);
ERR-CHROME-TIP-PLACEMENT (live/psui/err-tip-placement-1123: title tip at title right −25, top +1 via ui.rs
with_dialog_title_tooltip; field tip inset −1 = box right +2; list matrix row "a feature row's tooltip 3 px right";
a_hole_without_points asserts the title tip place); FEEL-ROW-TIP-AFTER-DBLCLICK (live row-tip-dblclick-1123: none
after a double-click, a shown tip stays): gpui-pre div.rs GATE_FREE_BLOCKED (press before the tip shows blocks it
till the pointer leaves); matrix row ungapped. If the wf10 row breaks again, revert div.rs and re-gap.
QUEUE after: qa 11-23 (cd5c34163) P0 SKT-POSTCOMMIT-HOVER (no hover on the just-committed curve under the pointer),
P0 SKT-USE-HOVER (Use: face loop light orange, vertex offers its edge, edge 20 px light blue), P1 UIS-STATE-GEOMETRY;
evidence /root/qa-scratch/tickets/SKT-<X>-1123.png (pixel-compare by script only). MC centre white vs live yellow-green
(cell 5) unverified.

## Now (04:30 UTC 11-23) — all below UNCOMMITTED in agent/psui (rebased on origin/main 04:00)
Landed 03:39: b8a17535 (7e017ac17) + c318cefa (0c1d80e6); matrix 113/125 reported to lead; dx/qa told.
Runs on vibex8 (logs ~/tmp/psui/tNN.log; grep only): t92 GREEN (Mac mini e2e, clippy psui/ops, 12 list/appearance
tests); t93 (clippy part-studio-ui + app, custom_feature parts_list the_feature_list_matrix tooltip) running; t94
(clippy scene/render/psui, edge_looks::, remove_extrude/extrude_ with the new tool_failed assert in
remove_extrude_result) chained; t95 (print_failed_tool GPU print, ignored test in edge_looks.rs) chained.
Batch commit 1 (after t93+t94 green): edge core (LINED), front fill (`face_selected_dim` in shaders.wgsl), dim leader
1.5 (skasm told), parts rows 26 px (slot h 26, inner h 24 my 1, selector `part-band:{name}`), tooltip kept, appearance
P0/P1/P2s, UIS-FEATURE-INITIALS (`script_feature_name`), CUSTOM-FEATURE-ROW-ICON part (`feature-new-initials`),
UIS-LIST-ROW-MENU-ROWS (app ui_script locate list_row:→menu:), FEEL-ROW-TIP-AFTER-DBLCLICK (gpui-pre TooltipPressGate),
region outline only on a failed tool (Highlighting.regions_outlined = tool_failed()).
Commit 2 (Mac mini, user P0 APPEARANCE-PART-UNDER-FACE-COLOURS): ops e2e
`a_part_appearance_over_an_imports_face_colour_turns_the_other_faces` (green in t92). Then the user's doc in the app once
/root/green has 48012be79; then delete ~/tmp/psui/macmini-m6.noshape and /root/psui-scratch/Mac-mini-2024.step.
Keymap vs live/shortcuts-1123 DONE (script /root/psui-scratch/keygap.py): every key live binds in ps/psdlg/sk/sktool is
bound in ours except the debug panel (⌥D/⌃D), Contact support (⌃U) and F1 web help (cloud); reverse: only j (asm-only,
right) and secondary-n/o/w (desktop window keys). Report to lead with the next land.
ERR-TOOL-BODY-COLOUR fit so far: live revolve remove-miss body is LIT (118,59,63)/(134,63,68) = base 0x8a3a3f at α≈.95
over bg ~220; live extrude miss cylinder flat (112-117,76-78,79-80), edge (106,63,66); ours in qa's shot (139,86,92).
t95 prints ours (top/front/side, α .9 and 1) to fit.
ERR-LIST-TOOLTIP-CHROME (uncommitted, t96 runs clippy assembly-ui + `flyout failed_mate`): a failing/inactive mate's
flyout lists every row unticked (assembly_view.rs mate_flyout `flagged`), title wraps at 400 px (assembly_flyout.rs
MAX_W, head 26 + 18/line), root badge 12 px at right/bottom −3 (live 7 px into the icon); test
a_failed_mates_flyout_lists_every_row (assembly_feel_tests). Tangent: live "Tangent propagation" only (ours the type).
ERR-MATE-HOVER-GLYPH: blocked on asm/core: a failed mate has no frames (eval.rs ~3392 `continue` before solving, so
MateStatus.frame_a/b None and no glyph); tell lead "route to asm". UI half (orange part edges, not tint) after that.
ERR-SM-BAD-SELECTION-RED: flange joint error is err_at(["edges"]) with invalid_input false, so `failing` should redden
the selected edge (mod.rs ~1557) — qa sees grey entry + no red edge: repro in a gpui test (synthetic flange on a bend
edge) before changing anything; LIVE asks the entry text red (#AA252D?) too.
ERR-CHROME-TIP-PLACEMENT: asked live for absolute tooltip rects (REQUESTS 04:26).
04:45: t93 failed 2: (1) new row initials box missing (features::get None for custom types; now uses the dialog's
ty.name); (2) the gpui-pre TooltipPressGate change broke "edit / a double-click keeps the row's name tooltip" (wf10)
and didn't fix the new row: REVERTED div.rs + app-core ui.rs to main, new row gap: true, FEEL-ROW-TIP-AFTER-DBLCLICK
reopened, live asked (REQUESTS 04:40). t94-t96 never ran (pgrep -u guard waits on other agents); all in t97 now
(run-t97.sh: clippy scene/render/psui/asm, edge_looks::, print_failed_tool, asm flyout/failed_mate, psui list/extrude/
flange/custom tests).
04:55 CUSTOM-FEATURE-ROW-ICON done after t97 started (needs a run: app-core clippy + custom_feature): ui.rs
initials_row_icon(name, failing, selected) 17×15 in a 20 slot, #999 never red, 10 px #333 / #AA252D, semibold selected;
initials_icon (dropdown) 20 px r3 11 px #333; initials_icon_status/initials_border removed; both mod.rs rows use it.
NEXT after the land — live hover-matrix recheck (live/psui/hover-matrix-recheck-1123/, all 12 ≠ are ours): new line's
end dot UNDER the lit target (#D6BD95, #C3B89C under the point's disc); line away from midpoint blue through
(#CA9E96); Fillet picked strip core uniform #FFAF25 between #FFA90A rims, 1 px dark tangent line 3 px above, no #FFC685
along it; MC soft rim (darkest #1A3F56..#304F61), no navy square, fill #F4DF9A / #F6DB83; assembly MC owner edges
#D4AC63/#F5CD83 at that zoom.
Asked live (REQUESTS 11-23): PS-PROPERTIES-DIALOG-SIZE, CUSTOM-FEATURE-ROW-ICON box size/colours.
ERR-TOOL-BODY-COLOUR (P0, next after the lands; live errors-1123/extrude.md rows 10-13): the failed tool body is an
ordinary body recoloured in behaviors.rs display_for_scene_inner (~3156: TOOL_PREVIEW_FAIL 0x8a3a3f α 0.9, its edges
emptied, plain shade()); live (114,77,79) body, (141,68,73) ring, region ORANGE OUTLINE ONLY (ours: orange 0.34 fill,
builder.rs ~1522). Plan: Highlighting flag "regions outlined" from tool_missed()||Error; fit the colour by a GPU test
(render the miss, sample the body) not by guessing; the ring is probably the body's edges in red (ours drop them).
Open error rows: ERR-SM-BAD-SELECTION-RED viewport red joint edge (repro qa-scratch/err/c/cells/sm-fl-joint2.json),
ERR-CHROME-TIP-PLACEMENT, ERR-TOOL-BODY-COLOUR (live tool (113,76,79), ring (141,68,73), region outline only),
ERR-MATE-HOVER-GLYPH + ERR-LIST-TOOLTIP-CHROME (rebase on asm's a936c3a6 first), ERR-MISSING-SELECTION (core's flag).
PARTLY rows: CORNER-BREAK-VERTEX-PICK, CONFIG-FILTER-HELP, TRANSFORM-ROTATE-EDGE-SENSE rest, FEEL-EDIT-BODY-SHADE rest,
TRANSFORM-PLANE-HANDLE X/Y, TRIAD-LOOK, TRIAD-RELEASE-FOCUS, PS-BOX-DRAG-HOVER, ROLLBACK-CLAMP-HOVER.
USER P0 Mac mini: test APPEARANCE-PART-UNDER-FACE-COLOURS once dx lands, user's doc on the 700 copy
(~/tmp/psui/macmini-m6.noshape), then delete it and /root/psui-scratch/Mac-mini-2024.step.
Queue P2: FEEL-ROW-TIP-AFTER-DBLCLICK, UIS-LIST-ROW-MENU-ROWS, CUSTOM-FEATURE-ROW-ICON, UIS-FEATURE-INITIALS.
Context rule (lead 03:05): sed -n ranges ≤60 lines and grep only; never cat shaders/whole files/test logs.

## Agents

- skasm owns the hover matrix's sk/* cells (lead 11-22); psui the 3D / Part Studio cells.
- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
