//! noshape-ops: the headless operations layer.
//!
//! Every operation an agent can perform (CLI or MCP) is an [`registry::Op`]
//! whose handler calls the same model code the UI uses: the feature
//! parameter schemas, the regenerator and the document commit (one
//! microversion per edit, undoable). GUI-free logic the UI needs as well
//! (the document [`session`], [`derived`] part providers) lives here and the
//! app re-exports it. See `docs/AGENT_API.md`.

pub use noshape_kernel::{OCCT_PATCHES, VERSION};
/// The documents folder the app lists ("Owned by me"): where `document_create` puts a document by default.
pub use noshape_scene::documents_dir;

pub mod about;
pub mod analysis;
pub mod analysis_tools;
pub mod appearance;
pub mod asm_config;
pub mod assembly;
pub mod config;
pub mod coverage;
pub mod derived;
pub mod doc;
pub mod doc_library;
pub mod doc_trash;
pub mod drawing;
pub mod drawing_export;
pub mod examples;
pub mod history_filter;
pub mod docedit;
pub mod error;
pub mod export;
pub mod file_preview;
pub mod import;
pub mod import_documents;
pub mod in_context;
pub mod install;
pub mod live;
pub mod material_data;
pub mod ops;
pub mod parity;
pub mod prefs;
pub mod reference;
pub mod refs;
pub mod release;
pub mod release_conditions;
pub mod release_settings;
pub mod custom_properties;
pub mod registry;
pub mod session;
pub mod sheetmetal;
pub mod shortcuts;
pub mod show_code;
pub mod sketch_export;
pub mod sketch_use;
pub mod standard_props;
pub mod studio;
pub mod tab_move;
pub mod tool_search;
pub mod ui_parity;
pub mod values;
pub mod variable_studio;
pub mod versions;
pub mod vs_config;
pub mod window;
pub mod workspace_update;

pub use doc::{Ctx, OpenDoc};
pub use window::UiRequest;
pub use error::OpError;
