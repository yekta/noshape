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
