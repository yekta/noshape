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

### Set library: who reads the library folders (for the lead to route)
`noshape_ops::prefs::LibraryFolders::load().0` maps a library key to its folder (op `documents_set_library`); each
consumer lists that folder's `.noshape` documents (and their elements) next to Onshape's own library, as live's "My …"
entries. Nothing reads them yet:
- `frame_profiles` "My frame profiles library": the Frame feature's profile picker (part-studio-ui `select_item.rs`,
  model-core `regen/features/frame_library.rs`: today only "Onshape frame profiles"): core / psui.
- `sheet_metal_forms` "My sheet metal forms": the sheet metal Form feature's form picker (custom
  `host/sheetmetal.rs`, model-core `regen`): core (sheet metal).
- `tolerances` "My default tolerances": the default tolerance tables (model-data `fits/tables.rs`, the hole and
  dimension tolerance pickers in Part Studios and drawings): core + dx.
- `images` "My images": Insert image in drawings (drawing-ui `drawing_images.rs`) and sketches: dx + skasm.
- `standard_content` "My standard content": the assembly's Insert › Standard content (assembly-ui
  `assembly_standard.rs`): asm.

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
