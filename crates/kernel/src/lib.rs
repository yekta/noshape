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
mod brep_bridge;
mod mesh_bridge;
pub use mesh_bridge::{set_rust_mesh, set_rust_mesh_everywhere};

/// Rhino (.3dm) export is built in (openNURBS, OPENNURBS_ROOT at build time).
pub const RHINO_EXPORT: bool = cfg!(noshape_opennurbs);

/// noshape's version as files and the agent API report it: the release
/// tag the release workflow stamps (`NOSHAPE_VERSION`, "0.4.8"), else the
/// crate's (dev builds); the app shows the same (qa VERSION-0.4.6).
pub const VERSION: &str = match option_env!("NOSHAPE_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

pub mod bool_corpus;
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
mod step_out;
pub mod surfacing;
pub mod transform;

pub use brep_bridge::{brep_body, last_rust_decline, set_rust_boolean, set_rust_boolean_only};
pub use boolean::{
    BooleanOp, BooleanOptions, Glue, boolean, common, cut, fuse, fuse_all, imprint, project_curves, section,
    kernel_profile_mark, last_boolean_path, last_unify_path, section_caps, set_parallel_booleans, section_plane, section_wires, split, split_by_plane, unify_same_domain, unify_same_domain_whole, with_cancel,
};
pub use construct::*;
pub use error::{KernelError, Result};
pub use geom::{BSplineCurveData, BoundingBox, Curve, EdgeInfo, FaceProjection, Plane, PointClass, Surface};
pub use history::{History, HistoryRecord, OpResult, Relation};
pub use hlr::{EdgeClass, ProjectedEdge, Visibility, project_hlr, project_hlr_with};
pub use io::{
    ExportShape, ImportedShape, LengthUnit, StepOptions, StepSchema, StlFormat, read_brep, read_iges, read_mesh, read_step, write_brep,
    rhino_brep_count, write_acis, write_iges, write_jt, write_obj, write_obj_mesh, write_parasolid, write_parasolid_assembly, write_parasolid_parts, write_parasolid_version, PARASOLID_VERSIONS, write_pvz, read_parasolid, read_parasolid_parts, read_native_parts, read_native_parts_configuration, read_native_assembly, read_native_assembly_in, extract_zip, NativeComponent, read_acis_parts, read_jt_parts, read_rhino_parts, read_pvz, read_pvz_parts, read_pvz_instances, read_step_assembly, read_parasolid_assembly, write_step_assembly, StepAssembly, StepInstance, PvzInstance, read_mesh_bodies, write_rhino, write_step, write_step_shapes, write_step_with, write_stl, write_stl_mesh,
};
pub use mesh::{EdgeRange, FaceRange, MeshOptions, Tessellation, tessellate, tessellate_with};
pub use ops::*;
pub use props::{Distance, MassProperties, angle_between, distance, linear_properties, mass_properties, surface_properties, volume_properties};
pub use shape::{AncestorMap, Orientation, Shape, ShapeType, ShownTopology};
pub use surfacing::{
    BSplineCurve2dData, BSplineSurfaceData, Developable, Placement2d, SurfaceFit, SurfaceFitOptions, bspline_surface_from_grid, fit_surface,
    isocline_curves, make_bspline_surface, max_deviation, wrap_faces,
};
pub use transform::{Transform, transform};

/// Re-export of the math types used in the API.
pub use glam::{DMat3, DMat4, DVec2, DVec3};
