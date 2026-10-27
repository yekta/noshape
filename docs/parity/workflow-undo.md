# Workflow: undo/redo stress across studios

qa5, 2026-10-21 pass 3, /tmp/qa5/noshape-1448, document qa5-undo.noshape (copy of qa5 assembly).
Live behaviour pending (REQUESTS.md "qa5 undo across tabs"). Status: ✅ ok, ✗ differs/broken, ? live pending.

| # | Step | Ours | Status | Verified |
|---|---|---|---|---|
| 1 | Part Studio 1: Fillet (Shift+F, Enter with nothing picked) | "Fillet 1" added in red (error) | ? (live allows ✓ with errors) |  |
| 2 | Assembly 1: i, insert Part 2, drop, ✓ | Part 2 <2> | ✅ |  |
| 3 | Drawing 1: open it | views + BOM **updated by themselves** (quantity 2) without any "Update" prompt | ? |  |
| 4 | Back in Part Studio 1, Ctrl+Z | removes Fillet 1 only (per-tab history; the newer assembly/drawing changes stay) | ? (live) |  |
| 5 | Assembly 1, Ctrl+Z right after the tab switch | **nothing visible**, Undo stays enabled; the next Undo (button) removes Part 2 <2> → an invisible extra history step | ✗ **UNDO1** |  |
| 6 | Redo, Ctrl+Z, Ctrl+Y after tab round trips | work by keyboard without clicking first | ✅ |  |
| 7 | Second Ctrl+Z (nothing visible again) | Undo still enabled, no visible change | ✗ UNDO1 |  |
| 8 | Drawing after the assembly undo | **"Out of date — Update (Ctrl+Q)"** now (step 3 updated silently); Ctrl+Q fixes it | ✗ **UNDO2** (inconsistent: one change auto-updates the drawing, the undo of it needs Update) |  |
| 9 | Undo button tooltip | "Undo (cmd+z/ctrl+z)" | ? |  |
| – | No crash, no hang in ~15 undo/redo operations across 3 tabs | | ✅ |  |

## Update from live's answer (behaviours.md "qa5 undo across tabs", 10-21)

| # | Row | Live | Ours | Verdict |
|---|---|---|---|---|
| 4 | Ctrl+Z in a Part Studio | per-tab stack: only that tab's last change; no toast, no tab switch | same | ✅ |
| 5/7 | Assembly: Insert session + mate, Ctrl+Z | #1 removes the mate, #2 removes **all instances of one Insert session at once**, #3 nothing | after one insert (no mate): #1 **nothing visible** (Undo stays enabled), #2 (button) removes the instance | ✗ UNDO1 (an extra invisible step) |
| – | Drawing note | **two** undo steps (1st leaves the note, enables Redo; 2nd removes it) | not checked | ? |
| 9 | Undo tooltip | always "Undo (cmd+z/ctrl+z)" / "Redo (ctrl+y/shift+cmd+z)" | "Undo (cmd+z/ctrl+z)" | ✅ |
| – | Stacks after a reload | cleared | not checked | ? |
| – | Tab ops (rename, reorder, delete, duplicate) | not undoable | not checked | ? |

## Re-run on integration 5d28447 (2026-10-21 17:05–17:20, /tmp/qa5/noshape-1659 = /root/target-lead/debug/noshape 16:59; no CLI built from 5d28447 yet, so CLI-only geometry rows stay on the 14:36 CLI result)

## Re-check on main 88fbf8f5 (qa4, 2026-10-23 18:00–19:00, FRESH=1, /root/qa4-scratch/bin/noshape-m88)
| Finding | Result |
|---|---|
| UNDO1 insert + Ctrl+Z | ✅ one Ctrl+Z removes the inserted instance (Instances 4 → 3), no invisible step |
| UNDO2 drawing "Out of date" after an assembly undo | fix cited in code; not re-run |

## Re-run pass 21 (2026-10-23, main 40e23871 binary /root/target-lead/debug/noshape)

| Row | Ours now | Status | Verified |
|---|---|---|---|
| UNDO1 | Insert the qa5 bool Part Studio (3 instances) into Assembly 1, ✓, switch to the Part Studio tab and back, Ctrl+Z once: all 3 instances go (Instances (0)); Ctrl+Y brings them back | ✅ fixed |  |
| UNDO2 | Drawing of Assembly 1 (suggested views): an assembly insert shows the drawing's Update button highlighted (out of date); after Ctrl+Q, Ctrl+Z in the assembly shows it out of date again. Both directions behave the same | ✅ fixed |  |

## Re-run pass 24 (qa5, 2026-10-24 ~00:10, main 538981d5, target-lead 23:22; d10 drawing copy)
| Row | Live | Ours | Status | Verified |
|---|---|---|---|---|
| Drawing note, Ctrl+Z | **two** steps: the 1st leaves the note and enables Redo, the 2nd removes it (behaviours "qa5 undo across tabs") | one Ctrl+Z removes "hello" at once (Undo greys, Redo enabled); Ctrl+Y brings it back | ✗ **UNDO3** (asb, minor) |  |

## Release re-check v0.4.8 (qa5, 2026-10-25 02:30, release asset noshape 0.4.8 linux-x86_64)
| Row | Ours | Status | Verified |
|---|---|---|---|
| UNDO1 | Insert Part Studio 1 (2 parts) into Assembly 1, ✓, switch to the Part Studio tab and back, Ctrl+Z once: Instances (0); Ctrl+Y: both back | ✅ |  |
| UNDO2 | Drawing of Assembly 1 (suggested views): Update button pale (disabled) when created; second insert in the assembly → saturated yellow (out of date); Ctrl+Q → pale; Ctrl+Z in the assembly (Instances 4 → 2) → saturated again. Both directions agree (live: pale / saturated, behaviours 6902) | ✅ |  |
| UNDO3 | not re-run (asb) | — |  |
