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

/// A hasher for the sketch's own small integer ids (its entity and constraint maps, looked up all through a solve and
/// a constructive placement): a multiplicative mix instead of SipHash.
#[derive(Default, Clone, Copy)]
pub struct IdHasher(u64);

impl std::hash::Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0.rotate_left(5) ^ u64::from(*b)).wrapping_mul(0x517c_c1b7_2722_0a95);
        }
    }
    fn write_u32(&mut self, n: u32) {
        self.0 = (self.0.rotate_left(5) ^ u64::from(n)).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
}

/// [`IdHasher`]'s builder.
pub type IdState = std::hash::BuildHasherDefault<IdHasher>;
