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

/// Radial exploded view: every top-level instance moves away from the
/// assembly center by `factor` times its distance (min. a fraction of the size).
pub fn auto_exploded_view(result: &AssemblyResult, name: &str, factor: f64) -> ExplodedView {
    let Some((lo, hi)) = result.bounds() else { return ExplodedView { name: name.into(), steps: vec![] } };
    let center = (lo + hi) * 0.5;
    let size = (hi - lo).length().max(1e-3);
    let mut tops: Vec<InstanceId> = Vec::new();
    for o in &result.occurrences {
        if !tops.contains(&o.top()) {
            tops.push(o.top());
        }
    }
    let mut steps = Vec::new();
    for (n, id) in tops.iter().enumerate() {
        let mut acc: Option<(DVec3, DVec3)> = None;
        for o in result.occurrences.iter().filter(|o| o.top() == *id) {
            if let Some((a, b)) = o.world_bounds() {
                acc = Some(match acc {
                    None => (a, b),
                    Some((x, y)) => (x.min(a), y.max(b)),
                });
            }
        }
        let Some((a, b)) = acc else { continue };
        let c = (a + b) * 0.5;
        let mut d = c - center;
        if d.length() < size * 0.05 {
            // Centered instances move along +Z in a stack.
            d = DVec3::Z * size * 0.25 * (n as f64 + 1.0) / tops.len() as f64;
        }
        let t = d * factor;
        steps.push(ExplodeStep { instances: vec![*id], translation: t.into(), rotation_axis: None, rotation_angle: 0.0 });
    }
    ExplodedView { name: name.into(), steps }
}
