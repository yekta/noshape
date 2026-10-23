# Workflow: import formats matrix (every sample in /root/onshape-research/live/samples)

qa5, 2026-10-21 pass 4, CLI /tmp/qa5/noshape-cli-1436 `import_files` (defaults), one doc /tmp/qa5/home/Documents/noshape/qa5-impmatrix.noshape,
script /root/qa5-scratch/pass3/impmatrix.py, raw results pass3/impmatrix.log/.json. Live values: behaviours.md G13–G38, G47.
Status: ✅ matches live (≤1e-5 relative), ✗ differs, ? live value unknown.

| Sample (format) | Ours | Live | Status |
|---|---|---|---|
| g36-g24b-v25.0 … v38.1 (27 Parasolid versions) | 7583.000869, 18 faces, all identical | B-rep 7582.912205 | ✗ I1 (+0.089, same for every version) |
| g25-b.sat / .step / .x_t | 7585.978524 / 7582.713024 / 7583.000869 | 7582.912205 | ✗ I1 |
| g47-1part.step (UI export, = g25-b.step) | 7582.713024 | 7582.912205 | ✗ I1 |
| g29-{none,auto,basic,advanced,bsurf}.step | 7582.713024 ×4, bsurf 7582.995371 | 7582.912205 | ✗ I1 |
| g29-tiny-none / auto | 8000.001, 11 faces | 11 faces (boss kept) | ✅ |
| g29-tiny-basic / advanced / bsurf | 8000.000335, 7 faces | 7 faces | ✅ |
| g36-pvzfillet v25 … v38.1, jt-fillet.jt | 7495.362697, 26 faces | 7495.362697409259, 26 | ✅ |
| g37-block.x_t / g37-hole.x_t | 8000 (6) / 7214.601837 (7) | 8000 / 7214.601836602553 | ✅ |
| onshape-cone .sat / .x_t / -v25.x_t | 376.991118, 2 faces (all three) | ? | ? |
| g26-revolved-spline .sat / .x_t | 1726.080718 / 1726.080764 (3 faces) | ? | ? (Δ 4.6e-5 between formats) |
| g35-kern-g24b-mm.stl | mesh, 7581.846183, 1 face | 7581.846183257335 | ✅ |
| g35-…-rest-meters.stl (default mesh unit) | 8e-06 mm³ (read as mm) | live dialog has a Units ▾ (default = document units) | ? (CLI has `mesh_unit`) |
| pvz-block / -assembly / -subasm / -rot-asm / -longname / rest-* | mesh solids 7999.999464, 1 face | 7999.999463558208 | ✅ |
| pvz-fillet / rest-pvz-fillet | 7484.089597 | 7484.089596687392 | ✅ |
| pvz-shapes.pvz | Part 1 1569.1766, Part 2 1770.2392, Part 3 9407.7781 (sum 12747.193949) + "Composite part 1" (Surface, 2 faces) | **parts named Cylinder, Torus, Cone**; sum 12747.193946661087; Composite part 1 (closed, 2 surfaces), no Surfaces section | ✗ **IM1** (part names from the file lost) |
| jt-block.jt / jt-2parts.jt | 8000 / 8000 + 1000 | 8000 / 8000 + 1000 | ✅ |
| g38b-psas_Bell_Crank.x_t | 2050.807089, 27 faces | 2050.83652761612, 27 | ✗ **IM2** (−0.029) |
| g38b-fluxpad_102_STEM.x_t | 255.767857, 167 faces | 255.7688571778665, 167 | ✗ IM2 (−0.001) |
| g38b-ktane_Bracket.x_t | 19377.868698, 12 faces | 19378.847109377377, 12 | ✗ **IM2 (−0.978, worst)** |
| g38b-mcmaster-91274A118.x_t | 254.521984, 25 faces | 254.4037797834923, 25 | ✗ IM2 (+0.118) |
| g38d-mcmaster-curve1 / polycarb-curves | Curve 1 / Curves 1–8 (wires) | 1 / 8 wires | ✅ |
| g39-rule-unique-parts.zip, g47-3parts-unique.zip (zips of 3 STEP parts) | **ERROR "Root assembly file not found. Ensure zip f…"** | ? (requested "qa5 zip import") | ? |
| g47-3parts.step | Part 1–3, 1000 each (listed Part 3, 2, 1) | 3 cubes | ✅ (order ?) |

Findings: **IM1** (kern/fs) PVZ part names; **IM2** (kern) Parasolid imports of the native-CAD samples differ in volume (ktane −0.978 mm³, Bell Crank −0.029, McMaster +0.118, fluxpad −0.001) while face counts match, i.e. a tolerance/evaluation issue in the volume of imported B-splines or fillets; I1 unchanged.

## Update from live's answer (behaviours.md "qa5 zip import", 10-21)

| Row | Live | Ours | Verdict |
|---|---|---|---|
| g47-3parts-unique.zip | upload OK but **translation fails**: toast "g47-3parts-unique.zip uploaded but failed to translate. Open document ×", notification "…was not translated. Translation is not supported for STEP (.stp) files."; a **BLOB tab** "g47-3parts-unique.zip" is still created | error "Root assembly file not found. Ensure zip f…" (CLI), no tab | ✗ **IM3** (message and BLOB tab) |
| onshape-cone.sat | 376.991118430775 | 376.991118 | ✅ |
| g26-revolved-spline.x_t | **1726.09834614350** | 1726.080764 (sat 1726.080718) | ✗ **IM2** (−0.018) |
| pvz-shapes.pvz | Part Studio "**pvz-shapes parts**" + Assembly "**RootAssembly**" + BOM tab; parts **Cylinder 9407.778, Torus 1770.239, Cone 1569.177** (that order) + "Composite parts (1)" | one tab "pvz-shapes", Part 1 1569.18 (Cone), Part 2 1770.24 (Torus), Part 3 9407.78 (Cylinder) + "Composite part 1" (Surface) | ✗ IM1 (names, order, tab name, no RootAssembly/BOM) |
| Import dialog options by type | STEP/x_t: appearances, material density, Y up, composite, join; SAT: no density; PVZ: no density, no join | UI dialog not re-checked this pass | ? |

## Re-run on integration 5d28447 (2026-10-21 17:05–17:20, /tmp/qa5/noshape-1659 = /root/target-lead/debug/noshape 16:59; no CLI built from 5d28447 yet, so CLI-only geometry rows stay on the 14:36 CLI result)

## Re-run pass 7 (2026-10-21 18:56–19:50, app /tmp/qa5/noshape-1851 = target-shared 18:51 (tree d5745c9 + uncommitted work), CLI /tmp/qa5/noshape-cli-1809 = target-engine 18:09)
All 106 imports (impmatrix7.log) are identical to the 14:36 CLI run: I1 (g25-b SAT 7585.978524 / STEP 7582.713024 / X_T 7583.000869), IM1 (PVZ part names, no RootAssembly/BOM), IM2 (ktane 19377.868698 etc.), IM3 (zip: "Root assembly file not found") ✗ still.

## Re-run pass 12 (2026-10-22 ~01:35, CLI /tmp/qa5/noshape-cli-0107 = target-engine 01:07; main still 5bd50b2)
All 106 earlier samples re-imported (/root/qa5-scratch/pass3/impmatrix12.py → impmatrix12.log/json); 104 rows identical to pass 7.
| Row | Ours | Status |
|---|---|---|
| IM3 zips (g39-rule-unique-parts.zip, g47-3parts-unique.zip) | import returns errors ["g39-rule-unique-parts.zip was not translated. Translation is not supported for STEP (.stp) files."] (= live's notification text) and creates a **File tab** named after the zip, no parts | ✅ **fixed** (UI toast "…uploaded but failed to translate. Open document ×" not checked) |
| IM1 PVZ (part names, RootAssembly + BOM tabs) | unchanged | ✗ still |
| IM2 Parasolid volumes (ktane, Bell Crank, McMaster, fluxpad, g26) | unchanged | ✗ still |
| I1 g25-b SAT/STEP/X_T | unchanged | ✗ still |
| New g48-ap203 / ap214 / inch .step (g47 3 cubes) | 3 × 1000, 6 faces each, listed Part 3, 2, 1 | ✅ volumes (order as g47, ?) |
| New g48-types.step (live 22595.3688587902, 15 faces) | 22595.371166, 15 | ✗ IM2-type (+0.0023) |
| New g48-two.step (live Part 1 22595.3688587902 + Part 2 1000) | 22595.371166 + 1000 | ✗ IM2-type (+0.0023) |
| New g48-surface.step | "Surface 1", 1 face | ✅ |
| New g49-chamfer2.x_t (live 44789.85021254086) | 44789.972897, 10 faces | ✗ IM2 (+0.123) |
| New g49-chamfer1.x_t (live 44833.437797237995) | 44833.53007, 10 faces | ✗ IM2 (+0.092) |
| New g50-reorder-shell1.x_t (live 5990.51958739, 20 faces) | 5990.523086, 20 | ✗ IM2-type (+0.0035) |
| New **g50-g46-shell1.x_t** (live 6052.69679568 mm³, 21 faces, area 12258.75 mm², part ~50×30×30 mm) | 21 faces but **volume 3720.632966, area 11691.098, bounding box 0..37350.68 × 0..23719.42 × 0..334.71 mm** (a face/edge is evaluated metres away: the 2 "OTHER" blend patches?) | ✗ **IM4** (kern, severe: corrupt import) |

## Re-run pass 13 (2026-10-22 ~05:20–06:40, main aa75fdf: app /tmp/qa5/noshape-0503 = /root/target-lead/debug/noshape 05:03, CLI /tmp/qa5/noshape-cli-0226 = target-engine 02:26)
impmatrix13 (116 samples) vs impmatrix12: 108 identical.
| Row | Ours | Status |
|---|---|---|
| IM4 g50-g46-shell1.x_t | now refused: "Import failed: … invalid input: parasolid: a curve of the file could not be rebuilt" (File tab only) | ✗ still (safeguard instead of corruption; live imports it, 6052.69679568 mm³) |
| **g38d-polycarb-curves.x_t** (live: Curve 1…Curve 8, 8 circles r 2.25) | **refused by the same safeguard** ("a curve of the file could not be rebuilt"); imported 8 wires on the 01:07 CLI | ✗ **IM5** (kern, regression) |
| Multi-part imports (g47-3parts, g48-ap203/ap214/inch, g48-two, jt-2parts) | names now **renumbered by list position**: g48-two → Part 1 = the 1000 cube, Part 2 = the 22595 G48 body (01:07 CLI and live: Part 1 = G48, Part 2 = cube); jt-2parts Part 1 = 1000, Part 2 = 8000 | ✗ **NAME3** (psb, side effect of the NAME1 fix: imported parts must keep the file's names) |
| IM1, IM2, I1, IM3 | unchanged (IM3 fixed) | as before |

## Re-run pass 15 (2026-10-22, CLI /tmp/qa5/noshape-cli-0521 = target-engine 05:21; main still aa75fdf)
impmatrix14 vs impmatrix13: 115 of 116 identical.
| Row | Ours | Status |
|---|---|---|
| IM5 g38d-polycarb-curves.x_t | Curve 1…Curve 8 (8 wires) again | ✅ **fixed** |
| IM4 g50-g46-shell1.x_t | still refused ("a curve of the file could not be rebuilt") | ✗ still |
| NAME3 multi-part import names | unchanged (positional: g48-two Part 1 = cube) | ✗ still |
| IM1, IM2, I1 | unchanged | ✗ still |

## Re-run pass 18 (2026-10-22 ~10:00–11:00, CLI /tmp/qa5/noshape-cli-0811 = target-engine 08:11; main 3ce6952)
impmatrix15 vs impmatrix14: 109 identical.
| Row | Ours | Status |
|---|---|---|
| NAME3 multi-part import names | file names kept again: g48-two Part 1 = 22595.371166 (G48), Part 2 = 1000 cube; g47-3parts / g48-ap203/ap214/inch listed Part 3, 2, 1; jt-2parts Part 1 = 8000, Part 2 = 1000 | ✅ **fixed** |
| IM4 g50-g46-shell1.x_t | imports again (no refusal) but still **corrupt**: 21 faces, volume 4550.392098, area 11830.50, bbox 0..38955.54 × 0..16845.92 × 0..320.42 mm (live 6052.69679568, a ~50×30×30 part) | ✗ still |
| IM5 g38d curves | 8 wires | ✅ (stays fixed) |
| IM1, IM2, I1 | unchanged | ✗ still |

## Re-run pass 21 (2026-10-23 ~17:30–18:30, app = /root/target-lead/debug/noshape 10-23 17:10 (main 88fbf8f), CLI = target-lead noshape-cli 10-23 13:45)
impmatrix16 (117 samples incl. the new psb-L11-boss-fillet.step: Part 1 8179.76547, 10 faces) vs impmatrix15: 115 identical.
| Row | Ours | Status |
|---|---|---|
| IM4 g50-g46-shell1.x_t | 21 faces, bbox 0..50 × 0..30 × 0..30, area 12258.695 (live 12258.75), volume **6053.320139** (live 6052.69679568, +0.62) | ✅ geometry **fixed**; volume offset joins IM2 |
| IM1 PVZ names / tabs | unchanged | ✗ still → routed kern/fs |
| IM2 Parasolid volumes (ktane, Bell Crank, McMaster, fluxpad, g26, g48, g49, g50) | unchanged | ✗ still → routed kern |
| I1 g25-b SAT/STEP/X_T | unchanged | ✗ still → routed kern |
