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
