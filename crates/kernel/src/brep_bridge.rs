//! noshape's own Boolean (crates/brep) for the C++ side: `op_boolean` converts its inputs to flat
//! [`ffi::BrepBody`]s (cpp/rust_boolean.cpp), calls [`brep_boolean`], and builds the result's shapes and
//! history from the [`ffi::BrepResult`].

use crate::ffi::ffi::{self as raw, BrepBody, BrepResult, BrepSections, OcShapeList, OpOut};
use crate::{bool_corpus, BooleanOp, BooleanOptions, OpResult, Shape};
use noshape_brep::nurbs::{NurbsCurve, NurbsSurface};
use noshape_brep::{Body, Curve, Decline, Edge, Face, Kind, Op, Origin, PCurve, Relation, SectionCurve, SectionSource, Solid, Surface, Use};
use std::collections::HashMap;
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

/// The NURBS curve or surface at `at` in `nurbs` (ffi::BrepBody::nurbs).
enum Nurbs {
    Curve(NurbsCurve),
    Surface(NurbsSurface),
}

fn nurbs_at(nurbs: &[f64], at: usize) -> Nurbs {
    let n = |i: usize| nurbs[at + i] as usize;
    let pts = |from: usize, count: usize| (0..count).map(|i| v3(nurbs, from + 3 * i)).collect::<Vec<_>>();
    if nurbs[at] == 0.0 {
        let (degree, nk, np, rational) = (n(2), n(3), n(4), n(5) == 1);
        let k0 = at + 6;
        let p0 = k0 + nk;
        let w0 = p0 + 3 * np;
        return Nurbs::Curve(NurbsCurve { degree, knots: nurbs[k0..p0].to_vec(), poles: pts(p0, np), weights: if rational { nurbs[w0..w0 + np].to_vec() } else { Vec::new() } });
    }
    let (udegree, vdegree, nku, nkv, nu, nv, rational) = (n(1), n(2), n(3), n(4), n(5), n(6), n(7) == 1);
    let ku = at + 8;
    let kv = ku + nku;
    let p0 = kv + nkv;
    let w0 = p0 + 3 * nu * nv;
    Nurbs::Surface(NurbsSurface {
        udegree,
        vdegree,
        uknots: nurbs[ku..kv].to_vec(),
        vknots: nurbs[kv..p0].to_vec(),
        nu,
        nv,
        poles: pts(p0, nu * nv),
        weights: if rational { nurbs[w0..w0 + nu * nv].to_vec() } else { Vec::new() },
    })
}

/// The body noshape's own Boolean gets for `shape` (tests), or why there is none.
pub fn brep_body(shape: &Shape) -> Result<Body, String> {
    let mut b = BrepBody::default();
    let why = raw::brep_body_of(shape.raw(), &mut b);
    if why.is_empty() { Ok(body_of(&b)) } else { Err(why) }
}

fn body_of(b: &BrepBody) -> Body {
    let mut out = Body { vertices: (0..b.vertices.len() / 3).map(|i| v3(&b.vertices, 3 * i)).collect(), ..Default::default() };
    // NURBS curves by where they start in `b.nurbs`.
    let mut curves: HashMap<usize, u32> = HashMap::new();
    let mut curve_at = |out: &mut Body, at: usize| -> u32 {
        *curves.entry(at).or_insert_with(|| {
            let Nurbs::Curve(c) = nurbs_at(&b.nurbs, at) else { unreachable!() };
            out.nurbs_curves.push(c);
            out.nurbs_curves.len() as u32 - 1
        })
    };
    for (e, c) in b.edges.chunks(2).zip(b.curves.chunks(11)) {
        let curve = match c[0] as u8 {
            0 => Curve::Line,
            1 => Curve::Circle { center: v3(c, 1), normal: v3(c, 4), xdir: v3(c, 7), radius: c[10] },
            5 => Curve::Nurbs { index: curve_at(&mut out, c[1] as usize), t0: c[2], t1: c[3] },
            _ => Curve::Degenerate,
        };
        out.edges.push(Edge { v: [e[0], e[1]], curve });
    }
    for f in 0..b.reversed.len() {
        let loops = (b.face_loops[f]..b.face_loops[f + 1])
            .map(|l| b.uses[b.loop_uses[l as usize] as usize..b.loop_uses[l as usize + 1] as usize].iter().map(|&u| Use { edge: u / 2, reversed: u % 2 == 1 }).collect())
            .collect();
        let s = &b.surfaces[12 * f..12 * f + 12];
        let surface = match s[0] as u8 {
            0 => Surface::Plane { origin: v3(s, 1), normal: v3(s, 4) },
            1 => Surface::Cylinder { origin: v3(s, 1), axis: v3(s, 4), xdir: v3(s, 7), radius: s[10] },
            3 => {
                let Nurbs::Surface(n) = nurbs_at(&b.nurbs, s[1] as usize) else { unreachable!() };
                out.nurbs_surfaces.push(n);
                Surface::Nurbs { index: out.nurbs_surfaces.len() as u32 - 1 }
            }
            _ => Surface::Cone { origin: v3(s, 1), axis: v3(s, 4), xdir: v3(s, 7), radius: s[10], angle: s[11] },
        };
        out.faces.push(Face { surface, reversed: b.reversed[f], loops });
    }
    for p in b.pcurves.chunks(6) {
        let curve = curve_at(&mut out, p[3] as usize);
        out.pcurves.push(PCurve { face: p[0] as u32, edge: p[1] as u32, second: p[2] != 0.0, curve, t0: p[4], t1: p[5] });
    }
    let shell = |h: usize| b.faces_of_shells[b.shell_faces[h] as usize..b.shell_faces[h + 1] as usize].to_vec();
    out.solids = (0..b.solid_shells.len().saturating_sub(1)).map(|s| Solid { shells: (b.solid_shells[s] as usize..b.solid_shells[s + 1] as usize).map(shell).collect() }).collect();
    out
}

/// `c` at the end of `nurbs` (ffi::BrepBody::nurbs); where it starts.
fn push_curve(nurbs: &mut Vec<f64>, c: &NurbsCurve) -> f64 {
    let at = nurbs.len() as f64;
    nurbs.extend([0.0, 3.0, c.degree as f64, c.knots.len() as f64, c.poles.len() as f64, if c.weights.is_empty() { 0.0 } else { 1.0 }]);
    nurbs.extend(&c.knots);
    nurbs.extend(c.poles.iter().flat_map(|p| [p.x, p.y, p.z]));
    nurbs.extend(&c.weights);
    at
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
            Curve::Ellipse { center, normal, xdir, major, .. } => {
                out.curves.push(3.0);
                out.curves.extend(xyz(center).into_iter().chain(xyz(normal)).chain(xyz(xdir)));
                out.curves.push(major);
            }
            Curve::Hyperbola { center, normal, xdir, major, .. } => {
                out.curves.push(4.0);
                out.curves.extend(xyz(center).into_iter().chain(xyz(normal)).chain(xyz(xdir)));
                out.curves.push(major);
            }
            Curve::Nurbs { index, t0, t1 } => {
                let at = push_curve(&mut out.nurbs, &b.nurbs_curves[index as usize]);
                out.curves.extend([5.0, at, t0, t1]);
                out.curves.extend([0.0; 7]);
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
            Surface::Nurbs { index } => {
                let s = &b.nurbs_surfaces[index as usize];
                let at = out.nurbs.len() as f64;
                out.nurbs.extend([1.0, s.udegree as f64, s.vdegree as f64, s.uknots.len() as f64, s.vknots.len() as f64, s.nu as f64, s.nv as f64, if s.weights.is_empty() { 0.0 } else { 1.0 }]);
                out.nurbs.extend(s.uknots.iter().chain(&s.vknots));
                out.nurbs.extend(s.poles.iter().flat_map(|&p| xyz(p)));
                out.nurbs.extend(&s.weights);
                out.surfaces.extend([3.0, at]);
                out.surfaces.extend([0.0; 10]);
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

/// Free-form faces' sections from OCCT (rust_boolean.cpp keeps its curves for building the result's edges).
struct Occt;

impl SectionSource for Occt {
    fn sections(&self, a: (u32, u32), b: (u32, u32)) -> Result<Vec<SectionCurve>, Decline> {
        let mut out = BrepSections::default();
        let why = raw::brep_occt_sections(a.0, a.1, b.0, b.1, &mut out);
        if !why.is_empty() {
            // (Reasons are static strings on the Rust side.)
            return Err(Decline(match why.as_str() {
                "a section OCCT makes as a conic" => "a section OCCT makes as a conic",
                "a section without its curves on the faces" => "a section without its curves on the faces",
                _ => "OCCT's section failed",
            }));
        }
        let curve = |at: f64| match nurbs_at(&out.nurbs, at as usize) {
            Nurbs::Curve(c) => c,
            Nurbs::Surface(_) => unreachable!(),
        };
        Ok(out.curves.chunks(6).map(|c| SectionCurve { curve: curve(c[3]), t0: c[1], t1: c[2], on: [curve(c[4]), curve(c[5])], id: c[0] as u32 }).collect())
    }
}

pub(crate) fn brep_boolean(op: u8, args: &[BrepBody], tol: f64, touching_only: bool, out: &mut BrepResult) -> bool {
    let op = match op {
        0 => Op::Fuse,
        1 => Op::Cut,
        _ => Op::Common,
    };
    let bodies: Vec<Body> = args.iter().map(body_of).collect();
    let refs: Vec<&Body> = bodies.iter().collect();
    let r = match noshape_brep::boolean_with(op, &refs, tol, touching_only, Some(&Occt)) {
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
    out.edge_section = r.edge_section;
    for e in &r.body.edges {
        let (minor, faces) = match e.curve {
            Curve::Ellipse { minor, faces, .. } | Curve::Hyperbola { minor, faces, .. } => (minor, [faces[0].0, faces[0].1, faces[1].0, faces[1].1]),
            _ => (0.0, [0; 4]),
        };
        out.ellipse_minor.push(minor);
        out.ellipse_faces.extend(faces);
    }
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
