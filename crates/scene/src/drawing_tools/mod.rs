//! The drawing editor's tools, GUI-free, shared by the app's Drawing tab and
//! the agent API (docs/handoff/agent-api.md, "Drawing tools extraction
//! plan"). Slice 1: the tool, pick, selection, drag and overlay types and the
//! vector helpers. Slice 2: picking ([`SheetPick`]). Slice 3: the smart
//! dimension, grips and drags, and the pending annotation / view previews.
//! Slice 4: the dialogs' options as plain values ([`ViewOptions`] etc.).
//! Slice 5: what the tools commit (`commit.rs`).

pub mod geom;
mod commit;
mod jog;
pub mod leaders;
mod opts;
mod overlay;
mod pending;
mod pick;
mod smart;
mod sketch;
mod types;

pub use commit::*;
pub use overlay::{Overlay, SnapMark};
pub use jog::{CentermarkArms, seg_dist};
pub use leaders::LeaderHit;
pub use opts::*;
pub use pending::ToolValues;
pub use pick::SheetPick;
pub use smart::{Refs, same, view_of};
pub use sketch::{SkCon, SkTool};
pub use types::*;
