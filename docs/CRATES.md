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
| 2 | **`noshape-model-core`** | regeneration, features, FeatureScript host/std, assembly solving, in-context, analysis |
| 3 | **`noshape-drawing`** | drawing views, HLR projection, sheet building, tables, export (PDF/DXF/DWG/DWT), the demo documents with drawings |
| 3 | **`noshape-model`** (facade) | re-exports model-data, model-core and drawing under the old paths; holds the model integration tests (`crates/model/tests`) |
| 2–3 | `noshape-store`, `noshape-scene`, `noshape-render` | persistence, scene building, the wgpu renderer |
| 4 | `noshape-ops`, `noshape-cli` | the agent API |
| 5 | app crates (planned split of `noshape`): workspace/chrome, Part Studio UI, sketch UI, assembly UI, drawing UI, other studios, the binary | GPUI application |

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

- Next: the app (138k lines): `app-core` (widgets, viewport, shared panels; never references a studio), one crate per
  studio UI (Part Studio 34k, drawing 27k, assembly 23k, variables), and the binary (workspace, wiring). Then assembly
  logic out of the core.
