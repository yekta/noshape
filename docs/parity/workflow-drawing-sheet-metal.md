# Workflow: drawing of a sheet-metal part with a flat view

qa5, 2026-10-21 pass 4, /tmp/qa5/noshape-1448, "qa5 sheet metal" (Sheet metal model, Flange, Hem, Bend relief, Corner, Finish).
Live: behaviours.md "Flat pattern view" (asb, ~line 1365). Status: ✅ matches live, ✗ differs, ? live pending.

| # | Step | Live | Ours | Status | Verified |
|---|---|---|---|---|---|
| 1 | Part 1 right-click | Rename, Properties…, Assign material…, Edit appearance… \| Copy here…, Copy Part 1 \| ✅ (no flat-pattern export items; = live) | 77870dd4 10-28 | Add comment \| Zoom to selection \| Delete… | same + **"Export flat pattern (DXF)…"** (the SVG item is gone) | ✗ SM13 (partly fixed) |  |
| 2 | Create Drawing of Part 1… › ANSI_A_MM, No views | "Part 1 Drawing 1" | same, inserted after Part Studio 1 | ✅ | 77870dd4 10-28 |
| 3 | Insert › picker filter "Flat patterns" | rows "Flat pattern of Part 1" with a **flat thumbnail** | "Flat pattern of Part 1", but the thumbnail shows the **folded** part | ✅ (Flat patterns toggle: "Flat pattern of Part 1" with a flat thumbnail) | 77870dd4 10-28 |
| 4 | Place the flat view (1:1, Top) | outline; each bend = two solid bend-region lines + dashed centre line; no view label | same style; no label | ✅ | 77870dd4 10-28 |
| 5 | Bend notes | vertical along each bend, "**UP 90.0° R.091**" (one decimal on the angle, document-unit radius) | "**DOWN 90° R2**" ×2, "DOWN 180° R0.01" (hem); horizontal-bend notes read horizontally above the bend | ✅ ("UP 90.0° R2" along each bend: SM5 / DSM2 fixed) | 77870dd4 10-28 |
| 6 | View right-click › Show/hide ▸ | Show hidden lines · Hide bend lines · Hide bend notes · Show shaded view · Hide threads · **Show faulty parts** · Show/hide sketches… · Show/hide edges… · Hide auto centermarks | no "Show faulty parts"; extra "Show center of mass" | ✅ (item for item = live, Show faulty parts, Show shaded view) | 77870dd4 10-28 |
| 7 | Hide bend notes | notes hidden | notes hidden; Ctrl+Z restores | ✅ | 77870dd4 10-28 |
| 8 | View right-click (whole menu) | (assembly/part view menus in behaviours.md drawing section) | Show/hide ▸, View orientation ▸, Tangent edges ▸, Adjust linestyle…, Insert custom table of Part Studio 1…, View properties…, Order ▸, Align views ▸, Rotate view ▸, Switch to Part Studio 1, Move to sheet…, Copy, Paste as table, Select for replication [beta] ▸, Replicate annotations [beta], Clear selection, Zoom to fit, Delete | ? |  |

## Re-run on integration 5d28447 (2026-10-21 17:05–17:20, /tmp/qa5/noshape-1659 = /root/target-lead/debug/noshape 16:59; no CLI built from 5d28447 yet, so CLI-only geometry rows stay on the 14:36 CLI result)

## Re-run pass 7 (2026-10-21 18:56–19:50, app /tmp/qa5/noshape-1851 = target-shared 18:51 (tree d5745c9 + uncommitted work), CLI /tmp/qa5/noshape-cli-1809 = target-engine 18:09)
| Row | Ours | Status | Verified |
|---|---|---|---|
| DSM1 picker thumbnail | flat thumbnail ✅; with the Flat patterns filter on, the folded "Part 1" is listed too (live lists the flat pattern rows) | ✅ **fixed** / ? minor | 77870dd4 10-28 |
| DSM2 bend notes | "DOWN 90° R2", "DOWN 180° R0.01" | ✗ still (live "UP 90.0° …") |  |
| DSM3 Show/hide ▸ | Show faulty parts ✅, no Show center of mass ✅; but **"Hide shaded view"** on an unshaded flat view (live "Show shaded view") | ✗ still (mostly fixed) |  |

## Re-run pass 8 on main 5bd50b2 (2026-10-21 23:13–00:10, app /tmp/qa5/noshape-2256 = /root/target-lead/debug/noshape 22:56; no newer CLI than target-engine 18:09, so CLI-only rows stay on pass 7; geometry rows re-read in the app where possible)
Live answered "qa5 pass 7" (4) + asb (d) "shaded drawing views": picker filters are 7 independent toggles (Part Studios, Parts (on), Composite parts, Sketches, Surfaces, Curves, Flat patterns); flat view default "Show shaded view"; a shaded flat view fills the flat-pattern faces with the appearance colour, drops the black edges, keeps bend lines/notes on top.
| Row | Ours | Status | Verified |
|---|---|---|---|
| DSM2 bend notes | "DOWN 90.0° R2" (one decimal ✅) | ✗ still (DOWN vs live UP = SM5) |  |
| DSM3 Show/hide label | "Show shaded view" when unshaded ✅ — but the existing flat view **opened shaded** | ✗ (default state) |  |
| DSM5 (new) shaded flat view | fills the **folded part's faces, offset** from the flat outline; **bend notes disappear** (one stray "D" left); black edges stay | ✗ **DSM5** (asb) |  |
| DSM4 picker filters | 4 icons (Part Studios, Parts, Sketches, Flat patterns) vs live's 7 independent toggles | ✗ **DSM4** (asb) |  |

## Re-run pass 13 (2026-10-22 ~05:20–06:40, main aa75fdf: app /tmp/qa5/noshape-0503 = /root/target-lead/debug/noshape 05:03, CLI /tmp/qa5/noshape-cli-0226 = target-engine 02:26)
| Row | Ours | Status | Verified |
|---|---|---|---|
| DSM5 shaded flat view | fill matches the flat outline, bend lines/notes kept | ✅ **fixed** (s02) | 77870dd4 10-28 |
| DSM3 label | "Hide shaded view" on the shaded view (consistent); the saved flat view is shaded from the old binary, so the default of a new flat view is not re-checked | ✅ label | 77870dd4 10-28 |
| DSM2 bend notes | "DOWN 90.0° R2", "DOWN 180.0° R0.01" | ✗ still (DOWN vs live UP = SM5) |  |
| DSM4 picker filters | 4 icons | ✗ still (live 7) |  |

## Re-run pass 19 (2026-10-22 ~11:00–11:40, app /tmp/qa5/noshape-1013 = /root/target-lead/debug/noshape 10:13: the queue's 09:58 batch build = main 3ce6952 + the ready branches of that batch; CLI 08:11)
| DSM2 bend notes | "DOWN 90.0° R2" (z12) | ✗ still |
| DSM4 picker filters | 4 icons (z13) | ✗ still |

## DSM4 re-check (10-22, app /tmp/qa5/noshape-1013; doc /tmp/qa5/d29/kinds.noshape from /root/qa5-scratch/pass20/mkkinds.py: parts, a composite part, a surface, a curve, sketches, a sheet-metal studio)
- Live (behaviours ~6840): filter icons Part Studios, Parts, Composite parts, Sketches, Surfaces, Curves, Flat patterns; **a document that lacks some kinds shows only the relevant ones**.
- Ours with all kinds present: **7 icons in that order** (z31). The sheet-metal doc has no composite parts, surfaces or curves, hence 4. → **DSM4 withdrawn** (asb's report was right).

## Re-check (qa4, 2026-10-23 20:40–21:10, main a0e360a3; code and the latest qa5 passes)
- DSM1, DSM5 fixed; DSM4 withdrawn; DSM3 label ✅ (new flat view's default shading not re-checked).
- **DSM2 / SM5** still open: bend notes "DOWN 90.0° R2" where live reads "UP 90.0° …". The notes print `FlatBendLine.up`
  (sheetmetal/flat.rs:300); fs's test `sheetmetal_frames.rs:271` still asserts a flange is *not* up. asb: lands with fs's
  std-route switch → qa5-routing DSM2 (**fs**).

## Re-check (skasm, 2026-10-30 evening, main 94056b26)
- DSM2 / SM5: fixed (step 5 above: "UP 90.0° R2"; the std route's bend directions, T2s).
- DSM3: a new view, flat ones included, starts Hidden lines removed (drawing create.rs: `RenderStyle::HiddenLinesRemoved`),
  i.e. unshaded, so its Show/hide offers "Show shaded view" as live. ✅
