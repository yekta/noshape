//! The Part Studio analysis tools' computations (Analysis tools menu:
//! Curve/surface, Deviation, Connection, Dihedral analysis, Interference
//! detection), GUI-free: the Part Studio's tool dialogs draw the results,
//! the agent API reports them. From `crates/app/src/part_studio/analysis_tools.rs`.

use glam::DVec3;
use noshape_model::analysis::{Comb, Continuity, ContinuityTolerances, Extremum, FaceConnections};
use noshape_model::display::StudioDisplay;
use noshape_model::interference::Interference;
use noshape_model::reference::{BodyId, Reference};
use noshape_model::regen::context::Context;

/// The analysis tools that compute (the view modes Zebra, Reflection,
/// Thickness, Flatten and Draft only change how parts are drawn).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnalysisKind {
    CurveSurface,
    Deviation,
    Connection,
    Dihedral,
    Interference,
}

/// The tool dialog's options that change what is computed.
#[derive(Clone, Debug, PartialEq)]
pub struct AnalysisOptions {
    pub u_curves: usize,
    pub v_curves: usize,
    /// Curve/surface: the tangent-connected faces too ("Extend across face").
    pub extend: bool,
    /// Curve/surface: control polygons of B-spline edges.
    pub control_points: bool,
    /// Deviation: Surface tab (else Curve).
    pub surface: bool,
    /// Connection: G1–G3 tolerances.
    pub tolerances: ContinuityTolerances,
}

impl Default for AnalysisOptions {
    fn default() -> Self {
        Self { u_curves: 8, v_curves: 8, extend: false, control_points: false, surface: false, tolerances: Default::default() }
    }
}

/// What a comb belongs to (the dialogs color them by it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombKind {
    /// A curve's curvature comb, or a dihedral comb.
    Curve,
    /// A face's isoparametric curves along U / V.
    U,
    V,
}

/// What a tool computed.
#[derive(Clone, Default)]
pub struct AnalysisResult {
    pub combs: Vec<(Comb, CombKind)>,
    /// Control polygons of B-spline edges.
    pub polylines: Vec<Vec<DVec3>>,
    /// Inflection points of sketch curves.
    pub inflections: Vec<DVec3>,
    /// Minimum radius and where.
    pub min_radius: Option<(f64, DVec3)>,
    /// Dihedral min / max angle (degrees).
    pub dihedral: Option<(f64, f64)>,
    /// Deviation minimum and maximum (distance, point on 1, point on 2).
    pub deviation: Option<(Extremum, Extremum)>,
    /// Interference detection: the overlaps with the two parts' names
    /// (`Interference::a` / `b` index `interference_parts`).
    pub interferences: Vec<(Interference, String, String)>,
    /// The parts interference detection checked, in order.
    pub interference_parts: Vec<BodyId>,
    /// Connection: curve endpoints and face connections.
    pub joints: Vec<(DVec3, Continuity)>,
    pub faces: FaceConnections,
}

/// Comb sample count (per curve).
const SAMPLES: usize = 32;

/// Run analysis `kind` on the selection (`sel2`: Deviation's second
/// curve chain / surface).
pub fn analyze(ctx: &Context, display: &StudioDisplay, kind: AnalysisKind, sel: &[Reference], sel2: &[Reference], o: &AnalysisOptions) -> AnalysisResult {
    let mut res = AnalysisResult::default();
    match kind {
        AnalysisKind::CurveSurface => {
            let mut faces: Vec<Reference> = sel.iter().filter(|r| matches!(r, Reference::Face(_))).cloned().collect();
            if o.extend {
                // Extend across face: the tangent-connected faces too.
                for f in faces.clone() {
                    for g in noshape_model::select::select_related(ctx, &f, noshape_model::select::Criterion::TangentFaces) {
                        if !faces.contains(&g) {
                            faces.push(g);
                        }
                    }
                }
            }
            for r in sel {
                match r {
                    Reference::Edge(_) => {
                        if let Some((comb, min)) = noshape_model::analysis::edge_curvature(ctx, r, SAMPLES) {
                            res.combs.push((comb, CombKind::Curve));
                            keep_min(&mut res.min_radius, min);
                        }
                        if o.control_points
                            && let Some(poles) = edge_poles(ctx, r)
                        {
                            res.polylines.push(poles);
                        }
                    }
                    Reference::SketchEntity { sketch, entity } => {
                        let Some(c) = display.sketches.iter().find(|s| s.feature == *sketch).and_then(|s| s.curves.iter().find(|c| c.entity == *entity)) else { continue };
                        let pts: Vec<DVec3> = c.points.iter().map(|p| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)).collect();
                        let (comb, min) = noshape_model::analysis::polyline_comb(&pts);
                        res.inflections.extend(inflections(&comb));
                        res.combs.push((comb, CombKind::Curve));
                        keep_min(&mut res.min_radius, min);
                    }
                    _ => {}
                }
            }
            for f in &faces {
                if let Some((along_u, along_v)) = noshape_model::analysis::face_curvature(ctx, f, o.u_curves, o.v_curves, SAMPLES) {
                    res.combs.extend(along_u.into_iter().map(|c| (c, CombKind::U)));
                    res.combs.extend(along_v.into_iter().map(|c| (c, CombKind::V)));
                }
            }
        }
        AnalysisKind::Deviation if o.surface => {
            let pts: Vec<DVec3> = sel.iter().flat_map(|r| face_triangles(display, r)).flatten().collect();
            let tris: Vec<[DVec3; 3]> = sel2.iter().flat_map(|r| face_triangles(display, r)).collect();
            res.deviation = noshape_model::analysis::surface_deviation(&pts, &tris);
        }
        AnalysisKind::Deviation => {
            let a = densify(&sel.iter().flat_map(|r| curve_points(display, r)).collect::<Vec<_>>(), 400);
            let b: Vec<DVec3> = sel2.iter().flat_map(|r| curve_points(display, r)).collect();
            res.deviation = noshape_model::analysis::curve_deviation(&a, &b);
        }
        AnalysisKind::Connection => {
            // Curves meet at their endpoints; faces along shared edges.
            let curves: Vec<Vec<DVec3>> = sel
                .iter()
                .filter_map(|r| match r {
                    Reference::Edge(_) => noshape_model::analysis::edge_polyline(ctx, r, 64),
                    Reference::SketchEntity { .. } => Some(curve_points(display, r)),
                    _ => None,
                })
                .collect();
            res.joints = noshape_model::analysis::curve_connections(&curves, &o.tolerances);
            let faces: Vec<Reference> = sel.iter().filter(|r| matches!(r, Reference::Face(_))).cloned().collect();
            res.faces = noshape_model::analysis::face_connections(ctx, &faces, SAMPLES, &o.tolerances);
        }
        AnalysisKind::Interference => {
            let ids: Vec<BodyId> = sel.iter().filter_map(|r| if let Reference::Body(b) = r { Some(b.clone()) } else { None }).collect();
            let bodies: Vec<&noshape_model::regen::context::Body> = ids.iter().filter_map(|id| ctx.bodies.iter().find(|b| b.id == *id).map(|b| &**b)).collect();
            let shapes: Vec<noshape_kernel::Shape> = bodies.iter().map(|b| b.shape.clone()).collect();
            res.interference_parts = bodies.iter().map(|b| b.id.clone()).collect();
            for i in noshape_model::interference::interferences(&shapes) {
                let (a, b) = (bodies[i.a].name.clone(), bodies[i.b].name.clone());
                res.interferences.push((i, a, b));
            }
        }
        AnalysisKind::Dihedral => {
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            for r in sel {
                if let Some((comb, a, b)) = noshape_model::analysis::edge_dihedral(ctx, r, SAMPLES) {
                    lo = lo.min(a);
                    hi = hi.max(b);
                    res.combs.push((comb, CombKind::Curve));
                }
            }
            res.dihedral = (lo <= hi).then_some((lo, hi));
        }
    }
    res
}

fn keep_min(cur: &mut Option<(f64, DVec3)>, new: Option<(f64, DVec3)>) {
    if let Some(n) = new
        && cur.is_none_or(|c| n.0 < c.0)
    {
        *cur = Some(n);
    }
}

/// Inflection points of a comb: where the tooth flips to the other side.
pub fn inflections(c: &Comb) -> Vec<DVec3> {
    let mut out = Vec::new();
    for i in 1..c.teeth.len().saturating_sub(1) {
        let (a, b) = (c.teeth[i - 1], c.teeth[i + 1]);
        if a.length() > 0.0 && b.length() > 0.0 && a.dot(b) < 0.0 && c.teeth[i].length() < a.length().max(b.length()) {
            out.push(c.base[i]);
        }
    }
    out
}

/// Control polygon of a B-spline part edge.
pub fn edge_poles(ctx: &Context, r: &Reference) -> Option<Vec<DVec3>> {
    let Reference::Edge(n) = r else { return None };
    ctx.bodies.iter().find_map(|b| {
        let e = *b.find_edges(n).first()?;
        let s = b.shape.sub_shape(noshape_kernel::ShapeType::Edge, e).ok()?;
        s.edge_bspline().ok().map(|d| d.poles)
    })
}

/// Points evenly spread along a polyline (at least `n` of them).
pub fn densify(pl: &[DVec3], n: usize) -> Vec<DVec3> {
    let total: f64 = pl.windows(2).map(|w| w[0].distance(w[1])).sum();
    if pl.len() < 2 || total <= 0.0 {
        return pl.to_vec();
    }
    let step = total / n.max(1) as f64;
    let mut out = vec![pl[0]];
    for w in pl.windows(2) {
        let l = w[0].distance(w[1]);
        let k = (l / step).ceil().max(1.0) as usize;
        for i in 1..=k {
            out.push(w[0].lerp(w[1], i as f64 / k as f64));
        }
    }
    out
}

/// Polyline of a part edge or sketch curve (display tessellation).
pub fn curve_points(display: &StudioDisplay, r: &Reference) -> Vec<DVec3> {
    let p3 = |p: &[f32; 3]| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
    match r {
        Reference::SketchEntity { sketch, entity } => display.sketches.iter().find(|s| s.feature == *sketch).and_then(|s| s.curves.iter().find(|c| c.entity == *entity)).map(|c| c.points.iter().map(p3).collect()).unwrap_or_default(),
        _ => display.bodies.iter().find_map(|b| b.edge_refs.iter().position(|x| x == r).and_then(|i| b.mesh.edges.get(i)).map(|pl| pl.iter().map(p3).collect())).unwrap_or_default(),
    }
}

/// Triangles of a part face (display tessellation).
pub fn face_triangles(display: &StudioDisplay, r: &Reference) -> Vec<[DVec3; 3]> {
    let p3 = |p: [f32; 3]| DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64);
    display
        .bodies
        .iter()
        .find_map(|b| {
            let fi = b.face_refs.iter().position(|x| x == r)?;
            let &(first, count) = b.mesh.face_ranges.get(fi)?;
            let idx = b.mesh.indices.get(first as usize..(first + count) as usize)?;
            Some(idx.as_chunks::<3>().0.iter().filter_map(|t| Some([p3(*b.mesh.positions.get(t[0] as usize)?), p3(*b.mesh.positions.get(t[1] as usize)?), p3(*b.mesh.positions.get(t[2] as usize)?)])).collect())
        })
        .unwrap_or_default()
}
