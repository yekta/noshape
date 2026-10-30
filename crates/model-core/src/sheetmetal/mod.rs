//! Sheet metal model data and flat pattern (03c SM-01, SM-18, SM-19, SM-22).
//!
//! noshape keeps the folded sheet metal part as an ordinary solid body and
//! records, per body, a lightweight "sheet metal model" in the regeneration
//! context ([`crate::regen::context::Context::sheet_metal`]): model
//! parameters (thickness, bend radius, K factor, relief defaults), the walls
//! (planar faces of one sheet side, by persistent name) and the bends joining
//! them (axis, angle, inner radius, K factor, the faces of the bend region).
//! Sheet metal features keep this data up to date; the flat pattern is
//! derived from it on demand ([`flat_pattern`]): starting from a base wall,
//! every bend is unrolled with its developed length `BA = angle * (R + K*T)`
//! and the actual wall faces of the body are transformed into the base
//! wall's plane.

use crate::naming::Name;
use crate::reference::{FeatureId, TopoName};
use glam::{DMat4, DVec3};

pub mod flat;
pub mod forms;
pub mod flat_dxf;
pub use flat::{FlatBendLine, FlatForm, FlatHead, FlatPattern, flat_pattern, flat_pattern_for};

/// Bend relief shape (model default or Bend relief override).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BendReliefStyle {
    Rectangle,
    Obround,
    Tear,
}

impl BendReliefStyle {
    pub fn from_id(id: &str) -> Self {
        match id {
            "RECTANGLE" | "SIZED_RECTANGLE" => BendReliefStyle::Rectangle,
            "TEAR" => BendReliefStyle::Tear,
            _ => BendReliefStyle::Obround,
        }
    }
}

/// What created a bend (bend table display).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BendKind {
    /// Regular bend between two walls.
    Bend,
    /// Bend of a hem.
    Hem,
    /// Rolled wall (sketch arc / rolled hem) - unrolled like a bend.
    Rolled,
}

/// A planar wall of the sheet metal model.
#[derive(Clone, Debug)]
pub struct Wall {
    pub feature: FeatureId,
    /// Faces of the wall on the model's reference sheet side (all walls use
    /// the same side, so the flat pattern is seen from that side).
    pub faces: Vec<Name>,
    /// Faces on the opposite sheet side.
    pub back: Vec<Name>,
}

impl Wall {
    /// A wall without faces (between two consecutive rolled sections).
    pub fn is_virtual(&self) -> bool {
        self.faces.is_empty()
    }
}

/// A bend joining a parent wall to an (optional) child wall.
///
/// Geometry convention: rotating `-dir_a` (the parent's direction continued
/// past the bend) by `+angle` around `axis` gives the child's direction away
/// from the bend. The parent's reference face is tangent to the bend region
/// on the line `origin + radial_a * rho + axis * s` (`rho` = `radius` when the
/// reference side is the bend's inner side, else `radius + thickness`).
#[derive(Clone, Debug)]
pub struct Bend {
    pub feature: FeatureId,
    pub kind: BendKind,
    pub parent: usize,
    pub child: Option<usize>,
    /// A point on the bend axis.
    pub origin: DVec3,
    /// Unit axis direction (see the struct doc for the orientation).
    pub axis: DVec3,
    /// Bend angle (deflection) in radians.
    pub angle: f64,
    /// Inner bend radius.
    pub radius: f64,
    pub k_factor: f64,
    /// Unit direction from the bend toward the parent wall (in its plane).
    pub dir_a: DVec3,
    /// Unit direction from the axis to the parent's tangent line.
    pub radial_a: DVec3,
    /// Extent of the bend along the axis, from `origin`.
    pub s0: f64,
    pub s1: f64,
    /// The reference sheet side is the inner (concave) side of this bend.
    pub ref_inner: bool,
    pub inner_faces: Vec<Name>,
    pub outer_faces: Vec<Name>,
    /// Unfolded in the model (Unfold feature): the bend region is a flat
    /// strip and the walls past it already lie in the flat position.
    pub flat: bool,
    /// When its definition entity appeared ([`crate::regen::context::Context::sm_births`]; 0 natively).
    pub birth: u64,
}

impl Bend {
    /// Developed length of the bend region (K factor method).
    pub fn allowance(&self, thickness: f64) -> f64 {
        self.angle * (self.radius + self.k_factor * thickness)
    }

    /// Radius of the reference sheet side in this bend.
    pub fn ref_radius(&self, thickness: f64) -> f64 {
        if self.ref_inner { self.radius } else { self.radius + thickness }
    }

    /// Rigid transform that unrolls the child side of the bend into the
    /// parent's plane (child geometry in folded coordinates -> parent's
    /// folded coordinates, extended past the parent's tangent line by the
    /// developed length).
    pub fn unfold_matrix(&self, thickness: f64) -> DMat4 {
        if self.flat {
            return DMat4::IDENTITY;
        }
        self.fold_inverse(thickness)
    }

    /// Unfolding transform of the bend's geometry (ignores [`Bend::flat`]).
    pub fn fold_inverse(&self, thickness: f64) -> DMat4 {
        let rot = DMat4::from_translation(self.origin) * DMat4::from_axis_angle(self.axis, -self.angle) * DMat4::from_translation(-self.origin);
        DMat4::from_translation(-self.dir_a * self.allowance(thickness)) * rot
    }

    /// Apply a rigid transform to the bend geometry.
    pub fn transform(&mut self, m: &DMat4) {
        self.origin = m.transform_point3(self.origin);
        self.axis = m.transform_vector3(self.axis).normalize_or_zero();
        self.dir_a = m.transform_vector3(self.dir_a).normalize_or_zero();
        self.radial_a = m.transform_vector3(self.radial_a).normalize_or_zero();
    }

    /// Point on the parent's reference tangent line at axis parameter `s`.
    pub fn tangent_point(&self, thickness: f64, s: f64) -> DVec3 {
        self.origin + self.radial_a * self.ref_radius(thickness) + self.axis * s
    }

    /// Whether the bend goes toward the viewer of the reference side.
    pub fn is_up(&self) -> bool {
        self.ref_inner
    }
}

/// A rip (open joint) between two walls.
#[derive(Clone, Debug)]
pub struct Rip {
    pub feature: FeatureId,
    pub a: usize,
    /// The other wall, or `usize::MAX` when the rip separated it into
    /// another part (`other` names it there).
    pub b: usize,
    /// EDGE / BUTT / BUTT2.
    pub style: String,
    pub gap: f64,
    /// Reference-side face of wall `b` when it is in another part.
    pub other: Option<Name>,
    /// Faces the rip cut (the walls' ends facing each other).
    pub faces: Vec<Name>,
    /// When its definition edge appeared (as [`Bend::birth`]).
    pub birth: u64,
}

impl Rip {
    /// Whether a face is one of the rip's cut faces.
    pub fn has_face(&self, n: &TopoName) -> bool {
        self.faces.iter().any(|f| n.descends_from(f))
    }
}

/// What a [`FlatMark`] stands for in the flat pattern.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlatMarkKind {
    /// A form tool footprint: the flat shows the region filled, with the
    /// outline and a center mark drawn on top (Onshape's form sketch).
    Form,
    /// A cutout (corner relief) removed from the flat.
    Cut,
}

/// Geometry attached to a wall for the flat pattern only (form outlines,
/// relief cutouts). Loops lie in the plane of the wall's reference face, in
/// folded coordinates (they follow the wall when later bends move it).
#[derive(Clone, Debug)]
pub struct FlatMark {
    pub feature: FeatureId,
    pub wall: usize,
    pub kind: FlatMarkKind,
    pub loops: Vec<Vec<DVec3>>,
    /// Center mark position (forms).
    pub origin: Option<DVec3>,
    /// Exact planar region filled in the flat (form footprint), if any.
    pub fill: Option<noshape_kernel::Shape>,
    /// A form's direction (its tools stand out along it), folded
    /// coordinates: the flat pattern tells up from down with it.
    pub direction: Option<DVec3>,
}

impl FlatMark {
    /// Center of the mark (the origin, else the first loop's average).
    pub fn center(&self) -> Option<DVec3> {
        self.origin.or_else(|| {
            let l = self.loops.first()?;
            (!l.is_empty()).then(|| l.iter().copied().sum::<DVec3>() / l.len() as f64)
        })
    }

    pub fn transform(&mut self, m: &DMat4) {
        for l in &mut self.loops {
            for p in l.iter_mut() {
                *p = m.transform_point3(*p);
            }
        }
        self.origin = self.origin.map(|p| m.transform_point3(p));
        self.fill = self.fill.as_ref().and_then(|f| f.transformed(&noshape_kernel::Transform::Matrix(*m)).ok());
    }
}

/// A form tool tagged in a Part Studio (Tag feature, purpose Form): the
/// part added to and the part removed from a sheet metal wall, placed by
/// aligning `origin` (z = outward normal of the sheet face, the sheet lies
/// below z = 0) with each form location.
#[derive(Clone, Debug)]
pub struct FormTool {
    pub feature: FeatureId,
    pub add: Option<crate::reference::BodyId>,
    pub remove: Option<crate::reference::BodyId>,
    /// Sketch drawn in the flat pattern (construction geometry), if any.
    pub sketch: Option<FeatureId>,
    /// Form origin: (origin, x axis, z axis).
    pub origin: Option<(DVec3, DVec3, DVec3)>,
}

/// Per-body sheet metal model.
#[derive(Clone, Debug)]
pub struct SheetMetalData {
    /// Feature that created the model.
    pub model: FeatureId,
    /// Active until a Finish sheet metal model feature.
    pub active: bool,
    pub thickness: f64,
    pub radius: f64,
    pub k_factor: f64,
    pub k_factor_rolled: f64,
    pub minimal_clearance: f64,
    /// K_FACTOR / BEND_ALLOWANCE / BEND_DEDUCTION (only the K factor method
    /// is evaluated; the others fall back to it).
    pub bend_calculation: String,
    pub bend_relief: BendReliefStyle,
    pub bend_relief_depth_scale: f64,
    pub bend_relief_width_scale: f64,
    /// Default corner relief style id (SIMPLE, CLOSED, ...).
    pub corner_relief: String,
    pub flip_direction_up: bool,
    /// Onshape's definition sheet is on the walls' back faces, so the flat is
    /// seen from them (Thicken: the side away from the thickened faces; live
    /// fs-smprobe-1 flattens a z 0..2 plate by (0, 0, -2 mm), its top up).
    pub view_back: bool,
    pub walls: Vec<Wall>,
    pub bends: Vec<Bend>,
    pub rips: Vec<Rip>,
    /// Form outlines and relief cutouts shown in the flat pattern.
    pub marks: Vec<FlatMark>,
}

impl SheetMetalData {
    pub fn new(model: FeatureId, thickness: f64, radius: f64, k_factor: f64) -> Self {
        Self {
            model,
            active: true,
            thickness,
            radius,
            k_factor,
            k_factor_rolled: 0.5,
            minimal_clearance: 2e-5,
            bend_calculation: "K_FACTOR".into(),
            bend_relief: BendReliefStyle::Obround,
            bend_relief_depth_scale: 2.0,
            bend_relief_width_scale: 1.0625,
            corner_relief: "SIMPLE".into(),
            flip_direction_up: false,
            view_back: false,
            walls: Vec::new(),
            bends: Vec::new(),
            rips: Vec::new(),
            marks: Vec::new(),
        }
    }

    /// Wall owning a face: (wall index, face is on the reference side).
    pub fn wall_of_face(&self, n: &TopoName) -> Option<(usize, bool)> {
        for (i, w) in self.walls.iter().enumerate() {
            if w.faces.iter().any(|f| n.descends_from(f)) {
                return Some((i, true));
            }
            if w.back.iter().any(|f| n.descends_from(f)) {
                return Some((i, false));
            }
        }
        None
    }

    /// Bend owning a face of a bend region.
    pub fn bend_of_face(&self, n: &TopoName) -> Option<usize> {
        self.bends.iter().position(|b| b.inner_faces.iter().chain(b.outer_faces.iter()).any(|f| n.descends_from(f)))
    }

    /// Indices of the bends attached to a wall.
    pub fn bends_of_wall(&self, w: usize) -> Vec<usize> {
        self.bends.iter().enumerate().filter(|(_, b)| b.parent == w || b.child == Some(w)).map(|(i, _)| i).collect()
    }

    /// Total developed length of all bends.
    pub fn total_allowance(&self) -> f64 {
        self.bends.iter().map(|b| b.allowance(self.thickness)).sum()
    }
}
