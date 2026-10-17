# noshape — Functional Specification of Onshape CAD Features

Target: a native clone of Onshape's CAD functionality ("noshape": Rust + GPUI + OpenCascade/OCCT kernel).
Compiled 2026-10-17 from Onshape's public documentation. Scope is CAD only: sketching, Part Studio
features, assemblies/mates, drawings, the document/version model, variables/expressions/units,
configurations, derived/insert, import/export, measurement/mass properties and appearance.
Out of scope: simulation, Render Studio, PDM/release management, enterprise admin, CAM, PCB, App Store.

## 0. Conventions and sources

### 0.1 Sources
- **Onshape Help** (MadCap WebHelp build of 2026-10-09): every page under `https://cad.onshape.com/help/Content/{Assembly,Collaboration,Document,Drawing,File,Home,Library,PartStudio,Sketch,VariableStudio,View}/…` — 382 pages were fetched (via the help TOC `Data/Tocs/CombinedHelp*.js`) and converted to text.
- **FeatureScript Standard Library** source (MIT-licensed, PTC Inc.; public mirror `github.com/javawizard/onshape-std-library-mirror`, snapshot dated 2026-05-15) and the FeatureScript docs (`https://cad.onshape.com/FsDoc/`). Each built-in feature's `precondition` block is the actual definition of its dialog: parameter id, UI label (`"Name"`), type, enum, default, bounds, selection filter and conditional visibility. Where this spec quotes FS ids (e.g. `endBound`, `operationType`), they come from there.
- Where the help and the FS source disagree (usually because the help is older or simplified), the item says so.
- Statements not directly stated in the docs are marked **(inferred)**; proposals for noshape's own architecture are marked **(design suggestion)**.

### 0.2 ID scheme (for parity tracking)
| Prefix | Section | Scope |
|---|---|---|
| DOC- | 1 | Document model: documents, tabs, workspaces, versions, branches, merge, history/undo, linked documents, units, expressions |
| SKB-/SKE-/SKT-/SKC-/SKD- | 2 | Sketch: basics & solver / entities / tools / constraints / dimensions |
| PS- | 3A | Part Studio solid modeling, patterns, direct editing |
| SF-/CV-/RG-/FR- | 3B | Surfaces / curves / reference geometry (planes, mate connectors) / frames (weldments) |
| SM-/VAR-/CFG-/PSM- | 3C | Sheet metal / variables / configurations / Part Studio management & part data |
| AS- | 4 | Assemblies, mates, relations |
| DR- | 5 | Drawings |
| MT- | 6 | Measurement, view and analysis tools |
| IO- | 7 | Import / export |
| KB- | 8 | Keyboard shortcuts, mouse navigation, selection |

### 0.3 Priorities
- **P0 core** — needed for a usable MVP (sketch → extrude/revolve → fillet → assembly → drawing → STEP).
- **P1 important** — expected by professional users; schedule right after MVP.
- **P2 nice to have** — niche, advanced, or replaceable by workarounds.

### 0.4 Notation
- Bounds are given as `min .. max, default` and are taken from FS `*_BOUNDS` specs (stored internally in meters/radians; the default shown depends on document units, e.g. `NONNEGATIVE_LENGTH_BOUNDS` = 1e-5 m .. 500 m, default 25 mm metric / 1 in imperial).
- Common bound specs (from `valueBounds.fs`; metric default / imperial default):
  | Spec | Range | Default |
  |---|---|---|
  | `LENGTH_BOUNDS` | -500 m .. 500 m | 25 mm / 1 in |
  | `NONNEGATIVE_LENGTH_BOUNDS` | 1e-5 m .. 500 m | 25 mm / 1 in |
  | `NONNEGATIVE_ZERO_INCLUSIVE_LENGTH_BOUNDS` | 0 .. 500 m | 25 mm / 1 in |
  | `ZERO_DEFAULT_LENGTH_BOUNDS` | -500 m .. 500 m | 0 |
  | `BLEND_BOUNDS` (fillet/chamfer) | 1e-5 m .. 500 m | 5 mm / 0.2 in |
  | `SHELL_OFFSET_BOUNDS` | 1e-5 m .. 500 m | 2.5 mm / 0.1 in |
  | `PLANE_SIZE_BOUNDS` | 1e-5 m .. 500 m | 150 mm / 6 in |
  | `ANGLE_360_BOUNDS` | -1e5° .. 1e5° | 30° |
  | `ANGLE_360_FULL_DEFAULT_BOUNDS` | -1e5° .. 1e5° | 360° |
  | `ANGLE_360_ZERO_DEFAULT_BOUNDS` | -1e5° .. 1e5° | 0° |
  | `ANGLE_STRICT_180_BOUNDS` | 0 .. 179.9° | 30° |
  | `ANGLE_STRICT_90_BOUNDS` (draft) | 0 .. 89.9° | 3° |
  | `CHAMFER_ANGLE_BOUNDS` | 0.1 .. 179.9° | 45° |
  | `POSITIVE_COUNT_BOUNDS` | 1 .. 1e5 | 2 |
  | `PRIMARY_PATTERN_BOUNDS` / `CURVE_PATTERN_BOUNDS` | 1 .. 2500 | 2 |
  | `SECONDARY_PATTERN_BOUNDS` | 1 .. 2500 | 1 |
  | `CIRCULAR_PATTERN_BOUNDS` | 1 .. 2500 | 4 |
  | `SCALE_BOUNDS` | 1e-5 .. 1e5 | 1 |
  | `FILLET_RHO_BOUNDS` | 0 .. 0.99999 | 0.5 |
  | `K_FACTOR_BOUNDS` | 0 .. 1 | 0.45 |
- "Filter" = which entity types a selection field accepts (FS `Filter` annotation): `EntityType.{VERTEX,EDGE,FACE,BODY}`, `GeometryType.{LINE,CIRCLE,ARC,PLANE,CYLINDER,CONE,…}`, `BodyType.{SOLID,SHEET,WIRE,POINT,MATE_CONNECTOR}`, `SketchObject.YES/NO`, `ConstructionObject.YES/NO`, `ActiveSheetMetal`, etc.
- OCCT class names refer to OpenCascade 7.8/7.9.

### 0.5 Architecture notes common to all sections (design suggestion)
- **Regeneration model:** a Part Studio is an ordered feature list replayed from scratch (or from the first changed feature, with caching) into a geometric context. Each feature = (type id, parameter map, suppression state). Parameters are typed values (expression strings with units, enums, booleans, queries). This mirrors Onshape exactly and makes FS precondition blocks directly translatable into Rust feature-parameter schemas.
- **References:** selections are stored as *queries* resolved against the current context (Onshape: transient queries + persistent "deterministic ids" derived from feature id + creation history). In OCCT, build persistent naming from `BRepBuilderAPI_MakeShape::Generated/Modified/IsDeleted` / `BRepTools_History` histories (or OCAF `TNaming`), keyed by feature id + generating sketch entity/edge id.
- **Kernel tolerance:** OCCT default `Precision::Confusion()` = 1e-7 mm-model units; Onshape's smallest length bound is 1e-5 m (0.01 mm) — use mm internally and keep 1e-7 confusion.
- **Bodies:** Onshape distinguishes solid parts, surfaces (sheet bodies), wire bodies (curves), point bodies, mate connectors, and sketch bodies. noshape should model these explicitly (TopoDS_Solid/TopoDS_Shell+Face/TopoDS_Wire/TopoDS_Vertex + metadata).

