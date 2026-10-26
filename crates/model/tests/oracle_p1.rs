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

fn edge_at(r: &RegenResult, p: DVec3) -> Reference {
    let v = noshape_kernel::make_vertex(p * 1e-3).unwrap();
    for b in &r.context.bodies {
        for (i, e) in b.shape.edges().iter().enumerate() {
            if noshape_kernel::distance(&v, e).is_ok_and(|d| d.distance < 1e-9) {
                return Reference::Edge((*b.edges[i]).clone());
            }
        }
    }
    panic!("no edge at {p:?}");
}

/// The block's edges and faces by a point on them (mm).
const FRONT_TOP: DVec3 = DVec3::new(30.0, 0.0, 20.0);
const BACK_TOP: DVec3 = DVec3::new(30.0, 40.0, 20.0);
const LEFT_TOP: DVec3 = DVec3::new(0.0, 20.0, 20.0);
const RIGHT_TOP: DVec3 = DVec3::new(60.0, 20.0, 20.0);
const FRONT_LEFT: DVec3 = DVec3::new(0.0, 0.0, 10.0);
const TOP: DVec3 = DVec3::new(30.0, 20.0, 20.0);
const BOTTOM: DVec3 = DVec3::new(30.0, 20.0, 0.0);
const SIDES: [DVec3; 4] = [DVec3::new(30.0, 0.0, 10.0), DVec3::new(30.0, 40.0, 10.0), DVec3::new(0.0, 20.0, 10.0), DVec3::new(60.0, 20.0, 10.0)];

/// One feature `ty` on BLOCK.
fn on_block(ty: &str, set: impl FnOnce(&mut ParamValues, &RegenResult)) -> Studio {
    let mut s = Studio::block();
    let r = s.regen();
    s.add(ty, |p| set(p, &r));
    s
}

fn edges(r: &RegenResult, at: &[DVec3]) -> ParamValue {
    ParamValue::Query(at.iter().map(|p| edge_at(r, *p)).collect())
}

/// Batch 3: Fillet; batch 4: Chamfer; batch 6: Shell and Draft (CASES.md).
fn blend_cases() -> Vec<(&'static str, Studio)> {
    let mut cases = Vec::new();
    let corner = [FRONT_TOP, LEFT_TOP, FRONT_LEFT];
    let tops = [FRONT_TOP, BACK_TOP, LEFT_TOP, RIGHT_TOP];
    cases.push(("F1", on_block("fillet", |p, r| {
        p.set("entities", edges(r, &[FRONT_TOP]));
        p.set("radius", mm(5.0));
    })));
    cases.push(("F2", on_block("fillet", |p, r| {
        p.set("entities", edges(r, &tops));
        p.set("radius", mm(5.0));
    })));
    cases.push(("F3", on_block("fillet", |p, r| {
        p.set("entities", edges(r, &corner));
        p.set("radius", mm(3.0));
    })));
    cases.push(("F4", on_block("fillet", |p, r| {
        p.set("entities", edges(r, &[FRONT_TOP]));
        p.set("radius", mm(2.0));
        p.set("isVariable", ParamValue::Bool(true));
        let vertex = |at: DVec3, radius: f64| {
            let mut v = ParamValues::default();
            v.set("vertex", q(vertex_at(r, at)));
            v.set("vertexRadius", mm(radius));
            v
        };
        p.set("vertexSettings", ParamValue::Array(vec![vertex(DVec3::new(0.0, 0.0, 20.0), 2.0), vertex(DVec3::new(60.0, 0.0, 20.0), 6.0)]));
    })));
    cases.push(("F5", on_block("fillet", |p, r| {
        p.set("entities", edges(r, &[FRONT_TOP]));
        p.set("crossSection", en("CONIC"));
        p.set("blendControlType", en("WIDTH"));
        p.set("width", mm(5.0));
        p.set("rho", ParamValue::Expr(ExprValue::new("0.7")));
    })));
    cases.push(("F6", on_block("fillet", |p, r| {
        p.set("entities", edges(r, &[FRONT_TOP]));
        p.set("crossSection", en("CURVATURE"));
        p.set("radius", mm(5.0));
    })));
    cases.push(("F7", on_block("fillet", |p, r| {
        p.set("entities", edges(r, &[FRONT_TOP]));
        p.set("radius", mm(25.0));
        p.set("allowEdgeOverflow", ParamValue::Bool(false));
    })));
    let mut f8 = on_block("fillet", |p, r| {
        p.set("entities", edges(r, &[FRONT_TOP]));
        p.set("radius", mm(5.0));
    });
    let r = f8.regen();
    f8.add("fillet", |p| {
        p.set("entities", edges(&r, &[BACK_TOP]));
        p.set("radius", mm(5.0));
        p.set("tangentPropagation", ParamValue::Bool(true));
    });
    cases.push(("F8", f8));
    cases.push(("F9", on_block("fillet", |p, r| {
        p.set("entities", edges(r, &corner));
        p.set("radius", mm(3.0));
        p.set("smoothCorners", ParamValue::Bool(true));
    })));
    let chamfer = |name: &'static str, at: &[DVec3], set: &dyn Fn(&mut ParamValues)| {
        let at = at.to_vec();
        (name, on_block("chamfer", move |p, r| {
            p.set("entities", edges(r, &at));
            set(p);
        }))
    };
    cases.push(chamfer("C1", &[FRONT_TOP], &|p| {
        p.set("chamferType", en("EQUAL_OFFSETS"));
        p.set("width", mm(3.0));
    }));
    cases.push(chamfer("C2", &[FRONT_TOP], &|p| {
        p.set("chamferType", en("TWO_OFFSETS"));
        p.set("width1", mm(3.0));
        p.set("width2", mm(5.0));
    }));
    cases.push(chamfer("C3", &[FRONT_TOP], &|p| {
        p.set("chamferType", en("OFFSET_ANGLE"));
        p.set("width", mm(3.0));
        p.set("angle", deg(30.0));
    }));
    cases.push(chamfer("C4", &tops, &|p| {
        p.set("chamferType", en("EQUAL_OFFSETS"));
        p.set("width", mm(2.0));
        p.set("tangentPropagation", ParamValue::Bool(true));
    }));
    cases.push(chamfer("C5", &[FRONT_TOP], &|p| {
        p.set("chamferType", en("EQUAL_OFFSETS"));
        p.set("width", mm(25.0));
    }));
    let shell = |name: &'static str, set: &dyn Fn(&mut ParamValues, &RegenResult)| (name, on_block("shell", |p, r| set(p, r)));
    cases.push(shell("S1", &|p, r| {
        p.set("entities", q(face_at(r, TOP)));
        p.set("thickness", mm(2.0));
    }));
    cases.push(shell("S2", &|p, r| {
        p.set("entities", q(face_at(r, TOP)));
        p.set("thickness", mm(2.0));
        p.set("oppositeDirection", ParamValue::Bool(true));
    }));
    cases.push(shell("S3", &|p, r| {
        p.set("isHollow", ParamValue::Bool(true));
        p.set("parts", q(Reference::Body(r.context.bodies[0].id.clone())));
        p.set("thickness", mm(2.0));
    }));
    cases.push(shell("S4", &|p, r| {
        p.set("entities", q(face_at(r, TOP)));
        p.set("thickness", mm(25.0));
    }));
    let draft = |name: &'static str, set: &dyn Fn(&mut ParamValues, &RegenResult)| (name, on_block("draft", |p, r| set(p, r)));
    cases.push(draft("D1", &|p, r| {
        p.set("neutralPlane", q(face_at(r, TOP)));
        p.set("draftFaces", ParamValue::Query(SIDES.iter().map(|x| face_at(r, *x)).collect()));
        p.set("angle", deg(5.0));
    }));
    cases.push(draft("D2", &|p, r| {
        p.set("neutralPlane", q(face_at(r, BOTTOM)));
        p.set("draftFaces", q(face_at(r, SIDES[0])));
        p.set("angle", deg(10.0));
    }));
    cases.push(draft("D3", &|p, r| {
        p.set("neutralPlane", q(face_at(r, TOP)));
        p.set("draftFaces", ParamValue::Query(SIDES.iter().map(|x| face_at(r, *x)).collect()));
        p.set("angle", deg(5.0));
        p.set("pullDirection", ParamValue::Bool(true));
    }));
    cases
}

/// Batch 5: Hole at a sketch point (20,20) on the block's top face (H7: at
/// (100,20) on Top, off the part).
fn hole_cases() -> Vec<(&'static str, Studio)> {
    let hole = |name: &'static str, off_part: bool, set: &dyn Fn(&mut ParamValues)| {
        let mut s = Studio::block();
        let r = s.regen();
        let (plane, at) = if off_part { (top(), (100.0, 20.0)) } else { (face_at(&r, TOP), (20.0, 20.0)) };
        let mut sk = noshape_sketch::Sketch::new();
        let point = sk.add_point(DVec2::new(at.0 * 1e-3, at.1 * 1e-3));
        let sketch = s.add("sketch", |pv| {
            pv.set("sketchPlane", q(plane));
            pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
        });
        s.add("hole", |p| {
            p.set("locations", q(Reference::SketchEntity { sketch, entity: point.0 }));
            p.set("unitsSystem", en("METRIC"));
            noshape_model::features::hole_tables::apply_table_to_params(p);
            if !off_part {
                p.set("scope", q(Reference::Body(r.context.bodies[0].id.clone())));
            }
            p.set("styleV2", en("SIMPLE"));
            p.set("holeDiameterV3", mm(6.0));
            set(p);
        });
        (name, s)
    };
    let blind = |p: &mut ParamValues| {
        p.set("endStyleV2", en("BLIND"));
        p.set("holeDepthV3", mm(10.0));
    };
    vec![
        hole("H1", false, &blind),
        hole("H2", false, &|p| p.set("endStyleV2", en("THROUGH"))),
        hole("H3", false, &|p| {
            p.set("endStyleV2", en("THROUGH"));
            p.set("styleV2", en("C_BORE"));
            p.set("cBoreDiameterV3", mm(11.0));
            p.set("cBoreDepthV3", mm(6.0));
        }),
        hole("H4", false, &|p| {
            p.set("endStyleV2", en("THROUGH"));
            p.set("styleV2", en("C_SINK"));
            p.set("cSinkDiameterV3", mm(12.0));
            p.set("cSinkAngleV3", deg(90.0));
        }),
        hole("H6", false, &blind),
        hole("H7", true, &blind),
    ]
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
    ("R5", "psb: Thin revolve wall side follows the loop direction (live fs 10-25 j): counter-clockwise in sketch coordinates inside, clockwise outside (Thin extrude: closed loops always outside, open chains right of travel); this CCW rectangle: live inside 10178.76 mm³, ours outside 12440.71"),
    ("F2", "kern: R5 on the 4 top edges, corner blends: ours 46974.930 mm³ / 8356.638 mm², live 46976.458 / 8356.751"),
    ("F4", "kern: variable fillet R2 → R6 along the edge: ours 47776.798 mm³, live 47776.692"),
    ("F5", "kern: conic fillet width 5, rho 0.7: ours 47945.138 mm³ / 8747.291 mm², live 47945.028 / 8747.516"),
    ("F6", "kern: curvature (G2) fillet R5, magnitude 0.5: ours 47755.024 mm³ / 8678.783 mm², live 47754.935 / 8679.694"),
    ("F7", "psb: R25 with Allow edge overflow OFF: live 40608.859 mm³ / 7938.092 mm² (6 faces), ours 40002.735 / 7857.276, the overflow-on result"),
    ("H7", "psb: a hole off the part with an empty Merge scope: live \"No target parts selected for hole.\" (HOLE_EMPTY_SCOPE), ours \"None of the holes intersected a part.\""),
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
    ("F1", "OK", "", 47678.0972, 7, 8660.5088, 1),
    ("F2", "OK", "", 46976.458, 10, 8356.751, 1),
    ("F3", "OK", "", 47772.7499, 10, 8638.4181, 1),
    ("F4", "OK", "", 47776.692, 7, 8688.477, 1),
    ("F5", "OK", "", 47945.0278, 7, 8747.5155, 1),
    ("F6", "OK", "", 47754.9351, 7, 8679.6943, 1),
    ("F7", "OK", "", 40608.8586, 6, 7938.0924, 1),
    ("F8", "OK", "", 47356.1945, 8, 8521.0176, 1),
    ("F9", "OK", "", 47772.7499, 10, 8638.4181, 1),
    ("C1", "OK", "", 47730.0, 7, 8685.5584, 1),
    ("C2", "OK", "", 47550.0, 7, 8654.8571, 1),
    ("C3", "OK", "", 47844.1154, 7, 8718.7269, 1),
    ("C4", "OK", "", 47610.6667, 10, 8559.058, 1),
    ("C5", "OK", "", 30000.0, 6, 6897.0563, 1),
    ("H1", "OK", "", 47700.2677, 8, 8993.207, 1),
    ("H2", "OK", "", 47434.5133, 7, 9120.4425, 1),
    ("H3", "OK", "", 47033.9603, 9, 9214.6902, 1),
    ("H4", "OK", "", 47321.416, 8, 9099.0286, 1),
    ("H5", "OK", "", 47754.549, 8, 8991.7674, 1),
    ("H6", "OK", "", 47700.2677, 8, 8993.207, 1),
    ("H7", "ERROR", "No target parts selected for hole.", 48000.0, 6, 8800.0, 1),
    ("H8", "OK", "", 47524.8341, 7, 9098.0586, 1),
    ("S1", "OK", "", 11712.0, 11, 12112.0, 1),
    ("S2", "OK", "", 13952.0, 11, 14384.0, 1),
    ("S3", "OK", "", 15744.0, 12, 15776.0, 1),
    ("S4", "ERROR", "Could not shell part with selections.", 48000.0, 6, 8800.0, 1),
    ("D1", "OK", "", 51581.192, 6, 9317.9974, 1),
    ("D2", "OK", "", 50115.9238, 6, 9100.6351, 1),
    ("D3", "OK", "", 44582.099, 6, 8337.055, 1),
];

#[test]
fn p1_features_match_live() {
    let mut report = String::new();
    for (name, s) in extrude_cases().into_iter().chain(revolve_cases()).chain(blend_cases()).chain(hole_cases()) {
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
