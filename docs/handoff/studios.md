# studios: Documents, tabs, start page, preferences; versions and branches; Variable Studio; Feature Studio

Owner: studios (since 2026-11-07, taken over from psui, skasm and dx). Worktree /root/projects/noshape-wt/studios, builds
on vibe3 (tools/tiers/rb-hosts), display :108, scratch /root/studios-scratch/.

## Why

docs/parity/INDEX.md: four areas had no measurement and scored 0 (20 of 100 points). Each is measured as psui's Part
Studio is: `match × coverage`, from the interaction suite (tests/ui-parity, docs/PARITY-SUITES.md §2).

| Area | Weight | Code | Coverage script |
|---|---|---|---|
| Documents, tabs, start page, preferences | 7 | crates/start-ui, crates/preferences-ui, crates/app (tab bar, document menu) | tools/ui-parity/coverage_documents.py |
| Versions, branches, history, release | 5 | crates/history-ui, crates/release-ui | tools/ui-parity/coverage_versions.py |
| Variable Studio | 3 | crates/variable-studio-ui | tools/ui-parity/coverage_variable_studio.py |
| Feature Studio and the FeatureScript editor | 5 | crates/feature-studio-ui (the language and host stay dx's) | tools/ui-parity/coverage_feature_studio.py |

## How each area is measured

- The universe: every tool, menu row, shortcut, dialog and Preferences control live has in the area, from live's captures
  (behaviours.md, live/dom), in `tools/ui-parity/universe/<area>.json` (one item per row, its source line). Cloud
  features (sign-in, sharing, comments, notifications, App Store, plan upsells) are listed with `"cloud": true` and left
  out of the count.
- Coverage: the share of the universe's items that a scenario exercises (`coverage_<area>.py`).
- Match: the share of recorded states equal to live's (`tools/ui-parity/run.py`).

## State

(log below, newest first)

### 2026-11-12 03:25 UTC: the history panel's frame cost (lead 11-12)
- Timing records now split the first frame into layout / prepaint / paint (`timing.stages`). With Versions and
  history open a frame's layout took 12–38 ms (1 ms on a plain tab) and prepaint 21–66: the panel built every row.
  It now builds the rows on screen and 300 px around them (as the Variable Studio table and the documents list).
  A search listing 2000 changes: a frame 300–350 -> ~60 ms (layout 101–148 -> 15, prepaint 130–151 -> 4); the plain
  history (55 rows) about the same as before.
- Rows not built keep a marked placeholder while a UI script runs (history rows, Show more changes…, the Variable
  Studio's cells), so the driver finds and scrolls to them (ver-show-more-changes, the speed scenario's V10). They
  cost frames in that mode only (the search's frame ~119 ms under a script). The documents list has none (the page
  driver doesn't scroll).
- ver/vs: 210/216, the misses the known live-side ones and ver-create-version-drawings-warning's focus.

### 2026-11-12 00:57 UTC: the first key after a click drew a whole frame first (lead: the 40 ms Escape / Ctrl+A)
- Cause (vendored gpui-pre window.rs): a key that switches the input modality from mouse to keyboard called
  `refresh()` before dispatching, and `dispatch_key_event` draws a dirty window before handling a key: a full
  synchronous frame for every first key after a click, then another for its result. NOSHAPE PATCH (agent studios):
  the refresh comes after the key's dispatch (only hover and focus-visible styling follow the modality). Escape after
  Compare: dispatch 43 -> 0 ms, its frame 83 -> 53; Escape / Ctrl+A in a Variable Studio cell 36-40 -> 0 ms.
  Every scenario with a key or type step (876, all areas) running on vibe3 (~/tmp/studios/uip-keys) before landing.
- vibe3's ~/tmp/studios is pruned of files older than 2 h while its disk is over 80 % (lead's remote-disk-loop.sh):
  touch the app copy and fixtures during long runs, or re-copy them (bin/mkperf.py rebuilds the speed fixtures).
- UIS-RCLICK-WINDOWS: footer.py says the UI script's left clicks don't select rows on Windows either (the footer
  link's click works), so it's the documents list, not the right button. Diagnostic qa-os.yml run 43130109140 on
  branch studios-diag-rclick (page driver logs each click's point, viewport, scale, window bounds, menu state).

### 2026-11-11 23:18 UTC: speed in studios' area (lead 11-11: PLAN 2.7), Show in Explorer confirmed on Windows
Measured with the ui-parity driver's timing mode (`NOSHAPE_UI_TIMING=1 NOSHAPE_PERF=1`: each step's recording gets
`timing` = dispatch / first painted frame / idle, ms from the action) on fixtures from /root/studios-scratch/bin/mkperf.py
(hist.noshape: 2000 edits, V1–V50, B1–B3, 8 tabs; vs500.noshape: 500 variables; docs/: 1000 documents), scenarios in
/root/studios-scratch/perf/, runner bin/perfrun.py (vibe3, ~/tmp/studios; that folder is pruned of files a few hours old:
re-copy the scripts). vibe3 at load 33–41 (8 cores): relative numbers, same run. Live's timings asked (REQUESTS 2518).

| Operation | first frame (ms) | idle (ms) | after the fix |
|---|---|---|---|
| Tab switch (Part Studio / Assembly / Variable / Feature Studio) | 14–63 | 11–560 | – |
| Versions and history open (2000 entries, 50 versions) | 22–114 | 250–750 | – |
| Compare a version / Open a version / switch to B1 and back | 48–138 | 470–820 | – |
| Variable Studio, 500 variables: open, cell click, Ctrl+A, Enter, Esc | 249 / 283 / 507 / 478 / 496 | 480–730 | 42 / 33 / 78 / 73 / 69 |
| Documents page, 1000 documents: sort by Name, click Search | 310–385 / 263 | – | 29–47 / 21 |
| Documents page read (1000 documents) | first 480–570, then 20–30 | | |

Fixed (the two worst): the Variable Studio table and the documents list built every row each frame. Both now build
the rows on screen and 600 px around them (a zero-height probe above the rows records their top and the visible clip
as they're painted; a scroll that shows other rows asks for the next frame; spacers keep the height; a row with the
selection, an edit or a type search is always built). Next: the history panel (frames to 138 ms, idle to 820 ms).
- WIN-SHOW-IN-EXPLORER closed: qa-os.yml 43111545612 (windows-latest, main 27018fe16): a real right-click › Show in
  Explorer opened the folder with the document selected. The job still fails on P3 UIS-RCLICK-WINDOWS (the UI
  script's rclick opens no row menu on Windows; a real one does).
- IMPORT-SPLIT-DOCUMENTS (live 2511): Split now takes a STEP of loose parts (one document in a folder, its Assembly
  named after the file), a Parasolid assembly (xt_read.cpp records subassemblies and each child's placement, as the
  STEP tree) and an IGES one (kernel `read_iges_assembly_nested`: subfigure definitions 308 / instances 408 read
  from the file, solids matched to the reader's parts by label; the top Assembly named after the file, lead 11-11).
  e2e: documents_import_splits_loose_step_parts_into_one_document, _a_parasolid_assembly, _an_iges_assembly (live's
  files in crates/kernel/tests/data). The IGES reader keeps a subfigure as one compound part (ticket
  IGES-SUBFIGURE-ONE-PART, dx): the split breaks it into its solids by the definition's order. Not yet: an IGES
  subfigure placed twice (not split).

### 2026-11-11 21:50 UTC: Feature Studio undo steps and the notices count (lead 11-11)
- Undo: a typed run up to a pause (700 ms, the save delay) is one step; psui's "first character its own step" (2480)
  dropped (lead: live's recorder types at 90 ms a key and 3 of 4 takes undo the run whole). UI test
  `a_typed_run_undoes_in_one_step` (also Ctrl+Z straight after typing). fs-tb-redo stays live's split take.
- Notices: live's reader counts the notices pane's rows when it is open, else the gutter's marked rows (one per notice
  line, filled from row 1: behaviours 1683). Ours counted every notice; `FsUiState::notice_lines` now. fs-ed-error-
  squiggle step 7 (hover over the squiggle) reads 2 on live with one ✕ shown: asked live which cell (REQUESTS 2507).
- IMPORT-SPLIT-DOCUMENTS leftovers (parts-only STEP, Parasolid, IGES with Split) asked live (REQUESTS 2511).

### 2026-11-11 20:06 UTC: restarted after the quota pause
- My 4 commits of 11-08 had all landed (9535caa6 last); the parked merge-rows patch landed through psui (71c63ad3a).
  psui covered the area 11-09 to 11-11 (entries above and in psui.md: Feature Studio at a version 2485, undo steps
  2480, Create version 2481/2483, Variable Studio table drag and rename 2487, Ctrl+Z in cells 2476, Paste tab, advanced
  search).
- Suite on green e3552a6da: doc/docs/fs from qa's full run (vibe5, deleted mid-run at 20:34), prefs/ver/vs re-run on
  vibe3 (`~/tmp/studios/uip-1111`, /root/studios-scratch/bin/shards-names.sh; one scenario: bin/uip1.sh <names>).
  doc/docs/fs misses: doc-menu-close-document (fixed: the scenario seeds live's remembered "Owned by me" through
  config_files start.json; 3/3); live-side: docs-adv-state/type-list (live reads no list for its native pickers),
  docs-doc-menu-versions-and-history (live still loading), fs-ed-error-squiggle (reading artefact, re-record asked
  REQUESTS 2507), fs-tables-* (lane document), fs-ps-custom-feature-error (compile time). fs-undo, fs-redo,
  fs-undo-button, fs-tb-redo: ours leaves "/" after Undo (psui's 2480 rule: first character its own step); live's
  takes (typed at 90 ms/key) undo the whole run in 3 of 4 (2480's own 90 ms row: one step). Raised with the lead.
- Totals: prefs/ver/vs 266/273 scenarios, 1213/1229 states (prefs 58/58); all 584 studios scenarios: 19 differ, 1 of
  them ours after the fix below (ver-create-version-drawings-warning), 4 the undo rule (lead's call), 14 live-side.
- ver/vs: ver-create-version-drawings-warning step 5 (live: Update drawings. leaves no focus; ours keeps Name
  focused: the dialog host's focus trap hands its focus back to the first field; open), live-side ver-version-menu-branch
  (REQUESTS 2493), vs-ps-inserted-drag (2490), "Unknown of Sketch" (reader), ver-vrow-releases (free plan),
  ver-linked-selective-update (live's click missed).
- P1 WIN-SHOW-IN-EXPLORER: `file_dialogs::os_path` drops the `\\?\` (and `\\?\UNC\`) prefix before reveal / open
  (start_window `open_in_file_manager`); T1 test os_paths_drop_the_verbatim_prefix. Check on Windows: qa-os.yml.

### 2026-11-09 22:49 UTC: documents coverage 285 → 304 of 313 (psui, lead: lowest interaction area)

Nine scenarios (source "psui 11-09 (documents coverage)"), all running on ours; live has yet to record them (REQUESTS
"documents coverage"): doc-cad-file-card, doc-cad-file-import-options, doc-cad-file-import (start doc-cad-file: doc-files
plus uip-asm.step uploaded untranslated as a file tab, as live keeps a CAD file until its own Import…),
doc-file-tab-download, doc-tab-menu-change-to-version, doc-tab-menu-export-view-rules, prefs-env-profile-cycle,
prefs-units-learn-more, docs-account-preferences. New targets: `nav:<item>` in Preferences, `button:View export rules`,
`button:Units › Learn more about default units.`, the Import to Onshape modal's cards (`button:<title>`) and checks
(`checkbox:<label>`, noted as modal fields). Left uncovered (9): the Import modal's single/split choice, + › Import… of a
CAD or image file, Join adjacent surfaces and Update… (all need a file-pick step in both runners), the print toast, the
quick switcher's hover/mouse-up, shortcuts with focus on the top bar. docs/doc/prefs suite: 172/181, 709/725 states, no
scenario worse.
Then (23:00): a row's Versions and history opens the document on its Versions and history panel (live lands on
/documents/<id>/versionsAndHistory; the strip's icon keeps the page's panel; hook `open_document_history`), and Select as
document thumbnail toasts "Document thumbnail set to <tab>" (doc-tab-menu-thumbnail matches). Live-side, asked (REQUESTS
23:02): doc-units-angle-radian (stale start), docs-doc-menu-versions-and-history and doc-tab-menu-copy-paste (read too
early), doc-menu-close-document (the reader after leaving the document); with live2 since 09:06: doc-quick-switch(-once),
doc-nested-crumb-menu, doc-plus-menu.

### 2026-11-08 20:57 UTC: PAUSED (lead: quota). Restart from here on Monday
**Landed / landing today:** Print setup (Part Studio, Assembly), the Merge notice's Revert link (shared notice
bubble), Restore to Main… dialog, the Variable Studio's `#` autocomplete (`var_complete` now in app-core), live's
top-level snippet texts, and last: **the disabled-Button root fix** (vendored gpui-base `button.rs`, NOSHAPE PATCH): a
disabled Button no longer stops the mouse-down, so ancestors see the press and a focusable ancestor takes the focus
(a browser's disabled control: live vs-cfg-insert-all-disabled, focus leaves the field); only the release stops, which
keeps an ancestor's click from firing (gpui-base's test `disabled_button_is_inert_and_blocks_parent_activation` still
holds by design; gpui-base's tests don't run in the workspace). The Variable Studio footer workaround is gone.
Regression check: 129 of 486 studios scenarios re-run with the fix, none worse from it.

**In flight, not landed: the merge rows (live 2383, REQUESTS 2383, shots studios/q2383/).** Patch:
`/root/studios-scratch/keep/merge-rows.patch` (apply with `git apply`; built and store tests passed, no scenario run
yet). What it does, and what live showed:
- Two history-panel row kinds, `RowKind::Merge` (the merge microversion in the target: "Merge from B1", bold,
  version-name style, a small dot on its lane, Show changes…) and `RowKind::Merged` (the source's microversion the
  merge took, named by its change, a small dot where the source lane leaves). Each later merge adds another pair.
  Both read as kind "version" for the recorder. Not added when a version or Start is already at that point.
- Menus (live m1-results.json): Merge row Open, Open in new browser tab, (Copy link), Compare, View in repair,
  Restore to Main…, Create version here…; Merged row Open, Open in new browser tab, (Copy link), Compare, Create
  version here….
- Opening the Merged row shows "B1" in the navbar (`Session::shown_workspace_name`: a viewed point's own workspace).
- The store's merge entry is "Merge from B1" (was `Merge "B1" into "Main"`; store test updated).
- To do: apply, run ver-merge / ver-merge-* / ver-crow-* / ver-history-* (the graph's small dots: compare with
  q2383/m1-graph-zoom.png), the e2e tests that read history descriptions, then land.

**2026-11-08 22:42 UTC: psui took the Variable Studio list (lead, while studios is paused until Monday).** The 30 uncovered
items of `coverage_variable_studio.py --uncovered`: psui writes the vs-* scenarios and their `via` chains
(universe/variable-studio.json), asks live2 for the batch, and fixes what the recordings show. Documents and versions
stay yours.

**2026-11-08 23:49 UTC (psui): Variable Studio coverage 100 % (98/98); the recorded vs-* mismatches.** 27 + 2 scenarios
(vs-fs-monitor / vs-fs-profile on the new start `fs-vs`), batch with live2 (REQUESTS 22:59, follow-ups below). Of the
72 recorded: vs-ps-table-create-studio (Create Variable Studio from a Part Studio's table: the tab one commit, not
undoable; the Part Studio's reference its own undoable edit), vs-cfg-input-propagation (`ui_script::mark_note`: a
panel's notice reads before a toast), vs-cfg-value-unconfigure (run.py: a toast live had timed out, ≥4.4 s on live's
clock, isn't compared; Esc also drops a timed toast) now match. Left on live's side: vs-cfg-checkbox-dialog (live2's
reader reads a checkbox input's `kind` as its lowercased name, `default` a string), vs-insert-other-documents (step 3
reads no tabs and a "Standard" field: another panel's?), vs-tab-menu-copy-clipboard (Esc left the + menu open there,
closed in doc-plus-menu: focus after the context menu), vs-undo-value (live's Ctrl+Z on a selected cell does nothing:
a shortcoming, not a target), vs-type-* (focus only, as noted below).

**2026-11-08 23:53 UTC: psui took documents batch 7 (lead, while studios is paused until Monday)**: the via entries, the new
docs-* / doc-* / prefs-* scenarios and the documents page's marks below. Versions stay yours.

**2026-11-09 01:12 UTC (psui): live2's 27 vs-* recordings (798cb3b9) run: 4 → 19 match; all 99 vs-* 82 match
(538/563 states).** Fixed from them: Filter inputs takes live's `:name` / `:internalid` / `:variablename` /
`:hasvisibilitycondition`; a configuration row click only selects its cell (the row menu's Switch to shows it), Enter
in the add row leaves the cell selected, the add row's typed name reads as a row; Set to #<variable> has no toast; the
Insert dialog's ▦ is "Set to last viewed configuration" and ▾'s two items toast "Configuration parameters already
match last viewed / default value." when nothing changes, a pick unfolds the studio (and shows a configured one's
inputs), a lone studio is listed unfolded and several folded, a studio there only through Insert into all isn't
ticked, the Version graph opens over the list (which stays); an automatic table's ⋯ has no Edit / Remove reference;
the table title is its name's field (a click renames; the fold glyph folds); a Value literal with more decimals than
the workspace's shows rounded ("1.235 mm", the typed text its tooltip); after a number the autocomplete also lists
functions by the unit's first letter ("45 mm" → mm, max(, min(); Enter commits when the highlighted entry changes
nothing; **an edit of an inserted table made in a Part Studio's panel is undone in the Part Studio, not in the
studio** (store: an undo entry's `origin`, `Session::set_undo_origin`, `DocModel::edit_tab_in`). Runner / run.py:
"+" typed as "+" (it typed "-"), a hover where the pointer rests comes from outside, `panel_more` is the panel's first
⋯ (`ui_script::mark_first`), a modal's field over a feature dialog reads "other", a feature dialog's focused button /
link is fieldless focus, live's Free-plan upgrade modal (vs-tab-release) isn't compared. **Asked live2 again
(REQUESTS 2398 / 2399, 00:47):** vs-fs-monitor / -profile, the reader checks, vs-type-pick-then-type (the lead's
question: where the typed 45 lands), plus screenshots for vs-ps-* step 4 (Variable Studio 1 gone from the list after
a pick), vs-column-resize (undo on: is the width the document's?), vs-ps-new-variable ("Unknown of Sketch 1"), the
Version graph and a configured studio's inputs. Left: those, and the known live-side ones above.

**2026-11-09 01:12 UTC (psui): documents batch 7, chunk 2 written (25 scenarios, coverage 79.9 → 87.9 %).** Document
notes (Edit / Cancel, preview + Apply, Refresh), Performance, { } notices, the shortcut sheet's search / customize /
pop out, the Tab manager's help / preview / type filter / keyboard, the toolbar's Undo / Redo, Properties of a
drawing and a file tab, Copy version (viewing V1), the tab hover card, the Material Library tab's menu, the five
imported-file viewers (new start `doc-files` with CSV, JSON, TXT, PDF and PNG files), New document's location box
and labels; via only for Export Drawing (drw-export-dialog) and Export (tab). Not written: the Import to Onshape
modal (a CAD file picker), Ctrl/Shift+click tabs, scroll arrows, nested crumbs, Ctrl+Space, shortcuts from the top
bar, the location box's Sort (ours has none yet), Copy workspace's browser marks. Asked live2 for both chunks.

**2026-11-09 01:36 UTC (psui): documents batch 7, the rest (lead): 10 more scenarios, coverage 88.2 → 91.1 %.**
Ctrl/Cmd+click and Shift+click a tab (ours now opens it in a new window and this one stays, as the menu's Open in new
browser tab), the tab bar's scroll arrows (start `doc-many-tabs`, 31 tabs), nested folder crumbs and the … dropdown
(start `doc-nested`), Ctrl+Space held and released (the runner's new `release` step). The location box's Sort is in
(4c129d33). Import to Onshape and Copy workspace's New folder: live2 asked to describe them first (REQUESTS).

**2026-11-09 01:55 UTC: psui took Versions, branches, history and release (lead, while studios and skasm are paused).**
Applied /root/studios-scratch/keep/merge-rows.patch (store and history e2e tests pass; ver-* run against it: see
psui's land note). The Versions list, 22 scenarios in two batches (coverage 83.8 → 99.4 %): compare with no
differences / a tab missing (compare and version), moved-feature markers, the blend slider, compare hover, Feature
Studio compare, Show more changes…, the view-only bar's ? (now opens help), Publish custom features (Create version
checkbox, Publish FeatureScript… dialog), the version Properties tree, the drawings-out-of-date notices (Create
version, Workspace protection), Edit healthy moment, Repair's close and Zoom to fit, the merge dialog's per-tab
strategy, the assembly Insert dialog's Create version…; batch 2 on a new start `doc-linked` (a second document "uip
Source" referenced at V1 with a newer V2; start key `linked`): the tab's newer-versions badge, Update all references,
the Reference manager's Update all / Selective update / Version graph, Change to version…. Left (1): the merge
dialog's drawing row ⓘ / Open in <ws>. Known gaps ours will show: Show more changes… (none yet), Repair graphics' menu
(the panel draws no graphics yet).

**2026-11-09 02:40 UTC (psui): the merge-rows patch landed (71c63ad3; ver-* with it: no regression, +1 state); the older
ver-* differences.** Fixed (6f0ee049): search (on Enter) and filters list the matching changes alone, newest first,
else "No changes found"; another workspace's row opens on a click; Suppress / Unsuppress entries name the feature;
Return to Main closes Versions and history; Workspace protection's Apply toasts "Workspace protection settings
updated."; the filter's Tab select shows its tab as a tooltip; Show more changes… (50 at a time; doc-versions-many now
sixty changes); the Repair panel's graphics (part-studio-ui repair_view.rs: the snapshot regenerated off the UI
thread, following the main camera while synchronized, right-click Zoom to fit; repair_mode's #[allow(dead_code)]
gone). In the 21-scenario check: ver-search-history / -change, ver-filter-modified-by / -tab / -reset,
ver-history-entry-per-edit, ver-view-version-return now match. Asked live2 (REQUESTS 02:3x): re-records of takes
read mid-load or mid-close (8), ver-merge-replace's target, and five questions (Compare with base, Ctrl+Alt+V,
the date input, Copy version closing the panel, empty custom features / releases). Known: ver-branch-no-open (live's
checkbox takes the focus from the name field; ours keeps it).

**2026-11-09 16:43 UTC (psui): the documents page's import in the background, its progress in the bell (lead).**
Import files… (single and split, `StartView::import_cad`) reads the files with a 30 s timeout ("<file> could not be
read: no answer from the file system in 30 s", a failed entry and the page's alert), then translates and builds on
the background executor. The page now has live's Notifications bell (`notifications::bell_button`, shared with the
document window): the entry walks "Importing <file>…" › Uploading… / Translating… to "Import <file>" with ✓
Uploaded · Translated · Post-processing done · "Created link to <document>" (a split: the folder's name), whose link
opens the document; the page stays where it is (live behaviours "STEP import flow" 2.). Job entries:
`notifications::start_job` / `update_job` (steps, link), tests app `notification_job_tests`. Checked on Xvfb
(/root/psui-scratch/imp/: split, the link, a FIFO as a hung file). Asked live2 (REQUESTS 16:11) for the entry's look
while running and what a split's last line links to. Split itself (13:40 entry below, 14:49): op
`documents_import`, `noshape_ops::import_documents`; live2's 14:37 capture pending.

**2026-11-09 13:26 UTC (psui): lead's list from live2 2351 / 2403 / 2405.** Labels… in a row menu opens the Details
panel with Document labels open (no dialog), as the row's Labels icon. Import to Onshape: the cards swap the checkbox
list (Split: "Create one document per part or assembly(not recommended)" in the composite part's place, with live's
tooltip; Combine: no composite), and the page modal's labels are one line (548 px). Split's documents themselves are
still one per file: docs/TICKETS.md IMPORT-SPLIT-DOCUMENTS. Ctrl+Alt+V: the scenario's about and the versions universe
no longer count it (sc-create-version dropped; live's Customize list keeps Create version unassigned, as ours). Already
in main from this morning: the user guide's Ctrl+Alt+V line, Copy workspace's New folder, Copy version… closing the
panel, Compare with base in its own window, no custom features button in a version without any. docs/doc 170/181.

**2026-11-09 12:28 UTC (psui): the docs-* differences from live's 2407 answers.** Sort menus (Folders, the location
box) draw the checked row active (`#b2ddf6`, ✓) and read it as highlighted (`OsMenu::active_checked`). A "Last opened
by me" card opens its document on one click. A tooltip under a resting pointer as a browser's (vendored gpui-pre
`div.rs`, NOSHAPE PATCH `tooltip_painted_at`, and `Window::pointer_rests`): an element that appears or moves under a
pointer that doesn't move shows its tooltip (the top-level "Owned by me" title after the crumb click: its own element
id `place-title`; a documents row slid up under it: rows keyed by path); one an overlay uncovers doesn't (live: no
toolbar tooltip after a dialog closes). Tests: app `tooltip_rehover_tests`. run.py: live's document runner (run.mjs,
live's and live2's) blanks the tooltip after every step but a hover, so ours' isn't compared there; its page runner
(states with `url`: docs-*, prefs-*) reads it. Live-side, asked (REQUESTS 12:17): docs-trash-restore (the account's
trash had other documents), docs-new-doc-location-sort (Z → A kept from an earlier pick: does it persist?),
docs-doc-menu-versions-and-history (old reader's error), what Labels… opens. docs/doc: 162 → 169/181.

**2026-11-09 06:50 UTC (psui): the "open on ours" four.** The code editor's click now lands where live's runner
clicks Ace (400 px in, 300 px down, else the middle) and the editor's text is live's 14 px: fs-compare and
fs-ps-custom-feature-error land on live's line and column. fs-ed-ctrl-click-std is live-side: its click missed the
word (y 527 for line 6; asked live2, REQUESTS 06:04); ours opens std as live's notes say. Help › System check is a
page in its own window; documents windows are a list (a folder's Open in new browser tab opens another at the
folder; a bubble shows in its own window); the appearance editor commits as you type (Undo on at once, ✓ one step,
✕ / Esc restore with no step); View in repair only from a Part Studio. 202-scenario check: 127 → 131, none worse.
Left to dx: fs-compare's version text in Compare (live shows V1's), fs-ps-show-code, the notices counts.

**2026-11-09 04:45 UTC (psui): the full run's differences (lead: ours vs live-side, fix ours, biggest loss first).**
Points lost by area (weight × (1 − match × coverage), 02:57 run): documents 1.13, Feature Studio 0.40, versions 0.28,
Part Studio 0.14, Variable Studio 0.12 (drawings 0.55 and assemblies 0.10 left to dx and asm; asm's 8 in asm.md).
Fixed (landed 35fd1ddf, 88bb798a; queued with the regression check): Copy version closes Versions and history, Close
document shows the documents page, a new Feature Studio's caret at its end, the Material Library tab (notice, menu),
the documents page's Copy workspace… dialog, its Properties Undo / Redo, the row Labels icon, crumbs' titles, the
location box's select-then-Open, the column chooser's Esc, the Tab manager's Enter, dark mode from the account menu
(shown, not saved, its notice), notes' bubbles, drawing Properties (Drawn by, Approver, dates, Last changed,
Inspection count), the VS type select's keyboard (live 2399), column widths as a document edit (2398), the Insert
dialog's drill-in and greyed ✓, a custom table's Switch to at its definition, focusable native selects. Live-side,
asked again (REQUESTS 04:2x): 16 doc-tab-menu-* taken before live's tab menus gained Export…, takes read mid-load or
mid-close, reader / account-state ones (the + menu, the Shift+/ sheet, prefs after a library removal), questions on
the advanced search caret, crumb titles, sort menus' highlighted row, dark mode's link. Open on ours: fs-compare's
editor click lands 9 lines lower than live's on a long studio (live's runner clicks Ace's content box: not modelled
yet), Ctrl+click into std source (fs-ed-ctrl-click-std), System check opens a page in live, a second documents
window for a folder's Open in new browser tab.

**Coverage plan (lead: coverage is the lever; largest gap first).** Uncovered: documents/tabs/prefs 81 (74.1 %),
Variable Studio 30 (69.4 %), versions 27 (83.8 %). `python3 tools/ui-parity/coverage_<area>.py --uncovered` lists
them; an item counts once a scenario's steps contain one of its `via` chains (tools/ui-parity/universe/<area>.json;
71 / 29 / 27 of the uncovered have no `via` yet). Batch 7 (documents, not started) as designed:
- Add `via` only (already exercised): Search results view ← docs-search (field Search, Enter); Export (tab) ←
  doc-tab-menu-export (rclick doc_tab, menu Export…).
- New scenarios (start docs-page): Folders collapse/expand (button Folders ×2); folder breadcrumb (dblclick doc_row
  uip Folder, button "Owned by me"); sort by column (button Name); Folders sort / grid-list toggle; column chooser
  Search… (button Add, field Search…); Advanced search ? (caret Search, button help); Notes / Properties / Versions
  and history panels on a selected row (rail buttons; Properties: Description edit, Undo / Apply); row inline icons;
  Account menu (button Account menu), › My account, › View in dark mode beta (and back: ask live2 to restore).
- Preferences (start prefs-page): Units › Learn more…; Environment profile Create → Save profile settings → select
  → Delete profile (via already in the universe).
- Documents (start doc-tabs / block-drawing / block-drawing-files): Keyboard shortcuts sheet › Search shortcuts /
  Customize keyboard shortcuts / pop out; tab manager Help ?, Toggle preview, type filters, keyboard ↓ ↵; tab menu
  on file tabs (dx-plate.dxf, dx-image.png) Download / Update… / Import… (via already there); Properties… of a drawing
  tab and of a file tab; drawing tab Export… (Export Drawing); Create Material Library and its tab menu; Shift+click /
  Ctrl+click a tab; image tab viewer; Document notes (Edit notes, Cancel), Performance, { } notices; Undo/Redo
  toolbar buttons after an edit; Copy version… while viewing V1 (doc-versions); New document › Document labels /
  location browser / Sort; Copy workspace's location browser.
- Our page needs marks for several of these (the documents page has only ~15: `start_window.rs`); add them with the
  scenarios. Then ask live2 for the batch (README "TARGET" for new target names), fix what the recordings show, then
  the Variable Studio list, then versions.

**Other open items:**
- doc-shortcuts-sheet: in a Part Studio, live reads the Shift+/ sheet as no dialog, while the Help menu's Keyboard
  shortcuts read a modal (doc-help-menu-keyboard-shortcuts); ours notes a modal for both since 20:04 (2/3). Check the
  two recordings' dates; one may need a run.py rule or a re-record.
- ver-create-version-edit-props / ver-version-menu-branch: live still shows the dialog a step after Create (re-record?).
- vs-type-*: live's type picker leaves an empty input focused (focus only).
- prefs-units-frequency: live keeps the native select focused after Esc ("0: hertz"); docs-folder-open and
  docs-trash-row-menu re-records asked (REQUESTS 18:15); Labels… on the documents page asked there too.
- Print on A1, A0, ANSI D and E renders at 8192 px and scales up (live's size not recorded).
- Scratch: displays :196–:199 were mine (killed at pause); builds via rb.sh on vibe3; helper scripts in
  /root/studios-scratch/bin (run-half.sh for split runs).

### 2026-11-08 20:44 UTC: every top-level snippet inserts live's text (live 2337)
- Feature / Table / Computed property definition, Function, Type, Enum, Predicate, Operator, Constant declarations,
  New feature / table / computed property and Import insert live's texts with its selection after Enter and its Tab
  stops (the feature rows now also stop on the two comments). No stand-ins left; fs autocomplete scenarios 14/14.

### 2026-11-08 20:38 UTC: the greyed "Insert into all" takes the keyboard (time-boxed item, ~10 min)
- Cause: gpui-base's disabled Button stops the mouse-down from bubbling, so no parent listener ran. The footer takes
  the press in the capture phase (`capture_any_mouse_down`) and focuses the studio; vs-cfg-insert-all-disabled now
  matches. (Worth knowing elsewhere: a parent's `on_mouse_down` never sees a press on a disabled Button.)

### 2026-11-08 20:29 UTC: the Variable Studio's Value cell lists `#` variables (lead: move the autocomplete)
- `var_complete` moved from part-studio-ui to app-core (noted in psui.md). A Value cell being edited lists the
  variables above its row with their values (no "New variable…", as tables), ↑/↓, Enter inserts and commits, Tab
  inserts, Esc closes the list and keeps the cell editing (live vs-value-autocomplete, now matching).

### 2026-11-08 20:19 UTC: the Merge notice's "Revert to before this merge" (lead: one shared toast)
- `DocModel::info_with_link` keeps a notice's link; app-core `doc_model::notice_bubble` draws it in the Part Studio's,
  the Assembly's and the workspace's toasts (`notice_text` for the ui-parity message). A linked notice stays 35 s
  (live ≈ 35). Merge shows "Merged B1 into Main." with the link, which restores the workspace from before the merge
  (a Restore entry). A merge is no undo step and clears the undo stack (live ver-merge: Undo off after it).
  `history_merge` returns the notice and `revert_to` (history_restore to it reverts), with its e2e test.
- Open: live's history after a merge shows a "Merge from B1" row and an automatic version of the source's head
  ("Part Studio 1 :: Add feature : Extrude 3", version kind); ours shows neither (ver-merge step 4).

### 2026-11-08 20:04 UTC: ☰ › Print… in Part Studios and Assemblies (lead: studios' end to end)
- Live 2349: the Print setup flyout at the graphics area's top left (Paper Letter … ANSI E, Portrait / Landscape,
  Print, Download image, ?), a dashed frame 90 % of the area in the paper's shape, the setup remembered
  (`print_setup.json`). Download image saves the framed view on white at 299.64 dpi (Letter Portrait 2547 × 3296),
  lines and labels as thick relative to the page as on screen; Print opens it on a PDF page of the paper. Shared:
  app-core `print_setup.rs`, `noshape_scene::print` (frame, camera crop, sizes, PDF page); hooks in part-studio-ui
  and assembly-ui (noted in psui.md / asm.md); op `render_print` with an e2e test; guide section "Printing".
- Papers past 4096 px render once without supersampling, past 8192 px (A1, A0, ANSI D, E) at 8192 and scaled up
  (the GPU's texture limit); live's size for those wasn't recorded.

### 2026-11-08 19:13 UTC: Restore to Main… dialog, Feature Studio's top-level list, Backspace keeps the list
- Restore to <workspace>… on a version opens live's "Restoring changes into Main from V1" (the merge dialog's restore
  mode: Keep / Replace cards, "Changes in Main", Tab restore strategy, Restore / Cancel); a change row has none (live
  2351). The entry reads `Restore 2m 55s back: "<the restored point's change>"` (store); `history_restore` takes
  `keep` (tabs kept as they are, `Session::restore_picked`).
- Feature Studio, top level: live's rows per first letter (studios 2334: Define table / Define computed property
  verbatim, their Tab stops); rows live hasn't shown the text of (Feature definition, Type declaration, Import…) insert
  our best FeatureScript until asked. Backspace refines an open list (vendored NOSHAPE PATCH in gpui-base: a
  deletion leaves the list to `handle_completion_trigger`).
- Open: the Merge toast's "Revert to before this merge" (live 2351: blue, ≈35 s; reverts at once with a Restore entry)
  needs an Info event with an action through the Part Studio, Assembly and workspace toasts; Print setup (live 2349).

### 2026-11-08 19:01 UTC: dx takes the Feature Studio area's uncovered items (lead 11-08)
- docs/parity/INDEX.md gives "Feature Studio and FeatureScript editor" to dx. dx takes these 35 (batch fs-1, start
  `fs-custom`): fs-ac-click-accepts, -overloads, -tie-order, -enum-name-insert, -doc-tooltip, -placeholder-shift-tab,
  -reindent; fs-ed-gutter-line-numbers, -fold-arrows, -ctrl-click-definition, -ctrl-click-std, -error-squiggle;
  fs-notices-println, -expand-stack, -monitor-dropdown; fs-profile-total-badge, -line-bars; fs-commit-regenerates;
  fs-compare; fs-tables-panel, -selector, -switch-to-studio, -copy-table, -row-hover, -cell-tooltips, -add;
  fs-ps-ws-custom-features-menu, -ws-feature-tooltip, -insert-custom-feature, -custom-feature-error, -acf-click-adds,
  -acf-other-documents, -acf-close, -acf-learn, -added-tool-menu; and the fs-* mismatches that aren't yours
  (fs-ps-show-code, fs-shortcuts-sheet, the notices counts). Yours stay yours: Define table / Define computed
  property, fs-autocomplete-backspace, fs-tb-redo. Left for later: the narrow-toolbar items (fs-tb-*-compact,
  fs-tb-collapse-breakpoints) and fs-ed-dark-theme, fs-ed-hold-ctrl-hover.


### 2026-11-08 18:47 UTC: Variable Studio types keep their values; live's answers 2336
- A Variable Studio row keeps one value per type (`VariableDef::other_values`, as FeatureScript's Variable): a new
  type starts at 0 in the workspace unit, going back restores the old value (live vs-type-*). The table's title reads
  as its rename field while renaming; Enter leaves the keyboard with the studio.
- Edit shortcut › Reset to default: only its ↺ reacts and fills the field with the default; Confirm applies it (live
  2336). run.py drops the + menu's "Create table" (live: gone since 06:20).
- Open: the Value cell's `#` autocomplete (vs-value-autocomplete) needs `var_complete` moved from part-studio-ui to
  app-core; the type picker leaves an empty input focused in live (vs-type-*: focus only).

### 2026-11-08 18:29 UTC: versions 52/91 (from 46), documents page and ☰ menu fixes
- Versions: workspace / version Properties records its fields; a history entry's Create version is "from history"
  and offers Publish custom features and tables; the repair hint ends with "."; no row is selected while picking an
  entry to repair; the navbar keeps the workspace's name on a history entry and in view only mode (no "History");
  view only mode's ☰ is Copy workspace…, Switch to edit mode, Print…, Close document. run.py: history rows' native
  title tooltips aren't compared; "Add feature" (REST-built starts) reads as "Insert feature" in the compare bar too.
- Documents page: the breadcrumb leaf (`place`) is the folder's name inside a folder; Create › Folder… goes into the
  new folder; the search runs on Enter (the breadcrumb stays while typing); Remove from recently opened drops the
  selection; a trashed row's tooltip "This document is in the trash and cannot be opened.". About and the Keyboard
  shortcuts sheet are recorded as live's modals.
- Open (asked of live, REQUESTS 18:11 / 18:15): ☰ › Print… in a Part Studio does nothing in ours (live: a Print setup
  panel); Restore to Main… restores at once (live: a "Restoring changes into Main from V1" dialog); the Merge toast's
  "Revert to before this merge" link (needs a DocEvent with an action: Part Studio, Assembly and the workspace each
  draw their own toast).

### 2026-11-08 17:56 UTC: Preferences 49/56 scenarios (from 43)
- Add / Edit export rule records the body's text as live reads it; Toolbars › Reset to defaults greys after a click;
  Edit shortcut: Disable shortcut hides the hint and Reset to default; a Part Studio action's conflicts include the
  Sketch's (Extrude on shift+f: "multiple shortcuts … Fillet Sketch fillet"); a disabled row has no reset icon and
  reads as no key. run.py: a page's native select's popup isn't compared (live can't read it).
- Asked live (REQUESTS 17:52): documents-page recordings read before the opened document loaded, Cancel steps that
  found no button, the + menu's "Create table", the shortcut popup's Reset to default.

### 2026-11-08 17:36 UTC: full suite 266/394 scenarios, 1625/1814 states (89.6%, from 85.1%), no regressions
- Edit internal IDs records its fields (name / id pairs, as live's inputs); the Variable input dialog's Default,
  Minimum and Maximum are "number" fields for the numeric kinds; clicking a kind tab takes the keyboard from Name (live
  vs-cfg-variable-dialog: the typed text goes nowhere). The Tabs panel's Sort menu is a recorded list and Esc closes it
  (run.py: live reads its rows into the panel's name).
- Open: vs-cfg-insert-all-disabled step 5 (a click on the greyed checkbox doesn't reach a footer mouse-down listener:
  something above it takes the press; not found yet). Define table / Define computed property re-asked of live
  (REQUESTS 17:08). Batch 6 (REQUESTS 15:27) not recorded yet.

### 2026-11-08 17:04 UTC: live's 2331 answers applied; Feature Studio and Variable Studio configuration fixes
- Autocomplete as live: outside every function the list is snippets then keywords (`fs_complete::at_top_level`,
  `complete_top_level`: "Define feature" inserts the New feature template, "My Feature" then myFeature by Tab;
  "Constant declaration" `const constName = 0;`), inside a function std's names as before; rows record their kind
  (`list.meta`) and the highlighted row (vendored NOSHAPE PATCH `COMPLETION_SELECTED_ROW`). Enter accepting a row no
  longer also inserts a newline (vendored NOSHAPE PATCH: the menu propagated every action). Tab pads to the next tab
  stop (NOSHAPE PATCH, Ace's soft tabs). No bracket auto-close in the Feature Studio. Open: Define table / Define
  computed property (asked of live2 with the full top-level snippet list).
- Feature Studio: the Import panel takes the keyboard after a click in it and doesn't pass clicks to the code; the
  module outline's Esc closes it and drops the focus; parameter snippets select their name ("My Length") and end with
  a newline; the snippet menus, Monitor ▾ (its row "Profiling <studio>" stops profiling) and the notices filter
  (new: `:text`, `:elementid`, `:tabname`, `:currenttab`, `:minlevel` tokens, its icon's menu) are page dropdowns.
- Tabs panel menus as live's: a tab's Create… ▸ (Folder, Import…, Part Studio, Assembly, Variable Studio, Drawing…,
  Feature Studio, Material Library) then its own menu with Add selection to folder; a folder's Create…, Rename, Add
  selected to folder, Unpack, Move to document… (`workspace::panel_create_menu`, `panel_tab_menu`).
- Variable Studio configuration: Configure leaves the new list's Name row editing (live 2331); the row menu offers
  only what changes something; a configured cell's "Unconfigure"; the notice's second line only with one input;
  dialog fields' focus labelled ("Name"); closing the input dialogs or ticking Insert into all drops the keyboard.
- Not done: live drops a text field's focus on any click (a browser's); GPUI only on focusable elements, so a few
  focus states differ (vs-cfg-insert-all-disabled). Edit internal IDs records no fields yet.

### 2026-11-08 16:12 UTC: release management covered (lead 11-08: spec from Onshape docs, not recordable)
- 22 scenarios rel-* (`"recordable": false`, README): Release… on a part, the Create Release candidate dialog (Generate
  missing part numbers with a seeded numbering scheme, needs-attention filter, search and type filter, Add to Release…,
  Edit properties, Refresh, Revert unchanged items, Save draft, Release drafts panel and Delete draft, Release),
  Revision history (View release, Export, the version name opens it), Obsolete revision (re-releasable, notes required,
  Obsolete, View obsoletion, Add to Obsoletion…), Properties after a release, Company settings (Release management,
  Numbering schemes, the workflows, Edit revision table template); dx's drw-revision-* cover the revision table.
  Versions coverage 65.7 % → 83.8 %, release 29/29 (Submit and live's Free-plan upsell not counted: Submit goes to
  other users for approval, cloud as approvers; the upsell is a plan's limit).
- Driver targets added: the release dialog's buttons by tooltip / label, its fields (`Part number:<item>`,
  `Revision:<item>`, `Release name`, `Release notes`, `Search items`), `caret:Save draft`, the drafts panel's rows,
  the Revision history's version name (`history_row`), `button:Account menu` (documents page), Company settings' `nav`
  and the workflow `tab`s; the release dialog records `dialog` (title, an item per row, the release name); Company
  settings records `company.section`; `document.panel` "Release drafts".
- Open: after a release, the workspace part's Properties show Revision empty and State In progress (Onshape's docs to
  check what the workspace shows after release).

### 2026-11-08 15:50 UTC: help links open the offline guide (lead 11-08)
- No link to Onshape's website from the app: every "?" and "Learn more" (compare, merge, Workspace protection,
  Variable Studios, Variable tables, Configurations, Custom tables, Inspection data, Custom features, Custom materials,
  Exploded views, Document notes, Performance, Tabs, Default units, Onshape Labs, the documents page's Learning Center)
  calls `noshape_app_core::help::open(section)`: the app's guide window (`help_menu::open_section`) at that section of
  crates/app/assets/help/user-guide.md (sections added), "Whole guide" for the rest. Op `app_user_guide` `section`.
  Guard: coverage::every_help_link_names_a_guide_section (and no cad.onshape.com/help in the sources).

### 2026-11-08 15:12 UTC: live's answers to 2323 applied
- Compare: an Assembly tab is greyed and disabled ("Compare not supported for this tab.", ops `tab_compare` too):
  ver-compare-assembly matches.
- Edit reference… opens "Update Variable Studio": the edited studio's table a preview, Undo and Redo held while it's
  open (`DocModel::undo_held`, `variable_insert::open_update`): vs-inserted-edit-reference matches.
- Move to document…: the tabs the moved ones use (`tab_move::referenced`: a Part Studio's Variable Studio) as
  "N referenced tab(s) will be moved", Show details / Hide details, ☑ Move all referenced tabs; a moved tab others
  use shows live's info box; after the move the bubble "Part Studio 2, Variable Studio 1 have been moved to …"
  (op `tab_move_to_document` `referenced`, `message`). Not done: the bubble's "Open in new tab" link (the toast has
  no action; each studio shows its own messages), live's ~6 s "Moving 2 tabs to …" (server time).
- Feature Studio: Commit ▾ opens nothing with nothing to commit; Monitor ▾ is a page dropdown (Escape leaves it open):
  fs-commit-menu, fs-monitor-menu, fs-monitor-part-studio match.
- Insert Variable Studio › Other documents as live's: "Search or paste URL" (a path picks that document), the places
  Recent, Owned by me, Recently opened, Created by me, Shared with me, Public (the last two cloud: empty), a place's
  documents with ‹ back. Not drawn: live's filter funnel before the box.

### 2026-11-08 14:55 UTC: Feature Studio undo as live's
- Ctrl+Z / Ctrl+Y in the code are the document's Undo / Redo (the editor's own history isn't used; what's typed is
  saved first), as are the toolbar's buttons (they save on press); after either the caret sits at the end of what
  changed (`changed_end`). fs-undo, fs-redo, fs-undo-button, fs-new-feature(-undo), fs-key-back/forward-reference
  match. fs-tb-redo still differs: live saved partway through "// uip" (its Undo took back "ip" only), a save timing
  we don't copy.

### 2026-11-08 14:37 UTC: documents-page ops; ⋯ table menus stay open on Escape
- The documents page's file logic moved down into ops (`noshape_ops::doc_library`: naming, rename, move, copy,
  details, labels, the recent and watched-folder lists, and shared actions `rename_doc`, `move_doc`, `create_folder`,
  `rename_folder`, `move_folder`, `send_to_trash`, `restore`; `noshape_ops::doc_trash`: the Trash). app-core
  re-exports them; the page calls the shared actions, so the page and the ops do the same. Ops (group `documents`):
  list (folder, recent, trash, label; folder sort), create_folder, rename, move, copy, details (+ description), trash,
  restore, delete, empty_trash, label, forget_recent, set_library; e2e documents_page. coverage.rs scans the page
  (area `documents`, 18 rows). docs-* unchanged (146/165).
- The Variable Studio's table ⋯ and the Configuration panel's ⋯ menus are page dropdowns: Escape leaves them open
  (live vs-table-menu, vs-inserted-table-menu, vs-cfg-input-menu; as psui's configuration panel). The + menu stayed
  open in one recording (vs-tab-menu-copy-clipboard) but the ☰ menu closes: not changed on one recording.

### Set library: who reads the library folders
`noshape_ops::prefs::LibraryFolders::load().0` maps a library key to its folder (op `documents_set_library`). Ticketed
(P3, docs/TICKETS.md, lead 11-08): SETLIB-FRAME-PROFILES (psui), SETLIB-SHEET-METAL-FORMS (skasm), SETLIB-TOLERANCES
(core), SETLIB-IMAGES (dx), SETLIB-STANDARD-CONTENT (asm).

### 2026-11-08 13:42 UTC: whole-suite measure 82.1 %; Move to document…, Set library, modal fields, Feature Studio editor
- Measured (uip12, every studios scenario on b66c324a + the batch below in progress): 181/390 scenarios, 1475/1796
  states (82.1 %): documents (doc/docs/prefs) 80.5 %, versions 84.6 %, Variable Studio 83.5 %, Feature Studio 81.6 %.
- Move to document… is live's dialog: New document (Document name "Untitled document", selected) | Other documents,
  "1 tab selected" and the tabs, Move / Cancel; op `tab_move_to_document` (new or existing document, e2e
  tab_move_to_document; `crates/ops/src/tab_move.rs`). Open: the referenced-tabs notice and "Move all referenced tabs"
  (asked live), what happens after the move.
- Documents › folder › Set library ▸ (My frame profiles library, My sheet metal forms, My default tolerances, My images,
  My standard content): kept per user in libraries.json, op `documents_set_library`. Nothing reads it yet: the frame
  profile picker, sheet metal forms, tolerances, images and standard content need to list a library folder's documents
  (owners: core / sheet metal / drawings / assemblies).
- Modals record their fields as live's recorder reads them (`ui_script::note_modal_field`, `os::modal_input`,
  `os::modal_checkbox`, `os::note_modal_input`): Copy workspace / version, Document details, Create version / workspace
  (the name unlabelled, the description "Description cannot contain HTML."), Workspace protection, New environment
  profile, Move to document. The runner reads live's `<input type=button>` fields (Apply / Cancel) as buttons. Create
  Drawing (drawing-ui) still notes `fields: []`: dx's.
- Feature Studio: the Import panel recorded as live's picker (kind tabs, "Search <kind>", a row per element); New feature
  selects "My Feature"; a plain click is a Back reference step; the code editor's focus compared as "other" (Ace's
  textarea holds a piece of text).
- Variable Studio: a name or description being typed is the row's; Enter leaves the field; an added name that's taken
  becomes `<name>_0` with "Renamed some new variables to avoid name conflicts." (the op too); an invalid one stays in the
  add row; Copy only on a text description; Paste inputs only with inputs copied.
- Documents page: the trash toast fades as the restore one (3 s); the page's Empty trash… isn't one of the modal's
  buttons (`os::page_secondary_button`). Runner: "Paste tab", "Paste inputs" are clipboard rows; dropped scenarios'
  live recordings (Copy link) are skipped and deleted.
- Then (same day): measured again with these: 208/390 scenarios, 1528/1796 states (85.1 %): documents 84.7 %,
  versions 84.8 %, Variable Studio 86.6 %, Feature Studio 84.6 %. After it: the Variable Studio's name and
  description cells select on a click and edit on a key or a double-click, as its value cells (`text_cell`,
  `cell_selected` / `cell_editing` by input key); a rename follows in every tab that sees the studio with live's
  toast ("Renamed variable 'w' to 'width' in 3 tabs in this workspace."), a taken name becomes `<name>_0` ("Changed
  some variable names to avoid name conflicts."), in the op too (`variable_studio::rename_everywhere`).
- Next (then): documents-page ops (trash, folders, labels, moves: none exist); the
  Tabs panel's menus as live's (a row: Create…, then the tab bar's menu without Properties… and with Add selection to
  folder; a folder: Create…, Rename, Add selected to folder, Unpack, Move to document…), which needs the workspace's
  tab menu shared with the panel. For psui: the S shortcut toolbar opens on a key with the pointer off the graphics
  (doc-tab-rename-escape: typing over the tab bar opens it; live doesn't).

### 2026-11-08 13:00 UTC: compare, Insert Variable Studio and inserted tables to live's recordings
- The 24 ver-compare / vs-insert / vs-inserted scenarios: 6 → 16 matched, 125/135 states. Compare: change marks as
  live's (">" added = only in the base, "<" removed), the panel stands in for the feature list (`features`/`parts`
  empty, `document.workspace` null), a row clicked in it is the selection; "Compare history entries" clears the row
  selection. Insert Variable Studio: variable rows recorded (`child`), only the picked rows highlighted (a whole
  studio: its row), a search keeps the matching variables, no notice when nothing matches, the red title as
  `titleState`, a previewed table untitled until ✓ (`variable_insert::previewing`), Cancel leaves no undo or redo
  (`Session::forget_undo_since`), its search field focus a dialog field. Variable Studio: the ? popover closes on a
  second click and records its text, Copy table's toast, a collapsed table records no rows, Esc ends Rename.
- Runner: child rows compared as a set (live's order changes per run), "Create task…" cloud (assigns work to users),
  an unread feature-dialog header (`title` null) leaves `titleState`/`ok` unread.
- Not followed: live's ⋯ table menus stay open on Escape (vs-table-menu, vs-inserted-table-menu): a shortcoming.
- Asked of live/live2 (REQUESTS): compare step-3 states sampled before the panel loaded (`features` [],
  `only_differences` null in ver-compare-leave/-only-differences/-reverse/-entries); ver-compare-assembly lists the
  Part Studio's rows on the Assembly tab (is that what live shows?); vs-insert-other-documents reads tabs [] and a
  "Standard" field (another panel?); vs-inserted-edit-reference: undo off while Edit reference… is open.

### 2026-11-08 09:48 UTC: lead's decisions applied (Help menu, Update workspace…, focus rule)
- Help menu (both): Help · FeatureScript documentation · Keyboard shortcuts | System check · About | Install command
  line tool… · Connect to coding agents… (live's rows, its cloud ones left out; ours after a divider, not compared).
  Help opens the offline guide (crates/app/assets/help/user-guide.md; op `app_user_guide`); FeatureScript documentation the
  bundled std document read-only (op `featurestudio_std`); System check a dialog: system, threads, graphics adapter
  (software renderer flagged), memory, the documents folder's disk (`noshape_scene::system_info`, read off the UI
  thread with a 5 s timeout; op `app_system_check`). One list for both menus: crates/app/src/help_menu.rs.
- ☰ › Update workspace… is the feature library's: live's toast "Workspace is already at the latest feature library
  version." (ours ships one std; op `document_update_workspace`). The references dialog is now the Reference manager,
  opened from a tab's "Newer versions of linked documents available" icon (op `document_update_references`).
  Info toasts hide after 4.4 s as live's.
- Focus rule (run.py `no_field_focus`, README "Comparison"): two focuses with no text field compare equal.
  Studios after it (uip10, before these app changes): 121/347 scenarios, 1095/1540 states.
- Then: every alert box and toast records its whole text as `message` (os_ui::info_box, the workspace's, the
  documents page's and Preferences' toasts, the custom notices: `ui_script::mark_message`, joined " / " top to bottom
  as live's recorder); `variables.configuration` null when unconfigured; an Assembly's Properties has Override, Mass
  and Subassembly BOM behavior from the tab's menu too; Insert into all isn't an undo step (`edit_tab_not_undoable`);
  "Remove from recently opened" only in Recently opened. The CLI reserves `help`: the guide's op is `app_user_guide`
  (68ce53f1 failed the land on that).
- Measured with live2's ver/vs/fs batch (394 recorded, before the batch above): 128 scenarios, 1221/1814 states
  (67.3 %): documents 76.1 %, versions 74.9 %, Variable Studio 48.6 % (mostly `variables.configuration` and messages,
  fixed above), Feature Studio 59.6 % (the std 3112 line: dx).
- An Assembly tab's Properties from the tab's menu: its mass is solved off the UI thread in a session of its own
  (assembly_mass_properties on the saved file); Calculate value says "Calculating…" until it comes
  (`properties_dialog::open_properties_mass_later`, `button:Calculate value`).
- Focused text fields recorded as live's (`with_input_focus_in`: gpui-component's `window.focused_input` when the
  state named no field: a modal's name field, Document details' textarea); a Variable Studio cell being typed in
  reports what's typed; Document details focuses its description on open.
- The Repair panel's ↗ opens the entry in a window of its own (it did nothing); its ↗ and Synchronize view are
  driver buttons. The merge dialog lists its Merge button (built after the modal) and records its protection warning.
  Coverage batch 5: 4 ver-* (protected workspace's tab menu and merge, Repair ↗ and sync): versions 62.7 %.
- Open: Assembly Properties lacks Mass / Override / Subassembly
  BOM behavior; ours' modal fields aren't recorded (an Input hook in vendored gpui-component); dx has std 3112.

### 2026-11-08 06:44 UTC: first match numbers; fixes from live's batch 1
- live and live2 recorded 341 studios scenarios (doc 91, docs 32, prefs 54, ver 67, vs 43, fs 54; live 7eb40cea,
  live2 68ce2674). First compare (before these fixes, doc/docs/fs/prefs): 19/96 doc-docs scenarios clean.
- Fixed in ours from the recordings: the navbar's document name renames inline (click, double-click, ☰ › Rename
  document; the modal is gone); the documents page's Rename (rows and folder tiles) is inline; Folders folds only by
  its chevron and is remembered; + menu's "Paste tab" last; history rows (current workspace: no Open / Delete; the
  head change: no Restore; version rows always list Releases, greyed with none; toggles read "Show merges" once
  clicked); a tab dropped on a folder tab goes into it (the bar's gap moved it back); Workspace units isn't an undo
  step; Preferences lists its sections in page order with Create / Delete profile, Reset all, Add material
  library…, Add export rule…; Feature Studio's `committed` is against the committed text, `lines` counts the
  trailing empty line, the caret starts at 0,0; `history_create_branch` takes a `version` (Branch to create
  workspace…), and doc-branches' B1 is V1 as live's.
- Recorder: Workspace units as a panel (live's form), document modals list their buttons (os_ui buttons after a
  title-only modal), the Tabs panel is "Tabs", the documents page / Preferences / a Variable Studio holding the focus
  read "none"; studios' whole-document starts open on Part Studio 1. run.py: cloud rows (Copy link, Share…,
  Applications, CAM / PCB / Render Studio, tutorials, Explore Onshape, What's new, Forums, Report a bug) dropped;
  live's "Add feature" history rows and "live2 uip <start> <time>" names normalized.
- Then (second land): Preferences confirm every save with live's toast (recorded as `message`), the Edit shortcut
  popup's conflict lists every action on the key ("multiple shortcuts"), Reset all after the customized rows, the
  export rule's Convention not focused on open; `selection` null on pages; modal titles with custom headers named
  (Create version from / Create workspace from / Merging changes into … from … / Workspace protection for … /
  Publish custom features and tables at … / Revision history); version rows' Releases a submenu ("No releases");
  the history filter's ▾ closes the panel it opened; a Variable Studio tab reads focus "none" unless a field has
  the keyboard; hovering a folder tab keeps the drag's gap (the folder no longer slides from under the pointer, so
  the drop files the tab); the navbar's double-click rename keeps the field all selected. run.py folds a start
  sketch's "Edit" row into its "Insert feature" row (ours' start builder adds shapes in a second edit).
  doc-branches' B1 stays from Main's head (live's B1 has Sketch 2 / Extrude 2: live2's note was wrong there).
- Third land: a tab double-click no longer renames (live 2279 (b)); Restore deleted workspaces as live's (tick rows,
  Restore selected / Cancel, the empty line as `message`); ☰ and the documents page's Move to… titled "Move documents
  and folders to" with "Move here"; deleting a Variable Studio inserted into all Part Studios and Assemblies warns
  ("Delete tabs", every Part Studio and Assembly; `tab_users`); the Edit shortcut popup's checkbox is no page field.
  run.py: live's newer Properties reading (Description, the category row by its value, Undo / Redo named) and the
  Free plan's "Create public document" compared in ours' form.
- Fourth land: Preferences' Units labels in the UI language ("__name__ default unit"), the recorded fields as the
  page shows them (unit names English, as live's); "Create new label" prefilled "Untitled label"; the name prompts'
  location browser isn't a field. Match after the third land (347 recorded): 105 scenarios, 1058/1539 states.
- Coverage batch 4: 18 docs-* (+ Add column chooser, Advanced search fields, Reset / Search): documents 72.2 %.
- Asked live (REQUESTS 2279): (a) Export… missing on tab menus: live's export quota is throttled again, they
  re-record doc-tab-menu-export / -assembly when it lifts; (b) live doesn't rename a tab on double-click (ours no
  longer does: Rename is in the menu); (c), (d) re-recorded by live. live recorded the docs-trash scenarios;
  live2 is recording the 114 ver/vs/fs "11-08" ones (07:17).
- Open decisions (lead): Help menu (live: Help, FeatureScript documentation, System check are web pages; ours has
  Install command line tool / Connect to coding agents instead); ☰ › Update workspace… (live: the feature library
  version, a toast "Workspace is already at the latest feature library version."; ours: updates references to
  other documents' versions); the std version in a new Feature Studio (live 3112, ours 3095: dx's; **done** 11-08 11:34 UTC, 2f066886: new
  Feature Studios say `FeatureScript 3112;`, existing pins keep their release; rerun the fs-* scenarios).

### 2026-11-08 05:30 UTC: driver below-the-fold fix, batch 3 (55 scenarios), coverage up in all four areas
- Driver: a control in an inner scrolling list (Keyboard shortcuts, material libraries) is wheeled into view inside its
  own clip (app-core `ui_script::clip_of`); all 61 prefs-* run clean. The docs-page start puts its documents in
  Recently opened (live's account opened them) and pins the page to Owned by me (`start_window::open_on_owned_by_me`).
- Ours now: Labels nav and each label row named (`nav:Labels`, `nav:<label>`); another workspace's row › Open in new
  browser tab opens that workspace, editable, in a window of its own (history hook `open_workspace_window`); any
  `os_ui::checkbox` is `checkbox:<label>`; merge cards / "N tabs have no changes" / "Compare with base:<side>" are
  buttons; view-only bar's Return to <ws> and ⊞; Feature Studio's Commit ▾ (`caret:Commit Feature Studio`); Variable
  Studio's Configuration panel (README) and `variables.configuration`.
- Copy link (version / workspace / change rows, navbar link) marked cloud in the universe (it copies a cloud URL);
  their 4 scenarios removed.
- Batch 3: vs-* 26 (Configuration panel, tab and Description menus, inserted table menus), ver-* 17 (edit-properties
  buttons, Open new workspace, merge cards, protection on/off, version view ☰ / Return / ⊞, compare Esc / other row /
  Assembly), fs-* 5 (Commit ▾, outline search, mod+shift+f, Show code, Versions and history), docs-* 8 (Trash). All 46
  run clean on ours (/root/studios-scratch/uip5); batch 2 too, except what ours lacks.
- Still failing on ours (check against live's recordings first): Help menu has no Help / FeatureScript documentation /
  System check (web pages: decide with lead); folder › Set library (live's meaning unknown); version row › Releases
  (live shows only the Free plan's upsell); ver-version-restore (live's Restore to Main… dialog not captured).
- Coverage: documents 207/313 (66.1 %), versions 99/169 (58.6 %), Variable Studio 64/98 (65.3 %), Feature Studio
  74/119 (62.2 %). Match: no live recordings yet. Batches 2 + 3 (196 scenarios) filed with live and live2 (request
  draft /root/studios-scratch/req-batch23.md).
- Next: the universes' remaining items by area (documents page advanced search / column chooser / folders row,
  imported-file tabs, Tab manager filters; versions' repair panel and reference manager; Feature Studio editor items
  need editor targets: gutter, folds, Ctrl+click); then live's recordings as they come: fix mismatches P0/P1 first.

### 2026-11-07 23:44 UTC: batch 2 (147 scenarios), DOCS-SELECT-LEAVES-WAL fixed; batch 1 filed with live
- Batch 1 (198 scenarios, 7a746fd8) filed in live/REQUESTS.md for live and live2 (23:40). Ours: 176 of 185 clean; the
  failures were the driver (controls below the fold: now wheeled into view; the toolbar's Undo not named) and
  ver-version-restore (ours restores at once, live's Restore to Main… dialog not captured: wait for its recording).
- Batch 2: Preferences (Keyboard shortcuts incl. the Edit popup, Shortcut toolbars, export rules, material libraries,
  every unit and decimals, env profile, language save, pen input), Versions and history (Search history, the filter
  panel, Hide merges / automatic versions), Variable Studio (Insert Variable Studio dialog and its rows as
  `panel_row`, table ⋯ `Table menu:<title>`, chevron `Collapse:<title>`, footer checkbox and ?, Configuration panel;
  start vs-two), Feature Studio (Redo, commit all, references, Ctrl+Space, Shift+/, Alt+C), and one scenario for every
  universe item whose `via` had none (/root/studios-scratch/bin/gen_via.py). File it once on main.
- DOCS-SELECT-LEAVES-WAL (qa P2): a read-only handle's close removes the -wal / -shm it made (crates/store).
- Coverage: documents 201/313 (64.2 %), versions 81/174 (46.6 %), Variable Studio 40/98 (40.8 %), Feature Studio
  58/119 (48.7 %).
- Batch 2 on ours (325 recordings): every vs-* and fs-* clean. Gaps ours has against live's menus (check each
  against live's recordings first): documents page — no Labels nav, Help menu lacks Help / FeatureScript
  documentation / System check, folder rows lack Set library; Versions and history — no Copy link on any row, no
  Releases on version rows, Open in new browser tab only on the current workspace's row; Restore to Main… restores
  with no dialog (live's not captured). Compare mode is now in the state (`document.compare`).
- Next: run batch 2 on ours and file it; versions' release (31 items, mostly premium on live's Free plan: check which
  are cloud), compare and merge items; Feature Studio's Import panel, Monitor ▾, notices panel; universes' `gaps`.

### 2026-11-07 23:26 UTC: restarted on a pinned account; the previous agent's work landed
- Page driver (next-step 1): a document opened from the documents page is read as the document now (docs-create-document
  step 4: tabs Part Studio 1, Assembly 1, workspace Main); nothing left to fix there.
- Recorder: the documents page's name prompts (New document / folder, Rename, Create label) note their fields
  (name, labels, location as shown) and buttons; every primary / cancel / danger / secondary button is clickable by its
  text, `modal-ok` / `modal-cancel` by role; Preferences: every section's controls are noted as "<Section> › <label>"
  (Keyboard shortcuts: tab, customized rows, Edit / Reset per row, the Edit shortcut popup as a modal; Shortcut
  toolbars: Toolbar select, ticked Tools list, a checkbox per tool; Material libraries; User export rules and their
  dialog; Language, Theming, Mouse, Drawings, Labs, Updates).
- Starts: `branches` (workspace from Main's head with its own features; start doc-branches; ops test
  `a_start_with_a_workspace_builds`).
- Scenarios: 198 (doc 75, docs 15, prefs 28, ver 30, vs 19, fs 31); ours ran them on :108 (/root/studios-scratch/uip2).
- Coverage: documents 126/313 (40.3 %), versions 47/174 (27.0 %), Variable Studio 18/98 (18.4 %), Feature Studio
  42/119 (35.3 %). Match: no live recordings yet (req-batch1 to file next).

### 2026-11-07 22:31 UTC: P0 "an agent's first MCP calls fail" (lead, user's screenshot of a Claude Desktop session)
- `~` paths: `noshape_scene::expand_home` (`~`, `~/…`, `~\…`), applied in `registry::run` to every path argument
  at any depth (`PATH_ARGS`: doc, path, file, files, folder, csv, template, out…), so every op and the CLI take them.
- `document_create`: a relative path from the MCP server goes in the documents folder (the CLI keeps the shell's
  folder); a file already there: "…already exists: open it with document_open (or document_create with if_exists
  "open"), or choose another name"; `if_exists: "open"`; this session's own unedited document opens either way (an
  agent's retry; the reply says `existed`). `document_open` finds a bare name in the documents folder.
- The MCP instructions were written to the model ("Tell the user…", "Model the way…", "your calls"): rewritten as
  documentation of the tools; a test keeps second person, "the user", "remind", "must", "system" out of them.
- Guard: cli test `tool_list::an_agents_first_calls_work_or_say_what_to_do`.

### 2026-11-07 20:58 UTC: P0 "a Windows user can't find their documents" (lead), then paused again
- One documents folder for everything: `noshape_scene::documents_dir` (`NOSHAPE_DOCUMENTS_DIR`, else the OS's
  Documents Known Folder's `noshape`), used by app-core (`file_dialogs::documents_dir`), ops (`noshape_ops::documents_dir`,
  `document_create`) and the MCP server's instructions (its own copy is gone; the CLI doesn't depend on app-core).
- The documents page's footer shows the folder as the OS writes it (`file_dialogs::display_path`: Windows the full
  path with backslashes, macOS / Linux "~/…" under home) as a link that opens it in the file manager; a document's
  Details panel has its Location (a link that shows the file in its folder); the row menu has "Show in Explorer" /
  "Show in Finder" / "Show in folder" (`show_in_folder_label`). GPUI's test platform records open_with_system /
  reveal_path (vendor/gpui-pre: `opened_paths`, `revealed_paths`); app test `start_links_tests`.
- `document_create`: no path → "Untitled <n>.noshape" in the documents folder; a bare name → that name there
  (".noshape" added); a full path as given; the reply's `path` is absolute (`Ctx::documents` overrides the folder for
  tests). mcp_instructions.md tells agents to create there unless the user asks otherwise and to tell the user the path.
- The .noshape alone is the whole document (user: a friend's 3.7 MB Panda.noshape-wal): crates/store's
  `IdleCheckpoint` folds the log into the file (`wal_checkpoint(TRUNCATE)`, its own connection, never waiting on a
  reader) once the log has been unchanged 1 s; `Document::suspend` / `resume` close the file between MCP calls
  (`Ctx::suspend_files` / `resume_files` around every tools/call), so no -wal / -shm stays; every copy path already
  used VACUUM INTO or a fresh create. Cost (T2 `idle_checkpoint_cost`, vibe3): folding takes 4–7 ms off the UI thread;
  a commit arriving during one waits ≤ 10 ms, and only after a second of idle.

### 2026-11-07 20:05 UTC: paused (quota) until 11-10; where everything stands

Coverage (tools/ui-parity/coverage_<area>.py; non-cloud items; scenarios exercising them, own and others'):

| Area | Coverage | My scenarios | Match |
|---|---|---|---|
| Documents, tabs, start page, preferences | 68/313 (21.7 %) | doc-* 42, docs-* 15, prefs-* 6 | no live recordings yet |
| Versions, branches, history, release | 22/174 (12.6 %) | ver-* 16 | no live recordings yet |
| Variable Studio | 17/98 (17.3 %) | vs-* 19 | no live recordings yet |
| Feature Studio and editor | 19/119 (16.0 %) | fs-* 15 | no live recordings yet |

Ours ran: all 42 doc-* (on the 19:57 build: tab menus per kind, + menu, rename, duplicate, delete, paste, Tab manager,
☰ menu, Workspace units, folders after the fix), docs-create-document, vs-edit-value. Not yet run on ours: ver-*,
fs-*, prefs-*, the rest of docs-* and vs-*. Not yet filed with live: the request is drafted in
/root/studios-scratch/req-batch1.md (README spec, the recorder's new keys and targets, starts, the 113 scenarios);
file it (date -u) once the batches have run on ours.

Next, in order:
1. Page driver: after a step opens a document window (documents page › Create › Document…, a row's double-click) the
   state is still the page's: `start_page`'s `follow` doesn't take the new window (check `cx.windows()` order /
   the window being registered before the read; maybe wait for its `register_page`).
2. Run ver-*, vs-*, fs-*, docs-*, prefs-* on ours (`tools/ui-parity/run.py --app … --display :108 --all <names>`;
   summary script /root/studios-scratch/bin/show.py); fix what fails; file req-batch1 with live and live2.
3. Ours' findings already visible (check against live's recordings before changing anything): Esc doesn't close
   ☰ › Workspace units… (live unknown: universe gap); tab › Copy to clipboard then + › Paste names the copy
   "Part Studio 2 1" and puts it after the source (live unknown).
4. Preferences: only Units and Decimal format note their controls (`note_field`); the other sections (Language,
   Theming, Mouse, Part Studio / Assembly settings, Shortcut toolbars, Drawings, export rules, material libraries)
   still need it before prefs-* batches for them.
5. Coverage gaps by size: documents page (14/71), Preferences (8/74), release (0/31), merge (0/12), Feature Studio
   toolbar menus and panels; then the universes' `gaps` lists go to live as capture requests.

### 2026-11-07 evening: measurement in place (first lands)
- Universes from live's captures: tools/ui-parity/universe/{documents,versions,variable-studio,feature-studio}.json
  (353 / 179 / 100 / 123 items; 40 / 5 / 2 / 4 cloud, not counted). Each item has its `source`; `via` says which
  step chains exercise it (tools/ui-parity/coverage_universe.py). Scratch generators: /root/studios-scratch/bin/.
- Driver (crates/app/src/ui_script.rs): every state has `document` (tab bar, folder, inline rename, left panel,
  Ctrl+Space switcher, the workspace / version label, the history panel's rows); tabs that are no Part Studio,
  Assembly or Drawing get a generic state; page scenarios (`"page"` starts: the documents page, Preferences) run
  across windows (`start_page`), each window registering its state reader (app-core `ui_script::register_page`).
  README sections "Documents, tabs and preferences", "Documents page and Preferences".
- Starts: `doc-new`, `doc-tabs`, `doc-versions`, `docs-page`, `prefs-page` (starts.json keys `tabs`, `versions`,
  `page`, `documents`).
- Batches written: Documents 1 (42 doc-*: tabs, + menu, folders, ☰, Tab manager, Workspace units), Documents 2 (21
  docs-* / prefs-*), Versions 1 (16 ver-*), Variable Studio 1 (19 vs-*, start vs-vars), Feature Studio 1 (15 fs-*,
  start fs-studio).
