# psui (Part Studio UI): current state
Updated 2026-11-25 08:3x UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
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

## RESUME HERE (11-24 20:17 UTC; brief /root/lead-scratch/prompt-psui-1124.txt)
- Stack re-queued as 5175c04d, batch 1 as aeddb0f4 (landed, edb5cd135 on main), batch 2 as e4612e55, batch 3 queued
  now (f22bb2b4f before rebase).
- PS feel-coverage audit (docs/parity/ps-feel-audit.md; tests ps_audit_feel_tests.rs `audit::` + hover_matrix):
  covered before 142 of 164, after batch 1 153, batch 2 159, batch 3 162. Left 2: Hole icon rows (per-row crops vs
  live, not cheap), DCLICK-HEAVY (lead 11-24: skip).
  Batch 3 fixes: a feature's own preview (faces, edges, vertices it made) resolves hover and click to the nearest face
  of `ui.preview_before` along the ray unless the dialog holds it (mod.rs before_face_under / preview_hides; face_hits
  skips bodies whose box the ray misses); dialog checkbox labels mr 6 (cut at 174 px, was 180).
  Tests: a_fillets_preview_band_picks_the_faces_it_was_built_on (sample x 32 mm: the radius arrow at x 20 eats clicks;
  5° from the top tangent, 15° hits the picked edge's band), delete_parts_preview_outlines_only_the_silhouette,
  checkbox_labels_start_20_px_in_and_cut_at_174_px (Derived is "importDerived"; label selector cbl-<path>).
  a_failing_feature_patterns_direction_... flaked once on a loaded vibex8 (white frame), passed alone.
  Tests note: a field made active by user_focus_query alone is taken back by the focused field's tab stop on the next
  render: focus `tab_stop_handle("q:<path>")` as a click does. The viewport's context menu is `view.context_menu`.
  Batch 3 queued as b0f71099 (20:2x).
- DONE 11-25 00:4x: interaction latency reported (lead: "great table"); queued 65d02d59 (latency_tests.rs: T2
  everyday_interactions_on_a_corpus_part, NOSHAPE_LATENCY_DOC; timed_applied). Extrude evCollision slowness → core.
  Mac numbers: impeller hover 8.5 / click 3.8 / dialog 17 / field preview regen 215 / ✓ 6.7 s (core regen) / undo 9.
  Corpus doc: vibex8 ~/tmp/psui/impeller.noshape, Mac ~/noshape-host/psui-keep/impeller.noshape.
- 11-25 05:3x: CI flakes QUEUED 8ae4490c (lead told). (1) the short press waits for manip_hover == Center; (2)/(3)
  render::worker::full_frames_only(true) in both crates' gpu_pointer (Windows' software GL drew a 1x frame, the full
  one 150 ms later). Open: (3)'s failing frame had no amber band - check a 1x frame draws the body band if it recurs.
  Minor bug (not fixed): the frame cache keeps a seq's 1x image on software GL. macOS: kernel loft test, not mine.
- 11-25 05:5x-06:0x (lead): LINUX-MIME-ASSOC-HARDEN QUEUED 2dc33a82 (install.sh: mkdir ~/.config before xdg-mime, the
  mimeapps.list default written when xdg-mime didn't, mimeinfo.cache by hand without update-desktop-database;
  .github/release/check_install_linux.sh from release.yml: the lead's workflow_dispatch dry run pending).
  FEEL-TAB-TO-FLIP-FOCUS QUEUED 6a116778 + the count line (pattern_flip_focus handle on "pat-opp", Tab focuses it; the 1 px blue
  count line via QuantityInput companion focus), count line QUEUED 3126085b.
- 11-25 07:4x latency pass DONE (lead 06:1x), reported. 3126085b + 2ad72895 FAILED on rebase conflicts (lead 07:5x): rebased
  clean onto main 07:5x, re-landed (see QUEUED below / land.sh --status). Ours (local Xvfb :396, llvmpipe, debug):
  start page 0.45-0.49 s; small doc shown 0.78-0.94 s; impeller warm 0.57-0.68 s, cold 58-77 s (kernel SPEED-IMPELLER:
  Boolean Flansch+Gehäuse 21 s, Delete face 1 31 s); tab switch 50-75 ms to first frame (live 1.7-3.0 s settled);
  rename/undo/redo frame 54-70 ms; save (commit_tabs, impeller) 7.5 ms (serialise 2.0, write 5.5). Fixed: page open
  dispatch 163-192 -> 13-16 ms (GPUI pipelines cached per device, vendor/gpui-pre-wgpu cached_pipelines); thumbnail
  double regen (confirmed: regen at 60 s stopped 8 s later when the view showed). bc3f3f817.
- qa partly rows: PARTS-ROW-TOOLTIP (with_list_row_tooltip stays + tooltip_after_press; matrix row "parts / a row
  clicked into Boolean's field") and CONFIG-FILTER-HELP (help title at once, expanded at 1.97 s as live) b8d36439f.
- DONE 11-25 09:3x: shortcuts matrix (shortcut_matrix_tests.rs, `shortcut_matrix::`): live's key map (shortcuts-1123
  api scopes all+partStudio / all+sketchEdit) and every other key live pressed, resolved through our keymap
  (`actions::key_bindings_for(mac)`, now pub) in the real context stacks (Part Studio, Extrude dialog, sketch, Line
  tool), Windows and Mac; each matched action must have a handler there (Workspace-level ones excepted). All match;
  only Enter (Accept, commits what is open, never the sketch) and Tab/Shift+Tab (focus walk) are ours beyond live's map.
  Also b9c2f2423: a_circles_rim_lights test waits for the build + centre pick (failed core's 08:44 land under load).
- DONE 11-25 10:xx (lead): featurelist-1125 vs ours: all as live (no delete warning, reorder above a dependency moves
  then fails red, F2 nothing, empty name -> lowest free default (ops rename_feature), rolled-back/suppressed rows #999 =
  #333 at 0.5). New list matrix rows: undo/redo of a reorder, a rollback move, a menu Delete with dependents.
- LANDING: readout #666, drag pills, Use glyph landed. 26251e31 (shortcuts) failed on a rebase conflict (09:24);
  rebased clean at 10:06 and queued as b8e0a0b5 (rim test, shortcuts matrix, featurelist rows + handoffs). Await the lead.
- 11-25 11:2x: b8e0a0b5 (shortcuts) had landed (05bd974f0). Re-landed the asm UI batch (a38d25e2) + FLOW fix after a
  clean rebase; see land.sh --status. FLOW-REGION-UNDER-FACE-PICK fixed: before_face_under (mod.rs) also ray-casts the
  shown sketches' regions (nearest_region_hit) and returns the region when it is on or in front of the nearest face
  (5e-5); covers hover and click. Test sketch_hover_feel_tests a_sketch_region_on_a_rim_face_wins_over_the_face;
  ::hover_matrix:: ::dialogs:: ::sm:: ::feel:: green (175). TICKETS row marked fixed, to verify.
- 11-25 11:4x named views = live display-modes-1125 (PS + asm + op render_named_view): blank name saves nothing,
  a used one shows NAME_IN_USE (red alert, text kept, gone when the field empties or Enter on it empty), creation order
  (no sort), Shift+V/menu open-only (open_named_views focuses the field), 700 ms restore. Tests
  named_views_refuse_blank_and_used_names, a_named_view_is_no_undo_step, e2e assembly_named_views. Display modes: menu
  rows per mode and per-tab session-only mode already = live (camera_menu.rs, opts per view, not saved).
- asm UI batch (lead 10:0x: asm's open P0/P1 UI/feel rows, no solver internals) committed: Insert split button white,
  row untinted under it (quiet ids in item_row), OsSelect hover = blue underline only (set_hover_underline(false) for
  Insert config selects); mate dialog accept_pale() (reopened "not consistent" pale) and field_text() (Tangent's
  inferred picks read "Face of …"). TICKETS 778 + 694 "fixed, to verify". Not taken (solver): ERR-MATE-CREATING-REV-RED,
  ERR-MATE-OVERDEF-BOTH (eval.rs ~870 held_back). Left: IMP-ASM-ROW-ICON (import), ASM-PATTERN-PREVIEW-LATE (speed).
- 11-25 11:4x display-mode pixels vs live display-modes-1125 (1): study hover_matrix_tests print_display_mode_pixels
  (T2, --ignored --nocapture; live's block, iso 0.290 mm/px). Faces + tangent edges = live; outline heavier ->
  TICKETS LOOK-OUTLINE-WEIGHT (P2, mine, open). Fixture note: a REMOVE extrude from Top needs "symmetric": true
  (THROUGH_ALL / midplane missed the block). Live asked (REQUESTS 11-25 11:5x, edge ink profiles across slanted
  edges) before touching the line shader. QUEUED: 8ff1c3dc (FLOW fix + asm batch), 16f8487c (named views); commits
  7a13baed6 + 131779706 (study, ticket) local, land with the next change. Lead not told (was 865k): tell on next
  report: lands + solver rows not taken + LOOK-OUTLINE-WEIGHT. NEXT: asm rows IMP-ASM-ROW-ICON, ASM-PATTERN-PREVIEW-LATE.
- 11-25 12:0x: 6089774b2 IMP-ASM-ROW-ICON (subassembly rows never show "Has degrees of freedom": live behaviours
  "DOF display"; fn shows_dof_glyph) + ASM-INSERT-FIRST-FIT-BESIDE-DIALOG (fit_inset(): the Insert dialog is not left
  out, a feature dialog is); ASM-PATTERN-PREVIEW-LATE was already qa-verified (dropped its open repeats + other wf23
  repeats). Test note: the test window doesn't redraw the list/front slices after the evaluation lands, so the row
  and fit tests check the rule (shows_dof_glyph on the evaluated nodes, fit_inset()), not pixels. Queued with the
  study/ticket/handoff commits: QUEUED 17670d52 (carries 8ff1c3dc + 16f8487c). Lead not told yet (883k at 12:1x):
  lead: solver rows go to skasm (lead tells it). 12:0x ASM-CHANGE-CONFIG-PANEL-LOOK fixed (os_ui
  user_select_field_flat_focusable; ConfigSession.collapsed; Generate only with a non-list input). Queued 301da71f. 12:1x
  ASM-INSERT-THUMB-CONFIG fixed: the thumb cache keys (studio, configuration, part); the Insert dialog asks for its
  chosen configuration (insert_dialog_thumb, stale_insert_thumbs); the Change configuration panel still shows the
  default pictures. 12:2x 17670d52/16f8487c had failed on a handoff rebase conflict: rebased clean, the whole
  stack re-queued as QUEUED f9cd315a (12 commits; lead at 898k, not told). Keep handoff edits in their own last commit.
  13:0x lead: assembly feel-coverage audit (docs/parity/asm-feel-coverage-audit.md): 60/69 -> 68/69 rules tested,
  7 tests; fixed rename Tab + graphics click not committing. Left: limit clamp on drag (solver; asm/skasm).
  13:1x lead: EXPORT-ASM-STEP-MBD-FIELDS fixed (export.rs mbd_fields_shown(s, assembly); test
  an_assembly_step_export_has_no_mbd_fields in sketch_ui_tests). Live answered edge ink (live/psui/edge-ink-1125):
  numbers and plan in TICKETS LOOK-OUTLINE-WEIGHT. NEXT: LOOK-OUTLINE-WEIGHT (profile ours, then fs_line / width).
  13:2x qa rescore 8b55eeb86 left PARTLY (mine): SKT-USE-HOVER (glyph 14x13 vs 8-9 px, hover no orange),
  TRIAD-LOOK (sd 41 vs 12), CONFIG-FILTER-HELP (s3 title only); after LOOK-OUTLINE-WEIGHT.
  13:3x QUEUED 998aa0bf (export MBD fix + audit + stack); f9cd315a failed 13:29 on the handoff, rebased + re-QUEUED 455eb8d8 13:4x. LOOK-OUTLINE-WEIGHT: unlit part edges at PART_EDGE_INK 0.8
  (render scene.rs + shaders EDGE_INK, params.x bit 6; builder.rs / asm_scene.rs .part_edges()); offscreen
  part_edges_carry_lives_ink now 0.62 by its measure + peak <= 0.85. Wide run ~/psui-scratch/dm/t9.out (render,
  scene, ps-ui, asm-ui + clippy). NEXT: land it, then SKT-USE-HOVER / TRIAD-LOOK / CONFIG-FILTER-HELP.
  13:5x ink 0.85 (band-off run: band adds ~0.08 only, kept); temp switch removed; edge_looks now a luminance bound
  (1x 46-72, 2x 16-72), offscreen 0.66 + peak <= 0.92. Run ~/psui-scratch/dm/t11.out (render, scene
  --no-fail-fast, profiles, clippy). 0.85 profiles: a 1.07 col, p30 1.09, d 0.85. edge_looks hovered core then
  failed (the plain edge under a lit band was opaque): shader style bit 20 = part edge, its 22 % share x EDGE_INK.
  t12 green (4 crates + clippy). 14:4x QUEUED cd7a6606 (edge ink, parts menu material items, TICKETS, handoff);
  455eb8d8 is on main. Lead at 941k: not told (report cd7a6606 + 455eb8d8 when < 200k).
  14:5x lead: continue SKT-USE-HOVER (glyph size / orange hover), TRIAD-LOOK (spread sd 41 vs 12),
  CONFIG-FILTER-HELP (expanded at once at s3), then live's everyday PS / assembly / first-user captures vs ours.
  15:0x SKT-USE-HOVER: live's on-edge use glyph (s23/full/use-use-1-edge-done, use-3-hover-used-edge) is a 9 px black
  ring (2 px) round white, no tile; hovered a 12 px #ffc685 disc. sketch.rs glyph tile: `ring` (on_curve) branch
  draws that (selected #fab601). Green; QUEUED with cd7a6606 (carried) at 15:0x.
  15:0x lead (qa b92222847): FLOW-REGION-UNDER-FACE-PICK partly: opening + rim regions pick as live, but a click
  on the outer ring region beside the box (-1, 20, 30) still adds "Face of Extrude 1" (live: Face of Sketch 2): a
  region outside the part (over nothing or a side face) must win too.
  15:1x: fixed (preview_region_under in mod.rs: under a preview the GPU pick there was empty and the click's reach
  took the side face; hover + click now take the region when no before-face is in front). FEEL-TAB-TO-FLIP-FOCUS
  second direction fixed (assembly pattern_flip_focus is [FocusHandle; 3]). Both tests green on vibex8.
  16:00: QUEUED 14539067 (0a0afebb2 code + a3a594b9d TICKETS + handoff, on be01ac8f): FLOW-REGION outer ring,
  pattern Tab per direction, part paste under the pointer (waits for its first solve: ui.drop_on_result), suppressed
  menu keeps Where used…, IMP-FACE-APPEARANCE-ROWS (appearance.rs imported_face_rows, one row per import and colour),
  TOAST-QUEUE (app-core toasts.rs ToastQueue + pump in each view's render; workspace time_toast), Edit/Assign
  material… literal labels (ops coverage; 0a53127fc in be01ac8f's stack lacked them). Lead told.
  Capture items, all = live: BOM (bom.rs walk: suppressed out, hidden in); new-doc units from Preferences; sketch ✓ one
  undo step + re-entered sketch empty stack (test a_finished_sketch_is_one_undo_step…; Ctrl+Z outside a dialog
  propagates to the workspace's Undo, so the PS-only test window can't take the key); asm: fixed-part drag adds no
  step, one undo takes a Fastened mate and its move (tests, landed after 14539067); Insert session one step was
  already covered (asm-feel-coverage-audit). Full assembly-ui lib green (212).
  STATE 17:12: mistakes rows + manip dead zone COMMITTED (7af9d907f code + mate, 46cc1c84a arrows, 23d3c0564 TICKETS
  MISTAKES-ROWS / MANIP-DRAG-DEAD-ZONE "fixed, to verify"); t9 green (app check, part-studio-ui lib 614, model 61),
  t10 green (an_arrow_drag_writes_its_field_and_undoes_in_one_step, the_dialogs_matrix, an_edited_mate_down_to_one_
  connector_says_it_needs_two, a_reopened_inconsistent_mate_has_a_pale_check). Landed with this handoff (see below).
  Mistakes: failing_alone (detail.rs: a failed Fillet/Chamfer blames only picks that fail alone, <=24 retries, not in
  scratch runs); ui_state query "redItems"; has_export_geometry (parts/surfaces/composites); mate dialog clear_slot
  (x on an edited mate = touched: ✓ enabled, title "Mate needs two mate connectors."). Hole title "↧ 12 mm" = std's
  default (live's THRU was the account's remembered End).
  17:4x (lead): Move face arrow at the click point: ui.face_clicks (behaviors.rs face_click_point, mod.rs
  note_face_click on a graphics face click, ray hit on the model before the preview), manipulators moveFace OFFSET /
  Translate base; T1 move_faces_arrow_starts_where_the_face_was_clicked. Not checked: live commits Extrude's dragged
  depth as the 1-decimal value ("30.3*mm"), Fillet's at full precision. Parts 3+ (ring, triad, asm drags) from live.
  17:4x: arrow halo 3 -> 4 px (live wf23 03 s19 + manip-drag-1125 cross-sections), starting where the core does
  (an_edited_extrudes_end_cap_greys_its_top_once caught the overhang); TICKETS FEEL-CORNER-BREAK-VERTEX-PICK to verify.
  manip-drag part 3 (live 16:50-17:06) compared, no code change needed: Translate one arrow + Direction defaults to the
  face (behaviors.rs moveFace logic), Rotate ring only, triad keeps the focus (dialogs test), dialog Ctrl+Z goes to
  the draft history (resets dx), assembly drags start past 2.5 px (live 3). Untested: a T1 for the triad's dialog
  Ctrl+Z resetting dx in one step; the Rotate ring's position vs live (1175/1185,435).
  Other PARTLY rows checked: the rest are "fixed, to verify" / CAN'T TELL / [x] (FEEL-FS-COMMIT split button is studios').
  18:0x (lead 17:5x queue): ASM-CHANGE-CONFIG-PANEL-LOOK (qa PARTLY): no rule under the doc row; the select overflowed
  its row by 4 px (label flex_none): label now flex_1 truncate, select mr 14 (27 px from the right edge) and top -3;
  test asserts the 27 px. Dead zone 4.1 px for arrows AND angle rings (live: Fillet moves at 4.47, ring at 4.24, not
  2.83; arrows 4 none / 5 full); T1 move_faces_rotate_ring_snaps_by_5_degrees_and_shows_7_decimals_between (edited
  Move face ROTATE about edge [20,0,20]; field focused + selected, dialog Ctrl+Z no change, PS undo). Green: t14
  (asm test), t15 (ui_tests::dialogs:: + snap_tests::, 26). Open: live's ring sits ~25 mm from the axis at 0 deg
  (hits 1175/1185,435 = half the face box diagonal? std's on-axis branch), ours at the face anchor's radius (15 mm
  here): needs a live probe of the ring at other clicks before changing. Live's "plain click on a part picks nothing
  in Transform until hovered" is lost input (DECISIONS 11-07): not copied. Triad: no field focused + dialog Ctrl+Z
  resets the drag in one step: ours, tested (the_translate_triads_xy_square_drags_x_and_y_and_keeps_the_focus); asm drag 2.5 px (live 3: 2 none, 3 moves), no box, one undo (asm tests).
  18:1x (lead): Transform plain-click = lost input, agreed not copied. Ring position filed: REQUESTS "psui 11-25 18:10"
  (live/psui/manip-ring-1125; then make revolve_arc's moveFace anchor match). Asm 2.5 px threshold kept: live's band
  is (2, 3] (2 none, 3 drags), tested by app-core presses_drag_and_open_the_menu_as_live. LOOK-OUTLINE-WEIGHT: on main
  (f6da23b36 PART_EDGE_INK 0.85), "fixed, to verify" for qa; nothing left for me.
  maker-io-1125 DONE (queued 86ade867): Derived multi-part pick fixed ("tab#p1,p2"); names/Custom fields tested;
  bubble no change; core rows routed by the lead. Open: live's click on an already-picked part (ours: no change).
  18:37: FIRST-WEEK-ISOLATE + FEEL-ASM-ORIGIN-ROW-HOVER already = live, T1s added (queued 5a6910ab).
  18:4x (lead): hover matrix 7 cells (qa b27f45603 on green e34c6b838): as-mc-selhover already fixed 950a10c7e and
  qa-verified 13:2x (131/137); the other 6 (b1-mid-hovered[-other-selected], ps-linemid-hovered[...], ps-arc-hovered[...])
  are live's single-jump cells (feel doc: "not scored"; behaviours l.270-273): qa should score them with a stepped
  approach (live mid-paths shots). Combined states: rows in interaction-feel.md "Combined states" (2 = live b3, 10 "?")
  + REQUESTS "psui 11-25 18:41: hover in combined states" -> fill + qa scores when live answers.
  Ring (live manip-ring-1125): POSITION done (commit "Move face's Rotate ring sits at least 60 px...": ArcHandle.min_px,
  at_scale(view) on projected length, re-synced in render_viewport_underlay; test move_faces_rotate_ring_keeps_60_px_from_its_axis).
  Arrow: d133237f4 (click point) REVERTED (live's frame shows the shaft at x 1160, 9 px right of the click = the face's
  middle); test move_faces_arrow_starts_at_the_faces_middle (TICKETS MANIP-DRAG-DEAD-ZONE note corrected).
  Ring LOOK done 19:0x (593effc30): live's "47 px arc" is a straight stem + hollow head (iso: stem 24.5, tip 40 px up,
  17 across), radius line as wide as the stem, 15.5 px past it; ArcHandle::glyph_px (RING_* world px, min_px > 0
  only), viewport.manipulator_across (per handle; arrows MANIP_REACH_ACROSS_PX 14, ring 12), reach = drawn + 3 px.
  Tests move_faces_ring_head_is_drawn_as_live (hover_matrix, GPU) + move_faces_ring_grabs_its_head_and_3_px_round_it
  (t18.sh). Revolve/Draft arcs unchanged (no live reach data). Seen in passing: in our Move face dialog the picked top
  face turns a4bebf with orange edges, live keeps 9bc2d9 (b1 frames): unchecked, maybe a combined-state row.
  NEXT: 2) LOOK-OUTLINE-
  WEIGHT: our fix (e8b29bf7b, PART_EDGE_INK 0.85) is in the queued stack (land jobs cd7a6606 .. b0e47f46 pending on
  vibe2; origin/main still 9996450a4 15:39 at 17:0x); verify after it lands. 3) PARTLY TICKETS rows
  (~l.785 split-button ▾, 812 arrow outline, 828-831 filter popover / Tab focus).
  Parts-list menu: Edit material… / Show center of mass with a material (mod.rs; test
  a_part_with_a_material_offers_edit_material_and_its_center_of_mass in list_feel_tests, green).
  To compare (live, no ask yet): live/psui/everyday-1125 (undo scopes: sketch ✓ one step, re-entered sketch
  empty stack; Ctrl+V of a part: nothing in a PS, inserts under the pointer in an assembly) and
  live/psui/asm-everyday-1125 (drag = one undo step, fixed drag none, Insert session one step; hidden row
  opacity 0.5, suppressed strike + italic + 0.5, suppressed row menu, BOM counts hidden not suppressed).
  live/psui/first-user-1125 (new doc: no first-run hints, empty PS toolbar all enabled but Undo/Redo, tooltip
  short <=62 ms then expanded 400 px ~2.2 s, new doc units = Preferences, no units field in the dialog).
- Queued d97311a3 (Use glyph, pills, readout #666 + the 460f1fe4 batch).
- Re-landed after the rebase conflicts: QUEUED 460f1fe4 (11 commits: MC P0, count line, latency fixes, the two qa
  rows, handoffs). Result from the lead.
- SKT-USE-HOVER fixed (bb472f3b1): a used curve's own use glyph always drawn, centred on its middle (Show constraints
  off; live use-convert: 10x8 on the edge at y 69.5 of an edge at 69-70); Use over it: glyph under the pointer ->
  hover_constraint, edge hover None. Test use_over_a_used_edges_glyph_lights_the_glyph_not_the_edge. TICKETS row
  "fixed, to verify".
- Live pages 11-25 checked: featurelist-1125 (drag pills 14 px/500 31 tall at +10,+6 rollback / +10,+5 feature, drop
  dot 6 px: 1fd0df44d; bar colours, rename, empty name, delete without warning already as live), measure-1125 (value
  #666 not #333: 0d6298e21; assembly_view.rs:3097 has the same #333, asm's crate, left), section-1125 and
  appearance-1125 (already as live). core/dialogs-1125 (08:4x): fields per option/checkbox/tab of Extrude, Revolve,
  Fillet, Chamfer, Shell, Linear/Circular pattern, Mirror, Boolean all as live (schema probe, two levels deep:
  /root/psui-scratch/dlg/probe_tmp.rs, run by copying into model-data with `#[cfg(test)] mod probe_tmp;`, then
  removing); Fillet's "Edges to keep" folded under Allow edge overflow as live; empty-dialog texts and Mirror's auto
  Add + merge scope already there (a_mirror_across_a_side_face_adds_into_its_seed). Live's Fillet 3 mm / Chamfer 1 mm
  are that account's remembered values.
- 11-25 07:1x P0 (lead): as-mc-selhover regression from ae3a5f96a fixed: PointSprite::FLAT_RING (2048) keeps the
  old flat 18 px #6087bd ring on a selected mate connector; test mc_selected_hovered (60/0/58 px). QUEUED 3e10a3a2
  (with the count line 3126085b and two handoffs), reported to the lead.
- 11-25 05:3x TRIAD-LOOK round 3 (qa 8641c8b2a, a lighter px every ~7 cols): qa's "bluest px per column" is the
  outline rim, not the core. Live (psui/manip-feel-1122 transform-x-hover) ~4 px strokes, 1.45 px core, rim ~1.3 px
  each side, per-column rim sum median 334 sd 12 range 301-365; ours (3 / 1.5 px) a 0.75 px rim, never full. Now 4.5 /
  1.4 px: median 346 sd 9 range 346-387. Test checks a full outline px beside the core at every step (old: 19 of 29
  fail). Scripts /root/psui-scratch/triad2/{core,prof2}.py.
- 11-25 01:0x-01:3x, all awaiting qa: TRIAD-LOOK queued ec758926 (LineSegment::SMOOTH, render: a line antialiased
  across its width; the triad's strokes); FEEL-ASM-FIXED-GLYPH-AA + FEEL-ASM-MOVE-RING-RIMS queued together (asm parked;
  lead 11-25: take parked agents' Part Studio/assembly viewport feel rows of the same renderer class first). Ring:
  STROKE_RING|SOFT_CORE now 20 px, hole 0.58, #3a6ba8 rims, #99b2da core. section_manip.rs's arrows (3 / 1.5 px) are
  drawn like the old triad: give them SMOOTH if qa sees steps. Next candidate: FEEL-MC-MARKER-LOOK-ASM (TICKETS ~162,
  cap-centre markers 6x3 white vs live ~12x6 discs with a grey ring).
- a2341555 (skasm's stack) FAILED an ops parity test (Text letters' counter): skasm owns that land now (lead 11-24
  16:xx); don't touch psui-skrebase. eaa20b28 LANDED as 9d6b9fc61.
- Landing (14:2x, land-edb.out): TICKETS one row per psui ticket; list matrix row "a row clicked into a dialog's field
  keeps its tooltip" (passes: FEEL-ROW-DEPENDENCY-TIP-IN-DIALOG to verify); FEEL-EDIT-BODY-SHADE fix: an edited
  feature's (EditSession !is_new) preview bodies keep their color and rest shade (Instance::preview_lit: preview
  layering, part lighting; SceneOptions.preview_lit; preview_keeps_part_look), the end cap #595959 at 0.5 drawn in
  both frame and "with" layer (= over the mix). Test an_edited_extrudes_region_shows_through_its_sides_as_live (plain
  block, live edit-body-slider-1124 points). Left P2 in the TICKETS row: y=30 region edge outline through the right face
  (live none), datum planes darker at slider 0, region tint B -9 on the right.
- Windows CI: a_hover_after_a_rest_ + triad hardened (landed 9d6b9fc61). a_cleared_hover_...: read the next Windows
  main run's frame_diff message (run after 9d6b9fc61). the_pointer_lights_...: never failed on main CI; release run
  45375844269 job 138469739575 still in progress 13:5x (`gh api --allow-escape-sequences .../jobs/<id>/logs`).
  asm's the_second_pick_outlines_... failed once on Windows (told the lead).
- Told the lead (13:5x): qa score-a910 b3x-mc-radius / b1-midapp-sel-face mispaired.
- LANDED 0923c1fd as ad784f89 (16:43): owner-selected hover cells, Fillet handle bisector step, sm tab test T2.
- RE-QUEUED 17:54 as 75404bb7 (whole stack; 8903d37f failed 17:45 on a SIGTERM of the part-studio-ui lib binary, no test failed: earlyoom-like, same second slot 1 finished): depth-0 Extrude region outline red (live errors-1123; builder outline_corners:
  corners = a sketch curve end or a turn > 60°; test a_depth_0_extrudes_circle_region_has_a_red_outline); field
  errors = live (test feature_fields_reject_what_live_rejects); flip buttons (hover 1 px #1651B0 bottom line, arrow
  cursor; test the_flip_button_flips_on_click_takes_the_keyboard_and_ignores_space).
- Corner-break disc (FEEL-CORNER-BREAK-VERTEX-PICK): sheet metal, PARKED by the lead: test is T2, don't debug. Known:
  the scene holds the disc (layer 20 Points, POST, over_lines, mix 0.7); the renderer's preview-post pass drops it.
- IN 75404bb7 (was 8e11c924): FEEL-HOVER-OWNER-SELECTED (3): a hovered vertex at a selected edge's
  end gets a 6 px piece of the edge's band over its disc (behaviors vertex_band_piece, layer 21 after the disc; the ON_TOP
  disc writes near depth, so a depth-tested redraw can't cover it); TICKETS dup rows of SKT-USE-HOVER /
  SKT-POSTCOMMIT-HOVER dropped.
- skasm's 7e9bedfb7 (after its b99589dc) adds LineSegment::dash_from: new LineSegment literals need `dash_from: 0.0`.
  My vertex_band_piece literal (in 8e11c924) predates it: whichever lands second adds the field.
- QUEUED 53737a22 (df1e61e44) + 59a849f1 (empty-Loft title tests), stacked on 75404bb7: FEEL-ROW-TIP-AFTER-DBLCLICK per live row-tip-dblclick-1124: a
  double-click holds the row tip (ui.row_tip_held, cleared on leave or entering another row: the leave is lost
  while the dialog opens); a quick one (row_early_press) shows ui.row_name_tip (app-core ui::row_name_tip,
  typed_name, 300 ms fade) while a dialog is open. Also test an_empty_lofts_title_tooltip_names_its_error: the
  real flow already shows Loft 1's error tooltip (qa ERR-LOFT row 648's "no tooltip" not reproduced; ask qa's flow).
- 8903d37f FAILED 17:45 = SIGTERM of the part-studio-ui lib binary (no test failed): re-queued as 75404bb7.
- Other open rows checked 11-24 17:4x: all "fixed, to verify" (qa), sheet metal (parked), or skasm's.
- Then: PARTLY rows in TICKETS (TRANSFORM-*, PS-BOX-DRAG-HOVER), P2 queue
  (PS-PARTS-ROW-TOOLTIP-PITCH, CUSTOM-FEATURE-ROW-ICON). Sheet metal parked (ERR-SM-BAD-SELECTION-RED waits).
- qa's hover-matrix score: /root/qa-scratch/hm2/score-a910/sc8.txt; ascii maps score-a910/bin/ascii.py.

## Agents

- skasm owns the hover matrix's sk/* cells (lead 11-22); psui the 3D / Part Studio cells.
- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
