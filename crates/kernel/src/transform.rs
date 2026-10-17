//! Rigid / affine transforms of shapes (copy, move, rotate, scale, mirror).

use glam::{DMat3, DMat4, DVec3, DVec4};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::ffi::ffi;
use crate::geom::Plane;
use crate::history::OpResult;
use crate::shape::{Shape, ck};

/// A transform applied by [`transform`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Transform {
    Translate(DVec3),
    /// Rotation by `angle` radians around the axis (`origin`, `axis`).
    Rotate { origin: DVec3, axis: DVec3, angle: f64 },
    /// Uniform scale about `center`.
    Scale { center: DVec3, factor: f64 },
    /// Non-uniform scale about `center` (geometry becomes B-splines).
    ScaleXyz { center: DVec3, factors: DVec3 },
    /// Mirror about a plane.
    Mirror(Plane),
    /// General affine matrix (column-major glam `DMat4`, last row ignored).
    Matrix(DMat4),
}

impl Transform {
    /// The rigid transform mapping frame `from` onto frame `to`
    /// (origins, normals and X directions).
    pub fn frame_to_frame(from: &Plane, to: &Plane) -> Transform {
        let basis = |p: &Plane| DMat3::from_cols(p.unit_x(), p.unit_y(), p.unit_normal());
        let r = basis(to) * basis(from).transpose();
        let t = to.origin - r * from.origin;
        Transform::Matrix(DMat4::from_cols(
            r.x_axis.extend(0.0),
            r.y_axis.extend(0.0),
            r.z_axis.extend(0.0),
            t.extend(1.0),
        ))
    }

    /// The transform as a 4x4 matrix.
    pub fn matrix(&self) -> DMat4 {
        match *self {
            Transform::Translate(v) => DMat4::from_translation(v),
            Transform::Rotate { origin, axis, angle } => {
                DMat4::from_translation(origin)
                    * DMat4::from_axis_angle(axis.normalize_or_zero(), angle)
                    * DMat4::from_translation(-origin)
            }
            Transform::Scale { center, factor } => {
                DMat4::from_translation(center) * DMat4::from_scale(DVec3::splat(factor)) * DMat4::from_translation(-center)
            }
            Transform::ScaleXyz { center, factors } => {
                DMat4::from_translation(center) * DMat4::from_scale(factors) * DMat4::from_translation(-center)
            }
            Transform::Mirror(p) => {
                let n = p.unit_normal();
                let r = DMat3::IDENTITY - 2.0 * DMat3::from_cols(n * n.x, n * n.y, n * n.z);
                let t = 2.0 * n * p.origin.dot(n);
                DMat4::from_cols(
                    r.x_axis.extend(0.0),
                    r.y_axis.extend(0.0),
                    r.z_axis.extend(0.0),
                    DVec4::new(t.x, t.y, t.z, 1.0),
                )
            }
            Transform::Matrix(m) => m,
        }
    }

    fn rows(&self) -> [f64; 12] {
        let m = self.matrix();
        let mut r = [0.0; 12];
        for row in 0..3 {
            for col in 0..4 {
                r[row * 4 + col] = m.col(col)[row];
            }
        }
        r
    }

    fn is_similarity(&self) -> bool {
        match self {
            Transform::ScaleXyz { factors, .. } => {
                (factors.x - factors.y).abs() < 1e-12 && (factors.x - factors.z).abs() < 1e-12
            }
            Transform::Matrix(m) => {
                let a = DMat3::from_mat4(*m);
                let ata = a.transpose() * a;
                let s = ata.x_axis.x;
                s > 0.0
                    && (ata - DMat3::from_diagonal(DVec3::splat(s)))
                        .to_cols_array()
                        .iter()
                        .all(|v| v.abs() < 1e-9 * s.max(1.0))
            }
            _ => true,
        }
    }
}

/// Transform a shape. Rigid motions, mirrors and uniform scales use
/// BRepBuilderAPI_Transform (with `copy`, the geometry is duplicated so the
/// result shares nothing with the input); non-uniform scales/affine matrices
/// use BRepBuilderAPI_GTransform. History (arg 0): every sub-shape is
/// `Modified` into its transformed image.
pub fn transform(shape: &Shape, t: &Transform, copy: bool) -> Result<OpResult> {
    let rows = t.rows();
    let r = if t.is_similarity() {
        ck(ffi::op_transform(shape.raw(), &rows, copy))?
    } else {
        ck(ffi::op_gtransform(shape.raw(), &rows))?
    };
    Ok(OpResult::from_ffi(r))
}

impl Shape {
    /// Convenience: transformed copy of the shape (history discarded).
    pub fn transformed(&self, t: &Transform) -> Result<Shape> {
        Ok(transform(self, t, true)?.shape)
    }
    /// Convenience: translated shape (shares geometry; history discarded).
    pub fn translated(&self, v: DVec3) -> Result<Shape> {
        Ok(transform(self, &Transform::Translate(v), false)?.shape)
    }
}
