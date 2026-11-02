//! In-viewport manipulators for feature dialogs (Onshape's drag handles):
//! the Extrude depth arrow (dragging past zero flips direction) and the
//! Revolve angle arc (also Move face: offset/translate arrow, rotate arc).

use super::PartStudioView;
use crate::viewport::ViewportEvent;
use glam::{DVec2, DVec3};
use noshape_model::params::ParamValue;
use noshape_model::reference::Reference;
use noshape_render::{Drawable, Instance, LineData, LineSegment};
use noshape_units::{Dim, Quantity};
use gpui_kit::*;

/// A linear arrow manipulator bound to a length parameter.
#[derive(Clone, Copy, Debug)]
pub(super) struct Arrow {
    pub base: DVec3,
    pub dir: DVec3,
    pub length: f64,
    /// Length parameter the arrow drives.
    pub param: &'static str,
    /// Boolean flipped when the arrow is dragged past zero (None: the
    /// value stays positive).
    pub flip: Option<&'static str>,
    /// std's SECONDARY style (a second end's depth): a double chevron.
    pub secondary: bool,
}

/// The arrow glyph (pixels at the arrow, drawn in 3D like live's): shaft
/// length, head length and head width. Live (dialogs/extrude-03, 05; iso,
/// Z up, seen at 0.816): head 24.5 px tall on screen, 20 px wide, shaft 24 px.
pub(super) const ARROW_SHAFT_PX: f64 = 29.;
pub(super) const ARROW_HEAD_PX: f64 = 30.;
const ARROW_HEAD_W_PX: f64 = 20.;
/// An angle handle's stem past the arc's end and its head (live 11-01 Draft,
/// 4x crop: a 26 px stem, a 16 × 16 px head).
const ARC_STEM_PX: f64 = 26.;
const ARC_HEAD_PX: f64 = 16.;

/// An angular manipulator: an arc around an axis bound to an angle parameter.
#[derive(Clone, Copy, Debug)]
pub(super) struct ArcHandle {
    pub origin: DVec3,
    /// Unit axis; positive angles turn counter-clockwise about it.
    pub axis: DVec3,
    /// Vector from the axis to the profile anchor (perpendicular to `axis`).
    pub radial: DVec3,
    pub angle: f64,
    /// Angle parameter the arc drives.
    pub param: &'static str,
    /// Boolean flipped when the arc is dragged back past zero (std's
    /// revolveManipulatorChange: a negative angle is the opposite direction).
    pub flip: Option<&'static str>,
    /// std's SECONDARY style (Revolve's start angle): a double chevron.
    pub secondary: bool,
    /// The largest angle the drag sets (std's maxValue: a turn, Draft's 89.9°).
    pub max: f64,
}

impl ArcHandle {
    fn point(&self, a: f64) -> DVec3 {
        self.origin + glam::DQuat::from_axis_angle(self.axis, a) * self.radial
    }
}

// Built per frame and per event for the one edited feature, copied around by value: the handles' array
// outweighing the triad and section variants costs nothing worth a box.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, Debug)]
pub(super) enum Manip {
    /// A feature's handles in hit order (Extrude's depth and second depth,
    /// Revolve's angle and start angle or flip arrows, a partial Fillet's
    /// radius and bounds).
    Handles([Option<Handle>; MAX_HANDLES]),
    /// Transform ▸ Translate by XYZ (see `triad.rs`).
    Triad(super::triad::Triad),
    /// Section view: base circle, offset arrow, rotate rings (`section_manip.rs`).
    Section(super::section_manip::SectionManip),
}

/// The most handles a feature shows at once.
pub(super) const MAX_HANDLES: usize = 4;

impl Manip {
    /// A feature's handles (the first [`MAX_HANDLES`]); None without any.
    pub(super) fn of(list: impl IntoIterator<Item = Handle>) -> Option<Manip> {
        let mut out = [None; MAX_HANDLES];
        for (slot, h) in out.iter_mut().zip(list) {
            *slot = Some(h);
        }
        out[0].map(|_| Manip::Handles(out))
    }
}

/// One of a feature's handles: a length arrow (a flip arrow when it drives
/// no length) or an angle arc.
#[derive(Clone, Copy, Debug)]
pub(super) enum Handle {
    Arrow(Arrow),
    Arc(ArcHandle),
    /// A FeatureScript manipulator (`addManipulators`, its index in the
    /// feature state): a length or flip arrow, or an angle arc, whose moves
    /// go through the feature's "Manipulator Change Function".
    StdArrow(Arrow, usize),
    StdArc(ArcHandle, usize),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ManipDrag {
    /// Axis parameter at the grab point minus the current length (arcs: the
    /// dragged angle so far, unwrapped).
    pub grab_offset: f64,
    /// The dragged handle: 0 the first, 1 the second end's.
    pub which: usize,
    /// Arcs: the cursor's angle at the grab minus the handle's (its stem
    /// and head sit past the arc's end).
    pub delta: f64,
}

impl PartStudioView {
    /// The manipulator of the feature being edited, if it has one.
    pub(super) fn current_manip(&self, cx: &App) -> Option<Manip> {
        let Some(e) = self.edit.as_ref() else { return self.section_manip().map(Manip::Section) };
        // Live 10-27: a dialog over paused regeneration has no manipulator.
        if self.ui.regen_paused.is_some() {
            return None;
        }
        match e.dialog.read(cx).feature.type_id.as_str() {
            "extrude" => self.current_arrow(cx).and_then(|a| Manip::of([Handle::Arrow(a)].into_iter().chain(self.second_arrow(&a, cx).map(Handle::Arrow)))),
            "moveFace" if e.dialog.read(cx).feature.params.is("moveFaceType", "ROTATE") => self.revolve_arc(cx).and_then(|h| Manip::of([Handle::Arc(h)])),
            // Shell and Chamfer have none (std adds no manipulator; live psb 10-24).
            "cPlane" if e.dialog.read(cx).feature.params.is("cplaneType", "LINE_ANGLE") => self.plane_angle_arc(cx).and_then(|h| Manip::of([Handle::Arc(h)])),
            // Fillet: its radius, then a partial fillet's bounds (std's).
            "fillet" => Manip::of(self.param_arrow(cx).map(Handle::Arrow).into_iter().chain(self.std_handle_list(cx))),
            "cPlane" | "moveFace" | "offsetSurface" | "replaceFace" | "extendSurface" | "splitPart" => self.param_arrow(cx).and_then(|a| Manip::of([Handle::Arrow(a)])),
            "revolve" => self.revolve_handles(cx),
            "draft" if e.dialog.read(cx).feature.params.is("draftFeatureType", "PARTING_LINE") => self.std_handles(cx),
            "helix" => self.std_handles(cx),
            "draft" => self.draft_arc(cx).and_then(|h| Manip::of([Handle::Arc(h)])),
            "transform" => self.current_triad(cx).map(Manip::Triad).or_else(|| self.transform_handle(cx)),
            "routingCurve" => self.routing_triad(cx).map(Manip::Triad),
            // Features FeatureScript runs (std's sheet metal, custom features): their own addManipulators.
            _ => self.std_handles(cx),
        }
    }

    /// The angle arc of Move face's Rotate or a revolve (whatever its end type:
    /// [`Self::revolve_handles`] also places its flip arrows by it).
    fn revolve_arc(&self, cx: &App) -> Option<ArcHandle> {
        let d = self.edit.as_ref()?.dialog.read(cx);
        let p = &d.feature.params;
        let move_face = d.feature.type_id == "moveFace";
        let units = self.doc.read(cx).session.doc.settings.units;
        let angle = p.expr("angle").and_then(|x| x.cached.map(|q| q.value).or_else(|| noshape_units::evaluate_as(&x.text, Dim::ANGLE, &units, &self.display.variables).ok().map(|q| q.value)))?;
        // moveFace.fs: the rotate manipulator turns the first face about
        // the axis.
        let anchor = if move_face {
            let ctx = self.ui.before_context.as_ref().or(self.context.as_ref())?;
            p.query("moveFaces").iter().find_map(|r| noshape_model::select::face_anchor(ctx, r)).map(|(b, _)| b)?
        } else {
            p.query("entities").iter().find_map(|r| match r {
                Reference::SketchRegion { sketch, key } => {
                    let sk = self.display.sketches.iter().find(|s| s.feature == *sketch)?;
                    let reg = sk.regions.iter().find(|g| &g.key == key)?;
                    region_anchor(&reg.positions, &reg.indices)
                }
                _ => None,
            })?
        };
        let (origin, mut axis) = self.axis_of(p.query("axis").first()?)?;
        if p.bool("oppositeDirection") {
            axis = -axis;
        }
        let rel = anchor - origin;
        let radial = rel - axis * axis.dot(rel);
        if radial.length() < 1e-9 {
            return None;
        }
        let symmetric = p.bool("symmetric");
        Some(ArcHandle { origin: origin + axis * axis.dot(rel), axis, radial, angle: if symmetric { angle * 0.5 } else { angle }, param: "angle", flip: Some("oppositeDirection"), secondary: false, max: std::f64::consts::TAU })
    }

    /// Revolve's handles (std addRevolveManipulator, live psb 10-24 rev-*):
    /// a Blind end's angle arc, other ends a flip arrow at the profile across
    /// the turn; with Second end position the start's the same (a Blind
    /// start: an arc to the start angle about the same axis), double
    /// chevrons. None with Full revolve or Symmetric's start.
    fn revolve_handles(&self, cx: &App) -> Option<Manip> {
        let d = self.edit.as_ref()?.dialog.read(cx);
        let p = &d.feature.params;
        if p.bool("fullRevolve") {
            return None;
        }
        // The arc as for a Blind end: its anchor and axis serve the flips too.
        let units = self.doc.read(cx).session.doc.settings.units;
        let angle_of = |id: &str| p.expr(id).and_then(|x| x.cached.map(|q| q.value).or_else(|| noshape_units::evaluate_as(&x.text, Dim::ANGLE, &units, &self.display.variables).ok().map(|q| q.value)));
        let arc = self.revolve_arc(cx)?;
        // flipManipulator: at the profile, across the turn (the arc's start direction).
        let flip = |flip: &'static str, opposite: bool, secondary: bool| {
            let dir = arc.axis.cross(arc.radial).normalize_or_zero();
            Handle::Arrow(Arrow { base: arc.point(0.0), dir: if opposite { -dir } else { dir }, length: 0.0, param: "", flip: Some(flip), secondary })
        };
        let first = if p.is("endBound", "BLIND") { Handle::Arc(arc) } else { flip("oppositeDirection", false, false) };
        let second = if p.bool("symmetric") || !p.bool("hasStartBound") {
            None
        } else if p.is("startBound", "BLIND") {
            angle_of("angleBack").map(|angle| Handle::Arc(ArcHandle { angle, param: "angleBack", flip: None, secondary: true, ..arc }))
        } else {
            // std: the start's flip arrow is startOppositeDirection about the axis as picked (the arc's
            // axis is turned by oppositeDirection already).
            Some(flip("startOppositeDirection", p.bool("startOppositeDirection") != p.bool("oppositeDirection"), true))
        };
        Manip::of([first].into_iter().chain(second))
    }

    /// Draft's angle (std addNeutralPlaneDraftAngularManipulator, Neutral
    /// plane): about the line where the first face's tangent plane at its
    /// middle meets the neutral plane, from that face point, by the draft
    /// angle (Opposite direction: the other way), up to std's 89.9°.
    fn draft_arc(&self, cx: &App) -> Option<ArcHandle> {
        let d = self.edit.as_ref()?.dialog.read(cx);
        let p = &d.feature.params;
        if !p.is("draftFeatureType", "NEUTRAL_PLANE") {
            return None;
        }
        let ctx = self.ui.before_context.as_ref().or(self.context.as_ref())?;
        let r = p.query("neutralPlane").first()?;
        let (o, nn) = self.frame_of_ref(r).map(|f| (f.origin, f.normal)).or_else(|| noshape_model::select::face_anchor(ctx, r))?;
        let (pt, nf) = noshape_model::select::face_anchor(ctx, p.query("draftFaces").first()?)?;
        let origin = pt - nn * nn.dot(pt - o);
        let axis = nf.cross(nn).try_normalize()?;
        let radial = pt - origin;
        if radial.length() < 1e-9 {
            return None;
        }
        let units = self.doc.read(cx).session.doc.settings.units;
        let angle = p.expr("angle").and_then(|x| x.cached.map(|q| q.value).or_else(|| noshape_units::evaluate_as(&x.text, Dim::ANGLE, &units, &self.display.variables).ok().map(|q| q.value)))?;
        let axis = if p.bool("pullDirection") { -axis } else { axis };
        Some(ArcHandle { origin, axis, radial, angle, param: "angle", flip: Some("pullDirection"), secondary: false, max: 89.9f64.to_radians() })
    }

    /// Plane's Line angle (std cplane.fs addAngleManipulator): about the
    /// line, from a quarter of the plane's size out along the direction
    /// the angle is measured from (std takes that direction as a point and
    /// drops it onto the plane through the line's origin square to it; one
    /// line: across `perpendicular_vector`), by the angle (Opposite
    /// direction: the other way).
    fn plane_angle_arc(&self, cx: &App) -> Option<ArcHandle> {
        let d = self.edit.as_ref()?.dialog.read(cx);
        let p = &d.feature.params;
        let ctx = self.ui.before_context.as_ref().or(self.context.as_ref())?;
        let refs = p.query("entities");
        let (i, (o, axis)) = refs.iter().enumerate().find_map(|(i, r)| self.axis_of(r).map(|a| (i, a)))?;
        let in_plane = match refs.iter().enumerate().find(|(k, _)| *k != i).map(|(_, r)| r) {
            None => noshape_model::regen::features::cplane::perpendicular_vector(axis).cross(axis),
            Some(r) => match self.axis_of(r) {
                Some((o2, d2)) if d2.cross(axis).length() < 1e-9 => o2 - o,
                Some((_, d2)) => d2,
                None => match self.frame_of_ref(r).map(|f| f.normal).or_else(|| noshape_model::select::face_anchor(ctx, r).map(|(_, n)| n)) {
                    Some(n) => axis.cross(n),
                    None => noshape_model::regen::measure::resolve_shapes(ctx, r).first().and_then(|s| s.vertex_point().ok())? - o,
                },
            },
        };
        let rel = in_plane - o;
        let radial = (rel - axis * axis.dot(rel)).try_normalize()?;
        let size = self.display.planes.iter().find(|pl| pl.reference == Reference::FeaturePlane(d.feature.id)).map(|pl| pl.half_width * 2.0)?;
        let units = self.doc.read(cx).session.doc.settings.units;
        let angle = p.expr("angle").and_then(|x| x.cached.map(|q| q.value).or_else(|| noshape_units::evaluate_as(&x.text, Dim::ANGLE, &units, &self.display.variables).ok().map(|q| q.value)))?;
        let axis = if p.bool("oppositeDirection") { -axis } else { axis };
        Some(ArcHandle { origin: o, axis, radial: radial * size / 4.0, angle, param: "angle", flip: Some("oppositeDirection"), secondary: false, max: std::f64::consts::TAU })
    }

    /// Transform's Rotate angle and Translate by distance arrow (std
    /// transformCopy.fs): from the centre of the parts' box, about the
    /// axis / along the direction (two points, an edge's ends, a plane's
    /// normal), Opposite direction turning them.
    fn transform_handle(&self, cx: &App) -> Option<Manip> {
        let d = self.edit.as_ref()?.dialog.read(cx);
        let p = &d.feature.params;
        let (lo, hi) = self.parts_box("entities", cx)?;
        let c = (lo + hi) * 0.5;
        let units = self.doc.read(cx).session.doc.settings.units;
        let value = |id: &str, dim: Dim| p.expr(id).and_then(|x| x.cached.map(|q| q.value).or_else(|| noshape_units::evaluate_as(&x.text, dim, &units, &self.display.variables).ok().map(|q| q.value)));
        let opposite = p.bool("oppositeDirection");
        if p.is("transformType", "ROTATION") {
            let (o, axis) = self.axis_of(p.query("transformAxis").first()?)?;
            let origin = o + axis * axis.dot(c - o);
            let radial = c - origin;
            if radial.length() < 1e-9 {
                return None;
            }
            let axis = if opposite { -axis } else { axis };
            return Manip::of([Handle::Arc(ArcHandle { origin, axis, radial, angle: value("angle", Dim::ANGLE)?, param: "angle", flip: Some("oppositeDirection"), secondary: false, max: std::f64::consts::TAU })]);
        }
        if !p.is("transformType", "TRANSLATION_DISTANCE") {
            return None;
        }
        let ctx = self.ui.before_context.as_ref().or(self.context.as_ref())?;
        let refs = p.query("transformDirection");
        let shapes: Vec<noshape_kernel::Shape> = refs.iter().flat_map(|r| noshape_model::regen::measure::resolve_shapes(ctx, r)).collect();
        let points: Vec<DVec3> = shapes.iter().filter_map(|s| s.vertex_point().ok()).collect();
        let dir = match points.as_slice() {
            [a, b, ..] => *b - *a,
            _ => match shapes.iter().find_map(|s| s.endpoints().ok()) {
                Some((a, b)) => b - a,
                None => refs.iter().find_map(|r| self.frame_of_ref(r).map(|f| f.normal).or_else(|| noshape_model::select::face_anchor(ctx, r).map(|(_, n)| n)))?,
            },
        }
        .try_normalize()?;
        let dir = if opposite { -dir } else { dir };
        Manip::of([Handle::Arrow(Arrow { base: c, dir, length: value("distance", Dim::LENGTH)?, param: "distance", flip: Some("oppositeDirection"), secondary: false })])
    }

    /// World axis (point, unit direction) of a sketch line or straight/circular edge.
    fn axis_of(&self, r: &Reference) -> Option<(DVec3, DVec3)> {
        let f = |p: [f32; 3]| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
        match r {
            Reference::SketchEntity { sketch, entity } => {
                let c = self.display.sketches.iter().find(|s| s.feature == *sketch)?.curves.iter().find(|c| c.entity == *entity)?;
                let (a, b) = (f(*c.points.first()?), f(*c.points.last()?));
                ((b - a).length() > 1e-12).then(|| (a, (b - a).normalize()))
            }
            Reference::Edge(_) => {
                let ctx = self.context.as_ref()?;
                let e = noshape_model::regen::measure::resolve_shapes(ctx, r).into_iter().next()?;
                match e.edge_curve().ok()? {
                    noshape_kernel::Curve::Line { origin, direction } => Some((origin, direction.normalize())),
                    noshape_kernel::Curve::Circle { center, axis, .. } => Some((center, axis.normalize())),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// The extrude depth arrow.
    fn current_arrow(&self, cx: &App) -> Option<Arrow> {
        let e = self.edit.as_ref()?;
        let d = e.dialog.read(cx);
        if d.feature.type_id != "extrude" || d.feature.params.is("bodyType", "SURFACE") {
            return None;
        }
        // Live part-studio/2028: the other end types show the direction
        // arrow on the region (flip only, nothing to drag).
        let blind = d.feature.params.is("endBound", "BLIND");
        let depth = if !blind {
            0.0
        } else {
            d.feature.params.expr("depth").and_then(|x| x.cached.map(|q| q.value)).or_else(|| {
                let units = self.doc.read(cx).session.doc.settings.units;
                d.feature.params.expr("depth").and_then(|x| noshape_units::evaluate_as(&x.text, Dim::LENGTH, &units, &self.display.variables).ok()).map(|q| q.value)
            })?
        };
        // Profile center and normal from the first selected region, or a
        // picked planar face (it only exists before the feature).
        let ctx = self.ui.before_context.as_ref().or(self.context.as_ref());
        let (base, normal) = d.feature.params.query("entities").iter().find_map(|r| match r {
            Reference::SketchRegion { sketch, key } => {
                let sk = self.display.sketches.iter().find(|s| s.feature == *sketch)?;
                let reg = sk.regions.iter().find(|g| &g.key == key)?;
                Some((region_anchor(&reg.positions, &reg.indices)?, sk.frame.normal))
            }
            r => noshape_model::select::face_anchor(ctx?, r),
        })?;
        let mut dir = normal;
        if d.feature.params.bool("oppositeDirection") {
            dir = -dir;
        }
        let symmetric = d.feature.params.bool("symmetric");
        Some(Arrow { base, dir, length: if symmetric { depth * 0.5 } else { depth }, param: if blind { "depth" } else { "" }, flip: Some("oppositeDirection"), secondary: false })
    }

    /// Extrude's second end (std addExtrudeManipulator, live psb 10-24 ex2):
    /// with Second end position, not symmetric and Blind, a double chevron
    /// at the second cap driving Depth of the second end; other end types a
    /// double-chevron flip arrow at the profile.
    fn second_arrow(&self, first: &Arrow, cx: &App) -> Option<Arrow> {
        let d = self.edit.as_ref()?.dialog.read(cx);
        let p = &d.feature.params;
        if !p.bool("hasSecondDirection") || p.bool("symmetric") {
            return None;
        }
        // regen extrude: the second side runs along the first's direction, reversed by its own flip.
        let dir = if p.bool("secondDirectionOppositeDirection") { -first.dir } else { first.dir };
        // Other end types: std's SECONDARY flip arrow at the profile.
        if !p.is("secondDirectionBound", "BLIND") {
            return Some(Arrow { base: first.base, dir, length: 0.0, param: "", flip: Some("secondDirectionOppositeDirection"), secondary: true });
        }
        let units = self.doc.read(cx).session.doc.settings.units;
        let depth = p.expr("secondDirectionDepth").and_then(|x| x.cached.map(|q| q.value).or_else(|| noshape_units::evaluate_as(&x.text, Dim::LENGTH, &units, &self.display.variables).ok().map(|q| q.value)))?;
        Some(Arrow { base: first.base, dir, length: depth, param: "secondDirectionDepth", flip: Some("secondDirectionOppositeDirection"), secondary: true })
    }

    /// A blended Extrude preview's end cap (live 10-27, psui-prv-05a/b): the
    /// picked regions and faces moved to where the extrude ends, a flat
    /// grey (89,89,89) at 50 % in live's sRGB blend ((172,172,172) over
    /// white, (117,135,146) over the part's top; 0.46 here gives
    /// (172,172,172) and (116,137,150)), drawn over the mix like the other
    /// highlights, at either end of the slider.
    pub(super) fn extrude_end_cap(&self, cx: &App) -> Option<noshape_render::Instance> {
        self.built_base.as_ref()?;
        let arrow = self.current_arrow(cx).filter(|a| a.param == "depth" && a.length > 0.0)?;
        let d = self.edit.as_ref()?.dialog.read(cx);
        let shift = (arrow.dir * arrow.length).as_vec3();
        let mut mesh = noshape_render::MeshData::default();
        let mut push = |positions: &[[f32; 3]], indices: &[u32], normal: glam::Vec3| {
            let first = mesh.positions.len() as u32;
            mesh.positions.extend(positions.iter().map(|p| (glam::Vec3::from(*p) + shift).to_array()));
            mesh.normals.extend(std::iter::repeat_n(normal.to_array(), positions.len()));
            mesh.pick_ids.extend(std::iter::repeat_n(0, positions.len()));
            mesh.indices.extend(indices.iter().map(|i| first + i));
        };
        let before = self.ui.preview_before.as_ref().unwrap_or(&self.display);
        for r in d.feature.params.query("entities") {
            match r {
                Reference::SketchRegion { sketch, key } => {
                    let Some(sk) = self.display.sketches.iter().find(|s| s.feature == *sketch) else { continue };
                    let Some(reg) = sk.regions.iter().find(|g| &g.key == key) else { continue };
                    push(&reg.positions, &reg.indices, sk.frame.normal.as_vec3());
                }
                r => {
                    let Some((b, fi)) = before.bodies.iter().find_map(|b| b.face_refs.iter().position(|f| f == r).map(|fi| (b, fi))) else { continue };
                    let Some(&(first, count)) = b.mesh.face_ranges.get(fi) else { continue };
                    let Some(idx) = b.mesh.indices.get(first as usize..(first + count) as usize) else { continue };
                    // Local indices over the face's own vertices.
                    let mut map = std::collections::HashMap::new();
                    let mut pos: Vec<[f32; 3]> = Vec::new();
                    let local: Vec<u32> = idx.iter().map(|&i| *map.entry(i).or_insert_with(|| {
                        pos.push(b.mesh.positions[i as usize]);
                        pos.len() as u32 - 1
                    })).collect();
                    push(&pos, &local, arrow.dir.as_vec3());
                }
            }
        }
        if mesh.indices.is_empty() {
            return None;
        }
        let grey = crate::theme::lin(0x595959);
        Some(noshape_render::Instance::new(noshape_render::Drawable::Mesh(self.gpu.uploader().upload_mesh(&mesh))).with_color([grey[0], grey[1], grey[2], 0.46]).unlit().unpickable().ignore_clip().layer(crate::scene_builder::HIGHLIGHT_FILL_LAYER))
    }

    /// Drag handles of other features (Onshape): plane offset, fillet
    /// radius, Move face's distance.
    pub(super) fn param_arrow(&self, cx: &App) -> Option<Arrow> {
        let d = self.edit.as_ref()?.dialog.read(cx);
        let p = &d.feature.params;
        let units = self.doc.read(cx).session.doc.settings.units;
        let value = |id: &str| p.expr(id).and_then(|x| x.cached.map(|q| q.value).or_else(|| noshape_units::evaluate_as(&x.text, Dim::LENGTH, &units, &self.display.variables).ok().map(|q| q.value)));
        // Edges/faces the feature consumes only exist before it.
        let ctx = self.ui.before_context.as_ref().or(self.context.as_ref())?;
        match d.feature.type_id.as_str() {
            "cPlane" if p.is("cplaneType", "OFFSET") => {
                // A plane, or a circular / elliptical edge (offset from its plane).
                let f = p.query("entities").iter().find_map(|r| noshape_model::regen::features::cplane::circular_edge_frame(ctx, r).or_else(|| self.frame_of_ref(r)))?;
                let dir = if p.bool("oppositeDirection") { -f.normal } else { f.normal };
                Some(Arrow { base: f.origin, dir, length: value("offset")?, param: "offset", flip: Some("oppositeDirection"), secondary: false })
            }
            // moveFace.fs: Offset drives offsetDistance along the first
            // face's normal, Translate drives translationDistance along the
            // direction (Blind only); the base is the first face.
            "moveFace" if p.is("limitType", "BLIND") && !p.is("moveFaceType", "ROTATE") => {
                let (base, n) = p.query("moveFaces").iter().find_map(|r| noshape_model::select::face_anchor(ctx, r))?;
                let sign = if p.bool("oppositeDirection") { -1.0 } else { 1.0 };
                if p.is("moveFaceType", "OFFSET") {
                    return Some(Arrow { base, dir: n * sign, length: value("offsetDistance")?, param: "offsetDistance", flip: Some("oppositeDirection"), secondary: false });
                }
                let r = p.query("direction").first()?;
                let dir = self.axis_of(r).map(|(_, d)| d).or_else(|| self.frame_of_ref(r).map(|f| f.normal))?;
                Some(Arrow { base, dir: dir * sign, length: value("translationDistance")?, param: "translationDistance", flip: Some("oppositeDirection"), secondary: false })
            }
            // offsetSurface.fs: on the last face (a sketch region: its
            // plane), along its normal, the offset signed by the flip.
            "offsetSurface" => {
                let (base, n) = p.query("surfacesAndFaces").iter().rev().find_map(|r| match r {
                    Reference::SketchRegion { sketch, key } => {
                        let sk = self.display.sketches.iter().find(|s| s.feature == *sketch)?;
                        let reg = sk.regions.iter().find(|g| &g.key == key)?;
                        Some((region_anchor(&reg.positions, &reg.indices)?, sk.frame.normal))
                    }
                    r => noshape_model::select::face_anchor(ctx, r),
                })?;
                let dir = if p.bool("oppositeDirection") { -n } else { n };
                Some(Arrow { base, dir, length: value("offset")?, param: "offset", flip: Some("oppositeDirection"), secondary: false })
            }
            // replaceFace.fs: on the face to replace with, along its normal
            // (Flip alignment turns it twice there: not at all).
            "replaceFace" => {
                let (base, n) = p.query("templateFace").iter().find_map(|r| noshape_model::select::face_anchor(ctx, r).or_else(|| self.frame_of_ref(r).map(|f| (f.origin, f.normal))))?;
                let dir = if p.bool("oppositeDirection") { -n } else { n };
                Some(Arrow { base, dir, length: value("offset")?, param: "offset", flip: Some("oppositeDirection"), secondary: false })
            }
            // extend.fs (Move boundary, Blind): on the last edge, out of its face.
            "extendSurface" if p.is("endCondition", "BLIND") => {
                let (base, out) = p.query("entities").iter().rev().find_map(|r| noshape_model::select::edge_extend(ctx, r))?;
                let dir = if p.bool("oppositeDirection") { -out } else { out };
                Some(Arrow { base, dir, length: value("extendDistance")?, param: "extendDistance", flip: Some("oppositeDirection"), secondary: false })
            }
            // projectCurves.fs addDropCurveManipulator (Curve to face,
            // Direction): a flip arrow at the direction's plane (or axis)
            // origin along its normal.
            "projectCurves" if !p.is("curveProjectionType", "TWO_SKETCHES") && p.is("projectionType", "DIRECTION") => {
                let r = p.query("directionQuery").first()?;
                let (base, n) = self.frame_of_ref(r).map(|f| (f.origin, f.normal)).or_else(|| self.axis_of(r))?;
                Some(Arrow { base, dir: if p.bool("oppositeDirection") { -n } else { n }, length: 0.0, param: "", flip: Some("oppositeDirection"), secondary: false })
            }
            // frameTrim.fs addKeepSideManipulator (Face): a flip arrow on the
            // tool where it is nearest the parts to trim, along its normal
            // there; a mate connector's at its origin along its Z.
            "frameTrim" if p.is("trimType", "FACE") => {
                let r = p.query("tool").first()?;
                let flipped = p.bool("flipTrim");
                let (base, n) = if matches!(r, Reference::MateConnector { .. }) {
                    let f = self.frame_of_ref(r)?;
                    (f.origin, f.normal)
                } else {
                    let tool = match noshape_model::regen::measure::resolve_shapes(ctx, r).into_iter().next() {
                        Some(s) => s,
                        None => {
                            let f = self.frame_of_ref(r)?;
                            noshape_kernel::make_plane_face(&noshape_kernel::Plane::new(f.origin, f.normal, f.x), -1e3, 1e3, -1e3, 1e3).ok()?
                        }
                    };
                    let targets: Vec<noshape_kernel::Shape> = p.query("targets").iter().flat_map(|t| noshape_model::regen::measure::resolve_shapes(ctx, t)).collect();
                    let d = noshape_kernel::distance(&tool, &noshape_kernel::Shape::compound(&targets)).ok()?;
                    let face = tool.faces().into_iter().find(|f| noshape_kernel::make_vertex(d.point_a).ok().and_then(|v| noshape_kernel::distance(f, &v).ok()).is_some_and(|x| x.distance < 1e-7))?;
                    (d.point_a, face.normal_at_point(d.point_a).ok()?)
                };
                Some(Arrow { base, dir: if flipped { -n } else { n }, length: 0.0, param: "", flip: Some("flipTrim"), secondary: false })
            }
            // bodyDraft.fs createManipulators: a flip arrow along the pull
            // direction, at the first drafted entity's point nearest the
            // pull direction's entity (Draft on self), else on the parting
            // object (a face's middle by its parameters, a mate connector's
            // or plane's origin).
            "bodyDraft" => {
                let pull = p.query("pullDirection").first()?;
                let dir = self.axis_of(pull).map(|(_, d)| d).or_else(|| self.frame_of_ref(pull).map(|f| f.normal))?;
                let base = if p.bool("draftOnSelf") {
                    let key = if p.is("selectionType", "EDGES") { "topEdges" } else if p.is("selectionType", "FACES") { "faces" } else { "bodies" };
                    let first = noshape_model::regen::measure::resolve_shapes(ctx, p.query(key).first()?).into_iter().next()?;
                    let other = match noshape_model::regen::measure::resolve_shapes(ctx, pull).into_iter().next() {
                        Some(s) => s,
                        None => {
                            let f = self.frame_of_ref(pull)?;
                            noshape_kernel::make_plane_face(&noshape_kernel::Plane::new(f.origin, f.normal, f.x), -1e3, 1e3, -1e3, 1e3).ok()?
                        }
                    };
                    noshape_kernel::distance(&first, &other).ok()?.point_a
                } else {
                    let r = p.query("partingObject").first()?;
                    match r {
                        Reference::Face(_) => {
                            let f = noshape_model::regen::measure::resolve_shapes(ctx, r).into_iter().next()?;
                            let [u0, u1, v0, v1] = f.uv_bounds().ok()?;
                            f.point_at_uv((u0 + u1) * 0.5, (v0 + v1) * 0.5).ok()?
                        }
                        r => self.frame_of_ref(r)?.origin,
                    }
                };
                Some(Arrow { base, dir: if p.bool("flipPullDirection") { -dir } else { dir }, length: 0.0, param: "", flip: Some("flipPullDirection"), secondary: false })
            }
            // moveCurveBoundary.fs (Extend, Blind, with a help point): from
            // the first curve's end that grows, along its tangent.
            "trimCurve" => {
                let (base, dir) = noshape_model::interaction::move_curve_boundary_anchor(ctx, p, &units)?;
                Some(Arrow { base, dir, length: value("extensionDistance")?, param: "extensionDistance", flip: None, secondary: false })
            }
            // splitpart.fs addKeepSideManipulator (Part, not Keep both
            // sides): a flip arrow on the tool where it is nearest the parts
            // (here: the parts' box centre dropped onto the tool's plane),
            // along its normal for the front.
            "splitPart" if p.is("splitType", "PART") && !p.bool("keepBothSides") => {
                let r = p.query("tool").first()?;
                let (o, n) = self.frame_of_ref(r).map(|f| (f.origin, f.normal)).or_else(|| noshape_model::select::face_anchor(ctx, r))?;
                let (lo, hi) = self.parts_box("targets", cx)?;
                let c = (lo + hi) * 0.5;
                let base = c - n * n.dot(c - o);
                Some(Arrow { base, dir: if p.bool("keepFront") { n } else { -n }, length: 0.0, param: "", flip: Some("keepFront"), secondary: false })
            }
            "fillet" => {
                // Live (behaviours "Fillet", qa5 P1-3): on the last picked edge.
                let (base, dir) = p.query("entities").iter().rev().find_map(|r| noshape_model::select::edge_inward(ctx, r))?;
                Some(Arrow { base, dir, length: value("radius")?, param: "radius", flip: None, secondary: false })
            }
            _ => None,
        }
    }

    /// Publish the current arrow's handle to the viewport hit test.
    pub(super) fn sync_manipulator(&mut self, cx: &mut Context<Self>) {
        let arrow = |a: &Arrow| {
            let tip = a.base + a.dir * a.length;
            (tip, tip + a.dir * a.length.signum() * 1e-3, ARROW_SHAFT_PX + ARROW_HEAD_PX)
        };
        // The handle end of the arc (last few degrees) is grabbable.
        // The arc's last stretch, then the stem and head past its end (screen-sized).
        let arc = |h: &ArcHandle| {
            let end = h.point(h.angle);
            let tangent = (h.axis.cross(end - h.origin) * if h.angle < 0.0 { -1.0 } else { 1.0 }).normalize_or_zero();
            (end, end + tangent * 1e-3, ARC_STEM_PX + ARC_HEAD_PX)
        };
        // In handle order (Viewport::manipulator_index_at: 0 the first, 1 the second end's).
        let handles: Vec<(DVec3, DVec3, f64)> = match self.current_manip(cx) {
            // The arrow glyph at the end of the length (see manipulator_overlay).
            Some(Manip::Handles(hs)) => hs
                .into_iter()
                .flatten()
                .map(|h| match h {
                    Handle::Arrow(a) | Handle::StdArrow(a, _) => arrow(&a),
                    Handle::Arc(h) | Handle::StdArc(h, _) => arc(&h),
                })
                .collect(),
            Some(Manip::Triad(t)) => {
                // Routing curve's other points can be clicked (made active).
                let mut h = self.triad_hit_segments(&t, cx);
                h.extend(self.routing_points(cx).into_iter().map(|(_, p)| (p, p + DVec3::X * 1e-6, 0.0)));
                h
            }
            Some(Manip::Section(s)) => {
                let mut h = self.section_hit_segments(&s, cx);
                h.extend(self.section_other_manips().iter().flat_map(|(_, o)| self.section_hit_segments(o, cx)));
                h
            }
            None => Vec::new(),
        };
        // Datum boxes can be dragged too.
        let mut handles = handles;
        handles.extend(self.annotation_handles(cx));
        self.viewport.update(cx, |v, _| v.manipulators = handles);
    }

    /// The active manipulator's glyph, built by the renderer for every
    /// frame's camera (its head faces the viewer and keeps its screen size).
    pub(super) fn manipulator_geometry(&self, cx: &App) -> Option<std::sync::Arc<ManipGeometry>> {
        let manip = self.current_manip(cx)?;
        Some(std::sync::Arc::new(ManipGeometry {
            manip,
            arrow_hover: self.ui.manip_hover,
            arrow_dragging: self.manip_drag.map(|d| d.which),
            triad_hover: self.ui.triad_drag.as_ref().map(|d| d.handle).or(self.ui.triad_hover),
            triad_dragging: self.ui.triad_drag.is_some(),
            section_hover: self.ui.section_hover,
            section_dragging: if self.ui.section_base_drag {
                Some(super::section_manip::SectionHandle::Base)
            } else if let Some(r) = self.ui.section_rot.filter(|r| r.dragging) {
                Some(super::section_manip::SectionHandle::Ring(r.ring))
            } else {
                self.manip_drag.map(|_| super::section_manip::SectionHandle::Arrow)
            },
            section_face: self.section_face_marks(),
            section_others: self.section_other_manips().into_iter().map(|(i, s)| (s, self.ui.section_hover_other.filter(|h| h.0 == i).map(|h| h.1))).collect(),
        }))
    }

    /// Axis parameter of the point on the arrow axis closest to a view ray.
    fn axis_param(a: &Arrow, origin: DVec3, dir: DVec3) -> Option<f64> {
        // Closest points between the axis line and the ray.
        let w0 = a.base - origin;
        let (b, d, e) = (a.dir.dot(dir), a.dir.dot(w0), dir.dot(w0));
        let denom = 1.0 - b * b;
        if denom.abs() < 1e-9 {
            return None;
        }
        Some((b * e - d) / denom)
    }

    /// Handle tool events for a manipulator drag. Returns true if consumed.
    pub(super) fn manipulator_event(&mut self, ev: &ViewportEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let mut manip = self.current_manip(cx);
        // Another section plane's handle grabbed: that plane becomes the one the manipulator drives.
        if let (ViewportEvent::ToolDown { position, .. }, Some(Manip::Section(s))) = (ev, manip)
            && self.section_handle_at(&s, *position, cx).is_none()
            && let Some((i, _)) = self.section_other_at(*position, cx)
        {
            if let Some(d) = &mut self.ui.section_dialog {
                d.active = i;
            }
            self.reset_section_manip();
            self.ui.section_hover_other = None;
            self.apply_section(window, cx);
            manip = self.current_manip(cx);
        }
        // The handle grabbed (the nearest), kept for the drag.
        let count = match manip {
            Some(Manip::Handles(hs)) => hs.iter().flatten().count(),
            _ => 1,
        };
        let which = match ev {
            ViewportEvent::ToolDown { position, .. } => self.viewport.read(cx).manipulator_index_at(*position).filter(|&i| i < count).unwrap_or(0),
            _ => self.manip_drag.map_or(0, |d| d.which),
        };
        let a = match manip {
            Some(Manip::Handles(hs)) => match (hs.get(which).copied().flatten(), which) {
                (Some(Handle::Arrow(a)), _) => a,
                (Some(Handle::Arc(h)), which) => return self.arc_event(h, which, ev, window, cx),
                (Some(Handle::StdArrow(a, i)), which) => return self.std_arrow_event(a, i, which, ev, window, cx),
                (Some(Handle::StdArc(h, i)), which) => return self.std_arc_event(h, i, which, ev, window, cx),
                (None, _) => {
                    self.manip_drag = None;
                    return false;
                }
            },
            Some(Manip::Triad(t)) => return self.triad_event(t, ev, window, cx),
            // Rings and the base circle first; the arrow as any other.
            Some(Manip::Section(s)) => match self.section_manip_event(&s, ev, window, cx) {
                Some(done) => return done,
                None => s.arrow,
            },
            None => {
                self.manip_drag = None;
                return false;
            }
        };
        let ray = |this: &Self, p: DVec2, cx: &App| this.viewport.read(cx).ray(p);
        match ev {
            ViewportEvent::ToolDown { position, .. } => {
                let (o, d) = ray(self, *position, cx);
                let t = Self::axis_param(&a, o, d).unwrap_or(a.length);
                self.manip_drag = Some(ManipDrag { grab_offset: t - a.length, which, delta: 0.0 });
                true
            }
            ViewportEvent::ToolDrag { position, .. } => {
                let Some(m) = self.manip_drag else { return false };
                // A direction-only arrow (Up to …) has nothing to drag (moved: no click).
                if a.param.is_empty() {
                    self.manip_drag = Some(ManipDrag { delta: 1.0, ..m });
                    return true;
                }
                let (o, d) = ray(self, *position, cx);
                let Some(t) = Self::axis_param(&a, o, d) else { return true };
                if a.param == super::section::SECTION_PARAM {
                    self.drag_section_arrow(t - m.grab_offset, window, cx);
                    return true;
                }
                let mut len = t - m.grab_offset;
                let symmetric = self.edit.as_ref().is_some_and(|e| e.dialog.read(cx).feature.params.bool("symmetric"));
                if symmetric {
                    len *= 2.0;
                }
                let flip = len < 0.0 && a.flip.is_some();
                // Move curve boundary's past its end stays at the least (std's
                // extendCurveManipulatorChange: MIN_EXTEND_VALUE).
                if a.param == "extensionDistance" {
                    len = len.max(0.0);
                }
                // Live (dialogs/extrude-06, the triad and section arrows): a
                // dragged length moves in 0.1 mm steps ("13.9 mm", "2.6 mm"),
                // or 0.01 in in imperial units ("2.06 in", "1.76 in").
                let units = self.doc.read(cx).session.doc.settings.units;
                let imperial = matches!(units.length, noshape_units::LengthUnit::Inch | noshape_units::LengthUnit::Foot | noshape_units::LengthUnit::Yard);
                let step = if imperial { 0.000254 } else { 0.0001 };
                let len = ((len.abs() / step).round() * step).max(step);
                let text = noshape_units::format_quantity(Quantity::new(len, Dim::LENGTH), &units);
                if let Some(e) = &self.edit {
                    e.dialog.update(cx, |dlg, cx| {
                        if flip && let Some(f) = a.flip {
                            let cur = dlg.feature.params.bool(f);
                            dlg.feature.params.set(f, ParamValue::Bool(!cur));
                        }
                        dlg.set_expr_text(a.param, &text, window, cx);
                        // Live: the field shows the value selected while dragging.
                        dlg.focus_field_selected(a.param, window, cx);
                    });
                }
                if flip {
                    // Keep grabbing the same point after the flip.
                    self.manip_drag = Some(ManipDrag { grab_offset: -m.grab_offset, ..m });
                }
                true
            }
            // std's flipManipulator (live 11-01, Extrude Through all): a
            // click on a direction-only arrow flips it.
            ViewportEvent::ToolUp { .. } if a.param.is_empty() => {
                let clicked = self.manip_drag.take().is_some_and(|m| m.delta == 0.0);
                if clicked
                    && let (Some(f), Some(e)) = (a.flip, &self.edit)
                {
                    e.dialog.update(cx, |dlg, cx| {
                        let cur = dlg.feature.params.bool(f);
                        dlg.set_value(f, ParamValue::Bool(!cur), cx);
                    });
                }
                true
            }
            ViewportEvent::ToolUp { .. } => self.end_manip_drag(a.param, window, cx),
            _ => false,
        }
    }
}

impl PartStudioView {
    /// End of a manipulator drag: Onshape leaves the value selected in its field.
    fn end_manip_drag(&mut self, param: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let dragged = self.manip_drag.take().is_some();
        if dragged && let Some(e) = &self.edit {
            e.dialog.update(cx, |dlg, cx| dlg.focus_field_selected(param, window, cx));
        }
        dragged
    }

    /// An angle arc's drag (std angularManipulator, live psb 10-24 rev-*):
    /// the angle follows the cursor around the axis, continuous and snapping
    /// to a multiple of 5° within 2° of it (live's 45 → 47.5365 → 50 →
    /// 52.2405 → 52.8758 → 55), up to a whole turn either way; dragged back
    /// past zero the first arc flips the direction (std's change functions:
    /// `oppositeDirection = angle < 0`, the angle its magnitude). The field
    /// shows up to four decimals.
    fn arc_event(&mut self, h: ArcHandle, which: usize, ev: &ViewportEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        // Angle of the cursor around the axis (ray ∩ plane through the arc), in (−π, π].
        let angle_at = |this: &Self, p: DVec2, cx: &App| -> Option<f64> {
            let (o, d) = this.viewport.read(cx).ray(p);
            let denom = d.dot(h.axis);
            if denom.abs() < 1e-6 {
                return None;
            }
            let hit = o + d * ((h.origin - o).dot(h.axis) / denom);
            let v = hit - h.origin;
            let (x, y) = (h.radial.normalize(), h.axis.cross(h.radial).normalize());
            Some(v.dot(y).atan2(v.dot(x)))
        };
        let symmetric = h.param == "angle" && self.edit.as_ref().is_some_and(|e| e.dialog.read(cx).feature.params.bool("symmetric"));
        match ev {
            ViewportEvent::ToolDown { position, .. } => {
                // The cursor's angle next to the handle's: the drag unwraps from it.
                let tau = std::f64::consts::TAU;
                let a0 = angle_at(self, *position, cx).map_or(h.angle, |raw| h.angle + (raw - h.angle + std::f64::consts::PI).rem_euclid(tau) - std::f64::consts::PI);
                self.manip_drag = Some(ManipDrag { grab_offset: a0, which, delta: a0 - h.angle });
                true
            }
            ViewportEvent::ToolDrag { position, .. } => {
                let Some(m) = self.manip_drag else { return false };
                let Some(raw) = angle_at(self, *position, cx) else { return true };
                // Unwrapped next to the last angle, within a whole turn.
                let tau = std::f64::consts::TAU;
                let a = m.grab_offset + (raw - m.grab_offset + std::f64::consts::PI).rem_euclid(tau) - std::f64::consts::PI;
                self.manip_drag = Some(ManipDrag { grab_offset: a, ..m });
                let handle = (a - m.delta).clamp(-tau, tau);
                let mut value = if symmetric { (handle * 2.0).clamp(-tau, tau) } else { handle };
                // Only the first arc flips; the second keeps its magnitude (std angleBack = abs).
                let flip = value < 0.0 && h.flip.is_some();
                // Draft (not checked on live): no 5° snap, tenths of a degree.
                let deg = if h.max < std::f64::consts::TAU { ((value.abs().to_degrees() * 10.0).round() / 10.0).min(h.max.to_degrees()) } else { snap_degrees(value.abs().to_degrees()) };
                value = deg.to_radians();
                let units = self.doc.read(cx).session.doc.settings.units;
                let text = if matches!(units.angle, noshape_units::AngleUnit::Degree) {
                    format!("{} deg", format!("{deg:.4}").trim_end_matches('0').trim_end_matches('.'))
                } else {
                    noshape_units::format_quantity(Quantity::new(value, Dim::ANGLE), &units)
                };
                if let Some(e) = &self.edit {
                    e.dialog.update(cx, |dlg, cx| {
                        if flip && let Some(f) = h.flip {
                            let cur = dlg.feature.params.bool(f);
                            dlg.feature.params.set(f, ParamValue::Bool(!cur));
                        }
                        dlg.set_expr_text(h.param, &text, window, cx);
                        // Live: the field shows the value selected while dragging.
                        dlg.focus_field_selected(h.param, window, cx);
                    });
                }
                if flip {
                    // The axis turned over: the same cursor angle is the negated one.
                    self.manip_drag = Some(ManipDrag { grab_offset: -a, which, delta: -m.delta });
                }
                true
            }
            ViewportEvent::ToolUp { .. } => self.end_manip_drag(h.param, window, cx),
            _ => false,
        }
    }
}

impl PartStudioView {
    /// The edited feature's FeatureScript manipulators (`addManipulators`,
    /// from its last preview; the one being dragged where it is now), the
    /// drawable ones: lengths and flips as arrows, angles as arcs.
    fn std_handles(&self, cx: &App) -> Option<Manip> {
        Manip::of(self.std_handle_list(cx))
    }

    /// [`Self::std_handles`]' handles.
    fn std_handle_list(&self, cx: &App) -> Vec<Handle> {
        self.feature_manipulators(cx)
            .iter()
            .enumerate()
            .filter_map(|(i, (_, m))| {
                let m = match &self.ui.std_manip_live {
                    Some((j, live)) if *j == i => live,
                    _ => m,
                };
                std_handle(m, i)
            })
            .collect()
    }

    /// The edited feature's FeatureScript manipulators: what its preview
    /// added (`addManipulators`), else std's computed here for the native
    /// features std's change function serves (Draft's parting line).
    fn feature_manipulators(&self, cx: &App) -> Vec<(String, noshape_model::display::FeatureManipulator)> {
        let Some(d) = self.edit.as_ref().map(|e| e.dialog.read(cx)) else { return Vec::new() };
        if let Some(state) = self.display.feature_state(d.feature.id).filter(|s| !s.manipulators.is_empty()) {
            return state.manipulators.clone();
        }
        let p = &d.feature.params;
        match d.feature.type_id.as_str() {
            "draft" if p.is("draftFeatureType", "PARTING_LINE") => {
                let Some(ctx) = self.ui.before_context.as_ref().or(self.context.as_ref()) else { return Vec::new() };
                noshape_model::interaction::draft_parting_manipulators(ctx, p, &self.doc.read(cx).session.doc.settings.units)
            }
            "helix" => {
                let Some(ctx) = self.ui.before_context.as_ref().or(self.context.as_ref()) else { return Vec::new() };
                noshape_model::interaction::helix_manipulators(ctx, p, &self.doc.read(cx).session.doc.settings.units, &self.display.variables)
            }
            "mutualTrim" => {
                let Some(ctx) = self.ui.before_context.as_ref().or(self.context.as_ref()) else { return Vec::new() };
                noshape_model::interaction::mutual_trim_manipulators(ctx, p, &self.doc.read(cx).session.doc.settings.units)
            }
            "faceBlend" => {
                let Some(ctx) = self.ui.before_context.as_ref().or(self.context.as_ref()) else { return Vec::new() };
                noshape_model::interaction::face_blend_manipulators(ctx, p, &self.doc.read(cx).session.doc.settings.units)
            }
            "fillet" if p.bool("isPartial") => {
                let Some(ctx) = self.ui.before_context.as_ref().or(self.context.as_ref()) else { return Vec::new() };
                noshape_model::interaction::partial_fillet_manipulators(ctx, p, &self.doc.read(cx).session.doc.settings.units)
            }
            _ => Vec::new(),
        }
    }

    /// The FeatureScript manipulator `i` as it stands (dragged or as the preview added it), with its key.
    fn std_manipulator(&self, i: usize, cx: &App) -> Option<(String, noshape_model::display::FeatureManipulator)> {
        let (key, m) = self.feature_manipulators(cx).get(i)?.clone();
        Some((key, self.ui.std_manip_live.as_ref().filter(|(j, _)| *j == i).map_or(m, |(_, live)| live.clone())))
    }

    /// Manipulator `i` moved to `moved`: shown there at once; the feature's
    /// "Manipulator Change Function" runs in the background on the model
    /// before it and its parameter changes go to the dialog (a result for
    /// values changed since is dropped).
    fn std_manip_change(&mut self, i: usize, moved: noshape_model::display::FeatureManipulator, cx: &mut Context<Self>) {
        let Some((key, _)) = self.std_manipulator(i, cx) else { return };
        let Some(ctx) = self.ui.before_context.clone() else { return };
        let Some(dialog) = self.edit.as_ref().map(|e| e.dialog.clone()) else { return };
        let (ty, id, name, params) = {
            let d = dialog.read(cx);
            (d.feature.type_id.clone(), d.feature.id, d.feature.name.clone(), d.feature.params.clone())
        };
        self.ui.std_manip_live = Some((i, moved.clone()));
        self.refresh_scene(false, cx);
        let units = self.doc.read(cx).session.doc.settings.units;
        let sent = params.clone();
        self.ui.std_manip_task = Some(cx.spawn(async move |this, cx| {
            let changed = cx.background_spawn(async move { noshape_model::custom::std_feature::manipulator_change_in((*ctx).clone(), &ty, id, &name, &units, &sent, &key, &moved) }).await;
            let Some(changed) = changed else { return };
            let _ = this.update(cx, |this: &mut Self, cx| {
                if dialog.read(cx).feature.params != params {
                    return;
                }
                let changes: Vec<(String, ParamValue)> = changed.0.into_iter().filter(|(k, v)| params.get(k) != Some(v)).collect();
                if changes.is_empty() {
                    return;
                }
                let mut next = params;
                for (k, v) in &changes {
                    next.set(k, v.clone());
                }
                this.ui.std_edit_old = Some(next);
                dialog.update(cx, |d, cx| {
                    for (k, v) in changes {
                        d.set_value_logic(&k, v, cx);
                    }
                });
            });
        }));
    }

    /// End of a FeatureScript manipulator's drag: the drawn one follows the
    /// preview again; its primaryParameterId's field shows the value
    /// selected (manipulator.fs: the dialog focus moves there).
    fn std_manip_end(&mut self, primary: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        self.manip_drag = None;
        self.ui.std_manip_live = None;
        if let (Some(p), Some(e)) = (primary, &self.edit) {
            e.dialog.update(cx, |dlg, cx| dlg.focus_field_selected(&p, window, cx));
        }
    }

    /// A FeatureScript length or flip arrow: a drag moves a length along
    /// its direction (signed, within its min / max, 0.1 mm steps as ours);
    /// a click on a flip arrow flips it.
    fn std_arrow_event(&mut self, a: Arrow, i: usize, which: usize, ev: &ViewportEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        use noshape_model::display::FeatureManipulator as M;
        let Some((_, m)) = self.std_manipulator(i, cx) else { return false };
        let ray = |this: &Self, p: DVec2, cx: &App| this.viewport.read(cx).ray(p);
        match ev {
            ViewportEvent::ToolDown { position, .. } => {
                let (o, d) = ray(self, *position, cx);
                let t = Self::axis_param(&a, o, d).unwrap_or(a.length);
                self.manip_drag = Some(ManipDrag { grab_offset: t - a.length, which, delta: 0.0 });
                true
            }
            ViewportEvent::ToolDrag { position, .. } => {
                let Some(md) = self.manip_drag else { return false };
                self.manip_drag = Some(ManipDrag { delta: 1.0, ..md });
                let M::Linear { min, max, .. } = m else { return true };
                let (o, d) = ray(self, *position, cx);
                let Some(t) = Self::axis_param(&a, o, d) else { return true };
                let units = self.doc.read(cx).session.doc.settings.units;
                let imperial = matches!(units.length, noshape_units::LengthUnit::Inch | noshape_units::LengthUnit::Foot | noshape_units::LengthUnit::Yard);
                let step = if imperial { 0.000254 } else { 0.0001 };
                let value = (((t - md.grab_offset) / step).round() * step).clamp(min, max);
                let mut moved = m.clone();
                if let M::Linear { offset, .. } = &mut moved {
                    *offset = value;
                }
                self.std_manip_change(i, moved, cx);
                true
            }
            ViewportEvent::ToolUp { .. } => {
                let clicked = self.manip_drag.is_some_and(|d| d.delta == 0.0);
                let primary = match &m {
                    M::Linear { primary, .. } => primary.clone(),
                    _ => None,
                };
                if clicked && let M::Flip { flipped, .. } = &m {
                    let mut moved = m.clone();
                    if let M::Flip { flipped: f, .. } = &mut moved {
                        *f = !*flipped;
                    }
                    self.std_manip_change(i, moved, cx);
                }
                self.std_manip_end(primary.filter(|_| !clicked), window, cx);
                true
            }
            _ => false,
        }
    }

    /// A FeatureScript angle arc: dragged about its axis (signed, within
    /// its min / max, snapping as Revolve's).
    fn std_arc_event(&mut self, h: ArcHandle, i: usize, which: usize, ev: &ViewportEvent, window: &mut Window, cx: &mut Context<Self>) -> bool {
        use noshape_model::display::FeatureManipulator as M;
        let Some((_, m)) = self.std_manipulator(i, cx) else { return false };
        let M::Angular { min, max, ref primary, .. } = m else { return false };
        let angle_at = |this: &Self, p: DVec2, cx: &App| -> Option<f64> {
            let (o, d) = this.viewport.read(cx).ray(p);
            let denom = d.dot(h.axis);
            if denom.abs() < 1e-6 {
                return None;
            }
            let v = o + d * ((h.origin - o).dot(h.axis) / denom) - h.origin;
            let (x, y) = (h.radial.normalize(), h.axis.cross(h.radial).normalize());
            Some(v.dot(y).atan2(v.dot(x)))
        };
        let (pi, tau) = (std::f64::consts::PI, std::f64::consts::TAU);
        match ev {
            ViewportEvent::ToolDown { position, .. } => {
                let a0 = angle_at(self, *position, cx).map_or(h.angle, |raw| h.angle + (raw - h.angle + pi).rem_euclid(tau) - pi);
                self.manip_drag = Some(ManipDrag { grab_offset: a0, which, delta: a0 - h.angle });
                true
            }
            ViewportEvent::ToolDrag { position, .. } => {
                let Some(md) = self.manip_drag else { return false };
                let Some(raw) = angle_at(self, *position, cx) else { return true };
                let a = md.grab_offset + (raw - md.grab_offset + pi).rem_euclid(tau) - pi;
                self.manip_drag = Some(ManipDrag { grab_offset: a, ..md });
                let value = a - md.delta;
                let value = (snap_degrees(value.abs().to_degrees()).to_radians() * value.signum()).clamp(min, max);
                let mut moved = m.clone();
                if let M::Angular { angle, .. } = &mut moved {
                    *angle = value;
                }
                self.std_manip_change(i, moved, cx);
                true
            }
            ViewportEvent::ToolUp { .. } => {
                self.std_manip_end(primary.clone(), window, cx);
                true
            }
            _ => false,
        }
    }
}

/// A FeatureScript manipulator as one of our handles (`i`: its index).
fn std_handle(m: &noshape_model::display::FeatureManipulator, i: usize) -> Option<Handle> {
    use noshape_model::display::FeatureManipulator as M;
    Some(match m {
        M::Linear { base, direction, offset, secondary, .. } => Handle::StdArrow(Arrow { base: *base, dir: *direction, length: *offset, param: "", flip: None, secondary: *secondary }, i),
        M::Flip { base, direction, flipped, other_direction, secondary } => {
            let dir = if *flipped { other_direction.unwrap_or(-*direction) } else { *direction };
            Handle::StdArrow(Arrow { base: *base, dir, length: 0.0, param: "", flip: None, secondary: *secondary }, i)
        }
        M::Angular { axis_origin, axis_direction, rotation_origin, angle, min, max, secondary, .. } => {
            let rel = *rotation_origin - *axis_origin;
            let radial = rel - *axis_direction * axis_direction.dot(rel);
            if radial.length() < 1e-12 {
                return None;
            }
            Handle::StdArc(ArcHandle { origin: *axis_origin + *axis_direction * axis_direction.dot(rel), axis: *axis_direction, radial, angle: *angle, param: "", flip: None, secondary: *secondary, max: max.abs().max(min.abs()) }, i)
        }
        M::Points { .. } => return None,
    })
}

/// A dragged angle (degrees, ≥ 0) as live sets it: a multiple of 5° within
/// 2° of one, else as dragged; at most a whole turn.
pub(super) fn snap_degrees(deg: f64) -> f64 {
    let near = (deg / 5.0).round() * 5.0;
    (if (deg - near).abs() < 2.0 { near } else { deg }).min(360.0)
}

/// A point on a triangulated region near its area centroid (the centroid of
/// a region with holes can fall outside it).
fn region_anchor(positions: &[[f32; 3]], indices: &[u32]) -> Option<DVec3> {
    let v = |i: u32| positions.get(i as usize).map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64));
    let tris: Vec<(DVec3, f64)> = indices
        .as_chunks::<3>().0.iter()
        .filter_map(|t| {
            let (a, b, c) = (v(t[0])?, v(t[1])?, v(t[2])?);
            Some(((a + b + c) / 3.0, (b - a).cross(c - a).length() * 0.5))
        })
        .collect();
    let total: f64 = tris.iter().map(|t| t.1).sum();
    if total <= 0.0 {
        return None;
    }
    let centroid = tris.iter().fold(DVec3::ZERO, |acc, t| acc + t.0 * t.1) / total;
    // Nearest sizable triangle center to the centroid.
    let min_area = total / tris.len() as f64 * 0.25;
    tris.iter().filter(|t| t.1 >= min_area).min_by(|a, b| a.0.distance_squared(centroid).total_cmp(&b.0.distance_squared(centroid))).map(|t| t.0)
}

/// A manipulator's glyph for the renderer (`noshape_render::camera_geometry`).
pub(super) struct ManipGeometry {
    manip: Manip,
    /// The arrow / arc handle under the cursor and the one dragged (0 the
    /// first, 1 the second end's).
    arrow_hover: Option<usize>,
    arrow_dragging: Option<usize>,
    triad_hover: Option<usize>,
    triad_dragging: bool,
    section_hover: Option<super::section_manip::SectionHandle>,
    section_dragging: Option<super::section_manip::SectionHandle>,
    /// The face under a dragged section base (its implicit points).
    section_face: Option<super::section_manip::FaceMarks>,
    /// The other section planes' manipulators and the handle hovered on each.
    section_others: Vec<(super::section_manip::SectionManip, Option<super::section_manip::SectionHandle>)>,
}

impl ManipGeometry {
    /// A feature's handles' glyphs, the hovered or dragged one last (on top).
    fn lit_last<T>(&self, handles: &[Option<T>], glyph: impl Fn(&T, bool, bool) -> Vec<Instance>) -> Vec<Instance> {
        let lit = self.arrow_dragging.or(self.arrow_hover);
        let mut order: Vec<usize> = (0..handles.len()).filter(|&i| Some(i) != lit).collect();
        order.extend(lit.filter(|&i| i < handles.len()));
        order.into_iter().filter_map(|i| handles[i].as_ref().map(|h| glyph(h, self.arrow_hover == Some(i), self.arrow_dragging == Some(i)))).flatten().collect()
    }
}

impl noshape_render::camera_geometry::CameraGeometry for ManipGeometry {
    fn build(&self, view: &noshape_render::camera_geometry::View, up: &noshape_render::renderer::Uploader) -> Vec<Instance> {
        match self.manip {
            // The hovered or dragged handle drawn last: Revolve's start angle arc runs over the first's.
            Manip::Handles(hs) => self.lit_last(&hs, |h, hover, drag| match h {
                Handle::Arrow(a) | Handle::StdArrow(a, _) => arrow_glyph(a, hover, drag, view, up),
                Handle::Arc(h) | Handle::StdArc(h, _) => arc_glyph(h, hover, drag, view, up),
            }),
            Manip::Triad(t) => super::triad::triad_glyph(&t, self.triad_hover, self.triad_dragging, view, up),
            Manip::Section(s) => {
                use super::section_manip::SectionHandle;
                let mut out = arrow_glyph(&s.arrow, self.section_hover == Some(SectionHandle::Arrow), self.section_dragging == Some(SectionHandle::Arrow), view, up);
                out.extend(super::section_manip::section_glyph(&s, self.section_hover, self.section_dragging, self.section_face.as_ref(), view, up));
                for (o, hover) in &self.section_others {
                    out.extend(arrow_glyph(&o.arrow, *hover == Some(SectionHandle::Arrow), false, view, up));
                    out.extend(super::section_manip::section_glyph(o, *hover, None, None, view, up));
                }
                out
            }
        }
    }
}

/// A handle's stroke colours and widths (outline, core): idle a white core
/// in a grey halo, hovered the halo blue, dragged one solid blue stroke
/// (live psb 10-24: arrows and arcs alike).
fn handle_style(hover: bool, dragging: bool) -> ([u8; 4], [u8; 4], f32, f32) {
    // Idle: live "psui (2026-10-27, P2) interaction 6.2" measured the halo
    // at #6A767D 0.75 (145–158 over white, darker than a face over one); hovered
    // as icons2; while dragged, one solid blue stroke (≈ #2c61a3,
    // dialogs/extrude-06).
    if dragging {
        (crate::theme::rgb8(0x2c61a3), crate::theme::rgb8(0x2c61a3), 2.0, 2.0)
    } else {
        (if hover { crate::theme::rgb8(0x3e6eb0) } else { [0x6a, 0x76, 0x7d, 191] }, crate::theme::rgb8(0xffffff), if hover { 4.0 } else { 3.0 }, 1.5)
    }
}

/// An open triangular head (std SECONDARY: two stacked, half as long) with
/// its base at `base`, pointing along `dir`, `side` across the view; the
/// polylines in world space.
fn head_polylines(base: DVec3, dir: DVec3, side: DVec3, len: f64, width: f64, secondary: bool) -> Vec<Vec<DVec3>> {
    let tri = |b: DVec3, l: f64| vec![b + side * width * 0.5, b + dir * l, b - side * width * 0.5, b + side * width * 0.5];
    if secondary {
        // Live ex2 (3x): each chevron ~11 px long, the second right behind the first.
        let l = len * 0.45;
        vec![tri(base, l), tri(base + dir * (l * 1.1), l)]
    } else {
        vec![tri(base, len)]
    }
}

/// An angle handle (Revolve, Move face, Draft; live psb 10-24 rev-1, rev-10
/// and live 11-01 Draft): thin radial lines from the axis to the start and
/// to the swept end, the arc between them, then past the end a straight stem
/// along the turn (~26 px) and an open head (~16 px), in the arrows'
/// white-in-grey style (blue when hovered); a second end's has a double
/// chevron on half the stem.
fn arc_glyph(h: &ArcHandle, hover: bool, dragging: bool, view: &noshape_render::camera_geometry::View, up: &noshape_render::renderer::Uploader) -> Vec<Instance> {
    let f = |v: DVec3| -> [f32; 3] { [v.x as f32, v.y as f32, v.z as f32] };
    let (outline, core, outline_px, core_px) = handle_style(hover, dragging);
    let end = h.point(h.angle);
    let Some(wpp) = view.world_per_px(end) else { return Vec::new() };
    // The stem and head along the turn at the end, the head's width across the view.
    let sign = if h.angle < 0.0 { -1.0 } else { 1.0 };
    let tangent = (h.axis.cross(end - h.origin) * sign).try_normalize().unwrap_or(h.axis);
    let side = tangent.cross(view.direction()).try_normalize().unwrap_or_else(|| tangent.any_orthonormal_vector());
    let stem = ARC_STEM_PX * if h.secondary { 0.5 } else { 1.0 } * wpp;
    let head_base = end + tangent * stem;
    let heads = head_polylines(head_base, tangent, side, ARC_HEAD_PX * wpp, ARC_HEAD_PX * wpp, h.secondary);
    let n = ((h.angle.abs().to_degrees() / 4.0).ceil() as usize).clamp(2, 90);
    let arc: Vec<[f32; 3]> = (0..=n).map(|i| f(h.point(h.angle * i as f64 / n as f64))).collect();
    let mut back_lines = LineData::default();
    let mut front = LineData::default();
    for (data, color, w) in [(&mut back_lines, outline, outline_px), (&mut front, core, core_px)] {
        data.push_polyline(&arc, color, 0, w, LineSegment::ON_TOP);
        data.push_polyline(&[f(end), f(head_base)], color, 0, w, LineSegment::ON_TOP);
        data.push_polyline(&[f(h.origin), f(h.point(0.0))], color, 0, w * 0.75, LineSegment::ON_TOP);
        data.push_polyline(&[f(h.origin), f(end)], color, 0, w * 0.75, LineSegment::ON_TOP);
        for p in &heads {
            data.push_polyline(&p.iter().map(|v| f(*v)).collect::<Vec<_>>(), color, 0, w, LineSegment::ON_TOP);
        }
    }
    vec![
        Instance::new(Drawable::Lines(up.upload_lines(&back_lines))).per_primitive_color().unpickable().ignore_clip().layer(40),
        Instance::new(Drawable::Lines(up.upload_lines(&front))).per_primitive_color().unpickable().ignore_clip().layer(41),
    ]
}

fn arrow_glyph(a: &Arrow, hover: bool, dragging: bool, view: &noshape_render::camera_geometry::View, up: &noshape_render::renderer::Uploader) -> Vec<Instance> {
    let f = |v: DVec3| -> [f32; 3] { [v.x as f32, v.y as f32, v.z as f32] };
    // Live (part-studio/230, dialogs/extrude-05): a screen-sized arrow at
    // the end of the length: a 1.5 px white shaft and a hollow triangular
    // head, outlined light gray (blue when hovered).
    let tip = a.base + a.dir * a.length;
    let dir = a.dir * a.length.signum();
    let Some(wpp) = view.world_per_px(tip) else { return Vec::new() };
    // The head's width runs across the view.
    let side = dir.cross(view.direction()).try_normalize().unwrap_or_else(|| dir.any_orthonormal_vector());
    // Live ex2 (3x): a second end's stem is half as long, then two chevrons.
    let base = tip + dir * ARROW_SHAFT_PX * if a.secondary { 0.5 } else { 1.0 } * wpp;
    let heads = head_polylines(base, dir, side, ARROW_HEAD_PX * wpp, ARROW_HEAD_W_PX * wpp, a.secondary);
    let (outline, core, outline_px, core_px) = handle_style(hover, dragging);
    let mut back = LineData::default();
    let mut front = LineData::default();
    for (data, color, width) in [(&mut back, outline, outline_px), (&mut front, core, core_px)] {
        data.push_polyline(&[f(tip), f(base)], color, 0, width, LineSegment::ON_TOP);
        for p in &heads {
            data.push_polyline(&p.iter().map(|v| f(*v)).collect::<Vec<_>>(), color, 0, width, LineSegment::ON_TOP);
        }
    }
    vec![
        Instance::new(Drawable::Lines(up.upload_lines(&back))).per_primitive_color().unpickable().ignore_clip().layer(40),
        Instance::new(Drawable::Lines(up.upload_lines(&front))).per_primitive_color().unpickable().ignore_clip().layer(41),
    ]
}
