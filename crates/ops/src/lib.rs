//! noshape-ops: the headless operations layer.
//!
//! Every operation an agent can perform (CLI or MCP) is an [`registry::Op`]
//! whose handler calls the same model code the UI uses: the feature
//! parameter schemas, the regenerator and the document commit (one
//! microversion per edit, undoable). GUI-free logic the UI needs as well
//! (the document [`session`], [`derived`] part providers) lives here and the
//! app re-exports it. See `docs/AGENT_API.md`.

pub mod analysis;
pub mod assembly;
pub mod config;
pub mod coverage;
pub mod derived;
pub mod doc;
pub mod docedit;
pub mod error;
pub mod export;
pub mod ops;
pub mod refs;
pub mod registry;
pub mod session;
pub mod sketch_use;
pub mod studio;
pub mod values;

pub use doc::{Ctx, OpenDoc};
pub use error::OpError;
