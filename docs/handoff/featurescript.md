# FeatureScript (agent "fs")

Goal: Feature Studios take real FeatureScript and behave like Onshape's; custom features written in
FeatureScript run in Part Studios. Replaces the Rhai custom features (`crates/model/src/custom/`).

## Status (19 Oct ~18:00)

Build only in /root/target-engine (engine group, TEAM-RULES "Build"); prefer `cargo check` / single tests. Green:
`cargo test -p noshape-featurescript` (29), `cargo test -p noshape-model --test custom_features` (14 + 1
ignored y_threads, kernel sweep), clippy `-p noshape-model -p noshape-featurescript --all-targets`.

Done since the blockers:
- errors3 applied (dom/fs-errors3.txt) and pinned by `fs_errors_match_onshape` (runtime texts/positions,
  printed values) and `notices_match_onshape` (compile/parse warnings).
- `@evaluateSpline` back in (crates/featurescript/src/spline.rs, NURBS + derivatives, 2 tests).
- **Naming fix (shared core, naming.rs):** a merged face keeps the name of its first parent by
  (argument, face index) instead of the smallest name. The smallest name depended on random feature ids,
  so a flush Extrude-add randomly gave the cube's side faces the boss's identity and broke
  qCreatedBy(cube) for ~half of documents. Model lib suite (208) still green.
- Std harness tests (tests/fs/std_harness.fs, ops_harness.fs): extrude, fillet, chamfer, linear/circular
  patterns, mirror, shell, draft, boolean, split, skText, delete + op*/ev* value checks all pass from a
  custom feature. Fixes on the way: merged-face naming, `number * Matrix`, OWNED_BY_PART only for bodies
  (evLength of an edge summed the whole body), split pieces "created by" the original's creator.
- New natives: skText (Liberation fonts by family/style), skConicSegment, opSplitFace, opExtractSurface,
  opFlipOrientation, opCreateBSplineCurve, opModifyFillet/opEnclose (builtin adapter), pattern transforms.
- Dialog generator: computed keys (`definition[KEY]`), predicates with literal arguments
  (`isProfile(profile, "u")`), FeatureList, **lookup tables** ("Lookup Table" annotation → run-time table in
  hole_tables `register_dynamic_table`, dialog unchanged; the feature gets a LookupTablePath). The dialog
  of every std feature is compared with our built-in schemas (`std_feature_dialogs_match_builtin_schemas`,
  44/65 identical and pinned; deviations listed below for the schema owners).
- `replace()` keeps backslashes in the replacement (std's lookupTableEvaluate depends on it).
- After the reboot (main 756ef78, kern's sweep fix): `real_onshape_feature_y_threads` passes (un-ignored by
  the lead). More: tests/fs/std_harness2.fs (revolve, cPlane, thicken, transform copy, moveFace, deleteFace,
  offsetSurface, loft, sweep), ops harness adds opCreateCompositePart and clusterBodies.
- New natives: getTolerantParameterIds / getParameterToleranceInfo / validateToleranceSchema (no
  tolerances on parameter values yet: empty / NONE / []), opCreateCompositePart, opNameEntity (builtin
  adapter; the built-in nameEntity is a no-op), clusterBodies (counts + measure + inertia invariants).
- opOffsetFace keeps the part's face names (`keep_modified_names`; see the naming note for psb).
- **Flaky qCreatedBy(op, BODY) fixed (~40% of harness runs failed some check):** bodies created by an
  operation got index bits from FNV's high bits, which barely change between sibling ids (`id + "a"` /
  `id + "b"`), so ops collided and `body_creator` picked one by HashMap order. Now the bits come from a
  splitmix64 finalizer (`marker_bits`) and each created body's op is recorded exactly
  (`CtxState::body_ops`). 25/25 + 10/10 repeated runs green. **Lead:** custom-feature body ids changed
  once with this (stored references to custom-feature bodies made since 756ef78 may need reselecting).
- More ops: opSplitEdges (vertices as split tools; arc-length parameters), opExtendSheetBody (Move
  boundary, blind), opFillSurface (Fill), opBoundarySurface (profiles; derivative conditions not yet),
  opIntersectFaces (Intersection curve), evApproximateBSplineCurve (exact, or cubic non-rational fit).
- **Reference parameters (live dom/fs-refparams*.txt, part-studio/480–488), done:** `ParamKind::Reference`
  (params.rs `ReferenceKind`), values from file tabs (custom/reference.rs: csvData typed cells, image
  size/mediaType, jsonData, textData, dataId + BuiltIn handles; unset `{}`), pinned by
  `reference_parameters_read_file_tabs` (17 green). Dialog UI: crates/app/src/fs_reference.rs: the labelled
  214×27 field (type icon, "Select CSV file…" at 60%, the chosen tab's name, blue border + 500 weight while its
  panel is open) and the 300×559 "Select …" panel beside the dialog (title, Current/Other documents tabs,
  document row with Create version… / Version graph, search "Search CSV files"…, 40 px rows, Part Studio
  rows expand to their parts, "Import…" for CSV/image adds a file tab and picks it). Stored value: the
  tab id, or `tab#body` for one part of a Part Studio. Hooks: feature_dialog.rs `ParamKind::Reference`
  arm, `ref_*` fields, `DialogEvent::ImportReference`; part_studio/mod.rs `set_reference_sources`,
  `import_reference_file`. Verified on :108 (/tmp/fs/s10–s18, d4–d5): the Monitor prints the live lines.
  **Fix on the way:** regenerations without a provider (assembly eval.rs 1551/1578, thumbnails, sketch
  regen) used the fallback provider, which had no file tabs, and overwrote the Monitor output with `{}`.
  The fallback (`provider.rs Latest`) now serves the latest document's file tabs, and file changes are part
  of custom features' regen version.
  **Round 2 (19 Oct evening):** italic search placeholder (overlay, like the feature-list filter);
  70×40 Part Studio / part pictures in the panel (`fs_reference::render_thumbnails`, each other Part
  Studio regenerated and rendered offscreen at 140×80 when a PartStudioData dialog opens);
  `PartStudioData` value: `buildFunction` (a `Func::Native` "buildPartStudio" bound to the tab: a new
  Context holding the Part Studio's final state via the Derived provider, which runs on the feature's
  thread through `host::set_derived`), `partQuery` (transient ids `tab:body` registered in the built
  context), `configuration` / `configurationData` `{}`; `instantiate` / `addInstance` work
  (`part_studio_reference_instantiates_parts`). Printed object numbers restart per feature run, boxes and
  builtins counted separately, Context = `BuiltIn: 0` (value.rs `shown_number`; serials keep identity).
  **Round 3 (live dom/fs-refpanel2-*, part-studio/600–629):** Other documents tab (filter + "Search or
  paste URL" + 🔍, filters Recent / Owned by me / Recently opened / Created by me / Shared with me / Public,
  documents with picture, name and latest version, grey when they have nothing of the kind; opening one
  shows it at its latest version, never its workspace); Version graph (legend, Main / versions / Start on
  a blue line; Main selectable only in this document); the blue "N changes since the last version. /
  Create a version in <doc>." box; Create version… (this document: history_panel::open_create_version;
  another: opens its window); picking a file closes the panel (a Part Studio's stays open for parts);
  field tooltip "Table: fsrefs.csv"; switching to Other documents clears the field (✕ restores).
  Values from other documents / versions are `ext:<microversion>|<tab>|<path>` (`#part` after the tab),
  read through `DerivedProvider::file` (new default method; app's DocStudios implements it) and
  `DocStudios::context`. "No CSV files available." etc. per kind.
  **Round 4 (live dom/fs-refcfg*, fs-partquery):** configured Part Studios: the panel shows the inputs under
  an expanded configured studio (OsSelect / checkbox / fields, 40 px indent) + Generate (pale until a
  change) and a "Set to last viewed configuration" ▾ button on the row (release_dialog::last_viewed_config);
  the value stores the configuration after a tab (`key[#part]\tinput=value;…`). PartStudioData:
  `configuration` / `configurationData` keyed by the inputs' FeatureScript ids (= names, sanitized), list
  values = option names, quantities in meters, `referenceType` WORKSPACE / VERSION, `buildFunction(cfg)`
  regenerates in that configuration (missing inputs default; `DerivedProvider::context_configured`).
  partQuery: whole studio = UNION › UNION › BODY EVERYTHING; a part made by a sweep-like feature from sketch
  edges = Onshape's SWEPT_BODY query (`<feature>.opExtrude`, originals `<sketch>.wireOp` + sketch entity
  ids; new `DerivedProvider::feature_type`), resolved by query.rs `historical` "SWEPT_BODY"; other parts
  fall back to transient queries. Pinned by `part_studio_reference_configurations` and
  `part_studio_reference_swept_body_query`.
  **Still open:** Onshape's sketch entity ids are its own (ours print our entity numbers); external
  (`ext:`) studios ignore configurations; the panel's Generate "Generating preview" pictures.

**Rhai:** it is already gone in the working tree — the previous "fs" deleted `custom/{api,compile,eval,schema}.rs`
and dropped `rhai` from Cargo.toml before I took over; the FeatureScript path replaced it everywhere
(FeatureStudio content, demo, standard content, app hooks). Restoring Rhai side by side would mean bringing
those files back from git HEAD and re-adding a language switch; I did not do that. **Lead: decide** whether
FeatureScript replacing Rhai now is acceptable (tests were 8/8 at the previous agent's last run).

## Std operations backed by built-in evaluators (std's own feature code runs)

The std features bodyDraft, constrainedSurface, fitSpline, isocline, projectCurves and wrap now run std's
own code; their missing operations are natives (host/builtin.rs `BuiltinOp`: op fields → the built-in
feature's parameters, `renames`, `vectors` = Vector fields passed as a temporary plane with that normal,
`defaults`): opCreateIsocline / opSplitByIsocline (isocline), opBodyDraft (bodyDraft), opSplineThroughEdges
(fitSpline EDGES), opDropCurve (projectCurves CURVE_TO_FACE), opFaceBlend (renames), opRuledSurface (`adjust`
hook: width sign, ANGLE_FROM_FACE → Tangent + angle, vector types; returns frames at manipulatorPositions;
vertexOverrides ignored); plus opConstrainedSurface
and opWrap (WrapSurface plane/cylinder/cone/face + anchors) written against the kernel directly. Also
@constructPaths, @approximateSpline. All in tests/fs/ops_harness.fs. (`interp::override_global` remains
available but is unused.)

## Feature Studio imports of other tabs (live dom/fs-import-*, behaviours.md "Import button")

`import(path : "<tab id>", version : "…")` of a file tab exports `BLOB_DATA` (the reference-parameter map)
and `processedForeingDataInfo` (sic, a BuiltIn); of a Part Studio exports `build(configuration)` (prints
`<function 6>` like live); Variable Studio imports compile but export nothing yet (not captured). Two
un-namespaced imports that export different constants of one name → "Duplicate top level symbol name 'X'"
(reported at our import statement; Onshape lists it under each imported module). `skImage` places an
imported image keeping its proportions (BLOB_DATA's dataId finds the bytes). Pinned by
`imports_of_files_and_part_studios`. The Import panel itself is ui2's (fs_import.rs); its paths resolve here:
same document = tab id; other documents / versions = `<document id>/<version microversion>/<tab id>`
(document id = first microversion), turned into an ExternalRef by app `derived::import_ref` (documents
found by scanning the local library once per unknown id) and loaded through
`custom::set_external_tab_loader` (registered in main.rs). The reference panel is now its own entity
(select_item.rs `SelectItemPanel`, events Picked / Close / CreateVersion / ImportFile; asb uses its
`document_versions` / `changes_since` for drawing custom tables).

## Custom tables (`defineTable`) — API for asb (drawings) and the Part Studio Custom tables panel

- Compiling a Feature Studio also collects its custom tables: `CompiledStudio::tables` (`CustomTableDef`:
  `name` = exported constant, `display_name` = "Table Type Name", `description`, `params`).
  `DocumentScripts::from_document(doc).studios()` lists them per studio.
- Run one: `noshape_model::custom::table::evaluate_table(&*scripts, &studio_tab.to_string(), name,
  &regen_result.context, &units)` → `TableResult { tables: Result<Vec<CustomTable>, String>, output, notices }`.
  `CustomTable { title, title_suffix, columns: [TableColumn { id, name, alignment }], rows: [TableRow {
  cells: [(column id, TableCell { text, error, info })], entities: [Reference] }], entities }`.
  Cell text: strings as is, numbers / lengths / angles in the document units (`format_quantity`),
  `valueWithUnitsAndPrecision` with its precision, `templateString` `#key`s filled in. Pinned by
  `custom_tables_evaluate`.
- Live (behaviours.md "Custom tables (defineTable)", dom/fs-tables-*): cells "0.333", "40 mm",
  "25.4000 mm" (precision keeps zeros), "30 deg", "1290.32 mm²", "1500000000 mm³", missing → "Not set"
  (`TableRow::text`), error cells "?" pink with tooltip, info cells "0 ⓘ"; TableArray shows only non-empty
  tables (filtered in the model); a table without a precondition is only warned about ("Nonconforming
  table function 'x': Missing precondition") and not offered; importing geometry.fs in a studio that
  defines tables warns ("Importing geometry.fs when defining tables will likely hurt performance; …").
  All pinned in `custom_tables_evaluate`. Panel look, selector groups ("My custom tables" from versions /
  "Workspace custom tables"), Add custom tables popup are in that capture for whoever builds the panel.
  When a table throws, the panel shows `table::TABLE_ERRORS` and the notices pane a group
  "<table> (<Part Studio>)" with error, stack and println — `TableResult` carries notices/output; the
  group needs a caller (no panel yet). Table parameters are collected; tables run with `{}` for now.
- asb built the callers (20 Oct): Part Studio panel app/part_studio/custom_tables.rs, drawing custom tables
  (`TableKind::Custom`, app/drawing_custom_table.rs), the notices group via `custom::run::notice_entries`
  (factored out of `monitor`) and app/custom_tables.rs `set_notices`; see docs/handoff/drawing-behaviour.md.

## Design

- **Language core: new crate `crates/featurescript` (`noshape-featurescript`)**, no OCCT dependency, so its
  tests build fast. **Cargo.lock change for the lead:** new workspace member (path crate, deps: parking_lot, log,
  serde_json, regex, all already in the lock).
  - `lexer.rs`, `parser.rs` (recursive descent, ANTLR-style error wording: "extraneous input 'x' expecting
    {...}", "missing ';' at '}'", "mismatched input", "no viable alternative at input"), `ast.rs`.
  - `compile.rs`: modules, imports, symbol tables (functions overload across modules, `export import`,
    `NS::import`), lowering to `ir.rs` (locals → frame slots, closures capture by value, module symbols →
    `GlobalCell` / `OverloadSet`, `@natives` → fn pointers, operator overloads attached per module).
  - `interp.rs`: evaluator (value stack, exceptions = Rust errors carrying the thrown value, notices for every
    raised exception outside `try silent`, predicates/preconditions, overload resolution by specificity,
    lazy thread-safe constant init, deadline).
  - `value.rs`: values (tags in declaration order, enums as `Enum(tag, ordinal)`, COW arrays/maps, boxes,
    builtins), Onshape's total order (map keys) and the `~` string conversion.
  - `natives.rs`: core `@` builtins (math, strings, regex, JSON, containers, matrices, print/report).
- **The Onshape FeatureScript Standard Library runs unmodified.** Its 277 sources (v3095.0) are embedded from
  `crates/featurescript/std/` (build.rs → `STD_SOURCES`). License: every file says "distributed under the MIT
  License ... Copyright (c) 2013-Present PTC Inc." (std/LICENSE reproduces it). 14 files lack that header
  (holetables.gen.fs, isocline.fs, projectCurves.fs, bodyDraft.fs, tabReferences.fs, fittolerancetables.gen.fs,
  bridgingCurve.fs, faceIntersection.fs, constrainedSurface.fs, lookupTablePath.fs, compositeCurve.fs,
  routingCurve.fs, partStudio.fs, isoparametricCurve.fs); they are part of the same library document whose
  LICENSE tab is MIT. Asked live for the exact LICENSE tab text (REQUESTS.md "fs (01:55)"). **Lead: confirm.**
- The host (model crate) registers modeling natives (`@opExtrude`, `@evaluateQuery`, ...) with
  `interp::register_natives`; missing ones raise "@name is not supported" only when called.

## State

- Stage 1 (lexer + parser): done. All 277 std modules parse. Tests: `cargo test -p noshape-featurescript`.
- Stage 2 (evaluator): done for the language core; geometry.fs (all 260 modules) compiles with no errors in
  ~0.7 s (debug); the user's own y-ribs.fs / y-threads.fs (/root/onshape-featurescripts) compile.
- Stage 3 (modeling natives, `crates/model/src/custom/host/`): written, compiles; tests being run.
  - `mod.rs`: `FsContext` builtin (wraps the regen `Context`), operation markers + renaming (see module docs),
    transient ids, `run_op`, std tag/enum lookup, ErrorStringEnum texts (from the enum's comments).
  - `query.rs`: every QueryType (+ historical SKETCH_ENTITY / IMPRINT queries), `Ent` entity model.
  - `evaluate.rs`: ev* natives. `ops.rs`: op* natives. `sketch.rs`: newSketch / sk* / skSolve / constraints.
  - `status.rs`: start/end/abortFeature, feature status, variables, attributes, debug entities, versions.
- Rhai is removed: `custom/{api,compile,eval,schema}.rs` deleted, `rhai` dropped from Cargo.toml (Cargo.lock
  change for the lead). New: `custom/studio.rs` (compile, features, dialog schemas from preconditions incl.
  predicates, if/else visibility, enums with "Name"/"Hidden", bound specs, UIHints), `custom/run.rs`
  (evaluation on a 256 MB-stack thread, 60 s limit, status → feature error, notices/println per feature
  type in `run::outputs()`), `custom/provider.rs` (imports between Feature Studios by tab id).
- Demo Feature Studio (Flange, Bolt circle) and the standard content fasteners (assembly/standard.rs) are
  rewritten in FeatureScript; tests/custom_features.rs rewritten for FeatureScript.
- Stages 4–5 partly: dialog schemas done; UI wiring (notices, Monitor, symbol search) written in session 2, not yet verified in the app.

## Session 2 (19 Oct, from 04:30 UTC; new "fs" after quota loss)

- clippy.toml is gone (deleted; lead: don't commit it). The mutable-key lint is fixed structurally:
  `value::Shared<T>` holds boxes and functions behind a trait object, so `Value` has no visible interior
  mutability. `cargo clippy -p noshape-featurescript --all-targets -- -D warnings` is clean without it.
- std/LICENSE now quotes the Standard Library's LICENSE tab verbatim (live dom/fs-std-license.txt): MIT.
- Compile notices match live dom/fs-parse-errors.txt (test `notices_match_onshape`): TOP_SEMI wording,
  "Missing initializer expression", ANTLR no-viable input text from the operator, "Variable x not found."
  (warning when assigned, error when read), "Unused declaration: f" warnings, "Missing FeatureScript
  version" / version-mismatch errors, "Variable m set but not used" (at the body `{`, listed first),
  "Function f with n argument(s) not found" (compile warning; runtime error at the enclosing call).
  `CompileError.level` (`Level::{Error, Warning, Info}`), `CompiledModule.warnings` (non-errors);
  studio.rs maps them to `Severity::{Error, Warning, Info}`.
- Runtime texts from live (fs-probe-output.txt): "Attempt to dereference non-container undefined",
  "Array index 3 out of bounds.", "Can not add ValueWithUnits (map) and number.", "@sqrt: Invalid number
  (NaN)"; `catch (e)` of a language error binds "Execution error"; `~` prints tags as `Name : value`;
  expressions are located at their start (binary at the left operand, `a[3]` at `a`).
- Host fixes: wire-body edges are found by qCreatedBy (op_marker follows wrapped wire names);
  opSweep retries Frenet/Discrete frames when corrected Frenet fails (helices); bodies merged by
  opBoolean are also "created by" the boolean and the merged bodies' creators (std query.fs doc).
- Asked live (REQUESTS.md "fs (05:45)", source requests-fs/fs-errors3.fs) for ~35 more runtime texts,
  number printing, timers and a Monitor screenshot. **When it lands: update interp.rs texts + tests.**
- Notice levels: `compile::Level {Error, Warning, Info}` ("set but not used" is Info, ⓘ icon
  `assets/icons/fs-info.svg`), model `Severity::Info`, app `Notice.info`.
- Monitor (FeatureScript notices panel, Part Studio section): model `custom::run::monitor(doc, ps_tab)`
  → `MonitorEntry {message, severity, at, rows: (line:col, "module (function)")}` + printed text;
  outputs are per Part Studio feature (`run::output_of(FeatureId)`, `output_generation()`).
  fs_notices.rs renders entries, the println lines, then the Result line. Anonymous functions are named
  like Onshape's stacks: `const fsProbe` (const initializer) or the enclosing named function.
- Custom feature out of bounds → "A parameter value is not valid." with the field marked (live). The
  field hover "Enter a value between 1.00 and 100. Current value is 0.5." (3 significant digits) is not
  done: it is the dialog's (ui) and built-ins use the same message in Onshape (regen `check_bounds`,
  pinned by qa_regressions.rs) → lead decides.
- `host::run_op_with(params)`: op natives can run a built-in feature evaluator with translated params
  (opOffsetFace → direct::move_face OFFSET, opReplaceFace → direct::replace_face); opMoveFace uses
  `direct::move_rigid` (made pub(crate), psb's file). startTimer/printTimer (format provisional).
- Ctrl+Shift+F symbol mode now uses `lexer::symbol_occurrences` (identifier tokens only: no strings,
  comments, `.field` names or map keys) for results, highlights and Rename.
- New natives: computeLinear/Circular/CurvePatternTransforms (host/pattern.rs, ported from std's
  pre-builtin FeatureScript + skipped instances), opOffsetFace, opReplaceFace, opMoveFace, startTimer /
  printTimer; plus the unverified ones listed under PAUSED. Error codes passed to run_op are all real
  ErrorStringEnum members (checked against errorstringenum.gen.fs).

## Next

1. (done) blockers.
2. Apply the live part 3 answers (dom/fs-errors3.txt): runtime texts, "Non-numeric operand" /
   "Non-boolean operand" compile warnings (studio only, not in the Part Studio group), precondition failure
   text = annotation 'Message' or the statement's source, "Maximum call stack depth exceeded." (~1020
   frames), caught native errors are regenError maps (`{ faultyParameters : [ face ] , message :
   CANNOT_RESOLVE_ENTITIES }`) shown as "@evPlane: CANNOT_RESOLVE_ENTITIES", Java-style number printing
   (`1.23456789123e+8`, `1e-7`, `-0`), `Timer "t1": 0 ms`, `@printTimer: Unknown timer: "nope"`,
   Box/function/context/id printing, Monitor order = user modules by line then std by line. Original list (REQUESTS.md "fs (05:45)", source
   live/requests-fs/fs-errors3.fs): runtime error texts for ~35 cases (`BinOp::verb` beyond "add" is a guess,
   "Attempt to dereference non-container <type>" wording for non-undefined values, overload-mismatch text),
   number printing (1e21, 1e-7, -0, units), printTimer format, println of thrown values, Monitor styling
   (current Monitor rows reuse the Feature Studio notice row style; println lines are monospace #333).
   Notice order in the Monitor is grouped by the module the error occurs in (fits the one capture).
3. Remaining std natives (68, `comm` of std `@calls` vs registered; commands in this file's history):
   approximateSpline clusterBodies constructPaths evApproximateBSplineCurve/Surface evCornerType
   evEdge/FaceCurvatureDerivatives evFaults evMaxPathDeviation evMaxTolerance evMeshPoints evOffsetDetection
   evPointsDeviation evRuledSurfaceBases evSheetMetal* evTessellatedLoftMatches evTolerances getHoleAttributes
   getParameterToleranceInfo getTolerantParameterIds opBodyDraft opBooleanedPattern opBoundarySurface
   opConstrainedSurface opCreateBSplineSurface opCreateCompositePart opCreateCurvesOnFace opCreateIsocline
   opCreateOutline opDerip opDropCurve opEdgeChange opEditCurve opExtendSheetBody opFaceBlend opFillSurface
   opFullRoundFillet opHole opImportForeign opIntersectFaces opModifyCompositePart opMoveCurveBoundary
   opNameEntity opOffsetCurveOnFace opOffsetWire opRuledSurface opSMFlatOperation opSplineThroughEdges
   opSplitByIsocline opSplitBySelfShadow opSplitEdges opSplitFace opTessellatedLoft opWrap
   sheetMetalApplyInFlat skImage skText unpackQuery updateSheetMetalGeometry validateToleranceSchema
   (+ evaluateSpline parked). Many map onto built-in features with `builtin.rs::builtin_op` — but the op
   definition fields often differ from the feature's param ids (checked: opModifyFillet and opEnclose match;
   opFillSurface, opFaceBlend, opBoundarySurface, opRuledSurface, opExtendSheetBody do NOT) → add a field
   rename/translation per op before registering them.
4. (done 20 Oct) opExtractSurface `tangentPropagation` (smooth edges), skConicSegment low/highParameter
   (blossomed sub-conic), opMoveFace offsets planar faces translated along their normal (rigid otherwise).
   Also new: opEditCurve, evMaxPathDeviation, opModifyCompositePart (open composites; closed ones can only
   grow), evEdgeCurvatureDerivatives (central differences of the exact tangent), evPointsDeviation,
   evMeshPoints (+ opConstrainedSurface fits mesh points, interpolates picked ones), evRuledSurfaceBases,
   opFullRoundFillet (Fillet FULL_ROUND), opCreateCurvesOnFace (Isoparametric curve; DIR*_ISO curves under
   `id + name`). **Fix:** host `rename_created` dropped `Body::composite`, so composites made from
   FeatureScript were plain bodies. The panel lists a Part Studio's sketches after its parts (picture seen
   normal to the sketch plane; picking one gives a Sketch query). kern has the B-spline surface request
   (opCreateBSplineSurface). Still missing (34): sheet metal internals, tolerances, opHole, opWrap-free
   ones like opOffsetWire / opEdgeChange / opMoveCurveBoundary / opCreateOutline / opBooleanedPattern /
   opTessellatedLoft / opDerip / opImportForeign / opSplitBySelfShadow, evFaceCurvatureDerivatives,
   evOffsetDetection, evFaults, unpackQuery (Onshape's compressed query format is unknown).
   **20 Oct, round 5:** `@opHole` native (host/hole.rs): hole cylinder entrances/exits by rays on the axis
   and around the largest profile radius, profiles placed against AXIS_POINT / TARGET_START /
   LAST_TARGET_START / LAST_TARGET_END / UP_TO_NEXT / UP_TO_ENTITY / LAST_TARGET_START_IN_DEPTH (matched
   profiles: root start, *_END root end), identical neighbours collapsed, targetMustDifferFromPrevious /
   notApplicableForFirstTarget, subtract / keepTools / targetsToExcludeFromSubtraction; tool faces named
   `Side(Index(hole << 16 | faceIndex))` so `qOpHoleFace` / `qOpHoleProfile` (by name, by identity) work
   (CtxState.holes). Guesses to confirm with live (REQUESTS "fs (09:10)", requests-fs/fs-ophole.fs): radius
   of the "hole cylinder", holeDepth origin, errors when every hole fails (HOLE_TARGETS_DO_NOT_DIFFER /
   HOLE_NO_HITS). `@opOffsetWire` (ops.rs) = the Thin wall builder (thin.rs) with offset1/offset2/flip;
   open chains flipped when `normal` opposes the builder's plane normal; makeRegions → sheet bodies, else
   wire bodies of the offset curves. Live probes pending: evOffsetDetection + packed queries ("fs (08:00)"),
   sheet metal definition bodies + SMAttributes ("fs (09:40)", requests-fs/fs-smprobe.fs) before
   updateSheetMetalGeometry & co.
   **Round 5b (live answers in):** opHole matched to dom/fs-ophole.txt: depth extremes on the smallest
   (shaft) radius, each reference's root range on the largest radius of its profiles, entrances clamped at
   0, holeDepth = 0 when the last profile references AXIS_POINT / TARGET_START else (last profile with a
   radius) − first entrance, profile edges = edges between the two hole faces around the profile (cap
   gone → no edge), errors carry faultyParameters [targets] (`code_error_faulty`). Open: live 6b
   (LAST_TARGET_START_IN_DEPTH + notApplicableForFirstTarget, single part) threw HOLE_NO_HITS, probably the
   probe's tangent 2 mm cylinder; we skip the profiles (hole.fs relies on that).
   `@evOffsetDetection` (host/offsets.rs) from dom/fs-offset-detect.txt: thickness = smallest offset pair of
   the first body; every body's pairs at that thickness, connected pairs grouped, side0 = earlier face.
   `@unpackQuery` (host/packed.rs) — format decoded from Show code (dom/fs-packed-query.txt): `%`+typed
   values (M/A/S/D/F/T, B/C type names, R value refs, string pieces `<len>`/`-<ref>`), `&<hexlen>$base64(zlib)`;
   root Id prefixes every Id. DUMMY queries for Top/Front/Right/Origin resolve. Unknown: number format
   beyond `D1`. New dep: miniz_oxide in crates/model. **FS language:** `?:` is left-associative (live) —
   parser fixed, error text "First operand of logical or conditional operator must be boolean".
   Sheet metal: the definition body is hidden from qEverything; part entities link to it by association
   attributes (dom/fs-smprobe-*.txt). Probe 2 for the definition body + SMAttributes: REQUESTS "fs (10:30)".
   **App (qa4):** Feature Studio shortcuts ctrl/cmd+shift+O (Module outline), ctrl/cmd+[ / ] (Back / Forward
   reference) bound, also over the code editor (kit's outdent/indent give way, as Onshape's are locked);
   verified on :108. Module outline popover was drawn at the window's top-left (anchor cell recreated each
   render) — fixed. "New feature" + ctrl+z could NOT be reproduced (focused / unfocused editor, new studio,
   toolbar Undo: one step removes the block); live ground truth requested ("fs (10:50)"). Back/forward only
   record outline / notices jumps; Onshape's ctrl+click go-to-definition is still missing.
5. Verify in the app (Xvfb :108): Feature Studio editor notices (error/warning/info), FeatureScript notices
   panel with a monitored Part Studio running the FS probe (compare live part-studio/405, document/75),
   Ctrl+Shift+F symbol search/rename. Side-by-side screenshots for the lead.
6. Field hover text for out-of-bounds values ("Enter a value between 1.00 and 100. Current value is 0.5.")
   in the feature dialog (ui's file) — lead decision since built-ins share it (see Session 2).

## App verification (19 Oct ~12:00, binary /tmp/fs/noshape, display :108)

- Feature Studio tab looks like Onshape's (toolbar, colors, gutter). Committing a studio with
  `function f() { x = 1; }` lists "Variable x not found." 3:16 and "Unused declaration: f" 3:10 with ⚠
  icons under the studio in the FeatureScript notices panel — identical to live case 6
  (/tmp/fs/shot4.png). Hovering `x` shows the message.
- **For ui2 (editor rendering):** squiggles are not drawn at all (errors or warnings), although the
  diagnostics are set (hover finds them; crates/app/src/feature_studio.rs `recompile`). And the hover
  popover has no background (text overlaps the line above, /tmp/fs/shot7-crop.png). Onshape: 4 px wave
  PNG squiggles and an Ace tooltip (live document/fs-squiggles-zoom8x.png, fs-gutter-error-hover).

## Naming issue for psb (found by tests/fs/std_harness2.fs)

After a Move face **offset** (regen/features/direct.rs `move_face`), *every* face of the part (OCCT's
offset history reports them all as Generated) is renamed `Offset(parent)` under the Move face feature, so
"created by the part's feature" no longer holds for any of them. In Onshape a direct edit modifies faces,
it doesn't create them (std query.fs qCreatedBy doc). FeatureScript's opOffsetFace now restores the parent
names afterwards (`ops.rs keep_modified_names`); the built-in Move face still renames (psb: consider the
same, carefully, since stored references use these names).

## Built-in schemas that deviate from the std (for psb / whoever owns features/*.rs)

`cargo test -p noshape-model --test custom_features std_feature_dialogs -- --nocapture` prints a diff of
every std feature's dialog (generated from Onshape's own precondition = ground truth) against our built-in
schema. 44 of 65 are identical (pinned in the test). Built-in deviations found (Onshape's std is right):
- Enum member ids: editCurve planeReference `YZ/XZ/XY` (std `YZPLANE/XZPLANE/XYPLANE`), editPointMode
  `UVN_POLYGON` (std `UVN_CONTROL_POLYGON`); isoparametricCurve `U/V` (std `U_DIRECTION/V_DIRECTION`);
  offsetCurveOnFace scope `OFFSET_EXTEND(_SPLIT)` (std `OFFSET_AND_EXTEND`, `OFFSET_EXTEND_AND_SPLIT`);
  queryVariable `EDGES_AND_FACES/EDGES/FACES/VERTICES` (std `ALL/EDGE/FACE/VERTEX`); routingCurve
  `MULTIPLE`, `ADD_POINTS`, `ORTHOGONAL_PATH` (std `MULTI`, `ADD`, `ORTHOGONAL`); ruledSurface `NONE`
  (std `NO_CORNER`). Enum order: importForeign/routingCurve units (std: Centimeter, Foot, Inch,
  Millimeter, Meter, Yard).
- Labels: bridgingCurve "Match flow" (std "Match flow (G3)"); mutualTrim "First/Second surface" (std
  "First/Second set of surfaces", and no max-1 pick limit).
- Defaults/bounds: fillet radii min 1e-5 m (BLEND_BOUNDS), edgeParameter 0.001..0.999; helix
  height/pitch/revolutions min 1e-5; importForeign importAppearances default false; ruledSurface
  useCubicInterpolation default false; editCurve degree 0..15, CPs/spans 0..99; routingCurve targetLength.
- Extra std params missing in built-ins: extrude `flatOperationType` (sheet metal), loft connection edge
  queries/parameters, routingCurve point `index`, tag's PCB fields; built-in-only params: loft
  `endConditions`, fillet... (see the test output).

## For the lead (integration)

- New crate `crates/featurescript` + `noshape-featurescript` workspace dep; `rhai` removed from Cargo.toml →
  Cargo.lock changes (regenerate with a build).
- ~~New `clippy.toml`~~: removed; not needed any more (see Session 2). Don't commit a clippy.toml.
- Cross-area edits this session (surgical): regen/features/direct.rs `move_rigid` → pub(crate) (psb);
  app fs_notices.rs / feature_studio.rs / fs_search.rs (ui2) for the notices, Monitor and symbol search;
  new icon assets/icons/fs-info.svg.
- Rhai is fully replaced (custom/{api,compile,eval,schema}.rs deleted). Demo + standard content are FeatureScript;
  tests/custom_features.rs: 8/8 passed at the previous agent's last run; session 2 added fs_probe_matches_onshape (not run yet) and y_threads still fails (see PAUSED).
- Disk hit 0 bytes twice on 19 Oct; I deleted incremental dirs over 2 GB (lead, kern, psb, ui, ui2) per the rule.

## Lead requests

- (19 Oct) Ctrl+Shift+F must search text across Feature Studios like Onshape's. ui2's pane exists (`fs_search.rs`,
  bound in `feature_studio.rs` `bind_keys`); verify against the help in stage 5 and drive its symbol /
  standard-library modes from the FeatureScript parser.

## Open questions for live (ground truth)

- Answered: parse/compile notices (dom/fs-parse-errors.txt), license, FS probe (dom/fs-probe-output.txt).
- Pending: part 3 (REQUESTS.md "fs (05:45)").

## Gotchas

- `is`/`as` bind tighter than `*` (`p as Vector * meter`); `=> {` is a map literal when followed by `key :`.
- Imports must come before other top-level items (Onshape's grammar; error text lists the expected tokens).
- Maps iterate in key order (Onshape's total order), enum keys by ordinal.

## From the lead (2026-10-19)
- Embedding the std library unmodified is fine (MIT: per-file headers + the LICENSE tab captured live);
  `crates/featurescript/std/LICENSE` must stay next to the sources. The README has no third-party
  notices section; if one is added, list the FeatureScript Standard Library (MIT, PTC) there.
- Drawing custom tables (docs/parity/drawing.md TB25/TB26) need FeatureScript table support (`defineTable`, as in Onshape). Add it after the core plan; asb hooks the drawing side.

## From kern (2026-10-20): `opCreateBSplineSurface` kernel function ready
- `noshape_kernel::make_bspline_surface(&BSplineSurfaceData, boundary: &[BSplineCurve2dData], make_solid) -> Result<Shape>`
  (docs/kernel-api.md › surfacing; test `kernel/tests/surfacing.rs::bspline_surface_from_featurescript_definition`,
  std's two `bSplineSurface` doc examples + a boundary loop + a closed torus net).
- Pass std's `BSplineSurface` as `bSplineSurface()` returns it: `poles[i * nv + j]` = `controlPoints[i][j]` (row i along
  u), `weights` = the flattened `weights` matrix or `None`, `u_knots` / `v_knots` = the full (padded) knot arrays, the
  overlapped rows/columns of a periodic direction included, degrees and periodic flags as given. Lengths in metres.
- `boundaryBSplineCurves` → `BSplineCurve2dData { poles (u, v), weights, knots (full), degree, periodic }` each; they
  must form one closed loop (else `KernelError::Invalid`). No boundary = the whole parameter range.
- `makeSolid` → `make_solid`: a solid when the sheet closes (both ways periodic, or closed with collapsed ends), else
  the sheet. The result is one face (sheet) or one solid; give it your feature's naming/history.
- Errors are `KernelError::Invalid` with a message (bad knot count, decreasing knots, multiplicity > degree + 1,
  degree outside 1..25, non-positive weights). Onshape's own error text for these is not captured; map them to
  Onshape's message where you know it.

## Plan: std sheet-metal internals (fs), from live dom/fs-smprobe{,2}-{1,2,3}.txt
Definition body (hidden from qEverything, reached via getSMDefinitionEntities / association attrs):
- sheet of planar WALL faces meeting at SHARP edges; bends = shared edges with JOINT attr
  {jointType BEND, radius, angle, bendType STANDARD, holdAdjacentEdges}; rips = JOINT RIP (sm3 to confirm);
  body: MODEL attr (thickness, defaultBendRadius, kFactor, relief scales, active, controllingFeatureId…).
- definition plane = the "back" side face (sm1 thicken: definition at z=2 n=-z, part z 0..2).
Solid built by updateSheetMetalGeometry:
- each wall thickened by T in the thickness direction, trimmed back by the bend's inner radius
  from every BEND edge (base 40x20 -> 40x19 with r=1); bend = cylinders r and r+T (axis = edge offset by r);
  free edges -> side faces; association attr on every solid face/edge/vertex = transient of its definition entity
  (walls -> both caps; free edge -> side face + its 2 edges; vertex -> corner edge+verts; bend edge -> both cylinders + 4 lines).
Natives to write: updateSheetMetalGeometry (rebuild solid from definition + attrs), queryContainsFlattenedSheetMetal,
evSheetMetalFlatTransformation (live: walls of flat sm1 -> translation (0,0,-T)), sheetMetalApplyInFlat, opSMFlatOperation,
evSheetMetalBendUp, opDerip, evSheetMetalHole/FormToolBodies.
Order: (1) hidden definition bodies + association attributes in host (getSMDefinitionEntities works);
(2) updateSheetMetalGeometry for walls + BEND joints (sm1, sm2 exact face areas as tests);
(3) RIP / corners / reliefs; (4) flat ops. Reuse sheetmetal_util / flange kernels where possible (kern's files: coordinate).
Test fixtures: sm1 6 faces (800/800/80/80/40/40), sm2 14 faces (760,760,38,38,80,280,280,14,14,80, cyl 188.4956 r3, 62.8319 r1, 6.2832 x2).

**Status (20 Oct, after v0.4.1):** steps (1)+(2) done. `@updateSheetMetalGeometry` (custom/host/sheetmetal.rs):
rebuilds each MODEL-attributed definition sheet's part (walls pulled back R·tan(θ/2) from BEND edges, R = inner
radius + thickness on the concave side; bends = wall thickness revolved about the bend axis; fuse without unify,
so side / bend-end faces stay separate as live), copies association attributes onto every part face/edge/vertex
(walls→caps, free edges→sides & cap edges, bends→cylinders & tangent lines, vertices→corners/bend ends), reuses
the previous part's BodyId, hides the definition sheet. Hidden bodies + FS attributes now persist between
features: `Context.fs_hidden_bodies` / `Context.fs_attributes` (restored in `FsContext::new`, saved by
`FsContext::take_context`); `QueryCx::all` skips hidden bodies, ATTRIBUTE_FILTER sees them. opExtrude of
connected curves now gives ONE sheet (sewn) facing `tangent × direction` (live sm3). Test:
tests/fs/sm_harness.fs (`std_sheet_metal_matches_onshape`): std sheetMetalStart THICKEN (sm1) and EXTRUDE of an L
(sm3) give live's exact face areas and sm3's live face→definition mapping (4 faces / 8 edges / 2 vertices).
Next: RIP joints, corners & bend reliefs (live probes), std Flange/Hem/Tab (their internals), flat ops; and
`ctx.sheet_metal` for std-made parts so our flat pattern / bend table see them.

## PAUSED (20 Oct ~16:55, lead: server OOM + reboot) — exact state
- **Integrated (dc0da0a):** opHole (+qOpHoleFace/Profile, live-matched), opOffsetWire, evOffsetDetection, unpackQuery /
  qCompressed (+miniz_oxide), `?:` left-associative, updateSheetMetalGeometry + hidden definition bodies +
  `Context.fs_attributes` / `fs_hidden_bodies`, opExtrude of connected curves = one sewn sheet facing tangent×dir,
  Feature Studio shortcuts (ctrl+shift+O, ctrl+[ / ]), Module outline anchor fix, nav history with labels
  ("L, C" / "Symbol - name" / "Use - name"), ctrl+click go-to-definition (same studio), ▾ history menus.
- **Uncommitted, mine:** crates/model/tests/custom_features.rs (+1 expected line) and tests/fs/sm_harness.fs
  (+11 lines): sm3 face→definition check ("sm3 definitions { EDGE : 8 , FACE : 4 , NONE : 0 , VERTEX : 2 }",
  matches live). Last run green (custom_features 23/23, host units 6/6). Not mine: tests/features_b.rs.
- **Verification state:** model crate clippy clean for my files (run with `-A clippy::chunks_exact_to_as_chunks`
  only because of the foreign error at src/display.rs:50); featurescript clippy clean. App: NOT yet built with the
  nav changes on my side (3 attempts killed by OOM); the ctrl+click / history ▾ / outline-pick history are
  untested in the app. ctrl+shift+O, ctrl+[ / ], outline anchor and New feature + ctrl+z (not reproducible) were
  verified on :108 before.
- **Next on RESUME:** (1) build app (target-engine), copy to /tmp/fs/noshape, Xvfb :108 (recreate /tmp/fs —
  wiped), test: ctrl+click a call → definition, ctrl+[ back, ▾ lists "<tab>: …" most recent first, outline pick
  adds "Symbol - name(params)". (2) ctrl+click on a std symbol (live: opens std source read-only in a new tab at
  the export line) — not done. (3) Sheet metal next steps (see Status above): RIP joints, corners / bend reliefs
  (need live probes), std Flange/Hem/Tab internals, flat ops, `ctx.sheet_metal` for std-made parts.
- /tmp/fs-backup recreated (b<MMDDHHMM>) with my current files; plan text lives in this file (section above).

## Resumed (20 Oct ~21:00)
- Verified on :108: ctrl+click → definition (other → 9:10, helper → 4:10, as live 14:10), ctrl+[ back, ▾ back /
  forward menus ("<tab>: Use - name" / "Symbol - name", most recent first). Added: the caret before a ctrl+click
  is recorded first (live list order). New: ctrl+click on a std symbol → `crate::std_source` builds
  <config>/std-3095/std.noshape (one Feature Studio per std module, version "3095.0") once and opens it at that
  version in a new window on the module's tab at the export (evArea → evaluate.fs 152:17 as live); not yet tried
  in the app.
- Host: TRACKING queries follow entities renamed by operations (point snapshots per transient id, `CtxState::snapshots`,
  `QueryCx::tracked`); evEdgeTangentLines with "face" orients directions around that face (face on the left).
- std Flange (sm2 in sm_harness.fs) runs but is NOT live-equal yet: the joined sheet has the flange wall 44.25 wide
  (std's internal-flange offset of one bend-relief width fires at both ends) and the joint becomes RIP instead of
  BEND (the tracked bend edge doesn't map to the merged edge). Its assertion is left out (comment with live's
  areas in custom_features.rs) until fixed. Live request "fs (21:10)" pending for rips/corners/reliefs/hem/tab.

## 21 Oct: std Flange matches live (sm2)
- std `sheetMetalFlange` now gives live sm2's exact part (760,760,280,280,188.4956,80,80,62.8319,38,38,14,14,6.2832,6.2832);
  asserted in `std_sheet_metal_matches_onshape`. Fixes (host, shared query/eval code):
  - `edge_param`: parameters run along the edge's own direction (reversed edges flipped points vs tangents; std's
    getVectorForEdge then saw the neighbour edge pointing out → "internal flange" relief offset → 44.25 mm wall).
  - `evEdgeTangentLines` with "face", `evFaceTangentPlanesAtEdge` (x = edge direction, `usingFaceOrientation`):
    face on the left, as std documents.
  - Transient ids / TRACKING: point snapshots per id (faces, edges, vertices, sketch curves; owner body type);
    `QueryCx::resolve_transient` keeps the name's result only if it still lies there, else what lies there
    (hidden bodies included, solids only ↔ solids); moved entities (nothing there) keep their name.
- Ctrl+click on a std symbol verified on :108: "std 3095.0" window, view-only bar + [Return to Main], evaluate.fs:152.
  Feature Studios now show the version view-only bar.
- Profile ▾ (Monitor menu, from ui2): waiting for live capture REQUESTS "fs (22:55)".
- Next: rips / corners / bend reliefs / hem / tab (live "fs (21:10)" pending), flat pattern, ctx.sheet_metal for std parts.

## 21 Oct (after v0.4.3): Profile, CONVERT, corners, speed

- **Monitor › Profile** done in code (crates/app/src/fs_profile.rs, interp `LineProfile`,
  `custom::run::{set_profiling, profile}`, feature_studio.rs `render_profile`): total badge +
  per-line heat bars, grey while uncommitted, commit re-profiles, Variable Studios too. App check
  on :108 still to do.
- **Tracking** (host/query.rs `QueryCx::tracking`): a `startTracking` id now also yields what was
  made from the entity, a solid's edge carrying over to a sheet (std CONVERT extracts the box
  faces and finds its bend edges this way). sm4 now has 4 BEND + 4 RIP like live.
- **Simple corners and rips** (host/sheetmetal.rs `build_part`): walls stop `minimalClearance`
  short of a rip; a bend runs only where both its walls' pulled-back sides run (it ends where the
  neighbouring bend starts). From live sm4/sm6.
- **Closed Hem (sm7) matches live**: new `@opEdgeChange` (host/edge_change.rs, offset option on
  planar sheets; `transformList` / `replaceFace` not yet), a tracked sketch vertex follows onto the
  edge it swept (`QueryCx::tracking`), and curved definition faces are rolled bends thickened as
  they are (`rolled_bend`). sm4 and sm7 face areas are asserted in std_sheet_metal_matches_onshape.
- **Queries**: `qDependency` returns an extrude's / revolve's profile (`CtxState::dependencies`);
  `qSplitBy` bodies by side of the split plane and the faces the split cut (`CtxState::splits`);
  construction planes / points are their own owner body (std draft splits with `qOwnerBody(opPlane)`).
- Next: ROUND / RECTANGLE corner reliefs (sm6b/c) and bend reliefs (sm5) once the live variants
  arrive (REQUESTS.md fs 10-21: dom/fs-smprobe4-*.txt), Tab (sm8).
- Speed: perf on the std harness shows std lex/parse/compile is the biggest single cost, but it
  is paid once per process (STD_CACHE) and studios are cached by source hash; per-regen cost is
  interpreter + kernel. Release-build profiling still to do.
- tests/fs/std_reference.fs: all 23 cases run (revolve takes `fullRevolve`/`endBound`; `cube`
  clashed with std's cube feature; `opSphere` for the intersect tool). Live numbers requested
  (dom/fs-stdref.txt); then fill them into std_features_match_onshape and drop its #[ignore].

### 21 Oct, later (fs)
- **Tab (sm8) matches live**: sheet unions are sewn and same-domain unified (ops.rs
  `merge_sheet_faces`), so a tab merges into its wall's face (850 mm²).
- **Reliefs** (host/sheetmetal.rs; live dom/fs-smprobe4-*.txt, rules in the doc comments):
  - bend relief (partial flange): `BendRelief` from the model (`defaultBendReliefStyle` is a bare
    enum in the attribute); slot `scale × T` wide, straight `depth scale × r`, OBROUND half circle /
    RECTANGLE squared at D + w/2 / TEAR none; collinear sides pulled differently end in a step
    (`pulled_sides`). Harness cases sm5/5b/5c/5d (needs `flipFlangeBounds` for the partial flange).
  - corner relief (`corner_reliefs`, style from `defaultThreeCornerStyle`): plain styles scale with
    the bend-allowance square (RECTANGLE: side scale × BA; ROUND: radius scale × half diagonal),
    SIZED ones use the width / diameter (too small: not applied; live warns "A corner relief is too
    small…", not reported yet). sm6, sm6c, sm6e, sm6f, sm6g match live exactly; ROUND (sm6b/6d) is
    within ~1e-4 relative (the bend cut is a loft through the wrapped circle; check the mapping);
    CLOSED (sm6h) not done.
- **Onshape topology in queries** (host/query.rs `Topo::shown_*`, applied at the end of `eval`): no
  seam or degenerate edges, no vertices on closed curves only, cone apex kept (opHole tip).
- **std reference vs live** (dom/fs-stdref.txt): `box` is a reserved word (lexer `Kw::Box`); revolve
  reads std's `endBoundAngle`/`startBoundAngle`; draft reads `pullVec`; boolean INTERSECTION is
  tools-only, BOOLEAN_BAD_INPUT with fewer than two (as status, so std rethrows); `evBox3d` tight
  boxes lose OCCT's 1e-7 m margin. Remaining: sweep/loft come out of the kernel slightly different
  (sweep box 1610.0001 vs 1610; loft square→circle 7/15/10 vs live 6/12/8: Onshape splits the circle
  into 4 arcs to match the square), filletCorner volume differs at 1e-6 mm³.

## From ui (21 Oct): qa5 SM4, the "Sheet metal table and flat view" panel; split proposal (lead: fs + ui)
Live spec: behaviours.md "(8) Sheet metal table and flat view" (sm-090…105) and "(10) Export DXF/DWG of Flat pattern".
- **Yours (model):**
  - per sheet-metal model, a list of **bends**: name "Bend A…", radius, angle, Up/Down direction (SM5: ours says Down
    where live says Up), K factor;
  - **other joints**: name, type Rip|Bend, style Edge joint|…;
  - a **"Modify joint N"** feature appended when a table cell is edited (live: a bare "3" + Tab on Bend B's radius
    added it);
  - the flat body: a single solid with bend-zone strips, relief cut-outs (SM6), and bend centre lines as wire bodies
    "Bend line 1…".
  - The API I need: `fn sheet_metal_table(studio, model_feature) -> { bends: Vec<{name, radius, angle_deg, up, k}>,
    joints: Vec<{name, kind, style}> }`, an edit entry that appends Modify joint, and the flat solid + bend lines as a
    display body.
- **Mine (UI):**
  - the 717 px right panel: "Sheet metal context: [model ▾]";
  - collapsible **Bends** grid (# · Name · Radius(mm) · Angle(deg) · Bend direction · K Factor; horizontal
    scrollbar) and **Other joints** grid (Name · Type ▾ · Style ▾);
  - "#" click selects the row (highlighted in both views); Name click gives a cell outline; double-click edits
    (pink invalid, "Sheet metal model could not be rebuilt…" title);
  - the **flat view**: a second shaded viewport with its own view cube, dashed centre bend lines, and the
    right-click menu (Copy / Create Drawing / Export DXF/DWG of Flat pattern…).
- Until your API lands I'll build the panel on what `fab_panel.rs` has today. Tell me here if you'd split it
  differently.

### 21 Oct evening: built-in sheet metal by std (fs), qa5 SM findings
- `custom/std_feature.rs`: built-in feature types run by std's own feature (`sheetMetalFlange` of
  sheetMetalFlange.fs, ...), the definition built from std's precondition (`run::run_feature`, shared
  with custom features). Switched on by `set_std_sheet_metal(true)` / `NOSHAPE_STD_SHEET_METAL=1`
  (hook at the top of `regen::features::evaluator`); default off until the bridge below exists.
- tests/sheetmetal_std.rs: built-in Sheet metal model + Flange through std = live G34 exactly
  (8511.4987 mm³, 24 faces; qa5 SM1). Tab on the top face (SM2) added.
- Plan for the qa5 list (docs/parity/workflow-sheet-metal.md):
  - SM1/SM2/SM3/SM16: std route (+ std editing logic for Tab's auto-fill).
  - SM4/SM5/SM6/SM13/SM14/SM17: bridge the std definition into `SheetMetalData` (walls, bends with
    Up/Down from the geometry, rips, relief marks) so the panel, DXF and flat follow; DXF format
    per SM14; panel/export UI with ui + psb.
  - SM7: per-vertex bend relief attributes (like the per-vertex Corner ones now read in
    `corner_overrides`). SM8: our fields follow std's precondition (SIZED_* = absolute size),
    matching live sm6g; my request text "SIZED_RECTANGLE scale 2" was wrong, not live.
  - SM12: pcbStart.fs / pcbEnd.fs exist in std; types via std + toolbar entries (ui).
- Attributes now follow renamed entities (`CtxState::remap_attributes`, snapshots per attribute):
  definition vertices lost their association attributes when a flange renamed them, so Corner at
  a vertex failed (SHEET_METAL_RIP_NO_CORNER).

## HANDOVER (21 Oct ~19:00 UTC, fs moves to another account) — exact state
Main at d5745c9 had part of this; uncommitted on top (all compiling; clippy and tests were being re-run at
handover, see the last line): host/mod.rs, host/query.rs, host/sheetmetal.rs, regen/context.rs,
sheetmetal/flat.rs, tests/custom_features.rs, tests/sheetmetal_std.rs.

### What works now (verified against live numbers)
- std sheet metal through the FS host matches live exactly for: sm1–sm4 (CONVERT tray), sm7 (closed hem),
  sm8 (tab), sm5/5b/5c/5d (partial flange bend reliefs OBROUND/RECTANGLE/TEAR), sm6/6c/6e/6f/6g (corner reliefs
  SIMPLE/RECTANGLE/SIZED_ROUND-too-small/RECTANGLE/SIZED_RECTANGLE), G34 (qa5 SM1: 8511.4987 mm³, 24 faces).
  All asserted in tests/custom_features.rs `std_sheet_metal_matches_onshape` (tests/fs/sm_harness.fs).
- Built-in features through std (`custom/std_feature.rs`, opt-in): tests/sheetmetal_std.rs — G34 flanges exact,
  and `flat_pattern_keeps_the_model_placement`: the std-built flat (live placement, bend lines at -0.2777 UP,
  bounds -18.5553..60 × -18.5553..40) = live dom/qa5-sm-flat.dxf for those flanges. Tab-on-top-face test is
  `#[ignore]` (not yet run green).
- std reference probe: tests `std_features_match_onshape` asserts 19 live lines (dom/fs-stdref.txt).

### Fixes made this round (all in host/, mine)
- Transient ids: a cached id of an edge/vertex is re-checked when its body changed (`body_versions`,
  `transient_versions` in CtxState::transient): names get reused after an op, the stale id pointed at another
  edge (sm5's partial flange went onto the plate's right edge).
- Attributes follow renamed edges/vertices of sheet bodies (`attr_snapshots`, `remap_attributes(changed)` after
  each op, only for bodies the op changed; recomputed in FsContext::new). Definition vertices keep their
  association attributes (Corner at a vertex works).
- `@evCornerType` (sheetmetal.rs): NOT_A_CORNER / BEND_END / CLOSED_CORNER (≥2 bends + rip) / OPEN_CORNER;
  returns transient id *strings* (std wraps them with qTransient).
- qSplitBy for opSplitEdges (`CtxState::split_edges`), relief slots after a moved (INNER-aligned) bend edge
  (`relief_tools` follows the short connector to the plate edge line).
- Bridge for the app: `update_sheet_metal_geometry` now also stores `ctx.sheet_metal[part]` (SheetMetalData:
  walls by definition face, bends with geometry + `ref_inner` = up as in the flat, rips) and
  `ctx.sheet_metal_flats[part]` (FlatPattern from `flat_of`: walls' definition-plane faces + bend strips placed by
  `unfold`, bend centre lines); `sheetmetal::flat_pattern_for` returns the std flat first
  (`flat::flat_from_pieces`). Names renamed marker→feature after the op.

### Not done / next steps, in order
1. Verified at handover (19:10 UTC): `cargo clippy --locked -p noshape-model -p noshape-featurescript
   --all-targets -- -D warnings` exit 0; custom_features 24 passed / 1 ignored (bench), sheetmetal_std 2 passed /
   1 ignored (tab). The bridge (sheet_data, ctx.sheet_metal_flats) runs for every std-built part already (also from
   custom features), but the built-in route is still off by default.
2. Switch the std route on by default (std_feature.rs `STD_SHEET_METAL`), then fix what breaks: native-only tests
   in tests/sheetmetal_frames.rs and tests/features_c.rs read native SheetMetalData details; rewrite them against
   live numbers or set_std_sheet_metal(false) in that binary as an interim. Check the app (fab panel, DXF export,
   drawing flat view: model/src/drawing/views.rs uses flat_pattern_for) on :108. This closes qa5 SM1/SM2/SM3 in
   the app (lead's re-run still shows 8383.122 because the route is off).
3. CLOSED corner (sm6h, G34a): runs now but 8541.2499 mm³ / 39 faces vs live 8535.0418 / 23 (G34a) — the
   wrapped closed cut (`WrappedCut::Closed`, loft of 33 sections) leaves slivers (0.0283, 3e-4, 2e-4 mm² faces);
   build the bend end as one ruled face (two sections suffice where a(w) is linear), and check the plate corner
   chamfer prism. Live geometry: behaviours.md "G34" (a).
4. ROUND corner relief (sm6b/6d) ~1e-4 relative off (bend faces +0.0004 mm², front flange +0.0014): check the
   wrap mapping (per-fiber vs neutral) and loft accuracy.
5. SM14 DXF: write AC1014 with live's 49-layer table (samples/qa5-sm-flat.dxf: LINEs on SHEETMETAL_CUT_LINES,
   bend centre lines on SHEETMETAL_BEND_LINES_UP/DOWN (CENTER linetype), no TEXT); the flat to write is
   `flat_pattern_for` (std flat). SM16: Tab dialog auto-fill = std's `sheetMetalTabEditingLogic` (run std editing
   logic for std-backed built-ins like custom features do). SM12: pcbStart/pcbEnd via std + toolbar (ui).
   SM13: remove "Export flat pattern (DXF)…" from the part menu (psb/ui; live has only the flat view menu).
   SM7: per-vertex bend relief attributes (like `corner_overrides`).
6. Eval speed: no local release builds (team rule); profile in debug with perf on `bench_std_regen`
   (tests/custom_features.rs, ignored) or ask the lead for a CI perf job. Earlier flat profile: std
   lex/parse/compile dominates but is once per process; interpreter SipHash on Arc<str> maps is the next target.
7. App: Profile (Monitor ▾ › Profile) never checked on screen; ADV7/ADV6 (threads) are psb's with me.

### Scratch
/root/fs-scratch: live-areas.txt (live face areas per case), stdref-live.txt, freeze-1505-all.patch,
backup-featurescript-*.md.

## From asb (21 Oct night): drawing bend notes direction (qa5 DSM2 = your SM5)
- The drawing flat view's bend notes take UP/DOWN from `FlatPattern.bend_lines[].up` (model/sheetmetal/flat.rs:300,
  `b.is_up() != data.flip_direction_up`). qa5's sheet-metal drawing shows "DOWN 90°" where live shows "UP 90.0°", the
  same inversion as SM5. The angle format ("90.0°") is fixed on my side; the direction is in the flat pattern data:
  please check `up` against live when you fix SM5 (the std-route flat `flat_from_pieces` included). The drawing reads
  it as is.

## Session 21 Oct ~19:30 UTC (new fs, after handover) — state
- Disk was 100% full at takeover; deleted stale (>60 min) test executables in /root/target-engine/debug/deps (20 GB).
- **std route OFF by default** (lead, freeze 21 Oct): `NOSHAPE_STD_SHEET_METAL=1` / `set_std_sheet_metal(true)` turns it on;
  only pcbStart / pcbEnd always run by std (no native). Switch the default on only when every existing test passes with it
  on (sheetmetal_frames, features_c, drawing tests, custom_features). My std-route version of sheetmetal_frames.rs (live
  expectations: L/U walls lose R, flange up = UP, Finish needs parts + std message, hem bbox 2e-7, all face names stable,
  notices in `ok()` failures) is /root/fs-scratch/sheetmetal_frames.std-route.rs: put those changes in when switching.
- **Bridge fixes (host/sheetmetal.rs):**
  - rolled bends (definition faces with a BEND joint: a Hem's arc, std `annotateHemSheets`) are strips: `rolled_strip`,
    `Definition.rolled`, table bend kind Hem, unfolded across (`place_from`), flat strip + bend line;
  - walls joined only by rips are separate parts (live G34d): the fused solid is split into connected solids, each its own
    body (reusing `Definition.previous`, now all earlier parts), own `SheetMetalData` (walls re-indexed, rips to another part:
    `b = usize::MAX`, `other` = that wall's reference face) and own flat (`unfold` places one root per part).
- **sheetmetal_frames.rs** expectations moved to live: extruded L/U walls lose R (sm3: material outside the turn), flange
  up = UP (G34 DXF), Finish needs "Sheet metal parts" (std).
- **ADV7**: External thread "Split face" imprints the circle at the thread length (hole.rs `split_thread_face`); test in
  features_a `external_thread_is_cosmetic` (3 → 4 faces, edge at z = 10).
- **SM13**: "Export flat pattern (DXF/SVG)…" removed from both part context menus (menus.rs, mod.rs); live has it only in
  the flat view's menu ("Export DXF/DWG of Flat pattern…", ui's panel).
- **SM12**: `pcbStart` / `pcbEnd` feature types (features/sheetmetal.rs, schemas from pcbStart.fs / pcbEnd.fs) run by std,
  in the Sheet metal ▾ list after Finish sheet metal model, icons pcb-start.svg / pcb-end.svg (own drawing), tool texts,
  shortcuts list.
- **SM14**: `sheetmetal::flat_dxf` (`FlatExport { insunits, version, bend_centerlines }`, `flat_dxf`, `flat_dwg`): AC1014,
  live's 48-entry layer table with on/off/frozen and line types, LINEs on SHEETMETAL_CUT_LINES / SHEETMETAL_BEND_LINES_UP|DOWN.
  The app's export_flat("dxf") uses it. ui: the "Export as DXF/DWG" dialog for the flat view should call `flat_dxf`/`flat_dwg`.
  Curved outline pieces are still written as LINE segments (live probably ARCs; no live sample with arcs yet).
- New tests: sheetmetal_std `hem_on_g34_matches_live` (9378.1835 / 33), `flat_dxf_is_written_as_live`.

### Std route status (23 Oct ~02:30); still off by default (NOSHAPE_STD_SHEET_METAL=1)
sheetmetal_std: 8 of 9 pass (Hem on G34, Bend = live BEND1 4812.566370614357 / 14 faces, Tab on G34, Corner break at a
free corner, flanges, flat pattern, DXF). Landed on the way: FeatureScript tracking queries keep only what operations
after `lastOperationId` made or changed (std feature.fs; `touched` in CtxState, evaluated in the TRACKING branch of
query.rs); opWrap honours a cylinder's `allowanceRadius`; wrap onto a cylinder makes exact lines/arcs (surfacing.cpp);
opDeleteFace `leaveOpen` splits a sheet into its connected pieces.
Still to do:
1. Closed corner (`closed_corner_matches_live`, ignored): 8541.21 / 35 vs live 8535.0418 / 23. The bend ends follow
   x0 − (x0 − x1)·sin θ (fits live's cylinder areas); the corner cut (loft cutter in build_part) leaves the bends' end
   caps at x = 0.02 on the outer half of the thickness (4.979 + 0.702 mm² per bend). A ruled loft gives the same faces,
   so the cutter shape is not the cause; look at its s range / which wall is `a`.
2. Corner break next to a bend (sm_flat.rs only does a wall's free corner).
3. Rolled hem with a free end (rolled_strip wall b = usize::MAX; untested).
4. Part names stable across rebuilds (Flange after Finish finds its edge): the DefNames attempt (part faces named from
   definition entities) is in /root/fs-scratch/sheetmetal.rs.defnames; it changed sm2's flange areas (a 38 mm² face became
   80), so it was backed out. Find out why (std's flange relief tracking by name?) before retrying.
5. SM16: done (std editing logic runs in dialogs; `std_feature::editing_logic_in`).

### What still fails with the std route forced on (NOSHAPE_STD_SHEET_METAL=1, 23 Oct ~21:00)
Gate for switching it on: every one of these passes (or is shown to test native-only internals and is rewritten
against live numbers). features_c passes fully both ways (2 ignored: loft rect->triangle, below; one older); sheetmetal_std 9/9 + Closed corner ignored.
Done today: Jog preserve-off (kernel find_plane), native Jog follows std's applyEditLogicForBendAngle (Info, smaller
angle); Form on std end to end: Tag runs by std (std param ids; FeatureList values `{Id: function}` tagged FeatureList),
qConsumed, opMergeContexts / opPattern keep attributes, evCollision face pairs (crossing = INTERFERE), and
updateSheetMetalGeometry applies walls' formedToolBodyIds (cut removes, fuse adds; tools hidden with persistent
transient ids `JP<n>` in Context::fs_transients; flat footprint + outline). Native Form/Tag follow std's rules
(untagged studio fails, locations only on targetFaces, std messages). recognize / jog tests check geometry.
- Rip: done (24 Oct). Live = no-op (1 part, 8000 mm³, 6 faces, no joint); its probe: std's split works, and a rip
  between coplanar pieces of a wall leaves no gap. Host: coplanar definition faces get no rip gap, the part's faces
  are unified, the unfold carries them over, no table joint; native Rip = std's checks only. (Our qCreatedBy(split,
  EDGE) seems to give no edge where live gives 1; harmless here, look at it if a rip ever needs its attribute.)
- Sheet metal loft rect -> triangle (ignored test, live "fs (10-23 10:30)": 17452.843417228427 mm³, 49 faces: PLANE 37,
  CYL 10, OTHER 2; 5 bends + rip). Std (host loft.rs walks unequal profiles: shortest diagonal, parallel edges
  together = live's trapezoid + 5 triangles, bend angles match) gives 51 faces / 17449.70: where three bends meet live has
  2 swept corner faces, we have planar patches. Native: unequal corner counts unsupported.
- EXTRUDE process: live answered (REQUESTS "fs (10-23 05:00)"): default = 3211.3274122872 mm³, material OUTSIDE the L,
  flat 80.126106 x 20; flipped = 3051.33, inside, 76.126106. Std matches the default; the native route (inside) deviates.
- sheetmetal_frames (7; SM5 fixed the outer-flange one): hem bbox, bend count, flange outer bend direction, convert BUTT, corner relief volume.
Not mine but seen: drawing::tests::flat_pattern_view_shows_bend_lines_and_notes fails on "DOWN 90.0° R3" (format.rs angle).

## fs (24 Oct, new account): P1 std-oracle pass

Sheet-metal WIP 2c8697d2 reverted on top (f8bd0a64; `git revert f8bd0a64` brings it back when P3 resumes).

**Oracle infrastructure (tests/std_oracle.rs):**
- `core_dialogs_match_std_in_every_state`: for each feature in `STRICT`, every parameter (id, label, kind, bounds,
  metric/imperial defaults, hints, description) equals the dialog std's precondition declares
  (`std_feature::std_dialog`, works for defineSheetMetalFeature and non-exported features like fTransform), and the
  shown parameters agree in the default state, every single and pair of enum/bool choices and 400 random states.
  Query filters are not compared (the generator maps std filters roughly: no SketchObject.NO /
  ConstructionObject.NO / QueryFilterCompound); review them by reading std. `STD_ORACLE=fillet cargo test -p
  noshape-model --test std_oracle print_one -- --nocapture` prints one feature's diff.
- `<feature>_regenerates_as_std`: scenarios regenerated natively and with std's own feature
  (`std_feature::set_std_route(&["extrude"])` runs the core feature through std; CORE list in std_feature.rs);
  status, message, faulty fields and bodies (kind, faces, edges, volume/area) must agree. `STD_ORACLE_CASE=draft
  NOSHAPE_FS_TRACE=1` narrows to cases and traces std's operations.
- std's faulty parameters now reach FeatureError::faulty (run.rs).
- Dialog generator: a "Driving Parameter" group's boolean gets GROUP_HEADER (as built-in dialogs).

**Extrude (done, 7e603877, ready):** dialog identical in every state (added hidden `domain` + flat-view `flatOperationType`,
second depth not tolerant, second end hidden when symmetric). Native errors now std's texts + faulty fields
(EXTRUDE_SELECT_TERMINATING_*, EXTRUDE_SELECT_DIRECTION / DIRECTION_INVALID_ENTITY / DIRECTION_COPLANAR,
EXTRUDE_SELECT_START_OFFSET_ENTITY / START_OFFSET_BOUND_NOT_PLANAR / NOT_PARALLEL, OFFSET_WIRE_SELECT_WALL_PATH,
EXTRUDE_SURF_NO_CURVE); Through all / Up to next without parts = "Parts or Surfaces must be present in the Part
Studio." (CANNOT_BE_EMPTY, live dialogs/extrude-endtype-*). Start offset to an entity follows std's distance rule.
Host fixes found by the oracle: SWEPT_FACE historical query, qAdjacent of sketch regions, opExtrude up-to planes /
UP_TO_NEXT / endTranslationalOffset, evMateConnector only for mate connectors, coplanar overlapping faces interfere
in evCollision (boolean heuristics: boss on a face = Add, as live).

std's editing logic for core features: `std_feature::STD_EDITING_LOGIC` (extrude, fillet) makes `has_editing_logic` true,
so the app runs std's function (part_studio/behaviors.rs `std_edit_logic`, now on a background thread; results for
stale values are dropped; changes made before `before_context` arrives run when it does) and skips the native
`boolean_edit_logic` for them. Seven-parameter editing functions get `isCreating` (EditSession::is_new); returned
arrays convert item by item (run.rs `editing_value`). Debug-build timing: std extrudeEditLogic ~300 ms (it runs a
trial extrude + evCollision), filletEditLogic ~3 ms.

**Fillet (done):** dialog identical (BLEND_BOUNDS min 1e-5 m, Location 0.001..0.999, cross-section description,
variable fillet's vertex field always hidden). `fillet_regenerates_as_std` (24 cases): std's partial-fillet
checks now native (`blend_section::check_partial`: one chain, closed chain needs two bounds, offsets within the
chain, distinct bounds; std's texts), the second offset is measured from the chain's end (std), full round's
messages are opFullRoundFillet's (FULL_ROUND_SELECT_FACES / FULL_ROUND_COMMON_BODY; unsupported shapes =
FILLET_FAILED). Host: `opFillet` now runs the Fillet evaluator (every option: cross sections, width, asymmetric,
partial, variable), so the std route's fillet geometry equals native by construction; the oracle there compares
std's preprocessing and errors. Built-in evaluators' Onshape messages map back to their ErrorStringEnum member in
FS ops (`host::error_code`). filletEditLogic runs in the dialog (`fillet_editing_logic_runs_std`).

**Hole (done, pending checks):** dialog identical to std hole.fs in the V3 dialog (tests/std_oracle.rs `sync` keeps std's
legacy mirrors style/endStyle/showThreadClass = the V2 fields; std params no state shows are its V1 fields and are
skipped; large dialogs sample single changes + 1500 random states instead of all pairs). Fixes: tap drill default =
std's (5 mm / 0.25 in; the table sets the real one), computed depth's bounds, std's row order (computed depth before
blind depth). Lookup tables compared as cascades (`table_tree`): FS "Lookup Table" annotations now keep the
source order of the table literals (studio.rs `dyn_level` + `source_entries`, constants followed across imports),
which is Onshape's dropdown order (live L58) and gives levels without a default their first source entry; the
custom_features lookup test now expects source order. `hole_regenerates_as_std` (12 cases) passes; host fixes:
evAxis `allowSketchPoints` (sketch point -> its sketch normal), opHole failures thrown as the bare ErrorStringEnum
(hole.fs catches them as such). Known std-route gap: a hole that hits nothing — live flags Merge scope (native does),
std throws bare HOLE_NO_HITS (faulty field not compared in those 2 cases). Tapped depth linkage now follows std's
adjustDepthAndThreadParameters exactly (`hole_tables::link_tapped_depths`: of the two fields not edited, the later
one in Depth / Tapped depth / Tap clearance whose new value stays positive; live L49/L50 still hold);
`hole_tapped_depths_link_as_std` runs std's holeEditLogic against it. Hole's other editing logic stays native.
App: the native boolean edit logic is skipped at its caller (part_studio/mod.rs) for std-editing-logic features,
so api's shared `noshape_ops::studio::boolean_edit_logic` refactor merges cleanly.

**Loft and Transform (done, pending checks):** both dialogs identical to std in every state (Loft: array item names
"profile"/"guide"/"connection" and item templates as loft.fs, the End conditions group id `endConditionsGroup`
(the generator's naming), loft.fs's hidden guideDerivativeMagnitude / connectionEdgeQueries /
connectionEdgeParameters). `loft_regenerates_as_std` (9 cases) and `transform_regenerates_as_std` (12) pass.
Native fixes: Loft's empty-profiles error marks only the creation type's field; Transform follows
transformCopy.fs's checks and texts (two vertices / one edge / plane direction, TOO_MANY_ENTITIES_SELECTED,
TRANSFORM_TRANSLATE(_BY_DISTANCE)_INPUT, NO_TRANSLATION_DIRECTION, POINTS_COINCIDENT, CANNOT_RESOLVE_ENTITIES /
NO_ROTATION_AXIS from evAxis, TRANSFORM_SCALE_SELECTION, TRANSFORM_MATE_CONNECTORS). Host opLoft: `bodyType` is
an enum (surface lofts were solids), one profile = LOFT_TWO_PROFILES (live L15). std's loftEditLogic now runs in
the dialog (`loft_editing_logic_runs_std`). **Known gaps (kernel):** loft start/end conditions are ignored (native
warns; Onshape shapes the ends), loft guides and path are not supported natively ("not supported yet" errors).

**Rest of the P1 list (25 Oct):** Revolve, Chamfer, Shell, Linear/Circular pattern, Mirror, Sweep, Boolean, Plane,
Draft, Split are in `STRICT` (dialog in every state) with regeneration oracles (`*_regenerates_as_std`). Fixes:
- Generator: UIHint SHOW_CREATE_SELECTION / PRIMARY_AXIS / MATE_CONNECTOR_AXIS_TYPE (patterns' and Mirror's face fields,
  Draft's parting edges; Transform's mate-connector buttons now carry the latter two in the built-in too).
- Patterns: skip-instance items named "instance" with std's templates "(#index1, #index2)" / "#index".
- Native texts/fields per std and Onshape ops: Revolve REVOLVE_SELECT_AXIS [axis], Thin empty = REVOLVE_SURF_NO_CURVE;
  Shell SHELL_SELECT_FACES/PARTS marked; Boolean one tool = BOOLEAN_BAD_INPUT (live 7360), Subtract without targets
  = "Cannot resolve entities." (live 7216), empty tools unmarked; Split SPLIT_SELECT_TOOL / face split texts, SPLIT_NO_CHANGE
  as an INFO (not an error; live 10-25 06:15), and std's infos (plane or mate connector tool without Keep tools: "Planes and mate connectors will
  be kept."; a face: "Faces selected as split tools must be kept.").
- Host ops: opSplitPart keepType (front = the plane normal's side, live 7239) and SPLIT_SELECT_*/NO_CHANGE; opShell
  sharp outer corners (shell_solid, live S3); inward stays on k::shell (shell_solid gave 2220 vs live 2653.5 on
  std_reference shellTop, a kernel issue) but falls back to shell_solid when k::shell returns the body unchanged,
  then SHELL_FAILED; opBoolean / opDraft / opSweep empty-selection errors.
- Editing logic: run_editing_logic passes the arguments each function's parameter types ask for (cPlaneLogic has
  none, draftEditLogic isCreating+specified), and {} as oldDefinition on a new feature's first call (Onshape:
  preselection logic only then). STD_EDITING_LOGIC adds revolve, sweep, mirror, cPlane, draft, splitPart
  (`more_editing_logic_runs_std`: Split keeps a plane tool (live), Plane type from picks, Mirror/Revolve Add).
- Loft end conditions: psb's kernel support (3c25e9c9) is on ready/psb, not main yet; the two loft cases stay
  listed as a gap in `loft_regenerates_as_std` until it lands.

**Native argument audit (25 Oct, lead request after the knurl helix bug):** `custom/host/native_args.rs`
(`natives_read_the_maps_std_passes`) finds every `@native(…)` call in std (469 sites) and, where std passes a map (a
literal, or a `map` parameter it passes on), calls the native with a map of those keys and records the keys it reads
(`value::record_key_reads`: `Value::get`, `as_map` and `natives::map_arg` note reads while recording). It fails when a
native reads none of a map std passes (alignCanonically read argument 2 as the plane itself; fixed by the lead in
3fdf0b6a), when it reads only keys std never passes (a misspelt key), or when the set of natives std calls that do not
exist changes (`MISSING`, 18: evFaults, evTolerances, opCreateBSplineSurface, opImportForeign, opBooleanedPattern, …).
`IGNORES_MAP` lists the natives that need none of their map (no-ops, constants). `NATIVE_ARGS_REPORT=1` prints keys
read only in part; most are the probe's undefined values making a native stop early. Options found ignored and fixed:
evOwnerSketchPlane `checkAllEntities` (Sweep/Loft profiles), evRaycast `includeIntersectionsBehind`
(`evaluation_options_std_passes`). Still ignored: opPolyline `bendRadii` (Routing curve; reuse curves.rs
`filleted_polyline`), evCurveDefinition/evSurfaceDefinition `returnBSplinesAsOther` and evCollisionDetection `owners`
(live request "fs (10-25 b)"), addDebugEntities `color` (debug entities are not drawn at all), skConicSegment
`fixedRho`, skInterpolatedSpline `splinePointCount`, skSpline `guess`, evFacePeriodicity `trimmed`,
evFaceTangentPlanes `returnUndefinedOutsideFace` (no std caller passes the last five). Regression test for the helix:
`std_helix_from_a_circle_edge` (Turns and Pitch around a cylinder's circular edge).

**Pinned standard library versions (25 Oct, user's knurl script):** Onshape runs a Feature Studio against the std
version its imports pin. `crates/featurescript/std-versions/<version>/` holds older libraries (1963.0 so far, exported
unmodified from the public std document, MIT, live/std-1963), embedded by build.rs as `STD_VERSIONS`. An import of
`onshape/std/x.fs` pinned to v loads the nearest embedded version (`std_version_for`, the newer on a tie); an older
library's own imports stay in that version (`compile::load_std`, cache keyed by (version, module)). Types share tags
across versions by declaration key (`onshape/std/query.fs#Query`), except enums whose members changed (QueryType,
ErrorStringEnum, UIHint, FeatureScriptVersionNumber, ToleranceType, …): those get a tag per version and compare by
member name (`Value` equality, `is`, `value::map_get` for `{ … }[enum]` lookups); the host still makes 3095 values
(std_tag/std_enum), and isAtVersionOrLater compares the version number in the member name across versions. Exports of
1963 that 3095 dropped: HelixType, RevolveType, helixLogic, tagProfile, getIncrementingId, getFaceSweptData,
mutualTrimMaipulatorChange, the `*FromBuiltin` geometry constructors; signatures that changed include parsePitch /
computePitchValue / buildPitchAnnotation (now take context), getToleranceInfo, verifyVariableName, sweepEditLogic
(isCreating), helixManipulatorChange (full list: /root/fs-scratch/export-diff.txt). Tests:
`std_1963_pinned_helix_types` (custom_features), `natives_read_the_maps_older_std_passes` (the native audit on 1963).
To add a version: export it like live/std-1963 into std-versions/<version>/ and extend `MISSING_<version>`.
