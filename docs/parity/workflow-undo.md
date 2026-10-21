# Workflow: undo/redo stress across studios

qa5, 2026-10-21 pass 3, /tmp/qa5/noshape-1448, document qa5-undo.noshape (copy of qa5 assembly).
Live behaviour pending (REQUESTS.md "qa5 undo across tabs"). Status: ✅ ok, ✗ differs/broken, ? live pending.

| # | Step | Ours | Status |
|---|---|---|---|
| 1 | Part Studio 1: Fillet (Shift+F, Enter with nothing picked) | "Fillet 1" added in red (error) | ? (live allows ✓ with errors) |
| 2 | Assembly 1: i, insert Part 2, drop, ✓ | Part 2 <2> | ✅ |
| 3 | Drawing 1: open it | views + BOM **updated by themselves** (quantity 2) without any "Update" prompt | ? |
| 4 | Back in Part Studio 1, Ctrl+Z | removes Fillet 1 only (per-tab history; the newer assembly/drawing changes stay) | ? (live) |
| 5 | Assembly 1, Ctrl+Z right after the tab switch | **nothing visible**, Undo stays enabled; the next Undo (button) removes Part 2 <2> → an invisible extra history step | ✗ **UNDO1** |
| 6 | Redo, Ctrl+Z, Ctrl+Y after tab round trips | work by keyboard without clicking first | ✅ |
| 7 | Second Ctrl+Z (nothing visible again) | Undo still enabled, no visible change | ✗ UNDO1 |
| 8 | Drawing after the assembly undo | **"Out of date — Update (Ctrl+Q)"** now (step 3 updated silently); Ctrl+Q fixes it | ✗ **UNDO2** (inconsistent: one change auto-updates the drawing, the undo of it needs Update) |
| 9 | Undo button tooltip | "Undo (cmd+z/ctrl+z)" | ? |
| – | No crash, no hang in ~15 undo/redo operations across 3 tabs | | ✅ |

## Update from live's answer (behaviours.md "qa5 undo across tabs", 10-21)

| # | Row | Live | Ours | Verdict |
|---|---|---|---|---|
| 4 | Ctrl+Z in a Part Studio | per-tab stack: only that tab's last change; no toast, no tab switch | same | ✅ |
| 5/7 | Assembly: Insert session + mate, Ctrl+Z | #1 removes the mate, #2 removes **all instances of one Insert session at once**, #3 nothing | after one insert (no mate): #1 **nothing visible** (Undo stays enabled), #2 (button) removes the instance | ✗ UNDO1 (an extra invisible step) |
| – | Drawing note | **two** undo steps (1st leaves the note, enables Redo; 2nd removes it) | not checked | ? |
| 9 | Undo tooltip | always "Undo (cmd+z/ctrl+z)" / "Redo (ctrl+y/shift+cmd+z)" | "Undo (cmd+z/ctrl+z)" | ✅ |
| – | Stacks after a reload | cleared | not checked | ? |
| – | Tab ops (rename, reorder, delete, duplicate) | not undoable | not checked | ? |
