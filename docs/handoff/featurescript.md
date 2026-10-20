# FeatureScript (agent "fs")

Goal: Feature Studios take real FeatureScript and behave like Onshape's; custom features written in
FeatureScript run in Part Studios. Replaces the Rhai custom features (`crates/model/src/custom/`).

## Status (19 Oct ~18:00)

Build only in /root/target-shared (TEAM-RULES "Build"); prefer `cargo check` / single tests. Green:
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
4. opExtractSurface ignores `tangentPropagation`; skConicSegment ignores low/highParameter; opMoveFace always
   moves rigidly (Onshape offsets planar faces moved along their normal, like our Move face feature).
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
