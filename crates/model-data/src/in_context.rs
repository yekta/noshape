//! Assembly contexts of a Part Studio: the stored snapshot data (the snapshotting and the regeneration bodies are
//! in noshape-model).

use crate::document::TabId;
use crate::reference::BodyId;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// One instance's part in the snapshot, in the Part Studio's coordinates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContextBody {
    /// Instance path in the assembly (top-level first).
    pub path: Vec<u64>,
    /// Instance name ("Part 1 <2>") and part name.
    pub instance: String,
    pub part: String,
    /// Display color (linear RGBA).
    pub color: [f32; 4],
    /// The geometry (OCCT BREP text, already placed).
    pub brep: Arc<str>,
}

/// An Assembly context of a Part Studio (the "Assembly contexts" list
/// above its Feature list).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AssemblyContext {
    pub id: u32,
    pub name: String,
    pub assembly: TabId,
    /// The primary instance (solid context arrow) and its part.
    pub primary: Vec<u64>,
    #[serde(default)]
    pub primary_body: Option<BodyId>,
    pub bodies: Vec<ContextBody>,
    /// Bumped by Update context (features referencing it regenerate).
    #[serde(default)]
    pub revision: u32,
    /// Context instances hidden in the Part Studio (Hide/show context instances).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hidden: Vec<Vec<u64>>,
    /// The assembly's state when the snapshot was taken ([`state_hash`]);
    /// a different current state means "Update available".
    #[serde(default)]
    pub state: u64,
    /// Create Part Studio in context: where the Part Studio's origin sits
    /// in the assembly (Insert and go to assembly places its parts there).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<crate::assembly::Placement>,
}
