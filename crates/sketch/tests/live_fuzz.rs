//! Sketch solver fuzz against live Onshape (user 11-02): random but
//! plausible sketches (rectangles, polygons, line-arc-line chains, slots,
//! circles; random relations and dimensions near the drawn values, some
//! redundant or conflicting), each built the same way in ours and as
//! Onshape REST JSON (BTMSketch entities and constraints, lengths in metres)
//! for live to post. A batch file (`fixtures/live_fuzz/sketch-N.json`) holds
//! the cases with our answers; live's answers come back as
//! `sketch-N.live.json` and `live_matches` compares them (T2).
//!
//! Write batch n: `LIVE_FUZZ_WRITE=<n> cargo test -p noshape-sketch --test all live_fuzz::write_batch -- --ignored`.
use noshape_sketch::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }
    fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.next()
    }
    fn below(&mut self, n: usize) -> usize {
        ((self.next() * n as f64) as usize).min(n - 1)
    }
    fn chance(&mut self, p: f64) -> bool {
        self.next() < p
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Line,
    Arc,
    Circle,
    /// Fit spline or conic (named endpoints only).
    Free,
}

/// One case built in both forms.
struct Case {
    sk: Sketch,
    /// Named points (`l1.start`, `a2.center`) -> ours.
    points: Vec<(String, EntityId)>,
    /// Named curves -> ours, kind.
    curves: Vec<(String, EntityId, Kind)>,
    /// Constraint names -> ours (None: ours refused it).
    cons: Vec<(String, Option<ConstraintId>)>,
    /// What each constraint is (for the reader).
    spec: Vec<Value>,
    os_entities: Vec<Value>,
    os_constraints: Vec<Value>,
    /// Generator version 2 (batch 3 on, seeds from 201): splines in the form
    /// Onshape solves (live 1772), no relation repeated.
    v2: bool,
}

const MM: f64 = 1e-3;

fn s(id: &str, v: &str) -> Value {
    json!({"btType": "BTMParameterString-149", "parameterId": id, "value": v})
}
fn q(id: &str, mm: f64) -> Value {
    json!({"btType": "BTMParameterQuantity-147", "parameterId": id, "expression": format!("{mm:.6} mm"), "isInteger": false, "value": 0, "units": ""})
}
fn dir(v: &str) -> Value {
    json!({"btType": "BTMParameterEnum-145", "parameterId": "direction", "enumName": "DimensionDirection", "value": v})
}

impl Case {
    fn new() -> Case {
        Case { sk: Sketch::new(), points: Vec::new(), curves: Vec::new(), cons: Vec::new(), spec: Vec::new(), os_entities: Vec::new(), os_constraints: Vec::new(), v2: false }
    }
    fn point(&self, name: &str) -> EntityId {
        self.points.iter().find(|(n, _)| n == name).unwrap().1
    }
    fn curve(&self, name: &str) -> EntityId {
        self.curves.iter().find(|(n, _, _)| n == name).unwrap().1
    }
    fn pos(&self, name: &str) -> DVec2 {
        self.sk.point(self.point(name)).unwrap()
    }
    fn line(&mut self, a: DVec2, b: DVec2) -> String {
        let id = format!("l{}", self.curves.len() + 1);
        let e = self.sk.add_line(a * MM, b * MM);
        let Some(Geometry::Line { start, end }) = self.sk.entity(e).map(|x| x.geometry.clone()) else { unreachable!() };
        self.points.push((format!("{id}.start"), start));
        self.points.push((format!("{id}.end"), end));
        self.curves.push((id.clone(), e, Kind::Line));
        let d = (b - a) * MM;
        let l = d.length();
        self.os_entities.push(json!({"btType": "BTMSketchCurveSegment-155", "entityId": id, "startPointId": format!("{id}.start"), "endPointId": format!("{id}.end"), "startParam": 0.0, "endParam": l,
            "geometry": {"btType": "BTCurveGeometryLine-117", "pntX": a.x * MM, "pntY": a.y * MM, "dirX": d.x / l, "dirY": d.y / l}}));
        id
    }
    /// CCW arc from angle `a0` to `a1` (radians).
    fn arc(&mut self, c: DVec2, r: f64, a0: f64, a1: f64) -> String {
        let id = format!("a{}", self.curves.len() + 1);
        let e = self.sk.add_arc_angles(c * MM, r * MM, a0, a1);
        let Some(Geometry::Arc { center, start, end }) = self.sk.entity(e).map(|x| x.geometry.clone()) else { unreachable!() };
        self.points.push((format!("{id}.start"), start));
        self.points.push((format!("{id}.end"), end));
        self.points.push((format!("{id}.center"), center));
        self.curves.push((id.clone(), e, Kind::Arc));
        let a1 = if a1 <= a0 { a1 + std::f64::consts::TAU } else { a1 };
        self.os_entities.push(json!({"btType": "BTMSketchCurveSegment-155", "entityId": id, "startPointId": format!("{id}.start"), "endPointId": format!("{id}.end"), "centerId": format!("{id}.center"), "startParam": a0, "endParam": a1,
            "geometry": {"btType": "BTCurveGeometryCircle-115", "radius": r * MM, "xCenter": c.x * MM, "yCenter": c.y * MM, "xDir": 1.0, "yDir": 0.0, "clockwise": false}}));
        id
    }
    /// Open interpolating spline through `pts` (natural ends).
    fn spline(&mut self, pts: &[DVec2]) -> String {
        let id = format!("p{}", self.curves.len() + 1);
        let mm: Vec<DVec2> = pts.iter().map(|p| *p * MM).collect();
        let e = self.sk.add_fit_spline(&mm).unwrap();
        let (a, b) = self.sk.endpoints(e).unwrap();
        self.points.push((format!("{id}.start"), a));
        self.points.push((format!("{id}.end"), b));
        self.curves.push((id.clone(), e, Kind::Free));
        let flat: Vec<f64> = mm.iter().flat_map(|p| [p.x, p.y]).collect();
        if !self.v2 {
            self.os_entities.push(json!({"btType": "BTMSketchCurveSegment-155", "entityId": id, "startPointId": format!("{id}.start"), "endPointId": format!("{id}.end"), "startParam": 0.0, "endParam": 1.0,
                "geometry": {"btType": "BTCurveGeometryInterpolatedSpline-116", "isPeriodic": false, "interpolationPoints": flat, "startDerivativeX": 0.0, "startDerivativeY": 0.0, "endDerivativeX": 0.0, "endDerivativeY": 0.0, "startHandleX": 0.0, "startHandleY": 0.0, "endHandleX": 0.0, "endHandleY": 0.0, "derivatives": {}}}));
            return id;
        }
        // Live 1772's working form: internal point ids, the point count, the
        // ends tied to the first / last internal points, and real end
        // derivatives (ours: the natural ends', per unit of a 0–1 parameter).
        let (d0, d1) = match self.sk.curve(e) {
            Some(Curve2::Nurbs(nb)) => {
                let (a, b) = nb.domain();
                (nb.eval_ders(a)[1] * (b - a), nb.eval_ders(b)[1] * (b - a))
            }
            _ => (DVec2::X, DVec2::X),
        };
        let n = mm.len();
        let mut internal: Vec<String> = (0..n).map(|k| format!("{id}.{k}.internal")).collect();
        internal.push(format!("{id}.startHandle"));
        internal.push(format!("{id}.endHandle"));
        self.os_entities.push(json!({"btType": "BTMSketchCurveSegment-155", "entityId": id, "startPointId": format!("{id}.start"), "endPointId": format!("{id}.end"), "startParam": 0.0, "endParam": 1.0,
            "internalIds": internal,
            "parameters": [
                {"btType": "BTMParameterBoolean-144", "parameterId": "geometryIsPeriodic", "value": false},
                {"btType": "BTMParameterQuantity-147", "parameterId": "splinePointCount", "expression": format!("{n}.0"), "isInteger": false, "value": 0, "units": ""}
            ],
            "geometry": {"btType": "BTCurveGeometryInterpolatedSpline-116", "isPeriodic": false, "interpolationPoints": flat, "startDerivativeX": d0.x, "startDerivativeY": d0.y, "endDerivativeX": d1.x, "endDerivativeY": d1.y, "startHandleX": 0.0, "startHandleY": 0.0, "endHandleX": 0.0, "endHandleY": 0.0, "derivatives": {}}}));
        for (end, k) in [("start", 0), ("end", n - 1)] {
            self.os_constraints.push(json!({"btType": "BTMSketchConstraint-2", "constraintType": "COINCIDENT", "entityId": format!("{id}.{end}.tie"), "parameters": [s("localFirst", &format!("{id}.{end}")), s("localSecond", &format!("{id}.{k}.internal"))]}));
        }
        id
    }
    fn conic(&mut self, a: DVec2, ctrl: DVec2, b: DVec2, rho: f64) -> String {
        let id = format!("q{}", self.curves.len() + 1);
        let e = self.sk.add_conic(a * MM, ctrl * MM, b * MM, rho).unwrap();
        let (pa, pb) = self.sk.endpoints(e).unwrap();
        self.points.push((format!("{id}.start"), pa));
        self.points.push((format!("{id}.end"), pb));
        self.curves.push((id.clone(), e, Kind::Free));
        self.os_entities.push(json!({"btType": "BTMSketchCurveSegment-155", "entityId": id, "startPointId": format!("{id}.start"), "endPointId": format!("{id}.end"), "startParam": 0.0, "endParam": 1.0,
            "geometry": {"btType": "BTCurveGeometryConic-2284", "rho": rho, "points": [a.x * MM, a.y * MM, ctrl.x * MM, ctrl.y * MM, b.x * MM, b.y * MM]}}));
        id
    }
    /// Angle from line `a` to line `b`, counter-clockwise (degrees).
    fn angle(&mut self, a: &str, b: &str, deg: f64) {
        let (x, y) = (self.curve(a), self.curve(b));
        self.con(ConstraintKind::Angle { a: x, b: y, value: deg.to_radians() }, "ANGLE", vec![s("localFirst", a), s("localSecond", b),
            json!({"btType": "BTMParameterQuantity-147", "parameterId": "angle", "expression": format!("{deg:.6} deg"), "isInteger": false, "value": 0, "units": ""}),
            json!({"btType": "BTMParameterBoolean-144", "parameterId": "clockwise", "value": false}),
            json!({"btType": "BTMParameterBoolean-144", "parameterId": "aligned", "value": true}),
            json!({"btType": "BTMParameterBoolean-144", "parameterId": "flipped", "value": false})], json!({"first": a, "second": b, "deg": deg}));
    }
    fn circle(&mut self, c: DVec2, r: f64) -> String {
        let id = format!("c{}", self.curves.len() + 1);
        let e = self.sk.add_circle(c * MM, r * MM);
        let Some(Geometry::Circle { center, .. }) = self.sk.entity(e).map(|x| x.geometry.clone()) else { unreachable!() };
        self.points.push((format!("{id}.center"), center));
        self.curves.push((id.clone(), e, Kind::Circle));
        self.os_entities.push(json!({"btType": "BTMSketchCurve-4", "entityId": id, "centerId": format!("{id}.center"),
            "geometry": {"btType": "BTCurveGeometryCircle-115", "radius": r * MM, "xCenter": c.x * MM, "yCenter": c.y * MM, "xDir": 1.0, "yDir": 0.0, "clockwise": false}}));
        id
    }
    /// A constraint in both forms: ours (`kind`), Onshape's type and parameters.
    fn con(&mut self, kind: ConstraintKind, os_type: &str, params: Vec<Value>, spec: Value) {
        let name = format!("k{}", self.cons.len() + 1);
        let ours = self.sk.add_constraint(kind).ok();
        self.cons.push((name.clone(), ours));
        let mut sp = spec;
        sp["id"] = json!(name);
        sp["type"] = json!(os_type);
        self.spec.push(sp);
        self.os_constraints.push(json!({"btType": "BTMSketchConstraint-2", "constraintType": os_type, "entityId": name, "parameters": params}));
    }
    /// Removes constraints from the newest back while the sketch is
    /// over-defined (v2 batches; the shapes' own constraints never are).
    fn drop_overdefining(&mut self) {
        loop {
            let mut probe = self.sk.clone();
            let rep = probe.solve();
            let Some(&last) = rep.conflicting().iter().max() else { return };
            let Some(k) = self.cons.iter().position(|(_, o)| *o == Some(last)) else { return };
            self.sk.remove_constraint(last).unwrap();
            self.cons.remove(k);
            self.spec.remove(k);
            // Onshape constraints: the spline ties are extra entries; find by id.
            let name = format!("k{}", k + 1);
            if let Some(i) = self.os_constraints.iter().position(|x| x["entityId"] == name.as_str()) {
                self.os_constraints.remove(i);
            }
            // Keep the k-numbering dense: rename the later ones.
            for (j, entry) in self.cons.iter_mut().enumerate().skip(k) {
                entry.0 = format!("k{}", j + 1);
            }
            for (j, sp) in self.spec.iter_mut().enumerate().skip(k) {
                sp["id"] = json!(format!("k{}", j + 1));
            }
            let mut j = 0;
            for oc in self.os_constraints.iter_mut() {
                if oc["entityId"].as_str().is_some_and(|x| x.starts_with('k')) {
                    j += 1;
                    oc["entityId"] = json!(format!("k{j}"));
                }
            }
        }
    }

    fn coincident(&mut self, a: &str, b: &str) {
        let (x, y) = (self.point(a), self.point(b));
        self.con(ConstraintKind::Coincident { a: x, b: y }, "COINCIDENT", vec![s("localFirst", a), s("localSecond", b)], json!({"first": a, "second": b}));
    }
    fn on_curve(&mut self, p: &str, c: &str) {
        if self.v2 && self.spec.iter().any(|k| k["type"] == "COINCIDENT" && k["first"] == p && k["second"] == c) {
            return;
        }
        let (x, y) = (self.point(p), self.curve(c));
        self.con(ConstraintKind::Coincident { a: x, b: y }, "COINCIDENT", vec![s("localFirst", p), s("localSecond", c)], json!({"first": p, "second": c}));
    }
    fn pair(&mut self, os: &str, a: &str, b: &str) {
        // v2: a relation already there (either order) isn't added again.
        if self.v2 && self.spec.iter().any(|k| k["type"] == os && ((k["first"] == a && k["second"] == b) || (k["first"] == b && k["second"] == a))) {
            return;
        }
        let (x, y) = (self.curve(a), self.curve(b));
        let kind = match os {
            "PARALLEL" => ConstraintKind::Parallel { a: x, b: y },
            "PERPENDICULAR" => ConstraintKind::Perpendicular { a: x, b: y },
            "EQUAL" => ConstraintKind::Equal { a: x, b: y },
            "TANGENT" => ConstraintKind::Tangent { a: x, b: y },
            "CONCENTRIC" => ConstraintKind::Concentric { a: x, b: y },
            _ => unreachable!(),
        };
        self.con(kind, os, vec![s("localFirst", a), s("localSecond", b)], json!({"first": a, "second": b}));
    }
    fn hv(&mut self, horizontal: bool, l: &str) {
        let x = self.curve(l);
        let (kind, os) = if horizontal { (ConstraintKind::Horizontal { a: x, b: None }, "HORIZONTAL") } else { (ConstraintKind::Vertical { a: x, b: None }, "VERTICAL") };
        self.con(kind, os, vec![s("localFirst", l)], json!({"first": l}));
    }
    fn length(&mut self, l: &str, mm: f64) {
        let x = self.curve(l);
        self.con(ConstraintKind::Length { curve: x, value: mm * MM }, "LENGTH", vec![s("localFirst", l), dir("MINIMUM"), q("length", mm)], json!({"first": l, "mm": mm}));
    }
    /// Distance between two named points, or a point and a line.
    fn distance(&mut self, a: &str, b: &str, mm: f64) {
        let x = self.points.iter().find(|(n, _)| n == a).map(|p| p.1).unwrap_or_else(|| self.curve(a));
        let y = self.points.iter().find(|(n, _)| n == b).map(|p| p.1).unwrap_or_else(|| self.curve(b));
        self.con(ConstraintKind::Distance { a: x, b: y, value: mm * MM, mode: DistanceMode::Center }, "DISTANCE", vec![s("localFirst", a), s("localSecond", b), dir("MINIMUM"), q("length", mm)], json!({"first": a, "second": b, "mm": mm}));
    }
    fn radius(&mut self, c: &str, mm: f64, diameter: bool) {
        let x = self.curve(c);
        let (kind, os, v) = if diameter { (ConstraintKind::Diameter { curve: x, value: 2.0 * mm * MM }, "DIAMETER", 2.0 * mm) } else { (ConstraintKind::Radius { curve: x, value: mm * MM }, "RADIUS", mm) };
        self.con(kind, os, vec![s("localFirst", c), q("length", v)], json!({"first": c, "mm": v}));
    }
}

/// A random plausible sketch; `rich` adds splines, conics and angles (batch 2 on).
fn generate(seed: u64, rich: bool) -> Case {
    let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03);
    for _ in 0..4 {
        r.next();
    }
    let mut c = Case::new();
    c.v2 = seed >= 201;
    let shapes = 1 + r.below(3);
    for k in 0..shapes {
        let o = DVec2::new(k as f64 * 60.0 + r.range(-5.0, 5.0), r.range(-5.0, 5.0));
        match r.below(if rich { 7 } else { 5 }) {
            // Rectangle: 4 lines, corners coincident, some sides horizontal / vertical.
            0 => {
                let (w, h) = (r.range(10.0, 40.0), r.range(8.0, 30.0));
                let tilt = if r.chance(0.3) { r.range(-0.08, 0.08) } else { 0.0 };
                let p: Vec<DVec2> = [DVec2::ZERO, DVec2::new(w, 0.0), DVec2::new(w, h), DVec2::new(0.0, h)].iter().map(|v| o + DVec2::from_angle(tilt).rotate(*v)).collect();
                let l: Vec<String> = (0..4).map(|i| c.line(p[i], p[(i + 1) % 4])).collect();
                for i in 0..4 {
                    c.coincident(&format!("{}.end", l[i]), &format!("{}.start", l[(i + 1) % 4]));
                }
                for (i, line) in l.iter().enumerate() {
                    if r.chance(0.7) {
                        c.hv(i % 2 == 0, line);
                    }
                }
            }
            // Polygon of 3–5 sides.
            1 => {
                let n = 3 + r.below(3);
                let rad = r.range(10.0, 25.0);
                let p: Vec<DVec2> = (0..n).map(|i| o + DVec2::from_angle(std::f64::consts::TAU * i as f64 / n as f64 + r.range(-0.2, 0.2)) * rad * r.range(0.8, 1.2)).collect();
                let l: Vec<String> = (0..n).map(|i| c.line(p[i], p[(i + 1) % n])).collect();
                for i in 0..n {
                    c.coincident(&format!("{}.end", l[i]), &format!("{}.start", l[(i + 1) % n]));
                }
            }
            // Line, tangent arc, line.
            2 => {
                let rad = r.range(5.0, 15.0);
                let l1 = c.line(o + DVec2::new(-20.0, 0.0), o);
                let a = c.arc(o + DVec2::new(0.0, rad), rad, -std::f64::consts::FRAC_PI_2, 0.0);
                let l2 = c.line(o + DVec2::new(rad, rad), o + DVec2::new(rad, rad + 20.0));
                c.coincident(&format!("{l1}.end"), &format!("{a}.start"));
                c.coincident(&format!("{a}.end"), &format!("{l2}.start"));
                c.pair("TANGENT", &l1, &a);
                c.pair("TANGENT", &a, &l2);
            }
            // Slot: two lines and two arcs, tangent.
            3 => {
                let (len, rad) = (r.range(15.0, 35.0), r.range(4.0, 10.0));
                let l1 = c.line(o + DVec2::new(0.0, -rad), o + DVec2::new(len, -rad));
                let a1 = c.arc(o + DVec2::new(len, 0.0), rad, -std::f64::consts::FRAC_PI_2, std::f64::consts::FRAC_PI_2);
                let l2 = c.line(o + DVec2::new(len, rad), o + DVec2::new(0.0, rad));
                let a2 = c.arc(o, rad, std::f64::consts::FRAC_PI_2, 3.0 * std::f64::consts::FRAC_PI_2);
                c.coincident(&format!("{l1}.end"), &format!("{a1}.start"));
                c.coincident(&format!("{a1}.end"), &format!("{l2}.start"));
                c.coincident(&format!("{l2}.end"), &format!("{a2}.start"));
                c.coincident(&format!("{a2}.end"), &format!("{l1}.start"));
                for (x, y) in [(&l1, &a1), (&a1, &l2), (&l2, &a2), (&a2, &l1)] {
                    c.pair("TANGENT", x, y);
                }
            }
            // Fit spline over a horizontal line closing it.
            5 => {
                let (w, h) = (r.range(15.0, 35.0), r.range(5.0, 15.0));
                let n = 3 + r.below(2);
                let pts: Vec<DVec2> = (0..n).map(|i| {
                    let t = i as f64 / (n - 1) as f64;
                    o + DVec2::new(w * t, if i == 0 || i == n - 1 { 0.0 } else { h * r.range(0.6, 1.2) })
                }).collect();
                let p = c.spline(&pts);
                let l = c.line(o + DVec2::new(w, 0.0), o);
                c.coincident(&format!("{p}.end"), &format!("{l}.start"));
                c.coincident(&format!("{l}.end"), &format!("{p}.start"));
                if r.chance(0.7) {
                    c.hv(true, &l);
                }
            }
            // Conic closed by a line.
            6 => {
                let (w, h) = (r.range(15.0, 35.0), r.range(5.0, 15.0));
                let q = c.conic(o, o + DVec2::new(w * r.range(0.3, 0.7), h), o + DVec2::new(w, 0.0), (r.range(0.3, 0.7) * 100.0).round() / 100.0);
                let l = c.line(o + DVec2::new(w, 0.0), o);
                c.coincident(&format!("{q}.end"), &format!("{l}.start"));
                c.coincident(&format!("{l}.end"), &format!("{q}.start"));
                if r.chance(0.7) {
                    c.hv(true, &l);
                }
            }
            // Circle, sometimes with a concentric one.
            _ => {
                let rad = r.range(5.0, 15.0);
                let c1 = c.circle(o, rad);
                if r.chance(0.5) {
                    let c2 = c.circle(o + DVec2::new(r.range(-1.0, 1.0), r.range(-1.0, 1.0)), rad * r.range(0.3, 0.7));
                    c.pair("CONCENTRIC", &c1, &c2);
                }
            }
        }
    }
    let lines: Vec<String> = c.curves.iter().filter(|x| x.2 == Kind::Line).map(|x| x.0.clone()).collect();
    let round: Vec<String> = c.curves.iter().filter(|x| matches!(x.2, Kind::Arc | Kind::Circle)).map(|x| x.0.clone()).collect();
    let pts: Vec<String> = c.points.iter().map(|x| x.0.clone()).collect();
    // Relations the drawing nearly has already (a designer's next click):
    // Parallel / Perpendicular for lines within 15° of it, Equal for lengths
    // or radii within 30 %, a point onto a line it is within 3 mm of.
    let dir_of = |c: &Case, l: &str| (c.pos(&format!("{l}.end")) - c.pos(&format!("{l}.start"))).normalize();
    let len_of = |c: &Case, l: &str| (c.pos(&format!("{l}.end")) - c.pos(&format!("{l}.start"))).length();
    let rad_of = |c: &Case, k: &str| match c.sk.entity(c.curve(k)).map(|e| e.geometry.clone()) {
        Some(Geometry::Circle { radius, .. }) => radius,
        Some(Geometry::Arc { center, start, .. }) => (c.sk.point(start).unwrap() - c.sk.point(center).unwrap()).length(),
        _ => unreachable!(),
    };
    for _ in 0..12 {
        if c.cons.len() >= 60 {
            break;
        }
        match r.below(4) {
            0 | 1 if lines.len() >= 2 => {
                let (a, b) = (lines[r.below(lines.len())].clone(), lines[r.below(lines.len())].clone());
                if a == b {
                    continue;
                }
                let cos = dir_of(&c, &a).dot(dir_of(&c, &b)).abs();
                let ratio = len_of(&c, &a) / len_of(&c, &b);
                if rich && cos < 0.9 && cos > 0.1 && r.chance(0.5) {
                    // The drawn angle from a to b (counter-clockwise, a and b
                    // swapped when it is negative), give or take 10 %.
                    let (u, v) = (dir_of(&c, &a), dir_of(&c, &b));
                    let th = u.perp_dot(v).atan2(u.dot(v));
                    let (a, b, th) = if th < 0.0 { (b, a, -th) } else { (a, b, th) };
                    c.angle(&a, &b, (th.to_degrees() * r.range(0.9, 1.1) * 1000.0).round() / 1000.0);
                } else if cos > 15f64.to_radians().cos() && r.chance(0.5) {
                    c.pair("PARALLEL", &a, &b);
                } else if cos < 15f64.to_radians().sin() && r.chance(0.5) {
                    c.pair("PERPENDICULAR", &a, &b);
                } else if (0.77..1.3).contains(&ratio) {
                    c.pair("EQUAL", &a, &b);
                }
            }
            2 if round.len() >= 2 => {
                let (a, b) = (round[r.below(round.len())].clone(), round[r.below(round.len())].clone());
                if a != b && (0.77..1.3).contains(&(rad_of(&c, &a) / rad_of(&c, &b))) {
                    c.pair("EQUAL", &a, &b);
                }
            }
            _ if !lines.is_empty() && pts.len() > 2 => {
                let (p, l) = (pts[r.below(pts.len())].clone(), lines[r.below(lines.len())].clone());
                let (a, b, x) = (c.pos(&format!("{l}.start")), c.pos(&format!("{l}.end")), c.pos(&p));
                let off = (x - a).perp_dot((b - a).normalize()).abs() / MM;
                if !p.starts_with(&format!("{l}.")) && off < 3.0 {
                    c.on_curve(&p, &l);
                }
            }
            _ => {}
        }
    }
    // Dimensions near the drawn values (one in five a second, conflicting one).
    for _ in 0..1 + r.below(5) {
        let f = r.range(0.8, 1.25);
        match r.below(3) {
            0 if !lines.is_empty() => {
                let l = lines[r.below(lines.len())].clone();
                let v = (c.pos(&format!("{l}.end")) - c.pos(&format!("{l}.start"))).length() / MM;
                c.length(&l, (v * f * 1000.0).round() / 1000.0);
            }
            1 if !round.is_empty() => {
                let k = round[r.below(round.len())].clone();
                let rad = match c.sk.entity(c.curve(&k)).map(|e| e.geometry.clone()) {
                    Some(Geometry::Circle { radius, .. }) => radius / MM,
                    Some(Geometry::Arc { center, start, .. }) => (c.sk.point(start).unwrap() - c.sk.point(center).unwrap()).length() / MM,
                    _ => unreachable!(),
                };
                let d = r.chance(0.5);
                c.radius(&k, (rad * f * 1000.0).round() / 1000.0, d);
            }
            _ if pts.len() >= 2 => {
                let (a, b) = (pts[r.below(pts.len())].clone(), pts[r.below(pts.len())].clone());
                let d = (c.pos(&a) - c.pos(&b)).length() / MM;
                if d > 1.0 {
                    c.distance(&a, &b, (d * f * 1000.0).round() / 1000.0);
                }
            }
            _ => {}
        }
    }
    // v2: a relation or random dimension that over-defines is taken back
    // (the batch measures least change; the deliberate one below stays).
    if c.v2 {
        c.drop_overdefining();
    }
    // One case in four over-defines on purpose: a second dimension on a
    // dimensioned line or curve, at a different value (or the same: redundant).
    if r.chance(0.25) {
        let dimensioned: Vec<Value> = c.spec.iter().filter(|x| matches!(x["type"].as_str(), Some("LENGTH" | "RADIUS" | "DIAMETER"))).cloned().collect();
        if !dimensioned.is_empty() {
            let d = &dimensioned[r.below(dimensioned.len())];
            let (k, mm) = (d["first"].as_str().unwrap().to_string(), d["mm"].as_f64().unwrap());
            let v = if r.chance(0.5) { mm } else { (mm * r.range(0.85, 1.15) * 1000.0).round() / 1000.0 };
            match d["type"].as_str() {
                Some("LENGTH") => c.length(&k, v),
                Some("RADIUS") => c.radius(&k, v, false),
                _ => c.radius(&k, v / 2.0, true),
            }
        }
    }
    c
}

/// Ours: the solve's status, each constraint's state, DOF, each curve's
/// state (Onshape's black / blue / red), the solved points and radii (mm).
fn ours(c: &mut Case) -> Value {
    // As a regeneration solves a sketch (live posts it whole).
    let rep = c.sk.solve_with(&SolveOptions { constructive: true, ..Default::default() });
    let name_of = |id: ConstraintId| c.cons.iter().find(|(_, o)| *o == Some(id)).map(|(n, _)| n.clone());
    let conflicting: Vec<String> = rep.conflicting().iter().filter_map(|id| name_of(*id)).collect();
    let redundant: Vec<String> = rep.redundant.iter().filter_map(|id| name_of(*id)).collect();
    let refused: Vec<String> = c.cons.iter().filter(|(_, o)| o.is_none()).map(|(n, _)| n.clone()).collect();
    let state = |s: Option<&ConstraintState>| match s {
        Some(ConstraintState::FullyConstrained) => "fully",
        Some(ConstraintState::UnderConstrained) => "under",
        Some(ConstraintState::OverConstrained) => "over",
        None => "?",
    };
    let curves: BTreeMap<String, &str> = c.curves.iter().map(|(n, id, _)| (n.clone(), state(rep.entity_states.get(id)))).collect();
    let points = ours_points(c);
    let radii: BTreeMap<String, f64> = c.curves.iter().filter_map(|(n, id, k)| match (k, c.sk.entity(*id).map(|e| e.geometry.clone())) {
        (Kind::Circle, Some(Geometry::Circle { radius, .. })) => Some((n.clone(), (radius / MM * 1e6).round() / 1e6)),
        _ => None,
    }).collect();
    let status = match &rep.status {
        SolveStatus::Solved => "solved".to_string(),
        SolveStatus::Overconstrained { .. } => "overconstrained".to_string(),
        SolveStatus::Failed { reason } => format!("failed: {reason}"),
    };
    json!({"status": status, "conflicting": conflicting, "redundant": redundant, "refused": refused, "dof": rep.dof, "curves": curves, "points": points, "radii": radii})
}

/// The named points' positions (mm, 6 decimals).
fn ours_points(c: &Case) -> BTreeMap<String, [f64; 2]> {
    c.points.iter().map(|(n, id)| {
        let p = c.sk.point(*id).unwrap() / MM;
        (n.clone(), [(p.x * 1e6).round() / 1e6, (p.y * 1e6).round() / 1e6])
    }).collect()
}

fn batch(n: usize, first_seed: u64, rich: bool) -> Value {
    let cases: Vec<Value> = (0..n as u64).map(|k| {
        let seed = first_seed + k;
        let mut c = generate(seed, rich);
        let ours = ours(&mut c);
        // A drag on solved cases: the first line's start (or a curve's
        // center) moved 5 mm in X and 3 mm in Y, as one pointer drag.
        let drag = (ours["status"] == "solved").then(|| {
            let name = c.points.iter().map(|p| p.0.clone()).find(|n| n.ends_with(".start")).unwrap_or_else(|| c.points[0].0.clone());
            let to = c.pos(&name) / MM + DVec2::new(5.0, 3.0);
            let mut d = Case { sk: c.sk.clone(), points: c.points.clone(), curves: c.curves.clone(), cons: c.cons.clone(), spec: Vec::new(), os_entities: Vec::new(), os_constraints: Vec::new(), v2: c.v2 };
            d.sk.drag(&[(DragHandle::Point(d.point(&name)), to * MM)]);
            let after = ours_points(&d);
            json!({"point": name, "to": [to.x, to.y], "ours": after})
        });
        json!({"case": format!("s{seed}"), "seed": seed, "constraints": c.spec, "onshape": {"entities": c.os_entities, "constraints": c.os_constraints}, "ours": ours, "drag": drag})
    }).collect();
    json!({"units": "mm (Onshape JSON: metres, radians)", "plane": "Top", "cases": cases})
}

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/live_fuzz").join(name)
}

/// Writes batch 1 (seeds 1–20) when LIVE_FUZZ_WRITE is set; otherwise checks
/// the stored batch still has ours' answers (the generator is deterministic).
#[test]
#[ignore = "T2: live fuzz batch"]
fn write_batch() {
    for (n, first, rich) in BATCHES {
        check_batch(n, first, rich);
    }
}

/// The batches: (number, first seed, rich).
const BATCHES: [(usize, u64, bool); 3] = [(1, 1, false), (2, 101, true), (3, 201, true)];

/// Cases in batch `n` (batch 3 on: 40, where positions match least).
fn batch_size(n: usize) -> usize {
    if n >= 3 { 40 } else { 20 }
}

/// Batch `n` (20 seeds from `first`): written when LIVE_FUZZ_WRITE is set,
/// else checked unchanged.
fn check_batch(n: usize, first: u64, rich: bool) {
    let b = batch(batch_size(n), first, rich);
    let path = fixture(&format!("sketch-{n}.json"));
    // LIVE_FUZZ_WRITE=<n>: (re)write batch n only (a batch sent to live stays as sent).
    if std::env::var("LIVE_FUZZ_WRITE").is_ok_and(|v| v == n.to_string()) {
        std::fs::write(&path, serde_json::to_string_pretty(&b).unwrap()).unwrap();
    }
    let stored: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    for (x, y) in b["cases"].as_array().unwrap().iter().zip(stored["cases"].as_array().unwrap()) {
        assert_eq!(x["onshape"], y["onshape"], "{}: the generator changed", x["case"]);
    }
    let count = b["cases"].as_array().unwrap().len();
    let solved = b["cases"].as_array().unwrap().iter().filter(|c| c["ours"]["status"] == "solved").count();
    println!("batch {}: {count} cases, {solved} solved by ours", path.display());
}

/// Differences of ours from live on one case (empty: a match). Positions
/// match within 1e-3 mm; a constraint is in error in both or neither; each
/// curve's state (fully / under / over) is the same where live gave one.
fn differences(case: &Value, live: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let ours = &case["ours"];
    let close = |a: &Value, b: &Value| (a[0].as_f64().unwrap() - b[0].as_f64().unwrap()).hypot(a[1].as_f64().unwrap() - b[1].as_f64().unwrap()) < 1e-3;
    if let Some(pts) = live["points"].as_object() {
        for (n, p) in pts {
            if !close(&ours["points"][n], p) {
                out.push(format!("{n}: ours {} live {p}", ours["points"][n]));
            }
        }
    }
    if let Some(radii) = live["radii"].as_object() {
        for (n, v) in radii {
            if (ours["radii"][n].as_f64().unwrap_or(f64::NAN) - v.as_f64().unwrap()).abs() >= 1e-3 {
                out.push(format!("{n} radius: ours {} live {v}", ours["radii"][n]));
            }
        }
    }
    let ours_red: Vec<&str> = ours["conflicting"].as_array().unwrap().iter().chain(ours["refused"].as_array().unwrap()).filter_map(|v| v.as_str()).collect();
    if let Some(cons) = live["constraints"].as_object() {
        let live_red: Vec<&str> = cons.iter().filter(|(_, m)| m.as_str() != Some("OK")).map(|(k, _)| k.as_str()).collect();
        let mut a = ours_red.clone();
        let mut b = live_red.clone();
        a.sort_unstable();
        b.sort_unstable();
        if a != b {
            out.push(format!("in error: ours {a:?} live {b:?}"));
        }
    }
    if let Some(curves) = live["curves"].as_object() {
        for (n, st) in curves {
            // "under  (pixel mix …)": the colour read under the label.
            let st = st.as_str().and_then(|x| x.split_whitespace().next()).unwrap_or("");
            if ours["curves"][n] != st {
                out.push(format!("{n}: ours {} live {st}", ours["curves"][n]));
            }
        }
    }
    out
}

/// Live's UI drag of a case against ours dragged to where live's point
/// ended (live's pointer lands within a pixel of the target, 0.2 mm).
fn drag_differences(seed: u64, rich: bool, case: &Value, live: &Value) -> Vec<String> {
    let (Some(name), Some(ld)) = (case["drag"]["point"].as_str(), live["drag"].as_object()) else { return Vec::new() };
    let Some(to) = ld.get(name) else { return Vec::new() };
    let mut c = generate(seed, rich);
    c.sk.solve_with(&SolveOptions { constructive: true, ..Default::default() });
    let target = DVec2::new(to[0].as_f64().unwrap(), to[1].as_f64().unwrap()) * MM;
    c.sk.drag(&[(DragHandle::Point(c.point(name)), target)]);
    let after = ours_points(&c);
    ld.iter()
        .filter(|(n, p)| after.get(*n).is_none_or(|o| (o[0] - p[0].as_f64().unwrap()).hypot(o[1] - p[1].as_f64().unwrap()) >= 1e-3))
        .map(|(n, p)| format!("drag {n}: ours {:?} live {p}", after.get(n)))
        .collect()
}

/// Ours against live's answers (`sketch-N.live.json`, written by live:
/// `{"cases": {"s1": {"solved": bool, "points": {...}, "radii": {...}, "constraints": {...}, "curves": {...}, "drag": {...}}}}`).
/// Fails on a solvability difference (live "Sketch could not be solved."
/// against ours Solved, or the reverse); position differences are counted
/// and printed: under-defined geometry lands where each solver's least
/// change puts it, and live's rule is still being pinned down (handoff).
#[test]
#[ignore = "T2: live fuzz batch vs live"]
fn live_matches() {
    let (mut cases, mut status_ok, mut exact) = (0, 0, 0);
    let mut report = Vec::new();
    let mut positions = Vec::new();
    for (n, first, rich) in BATCHES {
        let Ok(live) = std::fs::read_to_string(fixture(&format!("sketch-{n}.live.json"))) else { continue };
        // Ours as the solver stands now (the batch file keeps ours as sent);
        // FUZZ_ONLY=<seed> for one case.
        let ours = match std::env::var("FUZZ_ONLY").ok().and_then(|v| v.parse::<u64>().ok()) {
            Some(seed) if (first..first + batch_size(n) as u64).contains(&seed) => batch(1, seed, rich),
            Some(_) => continue,
            None => batch(batch_size(n), first, rich),
        };
        let live: Value = serde_json::from_str(&live).unwrap();
        for case in ours["cases"].as_array().unwrap() {
            let name = case["case"].as_str().unwrap();
            let Some(l) = live["cases"].get(name) else { continue };
            // Not an answer: Onshape refused the posted spline JSON's form
            // (batch 2, to be reposted).
            if l["message"].as_str().is_some_and(|m| m.contains("could not be updated after solve")) {
                continue;
            }
            cases += 1;
            let solved = case["ours"]["status"] == "solved";
            match l["solved"].as_bool() {
                Some(ls) if ls != solved => report.push(format!("{name}: ours {}, live {}", case["ours"]["status"], l["message"])),
                _ => status_ok += 1,
            }
            let mut d = differences(case, l);
            d.extend(drag_differences(case["seed"].as_u64().unwrap(), rich, case, l));
            if d.is_empty() {
                exact += 1;
            } else {
                positions.push(format!("{name} ({} differ{}): {}", d.len(), if l["solved"] == false { ", live unsolved" } else { "" }, d.join("; ")));
            }
        }
    }
    println!("live fuzz (sketch): {cases} cases answered, solvability matches {status_ok}, everything matches {exact}");
    for p in &positions {
        println!("  {}", p.chars().take(std::env::var("FUZZ_WIDE").map_or(300, |_| 4000)).collect::<String>());
    }
    assert!(report.is_empty(), "{}", report.join("\n"));
}

/// Probes of live's least change (after batch 1, where under-defined
/// geometry landed elsewhere in ours): each a minimal sketch with one
/// constraint or dimension the drawing doesn't meet, so what moves shows
/// live's rule.
fn probes() -> Vec<(String, Case)> {
    let v = DVec2::new;
    let mut out = Vec::new();
    let mut add = |name: &str, f: &dyn Fn(&mut Case)| {
        let mut c = Case::new();
        f(&mut c);
        out.push((name.to_string(), c));
    };
    add("p1 lone line, length 20 -> 15", &|c| {
        let l = c.line(v(0.0, 0.0), v(20.0, 0.0));
        c.length(&l, 15.0);
    });
    add("p2 lone horizontal line, length 20 -> 15", &|c| {
        let l = c.line(v(0.0, 0.0), v(20.0, 0.0));
        c.hv(true, &l);
        c.length(&l, 15.0);
    });
    add("p3 L, first line 20 -> 15", &|c| {
        let a = c.line(v(0.0, 0.0), v(20.0, 0.0));
        let b = c.line(v(20.0, 0.0), v(20.0, 10.0));
        c.coincident(&format!("{a}.end"), &format!("{b}.start"));
        c.length(&a, 15.0);
    });
    add("p4 L, second line 10 -> 5", &|c| {
        let a = c.line(v(0.0, 0.0), v(20.0, 0.0));
        let b = c.line(v(20.0, 0.0), v(20.0, 10.0));
        c.coincident(&format!("{a}.end"), &format!("{b}.start"));
        c.length(&b, 5.0);
    });
    add("p5 concentric(big first, small second), big created first", &|c| {
        let a = c.circle(v(0.0, 0.0), 10.0);
        let b = c.circle(v(1.0, 1.0), 4.0);
        c.pair("CONCENTRIC", &a, &b);
    });
    add("p6 concentric(small, big), big created first", &|c| {
        let a = c.circle(v(0.0, 0.0), 10.0);
        let b = c.circle(v(1.0, 1.0), 4.0);
        c.pair("CONCENTRIC", &b, &a);
    });
    add("p7 concentric(small first, big second), small created first", &|c| {
        let a = c.circle(v(0.0, 0.0), 4.0);
        let b = c.circle(v(1.0, 1.0), 10.0);
        c.pair("CONCENTRIC", &a, &b);
    });
    add("p8 concentric, the second circle dimensioned", &|c| {
        let a = c.circle(v(0.0, 0.0), 10.0);
        let b = c.circle(v(1.0, 1.0), 4.0);
        c.pair("CONCENTRIC", &a, &b);
        c.radius(&b, 4.0, false);
    });
    add("p9 rectangle with H / V, width 20 -> 25", &|c| {
        let p = [v(0.0, 0.0), v(20.0, 0.0), v(20.0, 10.0), v(0.0, 10.0)];
        let l: Vec<String> = (0..4).map(|i| c.line(p[i], p[(i + 1) % 4])).collect();
        for i in 0..4 {
            c.coincident(&format!("{}.end", l[i]), &format!("{}.start", l[(i + 1) % 4]));
        }
        for (i, x) in l.iter().enumerate() {
            c.hv(i % 2 == 0, x);
        }
        c.length(&l[0], 25.0);
    });
    add("p10 line, tangent arc, line: first line 20 -> 15", &|c| {
        let l1 = c.line(v(-20.0, 0.0), v(0.0, 0.0));
        let a = c.arc(v(0.0, 6.0), 6.0, -std::f64::consts::FRAC_PI_2, 0.0);
        let l2 = c.line(v(6.0, 6.0), v(6.0, 26.0));
        c.coincident(&format!("{l1}.end"), &format!("{a}.start"));
        c.coincident(&format!("{a}.end"), &format!("{l2}.start"));
        c.pair("TANGENT", &l1, &a);
        c.pair("TANGENT", &a, &l2);
        c.length(&l1, 15.0);
    });
    add("p11 parallel(first flat, second tilted)", &|c| {
        let a = c.line(v(0.0, 0.0), v(20.0, 0.0));
        let b = c.line(v(0.0, 5.0), v(20.0, 7.0));
        c.pair("PARALLEL", &a, &b);
    });
    add("p12 parallel(second tilted, first flat)", &|c| {
        let a = c.line(v(0.0, 0.0), v(20.0, 0.0));
        let b = c.line(v(0.0, 5.0), v(20.0, 7.0));
        c.pair("PARALLEL", &b, &a);
    });
    add("p13 equal(20 long, 10 long)", &|c| {
        let a = c.line(v(0.0, 0.0), v(20.0, 0.0));
        let b = c.line(v(0.0, 5.0), v(10.0, 5.0));
        c.pair("EQUAL", &a, &b);
    });
    add("p14 equal(10 long, 20 long)", &|c| {
        let a = c.line(v(0.0, 0.0), v(20.0, 0.0));
        let b = c.line(v(0.0, 5.0), v(10.0, 5.0));
        c.pair("EQUAL", &b, &a);
    });
    out
}

/// Second probes: structure (a line keeps its start, dangling geometry
/// moves) against position (values at zero kept, smaller kept): the first
/// probes' cases away from the origin, drawn the other way, created in the
/// other order.
fn probes2() -> Vec<(String, Case)> {
    let v = DVec2::new;
    let mut out = Vec::new();
    let mut add = |name: &str, f: &dyn Fn(&mut Case)| {
        let mut c = Case::new();
        f(&mut c);
        out.push((name.to_string(), c));
    };
    add("q1 lone line far from the origin (100,50)->(120,50), length 15", &|c| {
        let l = c.line(v(100.0, 50.0), v(120.0, 50.0));
        c.length(&l, 15.0);
    });
    add("q2 lone line drawn right to left (20,0)->(0,0), length 15", &|c| {
        let l = c.line(v(20.0, 0.0), v(0.0, 0.0));
        c.length(&l, 15.0);
    });
    add("q3 lone line (-20,0)->(0,0), length 15", &|c| {
        let l = c.line(v(-20.0, 0.0), v(0.0, 0.0));
        c.length(&l, 15.0);
    });
    add("q4 L far from the origin, first line 20 -> 15", &|c| {
        let a = c.line(v(100.0, 50.0), v(120.0, 50.0));
        let b = c.line(v(120.0, 50.0), v(120.0, 60.0));
        c.coincident(&format!("{a}.end"), &format!("{b}.start"));
        c.length(&a, 15.0);
    });
    add("q5 equal, the 10 mm line created first", &|c| {
        let a = c.line(v(0.0, 5.0), v(10.0, 5.0));
        let b = c.line(v(0.0, 0.0), v(20.0, 0.0));
        c.pair("EQUAL", &a, &b);
    });
    add("q6 equal far from the origin (20 long first)", &|c| {
        let a = c.line(v(100.0, 50.0), v(120.0, 50.0));
        let b = c.line(v(100.0, 55.0), v(110.0, 55.0));
        c.pair("EQUAL", &a, &b);
    });
    add("q7 parallel, the tilted line created first", &|c| {
        let a = c.line(v(0.0, 5.0), v(20.0, 7.0));
        let b = c.line(v(0.0, 0.0), v(20.0, 0.0));
        c.pair("PARALLEL", &a, &b);
    });
    add("q8 parallel, both tilted (5° first, 10° second)", &|c| {
        let a = c.line(v(0.0, 0.0), v(20.0, 20.0 * 5f64.to_radians().tan()));
        let b = c.line(v(0.0, 10.0), v(20.0, 10.0 + 20.0 * 10f64.to_radians().tan()));
        c.pair("PARALLEL", &a, &b);
    });
    add("q9 concentric far from the origin, (101,51) r10 then (100,50) r4", &|c| {
        let a = c.circle(v(101.0, 51.0), 10.0);
        let b = c.circle(v(100.0, 50.0), 4.0);
        c.pair("CONCENTRIC", &a, &b);
    });
    add("q10 rectangle away from the origin (100,50) 20 x 10, width -> 25", &|c| {
        let p = [v(100.0, 50.0), v(120.0, 50.0), v(120.0, 60.0), v(100.0, 60.0)];
        let l: Vec<String> = (0..4).map(|i| c.line(p[i], p[(i + 1) % 4])).collect();
        for i in 0..4 {
            c.coincident(&format!("{}.end", l[i]), &format!("{}.start", l[(i + 1) % 4]));
        }
        for (i, x) in l.iter().enumerate() {
            c.hv(i % 2 == 0, x);
        }
        c.length(&l[0], 25.0);
    });
    out
}

fn probe_batch(list: Vec<(String, Case)>) -> Value {
    let cases: Vec<Value> = list.into_iter().map(|(name, mut c)| {
        let ours = ours(&mut c);
        let id = name.split(' ').next().unwrap().to_string();
        json!({"case": id, "what": name, "constraints": c.spec, "onshape": {"entities": c.os_entities, "constraints": c.os_constraints}, "ours": ours})
    }).collect();
    json!({"units": "mm (Onshape JSON: metres, radians)", "plane": "Top", "cases": cases})
}

/// The least-change probes (`sketch-probe-N.json`, live's answers in
/// `sketch-probe-N.live.json`); written with LIVE_FUZZ_WRITE=probe-N.
#[test]
#[ignore = "T2: live fuzz least-change probes"]
fn probe_matches() {
    for (n, list) in [(1, probes()), (2, probes2())] {
        let b = probe_batch(list);
        let path = fixture(&format!("sketch-probe-{n}.json"));
        if std::env::var("LIVE_FUZZ_WRITE").is_ok_and(|v| v == format!("probe-{n}")) {
            std::fs::write(&path, serde_json::to_string_pretty(&b).unwrap()).unwrap();
        }
        let Ok(live) = std::fs::read_to_string(fixture(&format!("sketch-probe-{n}.live.json"))) else { continue };
        let live: Value = serde_json::from_str(&live).unwrap();
        let (mut total, mut ok) = (0, 0);
        for case in b["cases"].as_array().unwrap() {
            let Some(l) = live["cases"].get(case["case"].as_str().unwrap()) else { continue };
            total += 1;
            let d = differences(case, l);
            if d.is_empty() {
                ok += 1;
            } else {
                println!("  {} ({}): {}", case["case"], case["what"], d.join("; "));
            }
        }
        println!("live probes {n}: {total} answered, {ok} match");
    }
}
