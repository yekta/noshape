//! P1 features option by option against live Onshape (fs oracle_p1, lead
//! 10-25): each case of /root/onshape-research/live/fs-oracle-p1/CASES.md
//! built natively and compared with live's status, message, volume, face
//! count, surface area and part count (live/fs-oracle-p1/results-*.jsonl).

use glam::{DVec2, DVec3};
use noshape_model::configuration::ConfigValues;
use noshape_model::display::{BodyKind, FeatureStatus};
use noshape_model::params::{ParamValue, ParamValues};
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, FeatureId, Reference};
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};

/// A Part Studio built feature by feature; the last feature is the case's.
struct Studio {
    ps: PartStudio,
    units: UnitPrefs,
}

impl Studio {
    fn new() -> Self {
        Self { ps: PartStudio::new(), units: UnitPrefs::default() }
    }

    fn add(&mut self, ty: &str, set: impl FnOnce(&mut ParamValues)) -> FeatureId {
        let mut f = self.ps.new_feature(ty, &self.units).unwrap_or_else(|| panic!("unknown feature {ty}"));
        set(&mut f.params);
        let id = f.id;
        self.ps.features.push(f);
        id
    }

    fn regen(&self) -> RegenResult {
        Regenerator::new().regenerate(&self.ps, &ConfigValues::default(), &self.units, &RegenOptions::default())
    }

    /// A sketch on `plane` (mm, sketch coordinates); its regions, largest first.
    fn sketch(&mut self, plane: Reference, draw: impl FnOnce(&mut noshape_sketch::Sketch)) -> Vec<Reference> {
        let mut sk = noshape_sketch::Sketch::new();
        draw(&mut sk);
        let sketch = self.add("sketch", |pv| {
            pv.set("sketchPlane", ParamValue::Query(vec![plane]));
            pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
        });
        let r = self.regen();
        let sk = r.display.sketches.iter().find(|s| s.feature == sketch).expect("sketch");
        let mut regions: Vec<_> = sk.regions.iter().collect();
        regions.sort_by(|a, b| b.area.total_cmp(&a.area));
        regions.into_iter().map(|g| Reference::SketchRegion { sketch, key: g.key.clone() }).collect()
    }

    /// BLOCK: rectangle (0,0)–(60,40) on Top, extruded 20 mm.
    fn block() -> Self {
        let mut s = Self::new();
        let r = s.sketch(top(), |sk| rect(sk, (0.0, 0.0), (60.0, 40.0)));
        s.add("extrude", |p| {
            p.set("entities", ParamValue::Query(vec![r[0].clone()]));
            p.set("depth", mm(20.0));
        });
        s
    }
}

fn top() -> Reference {
    Reference::Plane(DatumPlane::Top)
}

fn mm(x: f64) -> ParamValue {
    ParamValue::Expr(ExprValue::new(format!("{x} mm")))
}

fn deg(x: f64) -> ParamValue {
    ParamValue::Expr(ExprValue::new(format!("{x} deg")))
}

fn en(x: &str) -> ParamValue {
    ParamValue::Enum(x.into())
}

fn q(r: Reference) -> ParamValue {
    ParamValue::Query(vec![r])
}

fn rect(sk: &mut noshape_sketch::Sketch, a: (f64, f64), b: (f64, f64)) {
    let p = [(a.0, a.1), (b.0, a.1), (b.0, b.1), (a.0, b.1)].map(|(x, y)| sk.add_point(DVec2::new(x * 1e-3, y * 1e-3)));
    for i in 0..4 {
        sk.add_line_between(p[i], p[(i + 1) % 4]).unwrap();
    }
}

fn circle(sk: &mut noshape_sketch::Sketch, c: (f64, f64), d: f64) {
    sk.add_circle(DVec2::new(c.0 * 1e-3, c.1 * 1e-3), d * 0.5e-3);
}

/// The face / vertex of a body through `p` (mm).
fn face_at(r: &RegenResult, p: DVec3) -> Reference {
    let v = noshape_kernel::make_vertex(p * 1e-3).unwrap();
    for b in &r.context.bodies {
        for (i, f) in b.shape.faces().iter().enumerate() {
            if noshape_kernel::distance(&v, f).is_ok_and(|d| d.distance < 1e-9) {
                return Reference::Face((*b.faces[i]).clone());
            }
        }
    }
    panic!("no face at {p:?}");
}

fn vertex_at(r: &RegenResult, p: DVec3) -> Reference {
    let v = noshape_kernel::make_vertex(p * 1e-3).unwrap();
    for b in &r.context.bodies {
        for (i, x) in b.shape.vertices().iter().enumerate() {
            if noshape_kernel::distance(&v, x).is_ok_and(|d| d.distance < 1e-9) {
                return Reference::Vertex((*b.vertices[i]).clone());
            }
        }
    }
    panic!("no vertex at {p:?}");
}

/// What a case gives: status, message, volume (mm³), faces, area (mm²), parts.
#[derive(Debug)]
struct Outcome {
    status: &'static str,
    text: String,
    volume: f64,
    faces: usize,
    area: f64,
    parts: usize,
}

fn outcome(r: &RegenResult) -> Outcome {
    let s = r.display.features.last().expect("a feature");
    let status = match s.status {
        FeatureStatus::Ok => "OK",
        FeatureStatus::Warning => "WARNING",
        FeatureStatus::Info => "INFO",
        _ => "ERROR",
    };
    let parts: Vec<_> = r.context.bodies.iter().filter(|b| b.kind == BodyKind::Solid).collect();
    Outcome {
        status,
        text: s.message.clone().unwrap_or_default(),
        volume: parts.iter().map(|b| b.shape.volume().unwrap_or(0.0).abs()).sum::<f64>() * 1e9,
        faces: parts.iter().map(|b| b.shape.faces().len()).sum(),
        area: parts.iter().map(|b| b.shape.area().unwrap_or(0.0)).sum::<f64>() * 1e6,
        parts: parts.len(),
    }
}

/// Batch 1: Extrude (CASES.md).
fn extrude_cases() -> Vec<(&'static str, Studio)> {
    let mut cases = Vec::new();
    // CIRC: Ø10 at (20,20) on the block's top face; CIRC0: on Top; FAR: at (100,20) on Top.
    let on_block = |set: &dyn Fn(&mut ParamValues, &RegenResult)| {
        let mut s = Studio::block();
        let r = s.regen();
        let c = s.sketch(face_at(&r, DVec3::new(30.0, 20.0, 20.0)), |sk| circle(sk, (20.0, 20.0), 10.0));
        s.add("extrude", |p| {
            p.set("entities", q(c[0].clone()));
            set(p, &r);
        });
        s
    };
    let on_top = |at: (f64, f64), set: &dyn Fn(&mut ParamValues, &RegenResult)| {
        let mut s = Studio::block();
        let r = s.regen();
        let c = s.sketch(top(), |sk| circle(sk, at, 10.0));
        s.add("extrude", |p| {
            p.set("entities", q(c[0].clone()));
            set(p, &r);
        });
        s
    };
    let cube = |r: &RegenResult| q(Reference::Body(r.context.bodies[0].id.clone()));
    cases.push(("E1", on_block(&|p, r| {
        p.set("operationType", en("ADD"));
        p.set("booleanScope", cube(r));
        p.set("depth", mm(10.0));
    })));
    cases.push(("E2", on_block(&|p, r| {
        p.set("operationType", en("REMOVE"));
        p.set("booleanScope", cube(r));
        p.set("depth", mm(10.0));
        p.set("oppositeDirection", ParamValue::Bool(true));
    })));
    cases.push(("E3", on_block(&|p, r| {
        p.set("operationType", en("REMOVE"));
        p.set("booleanScope", cube(r));
        p.set("endBound", en("THROUGH_ALL"));
        p.set("oppositeDirection", ParamValue::Bool(true));
    })));
    cases.push(("E4", on_block(&|p, r| {
        p.set("operationType", en("REMOVE"));
        p.set("booleanScope", cube(r));
        p.set("endBound", en("UP_TO_NEXT"));
        p.set("oppositeDirection", ParamValue::Bool(true));
    })));
    cases.push(("E5", on_top((100.0, 20.0), &|p, _| {
        p.set("operationType", en("NEW"));
        p.set("symmetric", ParamValue::Bool(true));
        p.set("depth", mm(10.0));
    })));
    cases.push(("E6", on_top((100.0, 20.0), &|p, _| {
        p.set("operationType", en("NEW"));
        p.set("depth", mm(10.0));
        p.set("hasSecondDirection", ParamValue::Bool(true));
        p.set("secondDirectionDepth", mm(5.0));
    })));
    cases.push(("E7", on_block(&|p, r| {
        p.set("operationType", en("ADD"));
        p.set("booleanScope", cube(r));
        p.set("depth", mm(10.0));
        p.set("hasDraft", ParamValue::Bool(true));
        p.set("draftAngle", deg(5.0));
    })));
    // Thin: the sketch's circle, 2 mm (live: the wall lies outside, r 5..7).
    cases.push(("E8", on_block(&|p, r| {
        p.set("operationType", en("ADD"));
        p.set("booleanScope", cube(r));
        p.set("depth", mm(10.0));
        p.set("bodyType", en("THIN"));
        if let ParamValue::Query(e) = p.get("entities").cloned().unwrap_or(ParamValue::Query(Vec::new())) {
            p.set("wallShape", ParamValue::Query(e));
        }
        p.set("thickness1", mm(2.0));
    })));
    cases.push(("E9", on_top((20.0, 20.0), &|p, r| {
        p.set("operationType", en("ADD"));
        p.set("booleanScope", cube(r));
        p.set("endBound", en("UP_TO_SURFACE"));
        p.set("endBoundEntityFace", q(face_at(r, DVec3::new(30.0, 20.0, 20.0))));
    })));
    cases.push(("E10", on_top((20.0, 20.0), &|p, r| {
        p.set("operationType", en("NEW"));
        p.set("endBound", en("UP_TO_VERTEX"));
        p.set("endBoundEntityVertex", q(vertex_at(r, DVec3::new(60.0, 40.0, 20.0))));
    })));
    cases.push(("E11", on_top((20.0, 20.0), &|p, r| {
        p.set("operationType", en("ADD"));
        p.set("booleanScope", cube(r));
        p.set("endBound", en("UP_TO_BODY"));
        p.set("endBoundEntityBody", cube(r));
    })));
    cases.push(("E12", on_block(&|p, r| {
        p.set("operationType", en("ADD"));
        p.set("booleanScope", cube(r));
        p.set("depth", mm(10.0));
        p.set("startOffset", ParamValue::Bool(true));
        p.set("startOffsetDistance", mm(5.0));
    })));
    cases.push(("E13", on_top((100.0, 20.0), &|p, r| {
        p.set("operationType", en("REMOVE"));
        p.set("booleanScope", cube(r));
        p.set("depth", mm(10.0));
    })));
    // Up to a plane 50 mm above Top; E14b: up to the Right plane (parallel).
    for (name, offset) in [("E14", Some(50.0)), ("E14b", None)] {
        let mut s = Studio::block();
        let r = s.regen();
        let limit = match offset {
            Some(d) => {
                let plane = s.add("cPlane", |p| {
                    p.set("entities", q(top()));
                    p.set("cplaneType", en("OFFSET"));
                    p.set("offset", mm(d));
                });
                Reference::FeaturePlane(plane)
            }
            None => Reference::Plane(DatumPlane::Right),
        };
        let c = s.sketch(top(), |sk| circle(sk, (20.0, 20.0), 10.0));
        s.add("extrude", |p| {
            p.set("entities", q(c[0].clone()));
            p.set("operationType", en("ADD"));
            p.set("booleanScope", cube(&r));
            p.set("endBound", en("UP_TO_SURFACE"));
            p.set("endBoundEntityFace", q(limit));
        });
        cases.push((name, s));
    }
    cases.push(("E15", on_block(&|p, r| {
        p.set("operationType", en("REMOVE"));
        p.set("booleanScope", cube(r));
        p.set("depth", mm(25.0));
        p.set("oppositeDirection", ParamValue::Bool(true));
    })));
    cases
}

/// Batch 2: Revolve. RPROF: rectangle (40,0)–(50,10) on Front, about the
/// vertical line x = 0 of a second Front sketch.
fn revolve_cases() -> Vec<(&'static str, Studio)> {
    let mut cases = Vec::new();
    let revolve = |set: &dyn Fn(&mut ParamValues, Reference)| {
        let mut s = Studio::new();
        let front = Reference::Plane(DatumPlane::Front);
        let profile = s.sketch(front.clone(), |sk| rect(sk, (40.0, 0.0), (50.0, 10.0)));
        let mut sk = noshape_sketch::Sketch::new();
        let line = sk.add_line(DVec2::new(0.0, 0.0), DVec2::new(0.0, 0.02));
        let axis_sketch = s.add("sketch", |pv| {
            pv.set("sketchPlane", q(front));
            pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
        });
        let axis = Reference::SketchEntity { sketch: axis_sketch, entity: line.0 };
        s.add("revolve", |p| {
            p.set("entities", q(profile[0].clone()));
            p.set("axis", q(axis.clone()));
            set(p, profile[0].clone());
        });
        s
    };
    cases.push(("R1", revolve(&|p, _| p.set("fullRevolve", ParamValue::Bool(true)))));
    cases.push(("R2", revolve(&|p, _| {
        p.set("fullRevolve", ParamValue::Bool(false));
        p.set("angle", deg(90.0));
    })));
    cases.push(("R3", revolve(&|p, _| {
        p.set("fullRevolve", ParamValue::Bool(false));
        p.set("symmetric", ParamValue::Bool(true));
        p.set("angle", deg(90.0));
    })));
    cases.push(("R4", revolve(&|p, _| {
        p.set("fullRevolve", ParamValue::Bool(false));
        p.set("angle", deg(90.0));
        p.set("hasStartBound", ParamValue::Bool(true));
        p.set("angleBack", deg(45.0));
    })));
    cases.push(("R5", revolve(&|p, region| {
        p.set("fullRevolve", ParamValue::Bool(true));
        p.set("bodyType", en("THIN"));
        p.set("wallShape", q(region));
        p.set("thickness1", mm(1.0));
    })));
    cases
}

/// Differences reported to their owners (case, owner and what differs);
/// remove a line once it matches.
const KNOWN: &[(&str, &str)] = &[
    ("E9", "psb: an Add that changes nothing ends with INFO \"Boolean resulted in no geometry change. The parts either do not intersect or are totally contained.\" (ours OK)"),
    ("E11", "psb: as E9 (Up to part)"),
    ("E12", "psb: as E9 (start offset, the boss doesn't touch the part)"),
    ("E13", "psb: a Remove that misses its scope ends with INFO \"Selected tools and targets do not intersect.\" (ours OK)"),
    ("R4", "psb: Second end position is a start angle in the same direction (45° to 90°: 3534.29 mm³); ours sweeps 45° the other way (135°)"),
    ("R5", "psb: Thin revolve of the rectangle: live's 1 mm wall lies inside (10178.76 mm³), ours outside (12440.71); the circle's (E8) lies outside in both"),
];

/// Live's results (results-*.jsonl): case, status, text, volume, faces, area, parts.
const LIVE: &[(&str, &str, &str, f64, usize, f64, usize)] = &[
    ("E1", "OK", "", 48785.3982, 8, 9114.1593, 1),
    ("E2", "OK", "", 47214.6018, 8, 9114.1593, 1),
    ("E3", "OK", "", 46429.2037, 7, 9271.2389, 1),
    ("E4", "OK", "", 46429.2037, 7, 9271.2389, 1),
    ("E5", "OK", "", 48785.3982, 9, 9271.2389, 2),
    ("E6", "OK", "", 49178.0972, 9, 9428.3185, 2),
    ("E7", "OK", "", 48930.8406, 8, 9172.8397, 1),
    ("E8", "OK", "", 48753.9822, 10, 9553.9822, 1),
    ("E9", "INFO", "Boolean resulted in no geometry change. The parts either do not intersect or are totally contained.", 48000.0, 6, 8800.0, 1),
    ("E10", "OK", "", 49570.7963, 9, 9585.3982, 2),
    ("E11", "INFO", "Boolean resulted in no geometry change. The parts either do not intersect or are totally contained.", 48000.0, 6, 8800.0, 1),
    ("E12", "INFO", "Boolean resulted in no geometry change. The parts either do not intersect or are totally contained.", 48785.3982, 9, 9271.2389, 2),
    ("E13", "INFO", "Selected tools and targets do not intersect.", 48000.0, 6, 8800.0, 1),
    ("E14", "OK", "", 50356.1945, 8, 9742.4778, 1),
    ("E14b", "ERROR", "Failed to extrude selections, check input.", 48000.0, 6, 8800.0, 1),
    ("E15", "OK", "", 46429.2037, 7, 9271.2389, 1),
    ("R1", "OK", "", 28274.3339, 4, 11309.7336, 1),
    ("R2", "OK", "", 7068.5835, 6, 3027.4334, 1),
    ("R3", "OK", "", 7068.5835, 6, 3027.4334, 1),
    ("R4", "OK", "", 3534.2917, 6, 1613.7167, 1),
    ("R4b", "OK", "", 3534.2917, 6, 1613.7167, 1),
    ("R5", "OK", "", 10178.7602, 8, 20357.5204, 1),
    ("R6", "OK", "", 47497.3452, 8, 8841.0619, 1),
];

#[test]
fn p1_features_match_live() {
    let mut report = String::new();
    for (name, s) in extrude_cases().into_iter().chain(revolve_cases()) {
        let got = outcome(&s.regen());
        eprintln!("{name}: {got:?}");
        let Some(&(_, status, text, volume, faces, area, parts)) = LIVE.iter().find(|l| l.0 == name) else { continue };
        let near = |a: f64, b: f64| (a - b).abs() <= 1e-3 * b.abs().max(1.0) * 1e-3 + 1e-4;
        if got.status != status || got.text != text || !near(got.volume, volume) || got.faces != faces || !near(got.area, area) || got.parts != parts {
            if !KNOWN.iter().any(|k| k.0 == name) {
                report += &format!("{name}: ours {got:?}\n      live ({status}, {text:?}, {volume}, {faces}, {area}, {parts})\n");
            }
        } else if let Some(k) = KNOWN.iter().find(|k| k.0 == name) {
            report += &format!("{name} now matches live: drop it from KNOWN ({})\n", k.1);
        }
    }
    assert!(report.is_empty(), "differences from live:\n{report}");
}
