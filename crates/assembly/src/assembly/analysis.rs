//! Assembly analysis: interference detection, bill of materials and
//! automatic exploded views, computed from an evaluated [`AssemblyResult`].

use super::eval::{AssemblyResult, InstancePath, PartOccurrence};
use super::{Assembly, ExplodeStep, ExplodedView, InstanceId, InstancePattern, MateConnectorRef, PatternKind, Placement, ReplicateScope, ScopeEntity};
use crate::reference::{FeatureId, Inference, Reference, Role, TopoName};
use noshape_units::UnitPrefs;
use std::sync::Arc;
use crate::configuration::ConfigValues;
use crate::display::BodyKind;
use crate::document::TabId;
use crate::reference::BodyId;
use glam::DVec3;
use noshape_kernel as k;

#[derive(Clone, Debug)]
pub struct Interference {
    pub a: InstancePath,
    pub a_body: BodyId,
    pub b: InstancePath,
    pub b_body: BodyId,
    /// Common volume (m³).
    pub volume: f64,
}

fn world_shape(o: &PartOccurrence) -> Option<k::Shape> {
    let body = o.context.body(&o.body)?;
    if o.transform.abs_diff_eq(glam::DMat4::IDENTITY, 1e-15) {
        return Some(body.shape.clone());
    }
    body.shape.transformed(&k::Transform::Matrix(o.transform)).ok()
}

/// One row of the Interference detection dialog: the two part occurrences
/// and their overlap (volume, bounding box and mesh, world coordinates).
#[derive(Clone, Debug)]
pub struct InstanceInterference {
    pub a: InstancePath,
    pub b: InstancePath,
    pub overlap: crate::interference::Interference,
}

/// Interference detection in an assembly (live assembly/735–736): overlaps
/// between the solid parts of the top-level `instances` (parts of one rigid
/// Part Studio instance are not checked against each other); standard
/// content only with `include_standard`.
pub fn instance_interferences(result: &AssemblyResult, instances: &[InstanceId], include_standard: bool) -> Vec<InstanceInterference> {
    let occ: Vec<&PartOccurrence> = result
        .occurrences
        .iter()
        .filter(|o| o.kind == BodyKind::Solid && instances.contains(&o.path[0]) && (include_standard || o.studio != super::standard::STANDARD_TAB))
        .collect();
    let shapes: Vec<(usize, k::Shape)> = occ.iter().enumerate().filter_map(|(i, o)| world_shape(o).map(|s| (i, s))).collect();
    let kernel: Vec<k::Shape> = shapes.iter().map(|(_, s)| s.clone()).collect();
    crate::interference::interferences(&kernel)
        .into_iter()
        .filter_map(|overlap| {
            let (a, b) = (occ[shapes[overlap.a].0], occ[shapes[overlap.b].0]);
            (a.path != b.path).then(|| InstanceInterference { a: a.path.clone(), b: b.path.clone(), overlap })
        })
        .collect()
}

fn boxes_overlap(a: (DVec3, DVec3), b: (DVec3, DVec3), tol: f64) -> bool {
    a.0.x <= b.1.x + tol && b.0.x <= a.1.x + tol && a.0.y <= b.1.y + tol && b.0.y <= a.1.y + tol && a.0.z <= b.1.z + tol && b.0.z <= a.1.z + tol
}

/// Pairwise interference between solid occurrences of different top-level
/// instances: bounding-box prefilter, then kernel common volume > `tol` (m³).
pub fn interference(result: &AssemblyResult, tol: f64) -> Vec<Interference> {
    let occ: Vec<&PartOccurrence> = result.occurrences.iter().filter(|o| o.kind == BodyKind::Solid).collect();
    let boxes: Vec<Option<(DVec3, DVec3)>> = occ.iter().map(|o| o.world_bounds()).collect();
    let mut shapes: Vec<Option<Option<k::Shape>>> = vec![None; occ.len()];
    let mut out = Vec::new();
    for i in 0..occ.len() {
        for j in (i + 1)..occ.len() {
            // Bodies of one rigid instance (same path) are not checked against each other.
            if occ[i].path == occ[j].path {
                continue;
            }
            let (Some(bi), Some(bj)) = (boxes[i], boxes[j]) else { continue };
            if !boxes_overlap(bi, bj, -1e-9) {
                continue;
            }
            if shapes[i].is_none() {
                shapes[i] = Some(world_shape(occ[i]));
            }
            if shapes[j].is_none() {
                shapes[j] = Some(world_shape(occ[j]));
            }
            let (Some(Some(si)), Some(Some(sj))) = (&shapes[i], &shapes[j]) else { continue };
            let Ok(c) = k::common(si, sj) else { continue };
            let v = c.shape.volume().unwrap_or(0.0);
            if v > tol {
                out.push(Interference { a: occ[i].path.clone(), a_body: occ[i].body.clone(), b: occ[j].path.clone(), b_body: occ[j].body.clone(), volume: v });
            }
        }
    }
    out
}

#[derive(Clone, Debug)]
pub struct BomRow {
    pub item: usize,
    pub name: String,
    pub studio: TabId,
    pub body: BodyId,
    pub config: ConfigValues,
    pub quantity: usize,
    pub instances: Vec<InstancePath>,
    /// Mirrored (opposite-handed) version of the part.
    pub mirrored: bool,
    /// External reference key (part inserted from another document).
    pub external: Option<String>,
}

/// Flattened parts-only bill of materials grouped by source part and
/// configuration, in first-appearance order.
pub fn bom(result: &AssemblyResult) -> Vec<BomRow> {
    let mut rows: Vec<BomRow> = Vec::new();
    for o in result.occurrences.iter().filter(|o| matches!(o.kind, BodyKind::Solid | BodyKind::Surface)) {
        if let Some(r) = rows.iter_mut().find(|r| r.studio == o.studio && r.body == o.body && r.config == o.config && r.mirrored == o.mirrored && r.external == o.external) {
            r.quantity += 1;
            r.instances.push(o.path.clone());
        } else {
            rows.push(BomRow {
                item: rows.len() + 1,
                name: if o.mirrored { format!("{} (mirrored)", o.body_name) } else { o.body_name.clone() },
                studio: o.studio,
                body: o.body.clone(),
                config: o.config.clone(),
                quantity: 1,
                instances: vec![o.path.clone()],
                mirrored: o.mirrored,
                external: o.external.clone(),
            });
        }
    }
    rows
}

/// Automatic exploded view: instances are spread along the axis in which
/// their centers differ most (Z for coaxial stacks), ordered by position,
/// `factor` times half the assembly size apart.
pub fn auto_exploded_view(result: &AssemblyResult, name: &str, factor: f64) -> ExplodedView {
    let Some((lo, hi)) = result.bounds() else { return ExplodedView { name: name.into(), steps: vec![] } };
    let size = (hi - lo).length().max(1e-3);
    let mut tops: Vec<(InstanceId, DVec3)> = Vec::new();
    let mut seen: Vec<InstanceId> = Vec::new();
    for o in &result.occurrences {
        if seen.contains(&o.top()) {
            continue;
        }
        seen.push(o.top());
        let mut acc: Option<(DVec3, DVec3)> = None;
        for x in result.occurrences.iter().filter(|x| x.top() == o.top()) {
            if let Some((a, b)) = x.world_bounds() {
                acc = Some(match acc {
                    None => (a, b),
                    Some((p, q)) => (p.min(a), q.max(b)),
                });
            }
        }
        if let Some((a, b)) = acc {
            tops.push((o.top(), (a + b) * 0.5));
        }
    }
    if tops.is_empty() {
        return ExplodedView { name: name.into(), steps: vec![] };
    }
    let (mut cmin, mut cmax) = (tops[0].1, tops[0].1);
    for (_, c) in &tops {
        cmin = cmin.min(*c);
        cmax = cmax.max(*c);
    }
    let spread = cmax - cmin;
    let axis = if spread.max_element() < size * 1e-3 || (spread.z >= spread.x && spread.z >= spread.y) {
        DVec3::Z
    } else if spread.x >= spread.y {
        DVec3::X
    } else {
        DVec3::Y
    };
    let mut order: Vec<usize> = (0..tops.len()).collect();
    order.sort_by(|&a, &b| tops[a].1.dot(axis).partial_cmp(&tops[b].1.dot(axis)).unwrap_or(std::cmp::Ordering::Equal).then(a.cmp(&b)));
    let n = tops.len() as f64;
    let mut steps = Vec::new();
    for (rank, &i) in order.iter().enumerate() {
        let t = axis * (rank as f64 - (n - 1.0) * 0.5) * size * 0.5 * factor;
        if t.length() > 0.0 {
            steps.push(ExplodeStep::translate(vec![tops[i].0], t.into()));
        }
    }
    ExplodedView { name: name.into(), steps }
}

// ------------------------------------------------------------ matched patterns

/// Geometric signature used by Replicate to find "identical" geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Signature {
    Plane { area: f64 },
    Cylinder { radius: f64, length: f64 },
    Cone { half_angle: f64, ref_radius: f64 },
    Sphere { radius: f64 },
    Torus { major: f64, minor: f64 },
    Circle { radius: f64 },
    Line { length: f64 },
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-7 + 1e-6 * a.abs().max(b.abs())
}

impl Signature {
    fn matches(&self, o: &Signature) -> bool {
        use Signature::*;
        match (*self, *o) {
            (Plane { area: a }, Plane { area: b }) => close(a, b),
            (Cylinder { radius: r1, length: l1 }, Cylinder { radius: r2, length: l2 }) => close(r1, r2) && close(l1, l2),
            (Cone { half_angle: a1, ref_radius: r1 }, Cone { half_angle: a2, ref_radius: r2 }) => close(a1, a2) && close(r1, r2),
            (Sphere { radius: a }, Sphere { radius: b }) => close(a, b),
            (Torus { major: a1, minor: b1 }, Torus { major: a2, minor: b2 }) => close(a1, a2) && close(b1, b2),
            (Circle { radius: a }, Circle { radius: b }) => close(a, b),
            (Line { length: a }, Line { length: b }) => close(a, b),
            _ => false,
        }
    }
}

fn face_signature(face: &k::Shape) -> Option<Signature> {
    Some(match face.face_surface().ok()? {
        k::Surface::Plane(_) => Signature::Plane { area: face.area().ok()? },
        k::Surface::Cylinder { axis, radius, .. } => {
            let bb = face.bounding_box(true).ok()?;
            Signature::Cylinder { radius, length: bb.size().dot(axis.normalize().abs()) }
        }
        k::Surface::Cone { half_angle, ref_radius, .. } => Signature::Cone { half_angle, ref_radius },
        k::Surface::Sphere { radius, .. } => Signature::Sphere { radius },
        k::Surface::Torus { major_radius, minor_radius, .. } => Signature::Torus { major: major_radius, minor: minor_radius },
        _ => return None,
    })
}

fn edge_signature(edge: &k::Shape) -> Option<Signature> {
    let info = edge.edge_info().ok()?;
    Some(match info.curve {
        k::Curve::Circle { radius, .. } => Signature::Circle { radius },
        k::Curve::Line { .. } => Signature::Line { length: edge.length().ok()? },
        _ => return None,
    })
}

fn split_reference(r: &Reference) -> (Reference, Option<Inference>) {
    match r {
        Reference::Implicit { on, inference } => ((**on).clone(), Some(*inference)),
        other => (other.clone(), None),
    }
}

fn join_reference(entity: Reference, inference: Option<Inference>) -> Reference {
    match inference {
        Some(i) => Reference::Implicit { on: Box::new(entity), inference: i },
        None => entity,
    }
}

fn stable_code(r: &Reference) -> u32 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_vec(r).unwrap_or_default().hash(&mut h);
    (h.finish() & 0x7fff_ffff) as u32
}

/// Replace every occurrence of face name `from` by `to` inside a name
/// (edges and vertices are rebuilt so their face lists stay sorted).
fn substitute(n: &TopoName, from: &TopoName, to: &Arc<TopoName>) -> TopoName {
    if n == from {
        return (**to).clone();
    }
    match n {
        TopoName::Split { parent, by, ordinal } => TopoName::Split { parent: Arc::new(substitute(parent, from, to)), by: *by, ordinal: *ordinal },
        TopoName::Edge { faces, ordinal } => TopoName::edge(Arc::new(substitute(&faces[0], from, to)), Arc::new(substitute(&faces[1], from, to)), *ordinal),
        TopoName::Vertex { faces, ordinal } => TopoName::vertex(faces.iter().map(|f| Arc::new(substitute(f, from, to))).collect(), *ordinal),
        TopoName::Boundary { face, ordinal } => TopoName::Boundary { face: Arc::new(substitute(face, from, to)), ordinal: *ordinal },
        other => other.clone(),
    }
}

fn contains_name(n: &TopoName, needle: &TopoName) -> bool {
    if n == needle {
        return true;
    }
    match n {
        TopoName::Split { parent, .. } => contains_name(parent, needle),
        TopoName::Edge { faces, .. } => faces.iter().any(|f| contains_name(f, needle)),
        TopoName::Vertex { faces, .. } => faces.iter().any(|f| contains_name(f, needle)),
        TopoName::Boundary { face, .. } => contains_name(face, needle),
        _ => false,
    }
}

fn entity_name(r: &Reference) -> Option<&TopoName> {
    match r {
        Reference::Face(n) | Reference::Edge(n) | Reference::Vertex(n) => Some(n),
        _ => None,
    }
}

fn with_name(r: &Reference, n: TopoName) -> Reference {
    match r {
        Reference::Face(_) => Reference::Face(n),
        Reference::Edge(_) => Reference::Edge(n),
        _ => Reference::Vertex(n),
    }
}

/// The feature that made a face (through the splits of later features).
fn face_feature(n: &TopoName) -> Option<FeatureId> {
    match n {
        TopoName::Face { feature, .. } => Some(*feature),
        TopoName::Split { parent, .. } => face_feature(parent),
        _ => None,
    }
}

/// Replicate with a match scope (help: Replicate 6–7): the entities of the
/// scope's selections that match the seed mate's entity, the seed's own
/// excluded. (target connector template, code, entity)
fn scoped_candidates(result: &AssemblyResult, target: &MateConnectorRef, seed_path: &[InstanceId], seed_entity: &Reference, seed_body: &crate::regen::context::Body, scope: ReplicateScope, refs: &[ScopeEntity]) -> Result<Vec<(MateConnectorRef, u32, Reference)>, String> {
    let face_of = |b: &crate::regen::context::Body, n: &TopoName| b.faces.iter().position(|f| &**f == n).and_then(|i| b.shape.sub_shape(k::ShapeType::Face, i).ok());
    let edge_of = |b: &crate::regen::context::Body, n: &TopoName| b.edges.iter().position(|e| &**e == n).and_then(|i| b.shape.sub_shape(k::ShapeType::Edge, i).ok());
    let sig = match seed_entity {
        Reference::Face(n) => face_of(seed_body, n).and_then(|f| face_signature(&f)),
        Reference::Edge(n) => edge_of(seed_body, n).and_then(|e| edge_signature(&e)),
        _ => None,
    }
    .ok_or("Replicate needs a seed mate on a face or edge with analytic geometry")?;
    if refs.is_empty() {
        return Err(if scope.takes_parts() { "Select parts to find match in" } else { "Select the geometry to find match in" }.into());
    }
    let mut out: Vec<(MateConnectorRef, u32, Reference)> = Vec::new();
    for r in refs {
        let mut p = vec![r.instance];
        p.extend(r.path.iter().copied());
        let Some(o) = result.occurrences.iter().find(|o| o.path == p) else { continue };
        let Some(b) = o.context.body(&o.body) else { continue };
        let picked = r.entity.as_ref().and_then(entity_name).cloned();
        let faces = || b.faces.iter().map(|f| Reference::Face((**f).clone()));
        let edges = || b.edges.iter().map(|e| Reference::Edge((**e).clone()));
        let found: Vec<Reference> = match scope {
            ReplicateScope::FacesInParts => faces().collect(),
            ReplicateScope::FacesInFeatures => {
                let Some(fid) = picked.as_ref().and_then(face_feature) else { continue };
                faces().filter(|f| entity_name(f).and_then(face_feature) == Some(fid)).collect()
            }
            ReplicateScope::IndividualFaces | ReplicateScope::IndividualEdges => r.entity.iter().cloned().collect(),
            ReplicateScope::EdgesOnPlane => {
                let Some(k::Surface::Plane(pl)) = picked.as_ref().and_then(|n| face_of(b, n)).and_then(|f| f.face_surface().ok()) else { continue };
                let n = pl.normal.normalize();
                edges()
                    .filter(|e| {
                        let Some(es) = entity_name(e).and_then(|n| edge_of(b, n)) else { return false };
                        match es.edge_info().map(|i| i.curve) {
                            Ok(k::Curve::Circle { center, axis, .. }) => (center - pl.origin).dot(n).abs() < 1e-7 && axis.normalize().cross(n).length() < 1e-6,
                            Ok(k::Curve::Line { origin, direction }) => (origin - pl.origin).dot(n).abs() < 1e-7 && direction.normalize().dot(n).abs() < 1e-6,
                            _ => false,
                        }
                    })
                    .collect()
            }
            ReplicateScope::EdgesOnFace => {
                let Some(fi) = picked.as_ref().and_then(|n| b.faces.iter().position(|f| &**f == n)) else { continue };
                let Ok(edge_faces) = b.shape.edge_faces() else { continue };
                edge_faces.iter().enumerate().filter(|(_, f)| f.contains(&(fi as u32))).filter_map(|(ei, _)| b.edges.get(ei).map(|e| Reference::Edge((**e).clone()))).collect()
            }
        };
        let mut tmpl = target.clone();
        tmpl.instance = o.path[0];
        tmpl.path = o.path[1..].to_vec();
        for e in found {
            if o.path == seed_path && &e == seed_entity {
                continue;
            }
            let s = match &e {
                Reference::Face(n) => face_of(b, n).and_then(|f| face_signature(&f)),
                Reference::Edge(n) => edge_of(b, n).and_then(|x| edge_signature(&x)),
                _ => None,
            };
            if !s.is_some_and(|s| s.matches(&sig)) {
                continue;
            }
            let code = (stable_code(&e) ^ (o.path[0].0 as u32).wrapping_mul(0x9e37_79b9)) & 0x7fff_ffff;
            if !out.iter().any(|(t, _, x)| *x == e && t.instance == tmpl.instance && t.path == tmpl.path) {
                out.push((tmpl.clone(), code, e));
            }
        }
    }
    if out.is_empty() {
        return Err("Selected target did not contain identical geometry.".into());
    }
    Ok(out)
}

/// The face a circular edge bounds (a cylinder or cone around the circle's
/// axis), and which end of it the edge is.
struct CircleOwner {
    face: usize,
    radius: f64,
    /// The face's axis (body coordinates), pointing toward the edge's end.
    toward_edge: glam::DVec3,
}

fn surface_axis(face: &k::Shape) -> Option<(glam::DVec3, glam::DVec3)> {
    match face.face_surface().ok()? {
        k::Surface::Cylinder { origin, axis, .. } | k::Surface::Cone { origin, axis, .. } => Some((origin, axis.normalize())),
        _ => None,
    }
}

fn face_center(face: &k::Shape) -> Option<glam::DVec3> {
    let bb = face.bounding_box(true).ok()?;
    Some((bb.min + bb.max) * 0.5)
}

fn circle_of(edge: &k::Shape) -> Option<(glam::DVec3, f64)> {
    match edge.edge_info().ok()?.curve {
        k::Curve::Circle { center, radius, .. } => Some((center, radius)),
        _ => None,
    }
}

fn circle_owner_face(b: &crate::regen::context::Body, edge: &TopoName) -> Option<CircleOwner> {
    let ei = b.edges.iter().position(|e| &**e == edge)?;
    let (center, radius) = circle_of(&b.shape.sub_shape(k::ShapeType::Edge, ei).ok()?)?;
    let edge_faces = b.shape.edge_faces().ok()?;
    edge_faces.get(ei)?.iter().find_map(|&fi| {
        let face = b.shape.sub_shape(k::ShapeType::Face, fi as usize).ok()?;
        let (origin, axis) = surface_axis(&face)?;
        // The circle lies around the face's axis.
        let off = center - origin;
        if (off - axis * off.dot(axis)).length() > 1e-6 {
            return None;
        }
        let side = (center - face_center(&face)?).dot(axis);
        Some(CircleOwner { face: fi as usize, radius, toward_edge: if side >= 0.0 { axis } else { -axis } })
    })
}

/// The circular edge of face `fi` at the same end as the seed's edge on its
/// face (both measured along the faces' axes, oriented alike).
fn matching_end_edge(b: &crate::regen::context::Body, fi: usize, seed: &CircleOwner) -> Option<Reference> {
    let face = b.shape.sub_shape(k::ShapeType::Face, fi).ok()?;
    let (_, axis) = surface_axis(&face)?;
    let axis = if axis.dot(seed.toward_edge) >= 0.0 { axis } else { -axis };
    let mid = face_center(&face)?;
    let edge_faces = b.shape.edge_faces().ok()?;
    edge_faces.iter().enumerate().filter(|(_, f)| f.contains(&(fi as u32))).find_map(|(ei, _)| {
        let (center, radius) = circle_of(&b.shape.sub_shape(k::ShapeType::Edge, ei).ok()?)?;
        (close(radius, seed.radius) && (center - mid).dot(axis) > 0.0).then(|| Reference::Edge((*b.edges[ei]).clone()))
    })
}

/// Targets of a part pattern / replicate: one connector per matching
/// geometry on the seed mate's target instance (code, target connector,
/// initial placement for the copy). The seed's own target is excluded.
pub fn matched_targets(result: &AssemblyResult, asm: &Assembly, pattern: &InstancePattern, units: &UnitPrefs) -> Result<Vec<(u32, MateConnectorRef, Option<Placement>)>, String> {
    let (mate_id, by_pattern, all_instances, scope, scope_refs) = match &pattern.kind {
        PatternKind::PartPattern { mate } => (*mate, true, false, None, &[][..]),
        PatternKind::Replicate { mate, all_instances, scope, scope_refs } => (*mate, false, *all_instances, *scope, scope_refs.as_slice()),
        _ => return Err("Not a matched pattern".into()),
    };
    let seed = *pattern.seeds.first().ok_or("Select a seed instance")?;
    let mate = asm.mate(mate_id).ok_or("The seed mate no longer exists")?;
    let target = if mate.a.instance == seed {
        &mate.b
    } else if mate.b.instance == seed {
        &mate.a
    } else {
        return Err("The mate to be matched must mate the seed instance".into());
    };
    if target.connector.is_some() || target.instance == InstanceId::ORIGIN {
        return Err("The seed mate must reference geometry (not a mate connector) to be matched".into());
    }
    let mut path = vec![target.instance];
    path.extend(target.path.iter().copied());
    let occ = result.occurrences.iter().find(|o| o.path == path).ok_or("Target instance is not available")?;
    let body = occ.context.body(&occ.body).ok_or("Target part is not available")?;
    let (entity, inference) = split_reference(&target.reference);
    let name = entity_name(&entity).ok_or("The seed mate must reference a face, edge or vertex")?.clone();
    // (target connector template, code, entity) candidates.
    let mut candidates: Vec<(MateConnectorRef, u32, Reference)> = Vec::new();
    if by_pattern {
        // Faces created by Part Studio patterns as copies of a face the target uses.
        for f in &body.faces {
            let TopoName::Face { role: Role::Copy { instance, of }, .. } = &**f else { continue };
            if !contains_name(&name, of) {
                continue;
            }
            let r = with_name(&entity, substitute(&name, of, f));
            if !candidates.iter().any(|(_, _, c)| *c == r) {
                candidates.push((target.clone(), *instance, r));
            }
        }
        if candidates.is_empty() {
            return Err("The seed mate's target geometry is not patterned in its Part Studio".into());
        }
    } else if let Some(scope) = scope {
        candidates = scoped_candidates(result, target, &path, &entity, body, scope, scope_refs)?;
    } else {
        let sig_in = |b: &crate::regen::context::Body, r: &Reference| -> Option<Signature> {
            match r {
                Reference::Face(n) => {
                    let i = b.faces.iter().position(|f| &**f == n)?;
                    face_signature(&b.shape.sub_shape(k::ShapeType::Face, i).ok()?)
                }
                Reference::Edge(n) => {
                    let i = b.edges.iter().position(|e| &**e == n)?;
                    edge_signature(&b.shape.sub_shape(k::ShapeType::Edge, i).ok()?)
                }
                _ => None,
            }
        };
        // Live ("Match faces in parts"): a seed on a circular edge matches
        // through the face the circle bounds (a hole's cylinder), and each
        // copy goes on the matching face's edge at the same end.
        let owner = match &entity {
            Reference::Edge(n) => circle_owner_face(body, n),
            _ => None,
        };
        let sig = match &owner {
            Some(o) => face_signature(&body.shape.sub_shape(k::ShapeType::Face, o.face).map_err(|e| e.to_string())?),
            None => sig_in(body, &entity),
        }
        .ok_or("Replicate needs a seed mate on a face or edge with analytic geometry")?;
        // The target occurrence, plus other top-level instances of the same part.
        let mut occs: Vec<&PartOccurrence> = vec![occ];
        if all_instances {
            occs.extend(result.occurrences.iter().filter(|o| o.path != occ.path && o.path.len() == occ.path.len() && o.studio == occ.studio && o.body == occ.body && o.config == occ.config && o.path[0] != seed));
        }
        for (k, o) in occs.iter().enumerate() {
            let Some(b) = o.context.body(&o.body) else { continue };
            if let Some(seed_owner) = &owner {
                let mut tmpl = target.clone();
                tmpl.instance = o.path[0];
                tmpl.path = o.path[1..].to_vec();
                for fi in 0..b.faces.len() {
                    if k == 0 && fi == seed_owner.face {
                        continue;
                    }
                    let Ok(face) = b.shape.sub_shape(k::ShapeType::Face, fi) else { continue };
                    if !face_signature(&face).is_some_and(|s| s.matches(&sig)) {
                        continue;
                    }
                    if let Some(r) = matching_end_edge(b, fi, seed_owner) {
                        let code = if k == 0 { stable_code(&r) } else { (stable_code(&r) ^ (o.path[0].0 as u32).wrapping_mul(0x9e37_79b9)) & 0x7fff_ffff };
                        candidates.push((tmpl.clone(), code, r));
                    }
                }
                continue;
            }
            let names: Vec<Reference> = match &entity {
                Reference::Face(_) => b.faces.iter().map(|f| Reference::Face((**f).clone())).collect(),
                _ => b.edges.iter().map(|e| Reference::Edge((**e).clone())).collect(),
            };
            let mut tmpl = target.clone();
            tmpl.instance = o.path[0];
            tmpl.path = o.path[1..].to_vec();
            for r in names {
                if k == 0 && r == entity {
                    continue;
                }
                if sig_in(b, &r).is_some_and(|s| s.matches(&sig)) {
                    // Codes stay unique per (instance, entity).
                    let code = if k == 0 { stable_code(&r) } else { (stable_code(&r) ^ (o.path[0].0 as u32).wrapping_mul(0x9e37_79b9)) & 0x7fff_ffff };
                    candidates.push((tmpl.clone(), code, r));
                }
            }
        }
        if candidates.is_empty() {
            return Err("Selected target did not contain identical geometry.".into());
        }
    }
    let seed_place = asm.instance(seed).map(|i| i.placement).unwrap_or_default();
    let seed_pose = result.solved.placement(seed).unwrap_or(seed_place);
    let target_world = result.connector_world(target, units);
    let mut out = Vec::new();
    for (tmpl, code, r) in candidates {
        let mut conn = tmpl;
        conn.reference = join_reference(r, inference);
        let Some(w) = result.connector_world(&conn, units) else { continue };
        let placement = target_world.map(|t| {
            let rel = w.mul(&t.inverse());
            super::eval::frame_to_placement(&rel.mul(&super::eval::placement_to_frame(&seed_pose)))
        });
        out.push((code, conn, placement));
    }
    out.sort_by_key(|(c, _, _)| *c);
    Ok(out)
}

/// One matched target: (stable code, target connector, initial placement of the copy).
pub type MatchedTarget = (u32, MateConnectorRef, Option<Placement>);

/// Part patterns and replicates whose matched targets no longer agree with
/// their generated instances (the target geometry changed: more or fewer
/// holes, a different Part Studio pattern count...): (pattern id, targets
/// to pass to [`Assembly::sync_matched_pattern`]).
pub fn stale_matched_patterns(result: &AssemblyResult, asm: &Assembly, units: &UnitPrefs) -> Vec<(super::MateId, Vec<MatchedTarget>)> {
    let mut out = Vec::new();
    for p in asm.patterns.iter().filter(|p| !p.suppressed && matches!(p.kind, PatternKind::PartPattern { .. } | PatternKind::Replicate { .. })) {
        // An empty match list (the target geometry is gone) removes all copies.
        let targets = matched_targets(result, asm, p, units).unwrap_or_default();
        let mut want: Vec<(u32, &MateConnectorRef)> = targets.iter().map(|(c, t, _)| (*c, t)).collect();
        let mut have: Vec<(u32, &MateConnectorRef)> = p.generated.iter().filter_map(|g| g.target.as_ref().map(|t| (g.index, t))).collect();
        want.sort_by_key(|x| x.0);
        have.sort_by_key(|x| x.0);
        let same = want.len() == have.len() && want.iter().zip(&have).all(|(a, b)| a.0 == b.0 && a.1.instance == b.1.instance && a.1.path == b.1.path && a.1.reference == b.1.reference);
        if !same {
            out.push((p.id, targets));
        }
    }
    out
}

// ------------------------------------------------------------ replace instance

/// Geometric description of a face / edge / vertex used to find its
/// counterpart on a replacement part.
#[derive(Clone, Copy, Debug)]
enum EntityGeom {
    Face { kind: u8, point: DVec3, normal: DVec3, size: f64 },
    Edge { kind: u8, point: DVec3, dir: DVec3, size: f64 },
    Vertex { point: DVec3 },
}

fn surface_kind(s: &k::Surface) -> u8 {
    match s {
        k::Surface::Plane(_) => 0,
        k::Surface::Cylinder { .. } => 1,
        k::Surface::Cone { .. } => 2,
        k::Surface::Sphere { .. } => 3,
        k::Surface::Torus { .. } => 4,
        _ => 9,
    }
}

fn face_geom(f: &k::Shape) -> Option<EntityGeom> {
    let kind = surface_kind(&f.face_surface().ok()?);
    let point = f.centroid().ok()?;
    let normal = match f.face_surface().ok()? {
        k::Surface::Cylinder { axis, .. } | k::Surface::Cone { axis, .. } | k::Surface::Torus { axis, .. } => axis.normalize(),
        _ => f.normal_at_point(point).ok()?.normalize_or_zero(),
    };
    let size = f.bounding_box(true).map(|b| b.diagonal()).unwrap_or(0.0);
    Some(EntityGeom::Face { kind, point, normal, size })
}

fn edge_geom(e: &k::Shape) -> Option<EntityGeom> {
    let info = e.edge_info().ok()?;
    let (kind, dir) = match info.curve {
        k::Curve::Line { direction, .. } => (0, direction.normalize()),
        k::Curve::Circle { axis, .. } => (1, axis.normalize()),
        _ => (9, (info.end - info.start).normalize_or_zero()),
    };
    let point = e.point_at((info.first_param + info.last_param) * 0.5).unwrap_or((info.start + info.end) * 0.5);
    Some(EntityGeom::Edge { kind, point, dir, size: info.length })
}

fn entity_geom(b: &crate::regen::context::Body, r: &Reference) -> Option<EntityGeom> {
    match r {
        Reference::Face(n) => face_geom(&b.shape.sub_shape(k::ShapeType::Face, *b.find_faces(n).first()?).ok()?),
        Reference::Edge(n) => edge_geom(&b.shape.sub_shape(k::ShapeType::Edge, *b.find_edges(n).first()?).ok()?),
        Reference::Vertex(n) => Some(EntityGeom::Vertex { point: b.shape.sub_shape(k::ShapeType::Vertex, *b.find_vertices(n).first()?).ok()?.vertex_point().ok()? }),
        _ => None,
    }
}

/// Mismatch score between two entity descriptions (None: different types).
fn geom_score(a: &EntityGeom, b: &EntityGeom) -> Option<f64> {
    match (a, b) {
        (EntityGeom::Face { kind: k1, point: p1, normal: n1, size: s1 }, EntityGeom::Face { kind: k2, point: p2, normal: n2, size: s2 }) if k1 == k2 => {
            let scale = s1.max(*s2).max(1e-6);
            let ndev = if *k1 == 0 { 1.0 - n1.dot(*n2) } else { 1.0 - n1.dot(*n2).abs() };
            Some(p1.distance(*p2) / scale + 2.0 * ndev + 0.25 * (s1 - s2).abs() / scale)
        }
        (EntityGeom::Edge { kind: k1, point: p1, dir: d1, size: s1 }, EntityGeom::Edge { kind: k2, point: p2, dir: d2, size: s2 }) if k1 == k2 => {
            let scale = s1.max(*s2).max(1e-6);
            Some(p1.distance(*p2) / scale + 2.0 * (1.0 - d1.dot(*d2).abs()) + 0.25 * (s1 - s2).abs() / scale)
        }
        (EntityGeom::Vertex { point: p1 }, EntityGeom::Vertex { point: p2 }) => Some(p1.distance(*p2)),
        _ => None,
    }
}

/// Counterpart of `entity` (named in `old`) on the body `new`: the same
/// name when it still exists there, else the entity of the same surface /
/// curve type with the nearest position and normal / direction.
pub fn match_entity(old: &crate::regen::context::Body, new: &crate::regen::context::Body, entity: &Reference) -> Option<Reference> {
    let resolves = |r: &Reference| match r {
        Reference::Face(n) => !new.find_faces(n).is_empty(),
        Reference::Edge(n) => !new.find_edges(n).is_empty(),
        Reference::Vertex(n) => !new.find_vertices(n).is_empty(),
        _ => true,
    };
    if resolves(entity) {
        return Some(entity.clone());
    }
    let g = entity_geom(old, entity)?;
    let candidates: Vec<(Reference, Option<EntityGeom>)> = match entity {
        Reference::Face(_) => new.faces.iter().enumerate().map(|(i, n)| (Reference::Face((**n).clone()), new.shape.sub_shape(k::ShapeType::Face, i).ok().and_then(|f| face_geom(&f)))).collect(),
        Reference::Edge(_) => new.edges.iter().enumerate().map(|(i, n)| (Reference::Edge((**n).clone()), new.shape.sub_shape(k::ShapeType::Edge, i).ok().and_then(|e| edge_geom(&e)))).collect(),
        Reference::Vertex(_) => new.vertices.iter().enumerate().map(|(i, n)| (Reference::Vertex((**n).clone()), new.shape.sub_shape(k::ShapeType::Vertex, i).ok().and_then(|v| v.vertex_point().ok()).map(|p| EntityGeom::Vertex { point: p }))).collect(),
        _ => return None,
    };
    candidates.into_iter().filter_map(|(r, cg)| geom_score(&g, &cg?).map(|s| (r, s))).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)).map(|(r, _)| r)
}

/// Remap a connector reference from the old part to the new one (see [`match_entity`]).
fn remap_reference(old: &crate::regen::context::Body, new: &crate::regen::context::Body, r: &Reference) -> Option<Reference> {
    match r {
        Reference::Implicit { on, inference } => match &**on {
            Reference::Face(_) | Reference::Edge(_) | Reference::Vertex(_) => match_entity(old, new, on).map(|e| Reference::Implicit { on: Box::new(e), inference: *inference }),
            _ => Some(r.clone()),
        },
        Reference::Face(_) | Reference::Edge(_) | Reference::Vertex(_) => match_entity(old, new, r),
        _ => Some(r.clone()),
    }
}

/// After replacing instances, re-point mate connectors (mates, assembly
/// connectors, pattern directions) on them whose geometry names do not exist
/// on the replacement part to geometrically matching entities. `old` is the
/// result evaluated before the replacement, `new_ctx` / `new_body` the
/// replacement part (`None` body: a rigid Part Studio, matched against its
/// first part). Returns the number of references remapped.
pub fn remap_replaced(asm: &mut Assembly, ids: &[InstanceId], old: &AssemblyResult, new_ctx: &crate::regen::context::Context, new_body: Option<&BodyId>) -> usize {
    let new_b = match new_body {
        Some(id) => new_ctx.body(id),
        None => new_ctx.bodies.first(),
    };
    let Some(new_b) = new_b.cloned() else { return 0 };
    let mut count = 0;
    let mut fix = |c: &mut MateConnectorRef| {
        if c.connector.is_some() || !c.path.is_empty() || !ids.contains(&c.instance) {
            return;
        }
        let Some(occ) = old.occurrences.iter().find(|o| o.path == [c.instance]) else { return };
        let Some(old_b) = occ.context.body(&occ.body) else { return };
        if let Some(r) = remap_reference(old_b, &new_b, &c.reference)
            && r != c.reference
        {
            c.reference = r;
            count += 1;
        }
    };
    for m in &mut asm.mates {
        fix(&mut m.a);
        fix(&mut m.b);
        for e in &mut m.extra {
            fix(e);
        }
    }
    for c in &mut asm.connectors {
        fix(&mut c.on);
    }
    for p in &mut asm.patterns {
        match &mut p.kind {
            PatternKind::Linear { direction, second, .. } => {
                fix(direction);
                if let Some(s) = second {
                    fix(&mut s.direction);
                }
            }
            PatternKind::Circular { axis, .. } => fix(axis),
            PatternKind::Mirror { plane, .. } => fix(plane),
            _ => {}
        }
    }
    count
}

/// Measure tool in an assembly: entities picked on occurrences (instance
/// path, face / edge / vertex), measured at their solved world positions —
/// see [`crate::regen::measure::measure_shapes`].
pub fn measure_entities(result: &AssemblyResult, picks: &[(InstancePath, Reference)]) -> Vec<(String, crate::regen::measure::Measurement)> {
    use crate::regen::measure::resolve_shapes;
    let shapes: Vec<Vec<k::Shape>> = picks
        .iter()
        .map(|(path, r)| {
            let Some(o) = result.occurrences.iter().find(|o| &o.path == path && o.context.body(&o.body).is_some_and(|b| entity_on_body(b, r))) else { return vec![] };
            let mut ctx = (*o.context).clone();
            ctx.bodies.retain(|b| b.id == o.body);
            resolve_shapes(&ctx, r).into_iter().filter_map(|s| s.transformed(&k::Transform::Matrix(o.transform)).ok()).collect()
        })
        .collect();
    // Onshape's measure rules, as in a Part Studio.
    crate::regen::measure::measure_shapes(&shapes)
}

/// Whether a picked entity still exists in `result` (its instance and the
/// entity on that instance's body): the selection drops picks that a
/// re-evaluation removed (an undone or deleted part).
pub fn pick_exists(result: &AssemblyResult, (path, r): &(InstancePath, Reference)) -> bool {
    result.occurrences.iter().any(|o| &o.path == path && o.context.body(&o.body).is_some_and(|b| entity_on_body(b, r)))
}

fn entity_on_body(b: &crate::regen::context::Body, r: &Reference) -> bool {
    match r {
        Reference::Face(n) => !b.find_faces(n).is_empty(),
        Reference::Edge(n) => !b.find_edges(n).is_empty(),
        Reference::Vertex(n) => !b.find_vertices(n).is_empty(),
        _ => true,
    }
}

/// Isolate › Expand (Onshape's Component controls): the top-level instances
/// in the order they join the isolated set, one step per slider notch.
/// By distance, one instance per step, nearest (bounding boxes) to the
/// isolated instances first; by connectivity, every instance mated (or
/// grouped) to the previous step's instances joins together.
pub fn isolate_expansion(result: &AssemblyResult, asm: &Assembly, seeds: &[InstanceId], connectivity: bool) -> Vec<Vec<InstanceId>> {
    let mut done: Vec<InstanceId> = seeds.to_vec();
    let others: Vec<InstanceId> = asm.instances.iter().filter(|i| !i.suppressed && !seeds.contains(&i.id)).map(|i| i.id).collect();
    let mut steps: Vec<Vec<InstanceId>> = Vec::new();
    if connectivity {
        let mut links: Vec<(InstanceId, InstanceId)> = asm.mates.iter().filter(|m| !m.suppressed).map(|m| (m.a.instance, m.b.instance)).collect();
        for g in asm.groups.iter().filter(|g| !g.suppressed) {
            for w in asm.group_members(g).windows(2) {
                links.push((w[0], w[1]));
            }
        }
        let mut frontier = done.clone();
        loop {
            let mut next: Vec<InstanceId> = Vec::new();
            for &(a, b) in &links {
                for (x, y) in [(a, b), (b, a)] {
                    if frontier.contains(&x) && others.contains(&y) && !done.contains(&y) && !next.contains(&y) {
                        next.push(y);
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            done.extend(next.iter().copied());
            frontier = next.clone();
            steps.push(next);
        }
    } else {
        let aabb_distance = |a: (DVec3, DVec3), b: (DVec3, DVec3)| (a.0 - b.1).max(b.0 - a.1).max(DVec3::ZERO).length();
        let seed_boxes: Vec<(DVec3, DVec3)> = seeds.iter().filter_map(|s| result.instance_bounds(*s)).collect();
        let mut ranked: Vec<(f64, usize, InstanceId)> = others
            .iter()
            .enumerate()
            .filter_map(|(i, id)| {
                let bb = result.instance_bounds(*id)?;
                let d = seed_boxes.iter().map(|s| aabb_distance(*s, bb)).fold(f64::MAX, f64::min);
                Some((d, i, *id))
            })
            .collect();
        ranked.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        steps = ranked.into_iter().map(|(_, _, id)| vec![id]).collect();
    }
    steps
}

/// Mass properties of an assembly's parts where they sit (Onshape's Mass
/// properties tool in an assembly): the parts of the instances `tops` (every
/// instance when empty), each part's volume properties moved by its
/// occurrence transform, combined about the joint centre of mass. `density`
/// gives a part's material density (kg/m³); `mass` is None when a part has
/// none (Onshape: "One or more parts do not have a material defined"), and
/// `without_material` lists those occurrences.
pub fn assembly_mass_properties(result: &AssemblyResult, tops: &[InstanceId], density: &dyn Fn(&PartOccurrence) -> Option<f64>) -> Option<AssemblyMass> {
    struct Part {
        mass: f64,
        center: DVec3,
        inertia: glam::DMat3,
    }
    let (mut volume, mut area) = (0.0, 0.0);
    let mut parts = Vec::new();
    let mut without_material = Vec::new();
    for o in result.occurrences.iter().filter(|o| tops.is_empty() || tops.contains(&o.top())) {
        if !matches!(o.kind, BodyKind::Solid) {
            continue;
        }
        let Some(body) = o.context.bodies.iter().find(|b| b.id == o.body) else { continue };
        let (Ok(p), Ok(s)) = (k::volume_properties(&body.shape), k::surface_properties(&body.shape)) else { continue };
        let rho = density(o);
        if rho.is_none() {
            without_material.push(o.path.clone());
        }
        let r = glam::DMat3::from_mat4(o.transform);
        volume += p.mass;
        area += s.mass;
        let rho = rho.unwrap_or(1.0);
        parts.push(Part { mass: p.mass * rho, center: o.transform.transform_point3(p.center), inertia: r * (p.inertia * rho) * r.transpose() });
    }
    if parts.is_empty() {
        return None;
    }
    let total: f64 = parts.iter().map(|p| p.mass).sum();
    let center = if total > 0.0 { parts.iter().map(|p| p.center * p.mass).sum::<DVec3>() / total } else { DVec3::ZERO };
    // Each part's inertia about the joint centre (parallel axis theorem).
    let inertia = parts.iter().fold(glam::DMat3::ZERO, |acc, p| {
        let d = p.center - center;
        acc + p.inertia + (glam::DMat3::from_diagonal(DVec3::splat(d.length_squared())) - glam::DMat3::from_cols(d * d.x, d * d.y, d * d.z)) * p.mass
    });
    Some(AssemblyMass {
        props: crate::regen::measure::MassProperties { volume, surface_area: area, density: None, mass: without_material.is_empty().then_some(total), center_of_mass: center, inertia },
        without_material,
    })
}

/// Mass moments of inertia moved to a reference frame (Mass and section
/// properties' "Mate connector for reference frame"; help: the moments are
/// taken about the connector instead of the centre of gravity): about the
/// frame's origin (parallel-axis theorem), in its axes.
pub fn inertia_about_frame(inertia_cm: glam::DMat3, mass: f64, center: DVec3, frame: &super::solver::Frame) -> glam::DMat3 {
    let d = center - frame.origin;
    let shifted = inertia_cm + (glam::DMat3::IDENTITY * d.length_squared() - glam::DMat3::from_cols(d * d.x, d * d.y, d * d.z)) * mass;
    let r = glam::DMat3::from_quat(frame.rotation);
    r.transpose() * shifted * r
}

/// Mass and section properties › Face tab (live behaviours "Assembly Mass
/// and section properties: Face tab"): area, perimeter, and the area
/// moments about the faces' centroid in the plane's own axes.
#[derive(Clone, Debug, PartialEq)]
pub struct FaceSection {
    pub area: f64,
    pub perimeter: f64,
    /// Centroid (world).
    pub centroid: DVec3,
    /// [[Lxx, Lxy], [Lyx, Lyy]] (m⁴); Lxx = ∫y² dA about the centroid's x axis.
    pub moments: [[f64; 2]; 2],
    /// Polar moment Lzz = Lxx + Lyy (m⁴).
    pub polar: f64,
}

/// The Face tab's values for faces of the assembly (path, face); None when
/// a face isn't planar or the faces aren't coplanar (live: blank values, a
/// red title, "Face selections are not coplanar").
pub fn face_section_properties(result: &AssemblyResult, faces: &[(InstancePath, Reference)]) -> Option<FaceSection> {
    if faces.is_empty() {
        return None;
    }
    // (world triangles, area, perimeter, plane origin, normal, x axis) per face.
    let mut tris: Vec<[DVec3; 3]> = Vec::new();
    let (mut area, mut perimeter) = (0.0, 0.0);
    let mut plane: Option<(DVec3, DVec3, DVec3)> = None;
    for (path, reference) in faces {
        let Reference::Face(name) = reference else { return None };
        let (o, fi) = result.occurrences.iter().find_map(|o| (&o.path == path).then(|| o.face_refs.iter().position(|r| r == reference).map(|fi| (o, fi))).flatten())?;
        let body = o.context.body(&o.body)?;
        let &kfi = body.find_faces(name).first()?;
        let face = body.shape.sub_shape(k::ShapeType::Face, kfi).ok()?;
        let k::Surface::Plane(pl) = face.face_surface().ok()? else { return None };
        let rot = |v: DVec3| o.transform.transform_vector3(v).normalize_or_zero();
        let (origin, normal, x) = (o.transform.transform_point3(pl.origin), rot(pl.normal), rot(pl.x_dir));
        match plane {
            None => plane = Some((origin, normal, x)),
            Some((o0, n0, _)) => {
                if n0.cross(normal).length() > 1e-6 || n0.dot(origin - o0).abs() > 1e-9 {
                    return None;
                }
            }
        }
        area += face.area().ok()?;
        perimeter += face.edges().iter().map(|e| e.length().unwrap_or(0.0)).sum::<f64>();
        let &(first, count) = o.mesh.face_ranges.get(fi)?;
        let idx = o.mesh.indices.get(first as usize..(first + count) as usize)?;
        let p = |i: u32| o.mesh.positions.get(i as usize).map(|q| o.transform.transform_point3(DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64)));
        for t in idx.chunks(3) {
            if let [a, b, c] = *t
                && let (Some(a), Some(b), Some(c)) = (p(a), p(b), p(c))
            {
                tris.push([a, b, c]);
            }
        }
    }
    let (_, normal, x) = plane?;
    let y = normal.cross(x).normalize_or_zero();
    // Centroid (area-weighted triangle centroids), then the second moments.
    let tri_area = |t: &[DVec3; 3]| (t[1] - t[0]).cross(t[2] - t[0]).length() * 0.5;
    let total: f64 = tris.iter().map(tri_area).sum();
    if total <= 0.0 {
        return None;
    }
    let centroid = tris.iter().map(|t| (t[0] + t[1] + t[2]) / 3.0 * tri_area(t)).sum::<DVec3>() / total;
    let (mut ixx, mut iyy, mut ixy) = (0.0, 0.0, 0.0);
    for t in &tris {
        let a = tri_area(t);
        let uv: Vec<(f64, f64)> = t.iter().map(|q| ((*q - centroid).dot(x), (*q - centroid).dot(y))).collect();
        let (u, v) = ([uv[0].0, uv[1].0, uv[2].0], [uv[0].1, uv[1].1, uv[2].1]);
        let sq = |w: [f64; 3]| w[0] * w[0] + w[1] * w[1] + w[2] * w[2] + w[0] * w[1] + w[1] * w[2] + w[2] * w[0];
        ixx += a / 6.0 * sq(v);
        iyy += a / 6.0 * sq(u);
        ixy += a / 12.0 * (2.0 * (u[0] * v[0] + u[1] * v[1] + u[2] * v[2]) + u[0] * v[1] + u[1] * v[0] + u[0] * v[2] + u[2] * v[0] + u[1] * v[2] + u[2] * v[1]);
    }
    Some(FaceSection { area, perimeter, centroid, moments: [[ixx, ixy], [ixy, iyy]], polar: ixx + iyy })
}

/// Result of [`assembly_mass_properties`].
#[derive(Clone, Debug)]
pub struct AssemblyMass {
    pub props: crate::regen::measure::MassProperties,
    pub without_material: Vec<InstancePath>,
}
