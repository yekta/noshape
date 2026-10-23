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

impl InstanceId {
    /// The assembly origin (ground): a fixed frame at the world origin that
    /// mates and assembly mate connectors can reference.
    pub const ORIGIN: InstanceId = InstanceId(0);
}

/// What an instance refers to.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum InstanceSource {
    /// One part of a Part Studio tab.
    Part { studio: TabId, body: BodyId, config: ConfigValues },
    /// All parts of a Part Studio as one rigid instance.
    PartStudio { studio: TabId, config: ConfigValues },
    /// A sub-assembly tab, in one of its configurations.
    Assembly {
        assembly: TabId,
        #[serde(default, skip_serializing_if = "ConfigValues::is_empty")]
        config: ConfigValues,
    },
    /// Mirrored (opposite-handed) version of one part: the part's geometry
    /// reflected across the plane through `point` with normal `normal`
    /// (Part Studio coordinates). Created by mirrors of handed parts.
    MirroredPart { studio: TabId, body: BodyId, config: ConfigValues, point: [f64; 3], normal: [f64; 3] },
    /// A Part Studio of another document pinned to a version: `key` is the
    /// app's external reference (`ext:<microversion>|<tab>|<path>`), resolved
    /// through [`eval::ExternalStudios`]. `body`: one part (`None`: the whole
    /// Part Studio as one rigid instance).
    External {
        key: String,
        #[serde(default)]
        body: Option<BodyId>,
        #[serde(default)]
        config: ConfigValues,
    },
    /// A standard content part (fastener) of the built-in library.
    Standard { spec: standard::StandardSpec },
    /// A sketch of a Part Studio (Insert › Sketches): its curves as a wire
    /// body that can be positioned and mated like a part.
    Sketch { studio: TabId, sketch: crate::reference::FeatureId, config: ConfigValues },
}

impl InstanceSource {
    /// The configuration the source is used in (None for standard content).
    pub fn config_mut(&mut self) -> Option<&mut ConfigValues> {
        match self {
            InstanceSource::Part { config, .. }
            | InstanceSource::PartStudio { config, .. }
            | InstanceSource::Assembly { config, .. }
            | InstanceSource::MirroredPart { config, .. }
            | InstanceSource::External { config, .. }
            | InstanceSource::Sketch { config, .. } => Some(config),
            InstanceSource::Standard { .. } => None,
        }
    }

    /// The opposite-handed version of a part source (mirrored parts turn
    /// back into the plain part). `None` for Part Studios and assemblies.
    pub fn mirrored(&self) -> Option<InstanceSource> {
        match self {
            InstanceSource::Part { studio, body, config } => Some(InstanceSource::MirroredPart { studio: *studio, body: body.clone(), config: config.clone(), point: [0.0; 3], normal: [1.0, 0.0, 0.0] }),
            InstanceSource::MirroredPart { studio, body, config, .. } => Some(InstanceSource::Part { studio: *studio, body: body.clone(), config: config.clone() }),
            _ => None,
        }
    }
    /// Part Studio the source comes from (parts and rigid Part Studios).
    pub fn studio(&self) -> Option<TabId> {
        match self {
            InstanceSource::Part { studio, .. } | InstanceSource::PartStudio { studio, .. } | InstanceSource::MirroredPart { studio, .. } | InstanceSource::Sketch { studio, .. } => Some(*studio),
            InstanceSource::Assembly { .. } | InstanceSource::External { .. } | InstanceSource::Standard { .. } => None,
        }
    }
    /// Standard content spec (instances of the fastener library).
    pub fn standard(&self) -> Option<&standard::StandardSpec> {
        match self {
            InstanceSource::Standard { spec } => Some(spec),
            _ => None,
        }
    }
    /// External reference key (instances inserted from another document).
    pub fn external_key(&self) -> Option<&str> {
        match self {
            InstanceSource::External { key, .. } => Some(key),
            _ => None,
        }
    }
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
    /// Subassembly instances: flexible (Onshape's default), locked at the
    /// current position, or following one of the subassembly's named positions.
    #[serde(default, skip_serializing_if = "SubassemblyLock::is_flexible")]
    pub lock: SubassemblyLock,
    /// Flexible subassemblies: positions of the subassembly's instances in
    /// this assembly (relative to the subassembly instance), overriding the
    /// subassembly's own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nested: Vec<(InstanceId, Placement)>,
    /// "Use best available tessellation": the instance's parts are drawn
    /// with the finest tessellation.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub best_tessellation: bool,
    /// Instance properties… › Reference designator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_designator: Option<String>,
    /// Dynamic suppression › "Unsuppressed if" expression: the instance is
    /// suppressed while it evaluates to false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsuppressed_if: Option<String>,
}

impl Instance {
    pub fn new(id: InstanceId, name: String, source: InstanceSource, placement: Placement) -> Self {
        Instance { id, name, source, placement, fixed: false, hidden: false, suppressed: false, lock: SubassemblyLock::Flexible, nested: Vec::new(), best_tessellation: false, reference_designator: None, unsuppressed_if: None }
    }
}

/// How a subassembly instance solves in its parent (Onshape "Lock/follow position to").
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum SubassemblyLock {
    /// Its mates are solved with the parent's: its parts can be dragged.
    #[default]
    Flexible,
    /// Locked at the current position (one rigid body).
    Current,
    /// Locked at one of its named positions.
    Named(String),
}

impl SubassemblyLock {
    pub fn is_flexible(&self) -> bool {
        matches!(self, SubassemblyLock::Flexible)
    }
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
    /// Assembly-level mate connector (see [`AsmMateConnector`]). When set,
    /// `reference`/`path` are ignored and `instance` mirrors the connector's
    /// owner ([`InstanceId::ORIGIN`] for connectors on the assembly origin).
    #[serde(default)]
    pub connector: Option<MateId>,
    /// Entity whose direction gives the connector's primary axis at its
    /// origin (Onshape's inference points on a hovered face or edge: a
    /// vertex or edge midpoint of a planar face points along the face
    /// normal, an edge end along the edge).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<Box<Reference>>,
    /// Move offset of an implicit connector edited from the mate dialog
    /// (live assembly/578–580: it stays implicit, owned by the mate).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moved: Option<Box<ConnectorMove>>,
}

/// Translation and rotation of an implicit mate connector (as an assembly
/// mate connector's Move fields).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ConnectorMove {
    pub translation: [Option<ExprValue>; 3],
    /// Rotation axis: 0 X, 1 Y, 2 Z.
    pub rotation_axis: u8,
    pub rotation: Option<ExprValue>,
    /// Move unchecked: the values are kept but not applied (Onshape keeps
    /// the fields of an unchecked Move).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub off: bool,
}

impl MateConnectorRef {
    /// A linear pattern's direction picked on geometry (live "qa5 assembly"
    /// 6: "Edge of Part 1 <1>"): on an edge the copies go along the edge,
    /// so the connector's Z follows the edge's tangent; faces keep their
    /// normal / axis.
    pub fn linear_direction(mut self) -> Self {
        if self.connector.is_none() && self.align.is_none() {
            let entity = match &self.reference {
                Reference::Implicit { on, .. } => (**on).clone(),
                other => other.clone(),
            };
            if matches!(entity, Reference::Edge(_)) {
                self.align = Some(Box::new(entity));
            }
        }
        self
    }

    /// Connector at the assembly origin (world frame).
    pub fn origin() -> Self {
        Self { instance: InstanceId::ORIGIN, path: vec![], reference: Reference::Origin, flip: false, rotate_quarters: 0, connector: None, align: None, moved: None }
    }
    /// Reference to an assembly mate connector owned by `owner`.
    pub fn assembly_connector(id: MateId, owner: InstanceId) -> Self {
        Self { instance: owner, path: vec![], reference: Reference::Origin, flip: false, rotate_quarters: 0, connector: Some(id), align: None, moved: None }
    }
    /// Entity reference on an occurrence (implicit inference or bare entity).
    pub fn on(instance: InstanceId, path: Vec<InstanceId>, reference: Reference) -> Self {
        Self { instance, path, reference, flip: false, rotate_quarters: 0, connector: None, align: None, moved: None }
    }
    pub fn is_origin(&self) -> bool {
        self.instance == InstanceId::ORIGIN && self.connector.is_none()
    }
}

/// Mate connector feature created in the assembly. It is owned by one
/// instance occurrence (or the assembly origin) and moves rigidly with it.
/// Frame: the connector inferred on `on`, then flip / reorient, then the
/// local move (translation in the connector frame, rotation about one of its axes).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AsmMateConnector {
    pub id: MateId,
    pub name: String,
    /// Origin entity (`MateConnectorRef::origin()` for the assembly origin).
    pub on: MateConnectorRef,
    #[serde(default)]
    pub flip_primary: bool,
    #[serde(default)]
    pub reorient: u8,
    #[serde(default)]
    pub translation: [Option<ExprValue>; 3],
    /// Rotation axis (0 = X, 1 = Y, 2 = Z) and angle.
    #[serde(default = "default_rot_axis")]
    pub rotation_axis: u8,
    #[serde(default)]
    pub rotation: Option<ExprValue>,
    /// Move unchecked: the translation / rotation are kept but not applied.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub move_off: bool,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub suppressed: bool,
    /// Origin type Between entities: the origin is halfway between `on`
    /// and this entity (same instance).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub between: Option<MateConnectorRef>,
    /// Realign: the Z axis along this entity (face normal, edge direction)
    /// and the X axis toward the secondary entity's direction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_axis: Option<MateConnectorRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary_axis: Option<MateConnectorRef>,
}

fn default_rot_axis() -> u8 {
    2
}

impl AsmMateConnector {
    pub fn new(id: MateId, name: String, on: MateConnectorRef) -> Self {
        Self { id, name, on, flip_primary: false, reorient: 0, translation: [None, None, None], rotation_axis: 2, rotation: None, move_off: false, hidden: false, suppressed: false, between: None, primary_axis: None, secondary_axis: None }
    }
    pub fn owner(&self) -> InstanceId {
        self.on.instance
    }
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
    /// Axis of the rotation offset (0 = X, 1 = Y, 2 = Z).
    #[serde(default = "default_rot_axis")]
    pub rotation_axis: u8,
    #[serde(default)]
    pub flip_primary: bool,
    #[serde(default)]
    pub reorient_secondary: u8,
    #[serde(default)]
    pub limits: Vec<(String, Limits)>,
    #[serde(default)]
    pub suppressed: bool,
    /// Additional connectors. Width: `[width 2]` or `[width 2, tab 2]` with
    /// `a` = tab 1 and `b` = width 1. Unused by the other mate types.
    #[serde(default)]
    pub extra: Vec<MateConnectorRef>,
    /// Tangent mates: the contact may move onto faces tangent-connected to
    /// the selected ones (Onshape "Tangent propagation", on by default).
    #[serde(default = "default_true")]
    pub tangent_propagation: bool,
    /// Mate indicator hidden in the graphics area (eye icon / Hide).
    #[serde(default)]
    pub hidden: bool,
    /// Dynamic suppression: suppressed while this expression is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsuppressed_if: Option<String>,
}

fn default_true() -> bool {
    true
}

impl Mate {
    pub fn new(id: MateId, name: String, mate_type: MateType, a: MateConnectorRef, b: MateConnectorRef) -> Self {
        Mate {
            id,
            name,
            mate_type,
            a,
            b,
            offset_z: None,
            offset_x: None,
            offset_y: None,
            rotation: None,
            // Live (qa5 MT6, live 585): a Fastened mate's offset rotates
            // about X by default.
            rotation_axis: if mate_type == MateType::Fastened { 0 } else { 2 },
            flip_primary: false,
            reorient_secondary: 0,
            limits: vec![],
            suppressed: false,
            extra: vec![],
            tangent_propagation: true,
            hidden: false,
            unsuppressed_if: None,
        }
    }
    /// All connectors of the mate.
    pub fn connectors(&self) -> impl Iterator<Item = &MateConnectorRef> {
        [&self.a, &self.b].into_iter().chain(self.extra.iter())
    }
    /// Top-level instances the mate involves (without the assembly origin).
    pub fn instances(&self) -> Vec<InstanceId> {
        let mut v: Vec<InstanceId> = Vec::new();
        for c in self.connectors() {
            if c.instance != InstanceId::ORIGIN && !v.contains(&c.instance) {
                v.push(c.instance);
            }
        }
        v
    }
    pub fn uses_instance(&self, id: InstanceId) -> bool {
        self.connectors().any(|c| c.instance == id)
    }
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
    /// Free values of the two mates when the relation was created or last
    /// edited (the reference the coupling is measured from). `None` (old
    /// documents) measures from the positions at each solve.
    #[serde(default)]
    pub base: Option<[f64; 2]>,
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
    /// Values of the driving mates (revolute / slider / cylindrical... free
    /// values: rotation about Z, translations X/Y/Z) when the position was
    /// saved; restoring the position re-imposes them.
    #[serde(default)]
    pub mate_values: Vec<(MateId, [f64; 4])>,
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
    /// Rotation pivot (world, before the step). Default: the bounds center
    /// of the step's instances.
    #[serde(default)]
    pub pivot: Option<[f64; 3]>,
    /// Renamed (None: "Explode step N" by position; live assembly/805).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// ☑ Explode lines (the dashed trails; on by default).
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub lines: bool,
    /// "Edges, vertices, or faces" the explode lines start from (empty: the
    /// instances' centers).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub line_refs: Vec<MateConnectorRef>,
}

fn is_true(b: &bool) -> bool {
    *b
}

impl ExplodeStep {
    /// The step's name in the Exploded views tree (index from 0).
    pub fn display_name(&self, index: usize) -> String {
        self.name.clone().unwrap_or_else(|| format!("Explode step {}", index + 1))
    }
}

impl ExplodedView {
    /// Move step `from` to index `to` (steps in between shift by one).
    pub fn move_step(&mut self, from: usize, to: usize) -> bool {
        if from >= self.steps.len() || to >= self.steps.len() || from == to {
            return false;
        }
        let s = self.steps.remove(from);
        self.steps.insert(to, s);
        true
    }
}

impl ExplodeStep {
    pub fn translate(instances: Vec<InstanceId>, t: [f64; 3]) -> Self {
        Self { instances, translation: t, rotation_axis: None, rotation_angle: 0.0, pivot: None, name: None, lines: true, line_refs: Vec::new() }
    }
    pub fn rotate(instances: Vec<InstanceId>, axis: [f64; 3], angle: f64, pivot: [f64; 3]) -> Self {
        Self { instances, translation: [0.0; 3], rotation_axis: Some(axis), rotation_angle: angle, pivot: Some(pivot), name: None, lines: true, line_refs: Vec::new() }
    }
}

/// Instance pattern kinds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PatternKind {
    /// Copies along the Z axis of a mate connector (edge direction, face
    /// normal...), optionally in a second direction.
    /// `centered`: the seed in the middle, `count` instances (with the
    /// seed) each way (help: Assembly linear pattern 7).
    Linear {
        direction: MateConnectorRef,
        distance: ExprValue,
        count: u32,
        opposite: bool,
        equal_spacing: bool,
        second: Option<LinearDirection>,
        #[serde(default)]
        centered: bool,
    },
    /// Copies about the Z axis of a mate connector (cylinder axis, circular edge...).
    Circular {
        axis: MateConnectorRef,
        angle: ExprValue,
        count: u32,
        opposite: bool,
        equal_spacing: bool,
        #[serde(default)]
        centered: bool,
    },
    /// Mirror across the XY plane of a mate connector. By default copies are
    /// placed with a rigid transform (symmetric parts); with `handed`, part
    /// copies use a mirrored version of the part ([`InstanceSource::MirroredPart`]).
    Mirror {
        plane: MateConnectorRef,
        #[serde(default)]
        handed: bool,
    },
    /// Follow a Part Studio pattern: the seed's mate is re-created on every
    /// patterned copy of its target geometry.
    PartPattern { mate: MateId },
    /// Replicate: the seed's mate is re-created on all geometry matching its
    /// target (e.g. every hole of the same size) on the target instance
    /// (`all_instances`: also on other instances of the same part).
    Replicate {
        mate: MateId,
        /// Older documents (no `scope`): also match on other instances of
        /// the target part.
        #[serde(default)]
        all_instances: bool,
        /// Match scope (help: Replicate 6); None for older documents (the
        /// target instance's geometry, by signature).
        #[serde(default)]
        scope: Option<ReplicateScope>,
        /// "Parts to find match in": the parts, faces or edges the scope
        /// searches (help: Replicate 7).
        #[serde(default)]
        scope_refs: Vec<ScopeEntity>,
    },
}

/// Replicate's match scope (help: Replicate 6): face options when the seed
/// mate is on a face, edge options when it is on an edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReplicateScope {
    /// Matching faces of the selected parts.
    FacesInParts,
    /// Matching faces made by the features of the selected faces.
    FacesInFeatures,
    /// The selected faces that match.
    IndividualFaces,
    /// Matching edges lying on the selected plane (planar face).
    EdgesOnPlane,
    /// Matching edges bounding the selected face.
    EdgesOnFace,
    /// The selected edges that match.
    IndividualEdges,
}

impl ReplicateScope {
    /// Live's labels, in the select's order.
    pub const FACE: [ReplicateScope; 3] = [ReplicateScope::FacesInParts, ReplicateScope::FacesInFeatures, ReplicateScope::IndividualFaces];
    pub const EDGE: [ReplicateScope; 3] = [ReplicateScope::EdgesOnPlane, ReplicateScope::EdgesOnFace, ReplicateScope::IndividualEdges];

    pub fn label(self) -> &'static str {
        match self {
            ReplicateScope::FacesInParts => "Match faces in parts",
            ReplicateScope::FacesInFeatures => "Match faces in features",
            ReplicateScope::IndividualFaces => "Match individual faces",
            ReplicateScope::EdgesOnPlane => "Match edges on plane",
            ReplicateScope::EdgesOnFace => "Match edges on face",
            ReplicateScope::IndividualEdges => "Match individual edges",
        }
    }

    /// Whether the scope's selections are whole parts (else faces / edges).
    pub fn takes_parts(self) -> bool {
        self == ReplicateScope::FacesInParts
    }
}

/// A part (no entity) or one of its faces / edges, picked in an assembly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScopeEntity {
    pub instance: InstanceId,
    #[serde(default)]
    pub path: Vec<InstanceId>,
    #[serde(default)]
    pub entity: Option<Reference>,
}

/// Bit of a pattern copy's index for the copies on the far side of a
/// centered pattern (the others go along the direction, these against it).
pub const CENTERED_BACK: u32 = 0x8000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LinearDirection {
    pub direction: MateConnectorRef,
    pub distance: ExprValue,
    pub count: u32,
    pub opposite: bool,
}

/// One generated instance of a pattern.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatternInstance {
    pub seed: InstanceId,
    /// Stable index (linear: i + j * 65536; circular: i; matched patterns: target code).
    pub index: u32,
    pub instance: InstanceId,
    /// Generated mate (part pattern / replicate).
    #[serde(default)]
    pub mate: Option<MateId>,
    /// Target connector of the generated mate.
    #[serde(default)]
    pub target: Option<MateConnectorRef>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstancePattern {
    pub id: MateId,
    pub name: String,
    pub kind: PatternKind,
    pub seeds: Vec<InstanceId>,
    #[serde(default)]
    pub generated: Vec<PatternInstance>,
    #[serde(default)]
    pub suppressed: bool,
}

impl InstancePattern {
    pub fn type_name(&self) -> &'static str {
        match self.kind {
            PatternKind::Linear { .. } => "Linear pattern",
            PatternKind::Circular { .. } => "Circular pattern",
            PatternKind::Mirror { .. } => "Mirror",
            PatternKind::PartPattern { .. } => "Part pattern",
            PatternKind::Replicate { .. } => "Replicate",
        }
    }
    /// Indices the pattern should generate per seed (linear/circular only).
    pub fn grid_indices(&self) -> Vec<u32> {
        match &self.kind {
            // Copy indices: i + j * 65536, CENTERED_BACK marking the copies on
            // the far side of a centered pattern.
            PatternKind::Linear { count, second, centered, .. } => {
                let n1 = (*count).max(1);
                let n2 = second.as_ref().map(|s| s.count.max(1)).unwrap_or(1);
                let mut v = Vec::new();
                for j in 0..n2 {
                    for i in 0..n1 {
                        if i != 0 || j != 0 {
                            v.push(i + j * 65536);
                        }
                        if *centered && i != 0 {
                            v.push(i | CENTERED_BACK | (j * 65536));
                        }
                    }
                }
                v
            }
            PatternKind::Circular { count, centered, .. } => {
                let n = (*count).max(1);
                let mut v: Vec<u32> = (1..n).collect();
                if *centered {
                    v.extend((1..n).map(|i| i | CENTERED_BACK));
                }
                v
            }
            PatternKind::Mirror { .. } => vec![0],
            _ => Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Assembly {
    pub instances: Vec<Instance>,
    /// Variable Studio tables in this Assembly's Variable table.
    #[serde(default)]
    pub variable_studios: crate::variables::VariableStudioRefs,
    pub mates: Vec<Mate>,
    #[serde(default)]
    pub relations: Vec<Relation>,
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(default)]
    pub named_positions: Vec<NamedPosition>,
    /// Named positions' driving mate degrees of freedom (the table's value
    /// columns): (mate, value index 0 = Rz, 1..3 = Tx/Ty/Tz).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub position_drivers: Vec<(MateId, u8)>,
    #[serde(default)]
    pub exploded_views: Vec<ExplodedView>,
    /// Assembly mate connector features.
    #[serde(default)]
    pub connectors: Vec<AsmMateConnector>,
    /// Linear / circular patterns, mirrors, part patterns and replicates.
    #[serde(default)]
    pub patterns: Vec<InstancePattern>,
    /// Folders of the instance list and of the Mate features list.
    #[serde(default)]
    pub folders: Vec<ListFolder>,
    /// Display states: named visibility of instances, mates and mate connectors.
    #[serde(default)]
    pub display_states: Vec<DisplayState>,
    /// Top row › Properties… (the assembly's metadata; boxed to keep
    /// `TabContent` small).
    #[serde(default, skip_serializing_if = "properties_empty")]
    pub properties: Box<AssemblyProperties>,
    /// BOM Table panel settings (type, columns, ⋯ options).
    #[serde(default, skip_serializing_if = "bom::BomSettings::is_default")]
    pub bom: bom::BomSettings,
    /// Configuration inputs and configured instances / mates.
    #[serde(default, skip_serializing_if = "config::AsmConfiguration::is_empty")]
    pub configuration: config::AsmConfiguration,
}

fn properties_empty(p: &AssemblyProperties) -> bool {
    p.is_empty()
}

/// Onshape's assembly Properties (besides the name, which is the tab name).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AssemblyProperties {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    #[serde(default, skip_serializing_if = "crate::partstudio::ItemMeta::is_empty")]
    pub meta: crate::partstudio::ItemMeta,
    /// Subassembly BOM behavior.
    #[serde(default, skip_serializing_if = "BomBehavior::is_default")]
    pub bom_behavior: BomBehavior,
    /// Mass ☑ Override of the assembly (kg; Mass and section properties
    /// with the top-level assembly, and its Properties…; live assembly/720–730).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mass_override: Option<f64>,
}

impl AssemblyProperties {
    pub fn is_empty(&self) -> bool {
        *self == AssemblyProperties::default()
    }
}

/// Properties › "Subassembly BOM behavior".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BomBehavior {
    #[default]
    AssemblyAndComponents,
    AssemblyOnly,
    ComponentsOnly,
}

impl BomBehavior {
    pub const ALL: [BomBehavior; 3] = [BomBehavior::AssemblyAndComponents, BomBehavior::AssemblyOnly, BomBehavior::ComponentsOnly];
    pub fn label(self) -> &'static str {
        match self {
            BomBehavior::AssemblyAndComponents => "Show Assembly and components",
            BomBehavior::AssemblyOnly => "Show Assembly only",
            BomBehavior::ComponentsOnly => "Show components only",
        }
    }
    pub fn is_default(&self) -> bool {
        *self == BomBehavior::AssemblyAndComponents
    }
}

/// A folder in the instance list (`instances`) or in the Mate features
/// list (`features`: mates, mate connectors, relations, groups). Deleting a
/// folder deletes its contents; Unpack keeps them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListFolder {
    pub id: MateId,
    pub name: String,
    #[serde(default)]
    pub instances: Vec<InstanceId>,
    #[serde(default)]
    pub features: Vec<MateId>,
    /// Dynamic suppression of an instance folder (live assembly/537): its
    /// instances are suppressed while this expression is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsuppressed_if: Option<String>,
}

impl ListFolder {
    /// Folder of the Mate features list (else of the instance list).
    pub fn is_mate_folder(&self) -> bool {
        !self.features.is_empty() && self.instances.is_empty()
    }
}

/// Display state: what is hidden (Onshape captures the visibility of parts,
/// subassemblies, mates and mate connectors).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DisplayState {
    pub name: String,
    #[serde(default)]
    pub hidden_instances: Vec<InstanceId>,
    #[serde(default)]
    pub hidden_features: Vec<MateId>,
    /// Instances and features that existed when the state was captured
    /// (later ones keep their defaults when the state is applied).
    #[serde(default)]
    pub known: Vec<u64>,
}
pub mod analysis;
pub mod bom;
pub mod config;
pub mod eval;
pub mod ops;
pub mod solver;
pub mod standard;

impl Assembly {
    pub fn instance(&self, id: InstanceId) -> Option<&Instance> {
        self.instances.iter().find(|i| i.id == id)
    }
    pub fn instance_mut(&mut self, id: InstanceId) -> Option<&mut Instance> {
        self.instances.iter_mut().find(|i| i.id == id)
    }
    pub fn mate(&self, id: MateId) -> Option<&Mate> {
        self.mates.iter().find(|m| m.id == id)
    }
    pub fn connector(&self, id: MateId) -> Option<&AsmMateConnector> {
        self.connectors.iter().find(|c| c.id == id)
    }
    pub fn pattern(&self, id: MateId) -> Option<&InstancePattern> {
        self.patterns.iter().find(|p| p.id == id)
    }
    /// Pattern that generated an instance.
    pub fn generated_by(&self, id: InstanceId) -> Option<&InstancePattern> {
        self.patterns.iter().find(|p| p.generated.iter().any(|g| g.instance == id))
    }
    /// Pattern that generated a mate (part pattern / replicate).
    pub fn mate_generated_by(&self, id: MateId) -> Option<&InstancePattern> {
        self.patterns.iter().find(|p| p.generated.iter().any(|g| g.mate == Some(id)))
    }
    /// Mates a pattern generated: listed under its row in "Mate features (N)"
    /// (live qa5 asm-056), not in the top-level Mate features.
    pub fn pattern_mates<'a>(&'a self, p: &'a InstancePattern) -> impl Iterator<Item = &'a Mate> + 'a {
        self.mates.iter().filter(move |m| p.generated.iter().any(|g| g.mate == Some(m.id)))
    }
    /// The top-level "Mate features (N)" count: connectors, mates (not those
    /// a pattern generated), relations and groups.
    pub fn listed_mate_count(&self) -> usize {
        self.connectors.len() + self.mates.iter().filter(|m| self.mate_generated_by(m.id).is_none()).count() + self.relations.len() + self.groups.len()
    }
    /// Fresh id for instances, mates, relations, groups, connectors and patterns
    /// (unique within the assembly; never 0, which is the origin).
    pub fn next_id(&self) -> u64 {
        let m = self
            .instances
            .iter()
            .map(|i| i.id.0)
            .chain(self.mates.iter().map(|m| m.id.0))
            .chain(self.relations.iter().map(|r| r.id.0))
            .chain(self.groups.iter().map(|g| g.id.0))
            .chain(self.connectors.iter().map(|c| c.id.0))
            .chain(self.patterns.iter().map(|p| p.id.0))
            .chain(self.folders.iter().map(|f| f.id.0))
            .max()
            .unwrap_or(0);
        m + 1
    }
    /// Onshape-style instance name: `Base <n>` with the next free n.
    pub fn next_instance_name(&self, base: &str) -> String {
        let mut n = 1;
        loop {
            let name = format!("{base} <{n}>");
            if !self.instances.iter().any(|i| i.name == name) {
                return name;
            }
            n += 1;
        }
    }
    /// Instance name without the ` <n>` suffix.
    pub fn base_name(name: &str) -> &str {
        match name.rfind(" <") {
            Some(i) if name.ends_with('>') => &name[..i],
            _ => name,
        }
    }
    /// Next default feature name ("Fastened 1", "Group 2"...), in the UI
    /// language ([`crate::default_names`]).
    pub fn next_feature_name(&self, base: &str) -> String {
        let base = crate::default_names::base(base);
        let mut n = 1;
        loop {
            let name = format!("{base} {n}");
            let taken = self.mates.iter().any(|m| m.name == name)
                || self.relations.iter().any(|r| r.name == name)
                || self.groups.iter().any(|g| g.name == name)
                || self.connectors.iter().any(|c| c.name == name)
                || self.patterns.iter().any(|p| p.name == name)
                || self.folders.iter().any(|f| f.name == name);
            if !taken {
                return name;
            }
            n += 1;
        }
    }
    fn uses_dead_connector(c: &MateConnectorRef, dead: &[MateId]) -> bool {
        c.connector.is_some_and(|id| dead.contains(&id))
    }
    /// Remove an instance with the mates, relations, connectors, pattern
    /// copies and group memberships that use it.
    pub fn remove_instance(&mut self, id: InstanceId) {
        if id == InstanceId::ORIGIN {
            return;
        }
        // Patterns seeded by the instance lose their copies of it.
        let mut dead_instances: Vec<InstanceId> = vec![id];
        for p in &mut self.patterns {
            if p.seeds.contains(&id) {
                p.seeds.retain(|s| *s != id);
                dead_instances.extend(p.generated.iter().filter(|g| g.seed == id).map(|g| g.instance));
                p.generated.retain(|g| g.seed != id);
            }
            p.generated.retain(|g| g.instance != id);
        }
        let dead_patterns: Vec<MateId> = self.patterns.iter().filter(|p| p.seeds.is_empty()).map(|p| p.id).collect();
        for pid in dead_patterns {
            self.remove_mate(pid);
        }
        for d in dead_instances {
            self.remove_instance_only(d);
        }
    }
    fn remove_instance_only(&mut self, id: InstanceId) {
        self.instances.retain(|i| i.id != id);
        let dead_conn: Vec<MateId> = self.connectors.iter().filter(|c| c.on.instance == id).map(|c| c.id).collect();
        self.connectors.retain(|c| !dead_conn.contains(&c.id));
        let dead: Vec<MateId> = self.mates.iter().filter(|m| m.uses_instance(id) || m.connectors().any(|c| Self::uses_dead_connector(c, &dead_conn))).map(|m| m.id).collect();
        self.mates.retain(|m| !dead.contains(&m.id));
        self.relations.retain(|r| !r.mates.iter().any(|m| dead.contains(m)));
        for p in &mut self.patterns {
            for g in &mut p.generated {
                if g.mate.is_some_and(|m| dead.contains(&m)) {
                    g.mate = None;
                }
            }
        }
        self.patterns.retain(|p| match &p.kind {
            PatternKind::Linear { direction, second, .. } => !Self::uses_dead_connector(direction, &dead_conn) && direction.instance != id && second.as_ref().is_none_or(|s| s.direction.instance != id),
            PatternKind::Circular { axis, .. } => !Self::uses_dead_connector(axis, &dead_conn) && axis.instance != id,
            PatternKind::Mirror { plane, .. } => !Self::uses_dead_connector(plane, &dead_conn) && plane.instance != id,
            PatternKind::PartPattern { mate } | PatternKind::Replicate { mate, .. } => !dead.contains(mate),
        });
        for f in &mut self.folders {
            f.instances.retain(|i| *i != id);
        }
        for g in &mut self.groups {
            g.instances.retain(|i| *i != id);
        }
        // Like Onshape, a group goes away with its last member.
        self.groups.retain(|g| !g.instances.is_empty());
        for np in &mut self.named_positions {
            np.placements.retain(|(i, _)| *i != id);
        }
        for ev in &mut self.exploded_views {
            for s in &mut ev.steps {
                s.instances.retain(|i| *i != id);
            }
        }
    }
    /// Remove a mate, relation, group, connector or pattern (patterns take
    /// their generated instances and mates with them).
    pub fn remove_mate(&mut self, id: MateId) {
        for f in &mut self.folders {
            f.features.retain(|x| *x != id);
        }
        if let Some(p) = self.patterns.iter().find(|p| p.id == id).cloned() {
            self.patterns.retain(|x| x.id != id);
            for g in &p.generated {
                if let Some(m) = g.mate {
                    self.remove_mate(m);
                }
                self.remove_instance(g.instance);
            }
            return;
        }
        if self.connectors.iter().any(|c| c.id == id) {
            self.connectors.retain(|c| c.id != id);
            let dead: Vec<MateId> = self.mates.iter().filter(|m| m.connectors().any(|c| c.connector == Some(id))).map(|m| m.id).collect();
            for m in dead {
                self.remove_mate(m);
            }
            let dead = [id];
            self.patterns.retain(|p| match &p.kind {
                PatternKind::Linear { direction, .. } => !Self::uses_dead_connector(direction, &dead),
                PatternKind::Circular { axis, .. } => !Self::uses_dead_connector(axis, &dead),
                PatternKind::Mirror { plane, .. } => !Self::uses_dead_connector(plane, &dead),
                _ => true,
            });
            return;
        }
        self.mates.retain(|m| m.id != id);
        self.relations.retain(|r| r.id != id && !r.mates.contains(&id));
        self.groups.retain(|g| g.id != id);
        // A pattern driven by this mate (part pattern / replicate) goes too.
        if let Some(pid) = self.patterns.iter().find(|p| matches!(p.kind, PatternKind::PartPattern { mate } | PatternKind::Replicate { mate, .. } if mate == id)).map(|p| p.id) {
            self.remove_mate(pid);
        }
        for p in &mut self.patterns {
            for g in &mut p.generated {
                if g.mate == Some(id) {
                    g.mate = None;
                }
            }
        }
    }
    /// Dissolve a pattern: its generated instances and mates become regular ones.
    pub fn dissolve_pattern(&mut self, id: MateId) {
        self.patterns.retain(|p| p.id != id);
    }

    /// Bring a linear / circular / mirror pattern's generated instances in
    /// line with its definition (count changes, new seeds). Generated
    /// instances start at the seed's placement; the solver moves them.
    /// Returns the ids of new instances.
    pub fn sync_pattern(&mut self, id: MateId) -> Vec<InstanceId> {
        let Some(p) = self.pattern(id).cloned() else { return vec![] };
        let wanted: Vec<(InstanceId, u32)> = p.seeds.iter().flat_map(|s| p.grid_indices().into_iter().map(move |i| (*s, i))).collect();
        let created = self.apply_generated(id, wanted.into_iter().map(|(s, i)| (s, i, None, None)).collect());
        // Mirror copies of handed parts use the mirrored part (and back).
        if let PatternKind::Mirror { handed, .. } = p.kind
            && let Some(p) = self.pattern(id).cloned()
        {
            for g in &p.generated {
                let Some(seed) = self.instance(g.seed).map(|i| i.source.clone()) else { continue };
                let want = if handed { seed.mirrored().unwrap_or(seed) } else { seed };
                if let Some(inst) = self.instance_mut(g.instance) {
                    inst.source = want;
                }
            }
        }
        created
    }

    /// Set a matched pattern's (part pattern / replicate) targets: one
    /// generated instance per target, mated to it like the seed is mated to
    /// its own target. `targets` are (code, target connector, initial placement).
    pub fn sync_matched_pattern(&mut self, id: MateId, targets: Vec<(u32, MateConnectorRef, Option<Placement>)>) -> Vec<InstanceId> {
        let Some(p) = self.pattern(id).cloned() else { return vec![] };
        let seed = p.seeds.first().copied();
        let Some(seed) = seed else { return vec![] };
        self.apply_generated(id, targets.into_iter().map(|(code, t, pl)| (seed, code, Some(t), pl)).collect())
    }

    fn apply_generated(&mut self, id: MateId, wanted: Vec<(InstanceId, u32, Option<MateConnectorRef>, Option<Placement>)>) -> Vec<InstanceId> {
        let Some(p) = self.pattern(id).cloned() else { return vec![] };
        let seed_mate = match &p.kind {
            PatternKind::PartPattern { mate } | PatternKind::Replicate { mate, .. } => self.mate(*mate).cloned(),
            _ => None,
        };
        // Drop generated instances that are no longer wanted (or whose target changed).
        let keep = |g: &PatternInstance| wanted.iter().any(|(s, i, t, _)| *s == g.seed && *i == g.index && (t.is_none() || g.target.as_ref() == t.as_ref()));
        let stale: Vec<PatternInstance> = p.generated.iter().filter(|g| !keep(g)).cloned().collect();
        for g in &stale {
            if let Some(m) = g.mate {
                self.mates.retain(|x| x.id != m);
                self.relations.retain(|r| !r.mates.contains(&m));
            }
            if let Some(pp) = self.patterns.iter_mut().find(|x| x.id == id) {
                pp.generated.retain(|x| x.instance != g.instance);
            }
            self.remove_instance(g.instance);
        }
        let mut created = Vec::new();
        for (seed, index, target, placement) in wanted {
            let exists = self.pattern(id).is_some_and(|pp| pp.generated.iter().any(|g| g.seed == seed && g.index == index));
            if exists {
                continue;
            }
            let Some(src) = self.instance(seed).cloned() else { continue };
            let nid = InstanceId(self.next_id());
            let name = self.next_instance_name(Self::base_name(&src.name));
            self.instances.push(Instance { id: nid, name, source: src.source.clone(), placement: placement.unwrap_or(src.placement), fixed: false, hidden: src.hidden, suppressed: false, lock: SubassemblyLock::Flexible, nested: Vec::new(), best_tessellation: src.best_tessellation, reference_designator: None, unsuppressed_if: src.unsuppressed_if.clone() });
            let mut mate_id = None;
            if let (Some(sm), Some(t)) = (&seed_mate, &target) {
                let mid = MateId(self.next_id());
                let mut m = sm.clone();
                m.id = mid;
                m.name = self.next_feature_name(sm.mate_type.name());
                // The seed side moves to the copy; the target side to the matched geometry.
                for c in std::iter::once(&mut m.a).chain(std::iter::once(&mut m.b)).chain(m.extra.iter_mut()) {
                    if c.instance == seed {
                        c.instance = nid;
                    } else {
                        let (flip, rq) = (c.flip, c.rotate_quarters);
                        *c = t.clone();
                        c.flip = flip;
                        c.rotate_quarters = rq;
                    }
                }
                self.mates.push(m);
                mate_id = Some(mid);
            }
            if let Some(pp) = self.patterns.iter_mut().find(|x| x.id == id) {
                pp.generated.push(PatternInstance { seed, index, instance: nid, mate: mate_id, target });
            }
            created.push(nid);
        }
        created
    }

    /// Replace the source of instances (keeping their mates: references keep
    /// resolving wherever the replacement has geometry with the same names).
    /// With `all`, every instance with the same source is replaced.
    pub fn replace_instance(&mut self, id: InstanceId, source: InstanceSource, base_name: &str, all: bool) -> Vec<InstanceId> {
        let Some(old) = self.instance(id).map(|i| i.source.clone()) else { return vec![] };
        let ids: Vec<InstanceId> = if all { self.instances.iter().filter(|i| i.source == old).map(|i| i.id).collect() } else { vec![id] };
        for i in &ids {
            // Live 843: an instance whose name is already the replacement's
            // ("Part 1 <1>" replaced by another "Part 1") keeps it.
            let keep = self.instance(*i).is_some_and(|inst| inst.name.rsplit_once(" <").map_or(inst.name.as_str(), |(b, _)| b) == base_name);
            let name = if keep { None } else { Some(self.next_instance_name(base_name)) };
            if let Some(inst) = self.instance_mut(*i) {
                inst.source = source.clone();
                if let Some(name) = name {
                    inst.name = name;
                }
            }
        }
        ids
    }

    /// Re-point every instance using external reference `from` at `to`
    /// (update to another version of the other document). Returns the count.
    pub fn update_external(&mut self, from: &str, to: &str) -> usize {
        let mut n = 0;
        for i in &mut self.instances {
            if let InstanceSource::External { key, .. } = &mut i.source
                && key == from
            {
                *key = to.to_string();
                n += 1;
            }
        }
        n
    }

    /// Change the configuration of a Part / Part Studio instance.
    pub fn set_instance_config(&mut self, id: InstanceId, cfg: ConfigValues) -> bool {
        match self.instance_mut(id).map(|i| &mut i.source) {
            Some(InstanceSource::Part { config, .. }) | Some(InstanceSource::PartStudio { config, .. }) | Some(InstanceSource::MirroredPart { config, .. }) | Some(InstanceSource::External { config, .. }) | Some(InstanceSource::Assembly { config, .. }) => {
                *config = cfg;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests;
