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

/// The bit that marks a part's flat pattern as a body of its own.
const FLAT_BIT: u64 = 1 << 60;

/// A sheet metal part's flat pattern as a body (live: "Flat pattern of Part
/// 1" is a part of its own; an assembly can insert it, request 1605): the
/// part's id with the flat bit.
pub fn flat_body_id(part: &crate::reference::BodyId) -> crate::reference::BodyId {
    crate::reference::BodyId { feature: FeatureId(part.feature.0 | FLAT_BIT), index: part.index }
}

/// The part whose flat pattern `id` is ([`flat_body_id`]).
pub fn part_of_flat(id: &crate::reference::BodyId) -> Option<crate::reference::BodyId> {
    (id.feature.0 & FLAT_BIT != 0).then_some(crate::reference::BodyId { feature: FeatureId(id.feature.0 & !FLAT_BIT), index: id.index })
}

/// Face `i` of `part`'s flat body ([`flat_body`]): what a sketch on the flat names as its plane (live 2468: "Face of
/// <model>" on the flat; its faces are no part's).
pub fn flat_face_name(part: &crate::reference::BodyId, i: usize) -> TopoName {
    let face = std::sync::Arc::new(TopoName::face(FeatureId(0), crate::reference::Role::New(i as u32)));
    TopoName::face(flat_body_id(part).feature, crate::reference::Role::Copy { instance: part.index, of: face })
}

/// The part and face index a [`flat_face_name`] names.
pub fn flat_face_of(name: &TopoName) -> Option<(crate::reference::BodyId, usize)> {
    let TopoName::Face { feature, role: crate::reference::Role::Copy { instance, of } } = name else { return None };
    let TopoName::Face { role: crate::reference::Role::New(i), .. } = &**of else { return None };
    let part = part_of_flat(&crate::reference::BodyId { feature: *feature, index: *instance })?;
    Some((part, *i as usize))
}

/// A sketch's frame on a flat face ([`flat_face_name`]) in its flat place: its origin the face's own centre (live's
/// evPlane of a flat face, c2-sm-louver's Sketch 9: not the world origin projected as on a part's face), its x as on
/// a part's face.
pub fn flat_face_frame(ctx: &crate::regen::context::Context, name: &TopoName) -> Option<crate::display::PlaneFrame> {
    let (part, i) = flat_face_of(name)?;
    let shape = flat_pattern_for(ctx, &part).ok()?.flat_view_solid()?;
    let face = shape.faces().get(i)?.clone();
    let pl = face.face_plane().ok()??;
    let n = pl.unit_normal();
    let x = ctx.planar_face_x(&pl);
    let [u0, u1, v0, v1] = face.uv_bounds().ok()?;
    let origin = face.point_at_uv((u0 + u1) * 0.5, (v0 + v1) * 0.5).ok()?;
    Some(crate::display::PlaneFrame { origin, x, y: n.cross(x), normal: n })
}

/// The part's face a flat face ([`flat_face_name`]) shows: the wall's face its centre comes from (a bend's
/// developed strip and an edge face have none).
pub fn flat_face_counterpart(ctx: &crate::regen::context::Context, name: &TopoName) -> Option<crate::reference::Reference> {
    use noshape_kernel as k;
    let (part, i) = flat_face_of(name)?;
    let flat = flat_pattern_for(ctx, &part).ok()?;
    let face = flat.flat_view_solid()?.faces().get(i)?.clone();
    let [u0, u1, v0, v1] = face.uv_bounds().ok()?;
    let c = face.point_at_uv((u0 + u1) * 0.5, (v0 + v1) * 0.5).ok()?;
    let reach = flat.thickness * 1.001 + 1e-9;
    let near = |s: &k::Shape, p: glam::DVec3, tol: f64| k::make_vertex(p).ok().and_then(|v| k::distance(&v, s).ok()).is_some_and(|d| d.distance <= tol);
    // Its centre on the flat's definition plane (z 0) goes back onto a wall's definition face; a face off z 0 is
    // the wall's face that far to the side the sheet is on.
    let on_def = glam::DVec3::new(c.x, c.y, 0.0);
    let (q, nf) = flat.place.as_ref()?.walls.iter().find_map(|(def, m)| {
        let back = m.inverse();
        let q = back.transform_point3(on_def);
        near(def, q, reach).then(|| (q, back.transform_vector3(glam::DVec3::Z).normalize()))
    })?;
    let body = ctx.body(&part)?;
    let faces = body.shape.faces();
    let candidates = if c.z.abs() < 1e-9 { vec![q] } else { vec![q + nf * c.z.abs(), q - nf * c.z.abs()] };
    let fi = candidates.iter().find_map(|p| faces.iter().position(|f| f.face_plane().ok().flatten().is_some() && near(f, *p, 1e-7)))?;
    Some(crate::reference::Reference::Face((*body.faces[fi]).clone()))
}

/// The flat pattern of the part [`flat_body_id`] `id` names, as a body of
/// `ctx` (the flat view's solid: the sheet in its flat place, its faces
/// named by [`flat_face_name`]), named "Flat pattern of <name>".
pub fn flat_body(ctx: &crate::regen::context::Context, id: &crate::reference::BodyId, name: &str) -> Option<crate::regen::context::Body> {
    let part = part_of_flat(id)?;
    let shape = flat_pattern_for(ctx, &part).ok()?.flat_view_solid()?;
    let faces = (0..shape.faces().len()).map(|i| std::sync::Arc::new(flat_face_name(&part, i))).collect();
    Some(crate::regen::context::Body::new(id.clone(), format!("Flat pattern of {name}"), crate::display::BodyKind::Solid, shape, faces, 0, FeatureId(0)))
}

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
    /// Its joint attribute's id: a bend a cut split is one joint (live: one
    /// table row, qa4 S1); `None` natively.
    pub joint: Option<std::sync::Arc<str>>,
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
    /// Its joint attribute's id (as [`Bend::joint`]).
    pub joint: Option<std::sync::Arc<str>>,
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

/// A sheet metal model's joints over all its parts: the Sheet metal table
/// is the model's, while each part keeps its own data (a rip can leave a
/// wall a part of its own). `data` holds the parts' walls, bends and rips
/// one part after another in part-list order, a rip between two of them
/// once; `bend_of[i]`: the part and its own index of merged bend `i`.
#[derive(Clone, Debug)]
pub struct ModelJoints {
    pub data: SheetMetalData,
    pub bend_of: Vec<(crate::reference::BodyId, usize)>,
    /// The model's first part.
    pub first: crate::reference::BodyId,
}

/// The model `model`'s joints over its parts (see [`ModelJoints`]), `None`
/// when no part of it exists. Parts go in the part list's order, so the
/// table doesn't depend on the parts' ids (random per feature).
pub fn model_joints(ctx: &crate::regen::context::Context, model: FeatureId) -> Option<ModelJoints> {
    let parts: Vec<(&crate::reference::BodyId, &SheetMetalData)> = ctx.bodies.iter().filter_map(|b| ctx.sheet_metal.get(&b.id).filter(|d| d.model == model).map(|d| (&b.id, &**d))).collect();
    let (first, d0) = *parts.first()?;
    let mut data = d0.clone();
    data.walls.clear();
    data.bends.clear();
    data.rips.clear();
    let mut bend_of = Vec::new();
    for (body, d) in parts {
        let off = data.walls.len();
        data.walls.extend(d.walls.iter().cloned());
        for (i, b) in d.bends.iter().enumerate() {
            let mut b = b.clone();
            b.parent += off;
            b.child = b.child.map(|c| c + off);
            data.bends.push(b);
            bend_of.push((body.clone(), i));
        }
        for r in &d.rips {
            // A rip between two parts is in both parts' data.
            let same = |x: &Rip| (r.joint.is_some() && x.joint == r.joint) || (r.birth != u64::MAX && x.birth == r.birth);
            if data.rips.iter().any(same) {
                continue;
            }
            let mut r = r.clone();
            r.a += off;
            if r.b != usize::MAX {
                r.b += off;
            }
            data.rips.push(r);
        }
    }
    Some(ModelJoints { data, bend_of, first: first.clone() })
}

/// The sheet metal models of a regeneration, each once, in feature order of
/// their first part (part-list order).
pub fn models_of(ctx: &crate::regen::context::Context) -> Vec<FeatureId> {
    let mut out = Vec::new();
    for b in &ctx.bodies {
        if let Some(d) = ctx.sheet_metal.get(&b.id)
            && !out.contains(&d.model)
        {
            out.push(d.model);
        }
    }
    out
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
