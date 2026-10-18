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

/// Default part colors (linear RGBA): Onshape's eight-color rotation
/// ("Customizing appearances" help page), in order.
pub const PART_COLORS: [[f32; 4]; 8] = [
    [0.254, 0.462, 0.624, 1.0], // light blue #8ab5cf
    [0.323, 0.323, 0.323, 1.0], // gray #9a9a9a
    [0.076, 0.159, 0.474, 1.0], // dark blue #4e6fb7
    [0.831, 0.831, 0.831, 1.0], // white #ebebeb
    [0.578, 0.791, 0.913, 1.0], // pale blue #c8e6f5
    [0.982, 0.332, 0.021, 1.0], // orange #fd9c28
    [0.188, 0.188, 0.188, 1.0], // dark gray #787878
    [0.831, 0.440, 0.007, 1.0], // yellow #ebb115
];

/// `NOSHAPE_PERF` set: log meshing timings at info level.
pub fn perf_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("NOSHAPE_PERF").is_ok_and(|v| v != "0" && !v.is_empty()))
}

fn f3(p: DVec3) -> [f32; 3] {
    [p.x as f32, p.y as f32, p.z as f32]
}

/// Build the display; returns whether any body only has a preview mesh.
pub fn build(ctx: &Context, studio: &PartStudio, opts: &RegenOptions, meshes: &mut HashMap<u64, (u64, bool, Arc<MeshBuffers>)>) -> (StudioDisplay, bool) {
    let mut needs_refine = false;
    let t_all = std::time::Instant::now();
    let mut d = StudioDisplay::defaults();
    let mut used = std::collections::HashSet::new();
    for b in &ctx.bodies {
        // Part tessellation quality and the view-wide quality (Shift+R)
        // scale the chord tolerance; finer meshes are cached separately.
        let quality = studio.part_properties.get(&b.id).and_then(|p| p.tessellation).map(|q| q.factor()).unwrap_or(1.0) * (opts.relative_deflection / RegenOptions::default().relative_deflection);
        let key = if (quality - 1.0).abs() < 1e-9 { b.shape.hash_code() } else { b.shape.hash_code() ^ quality.to_bits().wrapping_mul(0x9E37_79B9_7F4A_7C15) };
        let nfaces = b.faces.len() as u64;
        used.insert(key);
        let cached = meshes.get(&key).filter(|(n, preview, _)| *n == nfaces && (!*preview || opts.preview_meshes)).map(|(_, p, m)| (*p, m.clone()));
        let mesh = match cached {
            Some((preview, m)) => {
                needs_refine |= preview;
                m
            }
            _ => {
                let bb = b.shape.bounding_box(false).ok();
                let diag = bb.map(|x| x.diagonal()).unwrap_or(0.1).max(1e-4);
                // Preview meshes are ~4x coarser for instant feedback.
                let base = RegenOptions::default().relative_deflection * quality;
                let ang0 = if quality < 1.0 { opts.angular_deflection * quality.max(0.4) } else { opts.angular_deflection };
                let (rel, ang) = if opts.preview_meshes { (base * 4.0, (ang0 * 1.8).min(0.9)) } else { (base, ang0) };
                let lin = (diag * rel).max(1e-6);
                let tt = std::time::Instant::now();
                let tess = noshape_kernel::tessellate(&b.shape, lin, ang, true);
                if perf_enabled() {
                    log::info!(target: "perf", "tessellate {} ({}): {:.1} ms (linear {lin:.2e}, angular {ang:.2})", b.name, if opts.preview_meshes { "preview" } else { "fine" }, tt.elapsed().as_secs_f64() * 1000.0);
                } else {
                    log::debug!("tessellate {}: {:?} (linear {lin:.2e})", b.name, tt.elapsed());
                }
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
                meshes.insert(key, (nfaces, opts.preview_meshes, m.clone()));
                needs_refine |= opts.preview_meshes;
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
    if perf_enabled() {
        log::info!(target: "perf", "display: bodies {:.1} ms", t_all.elapsed().as_secs_f64() * 1000.0);
    } else {
        log::debug!("display: bodies {:?}", t_all.elapsed());
    }

    let t_sk = std::time::Instant::now();
    for s in &ctx.sketches {
        let mut sd = sketch_display(s, opts.editing_sketch == Some(s.feature));
        sd.hidden = studio.feature(s.feature).is_some_and(|f| f.hidden);
        sd.color = studio.appearances.iter().rev().find(|a| a.target == crate::partstudio::AppearanceTarget::Feature(s.feature)).map(|a| a.color);
        d.sketches.push(sd);
    }
    log::debug!("display: sketches {:?}", t_sk.elapsed());
    for p in &ctx.planes {
        let hidden = match &p.reference {
            Reference::FeaturePlane(f) => studio.feature(*f).is_some_and(|f| f.hidden),
            r => studio.hidden_defaults.contains(r),
        };
        d.planes.push(PlaneDisplay { reference: p.reference.clone(), name: p.name.clone(), frame: p.frame, half_width: p.half_width, half_height: p.half_height, hidden });
    }
    // Renamed default planes and planes sized with "Size to Part Studio".
    for p in &mut d.planes {
        if let Some((_, n)) = studio.default_names.iter().find(|(r, _)| *r == p.reference) {
            p.name = n.clone();
        }
        if let Some((_, w, h)) = studio.plane_sizes.iter().find(|(r, ..)| *r == p.reference) {
            (p.half_width, p.half_height) = (*w, *h);
        }
    }
    d.show_origin = !studio.hidden_defaults.contains(&Reference::Origin);
    for c in &ctx.connectors {
        if c.reference.depends_on().and_then(|f| studio.feature(f)).is_some_and(|f| f.hidden) {
            continue;
        }
        d.connectors.push(ConnectorDisplay { reference: c.reference.clone(), name: c.name.clone(), origin: c.origin, x: c.x, z: c.z });
    }
    // Onshape keeps the default planes at their default size whatever the
    // model's extent (they only grow with "Size to Part Studio"), so they are
    // not scaled to the model here.
    (d, needs_refine)
}

pub fn sketch_display(s: &super::context::SketchResult, editing: bool) -> SketchDisplay {
    let frame = s.frame;
    let size = {
        let bb = s.sketch.bbox();
        ((bb.max - bb.min).length()).max(0.01)
    };
    let tol = (size * 2.5e-4).max(1e-6);
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
    // Interior spline control points (not curve endpoints).
    let mut handles = std::collections::HashSet::new();
    let mut ends = std::collections::HashSet::new();
    for e in s.sketch.entities() {
        if let Some((a, b)) = e.endpoints() {
            ends.insert(a);
            ends.insert(b);
        }
        if let noshape_sketch::Geometry::BSpline { control, .. } = &e.geometry {
            handles.extend(control.iter().copied());
        }
    }
    for e in s.sketch.entities() {
        // Text glyph outlines are derived geometry: no points, regular color.
        let glyph = noshape_sketch::text::glyph_text_id(e.id).is_some();
        if e.kind() == EntityKind::Point {
            if glyph {
                continue;
            }
            let Some(p) = s.sketch.point(e.id) else { continue };
            let standalone = s.sketch.users_of_point(e.id).is_empty();
            let handle = handles.contains(&e.id) && !ends.contains(&e.id);
            points.push(SketchPointDisplay { entity: e.id.0, position: f3(frame.to_world(p)), state: state_of(e.id), standalone, handle });
        } else {
            let pts: Vec<[f32; 3]> = s.sketch.tessellate(e.id, tol).into_iter().map(|p| f3(frame.to_world(p))).collect();
            curves.push(SketchCurveDisplay { entity: e.id.0, points: pts, construction: e.construction, external: e.external && !glyph, state: state_of(e.id) });
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
    let images = s
        .images
        .iter()
        .filter_map(|i| {
            let pts: Vec<[f32; 3]> = i.box_points.iter().filter_map(|p| s.sketch.point(noshape_sketch::EntityId(*p))).map(|p| f3(frame.to_world(p))).collect();
            let corners: [[f32; 3]; 4] = pts.try_into().ok()?;
            Some(crate::display::SketchImageDisplay { id: i.id, corners, data_base64: i.data_base64.clone() })
        })
        .collect();
    SketchDisplay { feature: s.feature, name: s.name.clone(), frame, curves, points, regions, dimensions: Vec::new(), glyphs: Vec::new(), hidden: false, dof, color: None, images }
}

#[cfg(test)]
mod plane_override_tests {
    use crate::reference::{DatumPlane, Reference};
    use crate::regen::{RegenOptions, Regenerator};

    #[test]
    fn renamed_and_resized_default_planes() {
        let mut ps = crate::partstudio::PartStudio::new();
        let top = Reference::Plane(DatumPlane::Top);
        ps.default_names.push((top.clone(), "Base".into()));
        ps.plane_sizes.push((top.clone(), 0.2, 0.1));
        let res = Regenerator::new().regenerate(&ps, &Default::default(), &noshape_units::UnitPrefs::default(), &RegenOptions::default());
        let pl = res.display.planes.iter().find(|p| p.reference == top).expect("top plane");
        assert_eq!(pl.name, "Base");
        assert_eq!((pl.half_width, pl.half_height), (0.2, 0.1));
    }
}
