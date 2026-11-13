//! The sketch snaps matrix (docs/parity/interaction-feel.md "Sketch snaps"): one row per target × inference × tool,
//! with live's reach (snaps up to, not from, CSS px) and the constraints its click creates. Each row is drawn on a
//! block's top face seen from above, in screen px from the face's centre, and probed through the viewport's events at
//! a device pixel ratio of 1 and 2; a model target is lit as the GPU's hover pick would light it. The rows that don't
//! match live yet are marked `gap` (a fix queued in the doc); the test fails when a matched row stops matching, and
//! when a gap starts matching (so the doc's status moves with it).

use super::*;
use noshape_sketch::ConstraintKind;

/// A point in screen px from the top face's centre, x right and y up (the sketch's axes, seen from above), or on the
/// part: the face's lower-left corner (a model vertex), a point a quarter along its bottom edge and that edge's
/// midpoint (a model edge), the sketch's origin.
#[derive(Clone, Copy, Debug)]
enum Pt {
    C(f64, f64),
    Corner,
    EdgeQuarter,
    EdgeMid,
    Origin,
}

#[derive(Clone, Copy, Debug)]
enum Draw {
    Point(Pt),
    Line(Pt, Pt),
    Circle(Pt, f64),
    /// Start, end, a point on the arc (the 3 point arc's clicks).
    Arc(Pt, Pt, Pt),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Lit {
    Nothing,
    Vertex,
    Edge,
}

struct Row {
    id: &'static str,
    tool: Tool,
    setup: &'static [Draw],
    /// The tool's first click, before the probe.
    start: Option<Pt>,
    /// Points rested on (live acquires a point after ~100 ms on it) before the probe.
    rest: &'static [Pt],
    /// Where the probe aims, and the direction (screen px, y up) it is moved off it.
    at: Pt,
    dir: (f64, f64),
    /// A click this far off snaps; from `not_from` it doesn't (`None`: not measured): live's reach, probed half a
    /// pixel inside and outside (its targets sit anywhere in their pixel, so its own takes vary by a pixel).
    snaps: f64,
    not_from: Option<f64>,
    /// What "doesn't snap" means at `not_from`: the first wanted constraint absent, or the point off the target.
    far: Far,
    /// Shift held through the probe (live: no snapping).
    shift: bool,
    /// A click after the probe's (a circle's radius).
    then: Option<Pt>,
    lit: Lit,
    /// The target lit while resting (a model edge woken).
    rest_lit: Lit,
    /// The constraints live's probe click creates (Onshape's names); `not` those it must not.
    want: &'static [&'static str],
    not: &'static [&'static str],
    /// The new point lands exactly on the target (a point, not the line through it).
    on_target: bool,
    /// Ours doesn't match live yet (interaction-feel.md "Sketch snaps" ≠).
    gap: bool,
}

/// `aim`: the target and the direction the probe moves off it; `reach`: snaps up to, not from.
const fn row(id: &'static str, tool: Tool, setup: &'static [Draw], start: Option<Pt>, aim: (Pt, (f64, f64)), reach: (f64, Option<f64>), want: &'static [&'static str]) -> Row {
    let ((at, dir), (snaps, not_from)) = (aim, reach);
    Row { id, tool, setup, start, rest: &[], at, dir, snaps, not_from, far: Far::Without, shift: false, then: None, lit: Lit::Nothing, rest_lit: Lit::Nothing, want, not: &[], on_target: false, gap: false }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Far {
    Without,
    OffTarget,
}

/// A point target (live's 15 px square): from `start`, onto `at` straight out along `dir` (6.5 px snaps, 8.5 doesn't)
/// or diagonally (9.5 / 11.5); `want` created and the point on the target.
const fn point_row(id: &'static str, setup: &'static [Draw], start: Pt, at: Pt, dir: (f64, f64), diagonal: bool, want: &'static [&'static str]) -> Row {
    let (snaps, far) = if diagonal { (9.5, 11.5) } else { (6.5, 8.5) };
    Row { on_target: true, far: Far::OffTarget, ..row(id, Tool::Line, setup, Some(start), (at, dir), (snaps, Some(far)), want) }
}

/// A curve or line zone (live's 15 px band): 6.5 px across snaps, 8.5 doesn't.
const fn band_row(id: &'static str, setup: &'static [Draw], start: Pt, at: Pt, dir: (f64, f64), want: &'static [&'static str]) -> Row {
    row(id, Tool::Line, setup, Some(start), (at, dir), (6.5, Some(8.5)), want)
}

use Pt::C;
const VERTICAL_LINE: &[Draw] = &[Draw::Line(C(0., -80.), C(0., 80.))];
/// A line through (20, 0) at 76° (x = 20 + y / 4): no direction of its own.
const SLANTED_LINE: &[Draw] = &[Draw::Line(C(0., -80.), C(40., 80.))];
const CIRCLE: &[Draw] = &[Draw::Circle(C(80., 0.), 50.)];
/// An arc from (-120, 0) over (-60, 45) to (0, 0) (centre (-60, -17.5)): at its end it heads along (0.28, -0.96).
const ARC: &[Draw] = &[Draw::Arc(C(-120., 0.), C(0., 0.), C(-60., 45.))];

const POINT_P: &[Draw] = &[Draw::Point(C(-60., 70.))];
/// Where the rows' lines start: clear of every target's horizontal and vertical.
const S: Pt = C(-170., -130.);
const H: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// The rows: live/feel/sketch-snaps.json (live2's catalogue 11-13, "reachZoomA", "wakeSummary", "toolNotes"), with
/// live/feel/sketch.json and behaviours "sketch snapping and hover" (qsnap 11-12) where the catalogue has no number.
fn rows() -> Vec<Row> {
    let d = (H, H);
    vec![
        // Reach. A point is a 15 px square (catalogue: 7 px straight out, 9–10 diagonally, not 8 / 11).
        point_row("reach/line-end/straight", VERTICAL_LINE, S, C(0., 80.), (1., 0.), false, &["COINCIDENT"]),
        point_row("reach/line-end/diagonal", VERTICAL_LINE, S, C(0., 80.), d, true, &["COINCIDENT"]),
        // (Along (0.45, 0.89): 9 px snaps, 10 doesn't.)
        Row { snaps: 8.0, not_from: Some(10.5), far: Far::Without, ..point_row("reach/line-midpoint", VERTICAL_LINE, S, C(0., 0.), (0.4472, 0.8944), false, &["MIDPOINT"]) },
        band_row("reach/line/across", VERTICAL_LINE, S, C(0., 30.), (1., 0.), &["COINCIDENT"]),
        point_row("reach/arc-end/straight", ARC, S, C(0., 0.), (1., 0.), false, &["COINCIDENT"]),
        point_row("reach/arc-end/diagonal", ARC, S, C(0., 0.), (H, -H), true, &["COINCIDENT"]),
        point_row("reach/arc-centre/straight", ARC, S, C(-60., -17.5), (1., 0.), false, &["COINCIDENT"]),
        point_row("reach/arc-centre/diagonal", ARC, S, C(-60., -17.5), d, true, &["COINCIDENT"]),
        Row { far: Far::Without, ..point_row("reach/arc-midpoint", ARC, S, C(-60., 45.), (1., 0.), false, &["MIDPOINT"]) },
        band_row("reach/circle-rim/outside", CIRCLE, S, C(130., 0.), (1., 0.), &["COINCIDENT"]),
        band_row("reach/circle-rim/inside", CIRCLE, S, C(130., 0.), (-1., 0.), &["COINCIDENT"]),
        Row { not: &["CONCENTRIC"], ..point_row("reach/circle-centre/straight", CIRCLE, S, C(80., 0.), (1., 0.), false, &["COINCIDENT"]) },
        point_row("reach/circle-centre/diagonal", CIRCLE, S, C(80., 0.), d, true, &["COINCIDENT"]),
        point_row("reach/point/straight", POINT_P, S, C(-60., 70.), (1., 0.), false, &["COINCIDENT"]),
        point_row("reach/point/diagonal", POINT_P, S, C(-60., 70.), d, true, &["COINCIDENT"]),
        Row { lit: Lit::Vertex, ..point_row("reach/model-vertex/straight", &[], C(-60., -40.), Pt::Corner, (-1., 0.), false, &["COINCIDENT"]) },
        Row { lit: Lit::Vertex, ..point_row("reach/model-vertex/diagonal", &[], C(-60., -40.), Pt::Corner, (-H, -H), true, &["COINCIDENT"]) },
        Row { lit: Lit::Edge, ..band_row("reach/model-edge/outside", &[], C(-60., 10.), Pt::EdgeQuarter, (0., -1.), &["COINCIDENT"]) },
        Row { lit: Lit::Edge, ..band_row("reach/model-edge/inside", &[], C(-60., 10.), Pt::EdgeQuarter, (0., 1.), &["COINCIDENT"]) },
        // An in-plane model edge's midpoint: MIDPOINT (catalogue: 6–7 px).
        Row { lit: Lit::Edge, far: Far::Without, ..point_row("reach/model-edge-midpoint", &[], C(-60., 10.), Pt::EdgeMid, (1., 0.), false, &["MIDPOINT"]) },
        point_row("reach/origin/straight", &[], S, Pt::Origin, (-1., 0.), false, &["COINCIDENT"]),
        point_row("reach/origin/diagonal", &[], S, Pt::Origin, (-H, -H), true, &["COINCIDENT"]),
        // The horizontal / vertical through the start: 15 px bands (catalogue: 8 / 6 and 7 / 7 px each side).
        band_row("reach/horizontal/above", &[], S, C(-20., -130.), (0., 1.), &["HORIZONTAL"]),
        band_row("reach/horizontal/below", &[], S, C(-20., -130.), (0., -1.), &["HORIZONTAL"]),
        band_row("reach/vertical/right", &[], S, C(-170., 20.), (1., 0.), &["VERTICAL"]),
        band_row("reach/vertical/left", &[], S, C(-170., 20.), (-1., 0.), &["VERTICAL"]),
        // Shift: no snapping (catalogue: 3 px off a line's end lands 3 px off, nothing made).
        Row { shift: true, not: &["COINCIDENT"], ..row("shift/line-end/none", Tool::Line, VERTICAL_LINE, Some(S), (C(0., 80.), (1., 0.)), (3., None), &[]) },
        // Waking. A sketch line snapped onto gives PARALLEL through the start (and PERPENDICULAR): 150 px long, 2 px
        // off parallel snaps, 3 doesn't.
        Row { rest: &[C(20., 0.)], ..row("wake/sketch-line/parallel", Tool::Line, SLANTED_LINE, Some(C(-150., -100.)), (C(-150. + 150. * 0.2425, -100. + 150. * 0.9701), (0.9701, -0.2425)), (2., Some(3.)), &["PARALLEL"]) },
        // A circle snapped onto gives the two tangent lines from the start (here 120 px along one, 2 px off it).
        Row { rest: &[C(130., 0.)], ..row("wake/circle/tangent-line", Tool::Line, CIRCLE, Some(C(-60., -120.)), (C(-60. + 120. * 0.5548, -120. + 120. * 0.8320), (0.8320, -0.5548)), (2., Some(6.)), &["TANGENT"]) },
        // Per tool: a circle's radius point takes coincident points and tangent points only: no midpoint, no curve.
        Row { not: &["MIDPOINT", "COINCIDENT"], ..row("tool/circle-radius/no-midpoint", Tool::Circle, VERTICAL_LINE, Some(C(-100., 0.)), (C(0., 0.), (1., 0.)), (3., None), &[]) },
        // Combinations and priority (live2 "feel (sketch)", the catalogue's priority classes).
        // A circle's centre onto a circle's centre: COINCIDENT, not CONCENTRIC (catalogue: 7 px, not 8).
        Row { on_target: true, far: Far::OffTarget, then: Some(C(80., 20.)), not: &["CONCENTRIC"], ..row("circle-centre/centre/coincident", Tool::Circle, CIRCLE, None, (C(80., 0.), (0., -1.)), (6.5, Some(8.5)), &["COINCIDENT"]) },
        // A line from an arc's end along its tangent: 100 px long snaps 2 px off it, not from 4; 200 px long 1, not 2.
        row("line/arc-end/tangent/100", Tool::Line, ARC, Some(C(0., 0.)), (C(28., -96.), (0.96, 0.28)), (2., Some(4.)), &["TANGENT"]),
        row("line/arc-end/tangent/200", Tool::Line, ARC, Some(C(0., 0.)), (C(56., -192.), (0.96, 0.28)), (1., Some(2.)), &["TANGENT"]),
        // A line from inside a line, square to it: 150 px long snaps 2 px off, not from 3; 300 px 1, not 2 (a slanted
        // line: no horizontal or vertical on the way).
        row("line/from-a-line/perpendicular/150", Tool::Line, SLANTED_LINE, Some(C(20., 0.)), (C(20. + 150. * 0.9701, -150. * 0.2425), (0.2425, 0.9701)), (2., Some(3.)), &["PERPENDICULAR"]),
        row("line/from-a-line/perpendicular/300", Tool::Line, SLANTED_LINE, Some(C(20., 0.)), (C(20. + 300. * 0.9701, -300. * 0.2425), (0.2425, 0.9701)), (1., Some(2.)), &["PERPENDICULAR"]),
        // A line ending on a circle at its tangent point (from (-60, -120), touching at (38.5, 27.7)): 6 px on along
        // it, not 10 (there the end is on the circle only).
        row("line-end/circle/tangent", Tool::Line, CIRCLE, Some(C(-60., -120.)), (C(-60. + 177.5 * 0.5548, -120. + 177.5 * 0.8320), (0.5548, 0.8320)), (6., Some(10.)), &["TANGENT"]),
        // Two at once: a horizontal line ending on a (slanted, free) line, both.
        row("line-end/line+horizontal/both", Tool::Line, SLANTED_LINE, Some(C(-150., 13.)), (C(20. + 13. * 0.25, 13.), (1., 0.)), (3., None), &["COINCIDENT", "HORIZONTAL"]),
        // A point beats horizontal: the end on a point 3 px off the start's horizontal takes the point.
        Row { on_target: true, not: &["HORIZONTAL"], ..row("line-end/point-over-horizontal", Tool::Line, VERTICAL_LINE, Some(C(-150., 77.)), (C(0., 80.), (1., 0.)), (0., None), &["COINCIDENT"]) },
        // A point beats its line: 3 px along the line from its end, the end takes the end point.
        Row { on_target: true, ..row("line-end/point-over-its-line", Tool::Line, VERTICAL_LINE, Some(C(-150., -40.)), (C(0., 80.), (0., -1.)), (3., None), &["COINCIDENT"]) },
        // Two acquired points: their horizontal and vertical meet (live2 acquisition "two-acquired-intersection").
        Row {
            rest: &[C(0., 80.), C(-120., -40.)],
            ..row("line-end/two-acquired/intersection", Tool::Line, &[Draw::Line(C(0., 80.), C(40., 120.)), Draw::Line(C(-120., -40.), C(-160., -80.))], Some(C(-200., 150.)), (C(0., -40.), (1., 0.)), (3., None), &["HORIZONTAL", "VERTICAL"])
        },
        // Model targets, lit under the pointer (user P0 11-13): onto a model vertex, onto a model edge.
        Row { lit: Lit::Vertex, on_target: true, ..row("line-end/model-vertex/coincident", Tool::Line, &[], Some(C(-60., -40.)), (Pt::Corner, (-H, -H)), (3., None), &["COINCIDENT"]) },
        Row { lit: Lit::Edge, ..row("line-end/model-edge/on", Tool::Line, &[], Some(C(-60., -10.)), (Pt::EdgeQuarter, (0., -1.)), (3., None), &["COINCIDENT"]) },
    ]
}

/// A block's top face sketched on, seen from above at `scale`: the window, the view, the face's centre and its
/// lower-left corner and bottom edge's quarter point on screen, the screen directions of the sketch's axes, and
/// the pick ids of that corner's vertex and that edge.
struct Bench {
    window: AnyWindowHandle,
    view: Entity<PartStudioView>,
    centre: DVec2,
    corner: DVec2,
    quarter: DVec2,
    edge_mid: DVec2,
    origin: DVec2,
    ex: DVec2,
    ey: DVec2,
    vertex_pick: u32,
    edge_pick: u32,
}

impl Bench {
    /// A sketch opened in a Part Studio kept for every probe (a window and a block's regeneration are most of a
    /// probe's time): on `top` (a block's top face: its centre, its lower-left corner, a quarter along and the middle
    /// of its bottom edge) or, without one, on Top (a 50 × 40 mm "face" whose corner is at (5, 0) mm, the origin left
    /// of it); seen from above at `scale`.
    fn open(cx: &mut TestAppContext, window: AnyWindowHandle, view: &Entity<PartStudioView>, top: Option<&Reference>, scale: f32) -> Self {
        cx.update_window(window, |_, w, cx| {
            view.update(cx, |v, cx| {
                v.selection = vec![top.cloned().unwrap_or(Reference::Plane(DatumPlane::Top))];
                v.start_feature("sketch", w, cx);
            })
        })
        .unwrap();
        draw(cx, window);
        wait_until(cx, window, |cx| view.read(cx).sketch.is_some() && !view.read(cx).ui_busy());
        cx.update_window(window, |_, w, _| w.set_scale_factor(scale)).unwrap();
        let frame = cx.update(|cx| view.read(cx).sketch.as_ref().unwrap().frame);
        // The face's corners in the sketch, and its corner vertex's and bottom edge's pick ids.
        let (lo, hi, vertex, edge) = match top {
            None => (DVec2::new(0.005, 0.0), DVec2::new(0.055, 0.04), 0, 0),
            Some(_) => cx.update(|cx| {
                let v = view.read(cx);
                let b = v.display.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Solid).unwrap();
                let top_z = b.mesh.positions.iter().map(|p| p[2]).fold(f32::MIN, f32::max);
                let local = |p: &[f32; 3]| frame.to_local(glam::DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64));
                let on_top: Vec<DVec2> = b.mesh.positions.iter().filter(|p| (p[2] - top_z).abs() < 1e-7).map(local).collect();
                let lo = on_top.iter().fold(DVec2::splat(f64::MAX), |a, p| a.min(*p));
                let hi = on_top.iter().fold(DVec2::splat(f64::MIN), |a, p| a.max(*p));
                let picks = &v.built.as_ref().unwrap().picks;
                let vi = b.mesh.vertices.iter().position(|q| (local(q) - lo).length() < 1e-6 && (q[2] - top_z).abs() < 1e-7).expect("the corner's vertex");
                let vertex = picks.ids_for(&b.vertex_refs[vi]).first().copied().expect("the vertex is pickable");
                let ei = b
                    .mesh
                    .edges
                    .iter()
                    .position(|pl| pl.iter().all(|p| (p[2] - top_z).abs() < 1e-7 && (local(p).y - lo.y).abs() < 1e-6))
                    .expect("the face's bottom edge");
                let edge = picks.ids_for(&b.edge_refs[ei]).first().copied().expect("the edge is pickable");
                (lo, hi, vertex, edge)
            }),
        };
        let mid = (lo + hi) * 0.5;
        cx.update_window(window, |_, _, cx| {
            view.update(cx, |v, cx| {
                v.viewport.update(cx, |vp, cx| {
                    let size = vp.size();
                    vp.camera.set_view(noshape_render::StandardView::Top);
                    vp.camera.fit_sphere(frame.to_world(mid), (hi.x - lo.x).max(hi.y - lo.y) * 0.5, size.x / size.y);
                    vp.request_frame(cx);
                })
            })
        })
        .unwrap();
        draw(cx, window);
        let screen = |cx: &mut TestAppContext, q: DVec2| cx.update(|cx| view.read(cx).viewport.read(cx).project(frame.to_world(q)).unwrap());
        let (centre, corner) = (screen(cx, mid), screen(cx, lo));
        let quarter = screen(cx, DVec2::new(lo.x + (hi.x - lo.x) * 0.25, lo.y));
        let edge_mid = screen(cx, DVec2::new((lo.x + hi.x) * 0.5, lo.y));
        let origin = screen(cx, DVec2::ZERO);
        let ex = (screen(cx, mid + DVec2::X * 1e-3) - centre).normalize();
        let ey = (screen(cx, mid + DVec2::Y * 1e-3) - centre).normalize();
        let size = (screen(cx, hi) - corner).abs();
        assert!(size.x.min(size.y) > 250.0, "the face fills enough of the view ({size:?} px)");
        Self { window, view: view.clone(), centre, corner, quarter, edge_mid, origin, ex, ey, vertex_pick: vertex, edge_pick: edge }
    }

    /// The probe's sketch cancelled: the Part Studio as it was, for the next probe.
    fn close(&self, cx: &mut TestAppContext) {
        cx.update_window(self.window, |_, w, cx| self.view.update(cx, |v, cx| v.cancel_edit(w, cx))).unwrap();
        draw(cx, self.window);
        wait_until(cx, self.window, |cx| self.view.read(cx).sketch.is_none() && !self.view.read(cx).ui_busy());
    }

    fn at(&self, p: Pt) -> DVec2 {
        match p {
            Pt::C(x, y) => self.centre + self.ex * x + self.ey * y,
            Pt::Corner => self.corner,
            Pt::EdgeQuarter => self.quarter,
            Pt::EdgeMid => self.edge_mid,
            Pt::Origin => self.origin,
        }
    }

    /// Screen px along `dir` (x right, y up on the sketch).
    fn along(&self, dir: (f64, f64)) -> DVec2 {
        (self.ex * dir.0 + self.ey * dir.1).normalize()
    }

    fn events(&self, cx: &mut TestAppContext, evs: &[ViewportEvent]) {
        cx.update_window(self.window, |_, w, cx| {
            self.view.update(cx, |v, cx| {
                let vp = v.viewport.clone();
                for ev in evs {
                    if let ViewportEvent::PointerMove { position, .. } = ev {
                        vp.update(cx, |vp, _| vp.place_pointer(*position));
                    }
                    v.on_viewport_event(&vp, ev, w, cx);
                }
            })
        })
        .unwrap();
        // (No frame: the editor answers each event as it comes; the matrix's time is in frames.)
        cx.run_until_parked();
    }

    fn click(&self, cx: &mut TestAppContext, at: DVec2, hover: Option<u32>) {
        self.click_with(cx, at, hover, Modifiers::default());
    }

    fn click_with(&self, cx: &mut TestAppContext, at: DVec2, hover: Option<u32>, m: Modifiers) {
        let mut evs = vec![ViewportEvent::PointerMove { position: at, modifiers: m }];
        if let Some(id) = hover {
            evs.push(ViewportEvent::Hover(Some(id)));
        }
        evs.extend([ViewportEvent::ToolDown { position: at, modifiers: m, click_count: 1 }, ViewportEvent::ToolUp { position: at, modifiers: m }]);
        self.events(cx, &evs);
    }

    fn tool(&self, cx: &mut TestAppContext, tool: Tool) {
        cx.update_window(self.window, |_, w, cx| {
            self.view.update(cx, |v, cx| {
                v.activate_tool(Tool::Select, w, cx);
                v.activate_tool(tool, w, cx);
            })
        })
        .unwrap();
        cx.run_until_parked();
    }

    fn draw_setup(&self, cx: &mut TestAppContext, d: &Draw) {
        match *d {
            Draw::Point(p) => {
                self.tool(cx, Tool::Point);
                self.click(cx, self.at(p), None);
            }
            Draw::Line(a, b) => {
                self.tool(cx, Tool::Line);
                self.click(cx, self.at(a), None);
                self.click(cx, self.at(b), None);
            }
            Draw::Circle(c, r) => {
                self.tool(cx, Tool::Circle);
                self.click(cx, self.at(c), None);
                self.click(cx, self.at(c) + self.ex * r, None);
            }
            Draw::Arc(a, b, m) => {
                self.tool(cx, Tool::Arc3);
                for p in [a, b, m] {
                    self.click(cx, self.at(p), None);
                }
            }
        }
    }

    /// The sketch's constraints by Onshape's name (COINCIDENT, HORIZONTAL, …), one per constraint.
    fn constraints(&self, cx: &mut TestAppContext) -> Vec<(String, ConstraintKind)> {
        cx.update(|cx| {
            let data = self.view.read(cx).sketch_data(cx).unwrap();
            data.sketch
                .constraints()
                .map(|c| {
                    let name = format!("{:?}", c.kind);
                    let name = name.split([' ', '{', '(']).next().unwrap_or_default().to_uppercase();
                    (if name == "MIDPOINTOF" { "MIDPOINT".into() } else { name }, c.kind.clone())
                })
                .collect()
        })
    }
}

/// The sketch's entity ids.
fn entity_ids(cx: &mut TestAppContext, b: &Bench) -> std::collections::HashSet<noshape_sketch::EntityId> {
    cx.update(|cx| b.view.read(cx).sketch_data(cx).unwrap().sketch.entities().map(|e| e.id).collect())
}

/// What one probe click created: the new constraints' names (a new curve's point merged into an existing point counts
/// as COINCIDENT, as Onshape makes one), and how far from the target, on screen, the new curve's nearest point is.
fn probe(cx: &mut TestAppContext, studios: &Studios, r: &Row, scale: f32, off: f64) -> (Vec<String>, f64) {
    let on_model = r.lit != Lit::Nothing || r.rest_lit != Lit::Nothing || matches!(r.at, Pt::Corner | Pt::EdgeQuarter | Pt::EdgeMid);
    let b = match on_model {
        true => Bench::open(cx, studios.block.0, &studios.block.1, Some(&studios.block.2), scale),
        false => Bench::open(cx, studios.plain.0, &studios.plain.1, None, scale),
    };
    for d in r.setup {
        b.draw_setup(cx, d);
    }
    b.tool(cx, r.tool);
    if let Some(s) = r.start {
        b.click(cx, b.at(s), None);
    }
    let lit = |l: Lit| match l {
        Lit::Nothing => None,
        Lit::Vertex => Some(b.vertex_pick),
        Lit::Edge => Some(b.edge_pick),
    };
    for p in r.rest {
        let q = b.at(*p);
        let mut evs = vec![ViewportEvent::PointerMove { position: q, modifiers: Modifiers::default() }];
        evs.extend(lit(r.rest_lit).map(|id| ViewportEvent::Hover(Some(id))));
        b.events(cx, &evs);
        std::thread::sleep(std::time::Duration::from_millis(130));
        b.events(cx, &[ViewportEvent::PointerMove { position: q + DVec2::splat(0.25), modifiers: Modifiers::default() }]);
    }
    let before = b.constraints(cx).len();
    let old = entity_ids(cx, &b);
    let target = b.at(r.at);
    let at = target + b.along(r.dir) * off;
    b.click_with(cx, at, lit(r.lit), Modifiers { shift: r.shift, ..Default::default() });
    if let Some(t) = r.then {
        b.click(cx, b.at(t), None);
    }
    // The probed point: the new curves' point nearest the target (an existing or model point it merged into
    // included); the constraints the probe made are those on it or on a new curve (not the start's own).
    let (probed, curves, merged, landed) = cx.update(|cx| {
        let v = b.view.read(cx);
        let data = v.sketch_data(cx).unwrap();
        let ed = v.sketch.as_ref().unwrap();
        let vp = v.viewport.read(cx);
        let mut best: Option<(f64, noshape_sketch::EntityId)> = None;
        let mut curves = Vec::new();
        for e in data.sketch.entities().filter(|e| !old.contains(&e.id) && !e.external) {
            let points: Vec<noshape_sketch::EntityId> = match (data.sketch.endpoints(e.id), e.center()) {
                (Some((p, q)), c) => [Some(p), Some(q), c].into_iter().flatten().collect(),
                (None, Some(c)) => vec![c],
                _ => continue,
            };
            curves.push(e.id);
            for p in points {
                if let Some(s) = data.sketch.point(p).and_then(|q| vp.project(ed.frame.to_world(q))) {
                    let d = (s - target).length();
                    if best.is_none_or(|b| d < b.0) {
                        best = Some((d, p));
                    }
                }
            }
        }
        let (landed, probed) = best.map_or((f64::MAX, None), |(d, p)| (d, Some(p)));
        // (Merged into an existing point, or a model vertex's used point.)
        let merged = probed.is_some_and(|p| old.contains(&p) || data.sketch.entity(p).is_some_and(|e| e.external));
        (probed, curves, merged, landed)
    });
    let mut created: Vec<String> = b
        .constraints(cx)
        .into_iter()
        .skip(before)
        .filter(|(_, k)| k.entities().iter().any(|e| Some(*e) == probed || curves.contains(e)))
        .map(|c| c.0)
        .collect();
    if merged {
        created.push("COINCIDENT".into());
    }
    b.close(cx);
    (created, landed)
}

/// The Part Studios the probes sketch in: a block (its top face) and an empty one.
struct Studios {
    block: (AnyWindowHandle, Entity<PartStudioView>, Reference),
    plain: (AnyWindowHandle, Entity<PartStudioView>),
}

/// The matrix at a device pixel ratio of 1 and 2: every row's snap at live's "snaps up to" and none at its "not
/// from". Prints matched / total.
#[gpui::test]
fn the_sketch_snaps_matrix_matches_live(cx: &mut TestAppContext) {
    let rows = rows();
    let studios = {
        let (window, view, top, _) = block_part(cx, "snaps-block");
        let plain = part_studio(cx, "snaps-plain");
        cx.update_window(plain.0, |_, w, _| w.activate_window()).unwrap();
        Studios { block: (window, view, top), plain }
    };
    let mut report = Vec::new();
    let mut wrong = Vec::new();
    for r in &rows {
        let mut ok = true;
        let mut why = Vec::new();
        for scale in [1.0f32, 2.0] {
            let (created, landed) = probe(cx, &studios, r, scale, r.snaps);
            let has = |n: &str| created.iter().any(|c| c == n);
            if !r.want.iter().all(|n| has(n)) || r.not.iter().any(|n| has(n)) || (r.on_target && landed > 1.0) {
                ok = false;
                why.push(format!("scale {scale}, {} px: created {created:?}, landed {landed:.2} px from the target", r.snaps));
            }
            if let Some(far) = r.not_from {
                let (created, landed) = probe(cx, &studios, r, scale, far);
                let snapped = match r.far {
                    Far::Without => created.iter().any(|c| r.want.first().is_some_and(|w| c == w)),
                    Far::OffTarget => landed < 1.0,
                };
                if snapped {
                    ok = false;
                    why.push(format!("scale {scale}, {far} px: still snapped: created {created:?}, landed {landed:.2} px from the target"));
                }
            }
        }
        report.push(format!("{} {}{}", if ok { "=" } else { "≠" }, r.id, if why.is_empty() { String::new() } else { format!(": {}", why.join("; ")) }));
        if ok == r.gap {
            wrong.push(r.id);
        }
    }
    let matched = report.iter().filter(|l| l.starts_with('=')).count();
    eprintln!("sketch snaps matrix: {matched} / {} rows match live\n{}", rows.len(), report.join("\n"));
    assert!(wrong.is_empty(), "rows whose status moved (update interaction-feel.md \"Sketch snaps\" and the row's `gap`): {wrong:?}");
}
