//! Assembly analysis: interference detection, bill of materials and
//! automatic exploded views, computed from an evaluated [`AssemblyResult`].

use super::eval::{AssemblyResult, InstancePath, PartOccurrence};
use super::{Assembly, ExplodeStep, ExplodedView, InstanceId, InstancePattern, MateConnectorRef, PatternKind, Placement};
use crate::reference::{Inference, Reference, Role, TopoName};
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
}

/// Flattened parts-only bill of materials grouped by source part and
/// configuration, in first-appearance order.
pub fn bom(result: &AssemblyResult) -> Vec<BomRow> {
    let mut rows: Vec<BomRow> = Vec::new();
    for o in result.occurrences.iter().filter(|o| matches!(o.kind, BodyKind::Solid | BodyKind::Surface)) {
        if let Some(r) = rows.iter_mut().find(|r| r.studio == o.studio && r.body == o.body && r.config == o.config) {
            r.quantity += 1;
            r.instances.push(o.path.clone());
        } else {
            rows.push(BomRow {
                item: rows.len() + 1,
                name: o.body_name.clone(),
                studio: o.studio,
                body: o.body.clone(),
                config: o.config.clone(),
                quantity: 1,
                instances: vec![o.path.clone()],
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

/// Targets of a part pattern / replicate: one connector per matching
/// geometry on the seed mate's target instance (code, target connector,
/// initial placement for the copy). The seed's own target is excluded.
pub fn matched_targets(result: &AssemblyResult, asm: &Assembly, pattern: &InstancePattern, units: &UnitPrefs) -> Result<Vec<(u32, MateConnectorRef, Option<Placement>)>, String> {
    let (mate_id, by_pattern, all_instances) = match pattern.kind {
        PatternKind::PartPattern { mate } => (mate, true, false),
        PatternKind::Replicate { mate, all_instances } => (mate, false, all_instances),
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
        let sig = sig_in(body, &entity).ok_or("Replicate needs a seed mate on a face or edge with analytic geometry")?;
        // The target occurrence, plus other top-level instances of the same part.
        let mut occs: Vec<&PartOccurrence> = vec![occ];
        if all_instances {
            occs.extend(result.occurrences.iter().filter(|o| o.path != occ.path && o.path.len() == occ.path.len() && o.studio == occ.studio && o.body == occ.body && o.config == occ.config && o.path[0] != seed));
        }
        for (k, o) in occs.iter().enumerate() {
            let Some(b) = o.context.body(&o.body) else { continue };
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
            return Err("No matching geometry found on the target instance".into());
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
