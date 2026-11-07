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
