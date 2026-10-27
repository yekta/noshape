//! noshape-model: the headless CAD document model, as one path.
//!
//! A facade over the layered crates (docs/CRATES.md): `noshape-model-data` (the document data), `noshape-model-core`
//! (regeneration, features), `noshape-custom` (the FeatureScript host), `noshape-assembly` (assembly evaluation, solving, analysis) and
//! `noshape-drawing` (drawing views and export). Every module keeps its old path: `noshape_model::document`,
//! `noshape_model::regen`, `noshape_model::assembly::eval`, `noshape_model::drawing::views`, …

pub use noshape_model_core::*;

/// Feature Studios: data and the FeatureScript host.
pub use noshape_custom::custom;

/// Assembly data and logic.
pub use noshape_assembly::assembly;
/// Assembly contexts (the snapshots of an evaluated assembly come from noshape-assembly).
pub use noshape_assembly::in_context;
pub use noshape_assembly::native_assembly;

/// Drawing data and logic.
pub use noshape_drawing::drawing;

/// Demo documents (the ones with assemblies come from noshape-assembly, with drawings from noshape-drawing).
pub mod demo {
    pub use noshape_assembly::demo::*;
    pub use noshape_drawing::demo::demo_document_with_drawing;
}

/// Demo fabrication documents.
pub mod demo_fab {
    pub use noshape_drawing::demo::custom_tables_document;
    pub use noshape_model_core::demo_fab::*;
}

/// The extension traits that give data types their logic methods: `use noshape_model::prelude::*;`.
pub mod prelude {
    pub use noshape_drawing::prelude::*;
    pub use noshape_model_core::prelude::*;
}
