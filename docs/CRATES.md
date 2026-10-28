# Crate structure (lead, from 2026-10-26)

Large Rust workspaces stay fast to build by being many crates in layers: an edit recompiles one small crate and
its dependents, and independent crates compile in parallel. noshape had two giants (`model` 98k lines, `app` 138k
lines), each one tangle where every module referenced every other, so any edit recompiled all of it. This file is the
target structure and the rules that keep it.

## Layers (lower never depends on higher)

| Layer | Crate | What |
|---|---|---|
| 0 | `noshape-units`, `noshape-sketch`, `noshape-kernel`, `noshape-featurescript` | quantities, the sketch solver, the OCCT kernel bridge, the FeatureScript language |
| 0 | `noshape-brep` (below `noshape-kernel`) | noshape's own B-rep, pure Rust: index-based topology and a Boolean for solids bounded by planes (docs/BOOLEAN-SPEEDUP.md phase 3); the kernel converts OCCT shapes to it and back (`brep_bridge.rs`, `cpp/rust_boolean.cpp`) |
| 1 | **`noshape-model-data`** | the document data model: documents, tabs, Part Studio feature lists, feature schemas, params, references, configurations, variables, and the data types of Assemblies, Drawings (sheets, views, annotations, templates, fonts) and Feature Studios. Serde types and plain methods on them; **no evaluation** |
| 2 | **`noshape-model-core`** | regeneration, features, sheet metal, in-context (context bodies in regeneration), analysis; what regeneration needs of the FeatureScript host (`regen::custom_features`: compiled studio types, `CustomFeatureProvider`, the `CustomHost` hook; `regen::std_route`: which built-in features std runs) |
| 3 | **`noshape-custom`** | the FeatureScript host: compiling Feature Studios, custom features and custom tables, the modeling natives, the built-in features std runs, the demo Feature Studio |
| 4 | **`noshape-assembly`** | assembly evaluation, the mate solver, assembly analysis (interference, exploded views), BOM, standard content, native assembly import, in-context snapshots, the demo documents with assemblies |
| 5 | **`noshape-drawing`** | drawing views, HLR projection, sheet building, tables, export (PDF/DXF/DWG/DWT), the demo documents with drawings |
| 5 | **`noshape-model`** (facade) | re-exports model-data, model-core, custom, assembly and drawing under the old paths; holds the model integration tests (`crates/model/tests`) |
| 2–5 | `noshape-store`, `noshape-scene`, `noshape-render` | persistence, scene building, the wgpu renderer |
| 6 | `noshape-ops`, `noshape-cli` | the agent API |
| 7 | **`noshape-app-core`** | the app's base layer: theme, assets (icons, i18n tables; the files stay in `crates/app/assets`), base widgets (`ui`, `os_ui`, `os_menu`, `os_select`, `quantity_field`, …), actions, cursors, touch, the GPU context, the 3D viewport and navigation, the camera menu, the dialogs and panels the Part Studio and the assembly share (graphics preferences, named views, section view, customize toolbar, tool synonyms), what several studios share (`custom_tables`, `file_dialogs`: the save/open dialogs, `library::DocEntry`, `actions::request_activate_tab`, `derived::document_versions`, `os_ui::checkbox_box`, the Configuration visibility conditions dialog `config_visibility`, the Properties dialogs `properties_dialog`, the View cube widget `view_cube`, the Reflection analysis's settings `reflection`, the Edit in context data `assembly_context`, `library::decode_thumbnail`, file tabs `file_tab_view`, the documents library scan and the Trash `start_library`/`start_trash`, the recent list, watched folders, open-documents registry and other documents `doc_files`, the user's units `user_units`, tab thumbnails `tab_thumbnail`, Repair `repair_mode`, the keyboard shortcuts registry and keymap `shortcuts`), `paths` (`config_dir`). Never references a studio |
| 8 | **`noshape-drawing-ui`** | the Drawing studio UI (`drawing_*`: the editor, its tools, panels and dialogs). What it needs of the app (the workspace's Import, other documents' windows, file tabs, the release log and watermark, the documents library, Create version, the export rules link, a preference) are the hooks in `drawing_hooks`, registered by main.rs at startup |
| 8 | **`noshape-assembly-ui`** | the Assembly studio UI (`assembly_*`: the editor, its panels, dialogs and tools; `measure_panel`, which only the assembly uses). Builds on `noshape-drawing-ui` (Create drawing). What it needs of the app (other documents' windows, the documents library and its folders, the release dialogs, Create version, tab properties, a preference, the Part Studio's Export and Input propagation dialogs, the Variable Studio's tables) are the hooks in `assembly_hooks`, registered by main.rs at startup |
| 8 | **`noshape-part-studio-ui`** | the Part Studio UI (`part_studio/`: the editor, its panels, tools and menus; `feature_dialog`, `sketch_mode`, `sketch_fonts`, `var_complete`, `compare_graphics`, the FeatureScript reference fields `fs_reference` and the Select item panel `select_item`, which the Feature Studio's import shares). Builds on `noshape-drawing-ui` (Create drawing). What it needs of the app (other documents' windows, the documents library, the release dialogs, Create version, preferences and the shortcuts page, the Variable Studio's tables and Type select, Compare and Repair, the added material libraries' materials and the Add material library modal) are the hooks in `part_studio_hooks`, registered by main.rs at startup |
| 8 | **`noshape-variable-studio-ui`** | the Variable Studio (`variable_studio`, `variable_config`, `variable_import`, `variable_insert`: the Variable table panel). Hooks (`variable_hooks`): another document's window, Create version |
| 9 | **`noshape-preferences-ui`** | the Preferences window and its pages (`preferences_window`, `prefs_general`, `prefs_export_rules`, `units_prefs`, `mouse_prefs`, `drawing_prefs`, `material_prefs`, `material_libraries`). Builds on drawing-ui and assembly-ui (their toolbar settings). Hooks (`preferences_hooks`): the Part Studio's tools for Shortcut toolbars, refreshing the documents page, re-theming |
| 10 | **`noshape-release-ui`** | release management (`release_dialog` + `release_add_source`, `release_workflow`; `release_prefs`, `release_conditions`, `release_settings`, `release_thumbs`, `revision_template`, `revision_tools`, `numbering_prefs`) and Company settings (`company_settings`, which only hosts these pages). Builds on preferences-ui. Hooks (`release_hooks`): other documents' and versions' windows, Create version, the workspace's Export |
| 10 | **`noshape-feature-studio-ui`** | the Feature Studio (`feature_studio`, `fs_notices`, `fs_search`, `fs_import`, `fs_profile`, `std_source`). Builds on preferences-ui (Labs). Hooks (`feature_studio_hooks`): Create version, a Feature Studio of a version in its own window |
| 11 | **`noshape-history-ui`** | Versions and history (`history_panel`, `compare_mode`, `merge_dialog`, `version_properties`). Builds on release-ui (release rows and dialogs) and feature-studio-ui (the code compare's highlighting). Hooks (`history_hooks`): the Part Studio's feature icons, the Workspace protection dialog |
| 11 | **`noshape-start-ui`** | the documents page (`start_window`, `start_search`). Builds on preferences-ui and release-ui (its Preferences and Company settings menu items). Hooks (`start_hooks`): a document's window, the build label |
| 12 | `noshape` (the binary) and, planned, the workspace/chrome as a crate above the studios | GPUI application: the workspace, tab manager, document panels and dialogs, files, shortcuts page, main.rs (registers every crate's hooks before the app runs), smoke |

`noshape-model` re-exports every `noshape-model-data` module under the same path (`noshape_model::document`,
`noshape_model::drawing::Sheet`, …), so code above it doesn't change when types move down.

## Rules

1. **Data down, logic up.** A new type that is stored in a document goes in `noshape-model-data`. A function that
   needs regeneration, solving or views goes in the crate that has them.
2. **Logic methods on data types are extension traits** in the logic crate (e.g. `ViewMoves`, `DrawingClipboard`,
   `SheetPaste`, `ModelAnnotations`, `CadTables`, `ContextBodies`), re-exported from `noshape_model::prelude`.
   Callers write `use noshape_model::prelude::*;`. Rust doesn't allow inherent `impl` blocks outside the type's crate.
3. **Upward calls are hooks, not dependencies.** When a lower crate must reach logic above it (feature schemas of
   compiled Feature Studios), it exposes a registration function (`features::set_custom_lookup`) that the upper crate
   calls; never add a dependency from a lower layer to a higher one.
   A hook a regeneration may need must be installed before it runs: the FeatureScript host (`noshape_custom::register`,
   idempotent) is installed by every compile, `DocumentScripts::from_document`, std's prewarm and the std switches,
   and at startup by the app, the agent API (`Ctx::new`, `Session::open`/`create`); without it a feature that needs
   it fails with `regen::custom_features::NO_HOST` (never silently).
4. **Tests live with the logic they exercise.** A unit test in `noshape-model-data` may only use data; tests that
   regenerate or move views go in `noshape-model` (e.g. `drawing/links_tests.rs`, `configuration_tests.rs`).
5. **Keep crates under ~25k lines.** When one grows past that, split it along these lines before it becomes a tangle.

## Target for the app: Zed's layout (checked 2026-10-27)

zed-industries/zed has 251 crates; its binary crate (`crates/zed`) is ~22k lines and its `main.rs` is mostly
`<feature>::init(cx)` calls (settings, project, debugger_ui, command_palette, language_models, …). Each feature is a
crate that registers itself from `init`: the `workspace` crate defines the item/panel traits and a registry and never
names a feature, so editing a feature recompiles that crate and a small binary. Ours (10-27 morning): the binary
`noshape` was 97k lines (Part Studio UI 34k, assembly UI 23k, workspace/chrome, preferences, release management, start
window), so every edit below it recompiled 97k lines (35–60 s). 10-27 noon: 6.4k (the workspace/chrome, files, main.rs, smoke), with
the Part Studio UI, the other studios and the document features in their own crates.

Plan, one step at a time (each lands green; the owning agent is warned before its files move):
1. ~~`noshape-assembly-ui` (assembly_*, 23k) out of the binary, like `noshape-drawing-ui` (hooks for app-level calls).~~
   Done 10-27 (see Progress); the binary is 71k lines.
1b. ~~`noshape-part-studio-ui` (part_studio/, feature_dialog, 40k) out of the binary, the same way.~~ Done 10-27; the
   binary is 30k lines.
2. `noshape-workspace`: the workspace and chrome with a **studio registry** (a trait per tab kind: create the view,
   menus, toolbar, save/restore; studios register from their `init(cx)`), so the workspace never names a studio.
3. `noshape-part-studio-ui` (part_studio/ + feature_dialog, 38k; psui), then Variable Studio, Feature Studio, start
   window, preferences, release management/history as their own crates, each with `init(cx)`. ~~The six small crates~~
   done 10-27 with hooks (see Progress); their `init(cx)` comes with step 2 (hooks become actions,
   docs/APP-ARCHITECTURE.md).
4. The binary: `main.rs` calling the `init`s plus app wiring, ~10–20k lines.

## Progress

- 2026-10-26: `noshape-model-data` split out of `noshape-model` (26k lines moved; model 98k → 72k).
- 2026-10-26: `noshape-drawing` (13k lines) split out above the core (`noshape-model-core`, 59k lines); `noshape-model`
  is now a facade. Measured on this machine (warm, incremental, quiet), edit → rebuild:

  | edit in | rebuild | time |
  |---|---|---|
  | drawing logic | drawing tests | 4 s |
  | drawing logic | the app | 35 s |
  | model core (regen) | core unit tests | 11 s |
  | model core | model integration tests | 15 s |
  | model core | the app | 35 s |
  | model data | core (check) | 9 s |
  | app | the app | 47 s |

- 2026-10-27: `noshape-assembly` (11.5k lines) split out above the core (evaluation, solver, analysis, BOM, standard
  content, native assembly import, the in-context snapshots of an evaluated assembly, the demo document's assembly);
  `noshape-model-core` is now 47k lines and no longer contains any assembly logic (`sketch_body_id` moved to
  `reference`, the core's `in_context` keeps what regeneration needs, `demo::plate_document` is the demo without the
  assembly). noshape-drawing builds on noshape-assembly. Measured warm, incremental, on a loaded machine (load ~30
  from the other agents), edit → rebuild:

  | edit in | rebuild | time |
  |---|---|---|
  | assembly logic (eval) | assembly unit tests | 6.5 s |
  | assembly logic (eval) | the app | 54–66 s (loaded; the drawing row above was 35 s quiet) |

- 2026-10-27: `noshape-custom` (14k lines, the FeatureScript host: compile, provider, run, the natives, custom tables,
  the features std runs, the demo Feature Studio) split out above the core; `noshape-model-core` is now 33k lines.
  Regeneration reaches the host through `regen::custom_features` (the compiled studio types and
  `CustomFeatureProvider` moved down; the `CustomHost` hook: fallback provider, custom and std evaluation; `error_text`,
  `image_size`) and `regen::std_route` (which built-in features std runs, the std switches); the custom table data
  types moved to model-data. noshape-assembly and noshape-drawing depend on noshape-custom. Measured warm,
  incremental, machine load ~20–27, edit → rebuild:

  | edit in | rebuild | time |
  |---|---|---|
  | FeatureScript host (run.rs) | custom unit tests | 5.5 s |
  | FeatureScript host (run.rs) | the app | 51 s |
  | model core (regen) | core unit tests | 7 s (was 11 s) |

- 2026-10-27: `noshape-app-core` (14k lines, the app's base layer) split out of the app; `noshape` is now 124k lines.
  The app re-exports its modules at its root (`use noshape_app_core::{theme, ui, viewport, …}` in main.rs), so every
  `crate::ui::…` path in the app is unchanged. Edges cut: `paths::config_dir` (moved from `files`, which re-exports it);
  `os_ui::export_rules_link` (the only caller of `preferences_window::show`) moved to `preferences_window`; main.rs
  binds the keys (`shortcuts::init(cx, actions::default_key_bindings())`, was `actions::bind_default_keys`). The
  source guards (no bare `white()` fills, no second hover on a `tree_row`) scan every `crates/app*/src`. Measured warm,
  incremental, machine load ~30, edit → `cargo build -p noshape`:

  | edit in | rebuild | time |
  |---|---|---|
  | a studio file (drawing_view.rs) | the app | 40 s (was 47 s quiet, 51–66 s loaded, with the whole app one crate) |
  | app-core (ui.rs) | the app | 91 s (app-core, then the app) |

- 2026-10-27: `noshape-drawing-ui` (27k lines, 64 `drawing_*` files: the Drawing studio UI) split out of the app above
  app-core; `noshape` is now 96k lines, app-core 15k. The app re-exports the four modules it uses (`drawing_create`,
  `drawing_keys`, `drawing_sketch`, `drawing_view`) at its root; `drawing_prefs` (a Preferences page) stayed in the app.
  Edges cut: `request_activate_tab` (+ its static) moved to app-core `actions` (`workspace` re-exports it); the
  save/open dialogs moved to app-core `file_dialogs` (`files` re-exports them); `custom_tables` (shared with the Part
  Studio) and `select_item::document_versions` (to `derived`) moved to app-core; `DocEntry`/`EXT` moved to app-core
  `library`; `PreferencesView::checkbox_box` is `os_ui::checkbox_box`; `sketch_mode`/`toolbar_prefs` were re-exports of
  `noshape_scene`/`noshape_ops`, used directly. The rest are hooks (`drawing_hooks::Hooks`, registered by
  `register_drawing_hooks` in main.rs before the app runs): the workspace's Import, opening another document's window,
  `file_tab_view::pick_file`, the release log, the drawing watermark text (moved to
  `release_dialog::drawing_watermark_text`), the documents library scan, Create version, the export rules link, the
  light model space preference. The crate is opt-level 0 in dev like the app. The source guards scan `crates/app*` and
  `crates/*-ui`; the menu coverage test and scripts read `crates/drawing-ui/src` (files `drawing_*` → area drawing).
  Measured warm, incremental, machine load 18–38 (other agents), edit → `cargo build -p noshape`:

  | edit in | rebuild | time |
  |---|---|---|
  | drawing UI (drawing_view.rs) | the app | 35–74 s, typically ~50 s (drawing-ui ~18 s, then the app ~27 s: any change to a crate makes its dependents recompile) |
  | Part Studio (part_studio/mod.rs) | the app | 36–80 s, typically ~42 s |
  | drawing UI | `cargo build -p noshape-drawing-ui` (check, tests) | ~18 s, without the app |

  So the split makes drawing-UI checks and unit tests fast but not yet the binary: the app (96k lines) still re-checks
  after any upstream change. The binary gets faster as more of it moves out (the app's own share shrinks).

- 2026-10-27: `noshape-assembly-ui` (23k lines: the 16 `assembly_*` files, `agent_window/assembly.rs`, and
  `measure_panel`, which only the assembly uses) split out of the app above app-core and drawing-ui; `noshape` is now
  71k lines (was 97k), app-core 17k. The app re-exports `assembly_view` and `assembly_scene` at its root. Moved down
  into app-core, being shared with the Part Studio (and others): `config_visibility`, `properties_dialog` (whole
  files), the View cube widget (`view_cube`; the Part Studio's camera menu stays in `part_studio/view_cube.rs`), the
  Reflection settings (`reflection::{EnvImageRef, ReflectionSettings}`), the Edit in context data
  (`assembly_context::{AssemblyContext, ContextOccurrence, context_arrow_icon}`), `library::decode_thumbnail`; the
  Part Studio's modules re-export them. Used directly instead of through app modules: `noshape_ops::docedit::place_after`,
  `noshape_ops::export::*`, `noshape_scene::environment_maps`, `file_dialogs`, `paths::config_dir`,
  `actions::request_activate_tab`, `library::{DocEntry, EXT}`; Create drawing calls `noshape_drawing_ui::drawing_create`
  (drawing-ui never names the assembly). The rest are hooks (`assembly_hooks::Hooks`, registered by
  `register_assembly_hooks` in main.rs next to the drawing's): other documents' and versions' windows, the documents
  library scan, watched folders and subfolders, file thumbnails, the release items and Release candidate dialog, the
  last viewed configuration, Create version, tab properties, the instance list properties preference, the Part
  Studio's Export modal and Store file in a new tab, the Input propagation dialog, the Variable Studio's tables (the
  view's tables are `AnyView`s) and the Variable table footer. The crate is opt-level 0 in dev; the menu coverage test
  and scripts read `crates/assembly-ui/src` (files `assembly_*` → area assembly); the source guards already scan
  `crates/*-ui`. Measured warm, incremental, machine load ~40 (other agents), edit → rebuild:

  | edit in | rebuild | time |
  |---|---|---|
  | assembly UI (assembly_view.rs) | the app (`cargo build -p noshape`) | 37–45 s (assembly-ui, then the 71k-line app) |
  | assembly UI (assembly_view.rs) | `cargo check -p noshape-assembly-ui` | 4.6–10 s, without the app |

- 2026-10-27: `noshape-part-studio-ui` (40k lines: `part_studio/` 34k, `feature_dialog` 4k, `agent_window/studio.rs`,
  and what only the Part Studio uses: `sketch_mode`, `sketch_fonts`, `var_complete`, `compare_graphics`,
  `fs_reference`, `select_item`) split out of the app above app-core and drawing-ui; `noshape` is now 30k lines (was
  71k). The app re-exports `part_studio` and `select_item` at its root (the Feature Studio's import uses Select item;
  `doc_panels::TabPerf` is the Part Studio's). Moved down into app-core, being shared: the keyboard shortcuts registry
  (`shortcuts`; the app's `shortcuts` re-exports it and keeps `run_in_drawing`), the added material libraries
  (`material_libraries::{AddedLibrary, added, onshape_listed, …}`; the app keeps the Add material library modal and
  `materials_of`), the mass formatting (`properties_dialog::{mass_unit, mass_number}`, re-exported by the assembly's
  `mass`) and `assembly_context::StaleContexts`. Used directly instead of through app modules:
  `noshape_ops::import::add_file_tab`, `file_dialogs`, `paths::config_dir`, `actions::request_activate_tab`,
  `library::{DocEntry, EXT}`. The rest are hooks (`part_studio_hooks::Hooks`, registered by
  `register_part_studio_hooks` in main.rs next to the assembly's): other documents' windows, the documents library
  scan, other documents for Variable Studio references, the release log, items, Release candidate and Revision history
  dialogs, the last viewed configuration (note and read), Create version, Preferences › Keyboard shortcuts, the Shift+/
  sheet, key chips, the export rules link, the parts list properties and feature dialog width preferences, the
  Variable Studio's tables (`AnyView`s), read-only tables, footer and Type select, Compare's blend, documents, panel
  and slider, Repair mode's state, close and panel, an added library's materials and the Add material library modal.
  The crate is opt-level 0 in dev; the menu coverage test and scripts read `crates/part-studio-ui/src` (`part_studio/`
  and `feature_dialog.rs` → area partStudio). Measured warm, incremental, machine load 15–25 (other agents paused),
  one-line edit → rebuild:

  | edit in | rebuild | before | after |
  |---|---|---|---|
  | Part Studio (part_studio/view_tools.rs) | the app (`cargo build -p noshape`) | 15.6–20.5 s | 16.2–19.2 s |
  | Part Studio (part_studio/mod.rs) | the app (`cargo build -p noshape`) | 21.4 s | 15.3–16.0 s (part-studio-ui 5.7 s, then the 30k-line app 8.3 s and the link) |
  | Part Studio (part_studio/mod.rs) | `cargo check -p noshape-part-studio-ui` | (the app) | 3.7–3.8 s, without the app |

  The binary barely moved at this load: the app's own share is now small, but a changed dependency still makes it
  recompile and relink. The win is the crate's checks and unit tests (4 s instead of the app).

- 2026-10-27: six small app crates split out of the binary, one commit each, each the drawing-ui pattern (crate-root
  `use noshape_app_core::*`, the app re-exports the moved modules it uses at its root so `crate::x::…` paths are
  unchanged, a `<area>_hooks` module for upward calls registered by `register_<area>_hooks` in main.rs before the app
  runs, opt-level 0 in dev): `noshape-variable-studio-ui` (3.4k lines), `noshape-preferences-ui` (3.2k),
  `noshape-release-ui` (5.1k), `noshape-feature-studio-ui` (3.5k), `noshape-history-ui` (3.6k), `noshape-start-ui`
  (3.8k); with the Part Studio split (above, landed first; `var_complete` and `compare_graphics` went with it)
  `noshape` is now 6.4k lines (was 30k after the Part Studio, 71k before both), app-core 19.5k. Dependencies
  between them are acyclic: preferences-ui on drawing-ui and assembly-ui (their toolbar settings), release-ui on
  preferences-ui (Company settings, which only hosts release pages, moved with release), feature-studio-ui on
  preferences-ui (Labs), history-ui on release-ui and feature-studio-ui (release rows, the code compare's
  highlighting), start-ui on preferences-ui and release-ui (its user menu); variable-studio-ui only on app-core.
  Moved down into app-core, being shared: `file_tab_view`, `start_library` and `start_trash` (the documents library
  scan and the Trash), `doc_files` (from `files`: the recent list, watched folders, the user name, the open-documents
  registry; `other_documents` from `variable_insert`), `user_units` (the data of `units_prefs`, doc_dialogs' decimals
  options), `tab_thumbnail` (from `tab_manager`), `repair_mode`, `shortcuts` (the registry and keymap; the app's
  `shortcuts` keeps `run_in_drawing`; the Part Studio split moved it too); `material_libraries` (the Add material library modal and `materials_of`; its data is app-core's since the Part Studio split) went to preferences-ui. Used directly instead of app
  re-exports: `noshape_ops::import`, `noshape_ops::docedit::place_after`, `noshape_ops::versions::PUBLISHED_FS_KEY`,
  `noshape_ops::prefs::ToolbarPrefs`, `noshape_scene::asm_scene`, `derived::document_versions`. Hooks: variable
  (another document's window, Create version), preferences (the Part Studio's toolbar tools and sketch rows for
  Shortcut toolbars, refreshing the documents page, `apply_kit_theme`), release (documents' and versions' windows,
  Create version, the workspace's Export), Feature Studio (Create version, opening std's source at a version: the
  workspace's `goto_studio`), history (the Part Studio's feature icons, the Workspace protection dialog), start (a
  document's window, the build label). The app dropped resvg, tiny-skia, regex, dirs, chrono, serde and rusqlite.
  The menu coverage test and scripts also read `crates/variable-studio-ui/src` and `crates/history-ui/src`
  (`variable_*`, `history_panel.rs` → area document); the source guards already scan `crates/*-ui`. Measured warm,
  incremental, machine load 20–27 (other agents), edit (a one-line probe, added then removed) → `cargo build -p
  noshape`, on the 6.4k-line binary:

  | edit in | rebuild | time |
  |---|---|---|
  | the workspace (workspace.rs) | the app | 7–14 s (was 37–45 s with the 71k-line app, at load ~40) |
  | history UI (history_panel.rs) | the app | 9–16 s (history-ui, then the app) |
  | Variable Studio (variable_studio.rs) | the app | 10–13.5 s |
  | documents page (start_window.rs) | the app | 11–13 s |
  | history UI (history_panel.rs) | `cargo check -p noshape-history-ui` | 7 s, without the app |

  Before the rebase on the Part Studio split (46k-line binary, load 13–17): workspace 13–14 s, history UI 15–19 s,
  Variable Studio 14 s. Most of what is left is linking the binary and re-checking the crates above the edited one.

- Next: step 2 (`noshape-actions` + `noshape-workspace`
  with the studio registry; the hooks become actions and each crate gets its `init(cx)`, docs/APP-ARCHITECTURE.md).
  What stays in the binary meanwhile: workspace, tab_manager, tab_thumbs, doc_panels, doc_dialogs, files,
  shortcuts(_page), import_dialog, material_library_view, workspace_protection, live_server, agent_window, smoke.
