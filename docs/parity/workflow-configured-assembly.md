# Workflow: configured part in an assembly (Change configuration, configured insert, BOM, Variable Studio)

qa5, 2026-10-21 pass 7, app /tmp/qa5/noshape-1851 (target-shared 18:51), doc qa5-cfgasm.noshape (copy of "qa5 config":
Part Studio 1 with configuration inputs d (10 mm) and List 2 (Default/Large), Variable Studio 1 (h), Assembly 1 with Part 1 <1>).
Live: behaviours.md "Drag cues … configured insert" item 6 (assembly/592–595), "Configured Part Studio instance in an assembly"
(cf-050–052), "Insert placement in an empty assembly" (ins-899…925), assembly/566. Status: ✅ matches live, ✗ differs, ? live pending.

| # | Step | Live | Ours | Status |
|---|---|---|---|---|
| 1 | Instance row of a configured Part Studio's part | icon `svg-icon-part-configured` (part icon with a blue grid), no chevron | plain part icon | ✗ **CA1** (skb / ui2) |
| 2 | Instance right-click | … Switch to qa5 cfg, **Change configuration…**, **Select instances with same configuration**, **Select instances with same part and same configuration**, Copy …, Move to new subassembly, Change to version…, Export…, **Release…**, (Add comment), Zoom to selection, Create Drawing of Part 1…, Add selection to folder…, Create empty subassembly, Delete | same order, but **no "Select instances with same configuration", no "Select instances with same part and same configuration", no "Release…"** | ✗ **CA2** (skb) |
| 3 | Select the instance (list row) | amber (242,182,72) top / (192,144,57) side, **amber edges**, white triad; only the instance row highlighted | (229,163,54) / (195,135,1), **black edges**, white triad; the **root "Assembly 1" row is highlighted too** | ✗ A15 (skb / ui2) |
| 4 | Change configuration… dialog | "Change configuration" ✓ ✕; document + branch; studio row with thumbnail + [▦ ▾]; one row per input; greyed [Generate]; part list | same rows (d 10 mm, List 2 ▾, Generate greyed until a change, Part 1, Sketch 1) ✅; **the lower half of the dialog body is translucent**: the model, the origin triad and the feature-list toggle show through (the Insert dialog has the same) | ✅ CA3 closed: live is as translucent (see "CA3 closed" below) |
| 5 | Type 25 into d, Tab | (variable box: "25" + Enter → "25 mm") ; Tab to the next field | field shows **"25"** (no unit); **Tab moves focus out of the dialog to the tab bar's "+" button**, after which shortcuts (f) do nothing until the graphics are clicked | ✗ **CA4** (ui) |
| 6 | ✓ | instance regenerates in the new configuration | instance taller (d = 25) | ✅ |
| 7 | Insert dialog, configured studio row | expanded row shows the inputs ("Size ▾ Default"); button "Set to last viewed configuration" with ▾ Set to last viewed / Reset to default; toast "Could not find last viewed configuration." | same (d 10 mm, List 2 ▾ Default, tooltip, menu, toast) | ✅ |
| 8 | Insert Part 1 again (default configuration), ✓ without entering the graphics | repeat instance of the same part at **+⅛ of its bbox** (5, 2.5, 1.25 for a 40×20×10 block); a different part at (0,0,0) | Part 1 <2> at **(−2.119, −17.446, 0.0001)** (CLI assembly_list) | ✗ **A18** (skb) |
| 9 | BOM | Item / Quantity / Part number / Description; one row per part+configuration | 2 rows, Part 1 each | ✅ (? configuration shown nowhere, as live's default template) |
| 10 | Variable Studio: h 15 → "20" + Enter | "20 mm" | "20 mm" | ✅ |
| – | Translucent white box behind the bottom-right Measure / Analysis / Mass icons when they sit over the model | ? | visible (p7-ca14) | ? |

## Re-run pass 8 on main 5bd50b2 (2026-10-21 23:13–00:10, app /tmp/qa5/noshape-2256 = /root/target-lead/debug/noshape 22:56; no newer CLI than target-engine 18:09, so CLI-only rows stay on pass 7; geometry rows re-read in the app where possible)
Live answered "qa5 pass 7" (5): BOM 1 row Qty 2 → 2 rows after d = 25 → merged again after 10 mm; Reset to default resets d.
| Row | Ours | Status |
|---|---|---|
| CA1 configured icon | part icon with blue grid | ✅ **fixed** |
| CA2 instance menu | Select instances with same configuration, … same part and same configuration, Release… present | ✅ **fixed** |
| CA3 translucent dialog | Change configuration body still shows the model and origin triad through it | ✗ still |
| CA4 unit / Tab | "25" + Tab → "25 mm" ✅; Tab still leaves the dialog (focus to the noshape logo) | ✗ still (partly) |
| CA5 Reset to default | d back to 10 mm | ✅ **fixed** |
| CA6 BOM merge | 1 row Qty 2 (later Qty 3 with a third instance) | ✅ **fixed** |

**CA3 status (lead, 10-21 after pass 8):** ui found live's dialog background is rgba(255,255,255,.95) in Onshape's CSS, and live cf-052 shows the model through it too. CA3 is **? pending ui's side-by-side crop**, not ✗.

### CA3 closed (ui, 21 Oct ~22:50): the dialog is as translucent as live's
Live's dialogs are `rgba(255,255,255,.95)` (woolsthorpe.css `.ns-dialog` / `.ns-dialog-frame` background =
`--os-feature-dialog-background` = `--os-background-transparent-primary` = rgba95; no hover change, no backdrop filter).
Side by side, Change configuration at the same spot (/root/ui-scratch/shots/ca3-side.png: live cf-052 | ours):

| What is behind the dialog body | live (cf-052) | ours | CSS .95 white predicts |
|---|---|---|---|
| the feature-list toggle icon (#333), at (250–280, 540–570) | 245 | 244 | 245 |
| the dark side face of the part (76,102,118), at (400,560) | (no part there in cf-052) | (245,246,247) | (246,247,248) |

The pixels match to within 1 level. In live the model shows through just as faintly whenever a dialog covers it; cf-052 only
happens to have empty space behind the dialog. ✅ **CA3 closed** (no change).

## Re-run pass 13 (2026-10-22 ~05:20–06:40, main aa75fdf: app /tmp/qa5/noshape-0503 = /root/target-lead/debug/noshape 05:03, CLI /tmp/qa5/noshape-cli-0226 = target-engine 02:26)
| Row | Ours | Status |
|---|---|---|
| CA4 "30" + Tab | "30 mm", focus moves to List 2 | ✅ **fixed** (c24); a value saved by the old binary still shows "25" without unit (legacy data) |

## Re-run pass 21 (qa5, 2026-10-23, main 40e23871 app /root/target-lead/debug/noshape)
| A15 | Part 3 <1> picked from the list: amber faces (241,178,49) top / (207,153,42) side, **amber edges**, white triad; only the instance row highlighted | ✅ fixed (shade within lighting) |
