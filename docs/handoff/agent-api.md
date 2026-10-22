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
- 2026-10-20 22:30 (api): config + assembly ops ENABLED (lib.rs `pub mod assembly; pub mod config;`, ops/mod.rs
  registers configuration + assembly). Clippy -p noshape-model -p noshape-ops clean. assembly_with_mates e2e passes.
  Added `config_activate` (the Configurations box's chosen values; `OpenDoc::active_config`, view state, used by
  every default regenerate and by feature_edit), `config_list.active`, `config::{kind_dim, value_error, current}`
  (copies of config_panel.rs `config_value_error` / config_var_dialog `kind_dim` / mod.rs `current_config`; the app
  switches to them with the config_panel switch). Found: sketch dimensions couldn't use configuration or Variable
  Studio variables (`display.variables` holds only the studio's own). Fixed for ops via new model fn
  `Configuration::variable_values` (extracted from regen/mod.rs, same behaviour); the APP has the same gap in
  part_studio/sketch.rs (dimension edits use display.variables) -> reported to the lead. Test `configurations`
  in e2e.rs written; full `cargo test -p noshape-ops` running (log /root/api-scratch/test.log). Coverage: assembly
  Mate/Tangent/Width/Insert dialog -> ops, PLANNED_CEILING 104. AGENT_API.md + mcp_instructions.md updated.
  Lead's clippy report: part_studio/mod.rs:19 + config_panel.rs:2671 (`var_table_drag_source`, unused) are NOT
  from the ops switch; told the lead.
  NEXT: confirm tests -> patches view_tools/behaviors/menus (lead told) -> config_panel.rs + assembly_view.rs
  switches -> drawing slices.
- 2026-10-20 23:45 (api): `cargo test -p noshape-ops` all green (lib 8, coverage 3, e2e 3: part, assembly_with_mates,
  configurations). APPLIED (lead told 22:30): view_tools.rs start_rename, behaviors.rs set_visibility/show_everything/
  body_of, menus.rs mass block -> noshape_ops (patch_*.py); config_panel.rs (add input, add variable input, rename/
  delete input, variable default, rename/delete/add option, configured values x3, configure parameter, reorder,
  config_value_error) + config_var_dialog.rs (kind_dim re-export, Variable dialog add/edit) -> noshape_ops::config
  (patch_config_panel.py; new ops fns push_variable_input, edit_variable_input; config_edit_input takes `range`).
  App clippy after these: nothing from my files (only compare_mode.rs dead code, someone's WIP; model
  drawing/tables.rs:233 needless struct update, drawing agent). assembly_view.rs (edit/fold_solved, insert_part,
  set_fixed/hidden/suppressed, delete_instances, delete_mate, accept_mate -> noshape_ops::assembly;
  patch_assembly_view.py) applied 23:45, clippy running.
- 2026-10-20 23:50 (api): assembly_view.rs switch verified (app clippy: nothing from my files; only compare_mode.rs
  dead code = ui2's Compare WIP). All step-5 switches + config/assembly switches DONE.
  PENDING (lead FYI 23:46): ui2 adds a Workspace protection flag to ops/src/session.rs (blocks model edits, allows
  versions/branches/merges). When it lands: ops `document_protect` / `document_unprotect`, map the session's protected
  error to OpError code `protected` (registry::run / From<anyhow>), document_info shows it, e2e test. ui2 is editing
  workspace.rs + part_studio/mod.rs (Compare): re-read before touching them.
  NEXT: drawing slice 1 (types) -- waiting for the lead to freeze drawing_view.rs, drawing_tools.rs, drawing_scene.rs.
- 2026-10-21 00:10 (api): DRAWING SLICE 1 DONE (lead froze drawing_view/tools/scene; script /root/api-scratch/slice1.py).
  New `crates/scene/src/drawing_tools/` {types.rs: Button, DimTool, PickedPoint, Pick, TableTool, EdgeRef, Tool, Sel,
  Drag (Pending.button: scene `Button`); sketch.rs: SkTool, SkCon (`needs` now pub); overlay.rs: Overlay + SnapMark
  + to_render; geom.rs: sub/dot/len/add2/norm/in_sweep/dim_offset/auto_orientation/intersect}. App re-exports:
  drawing_view.rs (`pub use ...{Button, DimTool, Drag, Pick, PickedPoint, Sel, TableTool, Tool}` + `drag_button` /
  `mouse_button` converters; ColoredHandles + DIALOG_TOP stay), drawing_tools.rs (`pub(crate) use ...geom::*`; 4
  button sites), drawing_scene.rs (Overlay, SnapMark), drawing_sketch.rs (SkTool, SkCon; tiny edit, file was idle).
  Agents' uncommitted changes in those files (Panel::Cell note, failed_sources, insert asm_position) untouched.
  Clippy -p noshape-scene -p noshape: nothing from these files. NEXT: sketch edit tools + import ops while the lead
  sets up slice 2.
- 2026-10-21 (api, new successor on Motile): read brief/rules/log; tree state as logged.
  STEP 1 written (clippy -p noshape-ops clean; e2e tests written, run pending): new `crates/ops/src/ops/sketch_edit.rs`
  (16 ops: sketch_trim/extend/split (clicks or a trim `path`), spline_point, fillet/chamfer (one tool session: first
  dimensioned, rest equal; size via the value box = apply_dimension), offset (`toward` point flips the arrow), mirror,
  linear/circular_pattern (+ edit by `pattern` id), transform, use, intersection, text (+ edit), insert_dxf,
  insert_image); `sketch_draw` gained elliptical_arc. New shared module `crates/ops/src/sketch_use.rs` = the app's
  part_studio/sketch.rs external_entity / use_hovered / curved_face / reference_plane / hovered_edge_refs /
  update_use_preview / intersect_hovered logic, GUI-free (`Model{context, display, frame}`); the APP STILL HAS ITS COPY
  -> switch part_studio/sketch.rs to it (tell the lead; owner psb/skb). scene sketch_mode.rs: `DXF_UNITS`,
  `insert_dxf_file`, `load_image` (same messages as the app's insert_dxf_file / load_image_file; app switch pending);
  scene's `image` dep gained jpeg/gif/bmp features (app already enables them: no Cargo.lock change).
  Sketch ops now regenerate up to the sketch (`OpenDoc::regenerate_with`, upto = index+1, editing_sketch) like sketch
  mode; tool defaults via `apply_unit_defaults`; sketch_constrain/sketch_dimension accept model edge/vertex/face/plane
  ids (linked construction entity, pierce for Pierce) like the UI's tools; sketch_get lists patterns/texts/images.
  Coverage: 17 sketch commands -> ops, PLANNED_CEILING 87.
  STEP 2 written: document_protect/unprotect (optional `branch`), document_info `protected` + `branch`, OpError code
  `protected` (and `read_only`) from the session's messages (error.rs From<anyhow>). Test `workspace_protection`.
  SLICE 2 prepared (scene only, not yet used by the app): `drawing_tools/pick.rs` `SheetPick<'a>` (borrowed view:
  display, sheet, built, geoms, tool, hover, cursor, zoom, snap_mid_quad) with hover_at, placement, view_at, picks_of,
  sel_of_pick, image_at, sel_at, snap_at, hovered_edge, line/arc/circle_pick, any_point, pick_sheet, axis_pick,
  raw_line_pick, circle_at, smart_pick (verbatim from the app). DECISION: no Deref-to-DrawingTools (autoderef borrows
  all of `self`, which breaks the app's many disjoint field borrows); the app keeps its fields and gets
  `fn pk(&self) -> SheetPick<'_>`, each moved method becoming a one-line delegation (callers unchanged).
- 2026-10-21 (api): STEP 1 + STEP 2 VERIFIED. `cargo test -p noshape-ops` all green (lib 9, coverage 3, e2e 9: + sketch_trim_extend_split,
  sketch_fillet_chamfer_offset, sketch_mirror_patterns_transform, sketch_use_intersection_and_model_references,
  sketch_text_and_imports, workspace_protection). Clippy -p noshape-ops -p noshape-scene -p noshape-cli --all-targets clean
  (the lead's too_many_arguments report on sketch_use.rs was a transient state, fixed by putting the plane frame in `Model`).
  Offset `toward` = the side whose trial offset passes nearer the point (arrow dot product was wrong for closed loops).
  Docs: AGENT_API.md (sketch edit tools, protection), mcp_instructions.md.
  WAITING ON THE LEAD (asked 2026-10-21): freeze for slice 2 = /root/api-scratch/slice2.py (signature-matched, prints
  "patched"; `--dry` checks): drawing_view.rs (new `pk()` before current_sheet ~538; bodies of picks_of ~959, sel_of_pick
  ~980, tol_mm ~1195, the hover computation inside update_hover ~1199-1250, placement ~1258, view_at ~1263),
  drawing_tools.rs 304-460 (sel_at, snap_at, hovered_edge, line/arc/circle_pick, any_point, pick_sheet), drawing_more.rs
  62-107 (axis_pick, raw_line_pick, circle_at), drawing_smart.rs 76-84 (smart_pick), drawing_images.rs 106-109 (image_at).
  And part_studio/sketch.rs 718-955 (hovered_edge_refs, update_use_preview, use_hovered, curved_face, reference_plane,
  external_entity, intersect_hovered) = /root/api-scratch/patch_sketch_use.py -> noshape_ops::sketch_use.
  After applying: possibly unused imports (views::SnapKind/EdgeGeom in drawing_tools.rs, sk_feat in sketch.rs) -> remove.
- 2026-10-21 (api): DRAWING SLICE 2 DONE + part_studio/sketch.rs Use/Intersection switch DONE (lead approved + froze asb/drw2/
  psb/ui). Before applying, compared every frozen body with the scene copy (normalized): only one new hunk since the copy,
  update_hover's SK7 arm (smart dimension picks drawing sketch entities) -> added to SheetPick::hover_at; the rest equal.
  Applied slice2.py + patch_sketch_use.py; removed the now-unused app leftovers (EdgeGeom/SnapKind imports in
  drawing_tools.rs, DrawingEditor::image_at, PartStudioView::hovered_edge_refs/curved_face). Pre-patch copies:
  /root/api-scratch/pre-slice2/. `cargo clippy --locked -p noshape --all-targets -D warnings` clean (also covers check).
  In-app (Xvfb :114, --demo-drawing-full): Use preview + click projects a hole edge; Intersection on the Front plane gives the
  top face's section line (and "That face does not cross the sketch plane" for a bore touching a face plane); drawing
  Select hover highlights the view under the pointer, click selects the view; dimension tool edge hover + vertex snap
  marker; a 2-click linear dimension snaps and places. Told the lead "slice 2 done".
  NEXT: slice 3 (pure builders: drawing_smart.rs, drawing_jog.rs pure half, pending_annotation(_raw),
  extra_/more_pending_annotation, pending_view, extra_pending_view, dragged_*, start_drag, overlays) with the same
  borrowed-view pattern; send the lead the file/line list first.
- 2026-10-21 (api): lead lifted the slice-2 freeze. Snap near a filleted corner: filed live REQUESTS "api (10-21 a)" (does
  Onshape's drawing Dimension snap to the fillet's tangent vertex like ours (98), or give 100? snap radius px?). If Onshape
  differs: fix in scene drawing_tools/pick.rs `snap_at` and tell asb. The drawing engine's snap code is NOT in bundle-raw.
  SLICE 3 written in scene (not compiled yet: kern's kernel bridge was mid-edit, read_iam_tree_ffi):
  drawing_tools/{smart.rs (smart dimension: cylinder_silhouettes, typed_linear_click, smart_takes, smart_click,
  angular_from_lines, linear, point_to_line, smart_dimension; free fns same/view_of, type Refs), jog.rs (seg_dist,
  dimension_frame, resolved_annotation, extension_line_at, jog/attach/centermark grips, dragged_centermark, reattached,
  dragged_jog, dragged_annotation), pending.rs (pending_annotation(+raw), extra_/more_pending_annotation, ordinate_normal,
  section_position, extra_pending_view, next_target_set, next_datum_label, sketch_dim_preview; dialog/tool inputs as plain
  `ToolValues`)}; geom.rs gained `mul`; SheetPick gained drawing, selection, sources. Bodies extracted mechanically
  (/root/api-scratch/slice3/extract.py) then 9 hand edits for ToolValues.
  App patch ready: /root/api-scratch/slice3/apply.py (dry run OK): pk() gets the 3 fields; new DrawingEditor::tool_values();
  delegations in drawing_tools.rs (pending_annotation, dragged_annotation), drawing_extra.rs (ordinate_normal,
  section_position, extra_pending_view, next_target_set, next_datum_label), drawing_smart.rs (6), drawing_jog.rs (9); drops
  pending_annotation_raw, extra_/more_pending_annotation, sketch_dim_preview, linear, point_to_line, centermark_arms (all
  callers were inside moved bodies). Files to freeze: drawing_view.rs, drawing_tools.rs, drawing_extra.rs, drawing_more.rs,
  drawing_sketch_dim.rs, drawing_smart.rs, drawing_jog.rs. Deferred to slice 4 (they read dialogs): pending_view
  (insert/aux/section/detail options), start_drag (note grips, extra_start_drag, inspection_at), the overlays.
- 2026-10-21 (api): v0.4.3 freeze + GO. During slice-3 prep I broke the app for a while: I added fields
  (drawing/selection/sources) to the shared `SheetPick` before updating the app's `pk()`. Fixed (pk() line in drawing_view.rs).
  RULE FOR SUCCESSORS: a shared-type change and its app call site land in the same step; `cargo check -p noshape` before saying done.
  Freeze verification: workspace clippy (--locked --workspace --all-targets -D warnings) clean; `cargo test -p noshape-ops` green 3x
  (lib 9, coverage 3, e2e 9). Integration's `sketch_fillet_chamfer_offset` failure (e2e.rs:336) was a snapshot taken before
  the offset `toward` fix; the test is deterministic (gaps 13 vs 17 mm, 3 vs 7 mm). A transient wgpu "Validation Error" in
  `sketch_extrude_fillet_pattern_export_render` came from another agent's in-progress section-cap render work, gone since.
  My files shipped in v0.4.3 (5d5808e) unchanged.
  SLICE 3: app sources re-extracted after GO = identical to the scene copies. apply.py dry run OK (pk() step removed: done).
  Freeze asked of the lead (asb + drw2): drawing_tools.rs, drawing_extra.rs, drawing_more.rs, drawing_sketch_dim.rs,
  drawing_smart.rs, drawing_jog.rs (drawing_view.rs not needed). After applying: drop now-unused leftovers the compiler
  names (likely drawing_smart.rs free fns same/view_of + local vector helpers, drawing_jog.rs CentermarkArms/seg_dist,
  imports), then check + clippy -p noshape + quick in-app drawing check (smart dimension, grips/drags, previews).
- 2026-10-21 (api): SLICE 3 APPLIED (lead froze asb/drw2 files; no new hunks since the dry run; backups /root/api-scratch/pre-slice3/).
  apply.py's forwarding generator mis-parsed signatures whose return type holds a tuple (5 calls got type names as
  arguments) -> fixed by hand at once (smart_dimension, angular_from_lines, jog/centermark/attach_grip_at). LESSON: compile
  a generated patch before reporting; parse params by paren matching. Then removed what the move left unused: drawing_smart.rs
  is now 3 forwarding methods (typed_linear_click, smart_takes, smart_click); drawing_jog.rs seg_dist/CentermarkArms;
  drawing_extra.rs next_target_set/ordinate_normal/section_position; unused imports. tool_values() fills datum_label with
  DrawingEditor::datum_dialog_label() (keeps drawing_datum.rs untouched; same fallback). App clippy: nothing from these files
  (only fs's WIP fs_profile.rs:17/76 -> reported to the lead).
  In-app check (Xvfb :114, --demo-drawing-full, fresh build): smart Dimension on a circle -> hover highlight + center snap,
  live "Ø28" preview following the pointer, placed; dragging the placed diameter swings its leader (dragged_annotation);
  smart Dimension on a straight edge -> "100" length preview. Told the lead "slice 3 done".
  NEXT: slice 4 (dialog option structs: Aux, Section, Detail, Insert, Callout, TableTool, Weld, GDT, Hatch, Datum ->
  plain *Opts in scene, the app's dialogs keep their Entity<InputState> fields around them) together with what it unblocks:
  pending_view, start_drag (note grips, extra_start_drag, inspection_at), the overlays. Survey + send the freeze list first.
- 2026-10-21 (api): SLICE 4 started. Design: no dialog struct changes; plain option structs in scene built from the dialogs on
  demand (like SheetPick/ToolValues). `drawing_tools/opts.rs`: ViewOptions {insert: InsertOpts, aux, section, detail} +
  SheetPick::pending_view(&ViewOptions) (extracted verbatim from the app), insert_sketch_frame, apply_section_options,
  apply_detail_options; HatchOpts (parse_auto, scale/angle_value, regions); table_size + general_table (TABLE_COL_W);
  GDT_TYPES + GdtRowValues + gdt_values. Weld/Callout need nothing: the API builds WeldSpec / (String, CalloutStyle) directly.
  MISTAKE: I registered opts.rs before it compiled (derive(Default) on InsertOpts: Simplification/AsmPosition have no Default)
  -> clippy broke for everyone; fixed (manual Default = the app's InsertViewState values) and UNREGISTERED it until verified.
  RULE: a new module is registered only by a command that compiles it and unregisters it on failure
  (/root/api-scratch/slice4/mod.clean.rs is the clean mod.rs). Also: never `pkill -f` a cargo pattern (it matched my own shell).
  opts.rs verified (scene clippy clean) and registered 10-21 ~06:00. App patch ready: /root/api-scratch/slice4/apply.py (dry run OK;
  params parsed by paren matching): drawing_tools.rs pending_view -> pk().pending_view(&view_options()) + new view_options();
  drop apply_section_options (drawing_section.rs 114-129), apply_detail_options (drawing_detail.rs 64-76),
  insert_sketch_frame (drawing_insert.rs 559-563); drawing_hatch.rs parse_auto removed, HatchDlg::opts(cx) + scale/angle/regions
  forward (61-95); drawing_table_tool.rs COL_W removed, table_tool_size/table (49-66) forward; drawing_gdt.rs GDT_TYPES ->
  `pub use` of scene's, gdt_values (183-217) builds GdtRowValues. At registration the app check failed only in asb/drw2 WIP
  (new drawing_label.rs missing imports; drawing_aux.rs:451 Fill2.pick not yet in the model) -> not from opts.
- 2026-10-21 (api): slice-4 freeze list re-verified and sent to the lead (the earlier message didn't reach them): drawing_tools.rs
  (pending_view 377-486, view_options() at 364), drawing_section.rs 114-129, drawing_detail.rs 64-76, drawing_insert.rs 559-563,
  drawing_hatch.rs 56-95, drawing_table_tool.rs 16-17 + 49-66, drawing_gdt.rs 22-37 + 183-217. On "frozen": re-extract/compare,
  apply /root/api-scratch/slice4/apply.py, app clippy, in-app check, then continue WITHOUT ending the turn: drawing ops on
  SheetPick/smart -> second real-client MCP test (assembly + drawing) -> analysis ops -> AGENT_API.md.
- 2026-10-21 (api): SLICE 4 APPLIED (frozen by the lead; asb now 4ade3a23). pending_view in the app had gained AuxRef.letter_at,
  which asb had already mirrored into opts.rs -> nothing lost. Backups /root/api-scratch/pre-slice4/. Leftovers removed:
  drawing_detail.rs DrawingView import, drawing_table_tool.rs TableKind import, drawing_extra.rs extra_pending_view.
  App clippy: nothing from my files (foreign: ui.rs:1100 recursion limit in #[test], assembly_standard.rs:18, drawing_aux.rs:443,
  compare_mode.rs:737, part_studio/sketch.rs:2461, smoke.rs:155).
  Also in this freeze: drawing_tools.rs Dimension click dispatch -> scene SheetPick::dimension_click / dimension_takes_more
  (smart.rs), applied through /root/api-scratch/guard.sh (keeps an edit only if the build finishes with no diagnostics in
  the edited files; reverts otherwise; retries when other agents' WIP breaks the build).
  DRAWING OPS written, NOT registered yet: crates/ops/src/drawing.rs (SheetModel::build = sources (+versions) + exact view
  geometry + assemble_sheet; pick() -> SheetPick at API_ZOOM 20 px/mm with hover; sheet_info (release log meta "releases");
  finish_edit = the editor's commit prep; new_drawing = Create Drawing's background job; template_rows COPY of the app's
  drawing_create.rs (switch the app to it at its next freeze)); crates/ops/src/ops/drawing.rs (drawing_templates, _create,
  _get (views with edges in sheet mm, annotations, tables), _insert_view, _projected_view, _dimension (kinds smart,
  point_to_point, point_to_line, line_to_line, radius, diameter, angle, angle_3_point, arc_length)). Next: drawing export
  extraction (drawing_export.rs write_export_with background job -> ops) needs its own freeze.
- 2026-10-21 (api): dimension click dispatch shared (scene SheetPick::dimension_click / dimension_takes_more; app
  drawing_tools.rs calls them; now-unused app forwarders removed: drawing_smart.rs is docs-only, DrawingEditor::arc_pick gone)
  -> verified by guard.sh ("CLEAN in my files"). guard.sh now waits for the cargo lock (flock .cargo-lock) before editing.
  DON'T kill a running guard: `cargo clippy`'s `cargo check` grandchild survives as an orphan (I had to kill 2 by PID).
  Analysis: crates/ops/src/analysis_tools.rs = the Part Studio analysis tools' computation (from part_studio/
  analysis_tools.rs recompute_analysis_tool, minus GPU/colors; AnalysisKind/Options/Result, CombKind) -> app switch at
  psb's next freeze; ops/src/ops/analysis.rs (analysis_curvature/deviation/connection/dihedral/interference).
  Second MCP test prepared: /root/api-scratch/mcp2/{config.json,prompt.txt} (hinge: block + pin, revolute, drawing with
  views + Ø and 80 dims). Drawing e2e test draft: /root/api-scratch/e2e_drawing.rs (append once ops are registered).
  AGENT_API.md: drawing + analysis sections added.
- 2026-10-21 ~09:20 (api): REGISTERED + VERIFIED (guard CLEAN): drawing ops (templates, create, get, insert_view,
  projected_view, dimension, export), analysis ops (curvature, deviation, connection, dihedral, interference), modules
  ops::drawing, ops::drawing_export (= the app's Export Drawing writers: DrawingFormat, ExportSettings, consts,
  parse_sheet_range, export_sheets_of, zipped, export_files, bundle; raster = caller rasterizes the SVG), ops::analysis_tools;
  scene png::decode_rgb. Coverage planned 72. Tests drawing_views_and_dimensions + analysis_tools green.
  drawing_get: a closed circle has a seam vertex at both ends (not `ends[0] == None`) -> classified by coinciding ends.
  NOTE for pick.rs snap_at (and the app before): Shift+Q mid/quadrant snaps treat a circle as closed only when
  `ends[0].is_none()` -> full circles with a seam vertex get a "midpoint" snap; check vs live with the snap capture.
  App switch patches READY (need freezes): /root/api-scratch/patch_drawing_export.py (drawing_export.rs, asb/drw2),
  /root/api-scratch/patch_analysis_tools.py (part_studio/analysis_tools.rs, psb). Also pending: drawing_create.rs
  template_rows -> ops::drawing::template_rows; workspace.rs create_drawing_tab job -> ops::drawing::new_drawing;
  document_set_units op -> Session::set_units (someone added it 10-21 ~09:00: pins unitless sketch dimensions).
  Slice-4 in-app check still pending: the app didn't build (foreign WIP part_studio/sketch.rs:2473 E0500).
- 2026-10-21 ~10:00 (api): SECOND REAL-CLIENT MCP TEST PASSED (/root/api-scratch/mcp2/run1.jsonl): claude built block + pin,
  revolute mate (1 DOF left), ISO A3 drawing (front/top/side/iso), Ø10 + 80 dims: 28 calls, 1 error, 72 s. Gaps it found ->
  fixed: no part rename (new ops part_rename / part_set_properties; shared studio::rename_part / set_part_properties /
  PartPropertyValues = the app's parts-list Rename and Properties save -> app switch at psb's next freeze: behaviors.rs
  start_part_rename, menus.rs Properties on_save); drawing_get now gives base views' orientation, projected views' side
  (projection) and each annotation's shown text. document_set_units already switched to Session::set_units by its owner.
  Slice 4: applied + verified (compile, my files lint-clean); told the lead "slice 4 done" with the in-app check still
  pending (app build broken by foreign WIP part_studio/sketch.rs:2473 E0500). Do the in-app check when the app builds:
  Insert / Projected / Section / Detail / Auxiliary previews, hatch, table, GTol.
- 2026-10-21 ~10:15 (api): SLICE 4 IN-APP CHECK DONE (Xvfb :114, --demo-drawing-full, build 09:43 with slice 4): Projected view
  preview follows the pointer; Section view (vertical) placed "SECTION C-C" with cut hatching; Detail view preview "DETAIL D"
  scale 2:1 from the dialog follows the pointer; Geometric tolerance 0.2 / datum A placed (⌖|0.2|A); Table 3 rows x 4 cols +
  header placed (cell editor opens). Not exercised: hatch, auxiliary view.
- 2026-10-21 ~12:30 (api): THIRD REAL-CLIENT MCP TEST (/root/api-scratch/mcp3: prompt.txt, run1.jsonl, plate.pdf) PASSED:
  claude built Plate 100x60x10 with 2 Ø8 holes + Bolt Ø8x30 (renamed parts), assembly Plate + 2 Bolts with cylindrical
  mates (4 DOF), ISO A3 drawing (front/top/side/iso) with Ø8, 100, 60, 60, PDF export: 42 calls, 1 error, 105 s.
  COMPARED IN THE APP (copy of clamp.noshape, build 09:43): parts list Plate/Bolt; app Mass properties Plate 58994.69 mm³
  = API; assembly Plate fixed, mates "Bolt 1 in hole -X"/"+X", dragging a bolt only slides it along its hole axis; the
  drawing tab shows the same views/dims as the API's PDF (the PDF hides "----" placeholders, as the app's export does).
  SAME PLATE BUILT BY HAND IN THE APP UI (Part Studio 2: sketch, center rectangle, Dimension 100/60, circles, Ø8 x2, 30/30,
  Extrude 10): 58994.69 mm³, 15501.593 mm² = the MCP-built Plate exactly.
  Gaps from the run, fixed (guard): query part lookup now matches the parts list's (renamed) names; drawing_get emits other
  drawn edges (iso ellipses, splines) as sampled `curve`s. Example session for AGENT_API.md: /root/api-scratch/example/
  session.py (runs through `noshape-cli call`, writes session.md). New: ops/src/reference.rs (generated reference, not yet
  registered).
- 2026-10-21 ~13:30 (api): GAP FILLING (item 2, worst first). Written, registering one at a time through
  /root/api-scratch/guard2.sh (= guard.sh that also removes new files on revert) via /root/api-scratch/chain.sh
  (log /tmp/api/chain.log):
  1. import_files (ops/src/import.rs + ops/import.rs; the Import dialog: CAD/mesh files, combine, CAD Imports folder) +
     generated reference (ops/src/reference.rs, `noshape-cli docs`, coverage test agent_api_reference_is_current).
  2. materials/appearances (ops/src/appearance.rs, material_data.rs, ops/appearance.rs: material_list, part_set_material,
     part_set_appearance, part_remove_appearance, part_appearances).
  3. assembly_pattern (linear / circular / mirror / part pattern / replicate, insert + edit; shared
     ops::assembly::commit_pattern, pattern_description, pattern_defaults, seed_mate), assembly_dissolve_pattern,
     assembly_bom (+csv), assembly_bom_settings (one edit per change, the panel's texts), assembly_interference (shared
     ops::assembly::interference_rows); assembly_delete also deletes patterns/groups/connectors; assembly_list lists
     patterns. delete_mate_description now names patterns and connectors (app bug: deleting a pattern logged "Delete : ").
  App switches to queue (need freezes): assembly_panels.rs accept_pattern/seed_mate/new_pattern_session defaults ->
  ops::assembly; assembly_interference.rs interference_rows -> ops::assembly::interference_rows.
  Rule (self): never edit registered ops files directly while a guard is pending; stage in /root/api-scratch and apply
  through the guard (I briefly had unverified assembly ops in the tree at ~13:20 and backed them out).
  ~14:00: stage 2 written (ops/src/ops/assembly/features.rs: assembly_group, assembly_relation, assembly_mate_connector,
  assembly_replace, assembly_rename, assembly_set_feature_state; shared ops::assembly commit_group, check_relation,
  commit_relation, relation_default_ratio, commit_connector, toggle_mate_suppressed, replace_instance, replace_message;
  mates take {"connector": "Mate connector 1"}), chain2.sh. Stage 3 written (ops/src/ops/assembly/positions.rs:
  assembly_named_position (add/update/apply/duplicate/delete/rename/move/set_value), assembly_named_position_drivers,
  assembly_exploded_view, assembly_explode_step; shared driven_dofs, capture/apply_named_position...), chain3.sh
  (also re-runs fix_names_curves, which the guard had reverted for a clippy is_multiple_of lint in my code).
  App switch ready for psb's freeze: /root/api-scratch/patch_part_rename_props.py (behaviors.rs start_part_rename,
  menus.rs Properties on_save -> ops::studio), dry-run OK on the current tree (keeps psb's hunks).
  Drawing gaps (section/detail/aux views, notes, centerlines, tables, GD&T) need slices 5-6 (the click state machine
  in drawing_tools.rs tool_click and the dialogs' click handlers) -> next, scene side first.
- 2026-10-21 ~15:00 (api): LANDED (guard-clean): import_files (+ generated reference / `noshape-cli docs` / coverage
  test agent_api_reference_is_current) and materials/appearances; ops tests green (e2e 13). FOUND + FIXING: the CLI
  panicked at start ("command name `part` is duplicated": appearance ops put `material` between `part` groups and the
  CLI/reference only merged adjacent groups) -> registry::groups() (each group once) + cli test command_tree_is_valid
  (clap debug_assert) via guard (fix_groups.py). Assembly stage 1/2 failed once (field descriptions built with
  format! must be &'static) -> fixed, re-queued in chain4.sh with stage 3 and the names/curves fix.
  Lead (I6, qa5: no import op in CLI/MCP): import_files covers STEP/IGES/Parasolid/ACIS/JT/Rhino/PVZ/BREP,
  native parts + assembly zips, STL/OBJ/3MF/glTF/GLB; qa5 most likely hit the CLI start-up panic. New tests queued
  (chain5.sh): import_every_format (export each part format, import back; B-rep keeps the volume),
  import_options_and_errors (combine, keep_files=false, mesh units, non-native zip / unknown type / missing file).
  Native assembly success path can't be tested here: no SolidWorks/Inventor/NX sample files exist on the server.
  Also queued: featurestudio_get / featurestudio_edit / featurestudio_commit (+ e2e custom feature used in a Part Studio).
  Drawing click state machine (slices 5-6): drw2 is changing the Section tool in drawing_tools.rs now -> needs lead timing.
- 2026-10-21 ~15:40 (api): CLI START-UP FIX LANDED (registry::groups(), cli test command_tree_is_valid); CLI works again.
  Import probe (export each format, import back, /root/api-scratch/import_probe/probe.py): Parasolid, ACIS, STEP,
  IGES, JT exact volume; PVZ / glTF / GLB / 3MF within 1e-4; STL needs matching units (unitless). BUG FOUND: OBJ export
  wrote no faces (ops/src/export.rs merged the parts' meshes without their face ranges; write_obj_mesh writes per face)
  -> affects the app's Export dialog too; fix + "file without extension gets the format's" queued (fix_export.py,
  chain7.sh after chain6.sh). Rhino export needs openNURBS (not in this build). Assembly stage 1 now applies as a
  patch over another agent's ops/src/assembly.rs hunks (mate_preview_moving, hidden new mates).
  ~16:00: stage 1 failed again: the model's PatternKind gained `centered` (Linear/Circular) and Replicate
  `scope`/`scope_refs` (assembly owner, in progress). assembly_pattern now takes `centered`, `scope`, `scope_refs`;
  the default scope moved to shared ops::assembly::default_replicate_scope (app's replicate_scope to switch to it).
  Re-queued as chain8.sh after chain7.sh (export/OBJ fix).
- 2026-10-21 ~17:30 (api): v-freeze reply "frozen, green" (ops/scene/cli clippy-clean, ops+cli tests green incl. e2e 13;
  noshape-model has 2 foreign lints: custom/std_feature.rs:23, drawing/cad_fit.rs:33). Freeze lifted at 5d28447:
  parked work re-queued as chain9.sh (log /tmp/api/chain9.log): assembly stages 1-3 (patterns incl. centered and
  Replicate scope, BOM, interference, groups, relations, connectors, replace, rename, feature state, named positions,
  exploded views), names/curves fix, Feature Studio ops, import tests, then UPDATE_DOCS + tests + CLI + probe.
  Rule: re-run `UPDATE_DOCS=1 cargo test -p noshape-ops --test coverage` whenever ops change (the chains do).
  ~17:50: stage 1 LANDED (assembly_pattern/dissolve_pattern/bom/bom_settings/interference). App pattern defaults are now
  live's 25 mm / 360 deg / 2 -> fix_pattern_defaults.py queued (chain10.sh). App switch READY (needs the assembly
  owner's freeze): /root/api-scratch/patch_asm_switch.py (--dry OK on 5d28447+): assembly_panels.rs 902-938
  new_pattern_session defaults, 948-956 replicate_on_edge, 1029-1039 seed_mate, 1093-1137 accept_pattern;
  assembly_interference.rs 105-127 interference_rows; assembly_view.rs 1334-1353 toggle_mate_suppressed,
  1361-1387 accept_group -> noshape_ops::assembly. Relations / connectors / replace / named positions follow after
  stages 2-3 land.
  ~18:20: stage 2 LANDED (groups, relations, mate connectors, replace, rename, feature state; PLANNED_CEILING 64) and
  the names/curves fix LANDED; stage 3 in its guard. qa5 API3 (coincident on an already-coincident corner gave
  "a constraint needs two different entities"): the corner drawn on the origin IS the origin point, so the API passed
  one entity twice. Fix queued (coinc/apply.py, chain11.sh): sketch_constrain treats `entities` as a selection set (each
  once, as the UI); coincident on points that are one point -> success, nothing added, no edit, message "The points
  are already coincident (they are the same point)."; other duplicates get the tool's own prompt ("Select 2
  entities for Equal ..."). Note: coincident on two distinct already-coincident points adds a second constraint in
  both app and API (shared apply_constraint) - left as the app does it.
- 2026-10-21 ~19:10 (api, new agent after quota takeover): chains 9-11 had run: stage 1/2, names/curves, Feature
  Studio ops, import tests, pattern defaults and the coincident no-op (API3) all LANDED; stage 3 had FAILED to apply
  (stale import anchor in asm_ops3/apply.py: assembly.rs now imports MateType) -> anchor fixed, re-applied via guard.
  e2e drawing_views_and_dimensions failed since drw2's DXF default became Release 14 (not UTF-8) -> test reads bytes.
  API3 second half: `sketch_dimension` `value` is now optional (keeps the measured value, as the Dimension tool's
  value box accepted unchanged); test in coincident_on_one_point_is_a_no_op.
  Pending app switches (need owners' freezes, unchanged): patch_asm_switch.py, patch_part_rename_props.py.
  ~19:50: stage 3 LANDED (assembly_named_position / _drivers / exploded_view / explode_step; asm_get lists
  named_positions + exploded_views). AGENT_API.md regenerated; ops + cli tests green (e2e 20). Clippy on
  noshape-ops currently stops in foreign crates/store/src/undo.rs:208/243 (type_complexity, someone's in-progress
  edit); the guarded clippy before it was clean in ops.
  NEXT: app switches (patch_asm_switch.py, patch_part_rename_props.py) and drawing slices 5-6 when the lead
  freezes the owners' files; then live mode.
- 2026-10-21 ~21:30 (api): App switches done (lead: no freeze, one file at a time): part_studio/menus.rs (Properties
  save), part_studio/behaviors.rs (Rename), assembly_panels.rs + assembly_interference.rs (patch_asm_switch.py with a
  file filter). assembly_view.rs part NOT yet applied (file was mid-edit): `python3 patch_asm_switch.py assembly_view.rs`.
  Notes left in part-studio-behaviour.md and assembly-behaviour.md. Backups in /root/api-scratch/bak_*.rs.
  API3 per live (no glyph, no message, no edit): scene sketch_mode `already_coincident` (uses the sketch crate's
  `joint_points`, now pub); apply_constraint skips already-coincident pairs, constraint_command returns false without
  a tool switch, tool-first picks do nothing. API reply keeps an informational "message". Unit test in scene.
  Drawing slice 5 started: scene drawing_tools/commit.rs (ortho_point, extend_cut, section_jog_at, aux_edge,
  view_refusal, silhouette/edge-pair centerlines, centerline/circle_centerline/virtual_sharp builders, view_break,
  view_crop, view_table, default_break_gap_mm), and ops/src/ops/drawing_tools.rs: drawing_section_view,
  _detail_view, _auxiliary_view, _break_view, _crop_view, _centerline, _virtual_sharp, _view_table (+ e2e
  drawing_section_detail_views_and_annotations). NEXT: switch the app's tool_click (drawing_tools.rs) and
  extra_tool_click (drawing_extra.rs) to commit.rs (asb's files, one at a time) + note in drawing-behaviour.md;
  then notes/callouts/centermarks/datums/GD&T/surface finish/weld/hole callout/ordinate ops; then live mode.
- 2026-10-22 ~02:05 (api): after "freeze lifted" (main 5bd50b2):
  * assembly_view.rs switch done (Suppress mate, Group commit; unused acfg/ConfigTarget/ParamValue/Group imports
    removed). All planned app switches are now in: menus.rs, behaviors.rs, assembly_panels.rs,
    assembly_interference.rs, assembly_view.rs, drawing_tools.rs, drawing_extra.rs (drw5/apply_*.py).
    Notes in part-studio-behaviour.md, assembly-behaviour.md, drawing-behaviour.md.
  * sketch_draw `shapes`: many shapes in one edit (perf: 400 circles one call at a time took 50 s); each shape is
    drawn by a fresh editor exactly as its own call; e2e sketch_draw_many_shapes_in_one_edit (same entities and
    constraints as separate calls; 400 circles).
  * Variable Studio variables (qa5 wf22): variable_list/create/set/delete/move take a Variable Studio `tab`; shared
    edits in crates/ops/src/variable_studio.rs (the app's table cells, same history text). e2e variable_studio_table.
    App switch of crates/app/src/variable_studio.rs (ui2's) to ops::variable_studio still to do.
  * LIVE MODE: crates/ops/src/live.rs (discovery file <temp>/noshape-live/<fnv(path)>.json {port, token, pid}; TCP
    127.0.0.1; one JSON line each way; registry::run forwards via live::forward unless ctx.serving; Ctx gains
    live/serving/set_live/unset_live/adopt/get_mut). App: crates/app/src/live_server.rs (serve(doc) from
    files.rs open_session_window; runs each call on the window's session by swapping it with a placeholder session
    adopted in a serving Ctx; emits Reloaded + HistoryChanged after mutating ops). Test crates/ops/tests/live.rs.
    Docs: AGENT_API.md "Live mode". NEXT: in-app check (open a doc in the app, CLI edit, screenshot, Ctrl+Z undoes).
  * Fresh CLI for qa5: /root/target-engine/debug/noshape-cli (01:07) and /root/api-scratch/bin/noshape-cli.
  * ~03:20 IN-APP VERIFIED (Xvfb :114, build 02:21): live mode (CLI sketch + extrude appear in the open window;
    Ctrl+Z in the app undoes the CLI's extrude; CLI history_list via the app shows the undo); part Properties › Save
    ("Part Studio 1 :: Properties : Base plate"); drawing Virtual sharp (hover + commit, "Drawing 1 :: Virtual
    sharp"); assembly Suppress mate ("Assembly 1 :: Suppress mate"). Screens in /root/api-scratch/livedemo/.
    FOUND (psb, not mine): parts list Rename panics in debug builds ("hover style already set",
    part_studio/mod.rs render_part_row: tree_row() sets hover when !selected, then .when(renaming, .hover(..)));
    fix: tree_row(id, selected || renaming) and drop the second .hover. Reported to the lead.
    live.rs: serve() removes stale discovery files / spare sessions of apps that are gone (killed apps run no
    destructors); spare session path = live::spare_path() (<pid>-<token>.noshape).
  * ~03:30 variable_studio.rs (ui2) switched to ops::variable_studio (apply: /root/api-scratch/vs/apply.py);
    unused VariableDef/ExprValue imports removed; app clippy: nothing from it (one foreign lint:
    drawing_drawprops.rs:165). Note in theme-studios.md.
    NEXT: drawing slice 6 tools (note, callout, centermark, datum, GD&T, surface finish, weld, hole callout,
    ordinate) as API ops on the shared pending_annotation builders; aux view In line / Axis modes.
  * ~03:55 VS switch IN-APP VERIFIED (build 03:39, Xvfb :114, /root/api-scratch/vsdemo/v*.png): Add variable (name row ->
    Length 0 mm), Value "40" -> "40 mm", Description, second Add, Rename, Type -> Number (value turns red), row
    menu Move up / Move down / Delete, Ctrl+Z restores the row; history texts as before (read through live mode).
- 2026-10-22 ~04:00-04:55 (api):
  * Drawing annotation ops (ops/src/ops/drawing_tools.rs): drawing_hole_callout, drawing_callout (anchor optional =
    free callout; text/above/below/left/right/border/chars/height = the Callout dialog), drawing_ordinate (origin,
    side, points; one edit per click: "Ordinate dimension" then "Add to ordinate group"), drawing_datum (anchor,
    position, label; datum_on attach to GD&T/dimension lines not yet). Shared builders added to
    scene/drawing_tools/commit.rs: free_callout, SheetPick::finish_callout (qa5 D8 part under the anchor).
    e2e drawing_callouts_ordinates_and_datums; coverage Callout/Datum/Hole callout/Ordinate -> ops, ceiling 47.
  * App switch: drawing_tools.rs Balloon arms -> dt::free_callout / pk().finish_callout
    (/root/api-scratch/drw5/apply_callout.py, backup drw5/bak2_drawing_tools.rs). Clippy clean;
    IN-APP CHECK STILL TO DO (Callout with leader on an edge, free callout in empty space; history "Callout").
  * Green-up 04:10 freeze: app clippy clean, scene/ops/cli clippy clean, ops+cli tests green (e2e 24, live 1).
  * NEXT: in-app check of the callout switch; then note / centermark / GD&T / surface finish / weld /
    max-min / foreshorten / chamfer (needs app chamfer_as_dimensions + more_dimension_pick moved to scene);
    aux view In line / Axis modes; datum on annotation lines.
- 2026-10-22 ~06:30 (api): freeze lifted at aa75fdf. New ops: drawing_dimension kinds chamfer / max_min / foreshorten
  (more_dimension_pick + chamfer_as_dimensions moved to scene), drawing_centermark (single/circular/linear + dialog
  options), drawing_note (text, leader, height). Coverage ceiling 42. App switches applied: drawing_more.rs,
  drawing_tools.rs (chamfer), drawing_centermark.rs, drawing_ui.rs (note) (scripts /root/api-scratch/drw5/apply_*.py).
  IN-APP VERIFIED (build 06:16): Callout with leader + free callout ("Callout"), Centermark single, Max/min dimension.
  Note switch: verify after the 06:30+ build. CLI rebuilt for qa5 after this batch.
  NEXT: GD&T, surface finish, weld (dialog-driven), aux view In line / Axis, datum on annotation lines.
- 2026-10-22 (api, worktree /root/wt/api, branch agent/api): new workflow (WORKFLOW.md): commit, merge main, check,
  `git branch -f ready/api HEAD`; status in /tmp/lead/mq/api.status. Never `git stash` (shared by all worktrees).
  Landed since the transition: drawing_gdt, drawing_surface_finish, drawing_weld (+ shared gdt / surface_finish /
  weld / weld_target builders in scene commit.rs); release ships noshape-cli (a6169ca, main cebd793): macOS
  noshape.app/Contents/MacOS/noshape-cli + loose in the dmg, Linux/Windows next to the app; README + AGENT_API.md
  quick start per OS with the macOS quarantine note. sketch_fillet default = live's rule (R5 at first, then the
  last radius used in that Part Studio while the document is open: OpenDoc.sketch_fillet_radius); e2e
  sketch_fillet_radius_default_is_r5_then_sticky.
  Parked app switches (apply when convenient, each checked in the app): /root/api-scratch/gdt/apply_app.py
  (drawing_gdt.rs, drawing_extra.rs surface finish, drawing_weld.rs -> scene builders).
  NEXT: those app switches + in-app check; aux view In line / Axis; datum on annotation lines; revision table /
  callout, general / custom tables, hatch, broken-out section, drawing sketch tools, insert image/DXF as ops.
- 2026-10-22 ~15:45 (api): app switches GD&T / surface finish / weld landed and checked in the app; new ops
  drawing_revision_table, drawing_revision_callout (+ app switch of drawing_revision.rs, checked in the app).
  Remaining drawing: aligned/jogged section, broken-out section, crop polygon/spline, aux In line/Axis, hatch,
  general/custom tables, insert image / DXF, drawing sketch tools, format painter, inspection items, Update.
