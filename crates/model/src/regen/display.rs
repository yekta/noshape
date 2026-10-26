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
/// With `NOSHAPE_DISPLAY_MESH=chord` ([`angle_display_mesh`] off) bodies
/// get the chord-based mesh from before live 459 (chord 8e-4 of the body's
/// diagonal, 0.35 rad; previews 4x the chord, 1.8x the angle).
fn display_deflection(shape: &noshape_kernel::Shape, quality: f64, preview: bool) -> (f64, f64) {
    let diag = shape.bounding_box(false).ok().map(|x| x.diagonal()).unwrap_or(0.1).max(1e-4);
    if !angle_display_mesh() {
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

/// 0: not decided yet, 1: chord based, 2: angle based.
static DISPLAY_MESH: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Whether display meshes are Onshape's angle-based ones (live 459, 144
/// segments per circle in Auto): yes unless `NOSHAPE_DISPLAY_MESH=chord`
/// (the coarser chord-based mesh from before, an escape hatch for slow
/// GPUs; docs/handoff/viewport-performance.md has the cost) at start;
/// [`set_angle_display_mesh`] after.
pub fn angle_display_mesh() -> bool {
    use std::sync::atomic::Ordering;
    match DISPLAY_MESH.load(Ordering::Relaxed) {
        0 => {
            let angle = std::env::var("NOSHAPE_DISPLAY_MESH").as_deref() != Ok("chord");
            DISPLAY_MESH.store(if angle { 2 } else { 1 }, Ordering::Relaxed);
            angle
        }
        m => m == 2,
    }
}

/// Mesh bodies the angle-based way (`true`) or the chord-based way from
/// now on ([`angle_display_mesh`]).
pub fn set_angle_display_mesh(angle: bool) {
    DISPLAY_MESH.store(if angle { 2 } else { 1 }, std::sync::atomic::Ordering::Relaxed);
}

/// Which of `shape`'s vertices Onshape's topology has (none at seam ends or
/// on full circles: [`noshape_kernel::Shape::shown_topology`]), in MapShapes
/// order. Display meshes and vertex references keep only those, together.
pub fn shown_vertices(shape: &noshape_kernel::Shape) -> Vec<bool> {
    shape.shown_topology().map(|t| t.vertices).unwrap_or_default()
}

/// `items` (one per vertex, MapShapes order) without the vertices Onshape
/// hasn't (all of them when `shown` is empty).
pub fn keep_shown<T>(items: impl IntoIterator<Item = T>, shown: &[bool]) -> Vec<T> {
    items.into_iter().enumerate().filter(|(i, _)| shown.get(*i).copied().unwrap_or(true)).map(|(_, x)| x).collect()
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
            vertices: keep_shown(t.vertices, &shown_vertices(shape)),
            mesh_lines: Vec::new(),
        }
        .with_mesh_lines(shape),
    )
}

/// Tessellates a body's `shape` for the display (`quality` scales Auto's
/// angle; preview meshes are Coarse).
/// Mesh of a failed feature's error geometry ([`FeatureState::error_mesh`]).
///
/// [`FeatureState::error_mesh`]: crate::display::FeatureState::error_mesh
pub(super) fn tessellate_error(shape: &noshape_kernel::Shape, opts: &RegenOptions) -> Arc<MeshBuffers> {
    tessellate_body(shape, "error geometry", 1.0, opts)
}

/// The Sheet metal flat view's display: the flat solid of `flat` (in the
/// flat view's place) as one solid body in `color`; its faces numbered, its
/// edges and vertices naming the body (the view only shows it).
pub fn flat_view_display(flat: &crate::sheetmetal::FlatPattern, color: [f32; 4]) -> Option<StudioDisplay> {
    let shape = flat.flat_view_solid()?;
    let mesh = tessellate_body(&shape, "flat pattern", 1.0, &RegenOptions::default());
    let id = crate::reference::BodyId { feature: crate::reference::FeatureId(0), index: 0 };
    let me = Reference::Body(id.clone());
    let body = BodyDisplay {
        id,
        name: "Flat pattern".into(),
        kind: BodyKind::Solid,
        color,
        hidden: false,
        // Each face its own name: the flat view outlines the face under the
        // pointer (live "ui (10-24 a)" (4a)).
        face_refs: (0..mesh.face_ranges.len()).map(|i| Reference::Face(crate::reference::TopoName::face(crate::reference::FeatureId(0), crate::reference::Role::New(i as u32)))).collect(),
        edge_refs: vec![me.clone(); mesh.edges.len()],
        vertex_refs: vec![me; mesh.vertices.len()],
        mesh,
        coarser: Vec::new(),
        levels_from: 0,
        bounds: shape.bounding_box(false).ok().map(|b| (b.min, b.max)),
        composite: None,
        is_mesh: false,
        face_colors: Vec::new(),
    };
    Some(StudioDisplay { bodies: vec![body], ..Default::default() })
}

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
                vertices: keep_shown(t.vertices, &shown_vertices(shape)),
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

/// The quality ([`TessQuality::factor`]) of each display level a body
/// gets, coarsest first, and the first one's index in
/// `TessQuality::LEVELS`. Onshape (live 10-23 a3) draws a part at its
/// Tessellation quality (Auto = Fine) whatever its size; only View in high
/// quality (Shift+R: a finer `relative_deflection`) refines it, view by
/// view, up to Very fine, so then it gets every level from its own up.
/// Preview meshes: Coarse, unless `meshes` still holds the body's own
/// levels from an earlier refine (the part didn't change: a dialog opened
/// or cancelled, undo); those are drawn as they are, so nothing is
/// re-meshed or refined. The chord-based mesh ([`angle_display_mesh`]
/// off) has one level, as before.
///
/// [`TessQuality::factor`]: crate::partstudio::TessQuality::factor
fn body_levels(studio: &PartStudio, b: &super::context::Body, opts: &RegenOptions, meshes: &HashMap<u64, (u64, bool, Arc<MeshBuffers>)>) -> (Vec<f64>, u8) {
    use crate::partstudio::TessQuality;
    let setting = studio.part_properties.get(&b.id).and_then(|p| p.tessellation);
    let view_scale = opts.relative_deflection / RegenOptions::default().relative_deflection;
    if !angle_display_mesh() {
        return (vec![setting.map(|q| q.factor()).unwrap_or(1.0) * view_scale], 0);
    }
    let own = setting.unwrap_or(TessQuality::Auto).level();
    let last = if view_scale < 1.0 { TessQuality::VeryFine.level() } else { own };
    let qualities: Vec<f64> = TessQuality::LEVELS[own..=last].iter().map(|q| q.factor()).collect();
    let refined = || qualities.iter().all(|&q| meshes.get(&world_key(&b.shape, q)).is_some_and(|(n, preview, _)| *n == b.faces.len() as u64 && !*preview));
    if opts.preview_meshes && !refined() {
        return (vec![TessQuality::Coarse.factor()], 0);
    }
    (qualities, own as u8)
}

/// Meshes `shape` at `qualities` (missing levels, coarsest first;
/// `with_finest`: the last one is the body's finest). BRepMesh keeps a
/// shape's stored triangulation unless the new tolerance is smaller, and
/// the shape may hold a finer one (from an earlier regen), so the coarser
/// levels are meshed, coarsest first, on a copy without triangulations;
/// the finest on the shape itself, as always.
fn tessellate_levels(shape: &noshape_kernel::Shape, name: &str, qualities: &[f64], with_finest: bool, opts: &RegenOptions) -> Vec<Arc<MeshBuffers>> {
    let (coarse, finest) = if with_finest { qualities.split_at(qualities.len().saturating_sub(1)) } else { (qualities, &[][..]) };
    let mut out = Vec::with_capacity(qualities.len());
    if !coarse.is_empty() {
        let copy = shape.transformed(&noshape_kernel::Transform::Translate(DVec3::ZERO)).ok();
        let s = copy.as_ref().unwrap_or(shape);
        out.extend(coarse.iter().map(|&q| tessellate_body(s, name, q, opts)));
    }
    out.extend(finest.iter().map(|&q| tessellate_body(shape, name, q, opts)));
    out
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
        /// The missing levels (cache key, quality), coarsest first.
        levels: Vec<(u64, f64)>,
        /// The last missing level is the body's finest.
        with_finest: bool,
        nfaces: u64,
        shape: noshape_kernel::Shape,
        name: String,
    }
    let mut jobs: Vec<Job> = Vec::new();
    let mut queued = std::collections::HashSet::new();
    let fresh = |m: &HashMap<u64, (u64, bool, Arc<MeshBuffers>)>, key: u64, nfaces: u64| m.get(&key).is_some_and(|(n, preview, _)| *n == nfaces && (!*preview || opts.preview_meshes));
    for b in ctx.bodies.iter().filter(|b| b.composite.is_none()) {
        let (qualities, _) = body_levels(studio, b, opts, meshes);
        let nfaces = b.faces.len() as u64;
        let partner = b.shape.partner_hash();
        let (keys, shape): (Vec<u64>, _) = if partner != 0 && partners.get(&partner).is_some_and(|&n| n > 1) {
            (qualities.iter().map(|&q| local_key(partner, q)).collect(), b.shape.unlocated().unwrap_or_else(|_| b.shape.clone()))
        } else {
            (qualities.iter().map(|&q| world_key(&b.shape, q)).collect(), b.shape.clone())
        };
        let have: Vec<bool> = keys.iter().map(|&k| fresh(meshes, k, nfaces)).collect();
        let Some(first) = have.iter().position(|h| !h) else { continue };
        if !queued.insert(keys[first]) {
            continue;
        }
        let levels = (first..keys.len()).filter(|&i| !have[i]).map(|i| (keys[i], qualities[i])).collect();
        jobs.push(Job { levels, with_finest: !have[keys.len() - 1], nfaces, shape, name: b.name.clone() });
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
            group
                .into_iter()
                .filter(|_| !opts.cancel.as_ref().is_some_and(super::RegenCancel::aborted))
                .flat_map(|j| {
                    let qualities: Vec<f64> = j.levels.iter().map(|l| l.1).collect();
                    let made = tessellate_levels(&j.shape, &j.name, &qualities, j.with_finest, opts);
                    j.levels.into_iter().zip(made).map(move |((key, _), m)| (key, j.nfaces, m))
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let mut keys = std::collections::HashSet::new();
    for (key, nfaces, m) in made {
        keys.insert(key);
        meshes.insert(key, (nfaces, opts.preview_meshes, m));
    }
    keys
}

/// A body's levels (coarsest first) as its finest and the coarser ones.
fn split_levels(mut levels: Vec<Arc<MeshBuffers>>) -> (Arc<MeshBuffers>, Vec<Arc<MeshBuffers>>) {
    let finest = levels.pop().unwrap_or_default();
    (finest, levels)
}

/// Build the display; returns whether any body only has a preview mesh.
/// Faces all planes and edges all lines (not a mesh body): its preview mesh
/// would be its final mesh.
fn planar_only(shape: &noshape_kernel::Shape) -> bool {
    use noshape_kernel::{Curve, Surface};
    !shape.is_mesh() && shape.faces().iter().all(|f| matches!(f.face_surface(), Ok(Surface::Plane(_)))) && shape.edges().iter().all(|e| matches!(e.edge_curve(), Ok(Curve::Line { .. })))
}

/// Meshes of bodies a run no longer shows kept for undo ([`build`]).
const KEEP_GONE_MESHES: usize = 32;

/// `owners`: each cached mesh's body shape and the run that last showed it
/// (the mesh keys are shape addresses: holding the shape keeps another one
/// from getting a kept mesh's key).
pub fn build(ctx: &Context, studio: &PartStudio, opts: &RegenOptions, meshes: &mut HashMap<u64, (u64, bool, Arc<MeshBuffers>)>, owners: &mut HashMap<u64, (u64, noshape_kernel::Shape)>, run: u64) -> (StudioDisplay, bool) {
    // Preview meshes are coarser only along curves: when every body is
    // planar with straight edges they would be the final ones, so the
    // preview pass meshes at the final quality and no refine pass follows
    // (fins x400: 1.2 s of meshing saved when opening).
    let final_opts;
    let opts = if opts.preview_meshes && !ctx.bodies.is_empty() && ctx.bodies.iter().all(|b| planar_only(&b.shape)) {
        final_opts = RegenOptions { preview_meshes: false, ..opts.clone() };
        &final_opts
    } else {
        opts
    };
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
        // The body's display levels (coarsest first); each level is cached
        // by its quality.
        let (qualities, levels_from) = body_levels(studio, b, opts, meshes);
        let keys: Vec<u64> = qualities.iter().map(|&q| world_key(&b.shape, q)).collect();
        let nfaces = b.faces.len() as u64;
        used.extend(keys.iter().copied());
        for &k in &keys {
            owners.insert(k, (run, b.shape.clone()));
        }
        let valid = |m: &HashMap<u64, (u64, bool, Arc<MeshBuffers>)>, key: u64| m.get(&key).is_some_and(|(n, preview, _)| *n == nfaces && (!*preview || opts.preview_meshes));
        // An open composite's members are drawn themselves.
        let open_composite = b.composite.as_ref().is_some_and(|c| !c.closed);
        // Rigidly moved pattern instances (one TShape): their unlocated
        // meshes, made once for all of them; their world meshes are made
        // from them. Whether each level was made just now.
        let partner = b.shape.partner_hash();
        let partner_levels = (partner != 0 && !open_composite && partners.get(&partner).is_some_and(|&n| n > 1)).then(|| {
            let lkeys: Vec<u64> = qualities.iter().map(|&q| local_key(partner, q)).collect();
            used.extend(lkeys.iter().copied());
            for &k in &lkeys {
                owners.insert(k, (run, b.shape.clone()));
            }
            // Made just now (in parallel): the copies' world meshes follow.
            let mut fresh: Vec<bool> = lkeys.iter().map(|k| meshed_here.contains(k)).collect();
            let have: Vec<bool> = lkeys.iter().map(|&k| valid(meshes, k)).collect();
            if let Some(first) = have.iter().position(|h| !h) {
                let missing: Vec<usize> = (first..lkeys.len()).filter(|&i| !have[i]).collect();
                let shape = b.shape.unlocated().unwrap_or_else(|_| b.shape.clone());
                let made = tessellate_levels(&shape, &b.name, &missing.iter().map(|&i| qualities[i]).collect::<Vec<_>>(), !have[lkeys.len() - 1], opts);
                for (&i, m) in missing.iter().zip(made) {
                    meshes.insert(lkeys[i], (nfaces, opts.preview_meshes, m));
                    fresh[i] = true;
                }
            }
            let local: Vec<Arc<MeshBuffers>> = lkeys.iter().map(|k| meshes[k].2.clone()).collect();
            (local, fresh)
        });
        let levels: Vec<Arc<MeshBuffers>> = if open_composite {
            vec![Arc::new(MeshBuffers::default())]
        } else {
            // A new shared mesh replaces world meshes made without it, so
            // every copy's world mesh matches what the GPU draws.
            let have: Vec<bool> = keys.iter().enumerate().map(|(i, &k)| !partner_levels.as_ref().is_some_and(|(_, fresh)| fresh[i]) && valid(meshes, k)).collect();
            match &partner_levels {
                // Pattern partners share the unlocated meshes, placed by
                // each copy's location.
                Some((local, _)) => {
                    for i in (0..keys.len()).filter(|&i| !have[i]) {
                        meshes.insert(keys[i], (nfaces, opts.preview_meshes, Arc::new(local[i].transformed(&b.shape.location()))));
                    }
                }
                None => {
                    if let Some(first) = have.iter().position(|h| !h) {
                        let missing: Vec<usize> = (first..keys.len()).filter(|&i| !have[i]).collect();
                        let made = tessellate_levels(&b.shape, &b.name, &missing.iter().map(|&i| qualities[i]).collect::<Vec<_>>(), !have[keys.len() - 1], opts);
                        for (&i, m) in missing.iter().zip(made) {
                            meshes.insert(keys[i], (nfaces, opts.preview_meshes, m));
                        }
                    }
                }
            }
            needs_refine |= keys.iter().any(|k| meshes[k].1);
            keys.iter().map(|k| meshes[k].2.clone()).collect()
        };
        let (mesh, coarser) = split_levels(levels);
        if let Some((local, _)) = partner_levels {
            let (local, local_coarser) = split_levels(local);
            d.shared_meshes.insert(b.id.clone(), SharedMesh { mesh: local, coarser: local_coarser, transform: b.shape.location(), world: mesh.clone() });
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
            coarser,
            levels_from,
            face_refs: b.faces.iter().map(|n| Reference::Face((**n).clone())).collect(),
            edge_refs: b.edges.iter().map(|n| Reference::Edge((**n).clone())).collect(),
            vertex_refs: keep_shown(b.vertices.iter().map(|n| Reference::Vertex((**n).clone())), &shown_vertices(&b.shape)),
            bounds,
            composite: b.composite.clone(),
            is_mesh: b.shape.is_mesh(),
            face_colors,
        });
    }
    // Meshes of bodies this run doesn't show (a deleted feature's result)
    // stay a few runs, so undoing the delete shows them without meshing.
    let mut gone: Vec<(u64, u64)> = owners.iter().filter(|(k, (r, _))| !used.contains(*k) && run - r <= super::KEEP_RUNS).map(|(k, (r, _))| (*r, *k)).collect();
    gone.sort_unstable_by_key(|g| std::cmp::Reverse(g.0));
    gone.truncate(KEEP_GONE_MESHES);
    let keep = |k: &u64| used.contains(k) || gone.iter().any(|(_, g)| g == k);
    meshes.retain(|k, _| keep(k));
    owners.retain(|k, _| keep(k));
    if perf_enabled() {
        log::info!(target: "perf", "display: bodies {:.1} ms", t_all.elapsed().as_secs_f64() * 1000.0);
    } else {
        log::debug!("display: bodies {:?}", t_all.elapsed());
    }

    let t_sk = std::time::Instant::now();
    for s in &ctx.sketches {
        let mut sd = sketch_display(s, opts.editing_sketch == Some(s.feature));
        let owner = ctx.sketch_copies.iter().find(|(_, c)| *c == s.feature).map_or(s.feature, |(p, _)| *p);
        sd.hidden = studio.feature(owner).is_some_and(|f| f.hidden);
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
        d.planes.push(PlaneDisplay { reference: p.reference.clone(), name: p.name.clone(), frame: p.frame, half_width: p.half_width, half_height: p.half_height, center: glam::DVec2::ZERO, hidden });
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
        if let Some((_, x, y)) = studio.plane_centers.iter().find(|(r, ..)| *r == p.reference) {
            p.center = glam::DVec2::new(*x, *y);
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
    // Points some other entity uses (one pass: asking per point was
    // quadratic, a third of a 300-entity sketch's drag frame).
    let mut used = std::collections::HashSet::new();
    for e in s.sketch.entities() {
        used.extend(e.point_refs().iter().copied().filter(|p| *p != e.id));
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
            let standalone = !used.contains(&e.id);
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

    /// A plane resized by a corner drag keeps its rectangle's centre.
    #[test]
    fn dragged_plane_rectangle() {
        let mut ps = crate::partstudio::PartStudio::new();
        let top = Reference::Plane(DatumPlane::Top);
        ps.plane_sizes.push((top.clone(), 0.08, 0.05));
        ps.plane_centers.push((top.clone(), 0.01, -0.02));
        let res = Regenerator::new().regenerate(&ps, &Default::default(), &noshape_units::UnitPrefs::default(), &RegenOptions::default());
        let pl = res.display.planes.iter().find(|p| p.reference == top).expect("top plane");
        assert_eq!(pl.center, glam::DVec2::new(0.01, -0.02));
        assert!((pl.corner(1.0, 1.0) - (pl.frame.origin + pl.frame.x * 0.09 + pl.frame.y * 0.03)).length() < 1e-12);
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
