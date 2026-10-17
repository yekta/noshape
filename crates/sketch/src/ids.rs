//! Stable identifiers for sketch entities and constraints.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable identifier of a sketch entity (point, line, arc, ...).
///
/// Ids are never reused within one sketch, so they can be referenced by
/// downstream features ("the region bounded by line 3 and arc 5").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityId(pub u32);

/// Stable identifier of a constraint or dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConstraintId(pub u32);

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "e{}", self.0)
    }
}

impl fmt::Display for ConstraintId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "c{}", self.0)
    }
}
