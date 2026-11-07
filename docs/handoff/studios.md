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
