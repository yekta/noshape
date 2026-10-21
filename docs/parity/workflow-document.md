# Workflow: document management (versions, branches, merge, copy/paste, derived, export formats)

Document "qa5 advanced". Live reference: behaviours.md "Documents page" (Versions and history panel), "Viewing a
version", "Version/workspace Properties … Create version… Branch to create workspace…", "Merge dialog (ui2 10-21)",
"Document chrome" (left rail, tab bar), G25/G29 (export headers).
Status: ✅ matches live, ✗ differs (finding #), ? live data missing.

**Last run: 2026-10-21 (session 13:37–14:29), qa5**, binary /tmp/qa5/noshape-1337.

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 1 | Left rail › Versions and history | panel (Search history ▾ 🔍, graph, Name/Modified, legend Workspace/Version/Change/Release candidate/Release/Contains obsolete/Automatic version, "Right-click on a row to perform actions") | same | ✅ |
| 2 | Rail › Create version… | "Create version from **Main**": Name V1 (selected), Description (Maximum of 10000 characters), Note, [Create] [Create version and edit properties] [Cancel] | same; Enter creates | ✅ |
| 3 | Select a feature row, Delete | deletes (core F15) | deletes | ✅ (F15 fixed) |
| 4 | Click V1 | opens V1: grey "V1" in the navbar, white bar "Versions are view only. Viewing V1." [⊞ Custom features] [Return to Main] (?); editing tools hidden | same | ✅ |
| 5 | V1 right-click | Open · Open in new browser tab · Branch to create workspace… · Copy link · Compare · View in repair · Restore to Main… · Properties… · Releases [Upgrade] · Publish FeatureScript… | Open · Branch to create workspace… · Compare · View in repair · Restore to Main… · Properties… · Publish FeatureScript… (browser/cloud/paid items omitted) | ✅ |
| 6 | Branch to create workspace… | "Create workspace from **V1**": Name, Description, ☑ Open new workspace, Note, [Create] [Create workspace and edit properties] [Cancel] | same (Name prefilled "B1") | ✅ / ? (prefill) |
| 7 | Create | switches to the new workspace; graph gets a coloured branch line | "B1" in the navbar, yellow branch line | ✅ |
| 8 | Main right-click (from B1) | Open, Open in new browser tab, Copy link, Compare, Merge into current workspace…, Workspace protection…, Properties…, Delete | Open, Compare, Merge into current workspace…, Workspace protection…, Properties… (**no Delete**) | ✗ DOC4 (Delete) |
| 9 | Merge into current workspace… | "Merging changes into [B1] from [Main]"; **three** overall cards: Keep [B1] / **Merge changes (default)** "Combine all changes to N tabs" / Replace with [Main]; Part Studio rows offer Keep / **Merge changes from both** / Replace; Merge button enabled | only **Keep [B1]** (default) / Replace with [Main]; rows offer Keep / Replace only; **Merge disabled** until a row is changed | ✗ DOC2 |
| 10 | Row Part Studio 3 → Replace with Main, Merge | applied | applied (Mirror 2 gone on B1) | ✅ |
| 11 | Merge dialog look | header chips with branch colours, Tabs table, "N tabs have no changes" group expanded | same structure | ✅ |
| 12 | Part right-click › Copy Part 1, other Part Studio, Ctrl+V | Derived feature of the copied part | "Derived 1" dialog opens with the part previewed; **Part Studio field shows the placeholder "Select Part Studio…"** | ✗ DOC3 |
| 13 | ✓ | Derived 1, part "Part 1" | same | ✅ |
| 14 | Right-click empty viewport | Show all, Select ▸, Zoom to fit, Isometric | same | ✅ |
| 15 | Part › Export… format list | PARASOLID, ACIS, STEP, IGES, PVZ, JT, RHINO, GLTF, GLB, OBJ, 3MF, STL (DXF/DWG via Export as DXF/DWG) | same | ✅ |
| 16 | IGES options | ? | ☐ Export models oriented Y axis up, ☑ Use compatibility mode for surfaces and curves, Options | ? |
| 17 | IGES file | Onshape header | global section "Open CASCADE IGES processor 8.0", file name "**Filename.iges**", author "root" | ✗ DOC1 |
| 18 | Info toast "Sketch 2 has been canceled. Restore" | goes away | still shown ~1 h later, across tabs and workspaces | ✗ C5/SM10 |

## Findings (2026-10-21)

| # | Finding | Repro | Owner |
|---|---|---|---|
| DOC2 | Merge dialog lacks Onshape's default "Merge changes" strategy (overall card and per-row "Merge changes from both"); defaults to Keep current with Merge disabled | branch B1 from V1, edit different Part Studios on Main and B1, merge Main into B1 | ui2 (+ model merge) |
| DOC3 | Paste of a copied part opens Derived with an empty "Part Studio" field | Copy Part 1 in Part Studio 3, Ctrl+V in Part Studio 1 | psb |
| DOC1 | IGES export header names Open CASCADE and "Filename.iges" (same class as qa4 F26 for STEP) | Part › Export… IGES | kern |
| DOC4 | Workspace row menu has no Delete | Versions and history, right-click another workspace | ui2 |

## Re-run pass 3 (2026-10-21 15:40, CLI /tmp/qa5/noshape-cli-1436; UI /tmp/qa5/noshape-1448)

| # | Row | Ours | Status |
|---|---|---|---|
| 17 | IGES global section | still "Open CASCADE IGES processor 8.0", "Filename.iges", author "root" | ✗ DOC1 |
| – | Parasolid header | still `APPL=noshape`, `SCH_1200000_12006` | ✗ I5 |
| – | Tab created with + after the active tab | shown at the end of the tab bar later and after reopening (Assembly 2 in qa5 assembly) | ✗ DOC5 (live request "qa5 tabs" (e)) |

## Re-run on integration 5d28447 (2026-10-21 17:05–17:20, /tmp/qa5/noshape-1659 = /root/target-lead/debug/noshape 16:59; no CLI built from 5d28447 yet, so CLI-only geometry rows stay on the 14:36 CLI result)
| DOC5 | new tabs keep their position after reopening | ✅ fixed |
