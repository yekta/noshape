//! Batch B feature evaluators: reference geometry, curves, surfaces, sheet
//! metal and import.

use glam::{DVec2, DVec3};
use noshape_model::configuration::ConfigValues;
use noshape_model::display::{BodyKind, FeatureStatus};
use noshape_model::params::{ParamValue, ParamValues};
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{BodyId, DatumPlane, FeatureId, Reference, Role, TopoName};
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};
use std::f64::consts::PI;

struct Studio {
    ps: PartStudio,
    units: UnitPrefs,
    reg: Regenerator,
}

fn expr(s: &str) -> ParamValue {
    ParamValue::Expr(ExprValue::new(s))
}

impl Studio {
    fn new() -> Self {
        Self { ps: PartStudio::new(), units: UnitPrefs::default(), reg: Regenerator::new() }
    }

    fn regen(&mut self) -> RegenResult {
        self.reg.regenerate(&self.ps, &ConfigValues::default(), &self.units, &RegenOptions::default())
    }

    /// Add a feature; returns its id.
    fn add(&mut self, ty: &str, set: impl FnOnce(&mut ParamValues)) -> FeatureId {
        let mut f = self.ps.new_feature(ty, &self.units).unwrap_or_else(|| panic!("unknown feature {ty}"));
        set(&mut f.params);
        let id = f.id;
        self.ps.features.push(f);
        id
    }

    fn sketch(&mut self, plane: Reference, sk: noshape_sketch::Sketch) -> FeatureId {
        self.add("sketch", |p| {
            p.set("sketchPlane", ParamValue::Query(vec![plane]));
            p.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
        })
    }

    /// Regenerate and assert the last feature succeeded.
    fn ok(&mut self) -> RegenResult {
        let r = self.regen();
        for (f, s) in self.ps.features.iter().zip(&r.display.features) {
            // Info: a note that is not a failure (std Finish sheet metal model).
            assert!(matches!(s.status, FeatureStatus::Ok | FeatureStatus::Info), "{} ({}) failed: {:?}", f.name, f.type_id, s.message);
        }
        r
    }

    /// Extrude the largest region of a sketch.
    fn extrude_largest(&mut self, sketch: FeatureId, depth: &str) -> FeatureId {
        let r = self.regen();
        let sk = r.display.sketches.iter().find(|s| s.feature == sketch).expect("sketch");
        let key = sk.regions.iter().max_by(|a, b| a.area.total_cmp(&b.area)).expect("region").key.clone();
        let depth = depth.to_string();
        self.add("extrude", move |p| {
            p.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch, key }]));
            p.set("depth", expr(&depth));
        })
    }
}

fn rect(sk: &mut noshape_sketch::Sketch, x0: f64, y0: f64, w: f64, h: f64) -> Vec<u32> {
    let p = [DVec2::new(x0, y0), DVec2::new(x0 + w, y0), DVec2::new(x0 + w, y0 + h), DVec2::new(x0, y0 + h)];
    let pts: Vec<_> = p.iter().map(|q| sk.add_point(*q)).collect();
    (0..4).map(|i| sk.add_line_between(pts[i], pts[(i + 1) % 4]).unwrap().0).collect()
}

/// 100 x 50 x 25 mm block on the Top plane at the origin.
fn block() -> Studio {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    rect(&mut sk, 0.0, 0.0, 0.1, 0.05);
    let id = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    s.extrude_largest(id, "25 mm");
    s
}

fn top_face(r: &RegenResult) -> Reference {
    let b = &r.context.bodies[0];
    let n = b.faces.iter().find(|n| matches!(&***n, TopoName::Face { role: Role::CapEnd(_), .. })).expect("top face");
    Reference::Face((**n).clone())
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

fn vnear(a: DVec3, b: DVec3) -> bool {
    (a - b).length() < 1e-7
}

fn wire_length(r: &RegenResult, idx: usize) -> f64 {
    r.context.bodies[idx].shape.edges().iter().map(|e| e.length().unwrap()).sum()
}

fn surface_area(r: &RegenResult, idx: usize) -> f64 {
    r.context.bodies[idx].shape.faces().iter().map(|f| f.area().unwrap()).sum()
}

#[test]
fn mate_connector_on_face_with_offset_drives_plane_and_sketch() {
    let mut s = block();
    let r = s.ok();
    let top = top_face(&r);
    let mc = s.add("mateConnector", |p| {
        p.set("originQuery", ParamValue::Query(vec![top.clone()]));
        p.set("entityInferenceType", ParamValue::Enum("CENTROID".into()));
        p.set("transform", ParamValue::Bool(true));
        p.set("translationZ", expr("10 mm"));
        p.set("translationX", expr("5 mm"));
    });
    let r = s.ok();
    let c = &r.context.connectors[0];
    assert!(vnear(c.origin, DVec3::new(0.055, 0.025, 0.035)), "origin {:?}", c.origin);
    assert!(vnear(c.z, DVec3::Z));
    assert_eq!(r.display.connectors.len(), 1);

    // Plane offset from the connector, and a sketch + extrude on the connector.
    let mc_ref = Reference::MateConnector { feature: mc, index: 0 };
    let mcr = mc_ref.clone();
    s.add("cPlane", move |p| {
        p.set("entities", ParamValue::Query(vec![mcr]));
        p.set("offset", expr("5 mm"));
    });
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(DVec2::ZERO, 0.005);
    let sid = s.sketch(mc_ref, sk);
    s.extrude_largest(sid, "10 mm");
    let r = s.ok();
    let plane = r.context.planes.iter().find(|p| matches!(p.reference, Reference::FeaturePlane(_))).unwrap();
    assert!(near(plane.frame.origin.z, 0.040, 1e-9), "plane z {}", plane.frame.origin.z);
    let cyl = r.context.bodies.iter().find(|b| b.id.feature != r.context.bodies[0].id.feature).expect("cylinder body");
    let bb = cyl.shape.bounding_box(true).unwrap();
    assert!(near(bb.min.z, 0.035, 1e-6) && near(bb.max.z, 0.045, 1e-6), "{bb:?}");
    assert!(near(bb.center().x, 0.055, 1e-6));
}

#[test]
fn mate_connector_on_edge_origin_and_flips() {
    let mut s = block();
    let r = s.ok();
    // A bottom edge along X: between the start cap and a side face, at y = 0.
    let b = &r.context.bodies[0];
    let e = b
        .shape
        .edges()
        .iter()
        .position(|e| {
            let (a, c) = e.endpoints().unwrap();
            a.z.abs() < 1e-9 && c.z.abs() < 1e-9 && a.y.abs() < 1e-9 && c.y.abs() < 1e-9
        })
        .unwrap();
    let edge = Reference::Edge((*b.edges[e]).clone());
    s.add("mateConnector", |p| {
        p.set("originQuery", ParamValue::Query(vec![edge]));
        p.set("entityInferenceType", ParamValue::Enum("MID_POINT".into()));
    });
    s.add("mateConnector", |p| {
        p.set("originQuery", ParamValue::Query(vec![Reference::Origin]));
        p.set("entityInferenceType", ParamValue::Enum("ORIGIN_X".into()));
        p.set("flipPrimary", ParamValue::Bool(true));
        p.set("transform", ParamValue::Bool(true));
        p.set("rotationType", ParamValue::Enum("ABOUT_Z".into()));
        p.set("rotation", expr("90 deg"));
        p.set("translationZ", expr("1 mm"));
    });
    let r = s.ok();
    let c0 = &r.context.connectors[0];
    assert!(vnear(c0.origin, DVec3::new(0.05, 0.0, 0.0)), "{:?}", c0.origin);
    assert!(near(c0.z.x.abs(), 1.0, 1e-9), "z along the edge {:?}", c0.z);
    let c1 = &r.context.connectors[1];
    // Origin X: Z = +X, flipped first, then moved 1 mm along the flipped Z
    // (live L38: the move goes in the flipped / reoriented frame).
    assert!(vnear(c1.origin, DVec3::new(-0.001, 0.0, 0.0)), "{:?}", c1.origin);
    assert!(vnear(c1.z, -DVec3::X), "{:?}", c1.z);
    assert!(c1.x.dot(c1.z).abs() < 1e-12);
}

#[test]
fn helix_about_axis_length_and_turns() {
    let mut s = Studio::new();
    s.add("helix", |p| {
        p.set("axisType", ParamValue::Enum("AXIS".into()));
        p.set("axis", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        p.set("startRadius", expr("10 mm"));
        p.set("height", expr("50 mm"));
        p.set("revolutions", expr("5"));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 1);
    assert_eq!(r.context.bodies[0].kind, BodyKind::Wire);
    let pitch = 0.01;
    let expected = 5.0 * ((2.0 * PI * 0.01f64).powi(2) + pitch * pitch).sqrt();
    let len = wire_length(&r, 0);
    assert!(near(len, expected, expected * 1e-4), "length {len} vs {expected}");
    let bb = r.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!(near(bb.min.z, 0.0, 1e-6) && near(bb.max.z, 0.05, 1e-6), "{bb:?}");
    // Wire bodies render as edge polylines.
    let mesh = &r.display.bodies[0].mesh;
    assert!(!mesh.edges.is_empty() && mesh.edges.iter().any(|e| e.len() > 10), "helix has no edge polyline");
}

#[test]
fn helix_on_cylinder_face_uses_face_height() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(DVec2::ZERO, 0.02);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    s.extrude_largest(sid, "40 mm");
    let r = s.ok();
    let b = &r.context.bodies[0];
    let side = b.faces.iter().find(|n| matches!(&***n, TopoName::Face { role: Role::Side(_), .. })).unwrap();
    let side = Reference::Face((**side).clone());
    let sd = side.clone();
    s.add("helix", move |p| {
        p.set("entities", ParamValue::Query(vec![sd]));
        p.set("pathType", ParamValue::Enum("PITCH".into()));
        p.set("helicalPitch", expr("10 mm"));
        p.set("handedness", ParamValue::Enum("CCW".into()));
    });
    // Tangent plane to the cylinder parallel to the Right plane.
    s.add("cPlane", move |p| {
        p.set("cplaneType", ParamValue::Enum("TANGENT_PLANE".into()));
        p.set("entities", ParamValue::Query(vec![side, Reference::Plane(DatumPlane::Right)]));
    });
    let r = s.ok();
    let h = r.context.bodies.iter().position(|b| b.kind == BodyKind::Wire).unwrap();
    let expected = 4.0 * ((2.0 * PI * 0.02f64).powi(2) + 0.01f64.powi(2)).sqrt();
    let len = wire_length(&r, h);
    assert!(near(len, expected, expected * 1e-4), "length {len} vs {expected}");
    let plane = r.context.planes.iter().find(|p| matches!(p.reference, Reference::FeaturePlane(_))).unwrap();
    assert!(near(plane.frame.origin.x.abs(), 0.02, 1e-9), "{:?}", plane.frame);
    assert!(near(plane.frame.normal.x.abs(), 1.0, 1e-9));
}

#[test]
fn offset_surface_area_and_position() {
    let mut s = block();
    let r = s.ok();
    let top = top_face(&r);
    let t2 = top.clone();
    s.add("offsetSurface", move |p| {
        p.set("surfacesAndFaces", ParamValue::Query(vec![t2]));
        p.set("offset", expr("5 mm"));
    });
    s.add("offsetSurface", move |p| {
        p.set("surfacesAndFaces", ParamValue::Query(vec![top]));
        p.set("offset", expr("0 mm"));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 3);
    for (i, z) in [(1, 0.030), (2, 0.025)] {
        let b = &r.context.bodies[i];
        assert_eq!(b.kind, BodyKind::Surface);
        assert!(near(surface_area(&r, i), 0.1 * 0.05, 1e-9), "area {}", surface_area(&r, i));
        let bb = b.shape.bounding_box(true).unwrap();
        assert!(near(bb.center().z, z, 1e-7), "z {}", bb.center().z);
    }
}

#[test]
fn fill_closed_sketch_loop() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let lines = rect(&mut sk, 0.0, 0.0, 0.04, 0.03);
    let sid = s.sketch(Reference::Plane(DatumPlane::Front), sk);
    s.add("fill", move |p| {
        let mut row = ParamValues::default();
        row.set("entities", ParamValue::Query(lines.iter().map(|&e| Reference::SketchEntity { sketch: sid, entity: e }).collect()));
        row.set("continuity", ParamValue::Enum("G0".into()));
        p.set("edges", ParamValue::Array(vec![row]));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 1);
    assert_eq!(r.context.bodies[0].kind, BodyKind::Surface);
    let a = surface_area(&r, 0);
    assert!(near(a, 0.04 * 0.03, 1e-8), "fill area {a}");
}

/// Live "psb (10-24 d) fill guides": Precise passes through the guide
/// curves, which have to reach the boundary at an end; Guides on with
/// nothing picked is the plain (flat) fill.
#[test]
fn fill_guides_as_fs() {
    // (status, message, distance from the arc's top to the fill) for an arc
    // in the Front plane from x = -a to a bulging to z = 10 mm.
    let run = |a: Option<f64>, mode: &str| {
        let mut s = Studio::new();
        let mut sk = noshape_sketch::Sketch::new();
        let lines = rect(&mut sk, -0.02, -0.02, 0.04, 0.04);
        let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
        let guide = a.map(|a: f64| {
            // Front: sketch x = X, y = Z; centre below so the arc tops at 10 mm.
            let c = (0.01 * 0.01 - a * a) / 0.02;
            let mut g = noshape_sketch::Sketch::new();
            let arc = g.add_arc(DVec2::new(0.0, c), DVec2::new(a, 0.0), DVec2::new(-a, 0.0));
            Reference::SketchEntity { sketch: s.sketch(Reference::Plane(DatumPlane::Front), g), entity: arc.0 }
        });
        let mode = mode.to_string();
        s.add("fill", move |p| {
            let mut row = ParamValues::default();
            row.set("entities", ParamValue::Query(lines.iter().map(|&e| Reference::SketchEntity { sketch: sid, entity: e }).collect()));
            row.set("continuity", ParamValue::Enum("G0".into()));
            p.set("edges", ParamValue::Array(vec![row]));
            p.set("addGuides", ParamValue::Bool(true));
            p.set("guideEntities", ParamValue::Query(guide.into_iter().collect()));
            p.set("constraintMode", ParamValue::Enum(mode));
        });
        let r = s.regen();
        let st = r.display.features.last().unwrap().clone();
        let top = noshape_kernel::make_vertex(DVec3::new(0.0, 0.0, 0.01)).unwrap();
        let d = r.context.bodies.iter().find(|b| b.kind == BodyKind::Surface).map(|b| noshape_kernel::distance(&top, &b.shape).unwrap().distance);
        (st.status, st.message, d, r.context.bodies.iter().position(|b| b.kind == BodyKind::Surface).map(|i| surface_area(&r, i)))
    };
    let (st, msg, _, area) = run(None, "PRECISE");
    assert_eq!(st, FeatureStatus::Ok, "{msg:?}");
    assert!(near(area.unwrap(), 0.04 * 0.04, 1e-9), "{area:?}");
    let (st, msg, d, _) = run(Some(0.02), "PRECISE");
    assert_eq!(st, FeatureStatus::Ok, "{msg:?}");
    assert!(d.unwrap() < 1e-6, "{d:?}");
    let (st, msg, _, _) = run(Some(0.01), "PRECISE");
    assert_eq!((st, msg.as_deref()), (FeatureStatus::Error, Some("Guide entities cannot be used as selected.")));
    // Sampled only passes through the samples.
    let (st, msg, d, _) = run(Some(0.02), "SAMPLED");
    assert_eq!(st, FeatureStatus::Ok, "{msg:?}");
    assert!(d.unwrap() < 1e-4, "{d:?}");
}

#[test]
fn sheet_metal_extrude_l_bracket_volume() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let p0 = sk.add_point(DVec2::new(0.0, 0.0));
    let p1 = sk.add_point(DVec2::new(0.05, 0.0));
    let p2 = sk.add_point(DVec2::new(0.05, 0.03));
    let l0 = sk.add_line_between(p0, p1).unwrap().0;
    let l1 = sk.add_line_between(p1, p2).unwrap().0;
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    s.add("sheetMetalStart", move |p| {
        p.set("process", ParamValue::Enum("EXTRUDE".into()));
        p.set("sketchCurves", ParamValue::Query(vec![Reference::SketchEntity { sketch: sid, entity: l0 }, Reference::SketchEntity { sketch: sid, entity: l1 }]));
        p.set("depth", expr("20 mm"));
        p.set("thickness", expr("2 mm"));
        p.set("radius", expr("3 mm"));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 1);
    let (t, rad, depth): (f64, f64, f64) = (0.002, 0.003, 0.02);
    // Material outside the turn, the default (live "fs (10-23 05:00)": 3211.3274122872 mm³):
    // the path is the inner face, walls lose R.
    let flats = (0.05 - rad) + (0.03 - rad);
    let bend = PI / 4.0 * ((rad + t).powi(2) - rad.powi(2));
    let expected = depth * (flats * t + bend);
    let v = r.context.bodies[0].shape.volume().unwrap();
    assert!(near(v, expected, expected * 1e-6), "volume {v} vs {expected}");
    assert!(near(v * 1e9, 3_211.327_412_287_2, 1e-6), "live volume: {}", v * 1e9);
}

#[test]
fn sheet_metal_thicken_region() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    rect(&mut sk, 0.0, 0.0, 0.06, 0.04);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let r = s.regen();
    let key = r.display.sketches[0].regions[0].key.clone();
    s.add("sheetMetalStart", move |p| {
        p.set("process", ParamValue::Enum("THICKEN".into()));
        p.set("regions", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key }]));
        p.set("thickness", expr("1.5 mm"));
    });
    // Finish needs its parts (live: "Select one or more sheet metal parts.").
    let part = Reference::Body(s.ok().context.bodies[0].id.clone());
    s.add("sheetMetalEnd", move |p| p.set("sheetMetalParts", ParamValue::Query(vec![part])));
    let r = s.ok();
    let v = r.context.bodies[0].shape.volume().unwrap();
    assert!(near(v, 0.06 * 0.04 * 0.0015, 1e-12), "volume {v}");
    // A flange without edges fails with a clear message.
    s.add("sheetMetalFlange", |_| {});
    let r = s.regen();
    let st = r.display.features.last().unwrap();
    assert_eq!(st.status, FeatureStatus::Error);
    assert!(!st.message.as_deref().unwrap_or("").is_empty(), "{:?}", st.message);
}

#[test]
fn curves_from_sketch_and_edges() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let a = sk.add_point(DVec2::new(0.0, 0.0));
    let b = sk.add_point(DVec2::new(0.02, 0.01));
    let c = sk.add_point(DVec2::new(0.04, 0.0));
    let l0 = sk.add_line(DVec2::new(0.0, 0.03), DVec2::new(0.02, 0.03)).0;
    let l1 = sk.add_line(DVec2::new(0.02, 0.03), DVec2::new(0.02, 0.05)).0;
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let se = |e: u32| Reference::SketchEntity { sketch: sid, entity: e };
    s.add("fitSpline", |p| p.set("vertices", ParamValue::Query(vec![se(a.0), se(b.0), se(c.0)])));
    let comp = s.add("compositeCurve", |p| p.set("edges", ParamValue::Query(vec![se(l0), se(l1)])));
    s.add("bridgingCurve", |p| {
        p.set("side1", ParamValue::Query(vec![se(l0)]));
        p.set("side2", ParamValue::Query(vec![se(c.0)]));
        p.set("match2", ParamValue::Enum("POSITION".into()));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 3);
    assert!(r.context.bodies.iter().all(|b| b.kind == BodyKind::Wire));
    assert!(near(wire_length(&r, 1), 0.04, 1e-9));
    // Extend the composite curve by 10 mm at its end nearest a help point.
    s.add("trimCurve", |p| {
        p.set("moveBoundaryType", ParamValue::Enum("EXTEND".into()));
        p.set("wires", ParamValue::Query(vec![Reference::Body(BodyId { feature: comp, index: 0 })]));
        p.set("extensionDistance", expr("10 mm"));
        p.set("helpPoint", ParamValue::Query(vec![se(c.0)]));
    });
    let r = s.ok();
    assert!(near(wire_length(&r, 1), 0.05, 1e-9), "extended length {}", wire_length(&r, 1));
    // The start end was extended (nearest the help point) to x = -10 mm. Trim at x = 10 mm.
    s.add("cPlane", |p| {
        p.set("entities", ParamValue::Query(vec![Reference::Plane(DatumPlane::Right)]));
        p.set("offset", expr("10 mm"));
    });
    let plane = s.ps.features.last().unwrap().id;
    s.add("trimCurve", |p| {
        p.set("moveBoundaryType", ParamValue::Enum("TRIM".into()));
        p.set("wires", ParamValue::Query(vec![Reference::Body(BodyId { feature: comp, index: 0 })]));
        p.set("trimTo", ParamValue::Query(vec![Reference::FeaturePlane(plane)]));
        p.set("helpPoint", ParamValue::Query(vec![se(c.0)]));
    });
    let r = s.ok();
    // Kept the piece nearest the help point: 10 mm of the first line plus the 20 mm second line.
    let len = wire_length(&r, 1);
    assert!(near(len, 0.03, 1e-7), "trimmed length {len}");
}

#[test]
fn intersection_and_ruled_surface_on_block() {
    let mut s = block();
    s.add("cPlane", |p| {
        p.set("entities", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        p.set("offset", expr("10 mm"));
    });
    let plane = s.ps.features.last().unwrap().id;
    let body = Reference::Body(BodyId { feature: s.ps.features[1].id, index: 0 });
    let bd = body.clone();
    s.add("intersectionCurve", move |p| {
        p.set("group1", ParamValue::Query(vec![bd]));
        p.set("group2", ParamValue::Query(vec![Reference::FeaturePlane(plane)]));
    });
    let r = s.ok();
    let w = r.context.bodies.iter().position(|b| b.kind == BodyKind::Wire).unwrap();
    assert!(near(wire_length(&r, w), 0.3, 1e-7), "section perimeter {}", wire_length(&r, w));
    // Ruled surface from a top edge, along +Z by 15 mm.
    let b = &r.context.bodies[0];
    let e = b.shape.edges().iter().position(|e| {
        let (a, c) = e.endpoints().unwrap();
        near(a.z, 0.025, 1e-9) && near(c.z, 0.025, 1e-9) && a.y.abs() < 1e-9 && c.y.abs() < 1e-9
    });
    let edge = Reference::Edge((*b.edges[e.unwrap()]).clone());
    s.add("ruledSurface", move |p| {
        p.set("edges", ParamValue::Query(vec![edge]));
        p.set("ruledType", ParamValue::Enum("ALIGNED_WITH_VECTOR".into()));
        p.set("axis", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        p.set("distance", expr("15 mm"));
    });
    let r = s.ok();
    let sf = r.context.bodies.iter().position(|b| b.kind == BodyKind::Surface).unwrap();
    assert!(near(surface_area(&r, sf), 0.1 * 0.015, 1e-9), "ruled area {}", surface_area(&r, sf));
}

#[test]
fn import_foreign_brep_file_and_missing_data() {
    let dir = std::env::temp_dir().join(format!("noshape-featb-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("box.brep");
    let bx = noshape_kernel::make_box(&noshape_kernel::Plane::xy(), 0.01, 0.02, 0.03).unwrap();
    noshape_kernel::write_brep(&bx, &path).unwrap();
    let mut s = Studio::new();
    let p2 = path.to_string_lossy().to_string();
    s.add("importForeign", move |p| p.set("blobData", ParamValue::Text(p2)));
    let r = s.ok();
    assert!(near(r.context.bodies[0].shape.volume().unwrap(), 6e-6, 1e-12));
    s.add("importForeign", |_| {});
    let r = s.regen();
    assert_eq!(r.display.features[1].status, FeatureStatus::Error);
    let _ = std::fs::remove_dir_all(dir);
}

/// Imports keep the file's part names and colours (Onshape's default
/// "Import appearances"): per part and per face, through later features.
#[test]
fn import_foreign_names_and_colors() {
    use noshape_model::features::importing::ImportPartInfo;
    let a = noshape_kernel::make_box(&noshape_kernel::Plane::xy(), 0.01, 0.01, 0.01).unwrap();
    let b = noshape_kernel::make_box_corners(DVec3::new(0.02, 0.0, 0.0), DVec3::new(0.03, 0.01, 0.01)).unwrap();
    let brep = String::from_utf8(noshape_kernel::Shape::compound(&[a, b.clone()]).to_brep_bytes().unwrap()).unwrap();
    // Box B's face 0 is red, the part itself blue; box A has only a name.
    let info = vec![
        ImportPartInfo { name: Some("Bracket".into()), color: None, opacity: None, faces: Vec::new() },
        ImportPartInfo { name: None, color: Some([0.0, 0.0, 1.0]), opacity: None, faces: vec![(0, [1.0, 0.0, 0.0])] },
    ];
    let json = serde_json::to_string(&info).unwrap();
    let mut s = Studio::new();
    let (b2, j2) = (brep.clone(), json.clone());
    s.add("importForeign", move |p| {
        p.set("blobData", ParamValue::Text(b2));
        p.set("partInfo", ParamValue::Text(j2));
    });
    let r = s.ok();
    assert_eq!(r.display.bodies.len(), 2);
    let named = r.display.bodies.iter().find(|d| d.name == "Bracket").expect("the file's name");
    let other = r.display.bodies.iter().find(|d| d.name != "Bracket").unwrap();
    assert!(named.face_colors.is_empty());
    assert_eq!(other.color, [0.0, 0.0, 1.0, 1.0], "the part's colour, linear");
    assert_eq!(other.face_colors.iter().filter(|c| c.is_some()).count(), 1);
    assert!(other.face_colors.contains(&Some([1.0, 0.0, 0.0, 1.0])));
    // The coloured face keeps its colour after a fillet elsewhere on the part.
    let bi = r.context.bodies.iter().position(|x| x.name != "Bracket").unwrap();
    let edge = Reference::Edge((*r.context.bodies[bi].edges[0]).clone());
    s.add("fillet", move |p| {
        p.set("entities", ParamValue::Query(vec![edge]));
        p.set("radius", ParamValue::Expr(noshape_units::ExprValue::new("1 mm")));
    });
    let r = s.ok();
    let other = r.display.bodies.iter().find(|d| d.name != "Bracket").unwrap();
    assert!(other.face_colors.contains(&Some([1.0, 0.0, 0.0, 1.0])));
    // Without "Import appearances": names only.
    let mut s = Studio::new();
    s.add("importForeign", move |p| {
        p.set("blobData", ParamValue::Text(brep));
        p.set("partInfo", ParamValue::Text(json));
        p.set("importAppearances", ParamValue::Bool(false));
    });
    let r = s.ok();
    assert!(r.display.bodies.iter().any(|d| d.name == "Bracket"));
    assert!(r.display.bodies.iter().all(|d| d.face_colors.is_empty() && d.color != [0.0, 0.0, 1.0, 1.0]));
}

/// Mesh imports (Onshape G22/G27): STL and PVZ parts are mesh parts (one
/// face, no edges); a PVZ part whose mesh doesn't close (Onshape's loft
/// sample) becomes a closed composite of its 2 open pieces.
#[test]
fn import_foreign_mesh_parts() {
    let dir = std::env::temp_dir().join(format!("noshape-featb-mesh-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("box.stl");
    let bx = noshape_kernel::make_box(&noshape_kernel::Plane::xy(), 0.01, 0.02, 0.03).unwrap();
    noshape_kernel::write_stl(&bx, &path, 1e-4, noshape_kernel::StlFormat::Binary).unwrap();
    let mesh = noshape_kernel::read_mesh_bodies(&path, noshape_kernel::LengthUnit::Meter).unwrap();
    let brep = String::from_utf8(mesh.to_brep_bytes().unwrap()).unwrap();
    let mut s = Studio::new();
    s.add("importForeign", move |p| p.set("blobData", ParamValue::Text(brep)));
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 1);
    let b = &r.context.bodies[0];
    assert!(b.shape.is_mesh() && b.kind == BodyKind::Solid && b.faces.len() == 1 && b.edges.is_empty());
    assert!(near(b.shape.volume().unwrap(), 6e-6, 1e-11));
    assert!(r.display.bodies[0].is_mesh);
    let _ = std::fs::remove_dir_all(dir);
    let Ok(shapes) = noshape_kernel::read_pvz(std::path::Path::new("/root/onshape-research/live/samples/pvz-shapes.pvz"), noshape_kernel::LengthUnit::Meter) else { return };
    let brep = String::from_utf8(shapes.to_brep_bytes().unwrap()).unwrap();
    let mut s = Studio::new();
    s.add("importForeign", move |p| p.set("blobData", ParamValue::Text(brep)));
    let r = s.ok();
    // Parts (3) + Composite parts (1) "Composite part 1 (closed) (2 surfaces)".
    let parts: Vec<_> = r.display.bodies.iter().filter(|b| b.composite.is_none()).collect();
    assert_eq!(parts.len(), 3, "{:?}", r.display.bodies.iter().map(|b| &b.name).collect::<Vec<_>>());
    assert!(parts.iter().all(|b| b.is_mesh && b.kind == BodyKind::Solid));
    let comp = r.display.bodies.iter().find(|b| b.composite.is_some()).expect("composite");
    let info = comp.composite.as_ref().unwrap();
    assert!(info.closed && info.members.len() == 2 && comp.name == "Composite part 1", "{info:?} {}", comp.name);
}

#[test]
fn reference_misc_point_composite_query() {
    let mut s = block();
    let r = s.ok();
    let b = &r.context.bodies[0];
    let e = Reference::Edge((*b.edges[0]).clone());
    s.add("cPoint", move |p| {
        p.set("entities", ParamValue::Query(vec![e]));
        p.set("parameter", expr("0.5"));
    });
    let body = Reference::Body(BodyId { feature: s.ps.features[1].id, index: 0 });
    let bd = body.clone();
    s.add("queryVariable", move |p| {
        p.set("name", ParamValue::Text("q".into()));
        p.set("selectionQuery", ParamValue::Query(vec![bd]));
    });
    s.add("nameEntity", |_| {});
    let pt = Reference::Body(BodyId { feature: s.ps.features[2].id, index: 0 });
    s.add("compositePart", move |p| p.set("bodies", ParamValue::Query(vec![body, pt])));
    let top = top_face(&r);
    s.add("offsetSurface", move |p| p.set("surfacesAndFaces", ParamValue::Query(vec![top])));
    let r = s.ok();
    assert_eq!(r.context.points.len(), 1);
    // Composite keeps face names: the top face is still found downstream.
    // Open (the default): the part stays, the composite comes after it.
    assert!(r.context.bodies.iter().any(|b| b.name.starts_with("Composite part")));
    assert!(!r.context.bodies[0].name.starts_with("Composite part"));
    assert_eq!(r.context.bodies.iter().filter(|b| b.kind == BodyKind::Surface).count(), 1);
}

#[test]
fn projected_curves_both_modes() {
    let mut s = block();
    s.add("cPlane", |p| {
        p.set("entities", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        p.set("offset", expr("50 mm"));
    });
    let plane = s.ps.features.last().unwrap().id;
    let mut sk = noshape_sketch::Sketch::new();
    let l = sk.add_line(DVec2::new(0.01, 0.01), DVec2::new(0.06, 0.04)).0;
    let above = s.sketch(Reference::FeaturePlane(plane), sk);
    let r = s.ok();
    let top = top_face(&r);
    s.add("projectCurves", move |p| {
        p.set("curveProjectionType", ParamValue::Enum("CURVE_TO_FACE".into()));
        p.set("dropTools", ParamValue::Query(vec![Reference::SketchEntity { sketch: above, entity: l }]));
        p.set("directionQuery", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        p.set("targets", ParamValue::Query(vec![top]));
    });
    // Two sketches: a line on Top and a sloped line on Front.
    let mut a = noshape_sketch::Sketch::new();
    let la = a.add_line(DVec2::new(0.0, 0.08), DVec2::new(0.04, 0.08)).0;
    let sa = s.sketch(Reference::Plane(DatumPlane::Top), a);
    let mut b = noshape_sketch::Sketch::new();
    let lb = b.add_line(DVec2::new(0.0, 0.01), DVec2::new(0.04, 0.03)).0;
    let sb = s.sketch(Reference::Plane(DatumPlane::Front), b);
    s.add("projectCurves", move |p| {
        p.set("curveProjectionType", ParamValue::Enum("TWO_SKETCHES".into()));
        p.set("sketchEdges1", ParamValue::Query(vec![Reference::SketchEntity { sketch: sa, entity: la }]));
        p.set("sketchEdges2", ParamValue::Query(vec![Reference::SketchEntity { sketch: sb, entity: lb }]));
    });
    let r = s.ok();
    let wires: Vec<usize> = (0..r.context.bodies.len()).filter(|&i| r.context.bodies[i].kind == BodyKind::Wire).collect();
    assert_eq!(wires.len(), 2);
    let l0 = wire_length(&r, wires[0]);
    assert!(near(l0, (0.05f64.powi(2) + 0.03f64.powi(2)).sqrt(), 1e-7), "dropped length {l0}");
    let bb = r.context.bodies[wires[0]].shape.bounding_box(true).unwrap();
    assert!(near(bb.max.z, 0.025, 1e-6), "{bb:?}");
    let l1 = wire_length(&r, wires[1]);
    assert!(near(l1, (0.04f64.powi(2) + 0.02f64.powi(2)).sqrt(), 1e-6), "two-sketch length {l1}");
}

#[test]
fn boundary_routing_edit_isoparametric() {
    let mut s = block();
    let mut sk = noshape_sketch::Sketch::new();
    let u0 = sk.add_line(DVec2::new(0.0, 0.05), DVec2::new(0.04, 0.05)).0;
    let u1 = sk.add_line(DVec2::new(0.0, 0.07), DVec2::new(0.04, 0.07)).0;
    let sid = s.sketch(Reference::Plane(DatumPlane::Front), sk);
    s.add("boundarySurface", move |p| {
        let row = |e: u32| {
            let mut v = ParamValues::default();
            v.set("uProfileEntities", ParamValue::Query(vec![Reference::SketchEntity { sketch: sid, entity: e }]));
            v.set("uCondition", ParamValue::Enum("DEFAULT".into()));
            v
        };
        p.set("uProfilesArray", ParamValue::Array(vec![row(u0), row(u1)]));
    });
    s.add("routingCurve", |p| {
        let pt = |x: &str, y: &str| {
            let mut v = ParamValues::default();
            v.set("referenceType", ParamValue::Enum("ORIGIN".into()));
            v.set("dx", expr(x));
            v.set("dy", expr(y));
            v.set("dz", expr("0 mm"));
            v
        };
        p.set("curveType", ParamValue::Enum("POLYLINE".into()));
        p.set("bendRadius", expr("5 mm"));
        p.set("points", ParamValue::Array(vec![pt("0 mm", "0 mm"), pt("50 mm", "0 mm"), pt("50 mm", "50 mm")]));
    });
    let helix = s.add("helix", |p| {
        p.set("axisType", ParamValue::Enum("AXIS".into()));
        p.set("axis", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        p.set("startRadius", expr("10 mm"));
        p.set("height", expr("20 mm"));
        p.set("revolutions", expr("2"));
    });
    s.add("editCurve", move |p| {
        p.set("wire", ParamValue::Query(vec![Reference::Body(BodyId { feature: helix, index: 0 })]));
        p.set("planarize", ParamValue::Bool(true));
        p.set("planeReference", ParamValue::Enum("XY".into()));
    });
    let r = s.ok();
    let top = top_face(&r);
    s.add("isoparametricCurve", move |p| {
        p.set("face", ParamValue::Query(vec![top]));
        p.set("equalSpacing", ParamValue::Bool(true));
        p.set("nCurves", expr("3"));
    });
    let r = s.ok();
    let sf = r.context.bodies.iter().position(|b| b.kind == BodyKind::Surface).unwrap();
    assert!(near(surface_area(&r, sf), 0.04 * 0.02, 1e-8), "boundary area {}", surface_area(&r, sf));
    let wires: Vec<usize> = (0..r.context.bodies.len()).filter(|&i| r.context.bodies[i].kind == BodyKind::Wire).collect();
    assert_eq!(wires.len(), 5, "routing + helix + 3 isoparametric");
    let route = wire_length(&r, wires[0]);
    let expected = 0.1 - 2.0 * 0.005 + PI / 2.0 * 0.005;
    assert!(near(route, expected, 1e-7), "routing length {route} vs {expected}");
    let bb = r.context.bodies[wires[1]].shape.bounding_box(true).unwrap();
    assert!(bb.max.z.abs() < 1e-6 && bb.min.z.abs() < 1e-6, "planarized helix {bb:?}");
    for &w in &wires[2..] {
        let l = wire_length(&r, w);
        assert!(near(l, 0.1, 1e-6) || near(l, 0.05, 1e-6), "iso length {l}");
    }
}

#[test]
fn offset_curve_move_boundary_mutual_trim() {
    let mut s = block();
    let r = s.ok();
    let top = top_face(&r);
    let b = &r.context.bodies[0];
    let e = b.shape.edges().iter().position(|e| {
        let (a, c) = e.endpoints().unwrap();
        near(a.z, 0.025, 1e-9) && near(c.z, 0.025, 1e-9) && a.y.abs() < 1e-9 && c.y.abs() < 1e-9
    });
    let edge = Reference::Edge((*b.edges[e.unwrap()]).clone());
    let t2 = top.clone();
    s.add("offsetCurveOnFace", move |p| {
        p.set("edges", ParamValue::Query(vec![edge]));
        p.set("distance", expr("5 mm"));
        p.set("targets", ParamValue::Query(vec![t2]));
    });
    let off = s.add("offsetSurface", move |p| {
        p.set("surfacesAndFaces", ParamValue::Query(vec![top]));
        p.set("offset", expr("10 mm"));
    });
    s.add("extendSurface", move |p| {
        p.set("entities", ParamValue::Query(vec![Reference::Body(BodyId { feature: off, index: 0 })]));
        p.set("extendDistance", expr("5 mm"));
    });
    let r = s.ok();
    let w = r.context.bodies.iter().position(|b| b.kind == BodyKind::Wire).unwrap();
    assert!(near(wire_length(&r, w), 0.1, 1e-7), "offset curve length {}", wire_length(&r, w));
    let bb = r.context.bodies[w].shape.bounding_box(true).unwrap();
    assert!(near(bb.center().y, 0.005, 1e-7), "{bb:?}");
    let sf = r.context.bodies.iter().position(|b| b.kind == BodyKind::Surface).unwrap();
    assert!(near(surface_area(&r, sf), 0.11 * 0.06, 1e-8), "extended area {}", surface_area(&r, sf));

    // Mutual trim of two crossing fills.
    let mut s = Studio::new();
    let mut a = noshape_sketch::Sketch::new();
    let la = rect(&mut a, 0.0, 0.0, 0.05, 0.03);
    let sa = s.sketch(Reference::Plane(DatumPlane::Front), a);
    s.add("cPlane", |p| {
        p.set("entities", ParamValue::Query(vec![Reference::Plane(DatumPlane::Right)]));
        p.set("offset", expr("20 mm"));
    });
    let pl = s.ps.features.last().unwrap().id;
    let mut b = noshape_sketch::Sketch::new();
    let lb = rect(&mut b, -0.01, 0.0, 0.04, 0.03);
    let sb = s.sketch(Reference::FeaturePlane(pl), b);
    let mut fills = Vec::new();
    for (sk, lines) in [(sa, la), (sb, lb)] {
        fills.push(s.add("fill", move |p| {
            let mut row = ParamValues::default();
            row.set("entities", ParamValue::Query(lines.iter().map(|&e| Reference::SketchEntity { sketch: sk, entity: e }).collect()));
            p.set("edges", ParamValue::Array(vec![row]));
        }));
    }
    s.add("mutualTrim", move |p| {
        p.set("body1", ParamValue::Query(vec![Reference::Body(BodyId { feature: fills[0], index: 0 })]));
        p.set("body2", ParamValue::Query(vec![Reference::Body(BodyId { feature: fills[1], index: 0 })]));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 1);
    let a = surface_area(&r, 0);
    // Front keeps x in [0.02, 0.05] (0.03 x 0.03), the other keeps y in [0, 0.03].
    let front_y = r.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!(near(a, 0.03 * 0.03 * 2.0, 1e-8), "trimmed area {a} {front_y:?}");
}

/// Mesh parts in booleans and splits (live G35/G35-retry): a split by a plane
/// gives two MIXED parts (the mesh face plus a planar cap), a translation
/// keeps an all-mesh part, a union with a B-rep part gives one MIXED part.
#[test]
fn mesh_parts_split_transform_union() {
    let dir = std::env::temp_dir().join(format!("noshape-featb-meshops-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("box.stl");
    // 20 x 20 x 30 mm, centred on the Right plane (x = 0).
    let bx = noshape_kernel::make_box_corners(DVec3::new(-0.01, 0.0, 0.0), DVec3::new(0.01, 0.02, 0.03)).unwrap();
    noshape_kernel::write_stl(&bx, &path, 1e-4, noshape_kernel::StlFormat::Binary).unwrap();
    let mesh = noshape_kernel::read_mesh_bodies(&path, noshape_kernel::LengthUnit::Meter).unwrap();
    let mesh_brep = String::from_utf8(mesh.to_brep_bytes().unwrap()).unwrap();
    let _ = std::fs::remove_dir_all(dir);
    let mm3 = |v: f64| v * 1e9;
    let studio = || {
        let mut s = Studio::new();
        let b = mesh_brep.clone();
        s.add("importForeign", move |p| p.set("blobData", ParamValue::Text(b)));
        let r = s.ok();
        let id = r.context.bodies[0].id.clone();
        // Onshape's generated orange for a mesh part without a colour (live G35).
        let c = r.context.part_appearance[&id];
        assert!((c[0] - 0.939).abs() < 1e-3 && (c[1] - 0.242).abs() < 1e-3 && c[2] < 1e-3, "{c:?}");
        (s, id)
    };
    let mixed = |b: &noshape_model::regen::context::Body| {
        let faces = b.shape.faces();
        (faces.iter().filter(|f| f.is_mesh()).count(), faces.iter().filter(|f| !f.is_mesh()).count())
    };

    // Split by the Right plane.
    let (mut s, id) = studio();
    s.add("splitPart", move |p| {
        p.set("targets", ParamValue::Query(vec![Reference::Body(id)]));
        p.set("tool", ParamValue::Query(vec![Reference::Plane(DatumPlane::Right)]));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 2);
    for b in r.context.bodies.iter() {
        assert_eq!(mixed(b), (1, 1), "{}: one mesh face and its planar cap", b.name);
        assert!((mm3(b.shape.volume().unwrap()) - 6000.0).abs() < 1e-3, "{}: {}", b.name, mm3(b.shape.volume().unwrap()));
        assert!(r.display.bodies.iter().find(|d| d.id == b.id).is_some_and(|d| d.is_mesh));
    }

    // Translate by XYZ: still all mesh.
    let (mut s, id) = studio();
    s.add("transform", move |p| {
        p.set("entities", ParamValue::Query(vec![Reference::Body(id)]));
        p.set("transformType", ParamValue::Enum("TRANSLATION_3D".into()));
        p.set("dx", expr("10 mm"));
    });
    let r = s.ok();
    let b = &r.context.bodies[0];
    assert_eq!(mixed(b), (1, 0));
    let bb = b.shape.bounding_box(true).unwrap();
    assert!((bb.min.x - 0.0).abs() < 1e-6 && (bb.max.x - 0.02).abs() < 1e-6, "{bb:?}");

    // Union with an overlapping B-rep box (x 0..20 mm): one MIXED part.
    let (mut s, id) = studio();
    let block = String::from_utf8(noshape_kernel::make_box_corners(DVec3::new(0.0, 0.0, 0.0), DVec3::new(0.02, 0.02, 0.01)).unwrap().to_brep_bytes().unwrap()).unwrap();
    s.add("importForeign", move |p| p.set("blobData", ParamValue::Text(block)));
    let r = s.ok();
    let other = r.context.bodies.iter().find(|b| b.id != id).unwrap().id.clone();
    s.add("booleanBodies", move |p| {
        p.set("operationType", ParamValue::Enum("UNION".into()));
        p.set("tools", ParamValue::Query(vec![Reference::Body(id), Reference::Body(other)]));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 1);
    let b = &r.context.bodies[0];
    let (m, planes) = mixed(b);
    assert!(m == 1 && planes > 0, "MIXED: {m} mesh, {planes} B-rep faces");
    // 12000 + 2000 (the block's half outside the mesh box).
    assert!((mm3(b.shape.volume().unwrap()) - 14000.0).abs() < 1e-3, "{}", mm3(b.shape.volume().unwrap()));
}

/// qa5 NAME3: imported parts keep the file's names and order, even names
/// like "Part 2" listed first ("Part N" renumbering is for parts made here).
#[test]
fn imported_part_names_are_not_renumbered() {
    use noshape_model::features::importing::ImportPartInfo;
    let a = noshape_kernel::make_box(&noshape_kernel::Plane::xy(), 0.01, 0.01, 0.01).unwrap();
    let b = noshape_kernel::make_box_corners(DVec3::new(0.02, 0.0, 0.0), DVec3::new(0.03, 0.01, 0.01)).unwrap();
    let brep = String::from_utf8(noshape_kernel::Shape::compound(&[a, b]).to_brep_bytes().unwrap()).unwrap();
    let info = vec![
        ImportPartInfo { name: Some("Part 2".into()), color: None, opacity: None, faces: Vec::new() },
        ImportPartInfo { name: Some("Part 1".into()), color: None, opacity: None, faces: Vec::new() },
    ];
    let json = serde_json::to_string(&info).unwrap();
    let mut s = Studio::new();
    s.add("importForeign", move |p| {
        p.set("blobData", ParamValue::Text(brep));
        p.set("partInfo", ParamValue::Text(json));
    });
    let r = s.ok();
    let names: Vec<&str> = r.display.bodies.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["Part 2", "Part 1"]);
}

/// Live L28 ("psb L28 boundary", 2099/2100): one U and one V profile meeting
/// at a corner make the sheet the U sweeps along the V; a V crossing the U
/// profiles in their middle can't be matched at the ends.
type Seg = ((f64, f64), (f64, f64));

#[test]
fn boundary_one_u_one_v_as_live() {
    let run = |u: &[Seg], v: &[Seg]| {
        let mut s = Studio::new();
        let mut one = |a: (f64, f64), b: (f64, f64)| {
            let mut sk = noshape_sketch::Sketch::new();
            let l = sk.add_line(DVec2::new(a.0, a.1), DVec2::new(b.0, b.1)).0;
            (s.sketch(Reference::Plane(DatumPlane::Top), sk), l)
        };
        let us: Vec<_> = u.iter().map(|(a, b)| one(*a, *b)).collect();
        let vs: Vec<_> = v.iter().map(|(a, b)| one(*a, *b)).collect();
        s.add("boundarySurface", move |p| {
            let rows = |list: &[(FeatureId, u32)], ents: &str, cond: &str| {
                ParamValue::Array(
                    list.iter()
                        .map(|(sk, e)| {
                            let mut v = ParamValues::default();
                            v.set(ents, ParamValue::Query(vec![Reference::SketchEntity { sketch: *sk, entity: *e }]));
                            v.set(cond, ParamValue::Enum("DEFAULT".into()));
                            v
                        })
                        .collect(),
                )
            };
            p.set("uProfilesArray", rows(&us, "uProfileEntities", "uCondition"));
            p.set("vProfilesArray", rows(&vs, "vProfileEntities", "vCondition"));
        });
        let r = s.regen();
        let st = r.display.features.last().unwrap().clone();
        (st.status, st.message, r.context.bodies.iter().position(|b| b.kind == BodyKind::Surface).map(|i| surface_area(&r, i)))
    };
    let (st, msg, area) = run(&[((0.0, 0.0), (0.04, 0.0))], &[((0.0, 0.0), (0.0, 0.02))]);
    assert_eq!(st, FeatureStatus::Ok, "{msg:?}");
    assert!(near(area.unwrap(), 0.04 * 0.02, 1e-9), "{area:?}");
    let (st, msg, _) = run(&[((0.0, 0.0), (0.04, 0.0)), ((0.0, 0.02), (0.04, 0.02))], &[((0.02, -0.005), (0.02, 0.025))]);
    assert_eq!(st, FeatureStatus::Error);
    assert_eq!(msg.as_deref(), Some("Could not extract boundary profiles matching at end points."));
}

/// Live L28: a U row "Normal to profile" on a part edge (no sketch plane)
/// fails with loft.fs's planar-profile text; the same rows with None work.
#[test]
fn boundary_condition_needs_a_planar_profile() {
    let run = |cond: &str| {
        let mut s = block();
        let r = s.ok();
        let b = &r.context.bodies[0];
        let edge_at = |p: DVec3| {
            let v = noshape_kernel::make_vertex(p).unwrap();
            let i = b.shape.edges().iter().position(|e| noshape_kernel::distance(&v, e).is_ok_and(|d| d.distance < 1e-9)).unwrap();
            Reference::Edge((*b.edges[i]).clone())
        };
        let (front, back) = (edge_at(DVec3::new(0.05, 0.0, 0.025)), edge_at(DVec3::new(0.05, 0.05, 0.025)));
        let cond = cond.to_string();
        s.add("boundarySurface", move |p| {
            let row = |e: Reference, c: &str| {
                let mut v = ParamValues::default();
                v.set("uProfileEntities", ParamValue::Query(vec![e]));
                v.set("uCondition", ParamValue::Enum(c.into()));
                v.set("uMagnitude", ParamValue::Expr(ExprValue::new("1")));
                v
            };
            p.set("uProfilesArray", ParamValue::Array(vec![row(front, &cond), row(back, "DEFAULT")]));
        });
        let r = s.regen();
        let st = r.display.features.last().unwrap().clone();
        (st.status, st.message)
    };
    assert_eq!(run("DEFAULT").0, FeatureStatus::Ok);
    assert_eq!(run("NORMAL_TO_PROFILE"), (FeatureStatus::Error, Some("Only planar profiles are valid for Normal/Tangent to profile start condition.".into())));
}

/// mateConnector.fs "Owner entity": a connector moves with its owner part
/// (a Transform of the part moves it too).
#[test]
fn mate_connector_follows_its_owner() {
    let mut s = block();
    let r = s.ok();
    let top = top_face(&r);
    let part = r.context.bodies[0].id.clone();
    s.add("mateConnector", |p| {
        p.set("originQuery", ParamValue::Query(vec![top.clone()]));
        p.set("entityInferenceType", ParamValue::Enum("CENTROID".into()));
    });
    let before = s.ok().context.connectors[0].origin;
    s.add("transform", move |p| {
        p.set("entities", ParamValue::Query(vec![Reference::Body(part)]));
        p.set("transformType", ParamValue::Enum("TRANSLATION_3D".into()));
        p.set("dx", expr("10 mm"));
    });
    let after = s.ok().context.connectors[0].origin;
    assert!(vnear(after - before, DVec3::new(0.01, 0.0, 0.0)), "{:?}", after - before);
}
