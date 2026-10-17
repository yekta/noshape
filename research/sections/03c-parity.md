| SM-01 | Sheet metal master model (architecture) | P1 | Custom: hidden master TopoDS_Shell + SM attribute map; folded build via BRepPrimAPI_MakePrism/MakeRevol + BRepAlgoAPI_Fuse/Cut; flat via wall-graph unfold + gp_Trsf |
| SM-02 | Sheet metal model (sheetMetalStart) | P1 | Master from MakePrism (extrude) / MakeOffsetShape (convert) / face copy + Sewing (thicken); cylinders->sharp edges via GeomAPI_IntSS; then SM-01 build |
| SM-03 | Flange | P1 | Master edit: new planar wall (BRepBuilderAPI_MakeFace) + BEND attr; miter via BRepAlgoAPI_Section; rebuild |
| SM-04 | Tab | P2 | Coplanar 2D BRepAlgoAPI_Fuse/Cut on master wall + ShapeUpgrade_UnifySameDomain; BRepOffsetAPI_MakeOffset for clearance |
| SM-05 | Hem | P2 | Master edit adding walls/bends (straight/rolled/tear drop); rebuild |
| SM-06 | Jog | P2 | Two master bends: BRepAlgoAPI_Splitter imprint + rotate sub-graph |
| SM-07 | Bend | P2 | BRepProj_Projection of line, BRepAlgoAPI_Splitter, gp_Trsf rotation of moving region; BEND attr |
| SM-08 | Form | P2 | Regenerate form studio with thickness config, BRepBuilderAPI_Transform, BRepAlgoAPI_Fuse/Cut on folded solid; outline sketch in flat |
| SM-09 | Sheet metal form library | P2 | App data model (library index of form studios) |
| SM-10 | Make joint | P2 | GeomAPI_IntSS plane intersection, extend/trim faces, BRepBuilderAPI_Sewing into master, JOINT attr |
| SM-11 | Modify joint | P1 | Attribute edit on master + rebuild |
| SM-12 | Corner | P2 | Relief tool prisms + BRepAlgoAPI_Cut (flat and folded) |
| SM-13 | Corner break | P2 | 2D fillet/chamfer on flat outline (BRepFilletAPI_MakeFillet2d, ChFi2d_ChamferAPI) baked into master; custom conic/curvature |
| SM-14 | Bend relief | P2 | Relief tool prisms + BRepAlgoAPI_Cut |
| SM-15 | Rip | P2 | BRepAlgoAPI_Splitter of master wall by segment; RIP attr |
| SM-16 | Sheet metal loft | P2 | Ruled BRepOffsetAPI_ThruSections + custom planar faceting within chordal tolerance; facets->walls, edges->bends |
| SM-17 | Finish sheet metal model | P1 | App data model (deactivate; folded solid becomes regular body) |
| SM-18 | Sheet metal table and flat view | P1 | GPUI table + 2D viewport; edits route to Modify joint / owning feature; flat ops mapped back via flat->master transforms |
| SM-19 | Flat pattern DXF/DWG export | P1 | Custom DXF writer over flat body edges (BRepAdaptor_Curve, GCPnts_TangentialDeflection); bend lines from attrs |
| SM-20 | Recognize | P2 | Custom offset-face-pair detection (BRepExtrema_DistShapeShape), mid-surface extraction, GeomAPI_IntSS |
| SM-21 | Regular features on active sheet metal | P1 | Translate allowed ops (cut, hole, move face, face patterns, constant fillet/chamfer) into master edits via BRepAlgoAPI_Section/Cut |
| SM-22 | Bend calculation and flat-length math | P1 | Pure math (K factor / BA / BD / rolled) in flat builder |
| VAR-01 | Variable feature - Assigned | P0 | App data model: expression evaluator + context variable map |
| VAR-02 | Variable feature - Measured | P1 | BRepExtrema_DistShapeShape, BRepGProp::LinearProperties, BRepAdaptor radius |
| VAR-03 | Variable feature - From import (CSV) | P2 | App data model (CSV parse + regex) |
| VAR-04 | Variable naming, # references, scoping | P0 | App data model: ordered scope (config + studio + table + feature-list assignments) |
| VAR-05 | Expression language - grammar | P0 | Rust recursive-descent/Pratt parser + AST evaluator |
| VAR-06 | Expression units and dimensional rules | P0 | Unit dimension vectors, SI storage, conversion tables |
| VAR-07 | Expression built-in functions and constants | P0 | Rust f64 math library (trig in degrees for unitless) |
| VAR-08 | Variable autofill and inline creation | P1 | GPUI popover over scope query |
| VAR-09 | Variable dependency tracking | P1 | Reverse index from expression ASTs |
| VAR-10 | Variable table (Part Studio) | P0 | GPUI table + table variables evaluated before feature list |
| VAR-11 | Variable Studio | P1 | App data model (VariableStudio element) |
| VAR-12 | Inserting / referencing Variable Studios | P1 | Versioned cross-document references |
| VAR-13 | Query variable | P1 | Topology query engine (TopExp ancestors, convexity via BRepLProp_SLProps, GeomAdaptor types) + persistent names |
| VAR-14 | Boolean parameter as expression | P1 | App data model (parameter = literal or expression) |
| CFG-01 | Configuration model and terminology | P1 | App data model: inputs/tables, configuration key, per-config regen cache |
| CFG-02 | List input | P1 | App data model + GPUI table |
| CFG-03 | Checkbox input | P1 | App data model |
| CFG-04 | Configuration variable input | P1 | App data model; injected into variable scope |
| CFG-05 | Configure features mode | P1 | App data model + dialog overlay |
| CFG-06 | Configured / dynamic suppression | P1 | App data model |
| CFG-07 | Configuring sketch dimensions and text | P1 | App data model (sketch params) |
| CFG-08 | Configured properties | P1 | App data model; XCAFDoc_ColorTool on export |
| CFG-09 | Visibility conditions | P2 | Rule evaluation over ordered inputs |
| CFG-10 | Managing configurations | P2 | App data model + clipboard (JSON/TSV) |
| CFG-11 | Input propagation | P2 | Reference config bound to parent inputs |
| CFG-12 | Configuration selection and insertion | P1 | Reference = element + version + configuration key |
| CFG-13 | Variable Studio configurations | P2 | App data model |
| CFG-14 | Configuration limits and unconfigurable state | P2 | App data model (validation rules) |
| PSM-01 | Part Studio and regeneration model | P0 | Ordered feature replay into RegenContext; per-feature snapshots of TopoDS_Shape handles |
| PSM-02 | Persistent references / topological naming | P0 | Custom element map from BRepBuilderAPI_MakeShape Generated/Modified/IsDeleted (BRepTools_History) + fingerprints; alt TNaming |
| PSM-03 | Features list UI | P0 | GPUI virtualized tree |
| PSM-04 | Feature dialog conventions | P0 | GPUI dialogs; preview regen into temp context |
| PSM-05 | Rollback bar | P0 | Regenerate prefix up to rollback index |
| PSM-06 | Suppress / unsuppress features and folders | P0 | App data model |
| PSM-07 | Dynamic suppression | P1 | Boolean expression per feature |
| PSM-08 | Reorder features and dependency rules | P0 | Dependency graph validation on drop |
| PSM-09 | Rename features | P0 | App data model |
| PSM-10 | Feature folders | P1 | App data model (folder nodes) |
| PSM-11 | Feature list context menu | P0 | GPUI menus + commands |
| PSM-12 | Feature/parts list search and filters | P1 | Filter engine over feature/body indexes |
| PSM-13 | Feature failure states and error indicators | P0 | Feature status model + BRepCheck_Analyzer for faulty parts |
| PSM-14 | Pause regeneration and regeneration times | P2 | Dirty queue + per-feature timing |
| PSM-15 | Parts list | P0 | GPUI list by body kind (TopAbs type) |
| PSM-16 | Parts list context menu | P0 | GPUI menus |
| PSM-17 | Part properties (metadata) | P0 | App data model; XCAFDoc_ShapeTool names on export |
| PSM-18 | Materials and material libraries | P0 | App data model; XCAFDoc_MaterialTool/XCAFDoc_Material, XCAFDoc_VisMaterial |
| PSM-19 | Appearances | P1 | Renderer + BRepMesh_IncrementalMesh quality; XCAFDoc_ColorTool / XCAFDoc_VisMaterial on export |
| PSM-20 | Mass properties and mass override | P1 | BRepGProp Volume/Surface/LinearProperties, GProp_GProps COM/inertia |
| PSM-21 | Hide / show | P0 | Renderer/UI state |
| PSM-22 | Isolate / Make transparent | P1 | Renderer; BRepExtrema distance ordering |
| PSM-23 | Delete part (deleteBodies) | P0 | Remove bodies from context |
| PSM-24 | Composite part | P2 | App data model; TopoDS_Compound for export/mass |
| PSM-25 | Derived (importDerived) | P1 | Regenerate source (cached), BRepBuilderAPI_Copy + BRepBuilderAPI_Transform, prefixed names |
| PSM-26 | Mixed modeling (mesh bodies) | P2 | Poly_Triangulation faces; limited boolean support (custom) |
| PSM-27 | Custom features and extension model | P2 | Rust FeatureDef trait mirroring FS preconditions; future plugin API |
| PSM-28 | Custom tables | P2 | App extension point |
| PSM-29 | Cut list | P2 | BRepGProp/BRepExtrema for segment length/angles + composite |
| PSM-30 | Tag | P2 | App data model (attributes) |
| PSM-31 | Model-based definition (brief) | P2 | App data model; XCAFDoc_DimTolTool for AP242 PMI |
| PSM-32 | Part Studio tab operations | P1 | Document layer |
