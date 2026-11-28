# psui (Part Studio UI): current state
Updated 2026-11-27 16:2x UTC. History up to then: docs/handoff/archive/psui.md (not read by default). Standing rulings:
docs/DECISIONS.md (read first; it wins over anything here).

## Setup

- Worktree /root/projects/noshape-wt/psui, branch agent/psui. Commit as yekta (AGENTS.md), subject "psui: …".
- Builds, crate tests and scenario runs on vibe2, one rb.sh at a time: `NOSHAPE_RB_HOST=vibe2 tools/tiers/rb.sh bash -c "$(cat script)"`
  (the script exports OCCT_ROOT=/opt/occt CARGO_PROFILE_DEV_DEBUG=line-tables-only TMPDIR=$HOME/tmp/psui); `git diff --name-only origin/main | xargs touch -c` first. ~/bin/slot
  only for suites, gates, timing pairs (DECISIONS 9e5b481d4). Scripts in /root/psui-scratch/run*.sh. The GPUI dialog matrix is
  `dialogs::the_dialogs_matrix_matches_live` (rows filter: DIALOG_ROW=…).
- Hosts (lead 11-27, vibex6 gone): builds and app/replay runs on vibe2 (with the land queue: one run of mine at a
  time); scenario shards and suites on vibex8 through `~/bin/slot <cmd>` (run-shards.sh points there, an-run.sh at
  vibe2 via slot). Nothing kept on vibex6 survives.
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
  Run: `NOSHAPE_RB_HOST=vibex7 tools/tiers/rb.sh bash -c "$(cat /root/psui-scratch/uip-some.sh)" uip-some <out> <names…>`
  (`~` in remote paths expands locally: pass /home/ngyekta/… paths). Output buffered: the log fills at the end.
- `psui-shards.sh <out> <names file>`: the same in 3 shards, then one `--compare-only` score. The names file must be on
  the host (scp it first). psui-names.txt: the 1021 psui-area scenarios with live takes.
- Compare without rerunning: `python3 tools/ui-parity/run.py --compare-only --out <dir> <names…>` (through rb.sh).
- Runner env: NOSHAPE_UIP_LOGS=1 (app stderr), NOSHAPE_UI_SHOTS=<dir> (a screenshot per step), NOSHAPE_PERF=1,
  NOSHAPE_CACHE_DIR (fresh: cold runs).

## PLAN 11-28 01:28 (after compaction: start here; lead: do exactly this, core waits on inner_regions)
0. (01:3x) Step 1 done: the test's redo + rename-by-new-name steps were unfounded (no live take of redo after a
   rename) and dropped; it now checks the undo undid the Extrude. Open: ours loses a non-undo rename on redo of the
   step before it (redo restores the pre-rename state); ask live for a capture before changing it.
1. Fix the one ps47 failure: ops e2e `a_part_renamed_in_its_properties_is_not_an_undo_step` fails at its
   history_redo + part_set_properties step ("no part `Bracket`; parts: Part 1": the undo undid the extrude/rename
   chain differently than the test assumes). Read the test (crates/ops/tests/e2e.rs), fix the test's later steps
   (live properties-name-save-renames: Save is no undo step), re-run that test only on vibe2.
2. Commit as "psui: …" (code; commit says the "after B" unit test changed because it was wrong vs live
   appearance-part-rgb-escape; the dialogs row now reads the face dialog from state["feature"] (state moved in batch 9,
   expectation unchanged)), handoff last.
3. `git fetch && git rebase origin/main` (batch 10 takes come in). core's 434c2a49 carries my inner_regions hunk
   byte-identical: on a conflict in features/sketch.rs keep one copy.
4. ps48 (resp/ps48.sh, written): clippy pss + noshape, ui-bench build, replay of the 19 batch-10 config/export takes.
   Touch diff files first. Then land (read diff --stat, grep TICKETS rows for duplicates), report to the lead in ≤5 lines,
   tell core when it's queued.
5. Tick qa rows from ps47 (70/93 scenarios, 785/932 states): batch-9 rows 73c1c85edd mostly fixed (only remaining
   batch-9-ish diffs: mass-properties-add-part s5 message "One or more parts do not have a material defined.",
   mass volume 1 mm³ off (spline), measure-panel-spline-face s3 no panel); batch-8 2d235a33de.
- Already in the tree (uncommitted, since ps47 started, untested): run.py seeds "remembered": {"export"} into
  export_solid.json (remembered_export; the 13 export Format diffs), export File name " Copy N" normalised,
  "configurations" in LATER_KEYS; Part Studio state "variables" (config panel inputs, List columns "<feature> ›
  <param>", values per row, typed cell text) and "configurations" (config box) in part_studio/ui_script.rs;
  config_cell marks (config_panel.rs config_cell_mark, mark_first + "<input>|" form), config_param marks (config box),
  app ui_script control_name config_cell/config_param. Section-view prompt diffs: live inconsistent (same Front pick
  clears it in section-view-front, keeps it in -exclude-part): left as is.
- ps47 still open: export-parts-three-individual-files s5 (live "STL Format" first field), mistake-asm-import-step-*
  (2), mistake-section-view-pick-edge s2 (ours Front, live Top: edge [20,0,10] -> adjacent face pick order).

## IN FLIGHT 11-28 00:3x (superseded by PLAN above)
- FIRST: cb495ba6 FAILED in the land queue 00:19: part_studio::sketch::ui_tests::dialogs::the_dialogs_matrix_matches_live
  panicked at dialogs_feel_tests.rs:1271 (672 passed, 1 failed). Fix forward (likely the face-menu / appearance /
  Hide changes of a34bf898), then land with everything below.
- ps46 (resp/ps46.log, bg on vibe2) = batch-8 rescore + batch-9 diffs on the tree as of 00:0x; ps47.sh written (93 takes,
  tests incl. e2e::a_part_renamed_in_its_properties, clippy pss+ops+asm-ui) for all edits since: touch diff files first.
- Uncommitted since ps45 (43/49): panel-row Edit appearance selects the part; face appearance Rename popup reported
  as panel "Rename"; colour editor Tab from B -> Tessellation select (select_focus; unit test "after B, A" changed:
  wrong vs live appearance-part-rgb-escape, say so in the commit); Properties Save rename is no undo step
  (properties_save_is_undo_step(ps, body, v), callers updated, new e2e); section arrow click flips the plane
  (section_manip.rs ToolUp); Section planes picks Face/Plane/Connector only (behaviors.rs pick_kinds: edge click ->
  adjacent face, live mistake-section-view-pick-edge); cylinder Section view… uses section_through; export Custom
  number fields marked field:<label>; Mass properties "Parts to measure" focus once the selection changed
  (ui.mass_opened_with).
- EXPORT-TOAST-DOCUMENT-NAME: live's default File name = "<tab> - <part>", live's start tab is "start block Copy 2";
  check ours' in ps46 before deciding run.py vs ours.

## (earlier, 00:0x)
- QUEUED cb495ba6 (3 commits: batch-9 state + fixes, inner_regions, handoff); ec4c4387 landed before it. core was
  told (it lands its eval.rs Reference::Sketch use of features::sketch::inner_regions after mine). Never poll
  --status; the lead reports. On land: report <=5 lines; cc skasm on SKETCH-COINCIDENT-FREE-LINE-SLIDES /
  SKETCH-TANGENT-ARC-SPAN + REQUESTS "psui 11-27 16:26".
- Uncommitted (ps45 running on vibe2, log resp/ps45.log: pss tests appearance/material/menu/rename/propert, ops
  e2e::material + e2e::mass_properties, clippy pss+ops+app-core, build, batch-9 replay):
  parts-list rename emptied -> toast "Name is required." (behaviors.rs NAME_REQUIRED, cleared on commit/non-empty);
  Material Library select marked dropdown:Library, Escape closes its list before the panel (close_library_list);
  colour editor keeps parts' drawn colours at open (AppearanceEdit.shown/committed_color) so part_data shows live's
  pre-Enter colour; #/A (and R/G/B) fields select_on_focus(false); Properties text fields select_on_focus(false);
  material search Escape clears the search and keeps focus (2nd Esc closes the panel); hex Enter with an incomplete
  colour does nothing (editor stays); material densities from the CSV's kg/m³ (ONSHAPE_MATERIALS 4th field, both
  material_data.rs; ops LB_IN3 removed; mass 2.319 as live); surface face menu: Hide other/all surfaces (of the
  surfaces); after ps45 started (touch before the next run): material panel Custom fields report focus "field" with
  their label (focused_custom_field + ui_script branch).
  Then commit (psui: …), handoff last, land.
- After ps45 (uncommitted, ps46 = resp/ps46.sh: tests, clippy pss+asm-ui, build, batch-8 rescore + batch-9 diffs):
  PS-MENU-CYLINDER-FACE: a cylinder's face menu has View normal to (down its axis, mod.rs view_normal frame_of) +
  Section view… + Curve/surface analysis… (menus.rs `cylinder`, mod.rs cylinder_face); Insert panel ✕ marked
  "button:cancel" (ASM-INSERT-CANCEL closed in TICKETS: only the name); ASM-INSERT-ROW-ADDS-INSTANCE closed as not
  a defect (live's s9 is before its REST round trip; s10 lists it as ours). properties-material-field-click: ours
  already kept Properties open in ps43; recheck in ps46.
- qa batch 9 (TICKETS 73c1c85edd "qa 11-28 00:0x", 20/49 on b497dda05 = before cb495ba6): PART-DATA-STATE,
  APPEARANCE-EDITOR, APPEARANCES-PANEL-TABS, HIDE-CLEARS-SELECTION, PROPERTIES-DIALOG-FIELDS (P2), RENAME-BLANK-MESSAGE,
  MATERIAL-DIALOG-DETAILS (P3): check each against ps45/ps46 and tick the fixed ones in TICKETS.
- sketch-used 0/19: skasm's (lead: SKETCH-USED-CURVES-NOT-SELECTABLE is skasm's P0; don't duplicate).
- live's depth batch 10 (6ca15328 queued, 55 takes: Variables, Variable Studio, Configurations; config_data:
  state.variables/state.configurations, targets config_cell "<row>:<column>" and config_param; export takes carry
  "remembered": {"export": {...}}): ours needs those state keys and targets after batch 8 (README "Configurations").
- Then qa batch 8 (TICKETS 2d235a33de "qa 11-27 23:2x", scored on green 5128614a4, before ec4c4387): rescore on main
  first; P2 EXPORT-CUSTOM-FIELDS-NO-FOCUS, EXPORT-N-PARTS-DIALOG ("Export 2/3 Parts" + 2 checkboxes),
  SECTION-VIEW-ARROW-FLIP (+ {"world"} drag target), SECTION-VIEW-NEXT-FIELD (focus to Items to exclude; edge pick
  doesn't clear the plane), MEASURE-BRACKET-KEY ("[" opens Measure); P3 PART-MENU-CURVE-ANALYSIS,
  MASS-PROPERTIES-ADD-PART-FOCUS, EXPORT-TOAST-DOCUMENT-NAME (run.py drops live's " Copy N");
  ASM-MASS-PROPERTIES-BUTTON (lead's/asm; mine if only the button name).
- Export open: "Include hidden instances in export" still remembered.
- Hosts: vibe2 = builds/app runs (one at a time, land queue shares it); vibex8 for shards via ~/bin/slot.

## EARLIER
- sketch-tool-insert-image s3: retake asked (REQUESTS "psui 11-27 15:42").
- frame-corner-coped-limit s5 and the other focus disagreements: retakes asked in REQUESTS "psui 11-27 06:2x".

## NEXT
- qa's diffs from live's option-combination batches as filed; interaction-feel rows to verify (archive 11-27).

## Agents

- skasm owns the hover matrix's sk/* cells (lead 11-22); psui the 3D / Part Studio cells.
- Agent ids: ONLY in /root/lead-scratch/roster.md (lead rule 11-19); read it before every message_agent, never copy ids
  into notes. Context cap 200k (user 11-19).
- P0 sketch-vertical-key: matches live on green f6f8069f, 10 of 10 states (qa 11-20).

## Open decisions

- None.
