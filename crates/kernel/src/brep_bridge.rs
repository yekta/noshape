//! noshape's own Boolean (crates/brep) for the C++ side: `op_boolean` converts its inputs to flat
//! [`ffi::BrepBody`]s (cpp/rust_boolean.cpp), calls [`brep_boolean`], and builds the result's shapes and
//! history from the [`ffi::BrepResult`].

use crate::ffi::ffi::{self as raw, BrepBody, BrepResult, OcShapeList, OpOut};
use crate::{bool_corpus, BooleanOp, BooleanOptions, OpResult, Shape};
use noshape_brep::{Body, Curve, Edge, Face, Kind, Op, Origin, Relation, Solid, Surface, Use};
use std::cell::{Cell, RefCell};
use std::sync::OnceLock;

thread_local! {
    /// This thread's switch: None as NOSHAPE_BOOL_RUST says.
    static MODE: Cell<Option<bool>> = const { Cell::new(None) };
    /// A decline fails the Boolean instead of running OCCT (the replay's quick mode).
    static STRICT: Cell<bool> = const { Cell::new(false) };
    /// Every Boolean as if its caller merged faces after (the replay of Shell's direct ones).
    static MERGED_AFTER: Cell<bool> = const { Cell::new(false) };
}

/// This thread's Booleans check their results as Shell's direct ones do (`noshape_brep::merge_check`): for
/// the replay of those.
pub fn set_rust_boolean_merged_after(on: bool) {
    MERGED_AFTER.with(|m| m.set(on));
}

/// With `on`, this thread's Booleans run noshape's own Boolean only: one it declines fails
/// (`last_rust_decline` says why) instead of running OCCT.
pub fn set_rust_boolean_only(on: bool) {
    STRICT.with(|s| s.set(on));
    set_rust_boolean(on.then_some(true));
}

pub(crate) fn brep_strict() -> bool {
    STRICT.with(|s| s.get())
}

/// Turns noshape's own Boolean on or off for this thread's Booleans (`None`: as NOSHAPE_BOOL_RUST says,
/// on unless 0); the corpus replay compares it with OCCT.
pub fn set_rust_boolean(on: Option<bool>) {
    MODE.with(|m| m.set(on));
    brep_decline("not tried (an earlier path took it)");
}

pub(crate) fn brep_enabled() -> bool {
    static ENV: OnceLock<bool> = OnceLock::new();
    MODE.with(|m| m.get()).unwrap_or_else(|| *ENV.get_or_init(|| std::env::var("NOSHAPE_BOOL_RUST").as_deref() != Ok("0")))
}

thread_local! {
    static LAST_DECLINE: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Why this thread's last call of noshape's own Boolean declined ("" when it didn't).
pub fn last_rust_decline() -> String {
    LAST_DECLINE.with(|c| c.borrow().clone())
}

pub(crate) fn brep_decline(reason: &str) {
    LAST_DECLINE.with(|c| *c.borrow_mut() = reason.to_string());
}

fn v3(s: &[f64], i: usize) -> glam::DVec3 {
    glam::DVec3::new(s[i], s[i + 1], s[i + 2])
}

fn body_of(b: &BrepBody) -> Body {
    let mut out = Body { vertices: (0..b.vertices.len() / 3).map(|i| v3(&b.vertices, 3 * i)).collect(), ..Default::default() };
    out.edges = b
        .edges
        .chunks(2)
        .zip(b.curves.chunks(11))
        .map(|(e, c)| {
            let curve = match c[0] as u8 {
                0 => Curve::Line,
                1 => Curve::Circle { center: v3(c, 1), normal: v3(c, 4), xdir: v3(c, 7), radius: c[10] },
                _ => Curve::Degenerate,
            };
            Edge { v: [e[0], e[1]], curve }
        })
        .collect();
    for f in 0..b.reversed.len() {
        let loops = (b.face_loops[f]..b.face_loops[f + 1])
            .map(|l| b.uses[b.loop_uses[l as usize] as usize..b.loop_uses[l as usize + 1] as usize].iter().map(|&u| Use { edge: u / 2, reversed: u % 2 == 1 }).collect())
            .collect();
        let s = &b.surfaces[12 * f..12 * f + 12];
        let surface = match s[0] as u8 {
            0 => Surface::Plane { origin: v3(s, 1), normal: v3(s, 4) },
            1 => Surface::Cylinder { origin: v3(s, 1), axis: v3(s, 4), xdir: v3(s, 7), radius: s[10] },
            _ => Surface::Cone { origin: v3(s, 1), axis: v3(s, 4), xdir: v3(s, 7), radius: s[10], angle: s[11] },
        };
        out.faces.push(Face { surface, reversed: b.reversed[f], loops });
    }
    let shell = |h: usize| b.faces_of_shells[b.shell_faces[h] as usize..b.shell_faces[h + 1] as usize].to_vec();
    out.solids = (0..b.solid_shells.len().saturating_sub(1)).map(|s| Solid { shells: (b.solid_shells[s] as usize..b.solid_shells[s + 1] as usize).map(shell).collect() }).collect();
    out
}

fn flat(b: &Body, out: &mut BrepBody) {
    let xyz = |p: glam::DVec3| [p.x, p.y, p.z];
    out.vertices = b.vertices.iter().flat_map(|&p| xyz(p)).collect();
    out.edges = b.edges.iter().flat_map(|e| e.v).collect();
    for e in &b.edges {
        match e.curve {
            Curve::Line => out.curves.extend([0.0; 11]),
            Curve::Degenerate => {
                out.curves.push(2.0);
                out.curves.extend([0.0; 10]);
            }
            Curve::Circle { center, normal, xdir, radius } => {
                out.curves.push(1.0);
                out.curves.extend(xyz(center).into_iter().chain(xyz(normal)).chain(xyz(xdir)));
                out.curves.push(radius);
            }
        }
    }
    out.face_loops = vec![0];
    out.loop_uses = vec![0];
    for f in &b.faces {
        match f.surface {
            Surface::Plane { origin, normal } => {
                out.surfaces.push(0.0);
                out.surfaces.extend(xyz(origin).into_iter().chain(xyz(normal)).chain([0.0; 5]));
            }
            Surface::Cylinder { origin, axis, xdir, radius } => {
                out.surfaces.push(1.0);
                out.surfaces.extend(xyz(origin).into_iter().chain(xyz(axis)).chain(xyz(xdir)));
                out.surfaces.extend([radius, 0.0]);
            }
            Surface::Cone { origin, axis, xdir, radius, angle } => {
                out.surfaces.push(2.0);
                out.surfaces.extend(xyz(origin).into_iter().chain(xyz(axis)).chain(xyz(xdir)));
                out.surfaces.extend([radius, angle]);
            }
        }
        out.reversed.push(f.reversed);
        for l in &f.loops {
            out.uses.extend(l.iter().map(|u| u.edge * 2 + u.reversed as u32));
            out.loop_uses.push(out.uses.len() as u32);
        }
        out.face_loops.push(out.loop_uses.len() as u32 - 1);
    }
    out.solid_shells = vec![0];
    out.shell_faces = vec![0];
    for s in &b.solids {
        for h in &s.shells {
            out.faces_of_shells.extend(h);
            out.shell_faces.push(out.faces_of_shells.len() as u32);
        }
        out.solid_shells.push(out.shell_faces.len() as u32 - 1);
    }
}

fn origins(o: &[Origin]) -> Vec<u32> {
    o.iter()
        .flat_map(|o| match *o {
            Origin::Same { arg, index } => [0, arg, index],
            Origin::Piece { arg, index } => [1, arg, index],
            Origin::New => [2, 0, 0],
        })
        .collect()
}

pub(crate) fn brep_boolean(op: u8, args: &[BrepBody], tol: f64, touching_only: bool, merged_after: bool, out: &mut BrepResult) -> bool {
    let op = match op {
        0 => Op::Fuse,
        1 => Op::Cut,
        _ => Op::Common,
    };
    let bodies: Vec<Body> = args.iter().map(body_of).collect();
    let refs: Vec<&Body> = bodies.iter().collect();
    let r = match noshape_brep::boolean_touching(op, &refs, tol, touching_only).and_then(|r| if merged_after || MERGED_AFTER.with(|m| m.get()) { noshape_brep::merge_check(&r, tol).map(|_| r) } else { Ok(r) }) {
        Ok(r) => r,
        Err(d) => {
            brep_decline(d.0);
            return false;
        }
    };
    brep_decline("");
    flat(&r.body, &mut out.body);
    out.vertex_origin = origins(&r.vertex_origin);
    out.edge_origin = origins(&r.edge_origin);
    out.face_origin = origins(&r.face_origin);
    out.solid_origin = origins(&r.solid_origin);
    out.face_u0 = r.face_u0;
    let kind = |k: Kind| match k {
        Kind::Solid => 0,
        Kind::Face => 1,
        Kind::Edge => 2,
        Kind::Vertex => 3,
    };
    out.history = r
        .history
        .iter()
        .flat_map(|h| {
            let rel = match h.relation {
                Relation::Modified => 0,
                Relation::Generated => 1,
                Relation::Deleted => 2,
            };
            [kind(h.kind), h.arg, h.index, rel, kind(h.out_kind), h.out]
        })
        .collect();
    true
}

pub(crate) fn brep_recording() -> bool {
    bool_corpus::recording().is_some()
}

pub(crate) fn brep_record_direct(op: u8, objects: &OcShapeList, tools: &OcShapeList, out: OpOut, parallel: bool, ms: f64, path: &str) {
    let Some(dir) = bool_corpus::recording() else { return };
    let list = |l: &OcShapeList| -> Vec<Shape> { (0..raw::list_len(l).unwrap_or(0)).filter_map(|i| raw::list_get(l, i).ok()).map(Shape::from_ptr).collect() };
    let (objects, tools) = (list(objects), list(tools));
    let op = match op {
        0 => BooleanOp::Fuse,
        1 => BooleanOp::Cut,
        _ => BooleanOp::Common,
    };
    let opts = BooleanOptions { parallel, ..Default::default() };
    let call = bool_corpus::Call { op, objects: &objects, tools: &tools, opts: &opts };
    bool_corpus::record_result(&dir, &call, &OpResult::from_ffi(out), ms, path);
}
