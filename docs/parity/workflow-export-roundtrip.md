# Workflow: export → re-import round trips (bracket, every export format)

qa5, 2026-10-21 pass 4, CLI /tmp/qa5/noshape-cli-1436; source "qa5 bracket" (67191.3275328646 mm³, 20 faces = live's bracket);
script /root/qa5-scratch/pass3/roundtrip.py (export_parts defaults → import_files defaults into qa5-roundtrip.noshape).
Status: ✅ exact / expected, ✗ broken, ? live comparison pending.

| Format | File size | Re-imported | Status | Verified |
|---|---|---|---|---|
| Parasolid (.x_t) | 14739 B | 67191.327533, 20 faces | ✅ (header still v12, I5) | 77870dd4 10-28 (UI export header SCH_3801231_38101) |
| ACIS (.sat) | 21947 B | 67191.327533, 20 | ✅ | v0.4.12 CLI 10-28 (qa 10:48, bin/roundtrip_qa.py, source bracket 67643.716875 / 18 faces: .sat 20139 B → 67643.716875, 18) |
| STEP | 72081 B | 67191.327533, 20 | ✅ | 77870dd4 10-28 (UI: wf-block 6649.885 → STEP → re-import 6649.885) |
| IGES | 58968 B | 67191.327532, 20 | ✅ (header DOC1) | v0.4.12 CLI 10-28 (qa 10:48, bin/roundtrip_qa.py, source bracket 67643.716875 / 18 faces: .igs 50463 B → 67643.716875, 18) |
| JT | 34689 B | 67191.327533, 20 | ✅ | v0.4.12 CLI 10-28 (qa 10:48, bin/roundtrip_qa.py, source bracket 67643.716875 / 18 faces: .jt 26862 B → 67643.716875, 18) |
| PVZ | 70509 B | mesh 67194.905652 (+0.005 %), 1 face | ✅ (mesh, as Onshape) | v0.4.12 CLI 10-28 (qa 10:48, bin/roundtrip_qa.py, source bracket 67643.716875 / 18 faces: .pvz 51496 B → mesh 67646.192871 (+0.004 %), 1 face) |
| glTF / GLB | 46988 / 35484 B | mesh 67196.545601, 1 face | ✅ (mesh) | v0.4.12 CLI 10-28 (qa 10:48, bin/roundtrip_qa.py, source bracket 67643.716875 / 18 faces: .gltf 32519 / .glb 24632 B → mesh 67646.763954, 1 face) |
| 3MF | 78273 B | mesh 67196.546674, 1 face | ✅ (mesh) | v0.4.12 CLI 10-28 (qa 10:48, bin/roundtrip_qa.py, source bracket 67643.716875 / 18 faces: .3mf 42970 B → mesh 67646.765079, 1 face) |
| **Rhino (.3dm)** | – | **export_failed "unsupported: Rhino export needs openNURBS: build with OPENNURBS_ROOT"** (Onshape exports Rhino; live G-series has .3dm imports) | ✗ **EXP1** (kern / packaging: the shipped build must include it) |  |
| **OBJ** | 45214 B | **import fails: "i/o error: could not read mesh file …bracket.obj"** (our own OBJ export can't be read back) | ✗ **EXP2** (kern) |  |
| STL (CLI defaults) | 47284 B | 6.7e-05 mm³: the export defaults to **metres**, the import to **mm** | ? (live: UI export dialog Units sticky "Meter", REST always metres; UI import Units defaults to the document units, so Onshape behaves the same way. Check that our UI export dialog defaults/stickiness match) |  |

## Re-run on integration 5d28447 (2026-10-21 17:05–17:20, /tmp/qa5/noshape-1659 = /root/target-lead/debug/noshape 16:59; no CLI built from 5d28447 yet, so CLI-only geometry rows stay on the 14:36 CLI result)

## Re-run pass 7 (2026-10-21 18:56–19:50, app /tmp/qa5/noshape-1851 = target-shared 18:51 (tree d5745c9 + uncommitted work), CLI /tmp/qa5/noshape-cli-1809 = target-engine 18:09)
| Row | Ours | Status | Verified |
|---|---|---|---|
| EXP2 OBJ round trip | 72076 B, re-imports as mesh 67196.546728, 1 face | ✅ **fixed** | v0.4.12 CLI 10-28 (qa 10:48, bin/roundtrip_qa.py, source bracket 67643.716875 / 18 faces: OBJ exports b2.zip (.obj + .mtl; the zip itself is now refused with live's MTL message, OBJ-ZIP-MSG); the bare .obj → mesh 67646.764949, 1 face) |
| EXP1 Rhino | export_failed "unsupported: Rhino export needs openNURBS" | ✗ still | v0.4.12 CLI 10-28 (qa 10:48, bin/roundtrip_qa.py, source bracket 67643.716875 / 18 faces: Rhino now exports (.3dm 68317 B) and re-imports exactly 67643.716875, 18 faces: EXP1 no longer reproduces) |
| other nine formats | unchanged (exact or mesh) | ✅ | v0.4.12 CLI 10-28 (qa 10:48, bin/roundtrip_qa.py, source bracket 67643.716875 / 18 faces: x_t 13630 / STEP 23041 B exact; STL comes back 6.8e-05 mm³ (unitless file read in another unit, like the g35 STL row)) |

## Re-run pass 12 (2026-10-22 ~01:35, CLI /tmp/qa5/noshape-cli-0107 = target-engine 01:07; main still 5bd50b2)
| Row | Ours | Status | Verified |
|---|---|---|---|
| EXP1 Rhino | export_failed "unsupported: Rhino export needs openNURBS: build with OPENNURBS_ROOT (scripts/build-opennurbs.sh)" | ✗ still (the engine build has no openNURBS) |  |

## Re-run pass 13 (2026-10-22 ~05:20–06:40, main aa75fdf: app /tmp/qa5/noshape-0503 = /root/target-lead/debug/noshape 05:03, CLI /tmp/qa5/noshape-cli-0226 = target-engine 02:26)
| EXP1 Rhino | "Rhino export needs openNURBS" | ✗ still |

## Re-run pass 15 (2026-10-22, CLI /tmp/qa5/noshape-cli-0521 = target-engine 05:21; main still aa75fdf)
CLI rows unchanged: SF2 8000/10 faces, SF5 2400 mm², SF6 −514.304543; SM1 8383.1217/20, hem 9254.2367/30, SM3 "Closed corners are not supported yet"; ADV2 5927.754632; EXP1 openNURBS. ✗ still.

## Re-run pass 18 (2026-10-22 ~10:00–11:00, CLI /tmp/qa5/noshape-cli-0811 = target-engine 08:11; main 3ce6952)
CLI rows unchanged: SF2 8000/10 faces, SF5 2400 mm², SF6 −514.304543; SM1 8383.1217/20, hem 9254.2367/30, SM3 "Closed corners are not supported yet"; ADV2 5927.754632; EXP1 openNURBS; SML1 loft Add "not supported yet". ✗ still.

## Re-run pass 21 (2026-10-23 ~17:30–18:30, app = /root/target-lead/debug/noshape 10-23 17:10 (main 88fbf8f), CLI = target-lead noshape-cli 10-23 13:45)
EXP1 Rhino: "Rhino export needs openNURBS" → ✗ still, routed kern (packaging).

## Re-run pass 22 (qa5, 2026-10-24, CLI built from main c59c5af9 after the api import/export batch; /root/qa5-scratch/pass21/impmatrix21.*)
| Row | Ours now | Status | Verified |
|---|---|---|---|
| DOC1 IGES header | Start "File generated by NOSHAPE Version 0.4.6"; Global 1Hb (part), 6Hb.IGES, 7HNOSHAPE, 5H0.4.6, 32,24,8,56,15, 1., 2, 2HMM, 1000, 1., 13H261024.063348 (UTC), 1D-05, 1000000., empty author/org, 9, 0; space-padded sequence numbers | ✅ live's layout; product strings are noshape's own by decision | a6c56638 10-28 (UI IGES: "File generated by NOSHAPE Version 0.4.6") |
| I5 Parasolid header | PART1 fields as live (MC…unknown, FORMAT=text, GUISE=transmit, KEY/FILE = file name, DATE … (UTC)); FRU/APPL = "noshape 0.4.6"; SCH_3801231_38101 and modeller 3801231 in every version, as live | ✅ | 77870dd4 10-28 |
| RHINO1 (EXP1) | "unsupported: Rhino export needs openNURBS: build with OPENNURBS_ROOT" | ✗ packaging (kern) |  |

## Re-run pass 24 (qa5, 2026-10-24 ~00:20, CLI target-engine 13:51; source = tonight's bracket without step 8 (ADV9), 65431.327533 mm³ / 18 faces)
| Format | Re-import | Status | Verified |
|---|---|---|---|
| Parasolid, ACIS, STEP, IGES, JT | 65431.327533 mm³, 18 faces (exact) | ✅ | v0.4.12 CLI 10-28 (bracket24 67643.716875 / 18): x_t / sat / step / igs / jt all 67643.716875, 18 faces |
| Rhino (3dm) | 65431.327331, 18 faces (this CLI build has openNURBS) | ✅ (RHINO1: works where openNURBS is built in) | v0.4.12 CLI 10-28 (bracket24 67643.716875 / 18): 3dm 67643.716875, 18 faces (release build has openNURBS) |
| PVZ, glTF, GLB, OBJ, 3MF | 65434.9 – 65436.5 mm³ (mesh), 1 face | ✅ (mesh tolerance) | v0.4.12 CLI 10-28 (bracket24 67643.716875 / 18): pvz 67646.192871, gltf / glb 67646.763954, 3mf 67646.765079, obj (from its zip) 67646.764949, 1 face |
| STL, CLI defaults | 6.5e-05 mm³: exported in metres, imported in mm | ✅ as live (row 19: same in Onshape) | 77870dd4 10-28 (UI STL dialog: Units Meter by default, as live's sticky Meter) |

## Re-run 2026-10-28 (qa, v0.4.12 CLI, bin/roundtrip_qa.py on tickets/bracket24.noshape = 67643.716875 mm³ / 18 faces)
| Row | Ours | Status | Verified |
|---|---|---|---|
| Exact formats (Parasolid, ACIS, STEP, IGES, JT, Rhino) | 67643.716875, 18 faces each | ✅ | v0.4.12 CLI 10-28 |
| Mesh formats (PVZ, glTF, GLB, 3MF) | 67646.19 – 67646.77, 1 face | ✅ (mesh) | v0.4.12 CLI 10-28 |
| OBJ | export writes <name>.zip (obj + mtl) as live's OBJ download; import_files of that zip fails "Root assembly file not found. Ensure zip file name matches root assembly name and the archive does not have folders" (b2.zip holds b2.obj); the bare .obj re-imports 67646.764949 / 1 face | ✗ OBJ-ZIP-MSG (live 1540: such a zip isn't translated, "…Translation is not supported for MTL (.mtl) files."); live's own .obj imports 72215.0759 in ours = live | v0.4.12 CLI 10-28 |
| STL (CLI defaults) | 6.8e-05 mm³ (metres out, mm in) | ✅ as live | v0.4.12 CLI 10-28 |
