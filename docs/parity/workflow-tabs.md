# Workflow: multi-tab document work (rename, duplicate, move, folders)

qa5, 2026-10-21 pass 3, /tmp/qa5/noshape-1448, qa5-undo.noshape. Live: behaviours.md tab menus (line ~280), CAD Imports folder (line ~34);
the rest pending (REQUESTS.md "qa5 tabs"). Status: ✅ ok, ✗ differs, ? live pending.

| # | Step | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Double-click a tab | inline rename | inline rename, text selected | ✅ | a6c56638 10-28 |
| 2 | Type "Part Studio 1" + Enter | name with spaces | **"PartStudio1": spaces are swallowed** (also "a b" → "ab"); Space works in other text fields | ✗ **DOC6** → now ✅ DOC6 fixed (a6c56638; qa4 re-check "r t x" kept) |  |
| 3 | Rename to an existing tab's name | ? | allowed (became "PartStudio1" because of DOC6) | ? → now ✅ DOC6 fixed (a6c56638; qa4 re-check "r t x" kept) |  |
| 4 | Part Studio tab right-click | Delete \| Open in new browser tab, Rename, Properties…, Show code \| Duplicate, Copy to clipboard \| Select as document thumbnail \| Move to document…, Export… | same minus "Open in new browser tab" (browser-only) | ✅ | a6c56638 10-28 (+ Export… last, live rmb-tab-partstudio-2 / TAB-EXPORT-MISSING) |
| 5 | Duplicate | ? | "Part Studio 1 (copy)", inserted right after the source, not activated | ? → now ✅ TABS2 fixed ("<name> Copy 1") |  |
| 6 | Drag a tab 2 slots right | ? | ghost label follows the cursor, blue drop line; a tab hover-preview card pops up during the drag; drop moves the tab | ✅ / ? (preview card) → now ✅ live has no card during a drag (row 6 below), ours neither (TABS1 a6c56638) | a6c56638 10-28 |
| 7 | + › Create folder | ? | "Folder 1" right after the active tab, in rename mode | ? → now ✅ = live ("Folder 1" right after the active tab, in rename mode; behaviours "qa5 tabs" (c)) |  |
| 8 | Drag a tab onto the folder | ? | folder highlights light blue; tab moves into it | ? → now ✅ = live (the folder highlights light blue, the tab moves in; behaviours "qa5 tabs" (c)) |  |
| 9 | Open the folder | breadcrumb 🏠 › 📁 CAD Imports › tabs (live, line 34) | 🏠 › Folder 1 › PartStudio1 | ✅ | a6c56638 10-28 |
| 10 | New tab via + while a middle tab is active | inserted after the active tab | inserted there, **later shown at the end** (qa5 assembly "Assembly 2", also after reopening) | ✗ DOC5 → now ✅ DOC5 fixed (stays after reopen; qa4 re-check) |  |

## Update from live's answer (behaviours.md "qa5 tabs", 10-21)

| # | Row | Live | Ours | Verdict |
|---|---|---|---|---|
| 2/3 | Rename (duplicate name) | allowed silently | allowed; but spaces are swallowed | ✅ / ✗ DOC6 → now ✅ DOC6 fixed (a6c56638), duplicates allowed as live (row 3) |
| 5 | Duplicate | "**<name> Copy 1**", right after the source, not activated | "Part Studio 1 (copy)", after the source, not activated | ✗ **TABS2** (name) → now ✅ TABS2 fixed ("<name> Copy 1") |
| 6 | Drag | 100 px **ghost** (bg #eaeaea, shadow 0 5px 10px rgba(0,0,0,.2), label truncated), horizontal only; the other tabs close up and a **gap placeholder** (tab width) shows the drop place; **no line indicator**; dropping past the last tab does nothing | ghost label, a **blue drop line**, tabs don't open a gap; a **tab hover-preview card** pops up during the drag | ✗ **TABS1** → now ✅ TABS1 (a6c56638 10-28: ghost, gap placeholder, no line, no card; row "TABS1 drag" below) |
| 7 | Create folder | "Folder 1" right after the active tab, inline rename, not activated | same | ✅ |
| 8 | Drop on folder | folder light-blue highlight, ghost translucent over it, tab leaves the bar | same highlight; tab moves in | ✅ |
| 9 | Open folder | breadcrumbs 🏠 › 📁 Folder 1 › tabs; 🏠 returns | same | ✅ |
| – | Folder right-click | **Rename, Unpack, Move to document…** only; Unpack moves its tabs to the end, no dialog; no Delete | not checked | ? (next pass) → now ✅ TABS3 fixed (Rename, Unpack, Move to document…) |
| 10 | New tab from + while a middle tab is active | right after the active tab, **activated**, **stays there after reload** | inserted after the active tab and activated, later shown at the end (also after reopening) | ✗ DOC5 confirmed → now ✅ DOC5 fixed (stays after reopen; qa4 re-check) |
| – | Tab Delete | immediate, no confirmation, no undo | not checked | ? → now ✅ immediate, no confirmation (row "Tab › Delete" below) |
| – | Tab rename / reorder / delete / duplicate undoable? | **not undoable** | not checked | ? → now ✅ TABS5 fixed (rename / duplicate not undoable; skasm 10-31 CLI: tab_rename / tab_duplicate / tab_move leave the undo stack alone) |

### Follow-up checks (pass 4, 2026-10-21 16:35, /tmp/qa5/noshape-1448)
| Row | Live | Ours | Verdict |
|---|---|---|---|
| Folder right-click | Rename, Unpack, Move to document… | Rename, **Move to parent folder** (disabled), **Unpack folder**, Move to document… | ✗ **TABS3** → now ✅ TABS3 fixed (Rename, Unpack, Move to document…) |
| Unpack | no dialog; the folder's tabs go to the **end of the tab list** | no dialog; tab placed where the folder was | ✗ **TABS4** → now ✅ TABS4 fixed (Unpack → end of the tab list) |
| Rename, then Ctrl+Z | not undoable | **Ctrl+Z reverts the rename** | ✗ **TABS5** → now ✅ TABS5 fixed (rename / duplicate not undoable; skasm 10-31 CLI: tab_rename / tab_duplicate / tab_move leave the undo stack alone) |
| Tab › Delete | immediate, no confirmation, no toast | immediate, no confirmation | ✅ |

## Re-run on integration 5d28447 (2026-10-21 17:05–17:20, /tmp/qa5/noshape-1659 = /root/target-lead/debug/noshape 16:59; no CLI built from 5d28447 yet, so CLI-only geometry rows stay on the 14:36 CLI result)
| DOC6 rename spaces | "r t" → "rt" | ✗ still → now ✅ DOC6 fixed (a6c56638; qa4 re-check "r t x" kept) |
| TABS5 rename undoable | Ctrl+Z reverts it | ✗ still → now ✅ TABS5 fixed (rename / duplicate not undoable; skasm 10-31 CLI: tab_rename / tab_duplicate / tab_move leave the undo stack alone) |
| DOC5 new tab position | Assembly 3 after Part Studio 1, **stays there after reopening** (the old Assembly 2 remains at the end) | ✅ **fixed** for new tabs |

## Re-run pass 7 (2026-10-21 18:56–19:50, app /tmp/qa5/noshape-1851 = target-shared 18:51 (tree d5745c9 + uncommitted work), CLI /tmp/qa5/noshape-cli-1809 = target-engine 18:09)
| Row | Ours | Status | Verified |
|---|---|---|---|
| DOC6 rename with spaces | "r t" kept | ✅ **fixed** | a6c56638 10-28 |
| TABS5 rename undoable | Ctrl+Z still reverts the rename | ✗ still → now ✅ TABS5 fixed (rename / duplicate not undoable; skasm 10-31 CLI: tab_rename / tab_duplicate / tab_move leave the undo stack alone) |  |
| TABS2 Duplicate name | "Part Studio 4 Copy 1", right after the source, not activated | ✅ **fixed** | a6c56638 10-28 |
| TABS1 drag | ghost "Part St…" follows, gap placeholder opens, no line, no preview card | ✅ **fixed** | a6c56638 10-28 |
| TABS3 folder menu | Rename · Unpack · Move to document… | ✅ **fixed** | a6c56638 10-28 |
| TABS4 Unpack | the folder's tab goes to the end of the tab list | ✅ **fixed** | a6c56638 10-28 |

## Re-run pass 8 on main 5bd50b2 (2026-10-21 23:13–00:10, app /tmp/qa5/noshape-2256 = /root/target-lead/debug/noshape 22:56; no newer CLI than target-engine 18:09, so CLI-only rows stay on pass 7; geometry rows re-read in the app where possible)
| TABS5 rename undoable | Ctrl+Z leaves the rename (it undoes the studio's last feature instead) | ✅ **fixed** |

- 10-22 (ui): DOC5 re-checked in the app. + after Part Studio 1 puts the new tab right after it; it stays there after an edit, Ctrl+Z, tab switches and a reopen ✅.

## Re-check on main 88fbf8f5 (qa4, 2026-10-23 18:00–19:00, FRESH=1, /root/qa4-scratch/bin/noshape-m88)
| Finding | Result |
|---|---|
| DOC6 rename with spaces | ✅ "r t x" kept |
| TABS2 Duplicate name | ✅ "r t x Copy 1", right after the source, not activated |
| TABS5 rename / duplicate undoable | ✅ Ctrl+Z doesn't revert them |
| DOC5 new tab after the active middle tab | ✅ stays there after quit / reopen |
| Row 6 drag look | ✅ ~100 px ghost "r t x Co…", gap placeholder, no drop line |
| TABS3 folder menu | ✅ Rename, Unpack, Move to document… |
| TABS4 Unpack | ✅ the tab goes to the end of the tab list, folder removed (CLI document_info order) |
All ✗ rows of this file are fixed.

## ui2 look pass (2026-10-24, ui2 build of 13:50; live document/1210–1234, dark dk-920)
qa4's 10-23 re-check covers the behaviour rows above (all fixed); this pass compares the look in both themes.

| Item | Live | Ours | Status | Verified |
|---|---|---|---|---|
| Tab bar, light / dark | grey bar, white active tab with blue underline / dark tabs, white text | same | ✅ |  |
| Tab right-click menus, per type (live ui2-tabmenu-*, 10-24) | Delete \| Open in new browser tab, Rename, Properties…, (Part Studio) Show code \| Duplicate, Copy to clipboard \| (active Drawing only) Change to version… \| (Assembly) Create Drawing of … \| (Part Studio, Assembly, Drawing) Select as document thumbnail \| Move to document…, (Drawing only) Export…, (Assembly, Drawing, Variable Studio) Release… | had no "Open in new browser tab", Export… on Part Studios and Assemblies, Change to version… on inactive Drawings (fixed: the new-tab item opens another window on the same document with that tab; Export… on Drawings only; Change to version… on the active Drawing) | ✅ ("Upgrade" pill ⊘) | a6c56638 10-28 (Export… on PS/Asm tabs since dea57d5c, TAB-EXPORT-MISSING) |
| Tab / folder inside a folder | "Move to parent folder" (tabs after Properties…; folders: Rename, Move to parent folder, Unpack) | missing (fixed) | ✅ | a6c56638 10-28 |
| + menu | Applications ▸, Create Material Library / Feature Studio, CAM / PCB / Render Studio [Upgrade] \| Part Studio, Assembly, Variable Studio, Drawing…, folder, Import… (1224) | same items and widths, minus Applications and the paid app studios | ✅ (⊘ cloud / paid apps) | a6c56638 10-28 |
| Folder tab | folder icon + name, normal tab chrome (1226) | same | ✅ | a6c56638 10-28 |
| Opened folder (breadcrumbs) | 🏠 › Folder 1 as chevron crumbs outside the scrolling tabs: 27 high, 🏠 #eaeaea / #1c1c1c, the open folder white / #333 with a #666 / #ccc regular name (≤ 100 px); 8 px points, notched flags, 18 px apart; deeper than one level 🏠 › … › B, "…" (87 px) opens the hidden folders upward; dark: filled folder icons (ui2-crumbs-*) | square crumbs inside the scrolling strip, the folder name bold (fixed) | ✅ TABS6 | a6c56638 10-28 |
| Drag ghost / gap, drop on folder | (qa4 10-23) | ✅ | ✅ | a6c56638 10-28 |
