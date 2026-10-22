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

/// Default part colors (linear RGBA): Onshape's eight-color rotation, in
/// order (live: the Appearances swatches of ten new parts, dom/default-part-
/// colors.txt, and live's STEP export colours; it repeats from Part 9).
pub const PART_COLORS: [[f32; 4]; 8] = [
    [0.337, 0.624, 0.847, 1.0],  // light blue #9dcfed
    [0.376, 0.376, 0.376, 1.0],  // gray #a5a5a5
    [0.044, 0.120, 0.456, 1.0],  // dark blue #3b61b4
    [0.823, 0.823, 0.823, 1.0],  // white #eaeaea
    [0.552, 0.761, 0.896, 1.0],  // pale blue #c4e2f3
    [0.939, 0.242, 0.0003, 1.0], // orange #f88701
    [0.212, 0.212, 0.212, 1.0],  // dark gray #7f7f7f
    [0.956, 0.468, 0.0003, 1.0], // yellow #fab601
];

/// `NOSHAPE_PERF` set: log meshing timings at info level.
pub fn perf_enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("NOSHAPE_PERF").is_ok_and(|v| v != "0" && !v.is_empty()))
}

fn f3(p: DVec3) -> [f32; 3] {
    [p.x as f32, p.y as f32, p.z as f32]
}

/// Onshape's display mesh is angle based (live 459): Auto draws a full
/// circle with 144 segments whatever its size, `quality` scales that angle
/// (`TessQuality::factor`). The chord tolerance is the body's size (times
/// `quality`), so the angle alone decides; it still scales with `quality`
/// because BRepMesh keeps a shape's existing triangulation unless the new
/// chord tolerance is smaller (a finer quality must re-mesh). `preview`
/// meshes (instant feedback while a feature is edited) are Coarse.
///
/// `NOSHAPE_DISPLAY_MESH=chord` meshes as before live 459 (chord 8e-4 of
/// the body's diagonal, 0.35 rad; previews 4x the chord, 1.8x the angle),
/// to compare the cost (perf.yml `variants`).
fn display_deflection(shape: &noshape_kernel::Shape, quality: f64, preview: bool) -> (f64, f64) {
    let diag = shape.bounding_box(false).ok().map(|x| x.diagonal()).unwrap_or(0.1).max(1e-4);
    if chord_mesh() {
        let defaults = RegenOptions::default();
        let (lin, ang) = ((diag * defaults.relative_deflection * quality).max(1e-6), defaults.angular_deflection * quality.clamp(0.4, 1.0));
        return if preview { (lin * 4.0, (ang * 1.8).min(0.9)) } else { (lin, ang) };
    }
    let quality = if preview { quality.max(crate::partstudio::TessQuality::Coarse.factor()) } else { quality };
    // BRepMesh's angular deflection is between a segment and the tangent,
    // half of the turn per segment.
    let auto = 2.0 * std::f64::consts::TAU / f64::from(crate::partstudio::TessQuality::Auto.segments_per_circle());
    (diag * quality, auto * quality)
}

fn chord_mesh() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("NOSHAPE_DISPLAY_MESH").is_ok_and(|v| v == "chord"))
}

/// Tessellates one body shape at an angle factor relative to Auto (smaller
/// = finer, [`display_deflection`]), e.g. for an assembly instance's "Use
/// best available tessellation".
pub fn tessellate_shape(shape: &noshape_kernel::Shape, quality: f64) -> Option<MeshBuffers> {
    let (lin, ang) = display_deflection(shape, quality, false);
    let t = noshape_kernel::tessellate(shape, lin, ang, true).ok()?;
    Some(
        MeshBuffers {
            face_ranges: t.faces.iter().map(|f| (f.first_index, f.index_count)).collect(),
            edges: (0..t.edges.len()).map(|i| t.edge_polyline(i).to_vec()).collect(),
            positions: t.positions,
            normals: t.normals,
            indices: t.indices,
            vertices: t.vertices,
            mesh_lines: Vec::new(),
        }
        .with_mesh_lines(shape),
    )
}

/// Tessellates a body's `shape` for the display (`quality` scales Auto's
/// angle; preview meshes are Coarse).
fn tessellate_body(shape: &noshape_kernel::Shape, name: &str, quality: f64, opts: &RegenOptions) -> Arc<MeshBuffers> {
    let (lin, ang) = display_deflection(shape, quality, opts.preview_meshes);
    let tt = std::time::Instant::now();
    let tess = noshape_kernel::tessellate(shape, lin, ang, true);
    if perf_enabled() {
        log::info!(target: "perf", "tessellate {} ({}): {:.1} ms (linear {lin:.2e}, angular {ang:.2})", name, if opts.preview_meshes { "preview" } else { "fine" }, tt.elapsed().as_secs_f64() * 1000.0);
    } else {
        log::debug!("tessellate {}: {:?} (linear {lin:.2e})", name, tt.elapsed());
    }
    match tess {
        Ok(t) => Arc::new(
            MeshBuffers {
                face_ranges: t.faces.iter().map(|f| (f.first_index, f.index_count)).collect(),
                edges: (0..t.edges.len()).map(|i| t.edge_polyline(i).to_vec()).collect(),
                positions: t.positions,
                normals: t.normals,
                indices: t.indices,
                vertices: t.vertices,
                mesh_lines: Vec::new(),
            }
            .with_mesh_lines(shape),
        ),
        Err(e) => {
            log::warn!("tessellation failed for {}: {e}", name);
            Arc::new(MeshBuffers::default())
        }
    }
}

/// A body's tessellation quality: its part property and the view-wide
/// quality (Shift+R) scale the chord tolerance.
fn body_quality(studio: &PartStudio, b: &super::context::Body, opts: &RegenOptions) -> f64 {
    studio.part_properties.get(&b.id).and_then(|p| p.tessellation).map(|q| q.factor()).unwrap_or(1.0) * (opts.relative_deflection / RegenOptions::default().relative_deflection)
}

/// The mesh cache key of a body's world mesh (finer meshes are cached
/// separately).
fn world_key(shape: &noshape_kernel::Shape, quality: f64) -> u64 {
    if (quality - 1.0).abs() < 1e-9 { shape.hash_code() } else { shape.hash_code() ^ quality.to_bits().wrapping_mul(0x9E37_79B9_7F4A_7C15) }
}

/// The mesh cache key of pattern partners' shared (unlocated) mesh.
fn local_key(partner: u64, quality: f64) -> u64 {
    partner.rotate_left(17) ^ quality.to_bits().wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5348_4152_4544_0001
}

/// Meshes the bodies the cache misses before the display is built, in
/// parallel: BRepMesh only parallelizes within a shape (a threaded screw is
/// one big face), so each body gets its own thread. BRepMesh stores the
/// triangulation on the shape's faces and edges, so bodies sharing any
/// face or edge (a split part's cut face) are meshed one after another.
/// Pattern partners are meshed once, unlocated; composites (made of other
/// bodies' shapes) are left to the display loop. Returns the shared
/// meshes made here.
fn mesh_in_parallel(ctx: &Context, studio: &PartStudio, opts: &RegenOptions, partners: &HashMap<u64, usize>, meshes: &mut HashMap<u64, (u64, bool, Arc<MeshBuffers>)>) -> std::collections::HashSet<u64> {
    use rayon::prelude::*;
    struct Job {
        key: u64,
        nfaces: u64,
        shape: noshape_kernel::Shape,
        name: String,
        quality: f64,
    }
    let mut jobs: Vec<Job> = Vec::new();
    let mut queued = std::collections::HashSet::new();
    let fresh = |m: &HashMap<u64, (u64, bool, Arc<MeshBuffers>)>, key: u64, nfaces: u64| m.get(&key).is_some_and(|(n, preview, _)| *n == nfaces && (!*preview || opts.preview_meshes));
    for b in ctx.bodies.iter().filter(|b| b.composite.is_none()) {
        let quality = body_quality(studio, b, opts);
        let nfaces = b.faces.len() as u64;
        let partner = b.shape.partner_hash();
        let (key, shape) = if partner != 0 && partners.get(&partner).is_some_and(|&n| n > 1) {
            (local_key(partner, quality), b.shape.unlocated().unwrap_or_else(|_| b.shape.clone()))
        } else {
            (world_key(&b.shape, quality), b.shape.clone())
        };
        if !fresh(meshes, key, nfaces) && queued.insert(key) {
            jobs.push(Job { key, nfaces, shape, name: b.name.clone(), quality });
        }
    }
    if jobs.len() < 2 {
        return Default::default();
    }
    // Groups of jobs sharing a face or edge TShape (union-find).
    let mut parent: Vec<usize> = (0..jobs.len()).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut owner: HashMap<u64, usize> = HashMap::new();
    for (i, j) in jobs.iter().enumerate() {
        for sub in j.shape.faces().into_iter().chain(j.shape.edges()) {
            match owner.entry(sub.partner_hash()) {
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(i);
                }
                std::collections::hash_map::Entry::Occupied(e) => {
                    let (a, b) = (root(&mut parent, *e.get()), root(&mut parent, i));
                    parent[a] = b;
                }
            }
        }
    }
    let mut groups: HashMap<usize, Vec<Job>> = HashMap::new();
    for (i, j) in jobs.into_iter().enumerate() {
        groups.entry(root(&mut parent, i)).or_default().push(j);
    }
    let made: Vec<(u64, u64, Arc<MeshBuffers>)> = groups
        .into_values()
        .collect::<Vec<_>>()
        .into_par_iter()
        .flat_map_iter(|group| {
            group.into_iter().filter(|_| !opts.cancel.as_ref().is_some_and(super::RegenCancel::aborted)).map(|j| (j.key, j.nfaces, tessellate_body(&j.shape, &j.name, j.quality, opts))).collect::<Vec<_>>()
        })
        .collect();
    let mut keys = std::collections::HashSet::new();
    for (key, nfaces, m) in made {
        keys.insert(key);
        meshes.insert(key, (nfaces, opts.preview_meshes, m));
    }
    keys
}

/// Build the display; returns whether any body only has a preview mesh.
pub fn build(ctx: &Context, studio: &PartStudio, opts: &RegenOptions, meshes: &mut HashMap<u64, (u64, bool, Arc<MeshBuffers>)>) -> (StudioDisplay, bool) {
    let mut needs_refine = false;
    let t_all = std::time::Instant::now();
    let mut d = StudioDisplay::defaults();
    let mut used = std::collections::HashSet::new();
    // Bodies per TShape (pattern partners share one).
    let mut partners: HashMap<u64, usize> = HashMap::new();
    for b in &ctx.bodies {
        *partners.entry(b.shape.partner_hash()).or_default() += 1;
    }
    let meshed_here = mesh_in_parallel(ctx, studio, opts, &partners, meshes);
    for b in &ctx.bodies {
        // A newer edit made this run stale (meshes made so far stay cached).
        if opts.cancel.as_ref().is_some_and(super::RegenCancel::aborted) {
            return (d, needs_refine);
        }
        // Part tessellation quality and the view-wide quality (Shift+R)
        // scale the chord tolerance; finer meshes are cached separately.
        let quality = body_quality(studio, b, opts);
        let key = world_key(&b.shape, quality);
        let nfaces = b.faces.len() as u64;
        used.insert(key);
        // An open composite's members are drawn themselves.
        let open_composite = b.composite.as_ref().is_some_and(|c| !c.closed);
        // Rigidly moved pattern instances (one TShape): its unlocated mesh,
        // made once for all of them; their world meshes are made from it.
        let partner = b.shape.partner_hash();
        let mut fresh_partner_mesh = false;
        let partner_mesh = (partner != 0 && !open_composite && partners.get(&partner).is_some_and(|&n| n > 1)).then(|| {
            let local_key = local_key(partner, quality);
            used.insert(local_key);
            if let Some((_, _, m)) = meshes.get(&local_key).filter(|(n, preview, _)| *n == nfaces && (!*preview || opts.preview_meshes)) {
                // Made just now (in parallel): the copies' world meshes follow.
                fresh_partner_mesh = meshed_here.contains(&local_key);
                return m.clone();
            }
            let m = match b.shape.unlocated() {
                Ok(shape) => tessellate_body(&shape, &b.name, quality, opts),
                Err(_) => tessellate_body(&b.shape, &b.name, quality, opts),
            };
            meshes.insert(local_key, (nfaces, opts.preview_meshes, m.clone()));
            fresh_partner_mesh = true;
            m
        });
        // A new shared mesh replaces world meshes made without it, so every
        // copy's world mesh matches what the GPU draws.
        let cached = meshes.get(&key).filter(|(n, preview, _)| !fresh_partner_mesh && *n == nfaces && (!*preview || opts.preview_meshes)).map(|(_, p, m)| (*p, m.clone()));
        let mesh = match cached {
            _ if open_composite => Arc::new(MeshBuffers::default()),
            Some((preview, m)) => {
                needs_refine |= preview;
                m
            }
            _ => {
                // Pattern partners share one mesh of the unlocated shape,
                // placed by each copy's location.
                let m = match &partner_mesh {
                    Some(local) => Arc::new(local.transformed(&b.shape.location())),
                    None => tessellate_body(&b.shape, &b.name, quality, opts),
                };
                meshes.insert(key, (nfaces, opts.preview_meshes, m.clone()));
                needs_refine |= opts.preview_meshes;
                m
            }
        };
        if let Some(local) = partner_mesh {
            d.shared_meshes.insert(b.id.clone(), SharedMesh { mesh: local, transform: b.shape.location(), world: mesh.clone() });
        }
        let props = studio.part_properties.get(&b.id);
        let color = props
            .and_then(|p| p.appearance)
            .map(|a| a.color)
            .or_else(|| ctx.part_appearance.get(&b.id).copied())
            .unwrap_or(PART_COLORS[b.color % PART_COLORS.len()]);
        // Faces a feature colored (an import's face colours).
        let face_colors: Vec<Option<[f32; 4]>> = if ctx.face_appearance.is_empty() {
            Vec::new()
        } else {
            b.faces.iter().map(|n| ctx.face_appearance.iter().rev().find(|(id, f, _)| *id == b.id && n.descends_from(f)).map(|(_, _, c)| *c)).collect()
        };
        let face_colors = if face_colors.iter().any(Option::is_some) { face_colors } else { Vec::new() };
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
            composite: b.composite.clone(),
            is_mesh: b.shape.is_mesh(),
            face_colors,
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
    d.derived_sketches = ctx.derived_sketches.clone();
    for p in &ctx.planes {
        let hidden = match &p.reference {
            Reference::FeaturePlane(f) => studio.feature(*f).is_some_and(|f| f.hidden),
            r => studio.hidden_defaults.contains(r),
        };
        d.planes.push(PlaneDisplay { reference: p.reference.clone(), name: p.name.clone(), frame: p.frame, half_width: p.half_width, half_height: p.half_height, hidden });
    }
    // Renamed default planes and planes sized with "Size to Part Studio";
    // hidden default planes (they come from `StudioDisplay::defaults`, not
    // `ctx.planes`: Hide on Top/Front/Right left them drawn).
    for p in &mut d.planes {
        if matches!(p.reference, Reference::Plane(_)) {
            p.hidden = studio.hidden_defaults.contains(&p.reference);
        }
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
        // References of dimensions / constraints to model geometry are not
        // drawn (live: no sketch geometry; qa5 SK3).
        if e.reference_only {
            continue;
        }
        // Text glyph outlines are derived geometry: no points, regular color.
        let glyph = noshape_sketch::text::glyph_text_id(e.id).is_some();
        if e.kind() == EntityKind::Point {
            if glyph {
                continue;
            }
            let Some(p) = s.sketch.point(e.id) else { continue };
            let standalone = s.sketch.users_of_point(e.id).is_empty();
            let handle = handles.contains(&e.id) && !ends.contains(&e.id);
            points.push(SketchPointDisplay { entity: e.id.0, position: f3(frame.to_world(p)), state: state_of(e.id), standalone, handle, external: e.external });
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

    /// User bug (v0.4.0): Hide on a default plane left it drawn.
    #[test]
    fn hidden_default_planes_and_origin() {
        let mut ps = crate::partstudio::PartStudio::new();
        ps.hidden_defaults = vec![Reference::Plane(DatumPlane::Front), Reference::Origin];
        let res = Regenerator::new().regenerate(&ps, &Default::default(), &noshape_units::UnitPrefs::default(), &RegenOptions::default());
        let hidden = |d: DatumPlane| res.display.planes.iter().find(|p| p.reference == Reference::Plane(d)).is_some_and(|p| p.hidden);
        assert!(hidden(DatumPlane::Front));
        assert!(!hidden(DatumPlane::Top));
        assert!(!res.display.show_origin);
    }
}
