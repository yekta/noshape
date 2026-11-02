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
types and views 10 of them; noshape stores all 13 as file tabs; views PNG, JPEG, SVG, GIF ✅ and text, Markdown, CSV, JSON ◐ (as
plain text), 2 need libraries (⊘: PDF, video).

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
| PDF | .pdf | viewed | ⊘ | stored; a page view needs a PDF renderer (pdfium / MuPDF) |
| Video | .mp4 | viewed | ⊘ | stored; needs a video decoder |
| Images | .png .jpg .jpeg .svg .gif (and .bmp .tif .webp) | viewed | ✅ | the file tab shows the picture (actual size, scaled down to fit) under a bar with name, size, Update…, Download; app check 11-02; live's look asked (request 11-02) |
| Text, Markdown, CSV, JSON | .txt .md .csv .json | viewed | ◐ | shown as text (JSON pretty-printed, the first 5000 lines); Markdown not rendered yet, CSV not as a table: live's look asked |
| Programming files, MOV, Word | .py .java .js .mov .doc .docx | stored, not viewable | ✅ | stored as file tabs |

## Speed on big files (see docs/handoff/dx.md for the history)

To be filled from `tools/tiers/perf.sh` (vibe3, release) with live's own timings (live request 2026-11-02 01:57).
