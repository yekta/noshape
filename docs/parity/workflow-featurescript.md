# Workflow: FeatureScript custom feature (Feature Studio → Part Studio)

qa5, 2026-10-21 pass 4, /tmp/qa5/noshape-1448, document qa5-fs.noshape. Source: /root/qa5-scratch/pass3/qacube.fs
("QA cube": one Length parameter "Size" with LENGTH_BOUNDS, fCuboid from the origin). Live: behaviours.md "Feature Studio" (~line 1572);
open points requested ("qa5 featurescript"). Status: ✅ matches live/expected, ✗ differs, ? live pending.

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 1 | + › Create Feature Studio | tab "Feature Studio 1", Ace editor, lines 1–2 template, **empty lines 3–4**, caret on 4; toolbar Versions, Undo/Redo, New feature, 6 snippet ▾, Import, Format, Commit ▾ (pale), Search tools, Monitor Part Studio 1 ▾, Module outline ▾ | same toolbar and template, caret line 4 ("Line 4 , Column 1"), but **5 lines** | ✅ / ✗ **FS1** (minor) |
| 2 | Paste the source, Ctrl+S | commit; Commit pale again | same; Module outline shows "qaCube(conte…" | ✅ |
| 3 | Part Studio toolbar | ? (same-document custom features) | a new split button "Custom features in this workspace" ▾ → header "Custom features" + "QA cube" | ? |
| 4 | QA cube dialog | – | "QA cube 1", Size **25 mm** selected; preview cube | ? (default for LENGTH_BOUNDS in a mm doc) |
| 5 | ✓ | – | Part 1, **15625.000 mm³** | ✅ |
| 6 | Commit with `fCuboidX(` (undefined function) | parse/compile: ⚠ "Variable x not found." style warning at line:col, red {!} navbar icon for errors | ⚠ in the gutter at 12 + squiggle; **no tooltip on hovering the ⚠**; navbar { } icon unchanged | ✗ **FS2** (tooltip) / ? (icon for runtime errors) |
| 7 | Part Studio | feature red; tooltip "QA cube 1 did not regenerate properly: …"? | row red, tooltip only **"Error regenerating."**; Parts (0) but the **volume readout still shows 15625.000 mm³** | ✗ ADV3 / ✗ **FS3** (stale readout) |
| 8 | { } › FeatureScript notices | panel: Filter notices, per-studio sections, Monitor ▾, "Result: …" | Feature Studio 1 ⚠ "Function fCuboidX with 3 argument(s) not found 12:9"; Part Studio 1 ⚠ + {!} same text with stack 12:91 Feature Studio 1 (const qaCube) / 61:17 onshape/std/feature.fs (defineFeature) / Part Studio 1 (QA cube 1); "Result: Regeneration failed: Error regenerating." | ✅ (texts ?) |
| 9 | Fix the code, commit | regenerates | Part 1 back | ✅ |

## Update from live's answer (behaviours.md "qa5 featurescript", 10-21)

| # | Row | Live | Ours | Verdict |
|---|---|---|---|---|
| 3 | Toolbar | no own button; toolbar ends with "**Add custom features**" + "**Custom features in this workspace**" ▾ (feature-studio icon); entries with a **two-letter initials icon** ("Qc"), tooltip "QA cube" + italic "Feature from current workspace" | "Custom features in this workspace" ▾ with header "Custom features" + "QA cube" (small generic icon) | ✗ **FS4** (initials icon, tooltip line; "Add custom features" button + header row to check) |
| 4 | Dialog | "QA cube 1", Size **25 mm** focused, ✓ enabled | same | ✅ |
| 5 | Volume | 15625.0000000000 | 15625.000 | ✅ |
| 6 | Gutter after the broken commit | ⚠ on **line 1**, orange wavy underline under the whole call on line 12; tooltip "⚠ Function fCuboidX with 3 argument(s) not found" (gutter or underline hover) | ⚠ on line 12, short squiggle, **no tooltip** | ✗ FS2 |
| 7 | Part Studio row | red, "Qc" icon outlined red, header error icon, Parts (0); tooltip "**QA cube 1 did not regenerate properly: Error regenerating.**" | red; tooltip "Error regenerating." | ✗ ADV3 |
| 8 | Notices panel | same messages/stack; last line "**Result: Regeneration complete**" | "Result: Regeneration failed: Error regenerating." | ✗ **FS5** (minor) |
| 9 | Volume after the part disappears | Onshape has **no bottom-right volume readout**; the Mass properties panel empties within 2 s | our status bar shows "Volume: 15625.000 mm³" and keeps it | ✗ **RD1** (readout doesn't exist in Onshape + goes stale) |

## Re-run on integration 5d28447 (2026-10-21 17:05–17:20, /tmp/qa5/noshape-1659 = /root/target-lead/debug/noshape 16:59; no CLI built from 5d28447 yet, so CLI-only geometry rows stay on the 14:36 CLI result)

## Re-run pass 21 (qa5, 2026-10-23, main 40e23871 app; doc built by /root/qa5-scratch/pass21/mkfs.py: QA cube committed, then `fCuboid` → `fCuboidX` + Ctrl+S in the app)
| Row | Ours now | Status |
|---|---|---|
| FS1 | new Feature Studio: four lines, caret on 4 | 🔧 fixed on agent/qa5 |
| FS2 | ⚠ on line 1, orange wavy underline under the whole call on line 12; tooltip "⚠ Function fCuboidX with 3 argument(s) not found" on the gutter and on the underline | ✅ fixed |
| FS2 (navbar icon) | stays {✓} while only a Part Studio's Monitor shows the runtime {!} error (Feature Studio parse errors turn it red) | ? live (runtime errors) |
| FS4 | toolbar ends with the dashed "Add custom features" button + "Custom features in this workspace" ▾; entry "Qc" initials box + QA cube; tooltip "QA cube" / italic "Feature from current workspace" | ✅ fixed |
| ADV3 | row red; tooltip "QA cube 1 did not regenerate properly: Error regenerating." | ✅ fixed |
| ADV3 (icon) | the "Qc" box stayed grey on a failed feature (live: outlined red) | 🔧 fixed on agent/qa5 (outline in the error red) |
| FS3 / RD1 | Parts (0), no stale volume readout | ✅ fixed |
| FS5 | Notices panel: Feature Studio 1 ⚠ 12:9; Part Studio 1 ⚠ + {!} with the stack 12:91 / 61:17 feature.fs / Part Studio 1 (QA cube 1); "Result: Regeneration complete" | ✅ fixed |
