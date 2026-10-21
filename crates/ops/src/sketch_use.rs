//! The sketch tools that take model geometry: Use (project edges, faces,
//! vertices, planes and other sketches' entities into the sketch, linked so
//! they follow the model), Intersection (a face's section with the sketch
//! plane) and the external entities the constraint and dimension tools make
//! when they pick model geometry. The Part Studio's sketch mode and the agent
//! API both call these.

use glam::DVec2;
use noshape_model::display::{PlaneFrame, StudioDisplay};
use noshape_model::reference::Reference;
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
    /// Is the referenced face curved (not planar)?
    pub fn curved_face(&self, r: &Reference) -> bool {
        resolve_shapes(self.context, r).first().is_some_and(|f| !f.is_planar())
    }

    /// Plane frame of a plane-like reference (datum / feature plane, planar face).
    pub fn reference_plane(&self, r: &Reference) -> Option<PlaneFrame> {
        match r {
            Reference::Plane(_) | Reference::FeaturePlane(_) => self.display.planes.iter().find(|p| &p.reference == r).map(|p| p.frame),
            Reference::Face(_) => {
                let f = resolve_shapes(self.context, r).into_iter().next()?;
                let pl = f.face_plane().ok()??;
                let (n, x) = (pl.unit_normal(), pl.unit_x());
                Some(PlaneFrame { origin: pl.origin, x, y: n.cross(x), normal: n })
            }
            _ => None,
        }
    }

    /// The edges of a model face (an edge gives itself).
    pub fn edge_refs(&self, r: &Reference) -> Vec<Reference> {
        let mut refs: Vec<Reference> = Vec::new();
        match r {
            Reference::Edge(_) => refs.push(r.clone()),
            Reference::Face(_) => {
                if let Some(b) = self.display.bodies.iter().find(|b| b.face_refs.contains(r)) {
                    let fi = b.face_refs.iter().position(|x| x == r).unwrap_or(0);
                    if let Some(body) = self.context.body(&b.id)
                        && let Ok(face) = body.shape.sub_shape(noshape_kernel::ShapeType::Face, fi)
                    {
                        for e in face.edges() {
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
        let frame = self.frame;
        let mut out = Vec::new();
        // A curved face previews its silhouettes.
        if matches!(r, Reference::Face(_)) && self.curved_face(r) {
            if let Some(face) = resolve_shapes(self.context, r).into_iter().next() {
                out.extend(sk_feat::face_silhouettes(&face, frame).iter().map(|c| c.tessellate(tol)));
            }
            return out;
        }
        let refs = if matches!(r, Reference::SketchEntity { .. }) { vec![r.clone()] } else { self.edge_refs(r) };
        for er in refs {
            let shapes = resolve_shapes(self.context, &er);
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
    let want_point = pierce || matches!(r, Reference::Vertex(_));
    if let Some((id, _)) = data.external_refs.iter().find(|(id, v)| *v == r && data.sketch.entity(EntityId(**id)).is_some_and(|e| (e.kind() == EntityKind::Point) == want_point)) {
        return Some(EntityId(*id));
    }
    let shape = resolve_shapes(model.context, r).into_iter().next();
    enum Made {
        Point(DVec2),
        Curves(Vec<sk_feat::SectionCurve>),
    }
    let made = if let Some(pl) = model.reference_plane(r).filter(|_| !matches!(r, Reference::Face(_)) || !model.curved_face(r)) {
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
        } else {
            Made::Curves(vec![sk_feat::project_edge_any(&sh, frame)?])
        }
    };
    let mut first = None;
    match made {
        Made::Point(p) => {
            let id = data.sketch.use_external_point(p);
            data.external_refs.insert(id.0, r.clone());
            first = Some(id);
        }
        Made::Curves(curves) => {
            for id in sk_feat::add_section_curves(&mut data.sketch, &curves) {
                if construction {
                    let _ = data.sketch.set_construction(id, true);
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
        Reference::Vertex(_) | Reference::SketchEntity { .. } | Reference::Edge(_) => external_entity(data, model, world_per_px, cursor, r, false, false).is_some(),
        Reference::Face(_) if model.curved_face(r) => external_entity(data, model, world_per_px, cursor, r, false, false).is_some(),
        Reference::Face(_) => {
            let mut changed = false;
            for er in model.edge_refs(r) {
                changed |= external_entity(data, model, world_per_px, cursor, &er, false, false).is_some();
            }
            changed
        }
        _ => false,
    }
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
    let shapes = resolve_shapes(model.context, r);
    let face = shapes.first().ok_or_else(|| "Click a model face that crosses the sketch plane".to_string())?;
    let curves = sk_feat::section_face(face, model.frame).map_err(|e| format!("Intersection failed: {e}"))?;
    if curves.is_empty() {
        return Err("That face does not cross the sketch plane".into());
    }
    if ed.add_intersection(data, r.clone(), &curves) { Ok(()) } else { Err(ed.message.clone().unwrap_or_else(|| "That face does not cross the sketch plane".into())) }
}
