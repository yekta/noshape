//! Operation history: how sub-shapes of the inputs map to sub-shapes of the
//! output. This is the basis of persistent topological naming in the model
//! layer.
//!
//! All indices refer to `TopExp::MapShapes` order, i.e. the order of
//! [`Shape::sub_shapes`](crate::Shape::sub_shapes) of the respective input
//! argument / output shape.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::ffi::ffi;
use crate::shape::{Shape, ShapeType};

/// Relation between an input sub-shape and an output sub-shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum Relation {
    /// The output sub-shape is a modified version of the input sub-shape (same
    /// dimension; e.g. a face trimmed by a boolean, or moved by a transform).
    /// A split input yields several `Modified` records.
    Modified = 0,
    /// The output sub-shape was generated from the input sub-shape (usually of
    /// higher dimension; e.g. profile edge → extrusion side face, filleted
    /// edge → blend face, vertex → lateral edge).
    Generated = 1,
    /// The input sub-shape is not present in the output in any form.
    /// (`output_kind` is [`ShapeType::Unknown`], `output_index` is `u32::MAX`.)
    Deleted = 2,
    /// The input sub-shape is present unchanged in the output (same TShape and
    /// location).
    Kept = 3,
    /// Start cap of a sweep-type operation: the output sub-shape is the copy
    /// of the input sub-shape at the start (prism bottom, revolve start, loft
    /// first section, pipe start).
    First = 4,
    /// End cap of a sweep-type operation (prism top, revolve end, loft last
    /// section, pipe end, up-to target face).
    Last = 5,
}

impl Relation {
    fn from_u8(v: u8) -> Relation {
        match v {
            0 => Relation::Modified,
            1 => Relation::Generated,
            2 => Relation::Deleted,
            3 => Relation::Kept,
            4 => Relation::First,
            _ => Relation::Last,
        }
    }
}

/// One history record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HistoryRecord {
    /// Index of the input argument of the operation (see each operation's docs
    /// for the argument order; for booleans: objects first, then tools).
    pub arg: u32,
    pub input_kind: ShapeType,
    /// Index among the argument's sub-shapes of `input_kind`. When the record
    /// describes a whole sweep cap without a specific source sub-shape, this is
    /// face 0 (or wire/edge 0 if the argument has no face).
    pub input_index: u32,
    pub relation: Relation,
    pub output_kind: ShapeType,
    /// Index among the output's sub-shapes of `output_kind`.
    pub output_index: u32,
}

/// The history of one operation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct History {
    pub records: Vec<HistoryRecord>,
}

/// Result of a feature operation: the output shape and its history.
#[derive(Debug, Clone)]
pub struct OpResult {
    pub shape: Shape,
    pub history: History,
}

impl OpResult {
    pub(crate) fn from_ffi(o: ffi::OpOut) -> OpResult {
        OpResult {
            shape: Shape::from_ptr(o.shape),
            history: History::from_ffi(o.history),
        }
    }
}

impl History {
    pub(crate) fn from_ffi(recs: Vec<ffi::HistRec>) -> History {
        let mut records: Vec<HistoryRecord> = recs
            .into_iter()
            .map(|r| HistoryRecord {
                arg: r.arg,
                input_kind: ShapeType::from_u8(r.in_kind),
                input_index: r.in_index,
                relation: Relation::from_u8(r.relation),
                output_kind: ShapeType::from_u8(r.out_kind),
                output_index: r.out_index,
            })
            .collect();
        // Records can be produced twice through container expansion; keep the
        // first occurrence, preserving deterministic order.
        let mut seen = std::collections::HashSet::with_capacity(records.len());
        records.retain(|r| seen.insert(*r));
        History { records }
    }

    /// Records whose input is (`arg`, `kind`, `index`).
    pub fn of_input(&self, arg: u32, kind: ShapeType, index: u32) -> impl Iterator<Item = &HistoryRecord> {
        self.records
            .iter()
            .filter(move |r| r.arg == arg && r.input_kind == kind && r.input_index == index)
    }

    /// Records whose output is (`kind`, `index`) — i.e. where an output
    /// sub-shape came from.
    pub fn of_output(&self, kind: ShapeType, index: u32) -> impl Iterator<Item = &HistoryRecord> {
        self.records
            .iter()
            .filter(move |r| r.output_kind == kind && r.output_index == index)
    }

    /// Output indices (of the same kind) that the input became: `Modified` or
    /// `Kept` images.
    pub fn modified(&self, arg: u32, kind: ShapeType, index: u32) -> Vec<u32> {
        self.of_input(arg, kind, index)
            .filter(|r| matches!(r.relation, Relation::Modified | Relation::Kept) && r.output_kind == kind)
            .map(|r| r.output_index)
            .collect()
    }

    /// Output sub-shapes generated from the input.
    pub fn generated(&self, arg: u32, kind: ShapeType, index: u32) -> Vec<(ShapeType, u32)> {
        self.of_input(arg, kind, index)
            .filter(|r| r.relation == Relation::Generated)
            .map(|r| (r.output_kind, r.output_index))
            .collect()
    }

    /// True if the input sub-shape was deleted.
    pub fn is_deleted(&self, arg: u32, kind: ShapeType, index: u32) -> bool {
        self.of_input(arg, kind, index).any(|r| r.relation == Relation::Deleted)
    }

    /// Output faces that are unchanged copies of input faces (same TShape).
    pub fn kept_faces(&self) -> Vec<u32> {
        let mut v: Vec<u32> = self
            .records
            .iter()
            .filter(|r| r.relation == Relation::Kept && r.output_kind == ShapeType::Face)
            .map(|r| r.output_index)
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Output faces forming the start cap(s) of a sweep-type operation.
    pub fn first_faces(&self) -> Vec<u32> {
        self.faces_with(Relation::First)
    }

    /// Output faces forming the end cap(s) of a sweep-type operation.
    pub fn last_faces(&self) -> Vec<u32> {
        self.faces_with(Relation::Last)
    }

    fn faces_with(&self, rel: Relation) -> Vec<u32> {
        let mut v: Vec<u32> = self
            .records
            .iter()
            .filter(|r| r.relation == rel && r.output_kind == ShapeType::Face)
            .map(|r| r.output_index)
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Index from input key `(arg, kind, index)` to its records, for fast
    /// repeated lookups.
    pub fn by_input(&self) -> HashMap<(u32, ShapeType, u32), Vec<HistoryRecord>> {
        let mut m: HashMap<_, Vec<HistoryRecord>> = HashMap::new();
        for r in &self.records {
            m.entry((r.arg, r.input_kind, r.input_index)).or_default().push(*r);
        }
        m
    }

    /// Index from output key `(kind, index)` to its records.
    pub fn by_output(&self) -> HashMap<(ShapeType, u32), Vec<HistoryRecord>> {
        let mut m: HashMap<_, Vec<HistoryRecord>> = HashMap::new();
        for r in &self.records {
            if r.relation != Relation::Deleted {
                m.entry((r.output_kind, r.output_index)).or_default().push(*r);
            }
        }
        m
    }
}
