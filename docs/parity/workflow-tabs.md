# Workflow: multi-tab document work (rename, duplicate, move, folders)

qa5, 2026-10-21 pass 3, /tmp/qa5/noshape-1448, qa5-undo.noshape. Live: behaviours.md tab menus (line ~280), CAD Imports folder (line ~34);
the rest pending (REQUESTS.md "qa5 tabs"). Status: ✅ ok, ✗ differs, ? live pending.

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 1 | Double-click a tab | inline rename | inline rename, text selected | ✅ |
| 2 | Type "Part Studio 1" + Enter | name with spaces | **"PartStudio1": spaces are swallowed** (also "a b" → "ab"); Space works in other text fields | ✗ **DOC6** |
| 3 | Rename to an existing tab's name | ? | allowed (became "PartStudio1" because of DOC6) | ? |
| 4 | Part Studio tab right-click | Delete \| Open in new browser tab, Rename, Properties…, Show code \| Duplicate, Copy to clipboard \| Select as document thumbnail \| Move to document…, Export… | same minus "Open in new browser tab" (browser-only) | ✅ |
| 5 | Duplicate | ? | "Part Studio 1 (copy)", inserted right after the source, not activated | ? |
| 6 | Drag a tab 2 slots right | ? | ghost label follows the cursor, blue drop line; a tab hover-preview card pops up during the drag; drop moves the tab | ✅ / ? (preview card) |
| 7 | + › Create folder | ? | "Folder 1" right after the active tab, in rename mode | ? |
| 8 | Drag a tab onto the folder | ? | folder highlights light blue; tab moves into it | ? |
| 9 | Open the folder | breadcrumb 🏠 › 📁 CAD Imports › tabs (live, line 34) | 🏠 › Folder 1 › PartStudio1 | ✅ |
| 10 | New tab via + while a middle tab is active | inserted after the active tab | inserted there, **later shown at the end** (qa5 assembly "Assembly 2", also after reopening) | ✗ DOC5 |

## Update from live's answer (behaviours.md "qa5 tabs", 10-21)

| # | Row | Live | Ours | Verdict |
|---|---|---|---|---|
| 2/3 | Rename (duplicate name) | allowed silently | allowed; but spaces are swallowed | ✅ / ✗ DOC6 |
| 5 | Duplicate | "**<name> Copy 1**", right after the source, not activated | "Part Studio 1 (copy)", after the source, not activated | ✗ **TABS2** (name) |
| 6 | Drag | 100 px **ghost** (bg #eaeaea, shadow 0 5px 10px rgba(0,0,0,.2), label truncated), horizontal only; the other tabs close up and a **gap placeholder** (tab width) shows the drop place; **no line indicator**; dropping past the last tab does nothing | ghost label, a **blue drop line**, tabs don't open a gap; a **tab hover-preview card** pops up during the drag | ✗ **TABS1** |
| 7 | Create folder | "Folder 1" right after the active tab, inline rename, not activated | same | ✅ |
| 8 | Drop on folder | folder light-blue highlight, ghost translucent over it, tab leaves the bar | same highlight; tab moves in | ✅ |
| 9 | Open folder | breadcrumbs 🏠 › 📁 Folder 1 › tabs; 🏠 returns | same | ✅ |
| – | Folder right-click | **Rename, Unpack, Move to document…** only; Unpack moves its tabs to the end, no dialog; no Delete | not checked | ? (next pass) |
| 10 | New tab from + while a middle tab is active | right after the active tab, **activated**, **stays there after reload** | inserted after the active tab and activated, later shown at the end (also after reopening) | ✗ DOC5 confirmed |
| – | Tab Delete | immediate, no confirmation, no undo | not checked | ? |
| – | Tab rename / reorder / delete / duplicate undoable? | **not undoable** | not checked | ? |

### Follow-up checks (pass 4, 2026-10-21 16:35, /tmp/qa5/noshape-1448)
| Row | Live | Ours | Verdict |
|---|---|---|---|
| Folder right-click | Rename, Unpack, Move to document… | Rename, **Move to parent folder** (disabled), **Unpack folder**, Move to document… | ✗ **TABS3** |
| Unpack | no dialog; the folder's tabs go to the **end of the tab list** | no dialog; tab placed where the folder was | ✗ **TABS4** |
| Rename, then Ctrl+Z | not undoable | **Ctrl+Z reverts the rename** | ✗ **TABS5** |
| Tab › Delete | immediate, no confirmation, no toast | immediate, no confirmation | ✅ |
