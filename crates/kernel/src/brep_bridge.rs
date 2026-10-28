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
            let curve = if c[0] == 0.0 { Curve::Line } else { Curve::Circle { center: v3(c, 1), normal: v3(c, 4), xdir: v3(c, 7), radius: c[10] } };
            Edge { v: [e[0], e[1]], curve }
        })
        .collect();
    for f in 0..b.reversed.len() {
        let loops = (b.face_loops[f]..b.face_loops[f + 1])
            .map(|l| b.uses[b.loop_uses[l as usize] as usize..b.loop_uses[l as usize + 1] as usize].iter().map(|&u| Use { edge: u / 2, reversed: u % 2 == 1 }).collect())
            .collect();
        let s = &b.surfaces[11 * f..11 * f + 11];
        let surface = if s[0] == 0.0 { Surface::Plane { origin: v3(s, 1), normal: v3(s, 4) } } else { Surface::Cylinder { origin: v3(s, 1), axis: v3(s, 4), xdir: v3(s, 7), radius: s[10] } };
        out.faces.push(Face { surface, reversed: b.reversed[f], loops });
    }
    out.solids = (0..b.solid_faces.len().saturating_sub(1)).map(|s| Solid { shells: vec![b.faces_of_solids[b.solid_faces[s] as usize..b.solid_faces[s + 1] as usize].to_vec()] }).collect();
    out
}

fn flat(b: &Body, out: &mut BrepBody) {
    let xyz = |p: glam::DVec3| [p.x, p.y, p.z];
    out.vertices = b.vertices.iter().flat_map(|&p| xyz(p)).collect();
    out.edges = b.edges.iter().flat_map(|e| e.v).collect();
    for e in &b.edges {
        match e.curve {
            Curve::Line => out.curves.extend([0.0; 11]),
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
                out.surfaces.extend(xyz(origin).into_iter().chain(xyz(normal)).chain([0.0; 4]));
            }
            Surface::Cylinder { origin, axis, xdir, radius } => {
                out.surfaces.push(1.0);
                out.surfaces.extend(xyz(origin).into_iter().chain(xyz(axis)).chain(xyz(xdir)));
                out.surfaces.push(radius);
            }
        }
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
