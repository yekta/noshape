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
    /// Makes a curve construction geometry.
    fn construction(&mut self, c: &str) {
        let x = self.curve(c);
        self.sk.set_construction(x, true).unwrap();
        if let Some(e) = self.os_entities.iter_mut().find(|e| e["entityId"] == c) {
            e["isConstruction"] = json!(true);
        }
    }
    /// Points `a` and `b` symmetric about the line `axis`.
    fn symmetric(&mut self, a: &str, b: &str, axis: &str) {
        let (x, y, z) = (self.point(a), self.point(b), self.curve(axis));
        // (Onshape's type is MIRROR: live 1806.)
        self.con(ConstraintKind::Symmetric { a: x, b: y, axis: z }, "MIRROR", vec![s("localFirst", a), s("localSecond", b), s("localMirror", axis)], json!({"first": a, "second": b, "mirror": axis}));
    }
    /// Point `p` at the midpoint of line `l`.
    fn midpoint(&mut self, p: &str, l: &str) {
        let (x, y) = (self.point(p), self.curve(l));
        // (The UI's form, localEntity1 / localEntity2: live 1806.)
        self.con(ConstraintKind::Midpoint { point: x, curve: y }, "MIDPOINT", vec![s("localEntity1", p), s("localEntity2", l)], json!({"first": p, "second": l}));
    }
    fn radius(&mut self, c: &str, mm: f64, diameter: bool) {
        let x = self.curve(c);
        let (kind, os, v) = if diameter { (ConstraintKind::Diameter { curve: x, value: 2.0 * mm * MM }, "DIAMETER", 2.0 * mm) } else { (ConstraintKind::Radius { curve: x, value: mm * MM }, "RADIUS", mm) };
        self.con(kind, os, vec![s("localFirst", c), q("length", v)], json!({"first": c, "mm": v}));
    }
}

/// Batch 4 on (seeds 301 on): everyday profiles as a regeneration meets
/// them after a dimension's new value (a variable, a configuration): a
/// dimensioned corner rectangle, a filleted rectangle, a slot, an L-bracket,
/// a bolt circle; each drawn solved, then one or two of its dimensions set
/// 0.75–1.35 times what the drawing measures.
fn everyday(seed: u64) -> Case {
    use std::f64::consts::{FRAC_PI_2, PI};
    let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03);
    for _ in 0..4 {
        r.next();
    }
    let mut c = Case::new();
    c.v2 = true;
    let v = DVec2::new;
    let shapes = 1 + r.below(2);
    for k in 0..shapes {
        let o = v(k as f64 * 80.0 + r.range(-10.0, 10.0), r.range(-10.0, 10.0));
        // Each dimension at the drawing's value, or changed.
        let mut changes = 1 + r.below(2);
        let mut f = |r: &mut Rng| if changes > 0 && r.chance(0.6) {
            changes -= 1;
            r.range(0.75, 1.35)
        } else {
            1.0
        };
        // (Batch 6 on, seed 381: three more kinds; batch 7 on, seed 421,
        // three more; batch 8 on, seed 461, three more.)
        match r.below(if seed >= 461 { 14 } else if seed >= 421 { 11 } else if seed >= 381 { 8 } else { 5 }) {
            // Corner rectangle (live's tool constraints), width and height.
            0 => {
                let (w, h) = (r.range(15.0, 50.0), r.range(10.0, 35.0));
                let p = [o, o + v(w, 0.0), o + v(w, h), o + v(0.0, h)];
                let l: Vec<String> = (0..4).map(|i| c.line(p[i], p[(i + 1) % 4])).collect();
                for i in 0..4 {
                    c.coincident(&format!("{}.end", l[i]), &format!("{}.start", l[(i + 1) % 4]));
                }
                c.pair("PERPENDICULAR", &l[2], &l[3]);
                c.pair("PARALLEL", &l[0], &l[2]);
                c.pair("PARALLEL", &l[3], &l[1]);
                c.hv(true, &l[2]);
                let (fw, fh) = (f(&mut r), f(&mut r));
                c.length(&l[0], w * fw);
                if r.chance(0.8) {
                    c.length(&l[1], h * fh);
                }
            }
            // Rectangle with its four corners filleted (Equal arcs).
            1 => {
                let (w, h, rad) = (r.range(25.0, 50.0), r.range(18.0, 35.0), r.range(2.0, 6.0));
                let corners = [o + v(w, 0.0), o + v(w, h), o + v(0.0, h), o];
                let mut lines = Vec::new();
                let mut arcs = Vec::new();
                let pts = [(o + v(rad, 0.0), o + v(w - rad, 0.0)), (o + v(w, rad), o + v(w, h - rad)), (o + v(w - rad, h), o + v(rad, h)), (o + v(0.0, h - rad), o + v(0.0, rad))];
                for i in 0..4 {
                    lines.push(c.line(pts[i].0, pts[i].1));
                    let cc = corners[i] + match i { 0 => v(-rad, rad), 1 => v(-rad, -rad), 2 => v(rad, -rad), _ => v(rad, rad) };
                    let a0 = -FRAC_PI_2 + i as f64 * FRAC_PI_2;
                    arcs.push(c.arc(cc, rad, a0, a0 + FRAC_PI_2));
                }
                for i in 0..4 {
                    c.coincident(&format!("{}.end", lines[i]), &format!("{}.start", arcs[i]));
                    c.coincident(&format!("{}.end", arcs[i]), &format!("{}.start", lines[(i + 1) % 4]));
                    c.pair("TANGENT", &lines[i], &arcs[i]);
                    c.pair("TANGENT", &arcs[i], &lines[(i + 1) % 4]);
                    c.hv(i % 2 == 0, &lines[i]);
                }
                for i in 1..4 {
                    c.pair("EQUAL", &arcs[0], &arcs[i]);
                }
                let fr = f(&mut r);
                c.radius(&arcs[0], rad * fr, false);
                if r.chance(0.7) {
                    let fw = f(&mut r);
                    c.length(&lines[0], (w - 2.0 * rad) * fw);
                }
            }
            // Slot: two arcs, two lines; centre distance and radius.
            2 => {
                let (len, rad) = (r.range(15.0, 40.0), r.range(3.0, 9.0));
                let l1 = c.line(o + v(0.0, -rad), o + v(len, -rad));
                let a2 = c.arc(o + v(len, 0.0), rad, -FRAC_PI_2, FRAC_PI_2);
                let l3 = c.line(o + v(len, rad), o + v(0.0, rad));
                let a4 = c.arc(o, rad, FRAC_PI_2, 3.0 * FRAC_PI_2);
                c.coincident(&format!("{l1}.end"), &format!("{a2}.start"));
                c.coincident(&format!("{a2}.end"), &format!("{l3}.start"));
                c.coincident(&format!("{l3}.end"), &format!("{a4}.start"));
                c.coincident(&format!("{a4}.end"), &format!("{l1}.start"));
                for (a, b) in [(&l1, &a2), (&a2, &l3), (&l3, &a4), (&a4, &l1)] {
                    c.pair("TANGENT", a, b);
                }
                c.pair("EQUAL", &a2, &a4);
                c.hv(true, &l1);
                let (fl, fr) = (f(&mut r), f(&mut r));
                c.distance(&format!("{a4}.center"), &format!("{a2}.center"), len * fl);
                c.radius(&a2, rad * fr, false);
            }
            // L-bracket: six H / V lines, four lengths.
            3 => {
                let (w, h, t1, t2) = (r.range(25.0, 50.0), r.range(20.0, 40.0), r.range(4.0, 10.0), r.range(4.0, 10.0));
                let p = [o, o + v(w, 0.0), o + v(w, t1), o + v(t2, t1), o + v(t2, h), o + v(0.0, h)];
                let l: Vec<String> = (0..6).map(|i| c.line(p[i], p[(i + 1) % 6])).collect();
                for i in 0..6 {
                    c.coincident(&format!("{}.end", l[i]), &format!("{}.start", l[(i + 1) % 6]));
                    c.hv(i % 2 == 0, &l[i]);
                }
                let fs: Vec<f64> = (0..4).map(|_| f(&mut r)).collect();
                c.length(&l[0], w * fs[0]);
                c.length(&l[5], h * fs[1]);
                c.length(&l[1], t1 * fs[2]);
                c.length(&l[4], (h - t1) * fs[3]);
            }
            // Bolt circle: a big circle, three or four equal holes at a
            // distance from its centre.
            4 => {
                let (big, rad, n) = (r.range(15.0, 30.0), r.range(1.5, 4.0), 3 + r.below(2));
                let pcd = big * r.range(0.55, 0.75);
                let cb = c.circle(o, big);
                let holes: Vec<String> = (0..n).map(|i| c.circle(o + DVec2::from_angle(PI / 2.0 + i as f64 * 2.0 * PI / n as f64) * pcd, rad)).collect();
                for hh in &holes[1..] {
                    c.pair("EQUAL", &holes[0], hh);
                }
                let (fb, fp, fr) = (f(&mut r), f(&mut r), f(&mut r));
                c.radius(&cb, big * fb, true);
                c.radius(&holes[0], rad * fr, true);
                for hh in &holes {
                    c.distance(&format!("{cb}.center"), &format!("{hh}.center"), pcd * fp);
                }
            }
            // Washer: two concentric circles, both diameters.
            5 => {
                let (outer, inner) = (r.range(10.0, 25.0), r.range(3.0, 8.0));
                let co = c.circle(o, outer);
                let ci = c.circle(o + v(r.range(-0.5, 0.5), r.range(-0.5, 0.5)), inner);
                c.pair("CONCENTRIC", &co, &ci);
                let (fo, fi) = (f(&mut r), f(&mut r));
                c.radius(&co, outer * fo, true);
                c.radius(&ci, inner * fi, true);
            }
            // Plate with a hole: an H / V rectangle, width and height, a
            // circle's diameter and its centre's distances to the left and
            // bottom sides.
            6 => {
                let (w, h) = (r.range(30.0, 50.0), r.range(20.0, 35.0));
                let p = [o, o + v(w, 0.0), o + v(w, h), o + v(0.0, h)];
                let l: Vec<String> = (0..4).map(|i| c.line(p[i], p[(i + 1) % 4])).collect();
                for i in 0..4 {
                    c.coincident(&format!("{}.end", l[i]), &format!("{}.start", l[(i + 1) % 4]));
                    c.hv(i % 2 == 0, &l[i]);
                }
                let (dx, dy, rad) = (w * r.range(0.3, 0.7), h * r.range(0.3, 0.7), r.range(2.0, 5.0));
                let hole = c.circle(o + v(dx, dy), rad);
                let fs: Vec<f64> = (0..5).map(|_| f(&mut r)).collect();
                c.length(&l[0], w * fs[0]);
                c.length(&l[1], h * fs[1]);
                c.radius(&hole, rad * fs[2], true);
                c.distance(&format!("{hole}.center"), &l[3], dx * fs[3]);
                c.distance(&format!("{hole}.center"), &l[0], dy * fs[4]);
            }
            // Rectangle centred on a Vertical construction line: its bottom
            // corners Symmetric about it; width and height.
            8 => {
                let (w, h) = (r.range(20.0, 50.0), r.range(12.0, 35.0));
                let p = [o, o + v(w, 0.0), o + v(w, h), o + v(0.0, h)];
                let l: Vec<String> = (0..4).map(|i| c.line(p[i], p[(i + 1) % 4])).collect();
                for i in 0..4 {
                    c.coincident(&format!("{}.end", l[i]), &format!("{}.start", l[(i + 1) % 4]));
                    c.hv(i % 2 == 0, &l[i]);
                }
                let axis = c.line(o + v(w / 2.0, -6.0), o + v(w / 2.0, h + 6.0));
                c.construction(&axis);
                c.hv(false, &axis);
                c.symmetric(&format!("{}.start", l[0]), &format!("{}.end", l[0]), &axis);
                let (fw, fh) = (f(&mut r), f(&mut r));
                c.length(&l[0], w * fw);
                c.length(&l[1], h * fh);
            }
            // Triangle: a Horizontal base, its length, the left side's
            // length and the angle between them.
            9 => {
                let (b, side, deg) = (r.range(25.0, 50.0), r.range(15.0, 35.0), r.range(35.0, 75.0));
                let top = o + DVec2::from_angle(deg.to_radians()) * side;
                let l1 = c.line(o, o + v(b, 0.0));
                let l2 = c.line(o + v(b, 0.0), top);
                // (The left side drawn up from the base's start: the angle
                // from the base to it is the inner one, counter-clockwise.)
                let l3 = c.line(o, top);
                c.coincident(&format!("{l1}.end"), &format!("{l2}.start"));
                c.coincident(&format!("{l2}.end"), &format!("{l3}.end"));
                c.coincident(&format!("{l3}.start"), &format!("{l1}.start"));
                c.hv(true, &l1);
                let (fb, fs, fa) = (f(&mut r), f(&mut r), f(&mut r));
                c.length(&l1, b * fb);
                c.length(&l3, side * fs);
                // (Between 25° and 85°.)
                c.angle(&l1, &l3, (deg * fa).clamp(25.0, 85.0));
            }
            // Plate with a centred hole: an H / V rectangle, width and
            // height, a hole at the midpoint of its construction diagonal.
            10 => {
                let (w, h) = (r.range(30.0, 50.0), r.range(20.0, 35.0));
                let p = [o, o + v(w, 0.0), o + v(w, h), o + v(0.0, h)];
                let l: Vec<String> = (0..4).map(|i| c.line(p[i], p[(i + 1) % 4])).collect();
                for i in 0..4 {
                    c.coincident(&format!("{}.end", l[i]), &format!("{}.start", l[(i + 1) % 4]));
                    c.hv(i % 2 == 0, &l[i]);
                }
                let diag = c.line(o, o + v(w, h));
                c.construction(&diag);
                c.coincident(&format!("{diag}.start"), &format!("{}.start", l[0]));
                c.coincident(&format!("{diag}.end"), &format!("{}.start", l[2]));
                let rad = r.range(2.0, 6.0);
                let hole = c.circle(o + v(w / 2.0, h / 2.0), rad);
                c.midpoint(&format!("{hole}.center"), &diag);
                let (fw, fh, fr) = (f(&mut r), f(&mut r), f(&mut r));
                c.length(&l[0], w * fw);
                c.length(&l[1], h * fh);
                c.radius(&hole, rad * fr, true);
            }
            // Hexagon as the polygon tool draws it: six Equal sides, their
            // corners on a construction circle, one side Horizontal; the
            // circle's diameter.
            11 => {
                let rad = r.range(8.0, 20.0);
                let p: Vec<DVec2> = (0..6).map(|i| o + DVec2::from_angle(-FRAC_PI_2 - PI / 6.0 + i as f64 * PI / 3.0) * rad).collect();
                let l: Vec<String> = (0..6).map(|i| c.line(p[i], p[(i + 1) % 6])).collect();
                let circle = c.circle(o, rad);
                c.construction(&circle);
                for i in 0..6 {
                    c.coincident(&format!("{}.end", l[i]), &format!("{}.start", l[(i + 1) % 6]));
                    c.on_curve(&format!("{}.start", l[i]), &circle);
                }
                for i in 1..6 {
                    c.pair("EQUAL", &l[0], &l[i]);
                }
                c.hv(true, &l[0]);
                let fr = f(&mut r);
                c.radius(&circle, rad * fr, true);
            }
            // Rectangle with a chamfered corner: H / V sides, the chamfer's
            // two legs Equal (a distance from the corner each way); width,
            // height and one leg.
            12 => {
                let (w, h) = (r.range(25.0, 50.0), r.range(18.0, 35.0));
                let ch = r.range(3.0, 7.0);
                let p = [o, o + v(w - ch, 0.0), o + v(w, ch), o + v(w, h), o + v(0.0, h)];
                let l: Vec<String> = (0..5).map(|i| c.line(p[i], p[(i + 1) % 5])).collect();
                for i in 0..5 {
                    c.coincident(&format!("{}.end", l[i]), &format!("{}.start", l[(i + 1) % 5]));
                }
                c.hv(true, &l[0]);
                c.hv(false, &l[2]);
                c.hv(true, &l[3]);
                c.hv(false, &l[4]);
                c.angle(&l[0], &l[1], 45.0);
                let (fw, fh, fc) = (f(&mut r), f(&mut r), f(&mut r));
                c.length(&l[4], h * fh);
                c.length(&l[3], w * fw);
                c.length(&l[1], ch * std::f64::consts::SQRT_2 * fc);
            }
            // Parallelogram: Horizontal base and top, the sides Parallel;
            // the base's length, a side's length and the angle between them.
            13 => {
                let (b, side, deg) = (r.range(25.0, 50.0), r.range(12.0, 30.0), r.range(50.0, 80.0));
                let d = DVec2::from_angle(deg.to_radians()) * side;
                let p = [o, o + v(b, 0.0), o + v(b, 0.0) + d, o + d];
                let l1 = c.line(p[0], p[1]);
                let l2 = c.line(p[1], p[2]);
                let l3 = c.line(p[2], p[3]);
                // (The left side drawn up from the base's start: the angle is
                // the inner one, counter-clockwise.)
                let l4 = c.line(p[0], p[3]);
                c.coincident(&format!("{l1}.end"), &format!("{l2}.start"));
                c.coincident(&format!("{l2}.end"), &format!("{l3}.start"));
                c.coincident(&format!("{l3}.end"), &format!("{l4}.end"));
                c.coincident(&format!("{l4}.start"), &format!("{l1}.start"));
                c.hv(true, &l1);
                c.hv(true, &l3);
                c.pair("PARALLEL", &l2, &l4);
                let (fb, fs, fa) = (f(&mut r), f(&mut r), f(&mut r));
                c.length(&l1, b * fb);
                c.length(&l4, side * fs);
                c.angle(&l1, &l4, (deg * fa).clamp(35.0, 85.0));
            }
            // D shape: a Horizontal chord and an arc over it, the chord's
            // length and the arc's radius.
            _ => {
                let len = r.range(15.0, 30.0);
                let rad = len * r.range(0.55, 0.9);
                let hgt = (rad * rad - len * len / 4.0).sqrt();
                let l1 = c.line(o + v(-len / 2.0, 0.0), o + v(len / 2.0, 0.0));
                let start = (-hgt).atan2(len / 2.0);
                let a2 = c.arc(o + v(0.0, hgt), rad, start, PI - start);
                c.coincident(&format!("{l1}.end"), &format!("{a2}.start"));
                c.coincident(&format!("{a2}.end"), &format!("{l1}.start"));
                c.hv(true, &l1);
                let (fl, fr) = (f(&mut r), f(&mut r));
                c.length(&l1, len * fl);
                // (Never under half the new chord.)
                c.radius(&a2, (rad * fr).max(len * fl * 0.55), false);
            }
        }
    }
    c
}

/// A random plausible sketch; `rich` adds splines, conics and angles (batch 2 on).
fn generate(seed: u64, rich: bool) -> Case {
    if seed >= 301 {
        return everyday(seed);
    }
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
const BATCHES: [(usize, u64, bool); 8] = [(1, 1, false), (2, 101, true), (3, 201, true), (4, 301, true), (5, 341, true), (6, 381, true), (7, 421, true), (8, 461, true)];

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

/// The sum of the named points' distances from live's (mm).
fn position_error(case: &Value, live: &Value) -> f64 {
    let Some(pts) = live["points"].as_object() else { return 0.0 };
    let at = |v: &Value, i: usize| v[i].as_f64().unwrap_or(f64::NAN);
    pts.iter().map(|(n, p)| (at(&case["ours"]["points"][n], 0) - at(p, 0)).hypot(at(&case["ours"]["points"][n], 1) - at(p, 1))).sum()
}

/// Live's UI drag of a case against ours dragged to where live's point
/// ended (live's pointer lands within a pixel of the target, 0.2 mm).
fn drag_differences(seed: u64, rich: bool, case: &Value, live: &Value) -> Vec<String> {
    let (Some(name), Some(ld)) = (case["drag"]["point"].as_str(), live["drag"].as_object()) else { return Vec::new() };
    let Some(to) = ld.get(name) else { return Vec::new() };
    let mut c = generate(seed, rich);
    c.sk.solve_with(&SolveOptions { constructive: true, ..Default::default() });
    let target = DVec2::new(to[0].as_f64().unwrap(), to[1].as_f64().unwrap()) * MM;
    // In 25 pointer steps, as live's UI drag (and the editor's drags).
    let from = c.sk.point(c.point(name)).unwrap();
    for k in 1..=25 {
        let at = from + (target - from) * (k as f64 / 25.0);
        c.sk.drag(&[(DragHandle::Point(c.point(name)), at)]);
    }
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
                positions.push(format!("{name} ({} differ, {:.2} mm{}): {}", d.len(), position_error(case, l), if l["solved"] == false { ", live unsolved" } else { "" }, d.join("; ")));
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

/// Third probes (fuzz s115: a rectangle with its bottom and right side
/// dimensioned kept its top-right corner, not the lower left): rectangles with
/// one or two sides changed, growing and shrinking, dimensioned on either
/// side, drawn from another corner.
fn probes3() -> Vec<(String, Case)> {
    let v = DVec2::new;
    let mut out = Vec::new();
    // A rectangle drawn from `p0` counter-clockwise (or clockwise), H / V on
    // its sides, the given lengths on sides (index, mm).
    let rect = |c: &mut Case, corners: [DVec2; 4], dims: &[(usize, f64)]| {
        let l: Vec<String> = (0..4).map(|i| c.line(corners[i], corners[(i + 1) % 4])).collect();
        for i in 0..4 {
            c.coincident(&format!("{}.end", l[i]), &format!("{}.start", l[(i + 1) % 4]));
        }
        for (i, x) in l.iter().enumerate() {
            let d = corners[(i + 1) % 4] - corners[i];
            c.hv(d.y.abs() < 1e-9, x);
        }
        for (i, mm) in dims {
            c.length(&l[*i], *mm);
        }
    };
    let ccw = [v(0.0, 0.0), v(20.0, 0.0), v(20.0, 10.0), v(0.0, 10.0)];
    let mut add = |name: &str, f: &dyn Fn(&mut Case)| {
        let mut c = Case::new();
        f(&mut c);
        out.push((name.to_string(), c));
    };
    add("r1 rectangle (0,0) 20 x 10, bottom 25 and right side 12", &|c| rect(c, ccw, &[(0, 25.0), (1, 12.0)]));
    add("r2 rectangle, right side 10 -> 12 only", &|c| rect(c, ccw, &[(1, 12.0)]));
    add("r3 rectangle, bottom 20 -> 15 (shrink)", &|c| rect(c, ccw, &[(0, 15.0)]));
    add("r4 rectangle, bottom 15 and right side 8 (both shrink)", &|c| rect(c, ccw, &[(0, 15.0), (1, 8.0)]));
    add("r5 rectangle, top 25 (the side drawn right to left)", &|c| rect(c, ccw, &[(2, 25.0)]));
    add("r6 rectangle, left side 12 (drawn top to bottom)", &|c| rect(c, ccw, &[(3, 12.0)]));
    add("r7 rectangle drawn from the top-left corner clockwise, bottom 25", &|c| rect(c, [v(0.0, 10.0), v(20.0, 10.0), v(20.0, 0.0), v(0.0, 0.0)], &[(2, 25.0)]));
    add("r8 rectangle at (100,50), bottom 25 and right side 12", &|c| rect(c, ccw.map(|p| p + v(100.0, 50.0)), &[(0, 25.0), (1, 12.0)]));
    add("r9 as fuzz s115: (64.3,2.3) 38.5 x 23.1, bottom 36.4 (shrink) and right side 23.5 (grow)", &|c| rect(c, [v(64.324, 2.308), v(102.868, 2.308), v(102.868, 25.372), v(64.324, 25.372)], &[(1, 23.517), (0, 36.392)]));
    add("r10 as r9, the dimensions added the other order", &|c| rect(c, [v(64.324, 2.308), v(102.868, 2.308), v(102.868, 25.372), v(64.324, 25.372)], &[(0, 36.392), (1, 23.517)]));
    out
}

/// Fourth probes (fuzz s115: a rectangle whose left side is Perpendicular
/// to a separate, dimensioned line kept its top-right corner where a lone
/// rectangle keeps its lower left): a rectangle 20 x 10 at (40, 0) and a line
/// (0, 0)–(20, 0), related or not, either one dimensioned.
fn probes4() -> Vec<(String, Case)> {
    let v = DVec2::new;
    let mut out = Vec::new();
    // sides: 0 bottom, 1 right, 2 top, 3 left; `rel`: (relation, side);
    // `dims`: rectangle sides' lengths; `line`: the line's length.
    let build = |c: &mut Case, rel: Option<(&str, usize)>, dims: &[(usize, f64)], line: Option<f64>| {
        let corners = [v(40.0, 0.0), v(60.0, 0.0), v(60.0, 10.0), v(40.0, 10.0)];
        let l: Vec<String> = (0..4).map(|i| c.line(corners[i], corners[(i + 1) % 4])).collect();
        for i in 0..4 {
            c.coincident(&format!("{}.end", l[i]), &format!("{}.start", l[(i + 1) % 4]));
        }
        for (i, x) in l.iter().enumerate() {
            c.hv(i % 2 == 0, x);
        }
        let s = c.line(v(0.0, 0.0), v(20.0, 0.0));
        if let Some((kind, side)) = rel {
            c.pair(kind, &s, &l[side]);
        }
        for (i, mm) in dims {
            c.length(&l[*i], *mm);
        }
        if let Some(mm) = line {
            c.length(&s, mm);
        }
    };
    let mut add = |name: &str, rel: Option<(&str, usize)>, dims: &[(usize, f64)], line: Option<f64>| {
        let mut c = Case::new();
        build(&mut c, rel, dims, line);
        out.push((name.to_string(), c));
    };
    add("t1 line Perpendicular to the rectangle's left side; rectangle bottom 25 and right 12", Some(("PERPENDICULAR", 3)), &[(0, 25.0), (1, 12.0)], None);
    add("t2 as t1, the line also dimensioned 25", Some(("PERPENDICULAR", 3)), &[(0, 25.0), (1, 12.0)], Some(25.0));
    add("t3 line Perpendicular to the left side, only the line dimensioned 25", Some(("PERPENDICULAR", 3)), &[], Some(25.0));
    add("t4 line Perpendicular to the right side; rectangle bottom 25 and right 12", Some(("PERPENDICULAR", 1)), &[(0, 25.0), (1, 12.0)], None);
    add("t5 line Parallel to the bottom; rectangle bottom 25 and right 12", Some(("PARALLEL", 0)), &[(0, 25.0), (1, 12.0)], None);
    add("t6 line Parallel to the top; rectangle bottom 25 and right 12", Some(("PARALLEL", 2)), &[(0, 25.0), (1, 12.0)], None);
    add("t7 no relation, both dimensioned (rectangle bottom 25 / right 12, line 25)", None, &[(0, 25.0), (1, 12.0)], Some(25.0));
    add("t8 line Perpendicular to the left side; rectangle bottom 15 (shrink) and right 12 (grow)", Some(("PERPENDICULAR", 3)), &[(0, 15.0), (1, 12.0)], None);
    out
}

/// Probes 5 (after batch 3): a slot's size change (does the whole slot
/// scale, fuzz s17), where a line turns onto a Perpendicular / Parallel to
/// a separate line (fuzz s104's triangle turned about the two lines'
/// intersection), and an Equal between two lines tied at both ends (fuzz
/// s11's quad kept the longer one).
fn probes5() -> Vec<(String, Case)> {
    use std::f64::consts::FRAC_PI_2;
    let v = DVec2::new;
    let mut out = Vec::new();
    let mut add = |name: &str, f: &dyn Fn(&mut Case)| {
        let mut c = Case::new();
        f(&mut c);
        out.push((name.to_string(), c));
    };
    // A 20 mm slot of radius 5 from (0, 0): bottom, right arc, top, left arc.
    let slot = |c: &mut Case| -> [String; 4] {
        let l1 = c.line(v(0.0, 0.0), v(20.0, 0.0));
        let a2 = c.arc(v(20.0, 5.0), 5.0, -FRAC_PI_2, FRAC_PI_2);
        let l3 = c.line(v(20.0, 10.0), v(0.0, 10.0));
        let a4 = c.arc(v(0.0, 5.0), 5.0, FRAC_PI_2, 3.0 * FRAC_PI_2);
        c.coincident(&format!("{l1}.end"), &format!("{a2}.start"));
        c.coincident(&format!("{a2}.end"), &format!("{l3}.start"));
        c.coincident(&format!("{l3}.end"), &format!("{a4}.start"));
        c.coincident(&format!("{a4}.end"), &format!("{l1}.start"));
        for (a, b) in [(&l1, &a2), (&a2, &l3), (&l3, &a4), (&a4, &l1)] {
            c.pair("TANGENT", a, b);
        }
        c.pair("EQUAL", &a2, &a4);
        c.pair("PARALLEL", &l1, &l3);
        [l1, a2, l3, a4]
    };
    add("u1 slot, the right arc's radius 5 -> 4", &|c| {
        let [_, a2, _, _] = slot(c);
        c.radius(&a2, 4.0, false);
    });
    add("u2 slot, the right arc's chord (its start to the top line's start) 10 -> 8", &|c| {
        let [_, a2, l3, _] = slot(c);
        c.distance(&format!("{a2}.start"), &format!("{l3}.start"), 8.0);
    });
    add("u3 slot, the left arc's radius 5 -> 4", &|c| {
        let [_, _, _, a4] = slot(c);
        c.radius(&a4, 4.0, false);
    });
    add("u4 line B (30,10)-(31,30) Perpendicular to a Horizontal line A (0,0)-(20,0)", &|c| {
        let a = c.line(v(0.0, 0.0), v(20.0, 0.0));
        c.hv(true, &a);
        let b = c.line(v(30.0, 10.0), v(31.0, 30.0));
        c.pair("PERPENDICULAR", &b, &a);
    });
    add("u5 as u4, B below A: (30,-30)-(31,-10)", &|c| {
        let a = c.line(v(0.0, 0.0), v(20.0, 0.0));
        c.hv(true, &a);
        let b = c.line(v(30.0, -30.0), v(31.0, -10.0));
        c.pair("PERPENDICULAR", &b, &a);
    });
    add("u6 as u4, A not Horizontal", &|c| {
        let a = c.line(v(0.0, 0.0), v(20.0, 0.0));
        let b = c.line(v(30.0, 10.0), v(31.0, 30.0));
        c.pair("PERPENDICULAR", &b, &a);
    });
    add("u7 line B (30,10)-(50,11) Parallel to a Horizontal line A (0,0)-(20,0)", &|c| {
        let a = c.line(v(0.0, 0.0), v(20.0, 0.0));
        c.hv(true, &a);
        let b = c.line(v(30.0, 10.0), v(50.0, 11.0));
        c.pair("PARALLEL", &b, &a);
    });
    add("u8 triangle (40,0) (41,20) (60,10), its side (40,0)-(41,20) Perpendicular to a Horizontal line (0,0)-(20,0)", &|c| {
        let a = c.line(v(0.0, 0.0), v(20.0, 0.0));
        c.hv(true, &a);
        let p = [v(40.0, 0.0), v(41.0, 20.0), v(60.0, 10.0)];
        let l: Vec<String> = (0..3).map(|i| c.line(p[i], p[(i + 1) % 3])).collect();
        for i in 0..3 {
            c.coincident(&format!("{}.end", l[i]), &format!("{}.start", l[(i + 1) % 3]));
        }
        c.pair("PERPENDICULAR", &l[0], &a);
    });
    // Fuzz s11's quad (l4..l7 there), Equal between its first two sides.
    let quad = |c: &mut Case| -> Vec<String> {
        let p = [v(78.25015, 5.446345), v(66.052417, 13.353519), v(52.339045, 3.523184), v(63.70269, -9.138436)];
        let l: Vec<String> = (0..4).map(|i| c.line(p[i], p[(i + 1) % 4])).collect();
        for i in 0..4 {
            c.coincident(&format!("{}.end", l[i]), &format!("{}.start", l[(i + 1) % 4]));
        }
        l
    };
    add("u9 fuzz s11's quad alone, Equal (second side, first side): 16.87 vs 14.53 mm", &|c| {
        let l = quad(c);
        c.pair("EQUAL", &l[1], &l[0]);
    });
    add("u10 as u9, Equal (first side, second side)", &|c| {
        let l = quad(c);
        c.pair("EQUAL", &l[0], &l[1]);
    });
    out
}

/// Probes 6 (after batch 3, fuzz s226 / s215): a distance between points of
/// two separate loops that doesn't hold: which loop moves (by position,
/// creation order, how constrained each is, which one has its own change).
fn probes6() -> Vec<(String, Case)> {
    let v = DVec2::new;
    let mut out = Vec::new();
    let mut add = |name: &str, f: &dyn Fn(&mut Case)| {
        let mut c = Case::new();
        f(&mut c);
        out.push((name.to_string(), c));
    };
    // A closed quad from `p` (counter-clockwise); `hv` puts H / V on it.
    let quad = |c: &mut Case, p: [DVec2; 4], hv: bool| -> Vec<String> {
        let l: Vec<String> = (0..4).map(|i| c.line(p[i], p[(i + 1) % 4])).collect();
        for i in 0..4 {
            c.coincident(&format!("{}.end", l[i]), &format!("{}.start", l[(i + 1) % 4]));
        }
        if hv {
            for (i, x) in l.iter().enumerate() {
                c.hv(i % 2 == 0, x);
            }
        }
        l
    };
    let sq = |x: f64| [v(x, 0.0), v(x + 10.0, 0.0), v(x + 10.0, 10.0), v(x, 10.0)];
    // Square L at x 0, square R at x 30; the distance from L's lower-right
    // corner to R's lower-left corner (20 now) set to 25.
    add("w1 two H/V squares, L created first; L's right corner to R's left corner 20 -> 25", &|c| {
        let l = quad(c, sq(0.0), true);
        let r = quad(c, sq(30.0), true);
        c.distance(&format!("{}.end", l[0]), &format!("{}.start", r[0]), 25.0);
    });
    add("w2 as w1, R created first", &|c| {
        let r = quad(c, sq(30.0), true);
        let l = quad(c, sq(0.0), true);
        c.distance(&format!("{}.end", l[0]), &format!("{}.start", r[0]), 25.0);
    });
    add("w3 as w1, the distance's points the other order (R's first)", &|c| {
        let l = quad(c, sq(0.0), true);
        let r = quad(c, sq(30.0), true);
        c.distance(&format!("{}.start", r[0]), &format!("{}.end", l[0]), 25.0);
    });
    add("w4 as w1, L has no H / V (a free quad of the same shape)", &|c| {
        let l = quad(c, sq(0.0), false);
        let r = quad(c, sq(30.0), true);
        c.distance(&format!("{}.end", l[0]), &format!("{}.start", r[0]), 25.0);
    });
    add("w5 as w1, R has no H / V", &|c| {
        let l = quad(c, sq(0.0), true);
        let r = quad(c, sq(30.0), false);
        c.distance(&format!("{}.end", l[0]), &format!("{}.start", r[0]), 25.0);
    });
    add("w6 as w1, R's bottom also dimensioned 12 (R changes too)", &|c| {
        let l = quad(c, sq(0.0), true);
        let r = quad(c, sq(30.0), true);
        c.length(&r[0], 12.0);
        c.distance(&format!("{}.end", l[0]), &format!("{}.start", r[0]), 25.0);
    });
    add("w7 as w1, L's bottom also dimensioned 12 (L changes too)", &|c| {
        let l = quad(c, sq(0.0), true);
        let r = quad(c, sq(30.0), true);
        c.length(&l[0], 12.0);
        c.distance(&format!("{}.end", l[0]), &format!("{}.start", r[0]), 25.0);
    });
    add("w8 as w1, R one above the other: R at (0, 30), L's top-left to R's bottom-left 20 -> 25", &|c| {
        let l = quad(c, sq(0.0), true);
        let r = quad(c, [v(0.0, 30.0), v(10.0, 30.0), v(10.0, 40.0), v(0.0, 40.0)], true);
        c.distance(&format!("{}.start", l[3]), &format!("{}.start", r[0]), 25.0);
    });
    add("w9 an H/V square at x 0 and a lone line (30,0)-(40,0); the square's lower-right corner to the line's start 20 -> 25", &|c| {
        let l = quad(c, sq(0.0), true);
        let s = c.line(v(30.0, 0.0), v(40.0, 0.0));
        c.distance(&format!("{}.end", l[0]), &format!("{s}.start"), 25.0);
    });
    add("w10 as w9, the line on the left at (-30,0)-(-20,0), the square at x 0", &|c| {
        let s = c.line(v(-30.0, 0.0), v(-20.0, 0.0));
        let l = quad(c, sq(0.0), true);
        c.distance(&format!("{s}.end"), &format!("{}.start", l[0]), 25.0);
    });
    out
}

/// Drag probes (probes 7, after batch 1's drags s1 / s3 / s5 / s12): minimal
/// sketches that already solve, one point dragged in the UI by (dx, dy) mm
/// in 25 steps: what moves, what keeps its size or direction.
fn drag_probes() -> Vec<(String, Case, String, DVec2)> {
    let v = DVec2::new;
    let mut out = Vec::new();
    let mut add = |name: &str, point: &str, by: DVec2, f: &dyn Fn(&mut Case)| {
        let mut c = Case::new();
        f(&mut c);
        out.push((name.to_string(), c, point.to_string(), by));
    };
    // A closed quad from `p`; `hv` puts H / V on its sides.
    let quad = |c: &mut Case, p: [DVec2; 4], hv: [bool; 4]| -> Vec<String> {
        let l: Vec<String> = (0..4).map(|i| c.line(p[i], p[(i + 1) % 4])).collect();
        for i in 0..4 {
            c.coincident(&format!("{}.end", l[i]), &format!("{}.start", l[(i + 1) % 4]));
        }
        for (i, x) in l.iter().enumerate() {
            if hv[i] {
                c.hv(i % 2 == 0, x);
            }
        }
        l
    };
    let rect = [v(0.0, 0.0), v(20.0, 0.0), v(20.0, 10.0), v(0.0, 10.0)];
    add("d1 rectangle with H / V on all four sides, its lower-left corner dragged by (3, 2)", "l1.start", v(3.0, 2.0), &|c| {
        quad(c, rect, [true; 4]);
    });
    add("d2 as d1, the left side without its Vertical", "l1.start", v(3.0, 2.0), &|c| {
        quad(c, rect, [true, true, true, false]);
    });
    add("d3 as d1, the bottom dimensioned 20", "l1.start", v(3.0, 2.0), &|c| {
        let l = quad(c, rect, [true; 4]);
        c.length(&l[0], 20.0);
    });
    add("d4 as d1, width 20 and height 10 dimensioned", "l1.start", v(3.0, 2.0), &|c| {
        let l = quad(c, rect, [true; 4]);
        c.length(&l[0], 20.0);
        c.length(&l[1], 10.0);
    });
    add("d5 a free quad (no H / V), its lower-left corner dragged by (3, 2)", "l1.start", v(3.0, 2.0), &|c| {
        quad(c, rect, [false; 4]);
    });
    add("d6 an L: horizontal (0,0)-(20,0) and vertical (20,0)-(20,10), the corner dragged by (3, 2)", "l1.end", v(3.0, 2.0), &|c| {
        let a = c.line(v(0.0, 0.0), v(20.0, 0.0));
        let b = c.line(v(20.0, 0.0), v(20.0, 10.0));
        c.coincident(&format!("{a}.end"), &format!("{b}.start"));
        c.hv(true, &a);
        c.hv(false, &b);
    });
    add("d7 a line (0,0)-(20,0), Horizontal, length 20, its end dragged by (3, 2)", "l1.end", v(3.0, 2.0), &|c| {
        let a = c.line(v(0.0, 0.0), v(20.0, 0.0));
        c.hv(true, &a);
        c.length(&a, 20.0);
    });
    add("d8 fuzz s3's chain alone: line, tangent arc, perpendicular line with length 20; the first line's start dragged by (5, 3)", "l1.start", v(5.0, 3.0), &|c| {
        let l1 = c.line(v(-20.0, 0.0), v(0.0, 0.0));
        let a = c.arc(v(0.0, 6.0), 6.0, -std::f64::consts::FRAC_PI_2, 0.0);
        let l3 = c.line(v(6.0, 6.0), v(6.0, 26.0));
        c.coincident(&format!("{l1}.end"), &format!("{a}.start"));
        c.coincident(&format!("{a}.end"), &format!("{l3}.start"));
        c.pair("TANGENT", &l1, &a);
        c.pair("TANGENT", &a, &l3);
        c.pair("PERPENDICULAR", &l3, &l1);
        c.length(&l3, 20.0);
    });
    out
}

/// Probes 10 (batches 1-2's largest left, s6 and s102, both placed by a
/// least-change fallback in ours): each case again with one of its
/// relations left out, to see which relation moves what in live.
fn probes10() -> Vec<(String, Case)> {
    leave_one_out(&[6, 102])
}

/// Probes 11: the same for batch 3's largest left (s218, s236, s226).
fn probes11() -> Vec<(String, Case)> {
    leave_one_out(&[218, 236, 226])
}

/// Probes 13: the same for the rest of batches 1-3 left (s209, s11).
fn probes13() -> Vec<(String, Case)> {
    leave_one_out(&[209, 11])
}

/// Each case of `seeds` again with one of its relations left out.
fn leave_one_out(seeds: &[u64]) -> Vec<(String, Case)> {
    let mut out = Vec::new();
    for &seed in seeds {
        let c = generate(seed, seed > 100);
        let names: Vec<(String, String)> = c.spec.iter().filter(|k| k["type"] != "COINCIDENT" || !k["second"].as_str().unwrap_or(".").contains('.')).map(|k| (k["id"].as_str().unwrap().to_string(), k["type"].as_str().unwrap().to_string())).collect();
        for (k, kind) in names {
            let mut c = generate(seed, seed > 100);
            if let Some(i) = c.cons.iter().position(|(n, _)| *n == k)
                && let (_, Some(id)) = c.cons.remove(i)
            {
                c.sk.remove_constraint(id).unwrap();
            }
            c.spec.retain(|s| s["id"] != k.as_str());
            c.os_constraints.retain(|s| s["entityId"] != k.as_str());
            out.push((format!("r{seed}-{k} s{seed} without {k} ({kind})"), c));
        }
    }
    out
}

/// Writes the drag probes (LIVE_FUZZ_WRITE=probe-7 / probe-9) in the
/// batches' shape, each case's `drag` = the point, its target and ours
/// dragged there in 25 steps; compares with live's answers when there.
#[test]
#[ignore = "T2: live fuzz drag probes"]
fn drag_probe_matches() {
    drag_probe_batch(7, drag_probes());
    drag_probe_batch(9, drag_probes9());
}

fn drag_probe_batch(n: usize, list: Vec<(String, Case, String, DVec2)>) {
    let live: Option<Value> = std::fs::read_to_string(fixture(&format!("sketch-probe-{n}.live.json"))).ok().and_then(|t| serde_json::from_str(&t).ok());
    let cases: Vec<Value> = list.into_iter().map(|(name, mut c, point, by)| {
        let o = ours(&mut c);
        let from = c.pos(&point) / MM;
        let to = from + by;
        let id = name.split(' ').next().unwrap().to_string();
        // Ours dragged to where live's pointer landed (within a pixel of
        // `to`), when live answered.
        let end = live.as_ref().and_then(|l| l["cases"][&id]["drag"]["final"].as_array().map(|f| DVec2::new(f[0].as_f64().unwrap(), f[1].as_f64().unwrap()))).unwrap_or(to);
        let p = c.point(&point);
        for k in 1..=25 {
            c.sk.drag(&[(DragHandle::Point(p), (from + (end - from) * (k as f64 / 25.0)) * MM)]);
        }
        json!({"case": id, "what": name, "constraints": c.spec, "onshape": {"entities": c.os_entities, "constraints": c.os_constraints}, "ours": o, "drag": {"point": point, "to": [to.x, to.y], "ours": ours_points(&c)}})
    }).collect();
    let b = json!({"units": "mm (Onshape JSON: metres, radians)", "plane": "Top", "cases": cases});
    let path = fixture(&format!("sketch-probe-{n}.json"));
    if std::env::var("LIVE_FUZZ_WRITE").is_ok_and(|v| v == format!("probe-{n}")) {
        std::fs::write(&path, serde_json::to_string_pretty(&b).unwrap()).unwrap();
    }
    let Some(live) = live else { return };
    let (mut total, mut ok) = (0, 0);
    for case in b["cases"].as_array().unwrap() {
        let Some(after) = live["cases"][case["case"].as_str().unwrap()]["drag"]["after"].as_object() else { continue };
        total += 1;
        let d: Vec<String> = after.iter().filter(|(n, p)| {
            let o = &case["drag"]["ours"][n.as_str()];
            (o[0].as_f64().unwrap_or(f64::NAN) - p[0].as_f64().unwrap_or(0.0)).hypot(o[1].as_f64().unwrap_or(f64::NAN) - p[1].as_f64().unwrap_or(0.0)) >= 1e-3
        }).map(|(n, p)| format!("{n}: ours {} live {p}", case["drag"]["ours"][n.as_str()])).collect();
        if d.is_empty() {
            ok += 1;
        } else {
            println!("  {} ({}): {}", case["case"], case["what"], d.join("; "));
        }
    }
    println!("live drag probes {n}: {total} answered, {ok} match");
}

/// UI edit probes (probes 8, after live 1775 / 1785): everyday profiles that
/// already solve, one dimension then typed anew in the editor: what moves
/// when the profile has other dimensions too (no uniform scaling then).
/// (name, case, the edited constraint's id in `spec`, the new value in mm).
fn edit_probes() -> Vec<(String, Case, String, f64)> {
    use std::f64::consts::FRAC_PI_2;
    let v = DVec2::new;
    let mut out = Vec::new();
    let mut add = |name: &str, f: &dyn Fn(&mut Case) -> String, value: f64| {
        let mut c = Case::new();
        let id = f(&mut c);
        out.push((name.to_string(), c, id, value));
    };
    let last = |c: &Case| c.spec.last().unwrap()["id"].as_str().unwrap().to_string();
    // An H / V rectangle 40 x 20 from (10, 10), width and height dimensioned.
    let rect = |c: &mut Case| -> Vec<String> {
        let p = [v(10.0, 10.0), v(50.0, 10.0), v(50.0, 30.0), v(10.0, 30.0)];
        let l: Vec<String> = (0..4).map(|i| c.line(p[i], p[(i + 1) % 4])).collect();
        for i in 0..4 {
            c.coincident(&format!("{}.end", l[i]), &format!("{}.start", l[(i + 1) % 4]));
            c.hv(i % 2 == 0, &l[i]);
        }
        l
    };
    add("e1 rectangle 40 x 20 with both dimensioned: the width 40 -> 50 in the editor", &|c| {
        let l = rect(c);
        c.length(&l[1], 20.0);
        c.length(&l[0], 40.0);
        last(c)
    }, 50.0);
    add("e2 as e1, the height 20 -> 30", &|c| {
        let l = rect(c);
        c.length(&l[0], 40.0);
        c.length(&l[1], 20.0);
        last(c)
    }, 30.0);
    // L-bracket: 40 wide, 30 high, legs 8 thick, four lengths.
    add("e3 L-bracket, the bottom 40 -> 50 in the editor", &|c| {
        let p = [v(0.0, 0.0), v(40.0, 0.0), v(40.0, 8.0), v(8.0, 8.0), v(8.0, 30.0), v(0.0, 30.0)];
        let l: Vec<String> = (0..6).map(|i| c.line(p[i], p[(i + 1) % 6])).collect();
        for i in 0..6 {
            c.coincident(&format!("{}.end", l[i]), &format!("{}.start", l[(i + 1) % 6]));
            c.hv(i % 2 == 0, &l[i]);
        }
        c.length(&l[5], 30.0);
        c.length(&l[1], 8.0);
        c.length(&l[4], 22.0);
        c.length(&l[0], 40.0);
        last(c)
    }, 50.0);
    // Slot: centres 30 apart, radius 6.
    let slot = |c: &mut Case| -> [String; 4] {
        let l1 = c.line(v(0.0, -6.0), v(30.0, -6.0));
        let a2 = c.arc(v(30.0, 0.0), 6.0, -FRAC_PI_2, FRAC_PI_2);
        let l3 = c.line(v(30.0, 6.0), v(0.0, 6.0));
        let a4 = c.arc(v(0.0, 0.0), 6.0, FRAC_PI_2, 3.0 * FRAC_PI_2);
        c.coincident(&format!("{l1}.end"), &format!("{a2}.start"));
        c.coincident(&format!("{a2}.end"), &format!("{l3}.start"));
        c.coincident(&format!("{l3}.end"), &format!("{a4}.start"));
        c.coincident(&format!("{a4}.end"), &format!("{l1}.start"));
        for (a, b) in [(&l1, &a2), (&a2, &l3), (&l3, &a4), (&a4, &l1)] {
            c.pair("TANGENT", a, b);
        }
        c.pair("EQUAL", &a2, &a4);
        c.hv(true, &l1);
        [l1, a2, l3, a4]
    };
    add("e4 slot (centres 30 apart, R6, both dimensioned): the centre distance 30 -> 40 in the editor", &|c| {
        let [_, a2, _, a4] = slot(c);
        c.radius(&a2, 6.0, false);
        c.distance(&format!("{a4}.center"), &format!("{a2}.center"), 30.0);
        last(c)
    }, 40.0);
    add("e5 as e4, the radius 6 -> 8", &|c| {
        let [_, a2, _, a4] = slot(c);
        c.distance(&format!("{a4}.center"), &format!("{a2}.center"), 30.0);
        c.radius(&a2, 6.0, false);
        last(c)
    }, 8.0);
    // Rectangle 40 x 20 from (10, 10) with its four corners filleted R4.
    add("e6 filleted rectangle (R4, the straight bottom 32 dimensioned): the radius 4 -> 6 in the editor", &|c| {
        let (o, w, h, r) = (v(10.0, 10.0), 40.0, 20.0, 4.0);
        let corners = [o + v(w, 0.0), o + v(w, h), o + v(0.0, h), o];
        let pts = [(o + v(r, 0.0), o + v(w - r, 0.0)), (o + v(w, r), o + v(w, h - r)), (o + v(w - r, h), o + v(r, h)), (o + v(0.0, h - r), o + v(0.0, r))];
        let mut lines = Vec::new();
        let mut arcs = Vec::new();
        for i in 0..4 {
            lines.push(c.line(pts[i].0, pts[i].1));
            let cc = corners[i] + match i { 0 => v(-r, r), 1 => v(-r, -r), 2 => v(r, -r), _ => v(r, r) };
            let a0 = -FRAC_PI_2 + i as f64 * FRAC_PI_2;
            arcs.push(c.arc(cc, r, a0, a0 + FRAC_PI_2));
        }
        for i in 0..4 {
            c.coincident(&format!("{}.end", lines[i]), &format!("{}.start", arcs[i]));
            c.coincident(&format!("{}.end", arcs[i]), &format!("{}.start", lines[(i + 1) % 4]));
            c.pair("TANGENT", &lines[i], &arcs[i]);
            c.pair("TANGENT", &arcs[i], &lines[(i + 1) % 4]);
            c.hv(i % 2 == 0, &lines[i]);
        }
        for i in 1..4 {
            c.pair("EQUAL", &arcs[0], &arcs[i]);
        }
        c.length(&lines[0], w - 2.0 * r);
        c.radius(&arcs[0], r, false);
        last(c)
    }, 6.0);
    out
}

/// Writes the edit probes (LIVE_FUZZ_WRITE=probe-8): each case's `edit` =
/// the constraint and its new value, with ours after the editor's edit
/// (`scale_for_edit`, then the dimension edit's solve).
#[test]
#[ignore = "T2: live fuzz editor edit probes"]
fn edit_probe_matches() {
    let live: Option<Value> = std::fs::read_to_string(fixture("sketch-probe-8.live.json")).ok().and_then(|t| serde_json::from_str(&t).ok());
    let cases: Vec<Value> = edit_probes().into_iter().map(|(name, mut c, id, value)| {
        let o = ours(&mut c);
        let cid = c.cons.iter().find(|(n, _)| *n == id).and_then(|(_, x)| *x).unwrap();
        c.sk.set_dimension_value(cid, value * MM).unwrap();
        c.sk.scale_for_edit(cid);
        let opts = c.sk.dimension_edit_options(cid);
        let rep = c.sk.solve_with(&opts);
        let cname = name.split(' ').next().unwrap().to_string();
        json!({"case": cname, "what": name, "constraints": c.spec, "onshape": {"entities": c.os_entities, "constraints": c.os_constraints}, "ours": o, "edit": {"constraint": id, "value_mm": value, "ours_solved": rep.is_solved(), "ours": ours_points(&c)}})
    }).collect();
    let b = json!({"units": "mm (Onshape JSON: metres, radians)", "plane": "Top", "cases": cases});
    if std::env::var("LIVE_FUZZ_WRITE").is_ok_and(|v| v == "probe-8") {
        std::fs::write(fixture("sketch-probe-8.json"), serde_json::to_string_pretty(&b).unwrap()).unwrap();
    }
    let Some(live) = live else { return };
    let (mut total, mut ok) = (0, 0);
    for case in b["cases"].as_array().unwrap() {
        let Some(after) = live["cases"][case["case"].as_str().unwrap()]["edit"]["after"].as_object() else { continue };
        total += 1;
        let d: Vec<String> = after.iter().filter(|(n, p)| {
            let o = &case["edit"]["ours"][n.as_str()];
            (o[0].as_f64().unwrap_or(f64::NAN) - p[0].as_f64().unwrap_or(0.0)).hypot(o[1].as_f64().unwrap_or(f64::NAN) - p[1].as_f64().unwrap_or(0.0)) >= 1e-3
        }).map(|(n, p)| format!("{n}: ours {} live {p}", case["edit"]["ours"][n.as_str()])).collect();
        if d.is_empty() {
            ok += 1;
        } else {
            println!("  {} ({}): {}", case["case"], case["what"], d.join("; "));
        }
    }
    println!("live edit probes: {total} answered, {ok} match");
}

/// Drag probes 9: everyday profiles with their dimensions, one point dragged
/// in the UI by (dx, dy) mm (after probes 7 on minimal sketches).
fn drag_probes9() -> Vec<(String, Case, String, DVec2)> {
    use std::f64::consts::FRAC_PI_2;
    let v = DVec2::new;
    let mut out = Vec::new();
    let mut add = |name: &str, point: &str, by: DVec2, f: &dyn Fn(&mut Case)| {
        let mut c = Case::new();
        f(&mut c);
        out.push((name.to_string(), c, point.to_string(), by));
    };
    let lbracket = |c: &mut Case, dims: usize| {
        let p = [v(0.0, 0.0), v(40.0, 0.0), v(40.0, 8.0), v(8.0, 8.0), v(8.0, 30.0), v(0.0, 30.0)];
        let l: Vec<String> = (0..6).map(|i| c.line(p[i], p[(i + 1) % 6])).collect();
        for i in 0..6 {
            c.coincident(&format!("{}.end", l[i]), &format!("{}.start", l[(i + 1) % 6]));
            c.hv(i % 2 == 0, &l[i]);
        }
        let all = [(0, 40.0), (5, 30.0), (1, 8.0), (4, 22.0)];
        for (i, mm) in all.iter().take(dims) {
            c.length(&l[*i], *mm);
        }
    };
    add("g1 L-bracket, all four lengths: its top-right corner (8,30) dragged by (4, 3)", "l4.end", v(4.0, 3.0), &|c| lbracket(c, 4));
    add("g2 L-bracket, only the bottom and the left side dimensioned: the inner corner (8,8) dragged by (4, 3)", "l3.end", v(4.0, 3.0), &|c| lbracket(c, 2));
    let slot = |c: &mut Case, dims: bool| {
        let l1 = c.line(v(0.0, -6.0), v(30.0, -6.0));
        let a2 = c.arc(v(30.0, 0.0), 6.0, -FRAC_PI_2, FRAC_PI_2);
        let l3 = c.line(v(30.0, 6.0), v(0.0, 6.0));
        let a4 = c.arc(v(0.0, 0.0), 6.0, FRAC_PI_2, 3.0 * FRAC_PI_2);
        c.coincident(&format!("{l1}.end"), &format!("{a2}.start"));
        c.coincident(&format!("{a2}.end"), &format!("{l3}.start"));
        c.coincident(&format!("{l3}.end"), &format!("{a4}.start"));
        c.coincident(&format!("{a4}.end"), &format!("{l1}.start"));
        for (a, b) in [(&l1, &a2), (&a2, &l3), (&l3, &a4), (&a4, &l1)] {
            c.pair("TANGENT", a, b);
        }
        c.pair("EQUAL", &a2, &a4);
        c.hv(true, &l1);
        if dims {
            c.radius(&a2, 6.0, false);
            c.distance(&format!("{a4}.center"), &format!("{a2}.center"), 30.0);
        }
    };
    add("g3 slot with its radius and centre distance: the right arc's centre (30,0) dragged by (4, 3)", "a2.center", v(4.0, 3.0), &|c| slot(c, true));
    add("g4 the slot without dimensions: the top line's start (30,6) dragged by (4, 3)", "l3.start", v(4.0, 3.0), &|c| slot(c, false));
    add("g5 filleted rectangle (R4, bottom 32 dimensioned): the top-right arc's centre dragged by (3, 2)", "a4.center", v(3.0, 2.0), &|c| {
        let (o, w, h, r) = (v(10.0, 10.0), 40.0, 20.0, 4.0);
        let corners = [o + v(w, 0.0), o + v(w, h), o + v(0.0, h), o];
        let pts = [(o + v(r, 0.0), o + v(w - r, 0.0)), (o + v(w, r), o + v(w, h - r)), (o + v(w - r, h), o + v(r, h)), (o + v(0.0, h - r), o + v(0.0, r))];
        let mut lines = Vec::new();
        let mut arcs = Vec::new();
        for i in 0..4 {
            lines.push(c.line(pts[i].0, pts[i].1));
            let cc = corners[i] + match i { 0 => v(-r, r), 1 => v(-r, -r), 2 => v(r, -r), _ => v(r, r) };
            let a0 = -FRAC_PI_2 + i as f64 * FRAC_PI_2;
            arcs.push(c.arc(cc, r, a0, a0 + FRAC_PI_2));
        }
        for i in 0..4 {
            c.coincident(&format!("{}.end", lines[i]), &format!("{}.start", arcs[i]));
            c.coincident(&format!("{}.end", arcs[i]), &format!("{}.start", lines[(i + 1) % 4]));
            c.pair("TANGENT", &lines[i], &arcs[i]);
            c.pair("TANGENT", &arcs[i], &lines[(i + 1) % 4]);
            c.hv(i % 2 == 0, &lines[i]);
        }
        for i in 1..4 {
            c.pair("EQUAL", &arcs[0], &arcs[i]);
        }
        c.length(&lines[0], w - 2.0 * r);
        c.radius(&arcs[0], r, false);
    });
    add("g6 bolt circle (big Ø40, three Ø6 holes on a 14 mm radius): a hole's centre dragged by (3, 2)", "c2.center", v(3.0, 2.0), &|c| {
        let cb = c.circle(v(0.0, 0.0), 20.0);
        let holes: Vec<String> = (0..3).map(|i| c.circle(DVec2::from_angle(FRAC_PI_2 + i as f64 * 4.0 * FRAC_PI_2 / 3.0) * 14.0, 3.0)).collect();
        for hh in &holes[1..] {
            c.pair("EQUAL", &holes[0], hh);
        }
        c.radius(&cb, 20.0, true);
        c.radius(&holes[0], 3.0, true);
        for hh in &holes {
            c.distance(&format!("{cb}.center"), &format!("{hh}.center"), 14.0);
        }
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
    for (n, list) in [(1, probes()), (2, probes2()), (3, probes3()), (4, probes4()), (5, probes5()), (6, probes6()), (10, probes10()), (11, probes11()), (13, probes13())] {
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

/// One generated case (FUZZ_SEED=<seed>; 101 on are rich) or probe
/// (FUZZ_PROBE=t1) solved as live_matches does, its points printed as JSON.
#[test]
#[ignore = "T2: one live fuzz case"]
fn one_case() {
    let probe = std::env::var("FUZZ_PROBE").ok();
    let Some(mut c) = probe.as_ref().and_then(|p| [probes(), probes2(), probes3(), probes4(), probes5(), probes6(), probes10(), probes11(), probes13()].into_iter().flatten().find(|(n, _)| n.split(' ').next() == Some(p.as_str())).map(|(_, c)| c)).or_else(|| {
        let seed: u64 = std::env::var("FUZZ_SEED").ok()?.parse().ok()?;
        Some(generate(seed, seed > 100))
    }) else {
        return;
    };
    if std::env::var("CTRACE").is_ok() {
        eprintln!("CT names {:?}", c.curves.iter().map(|(n, id, _)| format!("{n}={id:?}")).collect::<Vec<_>>());
    }
    let o = ours(&mut c);
    eprintln!("{}", o["points"]);
}
