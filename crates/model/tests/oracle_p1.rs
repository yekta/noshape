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
        // Tapped M6 (1.00 mm coarse, Normal fit: the table's defaults), the
        // standard's depths as live built it: 12 mm deep, tapped 12 mm.
        hole("H5", false, &|p| {
            p.set("isoHoleTableEx", ParamValue::Lookup(["Tapped", "Straight tap", "M6"].map(String::from).to_vec()));
            noshape_model::features::hole_tables::apply_table_to_params(p);
            p.set("endStyleV2", en("BLIND"));
            p.set("holeDepthV3", mm(12.0));
            p.set("tappedDepthV3", mm(12.0));
            p.set("tapClearance", ParamValue::Expr(ExprValue::new("0")));
        }),
        hole("H6", false, &blind),
        hole("H7", true, &blind),
        // Clearance M5, Normal fit (set as live did): Ø 5.5 mm through.
        hole("H8", false, &|p| {
            p.set("isoHoleTableEx", ParamValue::Lookup(["Clearance", "M5", "Normal"].map(String::from).to_vec()));
            noshape_model::features::hole_tables::apply_table_to_params(p);
            p.set("endStyleV2", en("THROUGH"));
        }),
    ]
}

impl Studio {
    /// A sketch on `plane` whose entity ids `draw` returns; its feature and regions.
    fn sketch_with<T>(&mut self, plane: Reference, draw: impl FnOnce(&mut noshape_sketch::Sketch) -> T) -> (FeatureId, T, Vec<Reference>) {
        let mut sk = noshape_sketch::Sketch::new();
        let out = draw(&mut sk);
        let sketch = self.add("sketch", |pv| {
            pv.set("sketchPlane", ParamValue::Query(vec![plane]));
            pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
        });
        let r = self.regen();
        let sk = r.display.sketches.iter().find(|s| s.feature == sketch).expect("sketch");
        let mut regions: Vec<_> = sk.regions.iter().collect();
        regions.sort_by(|a, b| b.area.total_cmp(&a.area));
        (sketch, out, regions.into_iter().map(|g| Reference::SketchRegion { sketch, key: g.key.clone() }).collect())
    }

    /// A plane offset `d` from `from`.
    fn offset_plane(&mut self, from: Reference, d: f64) -> Reference {
        let plane = self.add("cPlane", |p| {
            p.set("entities", q(from));
            p.set("cplaneType", en("OFFSET"));
            p.set("offset", mm(d));
        });
        Reference::FeaturePlane(plane)
    }
}

fn pt(x: f64, y: f64) -> DVec2 {
    DVec2::new(x * 1e-3, y * 1e-3)
}

fn body(r: &RegenResult, i: usize) -> Reference {
    Reference::Body(r.context.bodies[i].id.clone())
}

/// Batch 7: Sweep and Loft. PATH on Front: line (0,0)–(0,40) then the
/// tangent arc to (20,60) about (20,40); PROF: circle Ø6 at the origin on Top.
fn sweep_loft_cases() -> Vec<(&'static str, Studio)> {
    let mut cases = Vec::new();
    let sweep = |set: &dyn Fn(&mut ParamValues, Reference)| {
        let mut s = Studio::new();
        let (path_sketch, (line, arc), _) = s.sketch_with(Reference::Plane(DatumPlane::Front), |sk| {
            let (a, b) = (sk.add_point(pt(0.0, 0.0)), sk.add_point(pt(0.0, 40.0)));
            let line = sk.add_line_between(a, b).unwrap();
            (line, sk.add_arc(pt(20.0, 40.0), pt(20.0, 60.0), pt(0.0, 40.0)))
        });
        let prof = s.sketch(top(), |sk| circle(sk, (0.0, 0.0), 6.0));
        s.add("sweep", |p| {
            p.set("profiles", q(prof[0].clone()));
            p.set("path", ParamValue::Query(vec![Reference::SketchEntity { sketch: path_sketch, entity: line.0 }, Reference::SketchEntity { sketch: path_sketch, entity: arc.0 }]));
            set(p, prof[0].clone());
        });
        s
    };
    cases.push(("SW1", sweep(&|_, _| {})));
    cases.push(("SW2", sweep(&|p, _| p.set("profileControl", en("KEEP_ORIENTATION")))));
    cases.push(("SW3", sweep(&|p, region| {
        p.set("bodyType", en("THIN"));
        p.set("wallShape", q(region));
        p.set("thickness1", mm(1.0));
    })));
    let mut s = Studio::block();
    let r = s.regen();
    let prof = s.sketch(face_at(&r, DVec3::new(30.0, 0.0, 10.0)), |sk| circle(sk, (30.0, 10.0), 6.0));
    let (path_sketch, line, _) = s.sketch_with(top(), |sk| {
        let (a, b) = (sk.add_point(pt(30.0, 0.0)), sk.add_point(pt(30.0, 40.0)));
        sk.add_line_between(a, b).unwrap()
    });
    s.add("sweep", |p| {
        p.set("profiles", q(prof[0].clone()));
        p.set("path", q(Reference::SketchEntity { sketch: path_sketch, entity: line.0 }));
        p.set("operationType", en("REMOVE"));
        p.set("booleanScope", q(body(&r, 0)));
    });
    cases.push(("SW4", s));
    let loft = |levels: &[(f64, bool, f64)], set: &dyn Fn(&mut ParamValues)| {
        let mut s = Studio::new();
        let mut profiles = Vec::new();
        for &(z, square, size) in levels {
            let plane = if z == 0.0 { top() } else { s.offset_plane(top(), z) };
            let region = s.sketch(plane, |sk| if square { rect(sk, (-size / 2.0, -size / 2.0), (size / 2.0, size / 2.0)) } else { circle(sk, (0.0, 0.0), size) });
            profiles.push(region[0].clone());
        }
        s.add("loft", |p| {
            p.set("sheetProfilesArray", ParamValue::Array(profiles.iter().map(|r| {
                let mut v = ParamValues::default();
                v.set("sheetProfileEntities", q(r.clone()));
                v
            }).collect()));
            set(p);
        });
        s
    };
    cases.push(("L1", loft(&[(0.0, true, 20.0), (20.0, false, 16.0)], &|_| {})));
    cases.push(("L2", loft(&[(0.0, true, 20.0), (20.0, false, 16.0), (40.0, true, 10.0)], &|_| {})));
    cases.push(("L3", loft(&[(0.0, true, 20.0), (20.0, false, 16.0)], &|p| {
        p.set("startCondition", en("NORMAL_TO_PROFILE"));
        p.set("startMagnitude", ParamValue::Expr(ExprValue::new("1")));
        p.set("endCondition", en("NORMAL_TO_PROFILE"));
        p.set("endMagnitude", ParamValue::Expr(ExprValue::new("1")));
    })));
    cases
}

/// BLOCK with BOSS (circle Ø6 at (10,10) on its top face, Add, Blind 10);
/// the boss feature and the model after it.
fn block_with_boss() -> (Studio, FeatureId, RegenResult) {
    let mut s = Studio::block();
    let r = s.regen();
    let c = s.sketch(face_at(&r, TOP), |sk| circle(sk, (10.0, 10.0), 6.0));
    let boss = s.add("extrude", |p| {
        p.set("entities", q(c[0].clone()));
        p.set("operationType", en("ADD"));
        p.set("booleanScope", q(body(&r, 0)));
        p.set("depth", mm(10.0));
    });
    let r = s.regen();
    (s, boss, r)
}

/// Batch 8: patterns and Mirror.
fn pattern_cases() -> Vec<(&'static str, Studio)> {
    let mut cases = Vec::new();
    let of_boss = |ty: &str, set: &dyn Fn(&mut ParamValues, &RegenResult)| {
        let (mut s, boss, r) = block_with_boss();
        s.add(ty, |p| {
            p.set("patternType", en("FEATURE"));
            p.set("instanceFunction", q(Reference::Feature(boss)));
            set(p, &r);
        });
        s
    };
    let linear = |p: &mut ParamValues, r: &RegenResult| {
        p.set("directionOne", q(edge_at(r, FRONT_TOP)));
        p.set("distance", mm(15.0));
        p.set("instanceCount", ParamValue::Expr(ExprValue::new("3")));
    };
    cases.push(("LP1", of_boss("linearPattern", &linear)));
    let two = |p: &mut ParamValues, r: &RegenResult| {
        linear(p, r);
        p.set("hasSecondDir", ParamValue::Bool(true));
        p.set("directionTwo", q(edge_at(r, LEFT_TOP)));
        p.set("distanceTwo", mm(10.0));
        p.set("instanceCountTwo", ParamValue::Expr(ExprValue::new("2")));
    };
    cases.push(("LP2", of_boss("linearPattern", &two)));
    cases.push(("LP2b", of_boss("linearPattern", &|p, r| {
        two(p, r);
        p.set("oppositeDirectionTwo", ParamValue::Bool(true));
    })));
    cases.push(("LP3", on_block("linearPattern", |p, r| {
        p.set("patternType", en("PART"));
        p.set("entities", q(body(r, 0)));
        p.set("directionOne", q(edge_at(r, FRONT_TOP)));
        p.set("distance", mm(80.0));
        p.set("instanceCount", ParamValue::Expr(ExprValue::new("2")));
    })));
    let circular = |p: &mut ParamValues, r: &RegenResult| {
        p.set("axis", q(edge_at(r, DVec3::new(60.0, 40.0, 10.0))));
        p.set("instanceCount", ParamValue::Expr(ExprValue::new("4")));
        p.set("angle", deg(360.0));
        p.set("equalSpace", ParamValue::Bool(true));
    };
    cases.push(("CP1", of_boss("circularPattern", &circular)));
    cases.push(("CP2", of_boss("circularPattern", &|p, r| {
        circular(p, r);
        p.set("instanceCount", ParamValue::Expr(ExprValue::new("3")));
        p.set("angle", deg(30.0));
        p.set("equalSpace", ParamValue::Bool(false));
    })));
    cases.push(("CV1", of_boss("curvePattern", &|p, r| {
        p.set("edges", q(edge_at(r, FRONT_TOP)));
        p.set("instanceCount", ParamValue::Expr(ExprValue::new("4")));
    })));
    cases.push(("LP4", of_boss("linearPattern", &|p, r| {
        linear(p, r);
        p.set("skipInstances", ParamValue::Bool(true));
        let mut v = ParamValues::default();
        v.set("index1", ParamValue::Expr(ExprValue::new("2")));
        v.set("index2", ParamValue::Expr(ExprValue::new("0")));
        p.set("skippedInstances", ParamValue::Array(vec![v]));
    })));
    let (mut s, boss, _) = block_with_boss();
    let plane = s.offset_plane(Reference::Plane(DatumPlane::Right), 30.0);
    s.add("mirror", |p| {
        p.set("patternType", en("FEATURE"));
        p.set("instanceFunction", q(Reference::Feature(boss)));
        p.set("mirrorPlane", q(plane));
    });
    cases.push(("M1", s));
    cases.push(("M2", on_block("mirror", |p, r| {
        p.set("patternType", en("PART"));
        p.set("entities", q(body(r, 0)));
        p.set("mirrorPlane", q(Reference::Plane(DatumPlane::Right)));
        p.set("operationType", en("NEW"));
    })));
    cases.push(("M3", on_block("mirror", |p, r| {
        p.set("patternType", en("PART"));
        p.set("entities", q(body(r, 0)));
        p.set("mirrorPlane", q(face_at(r, DVec3::new(60.0, 20.0, 10.0))));
        p.set("operationType", en("ADD"));
        p.set("booleanScope", q(body(r, 0)));
    })));
    cases
}

/// Batch 9: Boolean, Split and Transform. TWO: BLOCK and a 40×40×30 part
/// (rectangle (40,20)–(80,60) on Top, New 30).
fn boolean_split_transform_cases() -> Vec<(&'static str, Studio)> {
    let mut cases = Vec::new();
    let two = |set: &dyn Fn(&mut ParamValues, &RegenResult)| {
        let mut s = Studio::block();
        let c = s.sketch(top(), |sk| rect(sk, (40.0, 20.0), (80.0, 60.0)));
        s.add("extrude", |p| {
            p.set("entities", q(c[0].clone()));
            p.set("operationType", en("NEW"));
            p.set("depth", mm(30.0));
        });
        let r = s.regen();
        s.add("booleanBodies", |p| set(p, &r));
        s
    };
    cases.push(("B1", two(&|p, r| {
        p.set("operationType", en("UNION"));
        p.set("tools", ParamValue::Query(vec![body(r, 0), body(r, 1)]));
    })));
    cases.push(("B2", two(&|p, r| {
        p.set("operationType", en("SUBTRACTION"));
        p.set("targets", q(body(r, 0)));
        p.set("tools", q(body(r, 1)));
    })));
    cases.push(("B3", two(&|p, r| {
        p.set("operationType", en("SUBTRACTION"));
        p.set("targets", q(body(r, 0)));
        p.set("tools", q(body(r, 1)));
        p.set("keepTools", ParamValue::Bool(true));
    })));
    cases.push(("B4", two(&|p, r| {
        p.set("operationType", en("INTERSECTION"));
        p.set("tools", ParamValue::Query(vec![body(r, 0), body(r, 1)]));
    })));
    let mut s = Studio::block();
    let c = s.sketch(top(), |sk| circle(sk, (100.0, 20.0), 10.0));
    s.add("extrude", |p| {
        p.set("entities", q(c[0].clone()));
        p.set("operationType", en("NEW"));
        p.set("depth", mm(10.0));
    });
    let r = s.regen();
    s.add("booleanBodies", |p| {
        p.set("operationType", en("SUBTRACTION"));
        p.set("targets", q(body(&r, 0)));
        p.set("tools", q(body(&r, 1)));
    });
    cases.push(("B5", s));
    let split = |set: &dyn Fn(&mut ParamValues, &RegenResult, Reference)| {
        let mut s = Studio::block();
        let plane = s.offset_plane(Reference::Plane(DatumPlane::Right), 30.0);
        let r = s.regen();
        s.add("splitPart", |p| set(p, &r, plane.clone()));
        s
    };
    cases.push(("SP1", split(&|p, r, plane| {
        p.set("targets", q(body(r, 0)));
        p.set("tool", q(plane));
    })));
    cases.push(("SP2", split(&|p, r, plane| {
        p.set("targets", q(body(r, 0)));
        p.set("tool", q(plane));
        p.set("keepBothSides", ParamValue::Bool(false));
        p.set("keepFront", ParamValue::Bool(true));
    })));
    let mut s = Studio::block();
    let r = s.regen();
    let (line_sketch, line, _) = s.sketch_with(top(), |sk| {
        let (a, b) = (sk.add_point(pt(30.0, 0.0)), sk.add_point(pt(30.0, 40.0)));
        sk.add_line_between(a, b).unwrap()
    });
    s.add("splitPart", |p| {
        p.set("splitType", en("FACE"));
        p.set("faceTargets", q(face_at(&r, TOP)));
        p.set("faceTools", q(Reference::SketchEntity { sketch: line_sketch, entity: line.0 }));
    });
    cases.push(("SP3", s));
    cases.push(("SP4", on_block("splitPart", |p, r| {
        p.set("targets", q(body(r, 0)));
        p.set("tool", q(top()));
    })));
    let transform = |set: &dyn Fn(&mut ParamValues, &RegenResult)| on_block("transform", |p, r| {
        p.set("entities", q(body(r, 0)));
        set(p, r);
    });
    cases.push(("T1", transform(&|p, r| {
        p.set("transformType", en("TRANSLATION_DISTANCE"));
        p.set("transformDirection", q(edge_at(r, FRONT_TOP)));
        p.set("distance", mm(25.0));
    })));
    cases.push(("T2", transform(&|p, _| {
        p.set("transformType", en("TRANSLATION_3D"));
        p.set("dx", mm(10.0));
        p.set("dy", mm(20.0));
        p.set("dz", mm(30.0));
        p.set("makeCopy", ParamValue::Bool(true));
    })));
    cases.push(("T3", transform(&|p, r| {
        p.set("transformType", en("ROTATION"));
        p.set("transformAxis", q(edge_at(r, FRONT_LEFT)));
        p.set("angle", deg(90.0));
    })));
    cases.push(("T4", transform(&|p, _| {
        p.set("transformType", en("SCALE_UNIFORMLY"));
        p.set("scale", ParamValue::Expr(ExprValue::new("2")));
        p.set("scalePoint", q(Reference::Origin));
    })));
    cases.push(("T5", transform(&|p, _| p.set("transformType", en("TRANSLATION_DISTANCE")))));
    cases
}

/// Batch 10: Plane (CASES.md); the plane itself is compared too (PLANES).
fn plane_cases() -> Vec<(&'static str, Studio)> {
    let mut cases = Vec::new();
    let plane = |ty: &'static str, set: &dyn Fn(&mut ParamValues, &RegenResult)| {
        on_block("cPlane", |p, r| {
            p.set("cplaneType", en(ty));
            set(p, r);
        })
    };
    cases.push(("P1", plane("OFFSET", &|p, _| {
        p.set("entities", q(top()));
        p.set("offset", mm(10.0));
    })));
    cases.push(("P2", plane("MID_PLANE", &|p, r| p.set("entities", ParamValue::Query(vec![face_at(r, SIDES[2]), face_at(r, SIDES[3])])))));
    cases.push(("P3", plane("THREE_POINT", &|p, r| {
        let at = [DVec3::new(0.0, 0.0, 20.0), DVec3::new(60.0, 0.0, 20.0), DVec3::new(0.0, 40.0, 0.0)];
        p.set("entities", ParamValue::Query(at.iter().map(|x| vertex_at(r, *x)).collect()));
    })));
    cases.push(("P4", plane("LINE_ANGLE", &|p, r| {
        p.set("entities", q(edge_at(r, FRONT_TOP)));
        p.set("angle", deg(30.0));
    })));
    cases.push(("P5", plane("LINE_POINT", &|p, r| p.set("entities", ParamValue::Query(vec![vertex_at(r, DVec3::new(60.0, 40.0, 20.0)), edge_at(r, FRONT_TOP)])))));
    // A cylinder Ø20 at (100,0) and a sketch point at (110,0) on Top.
    let mut s = Studio::block();
    let c = s.sketch(top(), |sk| circle(sk, (100.0, 0.0), 20.0));
    s.add("extrude", |p| {
        p.set("entities", q(c[0].clone()));
        p.set("operationType", en("NEW"));
        p.set("depth", mm(20.0));
    });
    let mut sk = noshape_sketch::Sketch::new();
    let point = sk.add_point(pt(110.0, 0.0));
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", q(top()));
        pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    });
    let r = s.regen();
    s.add("cPlane", |p| {
        p.set("cplaneType", en("TANGENT_PLANE"));
        p.set("entities", ParamValue::Query(vec![face_at(&r, DVec3::new(90.0, 0.0, 10.0)), Reference::SketchEntity { sketch, entity: point.0 }]));
    });
    cases.push(("P6", s));
    cases.push(("P7", plane("OFFSET", &|_, _| {})));
    cases
}

/// Live's plane of a Plane case (mm): origin and normal (evPlane).
const PLANES: &[(&str, [f64; 3], [f64; 3])] = &[
    ("P1", [0.0, 0.0, 10.0], [0.0, 0.0, 1.0]),
    ("P2", [30.0, 20.0, 10.0], [-1.0, 0.0, 0.0]),
    ("P3", [20.0, 13.333333333333334, 13.333333333333334], [0.0, -0.447213595499958, -0.894427190999916]),
    ("P4", [30.0, 0.0, 20.0], [0.0, 0.8660254037844387, 0.49999999999999994]),
    ("P5", [60.0, 40.0, 20.0], [1.0, 0.0, 0.0]),
    ("P6", [110.0, 0.0, 20.0], [1.0, 0.0, 0.0]),
];

/// The plane the case's (last) feature made: origin (mm) and normal.
fn feature_plane(s: &Studio, r: &RegenResult) -> Option<(DVec3, DVec3)> {
    let id = s.ps.features.last()?.id;
    r.context.planes.iter().find(|p| p.reference == Reference::FeaturePlane(id)).map(|p| (p.frame.origin * 1e3, p.frame.normal))
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
    ("LP2", "fs/psb: feature pattern, second direction straddling the front face (left top edge, −y): live re-extrudes those 3 bosses down the front face to z 0 (50544.69 mm³); ours patterns the geometry (49696.46)"),
    ("CP2", "kern: circular feature pattern of BOSS about the vertical edge (60,40), 30° × 3: live OK 48802.32 mm³; the pattern now turns as live (the extrude's side edges run back toward the sketch, 10-26), ours 48565.49: the copy at 30° overhangs the block's corner and live grows its overhang down to the bottom as LP2 (+11.84 mm² × 20)"),
];

/// Live's mass properties bands (bands.jsonl, lead 10-26): Parasolid's
/// [min, max] estimate of each case's volume (mm³) and area (mm²); ours
/// passes inside them.
const BANDS: &[(&str, [f64; 2], [f64; 2])] = &[
    ("E1", [48784.063024, 48786.733303], [9114.122726, 9114.195805]),
    ("E2", [47213.295513, 47215.908161], [9114.123122, 9114.195408]),
    ("E3", [46427.911877, 46430.49547], [9271.202956, 9271.27484]),
    ("E4", [46427.911877, 46430.49547], [9271.202956, 9271.27484]),
    ("E5", [48784.063024, 48786.733303], [9271.202358, 9271.275438]),
    ("E6", [49176.754951, 49179.43954], [9428.281893, 9428.355168]),
    ("E7", [48929.502772, 48932.178355], [9172.803124, 9172.876275]),
    ("E8", [48752.647671, 48755.316803], [9553.945705, 9554.018769]),
    ("E9", [47998.679229, 48001.320771], [8799.963658, 8800.036342]),
    ("E10", [49569.446896, 49572.145758], [9585.361429, 9585.434898]),
    ("E11", [47998.679229, 48001.320771], [8799.963658, 8800.036342]),
    ("E12", [48784.063024, 48786.733303], [9271.202358, 9271.275438]),
    ("E13", [47998.679229, 48001.320771], [8799.963658, 8800.036342]),
    ("E14", [50354.830843, 50357.558137], [9742.440868, 9742.514724]),
    ("E14b", [47998.679229, 48001.320771], [8799.963658, 8800.036342]),
    ("E15", [46427.911877, 46430.49547], [9271.202956, 9271.27484]),
    ("R1", [28273.405782, 28275.261983], [11309.703088, 11309.764018]),
    ("R2", [7068.215154, 7068.951787], [3027.414197, 3027.45258]),
    ("R3", [7068.215154, 7068.951787], [3027.414197, 3027.45258]),
    ("R4", [3534.05971, 3534.52376], [1613.701462, 1613.731926]),
    ("R4b", [3534.05971, 3534.52376], [1613.701462, 1613.731926]),
    ("R5", [10178.290524, 10179.229872], [20357.498723, 20357.542067]),
    ("R6", [47496.033641, 47498.656709], [8841.025715, 8841.098145]),
    ("F1", [47676.782386, 47679.412104], [8660.472545, 8660.545067]),
    ("F2", [46965.023778, 46987.892311], [8355.708905, 8357.793053]),
    ("F3", [47771.433333, 47774.066531], [8638.381809, 8638.454379]),
    ("F4", [47763.91169, 47789.472348], [8687.891118, 8689.062846]),
    ("F5", [47942.168442, 47947.887197], [8747.302828, 8747.728112]),
    ("F6", [47749.611689, 47760.258494], [8679.417282, 8679.971393]),
    ("F7", [40607.677139, 40610.040031], [7938.058011, 7938.126755]),
    ("F8", [47354.885556, 47357.503425], [8520.981433, 8521.053792]),
    ("F9", [47771.433333, 47774.066531], [8638.381809, 8638.454379]),
    ("C1", [47728.684187, 47731.315813], [8685.522167, 8685.594715]),
    ("C2", [47548.687497, 47551.312503], [8654.820885, 8654.893342]),
    ("C3", [47842.797518, 47845.433337], [8718.690593, 8718.763199]),
    ("C4", [47609.353047, 47611.980286], [8559.021764, 8559.094252]),
    ("C5", [29999.034511, 30000.965489], [6897.025203, 6897.087347]),
    ("H1", [47698.952461, 47701.582994], [8993.170741, 8993.243274]),
    ("H2", [47433.202945, 47435.823699], [9120.406252, 9120.47865]),
    ("H3", [47032.657269, 47035.263249], [9214.654133, 9214.726327]),
    ("H4", [47320.107693, 47322.72428], [9098.99245, 9099.064791]),
    ("H5", [47753.232728, 47755.865257], [8991.731118, 8991.803679]),
    ("H6", [47698.952461, 47701.582994], [8993.170741, 8993.243274]),
    ("H7", [47998.679229, 48001.320771], [8799.963658, 8800.036342]),
    ("H8", [47523.522071, 47526.146151], [9098.022381, 9098.094825]),
    ("S1", [11711.484272, 11712.515728], [12111.97729, 12112.02271]),
    ("S2", [13951.42045, 13952.57955], [14383.975926, 14384.024074]),
    ("S3", [15743.371831, 15744.628169], [15775.974937, 15776.025063]),
    ("S4", [47998.679229, 48001.320771], [8799.963658, 8800.036342]),
    ("D1", [51579.806374, 51582.577721], [9317.960175, 9318.034625]),
    ("D2", [50114.564463, 50117.283074], [9100.598235, 9100.671972]),
    ("D3", [44580.841661, 44583.35627], [8337.019494, 8337.090411]),
    ("SW1", [2019.077995, 2019.397508], [1402.694529, 1402.719808]),
    ("SW2", [1677.924052, 1711.714246], [1293.650551, 1321.43999]),
    ("SW3", [1570.383139, 1570.653364], [3185.007176, 3185.030423]),
    ("SW4", [46867.726703, 46870.326587], [9497.397514, 9497.469624]),
    ("L1", [5915.213759, 5940.242286], [1897.211424, 1901.793542]),
    ("L2", [8753.54132, 8782.772731], [2642.874413, 2658.436224]),
    ("L3", [5939.088856, 5954.640587], [1905.907784, 1909.448247]),
    ("LP1", [48846.893731, 48849.566302], [9365.450122, 9365.523233]),
    ("LP2", [50543.323002, 50546.057097], [10221.246061, 10221.320008]),
    ("LP2b", [49695.108323, 49697.811743], [9930.93659, 9931.010121]),
    ("LP3", [95997.903407, 96002.096593], [17599.954211, 17600.045789]),
    ("CP1", [48281.417386, 48284.069291], [8988.459146, 8988.531973]),
    ("CP2", [48800.98579, 48803.656686], [9267.535079, 9267.608167]),
    ("CV1", [48846.893731, 48849.566302], [9365.450122, 9365.523233]),
    ("LP4", [48564.155554, 48566.817802], [9176.954634, 9177.027603]),
    ("M1", [48564.155554, 48566.817802], [9176.954634, 9177.027603]),
    ("M2", [95997.903407, 96002.096593], [17599.954211, 17600.045789]),
    ("M3", [95997.903407, 96002.096593], [15999.954211, 16000.045789]),
    ("B1", [87998.021565, 88001.978435], [14399.95552, 14400.04448]),
    ("B2", [39998.830393, 40001.169607], [7999.9658, 8000.0342]),
    ("B3", [87998.021565, 88001.978435], [15999.95552, 16000.04448]),
    ("B4", [7999.6, 8000.4], [2399.98, 2400.02]),
    ("B5", [47998.679229, 48001.320771], [8799.963658, 8800.036342]),
    ("SP1", [47998.679229, 48001.320771], [10399.963658, 10400.036342]),
    ("SP2", [23999.167966, 24000.832034], [5199.971155, 5200.028845]),
    ("SP3", [47998.679229, 48001.320771], [8799.963658, 8800.036342]),
    ("SP4", [47998.679229, 48001.320771], [8799.963658, 8800.036342]),
    ("T1", [47998.679229, 48001.320771], [8799.963658, 8800.036342]),
    ("T2", [95997.903407, 96002.096593], [17599.954211, 17600.045789]),
    ("T3", [47998.679229, 48001.320771], [8799.963658, 8800.036342]),
    ("T4", [383994.716916, 384005.283084], [35199.927315, 35200.072685]),
    ("T5", [47998.679229, 48001.320771], [8799.963658, 8800.036342]),
    ("P1", [47998.679229, 48001.320771], [8799.963658, 8800.036342]),
    ("P2", [47998.679229, 48001.320771], [8799.963658, 8800.036342]),
    ("P3", [47998.679229, 48001.320771], [8799.963658, 8800.036342]),
    ("P4", [47998.679229, 48001.320771], [8799.963658, 8800.036342]),
    ("P5", [47998.679229, 48001.320771], [8799.963658, 8800.036342]),
    ("P6", [54281.751656, 54284.618959], [10684.917729, 10684.993456]),
    ("P7", [47998.679229, 48001.320771], [8799.963658, 8800.036342]),
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
    ("SW1", "OK", "", 2019.2378, 4, 1402.7072, 1),
    ("SW2", "OK", "", 1694.8191, 4, 1307.5453, 1),
    ("SW3", "OK", "", 1570.5183, 6, 3185.0188, 1),
    ("SW4", "OK", "", 46869.0266, 7, 9497.4336, 1),
    ("L1", "OK", "", 5927.728, 6, 1899.5025, 1),
    ("L2", "OK", "", 8768.157, 6, 2650.6553, 1),
    ("L3", "OK", "", 5946.8647, 6, 1907.678, 1),
    ("LP1", "OK", "", 48848.23, 12, 9365.4867, 1),
    ("LP2", "OK", "", 50544.69, 21, 10221.283, 1),
    ("LP2b", "OK", "", 49696.46, 18, 9930.9734, 1),
    ("LP3", "OK", "", 96000.0, 12, 17600.0, 2),
    ("CP1", "ERROR", "Could not create all instances as entered. Try selecting \"Reapply features\" option.", 48282.7433, 8, 8988.4956, 1),
    ("CP2", "OK", "", 48802.3212, 10, 9267.5716, 1),
    ("CV1", "INFO", "For best results ensure that the path start point and pattern seed are touching.", 48848.23, 12, 9365.4867, 1),
    ("LP4", "OK", "", 48565.4867, 10, 9176.9911, 1),
    ("M1", "OK", "", 48565.4867, 10, 9176.9911, 1),
    ("M2", "OK", "", 96000.0, 12, 17600.0, 2),
    ("M3", "OK", "", 96000.0, 6, 16000.0, 1),
    ("B1", "OK", "", 88000.0, 11, 14400.0, 1),
    ("B2", "OK", "", 40000.0, 8, 8000.0, 1),
    ("B3", "OK", "", 88000.0, 14, 16000.0, 2),
    ("B4", "OK", "", 8000.0, 6, 2400.0, 1),
    ("B5", "INFO", "Selected tools and targets do not intersect.", 48000.0, 6, 8800.0, 1),
    ("SP1", "INFO", "Planes and mate connectors will be kept.", 48000.0, 12, 10400.0, 2),
    ("SP2", "INFO", "Planes and mate connectors will be kept.", 24000.0, 6, 5200.0, 1),
    ("SP3", "OK", "", 48000.0, 7, 8800.0, 1),
    ("SP4", "INFO", "Selected entity does not split selected parts.", 48000.0, 6, 8800.0, 1),
    ("T1", "OK", "", 48000.0, 6, 8800.0, 1),
    ("T2", "OK", "", 96000.0, 12, 17600.0, 2),
    ("T3", "OK", "", 48000.0, 6, 8800.0, 1),
    ("T4", "OK", "", 384000.0, 6, 35200.0, 1),
    ("T5", "ERROR", "Translation by distance requires two vertices, an edge, a plane, or a cylindrical face.", 48000.0, 6, 8800.0, 1),
    ("P1", "OK", "", 48000.0, 6, 8800.0, 1),
    ("P2", "OK", "", 48000.0, 6, 8800.0, 1),
    ("P3", "OK", "", 48000.0, 6, 8800.0, 1),
    ("P4", "INFO", "Select an additional point, plane, or axis to specify where the angle is measured from.", 48000.0, 6, 8800.0, 1),
    ("P5", "OK", "", 48000.0, 6, 8800.0, 1),
    ("P6", "OK", "", 54283.1853, 9, 10684.9556, 2),
    ("P7", "ERROR", "Offset plane requires a plane, circle, ellipse, or arc to offset from.", 48000.0, 6, 8800.0, 1),
];

#[test]
fn p1_features_match_live() {
    let mut report = String::new();
    for (name, s) in extrude_cases().into_iter().chain(revolve_cases()).chain(blend_cases()).chain(hole_cases()).chain(sweep_loft_cases()).chain(pattern_cases()).chain(boolean_split_transform_cases()).chain(plane_cases()) {
        let r = s.regen();
        let got = outcome(&r);
        eprintln!("{name}: {got:?}");
        // Plane cases: the plane too (origin mm, normal).
        let plane = PLANES.iter().find(|p| p.0 == name).map(|(_, o, n)| (DVec3::from(*o), DVec3::from(*n), feature_plane(&s, &r)));
        let plane_ok = plane.is_none_or(|(o, n, ours)| ours.is_some_and(|(po, pn)| po.distance(o) <= 1e-6 && pn.distance(n) <= 1e-9));
        let Some(&(_, status, text, volume, faces, area, parts)) = LIVE.iter().find(|l| l.0 == name) else { continue };
        let near = |a: f64, b: f64| (a - b).abs() <= 1e-3 * b.abs().max(1.0) * 1e-3 + 1e-4;
        let within = |x: f64, [lo, hi]: [f64; 2]| x >= lo - 1e-6 * lo.abs() && x <= hi + 1e-6 * hi.abs();
        let band = BANDS.iter().find(|b| b.0 == name);
        let volume_ok = band.map_or(near(got.volume, volume), |b| within(got.volume, b.1));
        let area_ok = band.map_or(near(got.area, area), |b| within(got.area, b.2));
        if got.status != status || got.text != text || !volume_ok || got.faces != faces || !area_ok || got.parts != parts || !plane_ok {
            if !KNOWN.iter().any(|k| k.0 == name) {
                report += &format!("{name}: ours {got:?}\n      live ({status}, {text:?}, {volume}, {faces}, {area}, {parts})\n");
                if let Some((o, n, ours)) = plane {
                    report += &format!("      our plane {ours:?}, live ({o}, {n})\n");
                }
            }
        } else if let Some(k) = KNOWN.iter().find(|k| k.0 == name) {
            report += &format!("{name} now matches live: drop it from KNOWN ({})\n", k.1);
        }
    }
    assert!(report.is_empty(), "differences from live:\n{report}");
}

/// A Feature pattern whose instances all miss the part fails asking for
/// Reapply features (live "qa5 bracket" †: a Remove patterned toward −x).
#[test]
fn feature_pattern_off_the_part_as_live() {
    let mut s = Studio::block();
    let r = s.regen();
    let c = s.sketch(face_at(&r, TOP), |sk| circle(sk, (10.0, 10.0), 6.0));
    let cut = s.add("extrude", |p| {
        p.set("entities", q(c[0].clone()));
        p.set("operationType", en("REMOVE"));
        p.set("booleanScope", q(body(&r, 0)));
        p.set("endBound", en("THROUGH_ALL"));
        p.set("oppositeDirection", ParamValue::Bool(true));
    });
    let r = s.regen();
    assert_eq!(outcome(&r).faces, 7, "the cut");
    s.add("linearPattern", |p| {
        p.set("patternType", en("FEATURE"));
        p.set("instanceFunction", q(Reference::Feature(cut)));
        p.set("directionOne", q(edge_at(&r, FRONT_TOP)));
        p.set("distance", mm(25.0));
        p.set("instanceCount", ParamValue::Expr(ExprValue::new("3")));
        p.set("oppositeDirection", ParamValue::Bool(true));
    });
    let got = outcome(&s.regen());
    assert_eq!((got.status, got.text.as_str()), ("ERROR", "Could not create all instances as entered. Try selecting \"Reapply features\" option."));
}
