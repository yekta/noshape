# Workflow: import formats matrix (every sample in /root/onshape-research/live/samples)

qa5, 2026-10-21 pass 4, CLI /tmp/qa5/noshape-cli-1436 `import_files` (defaults), one doc /tmp/qa5/home/Documents/noshape/qa5-impmatrix.noshape,
script /root/qa5-scratch/pass3/impmatrix.py, raw results pass3/impmatrix.log/.json. Live values: behaviours.md G13–G38, G47.
Status: ✅ matches live (≤1e-5 relative), ✗ differs, ? live value unknown.

**Grading rule for volumes and areas (lead, 2026-10-24):** Onshape's mass properties are estimates with a band
(REST massproperties `[value, lo, hi]`; e.g. "kern G24 b" volume 7582.912205 in 7576.2420 – 7589.5825, and its own
STEP / SAT re-imports give 7582.848 / 7582.872; behaviours.md "api (10-23) import parity"). An import **matches**
(✅) when our value lies inside live's band **and** the faces whose exact areas can be computed (planes, cylinders,
cones, spheres, tori) have those areas; digit-for-digit equality with live's estimate is not required. Rows without a
known band keep the ≤1e-5 rule until live supplies one.

| Sample (format) | Ours | Live | Status | Verified |
|---|---|---|---|---|
| g36-g24b-v25.0 … v38.1 (27 Parasolid versions) | 7583.000869, 18 faces, all identical | B-rep 7582.912205 | ✗ I1 (+0.089, same for every version) |  |
| g25-b.sat / .step / .x_t | 7585.978524 / 7582.713024 / 7583.000869 | 7582.912205 | ✗ I1 |  |
| g47-1part.step (UI export, = g25-b.step) | 7582.713024 | 7582.912205 | ✗ I1 |  |
| g29-{none,auto,basic,advanced,bsurf}.step | 7582.713024 ×4, bsurf 7582.995371 | 7582.912205 | ✗ I1 |  |
| g29-tiny-none / auto | 8000.001, 11 faces | 11 faces (boss kept) | ✅ | v0.4.12 CLI 10-28 (= 1b81b7ca code): 8000.001 / 11 |
| g29-tiny-basic / advanced / bsurf | 8000.000335, 7 faces | 7 faces | ✅ | v0.4.12 CLI 10-28 (= 1b81b7ca code): 8000.000335 / 7 (bsurf 8000.000336) |
| g36-pvzfillet v25 … v38.1, jt-fillet.jt | 7495.362697, 26 faces | 7495.362697409259, 26 | ✅ | v0.4.12 CLI 10-28 (= 1b81b7ca code): 7495.362697 / 26 on all 27 versions and the JT |
| g37-block.x_t / g37-hole.x_t | 8000 (6) / 7214.601837 (7) | 8000 / 7214.601836602553 | ✅ | v0.4.12 CLI 10-28 (= 1b81b7ca code): 8000 (6) / 7214.601837 (7) |
| onshape-cone .sat / .x_t / -v25.x_t | 376.991118, 2 faces (all three) | ? | ? |  |
| g26-revolved-spline .sat / .x_t | 1726.080718 / 1726.080764 (3 faces) | ? | ? (Δ 4.6e-5 between formats) |  |
| g35-kern-g24b-mm.stl | mesh, 7581.846183, 1 face | 7581.846183257335 | ✅ | v0.4.12 CLI 10-28 (= 1b81b7ca code): 7581.846183 / 1 |
| g35-…-rest-meters.stl (default mesh unit) | 8e-06 mm³ (read as mm) | live dialog has a Units ▾ (default = document units) | ? (CLI has `mesh_unit`) |  |
| pvz-block / -assembly / -subasm / -rot-asm / -longname / rest-* | mesh solids 7999.999464, 1 face | 7999.999463558208 | ✅ | v0.4.12 CLI 10-28 (= 1b81b7ca code): 7999.999464 / 1 each |
| pvz-fillet / rest-pvz-fillet | 7484.089597 | 7484.089596687392 | ✅ | v0.4.12 CLI 10-28 (= 1b81b7ca code): 7484.089597 |
| pvz-shapes.pvz | Part 1 1569.1766, Part 2 1770.2392, Part 3 9407.7781 (sum 12747.193949) + "Composite part 1" (Surface, 2 faces) | **parts named Cylinder, Torus, Cone**; sum 12747.193946661087; Composite part 1 (closed, 2 surfaces), no Surfaces section | ✗ **IM1** (part names from the file lost) |  |
| jt-block.jt / jt-2parts.jt | 8000 / 8000 + 1000 | 8000 / 8000 + 1000 | ✅ | v0.4.12 CLI 10-28 (= 1b81b7ca code): 8000 / 8000 + 1000 |
| g38b-psas_Bell_Crank.x_t | 2050.807089, 27 faces | 2050.83652761612, 27 | ✗ **IM2** (−0.029) |  |
| g38b-fluxpad_102_STEM.x_t | 255.767857, 167 faces | 255.7688571778665, 167 | ✗ IM2 (−0.001) |  |
| g38b-ktane_Bracket.x_t | 19377.868698, 12 faces | 19378.847109377377, 12 | ✗ **IM2 (−0.978, worst)** |  |
| g38b-mcmaster-91274A118.x_t | 254.521984, 25 faces | 254.4037797834923, 25 | ✗ IM2 (+0.118) |  |
| g38d-mcmaster-curve1 / polycarb-curves | Curve 1 / Curves 1–8 (wires) | 1 / 8 wires | ✅ | v0.4.12 CLI 10-28 (= 1b81b7ca code): 1 / 8 wires (no volume) |
| g39-rule-unique-parts.zip, g47-3parts-unique.zip (zips of 3 STEP parts) | **ERROR "Root assembly file not found. Ensure zip f…"** | ? (requested "qa5 zip import") | ? |  |
| g47-3parts.step | Part 1–3, 1000 each (listed Part 3, 2, 1) | 3 cubes | ✅ (order ?) | v0.4.12 CLI 10-28 (qa, /tmp/qa-imp/impmatrix.json: Part 3, Part 2, Part 1, 1000.0 / 6 faces each) |

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
| Row | Ours | Status | Verified |
|---|---|---|---|
| IM3 zips (g39-rule-unique-parts.zip, g47-3parts-unique.zip) | import returns errors ["g39-rule-unique-parts.zip was not translated. Translation is not supported for STEP (.stp) files."] (= live's notification text) and creates a **File tab** named after the zip, no parts | ✅ **fixed** (UI toast "…uploaded but failed to translate. Open document ×" not checked) | v0.4.12 CLI (qa 10:24: import_files of both zips → File tab named after the zip, no parts, errors ["<zip> was not translated. Translation is not supported for STEP (.stp) files."]) |
| IM1 PVZ (part names, RootAssembly + BOM tabs) | unchanged | ✗ still |  |
| IM2 Parasolid volumes (ktane, Bell Crank, McMaster, fluxpad, g26) | unchanged | ✗ still |  |
| I1 g25-b SAT/STEP/X_T | unchanged | ✗ still |  |
| New g48-ap203 / ap214 / inch .step (g47 3 cubes) | 3 × 1000, 6 faces each, listed Part 3, 2, 1 | ✅ volumes (order as g47, ?) | v0.4.12 CLI 10-28 (qa, /tmp/qa-imp/impmatrix.json: ap203 / ap214 / inch each Part 3, 2, 1 at 1000.0, 6 faces) |
| New g48-types.step (live 22595.3688587902, 15 faces) | 22595.371166, 15 | ✗ IM2-type (+0.0023) |  |
| New g48-two.step (live Part 1 22595.3688587902 + Part 2 1000) | 22595.371166 + 1000 | ✗ IM2-type (+0.0023) |  |
| New g48-surface.step | "Surface 1", 1 face | ✅ | v0.4.12 CLI 10-28 (= 1b81b7ca code): 1 face, no volume |
| New g49-chamfer2.x_t (live 44789.85021254086) | 44789.972897, 10 faces | ✗ IM2 (+0.123) |  |
| New g49-chamfer1.x_t (live 44833.437797237995) | 44833.53007, 10 faces | ✗ IM2 (+0.092) |  |
| New g50-reorder-shell1.x_t (live 5990.51958739, 20 faces) | 5990.523086, 20 | ✗ IM2-type (+0.0035) |  |
| New **g50-g46-shell1.x_t** (live 6052.69679568 mm³, 21 faces, area 12258.75 mm², part ~50×30×30 mm) | 21 faces but **volume 3720.632966, area 11691.098, bounding box 0..37350.68 × 0..23719.42 × 0..334.71 mm** (a face/edge is evaluated metres away: the 2 "OTHER" blend patches?) | ✗ **IM4** (kern, severe: corrupt import) |  |

## Re-run pass 13 (2026-10-22 ~05:20–06:40, main aa75fdf: app /tmp/qa5/noshape-0503 = /root/target-lead/debug/noshape 05:03, CLI /tmp/qa5/noshape-cli-0226 = target-engine 02:26)
impmatrix13 (116 samples) vs impmatrix12: 108 identical.
| Row | Ours | Status | Verified |
|---|---|---|---|
| IM4 g50-g46-shell1.x_t | now refused: "Import failed: … invalid input: parasolid: a curve of the file could not be rebuilt" (File tab only) | ✗ still (safeguard instead of corruption; live imports it, 6052.69679568 mm³) |  |
| **g38d-polycarb-curves.x_t** (live: Curve 1…Curve 8, 8 circles r 2.25) | **refused by the same safeguard** ("a curve of the file could not be rebuilt"); imported 8 wires on the 01:07 CLI | ✗ **IM5** (kern, regression) |  |
| Multi-part imports (g47-3parts, g48-ap203/ap214/inch, g48-two, jt-2parts) | names now **renumbered by list position**: g48-two → Part 1 = the 1000 cube, Part 2 = the 22595 G48 body (01:07 CLI and live: Part 1 = G48, Part 2 = cube); jt-2parts Part 1 = 1000, Part 2 = 8000 | ✗ **NAME3** (psb, side effect of the NAME1 fix: imported parts must keep the file's names) |  |
| IM1, IM2, I1, IM3 | unchanged (IM3 fixed) | as before |  |

## Re-run pass 15 (2026-10-22, CLI /tmp/qa5/noshape-cli-0521 = target-engine 05:21; main still aa75fdf)
impmatrix14 vs impmatrix13: 115 of 116 identical.
| Row | Ours | Status | Verified |
|---|---|---|---|
| IM5 g38d-polycarb-curves.x_t | Curve 1…Curve 8 (8 wires) again | ✅ **fixed** | v0.4.12 CLI 10-28 (qa, /tmp/qa-imp/impmatrix.json: Curve 1 … Curve 8, Wire) |
| IM4 g50-g46-shell1.x_t | still refused ("a curve of the file could not be rebuilt") | ✗ still |  |
| NAME3 multi-part import names | unchanged (positional: g48-two Part 1 = cube) | ✗ still |  |
| IM1, IM2, I1 | unchanged | ✗ still |  |

## Re-run pass 18 (2026-10-22 ~10:00–11:00, CLI /tmp/qa5/noshape-cli-0811 = target-engine 08:11; main 3ce6952)
impmatrix15 vs impmatrix14: 109 identical.
| Row | Ours | Status | Verified |
|---|---|---|---|
| NAME3 multi-part import names | file names kept again: g48-two Part 1 = 22595.371166 (G48), Part 2 = 1000 cube; g47-3parts / g48-ap203/ap214/inch listed Part 3, 2, 1; jt-2parts Part 1 = 8000, Part 2 = 1000 | ✅ **fixed** | v0.4.12 CLI 10-28 (qa, /tmp/qa-imp/impmatrix.json: g48-two Part 1 22595.37117 / 15 faces, Part 2 1000 cube) |
| IM4 g50-g46-shell1.x_t | imports again (no refusal) but still **corrupt**: 21 faces, volume 4550.392098, area 11830.50, bbox 0..38955.54 × 0..16845.92 × 0..320.42 mm (live 6052.69679568, a ~50×30×30 part) | ✗ still |  |
| IM5 g38d curves | 8 wires | ✅ (stays fixed) | v0.4.12 CLI 10-28 (qa, /tmp/qa-imp/impmatrix.json: 8 Wire bodies) |
| IM1, IM2, I1 | unchanged | ✗ still |  |

## Re-run pass 21 (2026-10-23 ~17:30–18:30, app = /root/target-lead/debug/noshape 10-23 17:10 (main 88fbf8f), CLI = target-lead noshape-cli 10-23 13:45)
impmatrix16 (117 samples incl. the new psb-L11-boss-fillet.step: Part 1 8179.76547, 10 faces) vs impmatrix15: 115 identical.
| Row | Ours | Status | Verified |
|---|---|---|---|
| IM4 g50-g46-shell1.x_t | 21 faces, bbox 0..50 × 0..30 × 0..30, area 12258.695 (live 12258.75), volume **6053.320139** (live 6052.69679568, +0.62) | ✅ geometry **fixed**; volume offset joins IM2 | v0.4.12 CLI 10-28 (qa, /tmp/qa-imp/impmatrix.json: 21 faces, volume now 6052.678152 (row 162)) |
| IM1 PVZ names / tabs | unchanged | ✗ still → routed kern/fs |  |
| IM2 Parasolid volumes (ktane, Bell Crank, McMaster, fluxpad, g26, g48, g49, g50) | unchanged | ✗ still → routed kern |  |
| I1 g25-b SAT/STEP/X_T | unchanged | ✗ still → routed kern |  |

## Re-run pass 22 (qa5, 2026-10-24, CLI built from main c59c5af9 after the api import/export batch; /root/qa5-scratch/pass21/impmatrix21.*)
| Row | Ours now | Status | Verified |
|---|---|---|---|
| IM1 PVZ | pvz-shapes.pvz → File tab + Assembly "RootAssembly" + Part Studio "pvz-shapes parts" with **Cylinder 9407.778113, Torus 1770.239209, Cone 1569.176627** + "Composite part 1" (live names and volumes); pvz-colors keeps "Face colored / Red / Transparent", pvz-longname its long name; multi-instance PVZ files give one part + the assembly | ✅ mostly fixed; live also makes a **"BOM : RootAssembly"** tab (behaviours ~6720), ours none → ✗ IM1b (api) | v0.4.12 CLI 10-28 (qa, /tmp/qa-imp/impmatrix.json: Part Studio "pvz-shapes parts": Cylinder 9407.778113, Torus 1770.239209, Cone 1569.176627, Composite part 1; IM1b BOM tab not re-checked) |
| Re-import names | second import of g25-b → "g25-b (1)", "g25-b (2)" | ✅ (live "(1)") | 77870dd4 10-28 (UI: g25-b, g25-b (1), g25-b (2)) |
| IM2 B-spline volumes | unchanged (ktane, McMaster, g26, g48, g49, g50 same digits as pass 16) | ✗ kern |  |
| I1 g25-b SAT / STEP / X_T | 7585.978524 / 7582.713024 / 7583.000869, unchanged | ✗ kern |  |
| No import errors | 0 errors over every sample (incl. qa4-s5-*) | ✅ | v0.4.12 CLI 10-28 (bin/impmatrix_qa.py over all 143 samples: 0 errors; out qa-scratch/impmatrix-v0412-1508.out) |

## IM2 graded under the band rule (api, 2026-10-24)
Live: behaviours.md "api (10-24) IM2 bands" (REST massproperties `[value, lo, hi]`; the four g38b rows are from live's
native Part Studios the .x_t files were exported from, not re-uploaded). Ours: CLI from agent/api 1958242d,
`import_files` with defaults, `query_mass_properties` + `query_entities` (/root/api-scratch/im2).
| Sample | Ours volume / area / faces | Live band volume / area / faces | Status | Verified |
|---|---|---|---|---|
| g26-revolved-spline.x_t | 1726.080764 / 764.757501 / 3 | 1723.48714–1728.70956 / 763.82643–765.64946 / 3 | ✅ | v0.4.12 CLI 10-28 (= 1b81b7ca code): 1726.080764 / 3 (volume, faces) |
| g48-types.step | 22595.371166 / 5859.027270 / 15 | 22593.47604–22597.26157 / 5858.64505–5859.41140 / 15 | ✅ | v0.4.12 CLI 10-28 (= 1b81b7ca code): 22595.371170 / 15 |
| g49-chamfer1.x_t | 44833.530070 / 7674.032869 / 10 | 44830.51265–44836.20455 / 7673.93667–7674.10522 / 10 | ✅ | v0.4.12 CLI 10-28 (= 1b81b7ca code): 44833.514326 / 10, in band (was 44833.530070) |
| g49-chamfer2.x_t | 44789.972897 / 7654.531075 / 10 | 44786.54841–44793.28002 / 7654.41848–7654.65347 / 10 | ✅ | v0.4.12 CLI 10-28 (= 1b81b7ca code): 44789.962583 / 10, in band |
| g50-reorder-shell1.x_t | 5990.523086 / 12131.769122 / 20 | 5986.86289–5994.17629 / 12131.64289–12131.90190 / 20 | ✅ | v0.4.12 CLI 10-28 (= 1b81b7ca code): 5990.523086 / 20 |
| g50-g46-shell1.x_t (IM4) | 6053.320139 / 12258.695405 / 21 | 6048.91104–6056.61040 / 12258.60418–12258.89664 / 21 | ✅ | v0.4.12 CLI 10-28 (= 1b81b7ca code): 6052.678152 / 21, in band (was 6053.320139) |
| g38b-ktane_Bracket.x_t | 19377.868698 / 9223.353906 / 12 | 19370.14999–19387.54423 / 9222.82507–9224.00498 / 12 | ✅ | v0.4.12 CLI 10-28 (= 1b81b7ca code): 19377.868698 / 12 |
| g38b-psas_Bell_Crank.x_t | 2050.807089 / 1921.487523 / 27 | 2050.43259–2051.24046 / 1921.37196–1921.61854 / 27 | ✅ | v0.4.12 CLI 10-28 (= 1b81b7ca code): 2050.807126 / 27, in band |
| g38b-mcmaster-91274A118.x_t | 254.521984 / 407.613254 / 25 | 248.20340–260.60416 / 397.20399–417.95194 / 25 | ✅ | v0.4.12 CLI 10-28 (= 1b81b7ca code): 254.419624 / 25, in band |
| g38b-fluxpad_102_STEM.x_t | 255.767857 / 636.018340 / 167 | 255.68612–255.85159 / 635.94739–636.09426 / 167 | ✅ | v0.4.12 CLI 10-28 (= 1b81b7ca code): 255.768144 / 167, in band |

IM2 is closed on volumes and areas: every value lies inside live's band, with the same face counts.

**IM2b, per-face check (live: behaviours.md "api (10-24) IMP2 parts order + IM2 per-face data", the Geom dump's evArea):**
| Sample | Faces equal to live (≤1e-6 rel.) | Faces that differ (ours − live, mm²) | Status | Verified |
|---|---|---|---|---|
| g50-g46-shell1.x_t | 17 of 21: every plane but F11, cylinders F6, F7, F17 | **F18 cylinder r2 −0.0617** (82.786126 vs 82.8478429); F8 −0.0093 (ours B-spline, live EXTRUDED); F19 OTHER +0.0081; F11 plane −0.0004 (45.999575 vs 46) | ✗ **IM2b** |  |
| g38b-mcmaster-91274A118.x_t (live: native import) | 22 of 25: every plane and cone, F4, F6 | **F0 cylinder r1.545 +0.0341** (13.946297 vs 13.9122437); thread flanks F1 −0.0008, F2 −0.0016 | ✗ **IM2b** |  |

The differing faces all meet the blend pieces (g50) or the thread flanks (McMaster): their shared edges are bounded
differently from live's. On g50, F18 + F8 + F19 + F11 add up to the whole −0.063 gap between our Σ 12258.695 and live's
Σ 12258.759. Live's EXTRUDED F8 is a B-spline in ours. The other samples have no live per-face data.

**IMP2 (parts order after a two-part STEP import): ✅.** Live lists "Part 2, Part 1" for g48-two.step, which is the
file's product/body order, the same as ours.

**IM2b progress (api, 2026-10-24 later):** the differences were mostly OCCT's area and volume integration across
**gaps** in a face's boundary. At a tolerant vertex one edge's pcurve ends away from where the next one starts (g50's F18:
0.02 mm in v). Parasolid closes such gaps with straight (u, v) segments; OCCT skips them, so its result depends on where
u is measured from. Our moments integration now closes them (kernel common.cpp `uv_gaps`) and centres the divergence
field on the shape's box, so the volume with gaps doesn't depend on where the part sits.
| Sample | Now | Status | Verified |
|---|---|---|---|
| g50 F18 cylinder | 82.8478429 (was 82.786126) | ✅ | v0.4.12 CLI 10-28 (bin/g50faces.py: F18 82.847884) |
| g50 F11 plane | 46.000000 (was 45.999575) | ✅ | v0.4.12 CLI 10-28 (F11 46.000000) |
| g50 F19 offset | 4.189139 (live 4.1893432, −0.0002) | ✗ small | v0.4.12 CLI 10-28: now **4.189385** (live 4.1893432, +0.00004; was −0.0002) — within 1e-5 relative |
| g50 F8 | 8.321612 (live 8.3308904, −0.0093) | ✗ live reports EXTRUDED: Onshape replaced the file's B_SURFACE (which collapses to the apex; its area over its whole parameter range is ours) with another surface on import. Live request filed ("api (10-24) IM2b follow-up") |  |
| g50 volume | 6052.680 (live 6052.761; was 6053.320) | ✅ band | v0.4.12 CLI 10-28 (qa, /tmp/qa-imp/impmatrix.json: 6052.678152) |
| McMaster F0 cylinder | 13.938182 (live 13.9122437, +0.026; was +0.034) | ✗ its helix edges: ours 159.394088 / 159.325155, live 159.392935 / 159.3450619; live request filed |  |
| McMaster thread flanks F1 / F2 | −0.0008 / −0.0012 | ✗ small |  |
| McMaster volume | 254.418923 (live 254.403780; was 254.521984) | ✅ band | v0.4.12 CLI 10-28 (qa, /tmp/qa-imp/impmatrix.json: 254.419624 (+0.0007 vs the row's 254.418923), in band) |

Surface types: a B-surface that is an exact linear extrusion (degree 1 across, rows translates) reports as an extrusion
(FeatureScript EXTRUDED, query_entities "extruded"), as Parasolid reports such surfaces; query_entities also names
REVOLVED faces "revolved". F8 isn't one in the file (see above).

## Re-run 2026-10-28 (qa, v0.4.12 CLI = green 1b81b7ca code, bin/impmatrix_qa.py over every sample, /tmp/qa-imp/impmatrix.json)
| Row | Ours | Status | Verified |
|---|---|---|---|
| I1 g36-g24b v25.0 … v38.1 / g25-b X_T | 7583.000866, 18 faces (all versions) | inside live's band 7576.2420 – 7589.5825 (grading rule 10-24); analytic-face areas not re-measured in this run | v0.4.12 CLI 10-28 (volumes, faces) |
| I1 g25-b SAT / STEP, g29-*.step, g47-1part.step | SAT 7582.996453 (was 7585.978524), STEP 7582.693101 (×5), bsurf 7582.995361, 18 faces | inside the band; areas to check | v0.4.12 CLI 10-28 (volumes, faces) |
| g50-g46-shell1 / g49-chamfer1 / g49-chamfer2 | 6052.678152 / 44833.514326 / 44789.962583 (were 6053.320139 / 44833.530070 / 44789.972897) | ✅ still in band; values moved since 10-24 | v0.4.12 CLI 10-28 |
| g35-…-rest-meters.stl | 8e-06 mm³ (read as mm) | ? unchanged (mesh unit) |  |
| dx-asm-xt-<v>.x_t (live's assembly exports, request 1536) | v0.4.12: 12.0 / 28.1 / 33.0 / 33.1 / 37.1 / 38.1 import nothing, 25.0 / 28.0 / latest ~1e-6 mm³ (XT-ASM-SCALE). Green 98667558: 25.0 / 28.0 / 38.1 / latest OK (pin 848.23), the other five fail. Green c82818b7: 12.0 / 28.1 / 33.0 / 33.1 / 37.1 import too (3 instances, pin 848.23); c38eb9cf: 30.0 / 30.1 / 31.0 / 31.1 as well | ✅ (XT-ASM-VERSIONS fixed bfe9101d) | c82818b7 10-28 (GUI) |
| Errors over all samples | 0 tool errors (dx-asm-xt failures come back as empty imports through the CLI) | ✅ | v0.4.12 CLI 10-28 (same run: 0 tool errors; 11 dx-asm-xt files and 2 zips (g39, g47) come back with no listed parts) |

## Re-run 2026-10-30 (dx, CLI from agent/dx b37ee1c3 = main 80499a93's code, the same script over every sample, /root/dx-scratch/imp/impmatrix.json)
128 of 143 samples are identical to qa's 10-28 run (every value, name and face count above stands). The 15 that changed:
| Row | Ours now | Live | Status | Verified |
|---|---|---|---|---|
| dx-asm-xt-<v>.x_t, all 14 files (12.0 … 38.1, latest, 28.1-recheck) | through the CLI too now: Part Studio "dx-asm-xt-<v> parts" with Part 1 8000.0 (6 faces) + Part 2 848.230017 (3 faces) | the block + the pin (request 1536) | ✅ (10-28 CLI: empty or ~1e-6 mm³) | dx CLI 10-30 (b37ee1c3) |
| dx-nist-assembly.x_t (live's G41 export, behaviours "dx (2026-10-27) (2) NIST assembly") | File tab "dx-nist-assembly.x_t", Part Studio "dx-nist-assembly parts" with 8 solids (91304A112_BLUE FLAT-HEAD SOCKET CAP SCREW-1, nist_mtc_crada_box_rev-D-1, …; 10-28: "SOLID" for the first and 41 / 44 faces on the 91274A118 / A141 screws, now 25 / 25), Assembly "nist_mtc_crada_assembly_rev-D" with 36 instances, no sub-assemblies | the same three, plus a "BOM : nist_mtc_crada_assembly_rev-D" element | ✅ names, tabs, instances; ✗ IM1b (no BOM element, api) | dx CLI 10-30 (b37ee1c3) |
| g35-…-rest-meters.stl | default unit = the workspace's (mm): 8e-06 mm³, as live's dialog defaults to the document units; `mesh_unit` Meter: 7542.497885 mm³, 1 face (the 252-triangle REST export) | dialog Units ▾ defaults to the document unit (behaviours G35) | ✅ default; Meter value: live's unknown | dx CLI 10-30 (b37ee1c3) |
| Errors over all samples | 0 tool errors; only the two zips (g39, g47) come back with no parts, as live ("not translated") | | ✅ | dx CLI 10-30 (b37ee1c3) |
| I1 g25-b.x_t (and g36-g24b v25.0 … v38.1) | 7583.000866; per face: the 14 planes and cylinders at their exact areas (96, 216, 384, 796.566371, 18.849556 = 2π·3·1, …), the 4 blends 13.201558 | live HIGH 7583.000048; live's x_t blend 13.2015552 (TICKETS I1) | ✅ (band, exact analytic faces, blends = live's) | dx CLI 10-30 (b37ee1c3) |
| I1 g25-b.sat | 7582.996453; planes / cylinders next to the blends off by up to −0.0009 mm² (384 → 383.999109; 91.398224 → 91.39858 / 91.39898; 217.06193 → 217.06229 / 217.06269) | live HIGH 7582.993215 | ✗ small (in band, but the analytic faces at the blends aren't exact) |  |
| I1 g25-b.step, g29-*.step, g47-1part.step | 7582.693101; the 14 analytic faces exact, the 4 blends 13.181875 each | live HIGH 7582.976771; live's STEP blends 13.20006–13.20009 = the file's patch integrated independently (TICKETS I1, dx 10-30) | ✗ (in band, but −0.284 mm³: our area / volume integral of the degenerate blend patch, core) |  |
| Import dialog options by type | x_t: appearances, material density, Y up, composite, join; STEP and SAT: no density; PVZ: appearances, Y up, composite (no density, no join); STL: no appearances, Units (app import_dialog.rs, `MATERIAL_DENSITY_TYPES`) | the same; live's NIST run (behaviours "dx (2026-10-27) (2)") corrects the 10-21 row: **the STEP dialog has no "Import material density"**, only Parasolid's | ✅ | dx 10-30 (code, main 80499a93) |
