//! Assembly analysis: interference detection, bill of materials and
//! automatic exploded views, computed from an evaluated [`AssemblyResult`].

use super::eval::{AssemblyResult, InstancePath, PartOccurrence};
use super::{ExplodeStep, ExplodedView, InstanceId};
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
            steps.push(ExplodeStep { instances: vec![tops[i].0], translation: t.into(), rotation_axis: None, rotation_angle: 0.0 });
        }
    }
    ExplodedView { name: name.into(), steps }
}
