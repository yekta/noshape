//! Hidden line removal (HLR) projection for drawings.

use glam::{DVec2, DVec3};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::ffi::ffi;
use crate::geom::p3;
use crate::shape::{Shape, ck};

/// Visibility of a projected edge segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Visibility {
    Visible,
    Hidden,
}

/// Classification of a projected edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeClass {
    /// Sharp edge between faces meeting at an angle (or free edge).
    Sharp,
    /// Tangent (G1-continuous) edge.
    Smooth,
    /// Silhouette (outline) of a curved surface; has no source edge.
    Outline,
    /// Seam / sewn edge (continuity higher than G1, e.g. cylinder seam).
    Seam,
}

/// One projected polyline in view coordinates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectedEdge {
    pub visibility: Visibility,
    pub class: EdgeClass,
    /// Points in the view plane: x = right, y = up.
    pub polyline: Vec<DVec2>,
    /// Index of the source edge in the projected shape's edges (MapShapes
    /// order), when known (`None` for silhouettes).
    pub source_edge: Option<u32>,
}

/// Project `shape` orthographically along `view_dir` (from the eye into the
/// scene) with `up` as the view's up direction. `exact = true` uses the exact
/// HLRBRep_Algo (slower, exact curves); `false` uses HLRBRep_PolyAlgo on a
/// triangulation (fast). Curves are discretized with deflection `deflection`
/// (model units; `<= 0` uses 0.01).
pub fn project_hlr_with(shape: &Shape, view_dir: DVec3, up: DVec3, exact: bool, deflection: f64) -> Result<Vec<ProjectedEdge>> {
    let out = ck(ffi::project_hlr(shape.raw(), p3(view_dir), p3(up), exact, deflection))?;
    Ok(out
        .edges
        .iter()
        .map(|e| {
            let s = e.start as usize;
            let polyline = (s..s + e.count as usize)
                .map(|i| DVec2::new(out.points[2 * i], out.points[2 * i + 1]))
                .collect();
            ProjectedEdge {
                visibility: if e.visible { Visibility::Visible } else { Visibility::Hidden },
                class: match e.kind {
                    1 => EdgeClass::Smooth,
                    2 => EdgeClass::Outline,
                    3 => EdgeClass::Seam,
                    _ => EdgeClass::Sharp,
                },
                polyline,
                source_edge: (e.source >= 0).then_some(e.source as u32),
            }
        })
        .collect())
}

/// [`project_hlr_with`] with a deflection of 1/500 of the shape size.
pub fn project_hlr(shape: &Shape, view_dir: DVec3, up: DVec3, exact: bool) -> Result<Vec<ProjectedEdge>> {
    let d = shape.bounding_box(false).map(|b| b.diagonal() / 500.0).unwrap_or(0.01);
    project_hlr_with(shape, view_dir, up, exact, d.max(1e-6))
}
