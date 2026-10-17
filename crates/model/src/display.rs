//! Kernel-free display model of a regenerated Part Studio: everything the UI
//! needs to draw, pick and select, with persistent [`Reference`]s attached to
//! every pickable element. Produced by regeneration; consumed by the app.

use crate::reference::{BodyId, FeatureId, Reference};
use glam::DVec3;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    Solid,
    Surface,
    Wire,
    Point,
}

/// Triangle mesh of a body; faces occupy contiguous index ranges.
#[derive(Clone, Debug, Default)]
pub struct MeshBuffers {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    /// Per face: (first index, index count) into `indices`.
    pub face_ranges: Vec<(u32, u32)>,
    /// Per edge: polyline.
    pub edges: Vec<Vec<[f32; 3]>>,
    pub vertices: Vec<[f32; 3]>,
}

#[derive(Clone, Debug)]
pub struct BodyDisplay {
    pub id: BodyId,
    pub name: String,
    pub kind: BodyKind,
    /// Linear RGBA.
    pub color: [f32; 4],
    pub hidden: bool,
    pub mesh: Arc<MeshBuffers>,
    pub face_refs: Vec<Reference>,
    pub edge_refs: Vec<Reference>,
    pub vertex_refs: Vec<Reference>,
    pub bounds: Option<(DVec3, DVec3)>,
}

/// Constraint state of a sketch entity (drives color).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SketchEntityState {
    Under,
    Full,
    Over,
    /// Inactive sketch (not being edited).
    Inactive,
}

#[derive(Clone, Debug)]
pub struct SketchCurveDisplay {
    pub entity: u32,
    pub points: Vec<[f32; 3]>,
    pub construction: bool,
    pub external: bool,
    pub state: SketchEntityState,
}

#[derive(Clone, Debug)]
pub struct SketchPointDisplay {
    pub entity: u32,
    pub position: [f32; 3],
    pub state: SketchEntityState,
    /// Endpoints of curves vs. standalone sketch points.
    pub standalone: bool,
}

#[derive(Clone, Debug)]
pub struct SketchRegionDisplay {
    pub key: Vec<u32>,
    /// Triangulated region in world space.
    pub positions: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    pub area: f64,
}

/// Coordinate frame of a sketch plane (world space).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneFrame {
    pub origin: DVec3,
    pub x: DVec3,
    pub y: DVec3,
    pub normal: DVec3,
}

impl PlaneFrame {
    pub fn to_world(&self, p: glam::DVec2) -> DVec3 {
        self.origin + self.x * p.x + self.y * p.y
    }
    pub fn to_local(&self, p: DVec3) -> glam::DVec2 {
        let d = p - self.origin;
        glam::DVec2::new(d.dot(self.x), d.dot(self.y))
    }
    /// Intersect a ray with the plane.
    pub fn intersect_ray(&self, origin: DVec3, dir: DVec3) -> Option<DVec3> {
        let denom = dir.dot(self.normal);
        if denom.abs() < 1e-12 {
            return None;
        }
        let t = (self.origin - origin).dot(self.normal) / denom;
        Some(origin + dir * t)
    }
    pub const TOP: PlaneFrame = PlaneFrame { origin: DVec3::ZERO, x: DVec3::X, y: DVec3::Y, normal: DVec3::Z };
    pub const FRONT: PlaneFrame = PlaneFrame { origin: DVec3::ZERO, x: DVec3::X, y: DVec3::Z, normal: DVec3::NEG_Y };
    pub const RIGHT: PlaneFrame = PlaneFrame { origin: DVec3::ZERO, x: DVec3::Y, y: DVec3::Z, normal: DVec3::X };
}

#[derive(Clone, Debug)]
pub struct DimensionDisplay {
    pub constraint: u32,
    /// Anchor position for the label (world).
    pub label_pos: [f32; 3],
    pub text: String,
    /// Leader/extension line segments (world).
    pub lines: Vec<[[f32; 3]; 2]>,
    pub driven: bool,
    pub state: SketchEntityState,
}

#[derive(Clone, Debug)]
pub struct ConstraintGlyph {
    pub constraint: u32,
    pub kind: &'static str,
    pub position: [f32; 3],
    pub conflicting: bool,
}

#[derive(Clone, Debug)]
pub struct SketchDisplay {
    pub feature: FeatureId,
    pub name: String,
    pub frame: PlaneFrame,
    pub curves: Vec<SketchCurveDisplay>,
    pub points: Vec<SketchPointDisplay>,
    pub regions: Vec<SketchRegionDisplay>,
    pub dimensions: Vec<DimensionDisplay>,
    pub glyphs: Vec<ConstraintGlyph>,
    pub hidden: bool,
    /// Remaining degrees of freedom (0 = fully defined).
    pub dof: usize,
}

#[derive(Clone, Debug)]
pub struct PlaneDisplay {
    pub reference: Reference,
    pub name: String,
    pub frame: PlaneFrame,
    pub half_width: f64,
    pub half_height: f64,
    pub hidden: bool,
}

#[derive(Clone, Debug)]
pub struct ConnectorDisplay {
    pub reference: Reference,
    pub name: String,
    pub origin: DVec3,
    pub x: DVec3,
    pub z: DVec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeatureStatus {
    Ok,
    Info,
    Warning,
    Error,
    Suppressed,
    /// Below the rollback bar.
    RolledBack,
}

#[derive(Clone, Debug)]
pub struct FeatureState {
    pub id: FeatureId,
    pub status: FeatureStatus,
    pub message: Option<String>,
    /// Sketch under-defined (blue in the list).
    pub underdefined: bool,
    /// Milliseconds spent regenerating this feature.
    pub time_ms: f32,
}

#[derive(Clone, Debug, Default)]
pub struct StudioDisplay {
    pub bodies: Vec<BodyDisplay>,
    pub sketches: Vec<SketchDisplay>,
    pub planes: Vec<PlaneDisplay>,
    pub connectors: Vec<ConnectorDisplay>,
    pub features: Vec<FeatureState>,
    /// Origin visible.
    pub show_origin: bool,
    /// Total regeneration time.
    pub regen_ms: f32,
}

impl StudioDisplay {
    /// Display of an empty Part Studio: origin and the three default planes.
    pub fn defaults() -> Self {
        let half = 0.05;
        let mk = |p: crate::reference::DatumPlane, name: &str, frame| PlaneDisplay {
            reference: Reference::Plane(p),
            name: name.into(),
            frame,
            half_width: half,
            half_height: half,
            hidden: false,
        };
        StudioDisplay {
            planes: vec![
                mk(crate::reference::DatumPlane::Top, "Top", PlaneFrame::TOP),
                mk(crate::reference::DatumPlane::Front, "Front", PlaneFrame::FRONT),
                mk(crate::reference::DatumPlane::Right, "Right", PlaneFrame::RIGHT),
            ],
            show_origin: true,
            ..Default::default()
        }
    }

    pub fn feature_state(&self, id: FeatureId) -> Option<&FeatureState> {
        self.features.iter().find(|f| f.id == id)
    }

    pub fn body(&self, id: &BodyId) -> Option<&BodyDisplay> {
        self.bodies.iter().find(|b| &b.id == id)
    }

    pub fn bounds(&self) -> Option<(DVec3, DVec3)> {
        let mut acc: Option<(DVec3, DVec3)> = None;
        for b in self.bodies.iter().filter(|b| !b.hidden) {
            if let Some((lo, hi)) = b.bounds {
                acc = Some(match acc {
                    None => (lo, hi),
                    Some((a, c)) => (a.min(lo), c.max(hi)),
                });
            }
        }
        acc
    }
}
