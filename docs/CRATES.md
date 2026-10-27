# Crate structure (lead, from 2026-10-26)

Large Rust workspaces stay fast to build by being many crates in layers: an edit recompiles one small crate and
its dependents, and independent crates compile in parallel. noshape had two giants (`model` 98k lines, `app` 138k
lines), each one tangle where every module referenced every other, so any edit recompiled all of it. This file is the
target structure and the rules that keep it.

## Layers (lower never depends on higher)

| Layer | Crate | What |
|---|---|---|
| 0 | `noshape-units`, `noshape-sketch`, `noshape-kernel`, `noshape-featurescript` | quantities, the sketch solver, the OCCT kernel bridge, the FeatureScript language |
| 1 | **`noshape-model-data`** | the document data model: documents, tabs, Part Studio feature lists, feature schemas, params, references, configurations, variables, and the data types of Assemblies, Drawings (sheets, views, annotations, templates, fonts) and Feature Studios. Serde types and plain methods on them; **no evaluation** |
| 2 | **`noshape-model-core`** | regeneration, features, sheet metal, in-context (context bodies in regeneration), analysis; what regeneration needs of the FeatureScript host (`regen::custom_features`: compiled studio types, `CustomFeatureProvider`, the `CustomHost` hook; `regen::std_route`: which built-in features std runs) |
| 3 | **`noshape-custom`** | the FeatureScript host: compiling Feature Studios, custom features and custom tables, the modeling natives, the built-in features std runs, the demo Feature Studio |
| 4 | **`noshape-assembly`** | assembly evaluation, the mate solver, assembly analysis (interference, exploded views), BOM, standard content, native assembly import, in-context snapshots, the demo documents with assemblies |
| 5 | **`noshape-drawing`** | drawing views, HLR projection, sheet building, tables, export (PDF/DXF/DWG/DWT), the demo documents with drawings |
| 5 | **`noshape-model`** (facade) | re-exports model-data, model-core, custom, assembly and drawing under the old paths; holds the model integration tests (`crates/model/tests`) |
| 2–5 | `noshape-store`, `noshape-scene`, `noshape-render` | persistence, scene building, the wgpu renderer |
| 6 | `noshape-ops`, `noshape-cli` | the agent API |
| 7 | **`noshape-app-core`** | the app's base layer: theme, assets (icons, i18n tables; the files stay in `crates/app/assets`), base widgets (`ui`, `os_ui`, `os_menu`, `os_select`, `quantity_field`, …), actions, cursors, touch, the GPU context, the 3D viewport and navigation, the camera menu, the dialogs and panels the Part Studio and the assembly share (graphics preferences, named views, section view, customize toolbar, tool synonyms), `paths` (`config_dir`). Never references a studio |
| 8 | `noshape` (the binary) and, planned, one crate per studio UI above app-core: workspace/chrome, Part Studio UI, sketch UI, assembly UI, drawing UI, other studios | GPUI application |

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

- Next: the app (124k lines above app-core), one studio at a time, each at a moment agreed with its owner: variables
  (3.5k) first, then drawing UI (27k), assembly UI (23k), Part Studio (34k), each on `noshape-app-core`. What has to be
  cut first: the studios reach `files`, `workspace`, `release_dialog`, `preferences_window`, `history_panel`, `start_library`
  (hooks or moves); modules that only need app-core today and could move down with their studio or into app-core when
  shared: `sketch_mode`, `sketch_fonts`, `measure_panel`, `doc_panels`, `mouse_prefs`, `import_dialog`.
