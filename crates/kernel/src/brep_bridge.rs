//! noshape's own Boolean (crates/brep) for the C++ side: `op_boolean` converts its inputs to flat
//! [`ffi::BrepBody`]s (cpp/rust_boolean.cpp), calls [`brep_boolean`], and builds the result's shapes and
//! history from the [`ffi::BrepResult`].

use crate::ffi::ffi::{BrepBody, BrepResult};
use noshape_brep::{Body, Curve, Edge, Face, Kind, Op, Origin, Relation, Solid, Surface, Use};
use std::cell::{Cell, RefCell};
use std::sync::OnceLock;

thread_local! {
    /// This thread's switch: None as NOSHAPE_BOOL_RUST says.
    static MODE: Cell<Option<bool>> = const { Cell::new(None) };
    /// A decline fails the Boolean instead of running OCCT (the replay's quick mode).
    static STRICT: Cell<bool> = const { Cell::new(false) };
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
/// 1 = on); the corpus replay compares it with OCCT.
pub fn set_rust_boolean(on: Option<bool>) {
    MODE.with(|m| m.set(on));
    brep_decline("not tried (an earlier path took it)");
}

pub(crate) fn brep_enabled() -> bool {
    static ENV: OnceLock<bool> = OnceLock::new();
    MODE.with(|m| m.get()).unwrap_or_else(|| *ENV.get_or_init(|| std::env::var("NOSHAPE_BOOL_RUST").is_ok_and(|v| v == "1")))
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

fn body_of(b: &BrepBody) -> Body {
    let v3 = |s: &[f64], i: usize| glam::DVec3::new(s[3 * i], s[3 * i + 1], s[3 * i + 2]);
    let mut out = Body { vertices: (0..b.vertices.len() / 3).map(|i| v3(&b.vertices, i)).collect(), ..Default::default() };
    out.edges = b.edges.chunks(2).map(|e| Edge { v: [e[0], e[1]], curve: Curve::Line }).collect();
    for f in 0..b.reversed.len() {
        let loops = (b.face_loops[f]..b.face_loops[f + 1])
            .map(|l| b.uses[b.loop_uses[l as usize] as usize..b.loop_uses[l as usize + 1] as usize].iter().map(|&u| Use { edge: u / 2, reversed: u % 2 == 1 }).collect())
            .collect();
        out.faces.push(Face { surface: Surface::Plane { origin: v3(&b.planes, 2 * f), normal: v3(&b.planes, 2 * f + 1) }, reversed: b.reversed[f], loops });
    }
    out.solids = (0..b.solid_faces.len().saturating_sub(1)).map(|s| Solid { shells: vec![b.faces_of_solids[b.solid_faces[s] as usize..b.solid_faces[s + 1] as usize].to_vec()] }).collect();
    out
}

fn flat(b: &Body, out: &mut BrepBody) {
    out.vertices = b.vertices.iter().flat_map(|p| [p.x, p.y, p.z]).collect();
    out.edges = b.edges.iter().flat_map(|e| e.v).collect();
    out.face_loops = vec![0];
    out.loop_uses = vec![0];
    for f in &b.faces {
        let Surface::Plane { origin, normal } = f.surface;
        out.planes.extend([origin.x, origin.y, origin.z, normal.x, normal.y, normal.z]);
        out.reversed.push(f.reversed);
        for l in &f.loops {
            out.uses.extend(l.iter().map(|u| u.edge * 2 + u.reversed as u32));
            out.loop_uses.push(out.uses.len() as u32);
        }
        out.face_loops.push(out.loop_uses.len() as u32 - 1);
    }
    out.solid_faces = vec![0];
    for s in &b.solids {
        out.faces_of_solids.extend(s.shells.iter().flatten());
        out.solid_faces.push(out.faces_of_solids.len() as u32);
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

pub(crate) fn brep_boolean(op: u8, args: &[BrepBody], tol: f64, out: &mut BrepResult) -> bool {
    let op = match op {
        0 => Op::Fuse,
        1 => Op::Cut,
        _ => Op::Common,
    };
    let bodies: Vec<Body> = args.iter().map(body_of).collect();
    let refs: Vec<&Body> = bodies.iter().collect();
    let r = match noshape_brep::boolean(op, &refs, tol) {
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
