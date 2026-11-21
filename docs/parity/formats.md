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

## Translator options audit (2026-11-21 03:46 UTC)

Live's Export and Import dialogs, format by format, each option against ours. Live's lists and defaults come from
live/feel/importexport.json (export_live2_1730: Part Studio PARASOLID, ACIS, STEP, IGES, PVZ, JT, RHINO, GLTF, GLB,
OBJ, 3MF, STL; Assembly the same with URDF and without RHINO). Evidence is `parity::exchange_round_trip` on the 22
exchange documents (Mac run 03:40): live's option files imported by ours, and ours with the same options read back
and compared with the model. Score: live's option exports in ours 39/50, ours with options read back 44/62.

Every option live shows has an `export_parts` / `import_files` parameter: no format and no option is missing.
- **full**: live's file and ours with the option match;
- **partial**: a row still differs (the gap is named);
- **untested**: the op has it, but there is no live file with the option yet (asked for in live/REQUESTS.md, dx
  2026-11-21 03:30).

### Export

| Format | Live option (default; choices) | Ours (`export_parts`) | Status | Evidence / gap |
|---|---|---|---|---|
| all | File name (the tab's) | `file` | full | e2e |
| all | Export models oriented Y axis up (off) | `y_up` | full | STL-YUP, OBJ-YUP-OBJ import as live's |
| all | Options: Download; Download and store file in a new tab; Store file in a new tab | `options` | full | e2e |
| all | Export unique parts as individual files (off) | `individual` | full | STEP-SPLIT, STL-SPLIT: zip entries as live's |
| all | Include hidden instances in export (off) | `include_hidden` | full | HIDDEN-EXCLUDED / -INCLUDED |
| PARASOLID | Version (38.1, 38.1 … 25.0; Use latest version) | `version` | partial | 25.0, 28.0, 33.1 full on housing + duct (25.0 to 27.1 files read since their LIMIT nodes have no term_use); live has no x_b option; cf-hose-routing's hoses read back with 14 edges / 4 vertices (live 12 / 0) |
| ACIS | – | – | partial | live.sat imports on 21 of 22 documents; c2-extension-spring area +0.01 %: tolerant edges' plane pcurves approximated by the healing (fix in progress) |
| STEP | Version (AP242; AP214, AP203) | `version` | full | AP203 (CONFIG_CONTROL_DESIGN), AP214 |
| STEP | Preprocessing (None; Automatic, Remove small entities only basic / advanced, … and convert all surfaces to b-surfaces) | `preprocessing` | partial | Automatic full on housing (live's file as None's); b-surfaces not converted: live's file has every surface a B-spline and closed faces in halves (46 faces, 118 edges), ours writes the exact surfaces (OCCT's angle split / revolution / B-spline conversion broke the housing: parked); duct waits on live's 17:20 rerun |
| STEP | Use custom units for export (off) | `units` | full | STEP-INCH (CONVERSION_BASED_UNIT('INCH')) |
| STEP | Include derived tolerances in export (on) | `derived_tolerances` | untested | e2e only |
| STEP | Use custom annotation text height for MBD export (off; 4 mm) | `mbd_text_height` | untested | e2e only |
| IGES | Use compatibility mode for surfaces and curves (off) | `iges_compatibility` | partial | entity counts as live's (46 / 52 / 104 on the housing); ours reads our housing file back with 39 faces (live 36): 2 split faces left unmerged |
| PVZ | Group composite parts into a single part file (on) | `group_composites` | full | both of live's files alike (35429 bytes); ours imports them as live on duct; housing: live's import is an empty sheet (its mesh isn't closed), ours keeps the mesh as sheets |
| JT | – | – | full | live.jt imports and reads back as live's on housing + duct |
| RHINO | Version (8; 7 … 2; Use latest version) | `version` | partial | V2, V4, V5, V6, V7, V8 import as live's (V6 / V7 duct wait on live's rerun); ours read back with 45 faces on the housing (live 46) |
| GLTF / GLB | Resolution (Medium; Coarse, Fine, Custom: distance / angular tolerance, max chord length) | `resolution`, `distance_tolerance`, `angular_tolerance`, `max_chord_length` | partial | live's Coarse / Fine / Compress files import as live on housing + duct; ours read back match except housing Coarse's area (+0.07 %, as OBJ / 3MF Coarse); ours meshes as OBJ / 3MF (below) |
| GLTF / GLB | Compress (off) | `compress` | full | Draco meshes (KHR_draco_mesh_compression, lossless, a buffer per face) as live-opt-compress.glb / .gltf (dx-urdf, draco-core 2.3); the import reads live's compressed files. The layout is live's: a primitive per face with TEXCOORD_0 (the face's u, v over 0..1), materials per colour named r_g_b_0.000000_0.000000; live's PTC_onshape_metadata node extension isn't written |
| OBJ | Resolution (Medium; as glTF) | as glTF | partial | facets as STL without its minimum facet width (dx-urdf f5cfb364): housing Coarse / Medium / Fine 1132 / 2156 / 5710 triangles (live 1258 / 2340 / 6188); rings-loft 20746 (live 26760): coarser on B-spline / cone faces |
| 3MF | Resolution (Medium; as glTF) | as glTF | partial | as OBJ |
| STL | STL Format (Binary; Text) | `stl_text` | full | STL-TEXT-INCH |
| STL | Units (Millimeter; Centimeter, Foot, Inch, Meter, Yard) | `units` | full | STL-CM, STL-FT, STL-TEXT-INCH |
| STL | Resolution (Medium; Coarse, Fine, Custom: chordal tolerance, angular deviation, minimum facet width) | `resolution`, `chordal_tolerance`, `angular_deviation`, `min_facet_width` | partial | housing Coarse full; Fine / Custom read back up to +0.02 % area; Coarse 678 triangles (live 924) |
| URDF (Assembly) | Geometry format (STL; GLTF, GLB, OBJ), Export rigid subassemblies as a single link | `geometry_format`, `rigid_subassemblies_single_link` | full | live's URDF file (URDF-LIVE-FILE-SHAPE) |

### Import

| Format | Live option (default) | Ours (`import_files`) | Status | Evidence / gap |
|---|---|---|---|---|
| all CAD | Import to this document / Combine to a single Part Studio | `combine` | partial | STEP-FLAT / -NOFLAT full; ASM-STEP-FLAT: our flattened parts lack live's names (Arm_1, Pin_1 …) |
| STEP, Parasolid, IGES, OBJ | Import appearances (on) | `appearances` | full | colours on every exchange document |
| all | Orient imported models with Y Axis Up (off) | `y_up` | full | YUP rows |
| all | Create a composite part when importing multiple or non-solid bodies (off) | `composite` | untested | e2e only |
| STEP, Parasolid, IGES | Join adjacent surfaces (off) | `join` | full | IGES-COMPAT-JOIN |
| Parasolid | Import material density (off) | `density` | full | PARASOLID-DENSITY imports as live on housing + duct |
| IGES | IGES postprocessing (off) | `iges_postprocessing` | partial | live's compatibility-mode file of c2-housing-draft, with it or without: ours 36 / 80 / 45 faces / edges / vertices, live 36 / 79 / 44. The one extra: a closed 4.33 mm B-spline rim at (12.5, 0, 2.7) mm, written as two IGES curves, read as two edges (join_loop_pieces joins only pieces of one curve; dx 11-21) |
| STL | Units (Millimeter) | `mesh_unit` | partial | a multi-part STL: c2-duct-multipart's live.stl reads as 2 parts, live 9 sheets (c2-bottle-split 2, live 148); joining only across edges two triangles share gives 31 / 142, so live's rule isn't that (REQUESTS dx 11-21 14:06) |
| OBJ | Units (Meter) | `mesh_unit` | full | OBJ-*-OBJ |
| DXF, PNG, PDF, CSV | no dialog: a file tab after the active tab | as live | full | e2e |

Next, in the lead's order (3MF, OBJ, ACIS, glTF, then the rest): the OBJ / 3MF / glTF meshes at live's density, then
the multi-part STL import, ACIS's spring, the IGES compatibility read-back. (The Rhino face count is as live: c2-housing-draft's
live-opt-v5.3dm reads 46 / 118 / 76, dx 11-21.)

## Live's own big files (request 1755, 11-02)

`dx_perf::timed_live_big_samples` imports live's samples (live/samples/dx-big, in `NOSHAPE_LIVE_BIG`). Ours is the
whole import in a dev build on vibe5; live's is the time to "Post-processing done".

| File | Ours | Live | Parts, names, colours, volume, instances |
|---|---|---|---|
| 1000-part / 2000-instance assembly, STEP AP242 | 4.6 s | 33.5 s | 1000, 1000, 900 + 100, 396562.770332 mm³ (live 396562.770315), 2000: as live |
| the same, Parasolid | 2.2 s | 12.9 s | as live |
| the same, IGES | 7.5 s | 69.3 s | as live (was 2000 flat parts, no colours: now read through XDE, shared subfigures as instances) |
| the 1000-part Part Studio, STEP AP242 | 3.8 s | 29.3 s | as live |
| NIST FTC 08 / FTC 09 / STC 09 (single PMI parts) | 0.6 / 0.6 / 0.6 s | 13.1 / 12.3 / 13.4 s | one solid each, live's volumes; live also lists their PMI curves |

## Exporting imported geometry (NIST, 11-02)

Every export format on the NIST assembly's 8 parts as imported (healed STEP), each part on its own:

| Format | Before | Now |
|---|---|---|
| STEP, ACIS, IGES, JT, glTF / GLB, OBJ, 3MF, STL | ✅ | ✅ |
| Parasolid | the writer crashed (signal 11) on the cap screws | ✅: a refit that misses a vertex is dropped (it read the dropped curve) |
| PVZ | 3 parts threw (`Geom_TrimmedCurve` out of range, then OCCT's NURBS conversion) | ✅: ranges trimmed to their curves; a shape the conversion throws on is repaired first. The screws read back as mesh within 1.7 % |
| Rhino | 7 of 8 failed openNURBS' check | ✅ all 8. Trims a period apart are shifted, trim ends matched, seam trims put on the domain edge; a part that still fails is converted again with every closed face split (loops crossing a seam), repaired first where OCCT's NURBS conversion throws (the cap screws, as PVZ) |

Kernel test: `imported_nist_parts_write_to_rhino_and_pvz`, `parasolid_writes_an_imported_thread`.

## Part, assembly and drawing formats

| Format | Extensions | Onshape import | ours | Onshape export | ours | Notes |
|---|---|---|---|---|---|---|
| Parasolid B-rep | .x_t .x_b (.xmt_txt .xmt_bin) | v10–38.1, parts and assemblies | ✅ | v25–37.0 (38.1 in the dialog) | ✅ | our own reader / writer (kernel xt.cpp); assemblies keep their structure; whole circles / ellipses are ring edges without a vertex, as Parasolid writes them (live 1855: our cylinder imports as a native one, 3 faces / 2 edges / 0 vertices) |
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

## Drawings of big models (live 1809, 1817; `dx_perf::timed_big_drawings`)

Live's "dx big asm" Assembly 1 (2000 instances) drawing: four views at 1:12 and the 1000-row BOM. Ours: dev on vibe5.

| Step | Ours | Live |
|---|---|---|
| Create Drawing with the suggested views | 1.3–2 s | 19.7 s |
| BOM table | 0.15 s | 13.1 s |
| Export PDF | 0.38 s (3.1 MB) | 7.2–11.6 s (499 KB) |
| Export DWG | 0.14 s (1.8 MB) | 13.9–16.0 s (1.35 MB) |
| Export DXF | 0.14 s (7.2 MB) | 17.6–18.3 s (18.7 MB) |
| Export PNG | 1.6 s | 9.4–9.6 s (3300 × 2550) |

Live shows a blue banner "<drawing> is being prepared for export. You will receive a notification with its status." for about 5 s, then the browser download.

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
- Parallel healing: OCCT patch `patches/occt/0019-step-parallel-shape-processing.patch` (TKDESTEP, STEPControl_ActorRead).
  - At a transfer's first root, the shape representations of every product (and the ones related to them) are transferred first, as the product transfer would (the root placement flag for top products, none for sub-products).
  - Their items' shape processing runs in parallel (OSD_Parallel), each with its own parameters and precision.
  - Each item's transfer information is merged over the entities it mapped, and the representations' compounds are rebuilt with the processed items. The rest of the transfer then reuses them.
  - Results are byte-identical BREPs: NIST's 9 parts, and every part of live's flat 1000-part file.
  - Dev on vibe5, with the patch built into a private OCCT:
    - NIST Transfer: 1252 → 540 ms;
    - live's flat 1000-part STEP: Transfer 1389 → 475 ms, import 3.8 → 2.1 s;
    - live's 2000-instance assembly: 4.0 → 1.8 s;
    - our 1000-part STEP: 1.4 → 0.54 s.
  - The build machines get it when their libTKDESTEP.a (and STEPControl_ActorRead.hxx) are rebuilt from the patch set (see docs/handoff/dx.md); until then results are the same, only slower.
  - OCCT heals inside its STEP actor (`STEPControl_ActorRead::TransferEntity`), per shape representation and with that representation's own uncertainty as the tolerance (NIST mixes 1e-8 and 2e-5 m).
  - The healed shape is bound at once for the parents (assemblies, mapped items) to use.
  - Healing afterwards with one tolerance would change results.
  - Doing it right means patching the actor to queue the jobs, run them on threads and merge the transfer info serially, then mapping face colours through the healing history.

`NOSHAPE_IMPORT_TIMING=1` and `NOSHAPE_EXPORT_TIMING=1` print the stages.
