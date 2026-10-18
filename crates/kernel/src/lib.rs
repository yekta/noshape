//! # noshape-kernel
//!
//! Safe Rust wrapper over OpenCascade (OCCT 8) for noshape: shapes and
//! topology exploration, geometry queries, construction from sketch data,
//! primitives, feature operations with full topological history, booleans,
//! tessellation, mass properties, HLR projection and file I/O.
//!
//! The kernel knows nothing about features: the model layer composes these
//! operations and uses [`History`] to carry persistent names.
//!
//! Every OCCT exception is caught at the C++ bridge boundary and returned as
//! a [`KernelError`]; no operation panics on modeling failures.
//!
//! Sub-shapes are referred to by index in `TopExp::MapShapes` order
//! ([`Shape::sub_shapes`]), consistently across history records,
//! tessellation ranges and HLR source edges.

// `!(x > 0.0)` is used deliberately to also reject NaN inputs.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

#[allow(dead_code)]
mod ffi;

pub mod boolean;
pub mod construct;
pub mod error;
pub mod geom;
pub mod history;
pub mod hlr;
pub mod io;
pub mod mesh;
pub mod ops;
pub mod props;
pub mod shape;
pub mod transform;

pub use boolean::{
    BooleanOp, BooleanOptions, Glue, boolean, common, cut, fuse, fuse_all, imprint, project_curves, section,
    section_caps, section_plane, section_wires, split, split_by_plane, unify_same_domain,
};
pub use construct::*;
pub use error::{KernelError, Result};
pub use geom::{BSplineCurveData, BoundingBox, Curve, EdgeInfo, FaceProjection, Plane, PointClass, Surface};
pub use history::{History, HistoryRecord, OpResult, Relation};
pub use hlr::{EdgeClass, ProjectedEdge, Visibility, project_hlr, project_hlr_with};
pub use io::{
    ExportShape, ImportedShape, LengthUnit, StlFormat, read_brep, read_iges, read_mesh, read_step, write_brep, write_iges,
    write_obj, write_obj_mesh, write_step, write_step_shapes, write_stl, write_stl_mesh,
};
pub use mesh::{EdgeRange, FaceRange, MeshOptions, Tessellation, tessellate, tessellate_with};
pub use ops::*;
pub use props::{Distance, MassProperties, angle_between, distance, linear_properties, surface_properties, volume_properties};
pub use shape::{AncestorMap, Orientation, Shape, ShapeType};
pub use transform::{Transform, transform};

/// Re-export of the math types used in the API.
pub use glam::{DMat3, DMat4, DVec2, DVec3};
