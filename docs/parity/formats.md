# File format coverage: Onshape's supported formats against noshape

Source: Onshape help "Supported File Formats" (cad.onshape.com/help/Content/File/supported_file_formats.htm, read
2026-11-02 02:30 UTC). Measured by `crates/ops/tests/e2e.rs` `every_export_format_imports_back`: one part exported to every export
format, each file imported back where we read the format, part count and volume compared (B-rep formats exact, meshes
within 0.01 %). Big files and speed: `crates/ops/tests/dx_perf.rs` (`timed_big_part_imports`, `timed_nist_import`).
✅ supported and tested, ◐ partly, ✗ missing (feasible), ⊘ needs a vendor library or an undocumented format.

## Counts

Onshape imports 22 CAD / 2D formats and exports 21. noshape: import 17 ✅, 1 ◐ (NX: parts yes, assemblies ⊘), 4 ⊘
(CATIA v5, Creo / Pro/E, Parasolid mesh, Parasolid mixed model); export 19 ✅, 2 ⊘ (Parasolid mesh, mixed model).
Every export format round-trips through our importers (12 part formats measured). Non-CAD: Onshape stores 13 file
types and views 10 of them. noshape stores all 13 as file tabs and views:
- PNG, JPEG, SVG, GIF, PDF, text, Markdown, CSV and JSON ✅;
- not video (⊘: needs a native decoder).

`import_files` stores any non-CAD file as a file tab, as the app's + › Import… does.

## Part, assembly and drawing formats

| Format | Extensions | Onshape import | ours | Onshape export | ours | Notes |
|---|---|---|---|---|---|---|
| Parasolid B-rep | .x_t .x_b (.xmt_txt .xmt_bin) | v10–38.1, parts and assemblies | ✅ | v25–37.0 (38.1 in the dialog) | ✅ | our own reader / writer (kernel xt.cpp); assemblies keep their structure |
| Parasolid mesh | zip (x_t + xmm_txt / x_b + xmm_bin), .x_b | v28–37 | ⊘ | v28–37 | ⊘ | the XMM mesh format is Siemens' and undocumented |
| Parasolid mixed model | zip | v32+ | ⊘ | yes | ⊘ | as above |
| ACIS | .sat (.sab) | up to 2023 1.0 | ✅ | Version 5 | ✅ | |
| STEP | .stp .step | AP203 / AP214 / AP242, geometry and face colour; assemblies | ✅ | AP203 / AP214 / AP242, geometry, MBD, face colour | ✅ | subassemblies kept (X1); MBD as AP242 semantic PMI (X3) |
| IGES | .igs .iges | up to 5.3 (parts) | ✅ | 5.3 | ✅ | |
| CATIA v5 | .CATPart .CATProduct | R7–R33, assemblies as zip | ⊘ | – | – | Dassault's format: needs a licensed reader (Spatial, CoreTechnologie, Datakit…) |
| SOLIDWORKS | .sldprt, Pack and Go zip | 1999–2026 | ✅ | – | – | the Parasolid stream embedded in the part; .sldasm zips |
| Inventor | .ipt, .iam zip | 9–2026 | ✅ | – | – | the ACIS stream embedded |
| Pro/ENGINEER, Creo | .prt .asm (zip) | 2000i–Creo 12 | ⊘ | – | – | PTC's format: needs a licensed reader; our .prt is NX's |
| JT | .jt | up to 11.0 | ✅ | up to 11.0 | ✅ | |
| Rhino | .3dm | 2–8 | ✅ | 2–8 | ✅ | openNURBS (export where the build has it: `RHINO_EXPORT`) |
| STL | .stl | mesh, view only | ✅ | yes | ✅ | Resolution › Custom (X2) |
| OBJ | .obj | mesh, view only | ✅ | yes (zip with .mtl) | ✅ | |
| NX | .prt, assemblies as zip | UG15–NX 2512 | ◐ | – | – | parts ✅ (Parasolid embedded); assemblies ⊘ (NX's structure is undocumented) |
| Solid Edge | .par .psm, .asm zip | 10–2026 | ✅ | – | – | |
| glTF | .gltf (.glb) | 2.0 | ✅ | yes | ✅ | |
| 3MF | .3mf | yes | ✅ | yes | ✅ | |
| PVZ | .pvz | yes | ✅ | 8.0 | ✅ | |
| URDF | – | – | – | assemblies | ✅ | |
| AutoCAD DWG | .dwg | up to 2018 | ✅ | R11–14 … 2018 | ✅ | drawings, sketches, flat patterns |
| DXF | .dxf | up to 2018 | ✅ | R11–14 … 2018 | ✅ | |
| DWT | .dwt | 2013, 2018 | ✅ | 2013, 2018 | ✅ | drawing templates |
| PDF / SVG / PNG / JPEG (drawings) | .pdf .svg .png .jpg | – | – | yes | ✅ | plus TIFF, inspection and definition lists |

## Non-CAD files (Onshape: import for viewing or storage)

| Type | Extensions | Onshape | ours | Notes |
|---|---|---|---|---|
| PDF | .pdf | viewed | ✅ | live shows Chrome's PDF viewer (dark): ours copies it. A bar with ☰, the title, the page box "1 / 2" and − 100 % +, page thumbnails at the left, the pages one under the other. The first 50 pages are rendered in the background by hayro, a pure-Rust rasterizer (MIT / Apache, `crates/ops/src/file_preview.rs`). The agent API: `render_file_page` (e2e `a_pdf_file_tab_renders_its_pages`) |
| Video | .mp4 | viewed | ⊘ | stored. Viewing needs an H.264 decoder: open-source ones (ffmpeg, openh264) exist, but they're native libraries, and H.264 is patent-licensed. Left out for now |
| Images | .png .jpg .jpeg .svg .gif (and .bmp .tif .webp) | viewed | ✅ | at actual size from the top left, on white, no toolbar (live 1756) |
| Text, Markdown, CSV, JSON | .txt .md .csv .json | viewed | ✅ | as live 1756: TXT and Markdown raw in monospace (live doesn't render Markdown); CSV a grid with column and row numbers, every line a row (a final line break gives an empty row); JSON as written, in a code view with line numbers and colours. The first 5000 lines |
| Programming files, MOV, Word | .py .java .js .mov .doc .docx | stored, not viewable | ✅ | stored as file tabs |

## Speed on big files (see docs/handoff/dx.md for the history)

Release timings from `tools/tiers/perf.sh` on vibe3 (`dx_perf::timed_nist_import`, `dx_perf::timed_big_part_imports`; vibe3 was shared with another perf run, so read them as upper bounds). Live's timings for the same files are pending (live request 2026-11-02 01:57); its NIST times come from the earlier import audit.

| File | Before (10-31) | Now | Live |
|---|---|---|---|
| NIST CTC assembly, STEP (8 parts) | 12.2 s | 8.0 s | 38.7 s |
| NIST CTC assembly, Parasolid | 7.4 s | 3.9 s | 24.5 s |
| 1000-part pattern, STEP import | | 1.6 s (1000 parts, volume exact) | pending |
| 1000-part pattern, Parasolid import | | 1.0 s | pending |
| 1000-part pattern, IGES import | | 1.4 s | pending |
| 1000-part pattern, STEP / x_t / IGES export | | 1.4 / 0.19 / 0.51 s | pending |

What changed (dev build timings on vibe5, 2000 parts):

- STEP import: face colours were looked up by a document-wide search per face (quadratic). They're now read from each part's sub-shape labels: collect went from 2920 ms to 3 ms, and the whole import from 4.6 s to 1.6 s.
- STEP / Parasolid assembly import: each part's body follows from the Import feature's numbering, with no probe regeneration. NIST apply went from 2.15 s to 0.1 s. `NOSHAPE_IMPORT_CHECK=1` compares the result with the probe.
- STEP export: all parts without model definitions go through one OCCT call, then are split by product. It took 2.0 s and now takes 1.15 s (OCCT 1404 → 752 ms, layout 582 → 337 ms).
- Big assembly (`dx_perf::timed_big_assembly_round_trip`, the 1000-instance pin fixture, dev on vibe5):

  | Format | Export | Import | Result |
  |---|---|---|---|
  | STEP | 16 ms (571 KB) | 250 ms | 1 part, 1000 instances, named "Part 1" |
  | Parasolid | 5 ms (111 KB) | 36 ms | same |
  | IGES | 378 ms (7.7 MB) | 939 ms | 1000 parts, flat (IGES has no assembly) |

- NIST import, what's left (dev): reading is 1.8 s, of which:
  - OCCT's parse: 0.15 s.
  - OCCT's Transfer: 1.24 s, nearly all of it shape healing. With healing off it takes 0.05 s, but every part is then invalid and the volumes are off by up to 70 %.
  - Writing the Import feature's BREP text: 0.41 s for 26 MB.

  Most of the healing time, and 17 of the 26 MB, are the two cap screws' thread pcurves. Onshape writes no pcurves, so OCCT projects each thread edge onto its B-spline surface, which gives 2D B-splines with thousands of poles. A user precision makes it slower: 1e-4 mm takes 28 s, with the same sizes.
- Exporting imported parts (`dx_perf::timed_many_heavy_parts`: NIST's cap screw as a part pattern of 25, STEP export, then import):

  | | export | file | re-import (Transfer) |
  |---|---|---|---|
  | before | 7.8 s | 99 MB | 433 s (419 s) |
  | now | 2.0 s | 16 MB | 18.8 s (11.2 s) |

  Two fixes in our STEP writer (IGES gets the first):
  - Healing split the screw's helix into 20 edges, each keeping the whole 848-pole curve. Each edge now writes only the segment it uses (`compact_bspline_edges`, on a copy).
  - Every part declared Onshape's 1e-8 m uncertainty, so readers healed the screw again at that precision (20 s per screw; 0.5 s at its own tolerance). A part now declares its greatest tolerance when that is over 1 µm. Modelled parts keep 1e-8 m.

  Kernel test: `step_writes_the_used_part_of_long_splines`.
- After reading, regeneration takes about 1.7 s, of which the features are 0.12 s and the display meshing the rest (the Part Studio display, psui's area).
- Not done: healing parts in parallel, which would cut the 0.84 s Transfer of the 2000-part import. Parked until live's numbers show we're behind.
  - OCCT heals inside its STEP actor (`STEPControl_ActorRead::TransferEntity`), per shape representation and with that representation's own uncertainty as the tolerance (NIST mixes 1e-8 and 2e-5 m).
  - The healed shape is bound at once for the parents (assemblies, mapped items) to use.
  - Healing afterwards with one tolerance would change results.
  - Doing it right means patching the actor to queue the jobs, run them on threads and merge the transfer info serially, then mapping face colours through the healing history.

`NOSHAPE_IMPORT_TIMING=1` and `NOSHAPE_EXPORT_TIMING=1` print the stages.
