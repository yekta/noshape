| DR-01 | Drawing element (tab) and its context menu | P0 | App data model: drawing element with versioned model references; tab commands |
| DR-02 | Creating a drawing (from part/assembly/Part Studio or empty) | P0 | App workflow + auto views via HLRBRep (front/top/right/iso) |
| DR-03 | Template selection dialog (filters, ANSI/ISO/JIS) | P1 | App UI over built-in + user template store |
| DR-04 | Custom template dialog (standard, size, units, projection, border, zones, title block) | P0 | Procedural 2D border/zone/title-block generator |
| DR-05 | Custom drawing templates (DWT authoring/import) | P1 | Native template format + DWT via DWG library (ODA/LibreDWG) |
| DR-06 | Drawing layers and Move to | P1 | Layer attribute in 2D annotation model; lock state |
| DR-07 | Title block and automatic property fields | P0 | Field-expression text runs bound to metadata |
| DR-08 | Sheets panel (sort, pin, replace, change configuration) | P1 | App UI over view/reference graph |
| DR-09 | Sheet operations (add, duplicate, copy, delete, reorder, rename, move views) | P0 | App data model operations |
| DR-10 | Sheet properties and replacing sheet format | P0 | App data model; custom size up to 1800x1800 in |
| DR-11 | Reference manager (update references) | P2 | App data model (version re-pointing) |
| DR-12 | Drawing Properties panel framework (update from DWT, lock) | P1 | Style system: override > drawing props > template |
| DR-13 | Drawing Properties - Units and Precision | P0 | Number/units formatting module (fractions, dual, zero suppression) |
| DR-14 | Drawing Properties - Dimensions | P1 | Style defaults for dimension renderer |
| DR-15 | Drawing Properties - Annotations | P1 | Style defaults for annotation renderer |
| DR-16 | Drawing Properties - Views (projection, edge styles, view-type defaults) | P0 | HLR edge-class to linestyle mapping; view defaults |
| DR-17 | Drawing Properties - Construction Geometry | P1 | Style defaults for centerlines/centermarks/sketches |
| DR-18 | Drawing Properties - Formats | P1 | Layer lock + styles |
| DR-19 | Drawing Properties - Tables | P1 | Table style defaults |
| DR-20 | Drawing Properties - Inspection | P2 | Inspection item style defaults |
| DR-21 | Styles panel (per-entity overrides) | P1 | Per-entity style override records |
| DR-22 | Format painter | P2 | Copy style/tolerance/content between annotations |
| DR-23 | Interaction basics (grips, snap points, inferencing, copy) | P0 | 2D R-tree hit testing, Geom2dAPI_ProjectPointOnCurve snapping |
| DR-24 | View model (parent/child, alignment, labels, creation matrix) | P0 | View DAG data model with gp_Ax2 per view |
| DR-25 | Insert view (base view) | P0 | HLRAlgo_Projector + HLRBRep_Algo/PolyAlgo on part/assembly compound |
| DR-26 | View properties dialog | P0 | App UI; gp_Trsf2d for rotation/scale |
| DR-27 | View display modes (hidden lines, tangent edges, shaded, render mode, simplification) | P0 | HLRBRep_Algo (exact) / HLRBRep_PolyAlgo + BRepMesh (fast); edge classes; offscreen raster for shaded |
| DR-28 | Projected view | P0 | Rotated gp_Ax2 + HLR; reuse parent cut solid |
| DR-29 | Auxiliary view | P1 | View direction from 2D edge via inverse projector + HLR |
| DR-30 | Section view (full, jogged/offset, partial, angular) | P0 | Jog polygon face -> BRepPrimAPI_MakePrism -> BRepAlgoAPI_Cut; section faces from history; HLR |
| DR-31 | Aligned section view | P2 | Split + BRepBuilderAPI_Transform rotation of revolved portion + Cut + HLR |
| DR-32 | Broken-out section view | P1 | GeomAPI_Interpolate closed spline face -> prism from depth -> BRepAlgoAPI_Cut |
| DR-33 | Detail view | P0 | 2D clip (Geom2dAPI_InterCurveCurve) + scale of parent HLR output |
| DR-34 | Break view | P1 | 2D strip removal and translation of HLR output; break-line symbols |
| DR-35 | Crop view | P1 | BRepAlgoAPI_Common with full-depth prism (or 2D clip) |
| DR-36 | Flat pattern view (sheet metal) | P1 | Planar flat-pattern outline projection + bend line attributes |
| DR-37 | Hatching (section hatch, hatch region tool) | P0 | Geom2dHatch_Hatcher with .pat-style line families |
| DR-38 | View context menu: show/hide options | P1 | HLR input/output filters; BRepGProp for center of mass |
| DR-39 | View context menu: orientation, tangent edges, line style, order, align/rotate | P1 | App logic; per-edge style overrides by edge id |
| DR-40 | View context menu: display state, explode/position, configuration | P1 | Assembly evaluation feeds instance transforms to HLR |
| DR-41 | Move/copy/delete/group views; replicate annotations | P1 | App logic; 2D geometric correspondence matching |
| DR-42 | Updating a drawing | P0 | Re-evaluate model, rerun cuts + HLR with cache, remap references |
| DR-43 | Dangling entities and reattachment | P0 | Reference resolution layer (persistent ids) with red grips |
| DR-44 | Cosmetic threads in views | P1 | Synthesized thread wires from hole/thread attributes + HLR occlusion; ISO 3/4 arc in 2D |
| DR-45 | Dimension framework (associativity, placement, override, jog, break) | P0 | Measure on 3D refs projected via projector; BRepExtrema / Geom2d extrema |
| DR-46 | Smart Dimension tool | P0 | Entity-type inference + DR-45 |
| DR-47 | 2-point linear dimension | P0 | 2D point distance / axis projection |
| DR-48 | Point-to-line dimension | P0 | gp_Lin2d::Distance |
| DR-49 | Line-to-line dimension | P0 | Parallel gp_Lin2d distance; silhouette-owner face for diameter symbol |
| DR-50 | Line-to-line angular dimension | P0 | gp_Dir2d::Angle with quadrant selection |
| DR-51 | 3-point angular dimension | P1 | 2D vector angle |
| DR-52 | Radial dimension | P0 | Geom_Circle radius of source edge; foreshortened jog rendering |
| DR-53 | Diameter dimension | P0 | Geom_Circle radius x2 |
| DR-54 | Foreshorten dimension | P1 | Centerline-to-edge distance x2 / cylinder radius |
| DR-55 | Arc length dimension | P1 | GCPnts_AbscissaPoint::Length |
| DR-56 | Chamfer dimension | P1 | 2D line intersection, length and angle; 45-degree style rules |
| DR-57 | Ordinate dimension (groups from datum) | P1 | 2D projection onto group axis; group layout |
| DR-58 | Maximum/minimum dimension | P2 | Center distance +/- radii; Geom2dAPI_ExtremaCurveCurve |
| DR-59 | Dimension palette (text, precision, units, symbols, tolerances, dual) | P0 | Text layout + tolerance formatter; fit tables from stdlib data |
| DR-60 | Tolerance model (ToleranceType, fits, default tolerances) | P1 | Shared tolerance data model with Part Studio/MBD |
| DR-61 | Note (rich text, leaders, fractions, symbols) | P0 | Text engine (rustybuzz/ttf-parser + custom SHX) |
| DR-62 | Property fields and date formats | P0 | Template-string evaluator over metadata |
| DR-63 | Flag notes | P2 | Note list-item flags referenced by callouts |
| DR-64 | Leaders, attachment and annotation grouping | P0 | 2D annotation attachment graph |
| DR-65 | Callout (balloon), BOM-linked and stacked | P0 | Annotation bound to BOM row via HLR edge provenance -> instance |
| DR-66 | Datum feature symbol | P1 | Annotation renderer |
| DR-67 | Datum target | P2 | Annotation renderer + locating dimensions |
| DR-68 | Geometric tolerance (feature control frame) | P1 | FCF layout + GD&T glyph library (XCAFDimTolObjects for STEP PMI if needed) |
| DR-69 | Surface finish symbol | P1 | Symbol renderer (ISO 1302 / ASME Y14.36) |
| DR-70 | Weld symbol | P2 | Symbol renderer (AWS A2.4 / ISO 2553) |
| DR-71 | Hole / thread callout | P1 | Hole feature attributes on faces -> callout text generator |
| DR-72 | Centerlines (2-point, edge-to-edge, circle) | P1 | 2D construction (gp_Lin2d, gce_MakeCirc2d) |
| DR-73 | Centermarks (single, circular, linear, automatic) | P1 | Circle centers from source edges + pattern attributes |
| DR-74 | Virtual sharp | P2 | IntAna2d_AnaIntersection of two lines |
| DR-75 | Insert image | P1 | Image decode + embed in display list/export |
| DR-76 | Inspection items | P2 | Annotation numbering model |
| DR-77 | Drawing inspection table and CSV exports | P2 | Table UI + CSV writer |
| DR-78 | MBD status panel and model annotations in views | P2 | Requires noshape MBD model; plane matching |
| DR-79 | Drawing sketch tools | P1 | Geom2d entities; Geom2dAPI_Interpolate; trim via Geom2dAPI_InterCurveCurve |
| DR-80 | Drawing sketch constraints | P2 | Reuse 2D sketch constraint solver with view geometry as fixed refs |
| DR-81 | Table framework (anchor, resize, cell panel, split, copy) | P0 | 2D table layout engine |
| DR-82 | General table and paste-from-spreadsheet | P1 | Table engine + clipboard TSV/HTML parsing |
| DR-83 | BOM table | P0 | Table bound to assembly BOM (flattened/top-level/multi-level) |
| DR-84 | Assembly BOM source model (columns, templates, subassembly behavior) | P1 | Assembly BOM data model |
| DR-85 | Hole table | P1 | Hole attributes + projected axis coordinates relative to origin |
| DR-86 | Revision table and revision callouts | P2 | Requires release/revision data (or manual rows) |
| DR-87 | Custom table (FeatureScript-defined) | P2 | Depends on noshape scripting/custom table API |
| DR-88 | Cut list table | P2 | Table bound to frame cut list data |
| DR-89 | Insert DXF or DWG into drawing | P1 | Rust `dxf` crate (MIT) for DXF; ODA/LibreDWG for DWG (licensing) |
| DR-90 | Importing a drawing (DWG/DXF -> drawing) | P2 | Same DXF/DWG libraries; convert to native 2D entities |
| DR-91 | Export drawing: dialog, PDF, SVG, PNG, JPEG | P0 | Vector PDF (pdf-writer/krilla/printpdf), SVG writer, tiny-skia raster |
| DR-92 | Export drawing: DXF, DWG, DWT | P1 | DXF via `dxf` crate (libdxfrw is GPL-2.0+); DWG/DWT via ODA SDK (commercial) or LibreDWG (GPL-3.0+) |
| DR-93 | Printing a drawing | P1 | PDF pipeline + OS print dialog |
| DR-94 | Drawing keyboard shortcuts | P1 | App keymap (customizable) |
| DR-95 | Drawing engine architecture (design suggestion) | P0 | HLRBRep (custom provenance extractor), BRepAlgoAPI_Cut sections, persistent-id associativity, 2D annotation model, vector PDF, DXF/DWG libs |
