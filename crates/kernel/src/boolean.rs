//! Booleans, split, imprint, unify, sections and curve projection.

use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::construct::{edges_to_wires, make_plane_face, require_nonempty};
use crate::error::Result;
use crate::ffi::ffi;
use crate::geom::{Plane, p3};
use crate::history::OpResult;
use crate::shape::{Shape, ck, vec_to_list};

/// Boolean operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BooleanOp {
    /// Union.
    Fuse,
    /// Subtract tools from objects.
    Cut,
    /// Intersection.
    Common,
}

/// Gluing option of the OCCT boolean engine (speeds up shapes that only touch
/// on coincident faces).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Glue {
    #[default]
    Off,
    Shift,
    Full,
    /// GlueShift where it is right: kept only when the fused volume is the
    /// sum of the inputs' (they merely touch), else the boolean runs again
    /// without glue. For patterns, whose instances usually stand on a face.
    ShiftChecked,
}

/// Options for [`boolean`] and [`split`].
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BooleanOptions {
    /// Fuzzy tolerance (0 = exact).
    pub fuzzy: f64,
    pub glue: Glue,
    /// Post-process with ShapeUpgrade_UnifySameDomain (merges coplanar /
    /// co-cylindrical faces and collinear edges). The history is composed
    /// through the unification, so merged faces show up as `Modified` from
    /// every contributing input face.
    pub unify: bool,
    /// Run the boolean in parallel.
    pub parallel: bool,
    /// Oriented bounding boxes for the interference pre-tests.
    #[serde(default)]
    pub obb: bool,
}

impl Default for BooleanOptions {
    fn default() -> Self {
        BooleanOptions { fuzzy: 0.0, glue: Glue::Off, unify: false, parallel: true, obb: false }
    }
}

impl BooleanOptions {
    fn ffi(&self) -> ffi::BoolOpts {
        ffi::BoolOpts {
            fuzzy: self.fuzzy,
            glue: match self.glue {
                Glue::Off => 0,
                Glue::Shift => 1,
                Glue::Full => 2,
                Glue::ShiftChecked => 3,
            },
            unify: self.unify,
            parallel: self.parallel,
            obb: self.obb,
        }
    }
}

/// Boolean of several objects with several tools
/// (BRepAlgoAPI_Fuse/Cut/Common with SetArguments/SetTools, non-destructive).
/// History args: objects in order, then tools.
pub fn boolean(op: BooleanOp, objects: &[Shape], tools: &[Shape], opts: &BooleanOptions) -> Result<OpResult> {
    require_nonempty(objects, "boolean objects")?;
    require_nonempty(tools, "boolean tools")?;
    let code = match op {
        BooleanOp::Fuse => 0,
        BooleanOp::Cut => 1,
        BooleanOp::Common => 2,
    };
    let a = vec_to_list(objects);
    let t = vec_to_list(tools);
    // The memory budget (NOSHAPE_BOOLEAN_MEMORY_MB, MB of growth; the kernel's
    // default 4096 otherwise): read here, where the process environment is
    // current on every platform.
    ffi::set_memory_budget_mb(std::env::var("NOSHAPE_BOOLEAN_MEMORY_MB").ok().and_then(|v| v.trim().parse().ok()).unwrap_or(0));
    // OCCT's booleans in parallel (NOSHAPE_BOP_PARALLEL; serial by default):
    // likewise.
    // OCCT patch 0004's pair intersections on the thread pool (on;
    // NOSHAPE_BOP_PAIRS_PARALLEL=0: off; always serial inside a CPU budget).
    ffi::set_bop_pairs_parallel(std::env::var("NOSHAPE_BOP_PAIRS_PARALLEL").map_or(true, |v| v.trim() != "0"));
    ffi::set_bop_parallel(match PARALLEL_OVERRIDE.load(std::sync::atomic::Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => std::env::var_os("NOSHAPE_BOP_PARALLEL").is_some(),
    });
    // NOSHAPE_OCCT_THREADS=n: OCCT's thread pool size (scaling and
    // determinism runs, BOOLEAN-SPEEDUP phase 0).
    static THREADS: std::sync::Once = std::sync::Once::new();
    THREADS.call_once(|| {
        if let Some(n) = std::env::var("NOSHAPE_OCCT_THREADS").ok().and_then(|v| v.trim().parse::<i32>().ok()) {
            ffi::set_occt_threads(n);
        }
    });
    let run = || Ok(OpResult::from_ffi(ck(ffi::op_boolean(code, &a, &t, &opts.ffi()))?));
    let res = match crate::bool_corpus::recording() {
        Some(dir) => crate::bool_corpus::record(&dir, op, objects, tools, opts, run),
        None => run(),
    }?;
    // A fuse of solids that comes back without a solid (OCCT on a tool that
    // only touches the part: a face blend's curvature filler on the elliptic
    // boss, live 11850.416 mm³): again with OCCT's glue, kept only when the
    // inputs merely touched, i.e. the volumes add up.
    let solids = |l: &[Shape]| l.iter().all(|x| x.count(crate::ShapeType::Solid) > 0);
    if op == BooleanOp::Fuse && opts.glue == Glue::Off && res.shape.count(crate::ShapeType::Solid) == 0 && solids(objects) && solids(tools) {
        let glued = BooleanOptions { glue: Glue::Shift, ..*opts };
        if let Ok(g) = ck(ffi::op_boolean(code, &a, &t, &glued.ffi())).map(OpResult::from_ffi) {
            let sum: f64 = objects.iter().chain(tools).filter_map(|x| x.volume().ok()).sum();
            if g.shape.count(crate::ShapeType::Solid) > 0 && g.shape.volume().is_ok_and(|v| (v - sum).abs() <= 1e-6 * sum.abs()) {
                return Ok(g);
            }
        }
    }
    Ok(res)
}

/// Runs `f` with this thread's [`section`]s limited to `seconds` of CPU
/// each (they fail with "section took too long" past it): for checks that
/// may give up (std's collision detection in editing logic).
pub fn with_call_budget<T>(seconds: f64, f: impl FnOnce() -> T) -> T {
    struct Restore(f64);
    impl Drop for Restore {
        fn drop(&mut self) {
            ffi::set_call_budget(self.0);
        }
    }
    let _restore = Restore(ffi::set_call_budget(seconds));
    f()
}

/// 0: NOSHAPE_BOP_PARALLEL decides; 1: serial; 2: parallel.
static PARALLEL_OVERRIDE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// OCCT's Booleans in parallel (`Some(true)`), serial (`Some(false)`) or as
/// NOSHAPE_BOP_PARALLEL says (`None`), for the whole process (tests comparing
/// both).
pub fn set_parallel_booleans(on: Option<bool>) {
    PARALLEL_OVERRIDE.store(on.map_or(0, |v| if v { 2 } else { 1 }), std::sync::atomic::Ordering::Relaxed);
}

/// The way this thread's last [`boolean`] went: "occt", or the fast path that
/// built it ("on_face", "through_holes", "fuse_slabs", "cut_slabs",
/// "trimmed", "by_faces").
pub fn last_boolean_path() -> String {
    ffi::last_boolean_path()
}

/// With NOSHAPE_KERNEL_PROFILE set: prints the kernel calls made since the
/// last mark under `label` (one feature's) and starts counting again.
pub fn kernel_profile_mark(label: &str) {
    ffi::kernel_profile_mark(label)
}

/// Run `f` with `flag` as this thread's boolean cancel flag: once it is set,
/// a running [`boolean`] stops inside OCCT and fails with "cancelled" (a
/// stale regeneration releases its thread quickly).
pub fn with_cancel<R>(flag: &std::sync::atomic::AtomicBool, f: impl FnOnce() -> R) -> R {
    thread_local!(static CURRENT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) });
    // Restores the previous flag, also when `f` panics.
    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            CURRENT.with(|c| c.set(self.0));
            ffi::set_thread_cancel(self.0);
        }
    }
    let addr = flag.as_ptr() as usize;
    let _restore = Restore(CURRENT.with(|c| c.replace(addr)));
    ffi::set_thread_cancel(addr);
    f()
}

/// `a ∪ b`.
pub fn fuse(a: &Shape, b: &Shape) -> Result<OpResult> {
    boolean(BooleanOp::Fuse, std::slice::from_ref(a), std::slice::from_ref(b), &BooleanOptions::default())
}

/// `a − b`.
pub fn cut(a: &Shape, b: &Shape) -> Result<OpResult> {
    boolean(BooleanOp::Cut, std::slice::from_ref(a), std::slice::from_ref(b), &BooleanOptions::default())
}

/// `a ∩ b`.
pub fn common(a: &Shape, b: &Shape) -> Result<OpResult> {
    boolean(BooleanOp::Common, std::slice::from_ref(a), std::slice::from_ref(b), &BooleanOptions::default())
}

/// Fuse all shapes together (first is the object, the rest are tools).
pub fn fuse_all(shapes: &[Shape], opts: &BooleanOptions) -> Result<OpResult> {
    require_nonempty(shapes, "shapes")?;
    if shapes.len() == 1 {
        return Ok(OpResult { shape: shapes[0].clone(), history: Default::default() });
    }
    boolean(BooleanOp::Fuse, &shapes[..1], &shapes[1..], opts)
}

/// Split `objects` by `tools` (faces, shells, solids, planes as faces, and
/// edges/wires to imprint curves on faces) with BRepAlgoAPI_Splitter. The
/// result is a compound of all pieces. History args: objects, then tools.
pub fn split(objects: &[Shape], tools: &[Shape], opts: &BooleanOptions) -> Result<OpResult> {
    require_nonempty(objects, "split objects")?;
    require_nonempty(tools, "split tools")?;
    let a = vec_to_list(objects);
    let t = vec_to_list(tools);
    Ok(OpResult::from_ffi(ck(ffi::op_split(&a, &t, &opts.ffi()))?))
}

/// Split a shape by an (unbounded) plane. History args: 0 = shape, 1 = the
/// internal plane face.
pub fn split_by_plane(shape: &Shape, plane: &Plane) -> Result<OpResult> {
    let bb = shape.bounding_box(false)?;
    let d = bb.diagonal() + 1.0;
    let c = plane.to_local(bb.center());
    let face = make_plane_face(plane, c.x - d, c.x + d, c.y - d, c.y + d)?;
    split(std::slice::from_ref(shape), &[face], &BooleanOptions::default())
}

/// Imprint (split faces by) edges or wires lying on the shape's faces.
pub fn imprint(shape: &Shape, curves: &[Shape]) -> Result<OpResult> {
    split(std::slice::from_ref(shape), curves, &BooleanOptions::default())
}

/// Merge same-domain faces/edges (ShapeUpgrade_UnifySameDomain) with history.
pub fn unify_same_domain(shape: &Shape) -> Result<OpResult> {
    Ok(OpResult::from_ffi(ck(ffi::op_unify(shape.raw()))?))
}

/// [`unify_same_domain`] over the whole shape, whatever is known of it (a
/// shape an earlier unify found nothing in, a local Boolean's result:
/// boolean.cpp `UnifyScope`): the reference the fast ways are checked
/// against.
pub fn unify_same_domain_whole(shape: &Shape) -> Result<OpResult> {
    Ok(OpResult::from_ffi(ck(ffi::op_unify_whole(shape.raw()))?))
}

/// Which way this thread's last [`unify_same_domain`] went: "known" (an
/// earlier unify found nothing to merge in the shape), "scoped" (a local
/// Boolean's result: only where it changed the part), "whole".
pub fn last_unify_path() -> String {
    ffi::last_unify_path()
}

/// Intersection curves of two shapes (BRepAlgoAPI_Section). History args:
/// 0 = a, 1 = b; faces `Generate` the section edges lying on them.
pub fn section(a: &Shape, b: &Shape) -> Result<OpResult> {
    Ok(OpResult::from_ffi(ck(ffi::op_section(a.raw(), b.raw()))?))
}

/// Planar cross-section: a compound of section edges.
pub fn section_plane(shape: &Shape, plane: &Plane) -> Result<Shape> {
    Shape::from_res(ffi::section_plane(shape.raw(), &plane.ffi()))
}

/// Planar cross-section connected into wires.
pub fn section_wires(shape: &Shape, plane: &Plane) -> Result<Vec<Shape>> {
    let edges = section_plane(shape, plane)?;
    edges_to_wires(&edges, 1e-6)
}

/// Cap faces of a planar cut through a solid (the solid ∩ plane region, with
/// holes), for section views.
pub fn section_caps(shape: &Shape, plane: &Plane) -> Result<Shape> {
    Shape::from_res(ffi::section_caps(shape.raw(), &plane.ffi()))
}

/// Project curves (edges/wires) onto a shape: along `direction` if given
/// (BRepProj_Projection), else along the target surface normals
/// (BRepOffsetAPI_NormalProjection). Returns a compound of projected edges.
pub fn project_curves(curves: &Shape, target: &Shape, direction: Option<DVec3>) -> Result<Shape> {
    match direction {
        Some(d) => Shape::from_res(ffi::project_along(curves.raw(), target.raw(), p3(d))),
        None => Shape::from_res(ffi::project_normal(curves.raw(), target.raw())),
    }
}
