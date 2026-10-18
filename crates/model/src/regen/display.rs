//! Build the UI display model from a regeneration context.

use super::RegenOptions;
use super::context::Context;
use crate::display::*;
use crate::partstudio::PartStudio;
use crate::reference::Reference;
use glam::{DVec2, DVec3};
use noshape_sketch::{ConstraintState, EntityKind};
use std::collections::HashMap;
use std::sync::Arc;

/// Default part colors (linear RGBA).
pub const PART_COLORS: [[f32; 4]; 8] = [
    [0.479, 0.509, 0.584, 1.0],
    [0.346, 0.468, 0.672, 1.0],
    [0.672, 0.479, 0.346, 1.0],
    [0.392, 0.624, 0.425, 1.0],
    [0.624, 0.392, 0.571, 1.0],
    [0.672, 0.644, 0.346, 1.0],
    [0.346, 0.651, 0.672, 1.0],
    [0.584, 0.396, 0.396, 1.0],
];

fn f3(p: DVec3) -> [f32; 3] {
    [p.x as f32, p.y as f32, p.z as f32]
}

pub fn build(ctx: &Context, studio: &PartStudio, opts: &RegenOptions, meshes: &mut HashMap<u64, (u64, Arc<MeshBuffers>)>) -> StudioDisplay {
    let t_all = std::time::Instant::now();
    let mut d = StudioDisplay::defaults();
    let mut used = std::collections::HashSet::new();
    for b in &ctx.bodies {
        let key = b.shape.hash_code();
        let nfaces = b.faces.len() as u64;
        used.insert(key);
        let mesh = match meshes.get(&key) {
            Some((n, m)) if *n == nfaces => m.clone(),
            _ => {
                let bb = b.shape.bounding_box(false).ok();
                let diag = bb.map(|x| x.diagonal()).unwrap_or(0.1).max(1e-4);
                let lin = (diag * opts.relative_deflection).max(1e-6);
                let tt = std::time::Instant::now();
                let tess = noshape_kernel::tessellate(&b.shape, lin, opts.angular_deflection, true);
                log::debug!("tessellate {}: {:?} (linear {lin:.2e})", b.name, tt.elapsed());
                let m = match tess {
                    Ok(t) => Arc::new(MeshBuffers {
                        face_ranges: t.faces.iter().map(|f| (f.first_index, f.index_count)).collect(),
                        edges: (0..t.edges.len()).map(|i| t.edge_polyline(i).to_vec()).collect(),
                        positions: t.positions,
                        normals: t.normals,
                        indices: t.indices,
                        vertices: t.vertices,
                    }),
                    Err(e) => {
                        log::warn!("tessellation failed for {}: {e}", b.name);
                        Arc::new(MeshBuffers::default())
                    }
                };
                meshes.insert(key, (nfaces, m.clone()));
                m
            }
        };
        let props = studio.part_properties.get(&b.id);
        let color = props.and_then(|p| p.appearance).map(|a| a.color).unwrap_or(PART_COLORS[b.color % PART_COLORS.len()]);
        let bounds = b.shape.bounding_box(false).ok().map(|x| (x.min, x.max));
        d.bodies.push(BodyDisplay {
            id: b.id.clone(),
            name: props.and_then(|p| p.name.clone()).unwrap_or_else(|| b.name.clone()),
            kind: b.kind,
            color,
            hidden: props.is_some_and(|p| p.hidden),
            mesh,
            face_refs: b.faces.iter().map(|n| Reference::Face((**n).clone())).collect(),
            edge_refs: b.edges.iter().map(|n| Reference::Edge((**n).clone())).collect(),
            vertex_refs: b.vertices.iter().map(|n| Reference::Vertex((**n).clone())).collect(),
            bounds,
        });
    }
    meshes.retain(|k, _| used.contains(k));
    log::debug!("display: bodies {:?}", t_all.elapsed());

    let t_sk = std::time::Instant::now();
    for s in &ctx.sketches {
        d.sketches.push(sketch_display(s, opts.editing_sketch == Some(s.feature)));
    }
    log::debug!("display: sketches {:?}", t_sk.elapsed());
    for p in &ctx.planes {
        d.planes.push(PlaneDisplay { reference: p.reference.clone(), name: p.name.clone(), frame: p.frame, half_width: p.half_width, half_height: p.half_height, hidden: false });
    }
    for c in &ctx.connectors {
        d.connectors.push(ConnectorDisplay { reference: c.reference.clone(), name: c.name.clone(), origin: c.origin, x: c.x, z: c.z });
    }
    // Scale default planes to the model.
    if let Some((lo, hi)) = d.bounds() {
        let half = ((hi - lo).length() * 0.6).max(0.05);
        for p in d.planes.iter_mut().filter(|p| matches!(p.reference, Reference::Plane(_))) {
            p.half_width = half;
            p.half_height = half;
        }
    }
    d
}

pub fn sketch_display(s: &super::context::SketchResult, editing: bool) -> SketchDisplay {
    let frame = s.frame;
    let size = {
        let bb = s.sketch.bbox();
        ((bb.max - bb.min).length()).max(0.01)
    };
    let tol = (size * 1e-3).max(1e-6);
    let state_of = |id: noshape_sketch::EntityId| -> SketchEntityState {
        if !editing {
            return SketchEntityState::Inactive;
        }
        match s.report.state(id) {
            Some(ConstraintState::FullyConstrained) => SketchEntityState::Full,
            Some(ConstraintState::OverConstrained) => SketchEntityState::Over,
            _ => SketchEntityState::Under,
        }
    };
    let mut curves = Vec::new();
    let mut points = Vec::new();
    for e in s.sketch.entities() {
        if e.kind() == EntityKind::Point {
            let Some(p) = s.sketch.point(e.id) else { continue };
            let standalone = s.sketch.users_of_point(e.id).is_empty();
            points.push(SketchPointDisplay { entity: e.id.0, position: f3(frame.to_world(p)), state: state_of(e.id), standalone });
        } else {
            let pts: Vec<[f32; 3]> = s.sketch.tessellate(e.id, tol).into_iter().map(|p| f3(frame.to_world(p))).collect();
            curves.push(SketchCurveDisplay { entity: e.id.0, points: pts, construction: e.construction, external: e.external, state: state_of(e.id) });
        }
    }
    let mut regions = Vec::new();
    for r in &s.regions {
        let outer = r.outer.tessellate(tol);
        let mut flat: Vec<f64> = Vec::new();
        let mut holes_idx = Vec::new();
        let mut all2: Vec<DVec2> = Vec::new();
        for p in &outer {
            flat.extend([p.x, p.y]);
            all2.push(*p);
        }
        for h in &r.holes {
            holes_idx.push(all2.len());
            for p in h.tessellate(tol) {
                flat.extend([p.x, p.y]);
                all2.push(p);
            }
        }
        let tris = earcutr::earcut(&flat, &holes_idx, 2).unwrap_or_default();
        regions.push(SketchRegionDisplay {
            key: super::eval::encode_region_key(&r.id),
            positions: all2.iter().map(|p| f3(frame.to_world(*p) + frame.normal * 1e-6)).collect(),
            indices: tris.into_iter().map(|i| i as u32).collect(),
            area: r.area(),
        });
    }
    let dof = s.report.dof.unwrap_or(0);
    SketchDisplay { feature: s.feature, name: s.name.clone(), frame, curves, points, regions, dimensions: Vec::new(), glyphs: Vec::new(), hidden: false, dof }
}
