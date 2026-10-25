//! Thin creation type shared by Extrude, Revolve and Sweep: wall curves are
//! offset in their plane into thin planar regions, which the feature then
//! sweeps like solid profiles.
//!
//! * Wall thicknesses: `thickness1` on one side and `thickness2` on the
//!   other (`flipWall` swaps them), or `thickness / 2` on both sides with
//!   Mid plane.
//! * Closed loops (sketch regions, planar faces, closed curve chains): every
//!   boundary loop is offset by `kernel::offset_wire_2d`; side 1 is outside
//!   the loop (Onshape's "offset1 points outward"), side 2 inside. Outer
//!   offsets try a sharp (intersection) join first and fall back to arcs.
//! * Open chains (sketch curves, edges): side 1 is to the left of the first
//!   selected edge's direction seen from the plane normal; the two offset
//!   curves are joined by straight end caps. Chains of collinear lines (which
//!   the kernel offset can't place in a plane) are translated instead.
//!
//! Naming: each region edge gets the source of the wall edge it was offset
//! from, tagged with its side (1 / 2) or end cap (3 = start, 4 = end) by
//! [`side_source`], so the wall faces of a thin feature have distinct,
//! stable names (`Side(<tagged source>)`).

use super::helpers_a::*;
use crate::display::PlaneFrame;
use crate::reference::{FeatureId, Reference, Source, TopoName};
use crate::regen::eval::{EvalCtx, FResult, Profile, err, kernel_plane};
use glam::DVec3;
use noshape_kernel as k;
use noshape_kernel::Shape;
use std::sync::Arc;

/// Wall thicknesses (side 1, side 2) from the Thin block.
pub fn thicknesses(ev: &EvalCtx) -> FResult<(f64, f64)> {
    let (mut t1, mut t2) = if ev.bool("midplane") {
        let t = ev.length("thickness")? * 0.5;
        (t, t)
    } else {
        (ev.length("thickness1")?, ev.length("thickness2")?)
    };
    if !ev.bool("midplane") && ev.bool("flipWall") {
        std::mem::swap(&mut t1, &mut t2);
    }
    if !(t1 >= 0.0 && t2 >= 0.0) || t1 + t2 <= 1e-9 {
        return err("The wall thickness must be positive");
    }
    Ok((t1, t2))
}

/// A wall source tagged with its side so the two walls (and the end caps)
/// generated from one curve get distinct names.
pub fn side_source(src: &Source, side: u16) -> Source {
    match src {
        Source::Sketch { sketch, entity, segment } => Source::Sketch { sketch: *sketch, entity: *entity, segment: segment.wrapping_add(side.wrapping_mul(0x1000)) },
        Source::Topo(n) => Source::Topo(Arc::new(TopoName::Split { parent: n.clone(), by: FeatureId(0), ordinal: 0xF000 + side })),
        Source::Region { sketch, key } => {
            let mut key = key.clone();
            key.push(0xF000_0000 + side as u32);
            Source::Region { sketch: *sketch, key }
        }
        Source::Index(i) => Source::Index(i.wrapping_add(side as u32 * 1_000_000)),
    }
}

/// Thin regions for the `wallShape` selection of a Thin feature.
pub fn thin_profiles(ev: &EvalCtx, refs: &[Reference]) -> FResult<Vec<Profile>> {
    let (t1, t2) = thicknesses(ev)?;
    let mut out = Vec::new();
    // Closed loops of regions and planar faces.
    let face_refs: Vec<Reference> = refs.iter().filter(|r| matches!(r, Reference::SketchRegion { .. } | Reference::Face(_))).cloned().collect();
    for p in ev.profiles(&face_refs)? {
        let edges = p.shape.edges();
        for w in p.shape.wires() {
            let sources: Vec<SourcedEdge> = w
                .edges()
                .into_iter()
                .map(|e| {
                    let src = edges.iter().position(|x| x.is_same(&e)).and_then(|i| p.edge_sources.get(i).cloned()).unwrap_or(Source::Index(0));
                    SourcedEdge { shape: e, source: src }
                })
                .collect();
            out.push(closed_wall(&p.frame, &w, &sources, t1, t2, p.key.clone())?);
        }
    }
    // Curves: sketch entities, whole sketches, edges, wire bodies.
    let curve_refs: Vec<Reference> = refs.iter().filter(|r| !matches!(r, Reference::SketchRegion { .. } | Reference::Face(_))).cloned().collect();
    let edges = resolve_curves(ev, &curve_refs)?;
    if !edges.is_empty() {
        let sketch_frame = curve_refs.iter().find_map(|r| match r {
            Reference::SketchEntity { sketch, .. } | Reference::Sketch(sketch) => ev.ctx.sketch(*sketch).map(|s| s.frame),
            _ => None,
        });
        for (i, w) in wires_from(&edges)?.into_iter().enumerate() {
            let chain: Vec<SourcedEdge> = edges.iter().filter(|e| w.edges().iter().any(|x| x.is_same(&e.shape) || same_edge(x, &e.shape))).cloned().collect();
            let chain = if chain.is_empty() { edges.clone() } else { chain };
            let frame = match sketch_frame {
                Some(f) => f,
                None => wire_frame(&w)?,
            };
            let key = chain.first().map(|e| e.source.clone()).unwrap_or(Source::Index(i as u32));
            if w.is_closed().unwrap_or(false) || closed_chain(&w) {
                out.push(closed_wall(&frame, &w, &chain, t1, t2, key)?);
            } else {
                out.push(open_wall(&frame, &w, &chain, t1, t2, key)?);
            }
        }
    }
    if out.is_empty() {
        return err("Select edges or faces to define the path of the thin wall.");
    }
    Ok(out)
}

fn same_edge(a: &Shape, b: &Shape) -> bool {
    match (edge_mid(a), edge_mid(b)) {
        (Some(p), Some(q)) => p.distance(q) < 1e-9 && (a.length().unwrap_or(0.0) - b.length().unwrap_or(0.0)).abs() < 1e-9,
        _ => false,
    }
}

fn closed_chain(w: &Shape) -> bool {
    ends(w).is_none()
}

/// Plane frame of a planar (non-collinear) wire.
fn wire_frame(w: &Shape) -> FResult<PlaneFrame> {
    let pl = w.find_plane(1e-6)?.ok_or_else(|| fe("The wall curves must lie in one plane"))?;
    Ok(PlaneFrame { origin: pl.origin, x: pl.unit_x(), y: pl.unit_normal().cross(pl.unit_x()), normal: pl.unit_normal() })
}

/// End points of an open wire (vertices with a single edge), or None when closed.
fn ends(w: &Shape) -> Option<(DVec3, DVec3)> {
    let ve = w.vertex_edges().ok()?;
    let verts = w.vertices();
    let free: Vec<DVec3> = ve.iter().enumerate().filter(|(_, es)| es.len() == 1).filter_map(|(i, _)| verts.get(i)?.vertex_point().ok()).collect();
    (free.len() == 2).then(|| (free[0], free[1]))
}

fn offset(w: &Shape, d: f64, open: bool) -> FResult<Shape> {
    let r = k::offset_wire_2d(w, d, k::JoinType::Intersection, open).or_else(|_| k::offset_wire_2d(w, d, k::JoinType::Arc, open)).map_err(kerr("The wall offset failed (thickness too large?)"))?;
    let r = r.wires().into_iter().next().unwrap_or(r);
    Ok(r)
}

fn area_of(w: &Shape) -> f64 {
    k::make_face_from_wire(w, true).and_then(|f| f.area()).unwrap_or(0.0)
}

/// Thin wall around a closed loop.
fn closed_wall(frame: &PlaneFrame, w: &Shape, sources: &[SourcedEdge], t_out: f64, t_in: f64, key: Source) -> FResult<Profile> {
    let a0 = area_of(w);
    let probe = t_out.max(t_in);
    let grow = offset(w, probe, false).map(|o| area_of(&o) > a0).unwrap_or(true);
    let s = if grow { 1.0 } else { -1.0 };
    let outer = if t_out > 0.0 { offset(w, s * t_out, false)? } else { w.clone() };
    let inner = if t_in > 0.0 {
        let i = offset(w, -s * t_in, false)?;
        if area_of(&i) <= 1e-18 {
            return err("The wall is too thick for the profile");
        }
        i
    } else {
        w.clone()
    };
    let face = k::make_face(&kernel_plane(frame), &outer, std::slice::from_ref(&inner)).map_err(kerr("Could not build the thin wall"))?;
    let edge_sources = tag_sources(&face, &outer, &inner, sources, None);
    Ok(Profile { shape: face, frame: *frame, edge_sources, key, from_face: false })
}

/// Unit vector to the left of the first selected edge (around `n`), at its midpoint.
fn left_of(e: &Shape, n: DVec3) -> FResult<(DVec3, DVec3)> {
    let i = e.edge_info()?;
    let t_mid = (i.first_param + i.last_param) * 0.5;
    let m = e.point_at(t_mid)?;
    let t = e.tangent_at(t_mid)?;
    let left = n.cross(t).normalize_or_zero();
    if left.length_squared() < 0.5 {
        return err("The wall curves must lie in the wall plane");
    }
    Ok((m, left))
}

/// Offset an open wire by `d` towards `left` (side test at `m`). Collinear
/// line chains are translated.
fn offset_side(w: &Shape, d: f64, m: DVec3, left: DVec3) -> FResult<Shape> {
    if d <= 0.0 {
        return Ok(w.clone());
    }
    let side_of = |o: &Shape| -> Option<f64> {
        let v = k::make_vertex(m).ok()?;
        let dd = k::distance(&v, o).ok()?;
        Some((dd.point_b - m).dot(left))
    };
    if let Ok(o) = offset(w, d, true)
        && let Some(sd) = side_of(&o)
    {
        if sd > 0.0 {
            return Ok(o);
        }
        if let Ok(o2) = offset(w, -d, true)
            && side_of(&o2).is_some_and(|x| x > 0.0)
        {
            return Ok(o2);
        }
    }
    // Straight chains have no plane for the kernel offset: translate.
    let straight = w.edges().iter().all(|e| matches!(e.edge_curve(), Ok(k::Curve::Line { .. })));
    if straight {
        return Ok(w.translated(left * d)?);
    }
    err("The wall offset failed (thickness too large?)")
}

/// Thin wall along an open chain: two offsets joined by end caps.
fn open_wall(frame: &PlaneFrame, w: &Shape, sources: &[SourcedEdge], t1: f64, t2: f64, key: Source) -> FResult<Profile> {
    let first = sources.first().map(|s| s.shape.clone()).unwrap_or_else(|| w.edges()[0].clone());
    let (m, left) = left_of(&first, frame.normal)?;
    let o1 = offset_side(w, t1, m, left)?;
    let o2 = offset_side(w, t2, m, -left)?;
    let (ws, we) = ends(w).ok_or_else(|| fe("The wall chain is not open"))?;
    let pair = |o: &Shape| -> FResult<(DVec3, DVec3)> {
        let (a, b) = ends(o).ok_or_else(|| fe("The wall offset is not an open chain"))?;
        Ok(if a.distance(ws) + b.distance(we) <= a.distance(we) + b.distance(ws) { (a, b) } else { (b, a) })
    };
    let (o1s, o1e) = pair(&o1)?;
    let (o2s, o2e) = pair(&o2)?;
    let mut edges: Vec<Shape> = o1.edges();
    edges.extend(o2.edges());
    let cap_s = k::make_line(o1s, o2s)?;
    let cap_e = k::make_line(o1e, o2e)?;
    edges.push(cap_s.clone());
    edges.push(cap_e.clone());
    let wires = k::edges_to_wires(&Shape::compound(&edges), 1e-6)?;
    let outline = match wires.as_slice() {
        [one] => one.clone(),
        _ => return err("Could not build the thin wall (self-intersecting offset?)"),
    };
    let face = k::make_face(&kernel_plane(frame), &outline, &[]).map_err(kerr("Could not build the thin wall"))?;
    let edge_sources = tag_sources(&face, &o1, &o2, sources, Some((ws, we)));
    Ok(Profile { shape: face, frame: *frame, edge_sources, key, from_face: false })
}

/// Source of every edge of a thin region (see the module docs).
fn tag_sources(face: &Shape, side1: &Shape, side2: &Shape, sources: &[SourcedEdge], open_ends: Option<(DVec3, DVec3)>) -> Vec<Source> {
    let tol = 1e-7;
    let dist = |p: DVec3, s: &Shape| -> f64 { k::make_vertex(p).ok().and_then(|v| k::distance(&v, s).ok()).map(|d| d.distance).unwrap_or(f64::INFINITY) };
    let nearest = |p: DVec3| -> Source {
        sources
            .iter()
            .map(|s| (dist(p, &s.shape), &s.source))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|x| x.1.clone())
            .unwrap_or(Source::Index(0))
    };
    face.edges()
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let Some(m) = edge_mid(e) else { return Source::Index(i as u32) };
            let (d1, d2) = (dist(m, side1), dist(m, side2));
            if d1 < tol && d1 <= d2 {
                side_source(&nearest(m), 1)
            } else if d2 < tol {
                side_source(&nearest(m), 2)
            } else {
                // End cap of an open wall: keyed by the chain end it closes.
                let cap = match open_ends {
                    Some((s, e2)) if m.distance(s) <= m.distance(e2) => 3,
                    Some(_) => 4,
                    None => 5,
                };
                side_source(&nearest(m), cap)
            }
        })
        .collect()
}
