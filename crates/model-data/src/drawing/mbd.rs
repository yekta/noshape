//! MBD annotations in drawings (live behaviours "MBD datums in drawings",
//! V5 / CR18 / A65): a Part Studio's Model definitions datums appear once,
//! in the first view of the suggested set that shows their face edge-on, as
//! ordinary drawing datums (filled triangle on the edge, a stem, the boxed
//! letter). They stay linked: the anchor follows the face, the box moves
//! only along the face normal, Delete hides them (the MBD status panel lists
//! them and "Show" brings them back where they were), and a view's
//! "Show annotations from model" off hides its own.

use super::ViewId;
use crate::document::TabId;
use serde::{Deserialize, Serialize};

/// A drawing annotation showing a Part Studio's MBD annotation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelRef {
    pub tab: TabId,
    pub id: u32,
    /// The view showing it.
    pub view: ViewId,
    /// The box's distance from the edge along the face normal (sheet mm).
    pub offset: f64,
    /// Deleted from the drawing (MBD status › Show restores it).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}

/// Live drawing/1071: the box centre sits about 0.28 in from the edge
/// (datum size 0.12 in, a short stem, the 0.24 in box).
pub const DEFAULT_OFFSET: f64 = 7.2;
