# Agent API: CLI + MCP server (agent: api)

## Goal (user, 2026-10-20)
"Our app will have a CLI and an MCP server as well. I want all the features and functions of this app to be
usable by agents. They should be able to create models just like I can."

So everything a person can do in noshape must be doable by an agent: create/open/save documents, tabs,
sketches (entities, constraints, dimensions), every Part Studio feature with every option the dialog has,
variables and Variable Studios, configurations, assemblies (insert, mates, mate connectors, exploded views,
named positions, BOM), drawings (views, dimensions, annotations, tables, export), import/export, measure and
mass properties, appearance and materials, FeatureScript custom features, versions/branches/undo, and seeing
the result (rendered images from any view, plus structured geometry queries).

## Architecture (decisions)
1. **One operations layer, shared by the UI and the agent API.** Agent-facing operations must call the same
   model code the UI uses: the same feature parameter structs, the same regeneration and the same document
   commit (microversion + undo). They must never re-implement behaviour. Where the UI currently mutates the
   model inline (in `crates/app`), move that logic into a GUI-free crate or module (e.g. `crates/ops`) and
   have the UI call it. Do this incrementally, coordinating with the agent who owns that UI file (via the lead).
2. **Headless core:** a new GUI-free crate (no gpui) that opens a `.noshape` document, applies operations,
   regenerates, queries and exports. Rendering images headless uses `noshape-render`'s offscreen path.
3. **CLI:** `noshape <subcommand>` (the same binary as the app; with no subcommand it opens the GUI) or a
   separate `noshape-cli` binary built from the headless crate. Use clap. Every operation is available, with
   JSON in and out (`--json`) so scripts and agents can chain calls. Include `noshape mcp` to start the MCP
   server.
4. **MCP server:** stdio transport (and optionally streamable HTTP on localhost). Tools are generated from the
   same operation registry as the CLI, so the two can't drift. Each tool has a precise JSON schema and a
   description an agent can act on, including the units and expression syntax (`25 mm`, `#width / 2`).
   Selection is done by stable references: query tools return ids for faces, edges, vertices, sketch entities,
   features, parts, instances and mates; operations take those ids. Errors are explicit (which parameter,
   why), and so are regeneration failures (which feature, the error text Onshape would show).
5. **Live mode:** when the GUI has the document open, the CLI/MCP attach to the running app over a local
   socket (Unix socket / named pipe), so edits appear live in the window and land on the same undo stack.
   When the app isn't running, they work on the file directly. Same operations either way.
6. **Coverage is tested:** keep a registry of every UI command and feature type and a test that fails if one
   has no API operation. End-to-end tests build real models through the API (e.g. a sketched, extruded,
   filleted, patterned part; a two-part assembly with mates; a drawing with views and dimensions) and check
   the geometry (volumes, face counts, mass properties).
7. **Docs:** `docs/AGENT_API.md`, with a quick start for Claude Code / other MCP clients (config snippet),
   the tool list and worked examples.

## Order of work
1. Survey: how the UI applies edits today (crates/app → model/store), what is already GUI-free, and the
   feature parameter structs. Write the operation-layer design into this file before coding.
2. Headless crate plus the CLI core: create document, Part Studio, sketch with entities, constraints and
   dimensions, extrude, fillet, save, export STEP/STL, render a PNG. One end-to-end test.
3. MCP server over the same registry; test it with a real MCP client (the `claude` CLI on this machine with
   `--mcp-config`, run non-interactively) building a part from a prompt.
4. Broaden coverage feature by feature (all Part Studio features, then variables and configurations, then
   assemblies, then drawings, then import/export/measure), with the coverage test going green step by step.
5. Live mode (attach to the running GUI).

## Rules
- docs/handoff/TEAM-RULES.md applies, including "Resource limits".
- Build: `CARGO_TARGET_DIR=/root/target-engine` (shared with perf). Prefer building only the headless crates
  (`-p <crate>`), which avoids compiling gpui.
- Files: new crates and `docs/AGENT_API.md` are yours. Moving logic out of `crates/app` touches other
  agents' files, so tell the lead which file and function first, and keep the UI behaving identically.
- Keep the tree compiling, clippy-clean (`-D warnings`) and the tests green. The lead integrates; don't commit.

## Design (api, 2026-10-20)

### Crates
- **`crates/ops` (`noshape-ops`)**, GUI-free (model, store, kernel, units, sketch; no gpui, no wgpu): the
  operations layer. Contents:
  - `session.rs`: the `Session` (moved verbatim from `crates/app/src/session.rs`, plus `edit_tab` /
    `edit_document` helpers that do exactly what `DocModel::edit_tab` / `edit_document` do: read-only check,
    mutate, commit one microversion described `"<tab> :: <description>"`).
  - `derived.rs`: `DocStudios` / external refs (moved from `crates/app/src/derived.rs`; the one GUI
    dependency, the document-library scan, is a registered hook `set_document_scanner`).
  - `export.rs`: the Export modal's writers (`PartFormat`, `ExportSettings`, `write_export`, glTF/3MF/URDF,
    moved from `crates/app/src/part_studio/export.rs`; the modal stays in the app).
  - `studio.rs`: the Part Studio edits the UI performs inline today (feature insert/edit commit from
    `part_studio/mod.rs::accept_edit`, delete, suppress, rollback, rename, hide/show...), as plain functions
    on `&mut PartStudio`. The UI and the API both call these.
  - `doc.rs`: `Ctx` (open documents by path, current document) and `OpenDoc` (session + per-tab
    `Regenerator` + Derived cache; `regen_options` builds exactly the options the Part Studio view uses).
  - `refs.rs`: stable string ids for `Reference`s (`plane:top`, `region:F1a2b:0,15`, `face:<deflated
    base64url TopoName JSON>`...). Self-contained, valid across processes as long as the UI's own selection
    would be. Raw JSON references are accepted too.
  - `values.rs`: feature parameters as JSON, converted through the feature's own `ParamSpec` (the schema the
    dialog renders), with explicit errors (`param: "params.depth"`, "out of range: must be between ...",
    "`FOO` is not an option of `endBound`; options: ..."). JSON schemas generated from `ParamSpec`.
  - `registry.rs` + `ops/*.rs`: the operation registry. Each `Op` has group, verb, summary, agent-facing
    description (units, expression syntax, ids), typed fields (JSON schema), mutates/destructive flags and a
    handler `fn(&mut Ctx, &Args) -> Result<OpOutput, OpError>`.
- **`crates/scene` (`noshape-scene`)**, GUI-free (model + render): the viewport scene builder moved from
  `crates/app/src/scene_builder.rs` (with the viewport palette and the `GraphicsPrefs` data it reads), plus
  headless PNG rendering (`Renderer` offscreen, the same supersample/resize/encode as `Gpu::render_png`).
  The app's viewport and thumbnails use it; the API renders images with it.
- **`crates/cli` (`noshape-cli`)**: binary `noshape-cli` (clap). Subcommands are generated from the registry
  (`noshape-cli feature add --doc a.noshape --tab "Part Studio 1" --type extrude --params '{...}'`), every
  command prints JSON (`--json` input object also accepted, `noshape-cli call <tool> '<json>'`,
  `noshape-cli batch` for JSON lines), and `noshape-cli mcp` runs the MCP server. The app binary forwards
  `noshape mcp` / `noshape <op>` to the same library later (so `noshape` with no subcommand opens the GUI).

### Operations (names = MCP tool `group_verb`, CLI `group verb`)
document (create/open/info/close/set_units), tab (list/create/rename/delete/duplicate/move/folders),
feature (types/schema/add/edit/get/list/delete/suppress/rollback/rename/reorder/folders/hide), sketch
(create, add entities: point/line/rect/circle/arc/spline/slot/polygon/text..., constraints, dimensions with
expressions, delete, solve status, regions), query (parts, faces/edges/vertices with geometry descriptors and
ids, find by normal/type/feature, measure, mass properties), export (all part formats; drawings), import,
render (PNG of any standard/named view, image content in MCP), variables, configurations, assembly (insert,
mates, mate connectors, groups, patterns, exploded views, named positions, BOM), drawing (create, views,
dimensions, annotations, tables, export), history (undo/redo/versions/branches/merge/restore), appearance,
materials, Feature Studio (FeatureScript source, compile diagnostics).

### Rules the ops follow
- One edit = one microversion, same description text the UI writes, same undo stack (`Session`).
- After every Part Studio edit the studio regenerates and the reply lists each feature's status; a feature
  that fails is reported as `{feature, name, status: "Error", message}` with the message the UI shows.
- Inputs are validated against the schema before anything changes; errors name the parameter.
- Selections are reference ids from query tools; nothing is selected by index.

### Coverage test
`crates/ops/tests/coverage.rs` lists every UI command id (shortcuts.rs `ENTRIES`, the feature registry,
assembly `TOOLS`, sketch/drawing tool lists, the edit descriptions from the inventory) mapped to an op, or
to `NotApplicable("view-only: ...")` for pure view/navigation commands; it reads the app sources so a new
UI command without an op fails the test. Every built-in feature type is covered automatically by
`feature_add` (schema from `ParamSpec`), and a test checks each type's schema converts and its defaults
round-trip.

### UI refactors needed (via the lead)
1. `crates/app/src/session.rs` -> `pub use noshape_ops::session::*;`
2. `crates/app/src/derived.rs` -> `pub use noshape_ops::derived::*;` and `files.rs::init` registers the
   library scan with `noshape_ops::derived::set_document_scanner`.
3. `crates/app/src/part_studio/export.rs`: delete the moved writer code, `use noshape_ops::export::*`.
4. `crates/app/src/scene_builder.rs` -> re-export `noshape_scene::builder`; `theme::viewport` re-exports
   `noshape_scene::palette::viewport`; `graphics_prefs.rs` keeps the gpui `Global` wrapper around
   `noshape_scene::GraphicsPrefs`; `Gpu::render_png` uses `noshape_scene::png` for the encode.
5. `part_studio/mod.rs` `accept_edit` / `delete_features` / `set_suppressed` / `set_rollback` and
   `behaviors.rs` hide/show call `noshape_ops::studio::*`.
Later (assemblies/drawings): `AssemblyView::edit`'s placement copy-in and the assembly MOVE sites, the
configuration panel MOVE sites, `import_dialog.rs:225`, the release transaction.

## Drawing tools extraction plan (api, 2026-10-20; owners asb 60ec2ad1 + drw2; lead freezes each file)

Survey facts: drawing picking is CPU-only (`BuiltSheet::hit`, `ViewGeometry::snap_near`, no GPU pick buffer);
geometry (`views::compute_sheet_views`, `assemble_sheet`, `regenerate_source`) is already in noshape_model.
The coupling is `impl DrawingEditor` (a GPUI entity) + dialog structs mixing options with `Entity<InputState>`.
Target: `crates/scene/src/drawing_tools/` (scene already has render types for `Overlay`), a headless
`DrawingTools` state driven like `SketchEditor` (`pointer_move(p)`, `tool_down(p, button, mods)`,
`tool_up(p, mods)`, `key`), returning an outcome `(description, Drawing)` the UI commits via `edit_tab` and the
API via `Session::edit_tab`. Panel-opening tools (Note, Surface finish, Datum) return `Outcome::OpenPanel`.

Slices (each compiles alone; app re-exports so callers don't change):
1. Types: DimTool, PickedPoint, Pick, TableTool, EdgeRef, Tool (+ SkTool/SkCon), Sel, Drag (own Button enum),
   Overlay/SnapMark, vector helpers (drawing_view.rs:38-260, drawing_tools.rs:12-83, drawing_scene.rs:278-300).
2. State + picking: `DrawingTools{drawing, preview, sheet, sources, info, cache, geoms, built, tool, cursor,
   hover, hover_snap, selection, drag, overlay, infer_lines, woken_*, snap_mid_quad, status, zoom, tool memory}`
   as `DrawingEditor.t` with Deref; move placement, view_at, picks_of, sel_of_pick, hover filter, sel_at,
   snap_at, *_pick, tol_mm, plus a synchronous rebuild().
3. Pure builders: drawing_smart.rs whole, pure half of drawing_jog.rs, pending_annotation(_raw),
   extra_/more_pending_annotation, pending_view, extra_pending_view, drags (dragged_*, start_drag), overlays.
4. Dialog option structs (*Opts plain + UI shell): Aux, Section, Detail, Insert, Callout, TableTool, Weld,
   GDT, Hatch, Datum.
5. Headless commits: edit_sheet/edit_view/edit_table/edit_sketch/edit_styles on DrawingTools -> outcome.
6. State machine: tool_click, extra_/more_tool_click, sketch/constraint clicks, centermark, callout stack,
   gdt, weld, tables, detail, section, revision, align/rotate, painter, replicate, drag arms, delete, set_tool,
   cancel.
7. Stays in app: menus, panels (thin calls), toolbar, rendering, camera, async regeneration, clipboard, file
   dialogs, custom tables (FeatureScript; passed via SheetInfo), release log for revisions (via SheetInfo).
Then drawing ops: drawing_create, drawing_insert_view, drawing_dimension, drawing_annotate, drawing_table,
drawing_export, all by driving DrawingTools with sheet-mm clicks.

## Status log
(append dated entries here)
- 2026-10-20 (api): read brief/rules/architecture; surveyed app edit sites (≈600 commit sites, inventory
  summarized in the Design section) and the model/kernel/render APIs. Wrote the design above. Started
  `crates/ops` (session, derived, export moved; registry, refs, values, doc) and `crates/cli` (MCP server).
- 2026-10-20 (api): milestone 1 working end to end. `crates/ops` (registry, 48 ops: document 5, tab 4, feature 13,
  sketch 8, query 5, render 1, export 1, history 8, ops_list), `crates/scene` (scene builder, sketch editor,
  palette, graphics prefs, offscreen PNG; copies of the app files until the lead switches the app to them),
  `crates/cli` (`noshape-cli` with clap subcommands generated from the registry, `call`, `batch`, `tools`, `mcp`
  stdio server). Test `crates/ops/tests/e2e.rs`: sketch (corner rectangle + circle through the UI's tools,
  diameter + expression dimensions) -> extrude -> fillet 4 edges -> linear pattern x3 -> STEP (read back, volume
  checked) + STL -> undo/redo -> reopen in a new session -> render PNG (llvmpipe). Clippy `--locked -D warnings`
  clean on ops/scene/cli. Gotchas: `store.history()` is newest-first; unit-variant `Reference`s serialize as
  plain JSON strings ("Origin"); clippy's len_without_is_empty fires on lib crates (not on the app binary).
  Next: MCP test with the real `claude` CLI, coverage registry test, docs/AGENT_API.md, then variables/config,
  assemblies, drawings, import, sketch edit tools (trim/offset/fillet/patterns/use), live mode.
- 2026-10-20 (api): refactors 1-4 done (lead approved). App now re-exports: session.rs, derived.rs
  (`files.rs::init` registers the library scan; `fs_import::document_id` re-exported), part_studio/export.rs
  (only the modal + view methods remain), scene_builder.rs, sketch_mode.rs, theme::viewport/lin/rgb8,
  graphics_prefs (gpui global is now `GraphicsGlobal(GraphicsPrefs)`; the one observer in
  part_studio/mod.rs:395 updated), files::config_dir -> noshape_scene::config_dir. Diffed every app copy
  before switching: no changes since the copy. noshape-ops / noshape-scene are in [workspace.dependencies].
  **sketch_mode / scene_builder edits now go to crates/scene/src/.** Note: sketch_mode's `cfg!(test)` (sketch
  options not read from disk in tests) now only applies to noshape-scene's own tests.
  Added: coverage table `crates/ops/src/coverage.rs` + `tests/coverage.rs` (155/263 UI commands, 21 UI-only,
  108 planned; all 86 feature types round-trip), variables ops (5), docs/AGENT_API.md, MCP instructions.
  Next: step 5 refactors file by file (lead warns owners), MCP test with claude CLI.
- 2026-10-20 (api): real MCP client test passed: `claude -p <prompt> --mcp-config ... --strict-mcp-config
  --allowedTools mcp__noshape` (prompt/config in /root/api-scratch/mcp/) built an L-bracket (base, wall, 2
  through holes, inside fillet) in 29 turns / 52 s / 0 tool errors; volume 24571.416 mm³ = hand value;
  STEP + PNG written. Re-run: `cd /root/api-scratch/mcp && claude -p "$(cat prompt.txt)" --mcp-config
  config.json --strict-mcp-config --allowedTools mcp__noshape --output-format stream-json --verbose`.
- 2026-10-20 ~22:30 (api) STATE FOR A SUCCESSOR:
  * Step 5 refactors: workspace.rs add_tab/place_after/duplicate_tab DONE (docedit). Lead approved, doing now:
    part_studio/mod.rs (accept_edit/delete_features/set_suppressed/set_rollback; prepared patch
    /root/api-scratch/patch_mod.py), workspace.rs:1664 drawing tab -> docedit::insert_drawing_tab. Prepared patches
    for view_tools.rs (patch_view_tools.py), behaviors.rs (patch_behaviors.py, incl. body_of -> studio::body_of),
    menus.rs mass block (patch_menus.py): tell the lead before applying each. Each script asserts the old text.
  * Written but NOT in the build (unverified; re-add `pub mod assembly; pub mod config;` to ops/src/lib.rs and
    `mod assembly; mod configuration;` + `v.extend(...)` to ops/src/ops/mod.rs, then clippy + tests):
    ops/src/config.rs + ops/src/ops/configuration.rs (Configurations panel), ops/src/assembly.rs +
    ops/src/ops/assembly.rs (insert, mate, entities, connectors, transform, set_state, delete). Assembly e2e test
    waiting in /root/api-scratch/e2e_assembly.rs (append to crates/ops/tests/e2e.rs once ops compile).
  * After verification, switch assembly_view.rs (skb) and config_panel.rs (psb) to the shared fns, telling the
    lead before each. Then drawings: plan the DrawingEditor tool extraction (asb 60ec2ad1 + drw2 own drawing_*),
    file by file with the lead freezing each. Then second real-client MCP test (assembly with mates + drawing).
  * Lint rule: run `cargo clippy --locked --workspace --all-targets -- -D warnings` before every report.
    Never leave uncompiled modules registered; never use pgrep -f in Monitor (it matches itself).
- 2026-10-20 23:xx (api) HANDOFF (moved off the Motile account). Exact state:
  DONE (in the tree, compiled earlier unless noted):
  * crates/ops, crates/scene, crates/cli; 51 ops registered (document, tab, feature, sketch, variable, query,
    render, export, history, ops_list). Tests: ops lib, tests/e2e.rs (part), tests/coverage.rs all green.
  * Refactors 1-4 (session, derived, export writers, scene_builder, sketch_mode, theme viewport, graphics prefs)
    and step 5 workspace.rs (add_tab, place_after, duplicate_tab).
  * JUST APPLIED, lint NOT yet confirmed: part_studio/mod.rs (accept_edit, delete_features, set_suppressed,
    set_rollback -> noshape_ops::studio) and workspace.rs:1669 Create Drawing -> docedit::insert_drawing_tab.
    FIRST THING: run `OCCT_ROOT=/root/occt CARGO_TARGET_DIR=/root/target-engine cargo clippy --locked --workspace
    --all-targets -- -D warnings`; fix anything in these two edits (last known foreign error:
    toolbar_customize.rs:240, ui's).
  UNCOMPILED (written, NOT registered, so the tree is clean):
  * ops/src/config.rs + ops/src/ops/configuration.rs; ops/src/assembly.rs + ops/src/ops/assembly.rs.
    To enable: add `pub mod assembly;` and `pub mod config;` to ops/src/lib.rs, `mod assembly; mod configuration;`
    and `v.extend(configuration::ops()); v.extend(assembly::ops());` in ops/src/ops/mod.rs; build with
    `cargo clippy -p noshape-ops --all-targets -- -D warnings`; then append /root/api-scratch/e2e_assembly.rs to
    crates/ops/tests/e2e.rs and add config tests. Update coverage.rs entries (assembly:Mate etc. -> Op) and
    lower PLANNED_CEILING in tests/coverage.rs.
  PREPARED PATCHES (python, each asserts the old text; tell the lead before each file):
  * /root/api-scratch/patch_view_tools.py (start_rename -> studio::rename_feature)
  * /root/api-scratch/patch_behaviors.py (set_visibility, show_everything, body_of -> studio::*)
  * /root/api-scratch/patch_menus.py (mass block -> analysis::studio_mass_properties)
  * (patch_mod.py already applied)
  OPEN APPROVALS (all granted by the lead 2026-10-20): part_studio/mod.rs (done); workspace.rs:1664 drawing tab
  (done); drawing tools extraction (plan above; asb 60ec2ad1 + drw2 own drawing_*; lead freezes each file; start
  after part_studio/assembly/config switches); assembly_view.rs (skb) and config_panel.rs (psb) switches once
  their ops are verified with tests (tell the lead before each).
  NEXT STEPS: lint check above -> enable + test config/assembly ops -> view_tools/behaviors/menus patches (tell
  lead each) -> switch assembly_view.rs and config_panel.rs (tell lead) -> drawing slices 1-7 -> drawing ops ->
  second real-client MCP test (assembly with mates + drawing; config at /root/api-scratch/mcp/config.json,
  binary /root/api-scratch/bin/noshape-cli, rebuild with `cargo build -p noshape-cli`) -> import ops, sketch edit
  tools (trim/offset/fillet/patterns/use via SketchEditor), analysis ops -> live mode. Keep docs/AGENT_API.md and
  coverage.rs current.
- 2026-10-20 21:35 (api, successor on Stablecog): workspace clippy run. My part_studio/mod.rs + workspace.rs edits
  are clean. Fixed scene/builder.rs:124 type_complexity (kern's mesh-cap cache -> `type CapLines`). Foreign errors
  reported to the lead: app drawing_props.rs:663 (useless conversion), units_prefs.rs:78/293 (type complexity,
  `#[test]` recursion limit). Now: enabling config/assembly ops.
