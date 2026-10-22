# Workflow: export → re-import round trips (bracket, every export format)

qa5, 2026-10-21 pass 4, CLI /tmp/qa5/noshape-cli-1436; source "qa5 bracket" (67191.3275328646 mm³, 20 faces = live's bracket);
script /root/qa5-scratch/pass3/roundtrip.py (export_parts defaults → import_files defaults into qa5-roundtrip.noshape).
Status: ✅ exact / expected, ✗ broken, ? live comparison pending.

| Format | File size | Re-imported | Status |
|---|---|---|---|
| Parasolid (.x_t) | 14739 B | 67191.327533, 20 faces | ✅ (header still v12, I5) |
| ACIS (.sat) | 21947 B | 67191.327533, 20 | ✅ |
| STEP | 72081 B | 67191.327533, 20 | ✅ |
| IGES | 58968 B | 67191.327532, 20 | ✅ (header DOC1) |
| JT | 34689 B | 67191.327533, 20 | ✅ |
| PVZ | 70509 B | mesh 67194.905652 (+0.005 %), 1 face | ✅ (mesh, as Onshape) |
| glTF / GLB | 46988 / 35484 B | mesh 67196.545601, 1 face | ✅ (mesh) |
| 3MF | 78273 B | mesh 67196.546674, 1 face | ✅ (mesh) |
| **Rhino (.3dm)** | – | **export_failed "unsupported: Rhino export needs openNURBS: build with OPENNURBS_ROOT"** (Onshape exports Rhino; live G-series has .3dm imports) | ✗ **EXP1** (kern / packaging: the shipped build must include it) |
| **OBJ** | 45214 B | **import fails: "i/o error: could not read mesh file …bracket.obj"** (our own OBJ export can't be read back) | ✗ **EXP2** (kern) |
| STL (CLI defaults) | 47284 B | 6.7e-05 mm³: the export defaults to **metres**, the import to **mm** | ? (live: UI export dialog Units sticky "Meter", REST always metres; UI import Units defaults to the document units, so Onshape behaves the same way. Check that our UI export dialog defaults/stickiness match) |

## Re-run on integration 5d28447 (2026-10-21 17:05–17:20, /tmp/qa5/noshape-1659 = /root/target-lead/debug/noshape 16:59; no CLI built from 5d28447 yet, so CLI-only geometry rows stay on the 14:36 CLI result)

## Re-run pass 7 (2026-10-21 18:56–19:50, app /tmp/qa5/noshape-1851 = target-shared 18:51 (tree d5745c9 + uncommitted work), CLI /tmp/qa5/noshape-cli-1809 = target-engine 18:09)
| Row | Ours | Status |
|---|---|---|
| EXP2 OBJ round trip | 72076 B, re-imports as mesh 67196.546728, 1 face | ✅ **fixed** |
| EXP1 Rhino | export_failed "unsupported: Rhino export needs openNURBS" | ✗ still |
| other nine formats | unchanged (exact or mesh) | ✅ |

## Re-run pass 12 (2026-10-22 ~01:35, CLI /tmp/qa5/noshape-cli-0107 = target-engine 01:07; main still 5bd50b2)
| Row | Ours | Status |
|---|---|---|
| EXP1 Rhino | export_failed "unsupported: Rhino export needs openNURBS: build with OPENNURBS_ROOT (scripts/build-opennurbs.sh)" | ✗ still (the engine build has no openNURBS) |
