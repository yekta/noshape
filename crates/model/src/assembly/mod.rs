//! Assemblies: instances of parts (from Part Studios) and sub-assemblies,
//! positioned by mates between mate connectors. The mate solver lives in
//! `assembly::solver`.

use crate::document::TabId;
use crate::configuration::ConfigValues;
use crate::reference::{BodyId, Reference};
use noshape_units::ExprValue;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct InstanceId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MateId(pub u64);

/// What an instance refers to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum InstanceSource {
    /// One part of a Part Studio tab.
    Part { studio: TabId, body: BodyId, config: ConfigValues },
    /// All parts of a Part Studio as one rigid instance.
    PartStudio { studio: TabId, config: ConfigValues },
    /// A sub-assembly tab.
    Assembly { assembly: TabId },
}

/// Rigid transform stored as translation (m) + rotation quaternion (x, y, z, w).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub translation: [f64; 3],
    pub rotation: [f64; 4],
}

impl Default for Placement {
    fn default() -> Self {
        Self { translation: [0.0; 3], rotation: [0.0, 0.0, 0.0, 1.0] }
    }
}

impl Placement {
    pub fn to_dmat4(&self) -> glam::DMat4 {
        let [x, y, z, w] = self.rotation;
        glam::DMat4::from_rotation_translation(glam::DQuat::from_xyzw(x, y, z, w).normalize(), self.translation.into())
    }
    pub fn from_dmat4(m: &glam::DMat4) -> Self {
        let (_, r, t) = m.to_scale_rotation_translation();
        Self { translation: t.into(), rotation: [r.x, r.y, r.z, r.w] }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Instance {
    pub id: InstanceId,
    pub name: String,
    pub source: InstanceSource,
    pub placement: Placement,
    #[serde(default)]
    pub fixed: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub suppressed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MateType {
    Fastened,
    Revolute,
    Slider,
    Planar,
    Cylindrical,
    PinSlot,
    Ball,
    Parallel,
    Tangent,
    Width,
}

impl MateType {
    pub const ALL: [MateType; 10] = [
        MateType::Fastened,
        MateType::Revolute,
        MateType::Slider,
        MateType::Planar,
        MateType::Cylindrical,
        MateType::PinSlot,
        MateType::Ball,
        MateType::Parallel,
        MateType::Tangent,
        MateType::Width,
    ];
    pub fn name(self) -> &'static str {
        match self {
            MateType::Fastened => "Fastened",
            MateType::Revolute => "Revolute",
            MateType::Slider => "Slider",
            MateType::Planar => "Planar",
            MateType::Cylindrical => "Cylindrical",
            MateType::PinSlot => "Pin slot",
            MateType::Ball => "Ball",
            MateType::Parallel => "Parallel",
            MateType::Tangent => "Tangent",
            MateType::Width => "Width",
        }
    }
    /// Remaining degrees of freedom.
    pub fn dof(self) -> u32 {
        match self {
            MateType::Fastened => 0,
            MateType::Revolute | MateType::Slider => 1,
            MateType::Cylindrical | MateType::PinSlot => 2,
            MateType::Planar | MateType::Ball => 3,
            MateType::Parallel => 4,
            MateType::Tangent => 5,
            MateType::Width => 5,
        }
    }
}

/// A mate connector on an instance: a reference into the instance's source
/// geometry (resolved in the source's own coordinates) plus local offsets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MateConnectorRef {
    pub instance: InstanceId,
    /// Path through sub-assemblies (outermost first) when the connector is
    /// on a nested instance.
    #[serde(default)]
    pub path: Vec<InstanceId>,
    pub reference: Reference,
    /// Flip primary axis / reorient secondary axis (90° steps).
    #[serde(default)]
    pub flip: bool,
    #[serde(default)]
    pub rotate_quarters: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Limits {
    pub min: Option<ExprValue>,
    pub max: Option<ExprValue>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mate {
    pub id: MateId,
    pub name: String,
    pub mate_type: MateType,
    pub a: MateConnectorRef,
    pub b: MateConnectorRef,
    /// Offsets along/around the first connector's axes.
    #[serde(default)]
    pub offset_z: Option<ExprValue>,
    #[serde(default)]
    pub offset_x: Option<ExprValue>,
    #[serde(default)]
    pub offset_y: Option<ExprValue>,
    #[serde(default)]
    pub rotation: Option<ExprValue>,
    #[serde(default)]
    pub flip_primary: bool,
    #[serde(default)]
    pub reorient_secondary: u8,
    #[serde(default)]
    pub limits: Vec<(String, Limits)>,
    #[serde(default)]
    pub suppressed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationType {
    Gear,
    RackAndPinion,
    Screw,
    Linear,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Relation {
    pub id: MateId,
    pub name: String,
    pub relation_type: RelationType,
    pub mates: [MateId; 2],
    pub ratio: ExprValue,
    #[serde(default)]
    pub reverse: bool,
    #[serde(default)]
    pub suppressed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub id: MateId,
    pub name: String,
    pub instances: Vec<InstanceId>,
    #[serde(default)]
    pub suppressed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NamedPosition {
    pub name: String,
    pub placements: Vec<(InstanceId, Placement)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExplodedView {
    pub name: String,
    pub steps: Vec<ExplodeStep>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExplodeStep {
    pub instances: Vec<InstanceId>,
    pub translation: [f64; 3],
    pub rotation_axis: Option<[f64; 3]>,
    pub rotation_angle: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Assembly {
    pub instances: Vec<Instance>,
    pub mates: Vec<Mate>,
    #[serde(default)]
    pub relations: Vec<Relation>,
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(default)]
    pub named_positions: Vec<NamedPosition>,
    #[serde(default)]
    pub exploded_views: Vec<ExplodedView>,
}
pub mod solver;
