//! The sketch tools that take model geometry: Use (project edges, faces,
//! vertices, planes and other sketches' entities into the sketch, linked so
//! they follow the model), Intersection (a face's section with the sketch
//! plane) and the external entities the constraint and dimension tools make
//! when they pick model geometry. The Part Studio's sketch mode and the agent
//! API both call these.

use glam::{DVec2, DVec3};
use noshape_model::display::{PlaneFrame, StudioDisplay};
use noshape_model::reference::{Inference, Reference};
use noshape_model::regen::context::Context;
use noshape_model::regen::features::sketch as sk_feat;
use noshape_model::regen::measure::resolve_shapes;
use noshape_model::sketch_feature::SketchData;
use noshape_scene::sketch_mode::SketchEditor;
use noshape_sketch::{EntityId, EntityKind};

/// The model the sketch is drawn in: the regenerated context (up to the
/// sketch), its display (planes, bodies' face references) and the sketch
/// plane geometry is projected onto.
#[derive(Clone, Copy)]
pub struct Model<'a> {
    pub context: &'a Context,
    pub display: &'a StudioDisplay,
    pub frame: &'a PlaneFrame,
}

impl Model<'_> {
    /// Kernel shapes of a reference: the regenerated model's, else the shown
    /// Assembly context's (its bodies join only the features referencing
    /// them, so a sketch on a context face can still Use them, qa5 CTX-USE).
    pub fn shapes(&self, r: &Reference) -> Vec<noshape_kernel::Shape> {
        let found = resolve_shapes(self.context, r);
        if !found.is_empty() || self.display.context_shapes.is_empty() {
            return found;
        }
        let ctx = Context { bodies: (self.display.context_shapes.clone()).into(), ..Default::default() };
        resolve_shapes(&ctx, r)
    }

    /// Is the referenced face curved (not planar)?
    /// The centre of circular edge `r` (model coordinates).
    pub fn circle_center(&self, r: &Reference) -> Option<DVec3> {
        let edge = self.shapes(r).into_iter().next()?;
        match edge.edge_info().ok()?.curve {
            noshape_kernel::Curve::Circle { center, .. } => Some(center),
            _ => None,
        }
    }

    /// The midpoint of open edge `r` (model coordinates), as regeneration resolves it: at its middle parameter.
    pub fn edge_midpoint(&self, r: &Reference) -> Option<DVec3> {
        let edge = self.shapes(r).into_iter().next()?;
        let info = edge.edge_info().ok()?;
        if info.start.distance(info.end) <= 1e-9 {
            return None;
        }
        edge.point_at(0.5 * (info.first_param + info.last_param)).ok()
    }

    pub fn curved_face(&self, r: &Reference) -> bool {
        self.shapes(r).first().is_some_and(|f| !f.is_planar())
    }

    /// Plane frame of a plane-like reference (datum / feature plane, planar face).
    pub fn reference_plane(&self, r: &Reference) -> Option<PlaneFrame> {
        match r {
            Reference::Plane(_) | Reference::FeaturePlane(_) | Reference::ConstructionPlane { .. } => self.display.planes.iter().find(|p| &p.reference == r).map(|p| p.frame),
            Reference::Face(_) => {
                let f = self.shapes(r).into_iter().next()?;
                let pl = f.face_plane().ok()??;
                let (n, x) = (pl.unit_normal(), pl.unit_x());
                Some(PlaneFrame { origin: pl.origin, x, y: n.cross(x), normal: n })
            }
            _ => None,
        }
    }

    /// The edges of a model face (an edge gives itself).
    pub fn edge_refs(&self, r: &Reference) -> Vec<Reference> {
        self.face_edge_refs(r, false)
    }

    /// A face's outer loop's edges (live's Offset of a face: the outer loop only); an edge itself.
    pub fn outer_edge_refs(&self, r: &Reference) -> Vec<Reference> {
        self.face_edge_refs(r, true)
    }

    fn face_edge_refs(&self, r: &Reference, outer: bool) -> Vec<Reference> {
        let mut refs: Vec<Reference> = Vec::new();
        match r {
            Reference::Edge(_) => refs.push(r.clone()),
            Reference::Face(_) => {
                let shown = self.display.bodies.iter().chain(&self.display.context_bodies);
                if let Some(b) = shown.into_iter().find(|b| b.face_refs.contains(r)) {
                    let fi = b.face_refs.iter().position(|x| x == r).unwrap_or(0);
                    if let Some(body) = self.context.body(&b.id).or_else(|| self.display.context_shapes.iter().find(|x| x.id == b.id))
                        && let Ok(face) = body.shape.sub_shape(noshape_kernel::ShapeType::Face, fi)
                    {
                        // (The outer loop: the wire spanning the most.)
                        let size = |w: &noshape_kernel::Shape| w.bounding_box(false).map_or(0.0, |b| b.diagonal());
                        let edges = if outer { face.wires().into_iter().max_by(|a, b| size(a).total_cmp(&size(b))).map_or_else(|| face.edges(), |w| w.edges()) } else { face.edges() };
                        for e in edges {
                            if let Some(i) = body.shape.index_of(&e, noshape_kernel::ShapeType::Edge)
                                && let Some(n) = body.edges.get(i)
                            {
                                refs.push(Reference::Edge((**n).clone()));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        refs
    }

    /// Use tool preview of a hovered reference: the projected curves,
    /// tessellated to `tol`.
    pub fn use_preview(&self, r: &Reference, tol: f64) -> Vec<Vec<DVec2>> {
        if let Reference::Implicit { on, inference: Inference::Midpoint } = r
            && matches!(**on, Reference::Edge(_))
        {
            return self.use_preview(on, tol);
        }
        let frame = self.frame;
        let mut out = Vec::new();
        // A curved face previews its silhouettes.
        if matches!(r, Reference::Face(_)) && self.curved_face(r) {
            if let Some(face) = self.shapes(r).into_iter().next() {
                out.extend(sk_feat::face_silhouettes(&face, frame).iter().map(|c| c.tessellate(tol)));
            }
            return out;
        }
        let refs = if matches!(r, Reference::SketchEntity { .. }) { vec![r.clone()] } else { self.outer_edge_refs(r) };
        for er in refs {
            let shapes = self.shapes(&er);
            match shapes.first().and_then(|e| sk_feat::project_edge_any(e, frame)) {
                Some(sk_feat::SectionCurve::Exact(c)) => out.push(c.tessellate(tol)),
                Some(sk_feat::SectionCurve::Sampled(p)) => out.push(p),
                None => {}
            }
        }
        out
    }
}

/// The external sketch entity standing for a model reference, created on
/// first use and linked so it follows the model: projected vertices, edges
/// and other sketches' entities, plane traces (a line through `cursor`'s
/// foot, sized to the sketch), silhouettes of curved faces and (with
/// `pierce`) the point where an edge crosses the sketch plane.
/// `construction` marks reference-only geometry (picked by the dimension /
/// constraint tools rather than used). `world_per_px` sizes plane traces
/// in an empty sketch.
pub fn external_entity(data: &mut SketchData, model: Model, world_per_px: f64, cursor: DVec2, r: &Reference, pierce: bool, construction: bool) -> Option<EntityId> {
    let frame = model.frame;
    let want_point = pierce || matches!(r, Reference::Vertex(_) | Reference::MeshPoint { .. } | Reference::Origin | Reference::Implicit { .. });
    // Use of an already-used edge makes another, coincident copy (live sketch-tools-1123, DECISIONS 11-23); a reference
    // the dimension / constraint tools pick, or one only a dimension refers to, is reused.
    let reusable = |e: &noshape_sketch::Entity| construction || e.reference_only;
    if let Some(id) = data.external_refs.iter().find(|(id, v)| *v == r && !data.use_ends.contains_key(*id) && data.sketch.entity(EntityId(**id)).is_some_and(|e| (e.kind() == EntityKind::Point) == want_point && reusable(e))).map(|(id, _)| EntityId(*id)) {
        // Use on geometry a dimension already refers to makes it sketch geometry.
        if !construction && data.sketch.entity(id).is_some_and(|e| e.reference_only) {
            let _ = data.sketch.set_reference_only(id, false);
            let _ = data.sketch.set_construction(id, false);
        }
        return Some(id);
    }
    let shape = model.shapes(r).into_iter().next();
    enum Made {
        Point(DVec2),
        Curves(Vec<sk_feat::SectionCurve>),
    }
    let made = if matches!(r, Reference::Origin) {
        Made::Point(frame.to_local(DVec3::ZERO))
    } else if let Reference::Implicit { on, inference: Inference::Midpoint } = r {
        Made::Point(frame.to_local(model.edge_midpoint(on)?))
    } else if let Reference::Implicit { on, .. } = r {
        Made::Point(frame.to_local(model.circle_center(on)?))
    } else if let Some(pl) = model.reference_plane(r).filter(|_| !matches!(r, Reference::Face(_)) || !model.curved_face(r)) {
        let (p, d) = sk_feat::plane_trace(&pl, frame)?;
        let half = (data.sketch.bbox().size().length() * 0.6).max(world_per_px * 150.0);
        let c = p + d * (cursor - p).dot(d);
        Made::Curves(vec![sk_feat::SectionCurve::Exact(noshape_sketch::Curve2::Line { a: c - d * half, b: c + d * half })])
    } else if matches!(r, Reference::Face(_)) {
        let face = shape?;
        Made::Curves(sk_feat::face_silhouettes(&face, frame).into_iter().map(sk_feat::SectionCurve::Exact).collect())
    } else {
        let sh = shape?;
        if let Ok(p) = sh.vertex_point() {
            Made::Point(frame.to_local(p))
        } else if pierce {
            Made::Point(sk_feat::pierce_point(&sh, frame)?)
        } else if sh.edge_info().is_ok_and(|i| matches!(i.curve, noshape_kernel::Curve::Line { .. }) && (frame.to_local(i.end) - frame.to_local(i.start)).length() < 1e-9) {
            // Live (skasm plane-use-1125 (12)): a straight edge seen end-on is used as a point, where it meets the plane.
            Made::Point(sk_feat::pierce_point(&sh, frame)?)
        } else {
            Made::Curves(vec![sk_feat::project_edge_any(&sh, frame)?])
        }
    };
    let mut first = None;
    match made {
        Made::Point(p) => {
            let id = data.sketch.use_external_point(p);
            // A dimension / constraint to model geometry adds no sketch
            // geometry (live, qa5 SK3): the reference is solved, not shown.
            if construction {
                let _ = data.sketch.set_reference_only(id, true);
            }
            data.external_refs.insert(id.0, r.clone());
            first = Some(id);
        }
        Made::Curves(curves) => {
            for id in sk_feat::add_section_curves(&mut data.sketch, &curves) {
                if construction {
                    let _ = data.sketch.set_construction(id, true);
                    let _ = data.sketch.set_reference_only(id, true);
                }
                data.external_refs.insert(id.0, r.clone());
                first.get_or_insert(id);
            }
        }
    }
    data.sketch.solve();
    first
}

/// The Use tool on a reference: a vertex, edge or sketch entity is
/// projected; a curved face gives its silhouettes, a planar face all its
/// edges. Returns whether anything was added.
pub fn use_reference(data: &mut SketchData, model: Model, world_per_px: f64, cursor: DVec2, r: &Reference) -> bool {
    match r {
        Reference::Origin | Reference::Vertex(_) | Reference::MeshPoint { .. } | Reference::SketchEntity { .. } | Reference::Edge(_) => external_entity(data, model, world_per_px, cursor, r, false, false).is_some(),
        // A circular edge's centre (live: a point Use on a hole's centre, bracket03-mattie's Sketch 2).
        Reference::Implicit { on, inference: Inference::Center | Inference::CenterRim | Inference::CenterOutward } if matches!(**on, Reference::Edge(_)) => external_entity(data, model, world_per_px, cursor, r, false, false).is_some(),
        // Live (skasm use-mid-1128): Use within an edge's midpoint reach makes a point there, not the edge.
        Reference::Implicit { on, inference: Inference::Midpoint } if matches!(**on, Reference::Edge(_)) => external_entity(data, model, world_per_px, cursor, r, false, false).is_some(),
        Reference::Face(_) if model.curved_face(r) => external_entity(data, model, world_per_px, cursor, r, false, false).is_some(),
        Reference::Face(_) => {
            // Live (behaviours "Use (u) on the face projected only its 4 outer edges"): the outer loop, not the holes.
            let made: Vec<EntityId> = model.outer_edge_refs(r).iter().filter_map(|er| external_entity(data, model, world_per_px, cursor, er, false, false)).collect();
            orient_loop(data, &made);
            join_loop_ends(data, &made);
            !made.is_empty()
        }
        _ => false,
    }
}

/// A used face's lines run along its loop, counterclockwise in the sketch (live maker-*: the top line of a plate's
/// outline goes from (52, 40) to (8, 40), the left one down, whatever way the model's edges run).
fn orient_loop(data: &mut SketchData, curves: &[EntityId]) {
    let items: Vec<(EntityId, noshape_sketch::Curve2)> = curves.iter().filter_map(|c| Some((*c, data.sketch.curve(*c)?))).filter(|(_, c)| !c.is_closed()).collect();
    let Some(first) = items.first() else { return };
    let near = |a: DVec2, b: DVec2| (a - b).length() <= 1e-7;
    // The loop's curves in order, each with whether it runs its own way.
    let mut chain = vec![(0, true)];
    let mut end = first.1.end_point();
    while chain.len() < items.len() {
        let next = items.iter().enumerate().filter(|(i, _)| !chain.iter().any(|(k, _)| k == i)).find_map(|(i, (_, c))| {
            if near(c.start_point(), end) {
                Some((i, true))
            } else {
                near(c.end_point(), end).then_some((i, false))
            }
        });
        let Some((i, forward)) = next else { return };
        end = if forward { items[i].1.end_point() } else { items[i].1.start_point() };
        chain.push((i, forward));
    }
    let mut ring: Vec<DVec2> = Vec::new();
    for &(i, forward) in &chain {
        let mut pts = items[i].1.tessellate(1e-5);
        if !forward {
            pts.reverse();
        }
        ring.extend(pts);
    }
    let area: f64 = ring.iter().zip(ring.iter().cycle().skip(1)).map(|(a, b)| a.perp_dot(*b)).sum();
    for (i, forward) in chain {
        // Clockwise: every curve's way turns round.
        let along = forward == (area > 0.0);
        if along || !matches!(items[i].1, noshape_sketch::Curve2::Line { .. }) {
            continue;
        }
        let Some((s, e)) = data.sketch.endpoints(items[i].0) else { continue };
        let (Some(ps), Some(pe)) = (data.sketch.point(s), data.sketch.point(e)) else { continue };
        let _ = data.sketch.set_point(s, pe);
        let _ = data.sketch.set_point(e, ps);
    }
}

/// A used face's edges meet in COINCIDENTs, each end with an earlier edge's end at its place (live sketch-tools-1123:
/// a rectangle's loop stores one per corner).
fn join_loop_ends(data: &mut SketchData, curves: &[EntityId]) {
    let ends: Vec<(EntityId, DVec2)> = curves.iter().filter_map(|c| data.sketch.endpoints(*c)).flat_map(|(s, e)| [s, e]).filter_map(|p| Some((p, data.sketch.point(p)?))).collect();
    let mut joined: Vec<EntityId> = Vec::new();
    for (i, (p, at)) in ends.iter().enumerate() {
        let Some((q, _)) = ends[..i - i % 2].iter().find(|(q, there)| !joined.contains(q) && (*there - *at).length() <= 1e-6) else { continue };
        if data.sketch.add_constraint(noshape_sketch::ConstraintKind::Coincident { a: *q, b: *p }).is_ok() {
            joined.extend([*p, *q]);
        }
    }
    data.sketch.solve();
}

/// What the Use tool reports when nothing could be used.
pub const USE_FAILED: &str = "That geometry can't be used in this sketch";

/// The Intersection tool on a face: its section with the sketch plane is
/// added as fixed curves that stay linked to the face. Errors carry the
/// message the tool shows.
pub fn intersect_face(ed: &mut SketchEditor, data: &mut SketchData, model: Model, r: &Reference) -> Result<(), String> {
    if !matches!(r, Reference::Face(_)) {
        return Err("Click a model face that crosses the sketch plane".into());
    }
    let shapes = model.shapes(r);
    let face = shapes.first().ok_or_else(|| "Click a model face that crosses the sketch plane".to_string())?;
    let curves = sk_feat::section_face(face, model.frame).map_err(|e| format!("Intersection failed: {e}"))?;
    if curves.is_empty() {
        return Err("That face does not cross the sketch plane".into());
    }
    if ed.add_intersection(data, r.clone(), &curves) { Ok(()) } else { Err(ed.message.clone().unwrap_or_else(|| "That face does not cross the sketch plane".into())) }
}
