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
        // Sheet metal runs by std, as in the app (the host's natives).
        noshape_model::custom::register();
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
            p.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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

/// A Mate connector at a cylinder's mid axis point: Z the cylinder's axis, X world X across it (world Y for an axis
/// along X), not the cylinder's own x (live "skasm (11-14) through-all hole cylinder frame": on a circle extruded
/// from Front x (1,0,0), whose cylinder's x is (0,0,−1); from Right x (0,1,0)).
#[test]
fn mate_connector_on_a_cylinder_takes_world_x() {
    for (plane, x) in [(DatumPlane::Front, DVec3::X), (DatumPlane::Right, DVec3::Y)] {
        let mut s = Studio::new();
        let mut sk = noshape_sketch::Sketch::new();
        sk.add_circle(DVec2::new(0.01, 0.02), 0.005);
        let id = s.sketch(Reference::Plane(plane), sk);
        s.extrude_largest(id, "20 mm");
        let r = s.ok();
        let b = &r.context.bodies[0];
        let side = b.shape.faces().iter().position(|f| matches!(f.face_surface(), Ok(noshape_kernel::Surface::Cylinder { .. }))).expect("the cylinder");
        let face = Reference::Face((*b.faces[side]).clone());
        s.add("mateConnector", |p| {
            p.set("originQuery", ParamValue::Query(vec![face]));
            p.set("entityInferenceType", ParamValue::Enum("MID_AXIS_POINT".into()));
        });
        let r = s.ok();
        let c = &r.context.connectors[0];
        assert!(vnear(c.x, x) && c.z.cross(x).length() > 1.0 - 1e-9 && c.z.dot(x).abs() < 1e-9, "{plane:?}: x {:?} z {:?}", c.x, c.z);
    }
}

/// A Virtual sharp mate connector on a face with a rounded corner's arc as its secondary: at the corner the arc
/// rounds, where the lines along its ends meet (live c2-cov2-mate-connector-inference's MC virtual sharp: (240, 30, 20)
/// on a 5 mm round, the face's centroid before).
#[test]
fn a_virtual_sharp_mate_connector_is_at_the_rounded_corner() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let p: Vec<_> = [(0.0, 0.0), (0.04, 0.0), (0.04, 0.015), (0.035, 0.02), (0.0, 0.02)].iter().map(|&(x, y)| sk.add_point(DVec2::new(x, y))).collect();
    let c = sk.add_point(DVec2::new(0.035, 0.015));
    sk.add_line_between(p[0], p[1]).unwrap();
    sk.add_line_between(p[1], p[2]).unwrap();
    sk.add_arc_between(c, p[2], p[3]).unwrap();
    sk.add_line_between(p[3], p[4]).unwrap();
    sk.add_line_between(p[4], p[0]).unwrap();
    let id = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    s.extrude_largest(id, "10 mm");
    let r = s.ok();
    let top = top_face(&r);
    let b = &r.context.bodies[0];
    let arc = b.shape.edges().iter().position(|e| matches!(e.edge_curve(), Ok(noshape_kernel::Curve::Circle { .. })) && e.endpoints().is_ok_and(|(a, _)| (a.z - 0.01).abs() < 1e-9)).expect("the top arc");
    let arc = Reference::Edge((*b.edges[arc]).clone());
    s.add("mateConnector", |p| {
        p.set("originQuery", ParamValue::Query(vec![top]));
        p.set("entityInferenceType", ParamValue::Enum("VIRTUAL_SHARP".into()));
        p.set("secondaryOriginQuery", ParamValue::Query(vec![arc]));
    });
    let r = s.ok();
    let c = &r.context.connectors[0];
    assert!(vnear(c.origin, DVec3::new(0.04, 0.02, 0.01)), "origin {:?}", c.origin);
    assert!(vnear(c.z, DVec3::Z), "z {:?}", c.z);
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

/// A mate connector on a sketch region (cf-transform-pattern's Mate connector 8): at its centroid, Z along the
/// sketch's normal.
#[test]
fn mate_connector_on_a_sketch_region() {
    let mut s = block();
    let mut sk = noshape_sketch::Sketch::new();
    rect(&mut sk, 0.0, 0.0, 0.04, 0.02);
    let sketch = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let r = s.ok();
    let key = r.display.sketches.iter().find(|x| x.feature == sketch).expect("sketch").regions[0].key.clone();
    let part = r.context.bodies[0].id.clone();
    s.add("mateConnector", |p| {
        p.set("originQuery", ParamValue::Query(vec![Reference::SketchRegion { sketch, key }]));
        p.set("entityInferenceType", ParamValue::Enum("CENTROID".into()));
        p.set("ownerPart", ParamValue::Query(vec![Reference::Body(part)]));
    });
    let r = s.ok();
    let c = &r.context.connectors[0];
    assert!(vnear(c.origin, DVec3::new(0.02, 0.01, 0.0)), "{:?}", c.origin);
    assert!(vnear(c.z.abs(), DVec3::Z), "{:?}", c.z);
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

/// Live 1708: a mate connector at the CENTER of a circular edge points out of
/// the material through the planar face the edge bounds (+z on top rims, −z
/// on bottom rims, a pin's and a hole's alike), its X along the sketch's x.
#[test]
fn mate_connectors_on_circular_edges_face_out_of_the_material() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    rect(&mut sk, 0.0, 0.0, 0.04, 0.02);
    sk.add_circle(DVec2::new(0.02, 0.01), 0.004);
    let id = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    s.extrude_largest(id, "10 mm");
    let mut pin = noshape_sketch::Sketch::new();
    pin.add_circle(DVec2::new(0.07, 0.01), 0.005);
    let id = s.sketch(Reference::Plane(DatumPlane::Top), pin);
    s.extrude_largest(id, "10 mm");
    let r = s.ok();
    let cases = [(DVec3::new(0.07, 0.01, 0.0), -1.0), (DVec3::new(0.07, 0.01, 0.01), 1.0), (DVec3::new(0.02, 0.01, 0.0), -1.0), (DVec3::new(0.02, 0.01, 0.01), 1.0)];
    for (centre, _) in cases {
        let (bi, ei) = r
            .context
            .bodies
            .iter()
            .enumerate()
            .find_map(|(bi, b)| {
                b.shape.edges().iter().position(|e| matches!(e.edge_info().map(|i| i.curve), Ok(noshape_kernel::Curve::Circle { center, .. }) if vnear(center, centre))).map(|ei| (bi, ei))
            })
            .unwrap_or_else(|| panic!("no circle at {centre:?}"));
        let edge = Reference::Edge((*r.context.bodies[bi].edges[ei]).clone());
        s.add("mateConnector", move |p| {
            p.set("originQuery", ParamValue::Query(vec![edge]));
            p.set("entityInferenceType", ParamValue::Enum("CENTER".into()));
        });
    }
    let r = s.ok();
    for (k, (centre, zsign)) in cases.iter().enumerate() {
        let c = &r.context.connectors[k];
        assert!(vnear(c.origin, *centre) && vnear(c.z, DVec3::Z * *zsign) && vnear(c.x, DVec3::X), "{k}: {:?} z {:?} x {:?}", c.origin, c.z, c.x);
    }
}

/// Live ("ui (10-25 d)", ui-mass/04): a mate connector on a block's corner
/// vertex is world-aligned (Z up, X along world X), whichever faces meet
/// there.
#[test]
fn mate_connector_on_a_vertex_is_world_aligned() {
    let mut s = block();
    let r = s.ok();
    let b = &r.context.bodies[0];
    let corner = DVec3::new(0.1, 0.05, 0.025);
    let i = (0..b.shape.vertices().len()).min_by(|&x, &y| {
        let d = |k: usize| (b.shape.vertices()[k].vertex_point().unwrap() - corner).length();
        d(x).total_cmp(&d(y))
    });
    let vertex = Reference::Vertex((*b.vertices[i.unwrap()]).clone());
    s.add("mateConnector", |p| p.set("originQuery", ParamValue::Query(vec![vertex])));
    let r = s.ok();
    let c = &r.context.connectors[0];
    assert!(vnear(c.origin, corner), "{:?}", c.origin);
    assert!(vnear(c.z, DVec3::Z) && vnear(c.x, DVec3::X), "z {:?} x {:?}", c.z, c.x);
}

/// A sketch vertex too, whatever plane it was sketched on (cf-surface-pattern's Mate connector 1 on a sphere's axis
/// end, sketched off Front: live's Surface Pattern 3 sets the spheres on the face, up along world Z).
#[test]
fn mate_connector_on_a_sketch_vertex_is_world_aligned() {
    let mut s = block();
    let mut sk = noshape_sketch::Sketch::new();
    let pt = sk.add_point(DVec2::new(0.01, 0.02));
    let sketch = s.sketch(Reference::Plane(DatumPlane::Front), sk);
    s.add("mateConnector", |p| p.set("originQuery", ParamValue::Query(vec![Reference::SketchEntity { sketch, entity: pt.0 }])));
    let r = s.ok();
    let c = &r.context.connectors[0];
    assert!(vnear(c.origin, DVec3::new(0.01, 0.0, 0.02)), "{:?}", c.origin);
    assert!(vnear(c.z, DVec3::Z) && vnear(c.x, DVec3::X), "z {:?} x {:?}", c.z, c.x);
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

/// A curve pattern along a helix carries its seed as a sweep along it would (curvePattern.fs sweeps a line along the
/// path; Onshape's frame turns with the helix): each instance is the seed screwed along the helix, a plate standing at
/// its start radially stays radial (c2-loft-guide-curves-driven-pattern's Curve pattern 1). Every 18° each instance
/// sits on a joint of the helix's 9° arcs, whose own curvature turns there: ours rolled 0-0.57° by where the frames
/// were sampled.
#[test]
fn a_curve_pattern_along_a_helix_screws_its_seed() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    rect(&mut sk, 0.008, -0.002, 0.004, 0.004);
    let plate = s.sketch(Reference::Plane(DatumPlane::Front), sk);
    let plate = s.extrude_largest(plate, "0.2 mm");
    let helix = s.add("helix", |p| {
        p.set("axisType", ParamValue::Enum("AXIS".into()));
        p.set("axis", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        p.set("startRadius", expr("10 mm"));
        p.set("height", expr("20 mm"));
        p.set("revolutions", expr("2"));
    });
    s.add("curvePattern", move |p| {
        p.set("entities", ParamValue::Query(vec![Reference::Body(BodyId { feature: plate, index: 0 })]));
        p.set("edges", ParamValue::Query(vec![Reference::Body(BodyId { feature: helix, index: 0 })]));
        p.set("instanceCount", expr("41"));
    });
    let r = s.ok();
    let solids: Vec<_> = r.context.bodies.iter().filter(|b| b.kind == BodyKind::Solid).map(|b| b.shape.bounding_box(true).unwrap()).collect();
    assert_eq!(solids.len(), 41);
    let seed = solids[0];
    // 18° turns, 0.5 mm up each (within the path samples' chord sag, 0.4 µm at 1° a sample here; the sampled
    // frames' roll put them up to 49 µm off).
    for (i, b) in solids.iter().enumerate().skip(1) {
        let rot = glam::DQuat::from_rotation_z(std::f64::consts::PI / 10.0 * i as f64);
        let corners = (0..8).map(|c| DVec3::new([seed.min.x, seed.max.x][c & 1], [seed.min.y, seed.max.y][(c >> 1) & 1], [seed.min.z, seed.max.z][c >> 2]));
        let moved: Vec<DVec3> = corners.map(|c| rot * c + DVec3::Z * 0.0005 * i as f64).collect();
        let lo = moved.iter().fold(DVec3::INFINITY, |a, &c| a.min(c));
        let hi = moved.iter().fold(DVec3::NEG_INFINITY, |a, &c| a.max(c));
        let off = (b.min - lo).abs().max_element().max((b.max - hi).abs().max_element());
        assert!(off < 5e-7, "instance {i}: {b:?}, the seed screwed {lo:?} {hi:?}");
    }
}

/// Helix › Circle takes a sketch circle as well as a model edge (its field
/// accepts both; a sketch circle failed "Select a circle or arc.").
#[test]
fn helix_on_a_sketch_circle() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let ring = sk.add_circle(DVec2::ZERO, 0.02);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    s.add("helix", move |p| {
        p.set("axisType", ParamValue::Enum("CIRCLE".into()));
        p.set("edge", ParamValue::Query(vec![Reference::SketchEntity { sketch: sid, entity: ring.0 }]));
        p.set("height", expr("30 mm"));
        p.set("revolutions", expr("3"));
    });
    let r = s.ok();
    let i = r.context.bodies.iter().position(|b| b.kind == BodyKind::Wire).expect("the helix");
    let expected = 3.0 * ((2.0 * PI * 0.02f64).powi(2) + 0.01f64.powi(2)).sqrt();
    let len = wire_length(&r, i);
    assert!(near(len, expected, expected * 1e-6), "length {len} vs {expected}");
}

/// An extruded circle's edges run from 0 round its cylinder's seam (qa's rust-zero-solid-f197: kept on the sketch
/// circle's frame, trimmed to [a, a + 2π], they made Fillet 1 fail): a Ø25.8 circle on each datum plane extruded 10 mm
/// either way, both its circular edges filleted R1.
#[test]
fn an_extruded_circles_edges_start_at_zero_and_fillet() {
    for plane in [DatumPlane::Top, DatumPlane::Front, DatumPlane::Right] {
        for opposite in [false, true] {
            let mut s = Studio::new();
            let mut sk = noshape_sketch::Sketch::new();
            sk.add_circle(DVec2::new(0.003, 0.002), 0.0129);
            let sid = s.sketch(Reference::Plane(plane), sk);
            let r = s.regen();
            let key = r.display.sketches.iter().find(|x| x.feature == sid).unwrap().regions[0].key.clone();
            s.add("extrude", move |p| {
                p.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key }]));
                p.set("depth", expr("10 mm"));
                p.set("oppositeDirection", ParamValue::Bool(opposite));
            });
            let r = s.ok();
            let b = r.context.bodies.iter().find(|b| b.kind == BodyKind::Solid).expect("the disc");
            let circles: Vec<usize> = b.shape.edges().iter().enumerate().filter(|(_, e)| e.edge_info().is_ok_and(|i| matches!(i.curve, noshape_kernel::Curve::Circle { .. }))).map(|(i, _)| i).collect();
            assert_eq!(circles.len(), 2);
            for &i in &circles {
                let info = b.shape.edges()[i].edge_info().unwrap();
                assert!(info.first_param.abs() < 1e-12 && (info.last_param - 2.0 * PI).abs() < 1e-12, "{plane:?} {opposite}: {} .. {}", info.first_param, info.last_param);
            }
            let edges: Vec<Reference> = circles.iter().map(|&i| Reference::Edge((*b.edges[i]).clone())).collect();
            s.add("fillet", move |p| {
                p.set("entities", ParamValue::Query(edges));
                p.set("radius", expr("1 mm"));
            });
            let r = s.ok();
            let v = r.context.bodies.iter().find(|b| b.kind == BodyKind::Solid).unwrap().shape.volume().unwrap() * 1e9;
            // Each rounded rim takes (1 − π/4)·f² of the section, its centroid f(10 − 3π)/(12 − 3π) in from the rim.
            let (rr, h, f) = (12.9f64, 10.0f64, 1.0f64);
            let want = PI * rr * rr * h - 2.0 * 2.0 * PI * (rr - f * (10.0 - 3.0 * PI) / (12.0 - 3.0 * PI)) * (1.0 - PI / 4.0) * f * f;
            assert!((v - want).abs() < 0.5, "{plane:?} {opposite}: {v} mm³, {want}");
        }
    }
}

/// Helix › Circle on an extruded circle's edge starts at the sketch circle's x, as Parasolid's cap edge keeps the
/// sketch circle (asm-hand-drill-pinslot studio bb72f3's Helix 1, live: at +x; ours started at -x, the cylinder's x).
#[test]
fn helix_on_an_extruded_circles_edge_starts_at_the_sketch_x() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(DVec2::ZERO, 0.0129);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    s.extrude_largest(sid, "2 mm");
    let r = s.ok();
    let v = noshape_kernel::make_vertex(DVec3::new(0.0, 0.0129, 0.0)).unwrap();
    let b = r.context.bodies.iter().find(|b| b.kind == BodyKind::Solid).expect("the disc");
    let i = b.shape.edges().iter().position(|e| noshape_kernel::distance(&v, e).is_ok_and(|d| d.distance < 1e-9)).expect("the bottom circle");
    let edge = Reference::Edge((*b.edges[i]).clone());
    s.add("helix", move |p| {
        p.set("axisType", ParamValue::Enum("CIRCLE".into()));
        p.set("edge", ParamValue::Query(vec![edge]));
        p.set("height", expr("6 mm"));
        p.set("revolutions", expr("1"));
    });
    let r = s.ok();
    let w = r.context.bodies.iter().find(|b| b.kind == BodyKind::Wire).expect("the helix");
    let (a, _) = w.shape.edges()[0].endpoints().unwrap();
    assert!(a.distance(DVec3::new(0.0129, 0.0, 0.0)) < 1e-9, "starts at {a:?}");
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

/// Offset surface: picked faces sharing an edge are one sheet, a sharp edge's offsets extended to meet; faces apart
/// are two (live 2034: a 40 × 20 × 10 block, R3 on its top front edge; 2 mm).
#[test]
fn offset_surface_joins_faces_sharing_edges() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    rect(&mut sk, 0.0, 0.0, 0.04, 0.02);
    let id = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    s.extrude_largest(id, "10 mm");
    let r = s.ok();
    let b = &r.context.bodies[0];
    let edge = b.shape.edges().iter().position(|e| {
        let (a, c) = e.endpoints().unwrap();
        a.y.abs() < 1e-9 && c.y.abs() < 1e-9 && near(a.z, 0.01, 1e-9) && near(c.z, 0.01, 1e-9)
    });
    let edge = Reference::Edge((*b.edges[edge.unwrap()]).clone());
    s.add("fillet", move |p| {
        p.set("entities", ParamValue::Query(vec![edge]));
        p.set("radius", expr("3 mm"));
        p.set("tangentPropagation", ParamValue::Bool(false));
    });
    let r = s.ok();
    let b = &r.context.bodies[0];
    let face = |want: &dyn Fn(DVec3) -> bool| {
        let fi = b.shape.faces().iter().position(|f| want(f.centroid().unwrap())).expect("face");
        Reference::Face((*b.faces[fi]).clone())
    };
    let top = face(&|c| near(c.z, 0.01, 1e-9));
    let round = face(&|c| c.y > 1e-6 && c.y < 0.003 && c.z > 0.007 && c.z < 0.01 - 1e-6);
    let front = face(&|c| c.y.abs() < 1e-9);
    let back = face(&|c| near(c.y, 0.02, 1e-9));
    let left = face(&|c| c.x.abs() < 1e-9);
    let right = face(&|c| near(c.x, 0.04, 1e-9));
    let cases = [
        ("top + back", vec![top.clone(), back.clone()], vec![(2, 7, 6)]),
        ("top + front", vec![top.clone(), front.clone()], vec![(1, 4, 4), (1, 4, 4)]),
        ("top + round + front", vec![top.clone(), round.clone(), front.clone()], vec![(3, 10, 8)]),
        ("all but the bottom", vec![top, round, front, back, left, right], vec![(6, 15, 10)]),
    ];
    let ids: Vec<_> = cases
        .iter()
        .map(|(_, picks, _)| {
            let picks = picks.clone();
            s.add("offsetSurface", move |p| {
                p.set("surfacesAndFaces", ParamValue::Query(picks));
                p.set("offset", expr("2 mm"));
            })
        })
        .collect();
    let r = s.ok();
    for ((name, _, want), f) in cases.into_iter().zip(ids) {
        let sheets: Vec<(usize, usize, usize)> = r
            .context
            .bodies
            .iter()
            .filter(|b| b.id.feature == f && b.kind == BodyKind::Surface)
            .map(|b| {
                let shown = b.shape.shown_topology().unwrap();
                (b.shape.faces().len(), shown.edges.iter().filter(|e| **e).count(), shown.vertices.iter().filter(|v| **v).count())
            })
            .collect();
        assert_eq!(sheets, want, "{name}");
    }
}

/// Offset sheets of one part's faces touching only at a corner are one surface (live c2-sm-hem-plus Offset surface 1:
/// a 100 mm box made sheet metal, 7 faces of its hems picked at live's points, 0.1 mm: one sheet 7 / 24 / 18).
#[test]
fn offset_surface_keeps_sheets_touching_at_a_corner_as_one() {
    let brep = include_str!("data/hem-box-sheet-metal.brep").to_string();
    let mut s = Studio::new();
    s.add("importForeign", move |p| p.set("blobData", ParamValue::Text(brep)));
    let r = s.ok();
    let b = &r.context.bodies[0];
    let faces = b.shape.faces();
    let picks: Vec<Reference> = [[40.25, -37.597, 2.929], [76.643, -64.233, -0.06], [-60.152, -76.243, 55.0], [-63.404, -76.604, 100.0], [-29.896, -40.185, 2.929], [44.795, -32.18, -4.142], [-70.114, -77.115, 55.0]]
        .iter()
        .map(|p| {
            let v = noshape_kernel::make_vertex(DVec3::from_array(*p) * 1e-3).unwrap();
            let fi = (0..faces.len()).min_by(|&x, &y| {
                let d = |i: usize| noshape_kernel::distance(&faces[i], &v).unwrap().distance;
                d(x).total_cmp(&d(y))
            });
            Reference::Face((*b.faces[fi.unwrap()]).clone())
        })
        .collect();
    let f = s.add("offsetSurface", move |p| {
        p.set("surfacesAndFaces", ParamValue::Query(picks));
        p.set("offset", expr("0.1 mm"));
    });
    let r = s.ok();
    let sheets: Vec<(usize, usize, usize)> = r
        .context
        .bodies
        .iter()
        .filter(|b| b.id.feature == f && b.kind == BodyKind::Surface)
        .map(|b| {
            let shown = b.shape.shown_topology().unwrap();
            (b.shape.faces().len(), shown.edges.iter().filter(|e| **e).count(), shown.vertices.iter().filter(|v| **v).count())
        })
        .collect();
    assert_eq!(sheets, [(7, 24, 18)]);
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

/// Bridging curve by std (bridgingCurve.fs): Match curvature meets each side's curvature, not only its tangent
/// (ours used to approximate curvature and flow with tangency, with a warning). Two arcs of radius 10 and 20 mm.
#[test]
fn bridging_curve_matches_curvature_as_std() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    // A quarter arc about the origin ending at (0, 10) mm going -x, and one about (60, 0) mm, radius 20, from (60, 20).
    let a1 = sk.add_arc_angles(DVec2::ZERO, 0.01, 0.0, std::f64::consts::FRAC_PI_2).0;
    let a2 = sk.add_arc_angles(DVec2::new(0.06, 0.0), 0.02, std::f64::consts::FRAC_PI_2, std::f64::consts::PI).0;
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let se = |e: u32| Reference::SketchEntity { sketch: sid, entity: e };
    for (m, deg) in [("TANGENCY", 3), ("CURVATURE", 5), ("G3", 7)] {
        let f = s.add("bridgingCurve", |p| {
            p.set("side1", ParamValue::Query(vec![se(a1)]));
            p.set("side2", ParamValue::Query(vec![se(a2)]));
            p.set("match1", ParamValue::Enum(m.into()));
            p.set("match2", ParamValue::Enum(m.into()));
        });
        let r = s.regen();
        let st = r.display.features.iter().find(|x| x.id == f).unwrap();
        assert!(st.message.is_none(), "{m}: {:?}", st.message);
        let body = r.context.bodies.iter().find(|b| b.id.feature == f).expect("the bridge");
        let e = &body.shape.edges()[0];
        let b = e.edge_bspline().unwrap();
        assert_eq!(b.degree, deg, "{m}: {b:?}");
        // Curvature at each end from the end poles: (n - 1) / n |d1 x d2| / |d1|^3.
        let n = b.degree as f64;
        let k = |p0: DVec3, p1: DVec3, p2: DVec3| (n - 1.0) / n * (p1 - p0).cross(p2 - p1).length() / (p1 - p0).length().powi(3);
        let p = &b.poles;
        let (k0, k1) = (k(p[0], p[1], p[2]), k(p[p.len() - 1], p[p.len() - 2], p[p.len() - 3]));
        let ends = [p[0], p[p.len() - 1]];
        // Which end is on which arc (std joins the arcs' nearest ends, (10, 0) and (40, 0) mm).
        let (ka, kb) = if (ends[0].length() - 0.01).abs() < 1e-9 { (k0, k1) } else { (k1, k0) };
        if m != "TANGENCY" {
            assert!((ka - 100.0).abs() < 1e-6 && (kb - 50.0).abs() < 1e-6, "{m}: {ka} {kb}");
        }
        s.ps.features.retain(|x| x.id != f);
    }
}

/// A bridging curve matching only position at both ends is a line, as live's (c2-hunting-knife's Bridging curve 1 between
/// two vertices: live LINE), not a B-spline of degree 1.
#[test]
fn a_bridging_curve_of_positions_is_a_line() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let a = sk.add_point(DVec2::new(0.0, 0.0));
    let b = sk.add_point(DVec2::new(0.03, 0.04));
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let se = |e: u32| Reference::SketchEntity { sketch: sid, entity: e };
    s.add("bridgingCurve", |p| {
        p.set("side1", ParamValue::Query(vec![se(a.0)]));
        p.set("side2", ParamValue::Query(vec![se(b.0)]));
        p.set("match1", ParamValue::Enum("POSITION".into()));
        p.set("match2", ParamValue::Enum("POSITION".into()));
    });
    let r = s.ok();
    let edges = r.context.bodies[0].shape.edges();
    assert_eq!(edges.len(), 1);
    let info = edges[0].edge_info().unwrap();
    assert!(matches!(info.curve, noshape_kernel::Curve::Line { .. }), "{:?}", info.curve);
    assert!(near(info.length, 0.05, 1e-12));
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

/// An intersection curve runs along n1 x n2 (live core q-manualblend-ic1): the block's section by a plane above its
/// base turns one way with the block first and the other way with the plane first.
#[test]
fn an_intersection_curve_runs_along_the_first_groups_normal_cross_the_seconds() {
    for body_first in [true, false] {
        let mut s = block();
        s.add("cPlane", |p| {
            p.set("entities", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
            p.set("offset", expr("10 mm"));
        });
        let plane = Reference::FeaturePlane(s.ps.features.last().unwrap().id);
        let body = Reference::Body(BodyId { feature: s.ps.features[1].id, index: 0 });
        let (g1, g2) = if body_first { (body, plane) } else { (plane, body) };
        s.add("intersectionCurve", move |p| {
            p.set("group1", ParamValue::Query(vec![g1]));
            p.set("group2", ParamValue::Query(vec![g2]));
        });
        let r = s.ok();
        let w = r.context.bodies.iter().position(|b| b.kind == BodyKind::Wire).unwrap();
        let ends: Vec<_> = r.context.bodies[w].shape.edges().iter().map(|e| e.endpoints().unwrap()).collect();
        let twice_area: f64 = ends.iter().map(|(a, c)| a.x * c.y - c.x * a.y).sum();
        let (lo, hi) = ends.iter().fold((f64::MAX, f64::MIN), |(lo, hi), (a, _)| (lo.min(a.x), hi.max(a.x)));
        let (ylo, yhi) = ends.iter().fold((f64::MAX, f64::MIN), |(lo, hi), (a, _)| (lo.min(a.y), hi.max(a.y)));
        // Body first: a side's outward normal x +Z, clockwise seen from above; plane first: anticlockwise.
        let want = if body_first { -2.0 } else { 2.0 } * (hi - lo) * (yhi - ylo);
        assert!(near(twice_area, want, 1e-9), "body first {body_first}: {twice_area} {ends:?}");
    }
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
    // Without "Import appearances": names and part colours, no face colours (live import-fidelity-1123, macmini-noapp).
    let mut s = Studio::new();
    s.add("importForeign", move |p| {
        p.set("blobData", ParamValue::Text(brep));
        p.set("partInfo", ParamValue::Text(json));
        p.set("importAppearances", ParamValue::Bool(false));
    });
    let r = s.ok();
    assert!(r.display.bodies.iter().any(|d| d.name == "Bracket"));
    assert!(r.display.bodies.iter().all(|d| d.face_colors.iter().all(Option::is_none)), "no face colours");
    assert!(r.display.bodies.iter().any(|d| d.color == [0.0, 0.0, 1.0, 1.0]), "the part keeps its colour");
}

/// A union of imported parts is its first tool (live psui step-colours-1122): that part's name, id and colour, every
/// face colour of both kept (split faces' pieces too), the other part's uncoloured faces in the result's colour.
#[test]
fn union_of_imported_parts_is_its_first_tool() {
    use noshape_model::features::importing::ImportPartInfo;
    let a = noshape_kernel::make_box(&noshape_kernel::Plane::xy(), 0.01, 0.01, 0.01).unwrap();
    let b = noshape_kernel::make_box_corners(DVec3::splat(0.005), DVec3::splat(0.015)).unwrap();
    let brep = String::from_utf8(noshape_kernel::Shape::compound(&[a, b]).to_brep_bytes().unwrap()).unwrap();
    let info = vec![
        ImportPartInfo { name: Some("Main".into()), color: Some([0.0, 1.0, 0.0]), opacity: None, faces: Vec::new() },
        ImportPartInfo { name: Some("Fan".into()), color: Some([0.0, 0.0, 1.0]), opacity: None, faces: vec![(0, [1.0, 0.0, 0.0])] },
    ];
    let json = serde_json::to_string(&info).unwrap();
    for (first, colour) in [("Main", [0.0, 1.0, 0.0, 1.0]), ("Fan", [0.0, 0.0, 1.0, 1.0])] {
        let mut s = Studio::new();
        let (b2, j2) = (brep.clone(), json.clone());
        s.add("importForeign", move |p| {
            p.set("blobData", ParamValue::Text(b2));
            p.set("partInfo", ParamValue::Text(j2));
        });
        let r = s.ok();
        let id = |name: &str| r.context.bodies.iter().find(|x| x.name == name).unwrap().id.clone();
        let (main, fan) = (id("Main"), id("Fan"));
        let tools = if first == "Main" { vec![main.clone(), fan] } else { vec![fan, main] };
        let kept = tools[0].clone();
        s.add("booleanBodies", move |p| {
            p.set("operationType", ParamValue::Enum("UNION".into()));
            p.set("tools", ParamValue::Query(tools.into_iter().map(Reference::Body).collect()));
        });
        let r = s.ok();
        assert_eq!(r.context.bodies.len(), 1);
        assert_eq!(r.context.bodies[0].id, kept);
        let d = &r.display.bodies[0];
        assert_eq!((d.name.as_str(), d.color), (first, colour));
        assert!(d.face_colors.contains(&Some([1.0, 0.0, 0.0, 1.0])), "{first}: Fan's red face kept");
        let others: Vec<_> = d.face_colors.iter().flatten().filter(|c| **c != [1.0, 0.0, 0.0, 1.0]).collect();
        assert!(others.is_empty(), "{first}: no other face colour {others:?}");
    }
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

/// Live dl-bracket-1125: an STL part is one face; Move face Offset offsets the whole mesh (1 mm on every side, no
/// error), Translate along the Top plane moves it whole (mesh-offset-1125) and Delete face removes the part.
#[test]
fn move_face_offsets_a_mesh_part_and_delete_face_removes_it() {
    let dir = std::env::temp_dir().join(format!("noshape-featb-meshmove-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("box.stl");
    let bx = noshape_kernel::make_box(&noshape_kernel::Plane::xy(), 0.01, 0.02, 0.03).unwrap();
    noshape_kernel::write_stl(&bx, &path, 1e-4, noshape_kernel::StlFormat::Binary).unwrap();
    let mesh = noshape_kernel::read_mesh_bodies(&path, noshape_kernel::LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_dir_all(dir);
    let brep = String::from_utf8(mesh.to_brep_bytes().unwrap()).unwrap();
    let imported = || {
        let mut s = Studio::new();
        let b = brep.clone();
        s.add("importForeign", move |p| p.set("blobData", ParamValue::Text(b)));
        let r = s.ok();
        (s, Reference::Face((*r.context.bodies[0].faces[0]).clone()), r.context.bodies[0].shape.bounding_box(false).unwrap())
    };
    let (mut s, face, before) = imported();
    s.add("moveFace", move |p| {
        p.set("moveFaces", ParamValue::Query(vec![face]));
        p.set("offsetDistance", expr("1 mm"));
    });
    let r = s.ok();
    let b = &r.context.bodies[0];
    assert!(b.shape.is_mesh() && b.faces.len() == 1);
    let bb = b.shape.bounding_box(false).unwrap();
    assert!((bb.min - before.min + DVec3::splat(1e-3)).length() < 1e-7 && (bb.max - before.max - DVec3::splat(1e-3)).length() < 1e-7, "{bb:?}");
    // Translate along the Top plane's normal moves the whole mesh.
    let (mut s, face, before) = imported();
    s.add("moveFace", move |p| {
        p.set("moveFaces", ParamValue::Query(vec![face]));
        p.set("moveFaceType", ParamValue::Enum("TRANSLATE".into()));
        p.set("direction", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        p.set("translationDistance", expr("5 mm"));
    });
    let r = s.ok();
    let b = &r.context.bodies[0];
    let bb = b.shape.bounding_box(false).unwrap();
    assert!(b.shape.is_mesh() && (bb.min - before.min - DVec3::new(0.0, 0.0, 5e-3)).length() < 1e-7 && (b.shape.volume().unwrap() - 6e-6).abs() < 1e-12, "{bb:?}");
    let (mut s, face, _) = imported();
    s.add("deleteFace", move |p| p.set("deleteFaces", ParamValue::Query(vec![face])));
    assert!(s.ok().context.bodies.is_empty());
    // A Point-Plane through one of its Mesh points (asm-scissor-lift 264eb3 Plane 2: "Point-Plane requires a point and
    // a plane." for a Mesh point).
    let (mut s, _, _) = imported();
    let body = s.ok().context.bodies[0].id.clone();
    s.add("cPlane", move |p| {
        p.set("cplaneType", ParamValue::Enum("PLANE_POINT".into()));
        p.set("entities", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top), Reference::mesh_point(body, [0.01, 0.02, 0.03])]));
    });
    let r = s.ok();
    let plane = r.context.planes.iter().find(|p| matches!(p.reference, Reference::FeaturePlane(_))).unwrap();
    assert!((plane.frame.origin.z - 0.03).abs() < 1e-9 && plane.frame.normal.z.abs() > 1.0 - 1e-12, "{:?}", plane.frame);
}

/// Live qa mistakes-1125 batch 3: Extrude Up to face on a mesh part's face stops at the mesh where it covers the
/// region (the new part MIXED: its end cap the mesh's); a region beside it fails "Failed to extrude selections, check
/// input.". A 20 mm STL cube at z 30..50, a 10 x 10 rectangle on Top under it and one beside it.
#[test]
fn extrude_up_to_a_mesh_face_stops_on_the_mesh() {
    let dir = std::env::temp_dir().join(format!("noshape-featb-meshupto-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("cube.stl");
    let bx = noshape_kernel::make_box(&noshape_kernel::Plane::xy(), 0.02, 0.02, 0.02).unwrap().translated(DVec3::new(0.0, 0.0, 0.03)).unwrap();
    noshape_kernel::write_stl(&bx, &path, 1e-4, noshape_kernel::StlFormat::Binary).unwrap();
    let mesh = noshape_kernel::read_mesh_bodies(&path, noshape_kernel::LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_dir_all(dir);
    let brep = String::from_utf8(mesh.to_brep_bytes().unwrap()).unwrap();
    let up_to = |x0: f64| {
        let mut s = Studio::new();
        let b = brep.clone();
        s.add("importForeign", move |p| p.set("blobData", ParamValue::Text(b)));
        let face = Reference::Face((*s.ok().context.bodies[0].faces[0]).clone());
        let mut sk = noshape_sketch::Sketch::new();
        rect(&mut sk, x0, 0.005, 0.01, 0.01);
        let id = s.sketch(Reference::Plane(DatumPlane::Top), sk);
        let e = s.extrude_largest(id, "1 mm");
        let f = s.ps.features.iter_mut().find(|f| f.id == e).unwrap();
        f.params.set("endBound", ParamValue::Enum("UP_TO_SURFACE".into()));
        f.params.set("endBoundEntityFace", ParamValue::Query(vec![face]));
        s.regen()
    };
    let r = up_to(0.005);
    let st = r.display.features.last().unwrap();
    assert_eq!(st.status, FeatureStatus::Ok, "{:?}", st.message);
    let b = r.context.bodies.iter().find(|b| (b.shape.volume().unwrap() - 3e-6).abs() < 1e-12).expect("the 10 x 10 x 30 mm part");
    assert!(b.shape.is_mesh() && b.faces.len() == 6, "MIXED, 6 faces: {}", b.faces.len());
    let bb = b.shape.bounding_box(false).unwrap();
    assert!((bb.max.z - 0.03).abs() < 1e-6 && bb.min.z.abs() < 1e-6, "{bb:?}");
    let r = up_to(0.03);
    let st = r.display.features.last().unwrap();
    assert_eq!((st.status, st.message.as_deref()), (FeatureStatus::Error, Some("Failed to extrude selections, check input.")));
}

/// An STL's open pieces are surfaces of their own, without a composite (live stl-split-1121: "Surface N" sheets; a mesh
/// that closes is a solid, DECISIONS 11-22).
#[test]
fn an_stl_s_open_pieces_are_surfaces() {
    let dir = std::env::temp_dir().join(format!("noshape-featb-stlopen-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("two.stl");
    let tri = |z: f64| format!("facet normal 0 0 1\nouter loop\nvertex 0 0 {z}\nvertex 1 0 {z}\nvertex 0 1 {z}\nendloop\nendfacet\n");
    std::fs::write(&path, format!("solid two\n{}{}endsolid two\n", tri(0.0), tri(1.0))).unwrap();
    let mesh = noshape_kernel::read_mesh_bodies(&path, noshape_kernel::LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_dir_all(dir);
    let brep = String::from_utf8(mesh.to_brep_bytes().unwrap()).unwrap();
    let mut s = Studio::new();
    s.add("importForeign", move |p| p.set("blobData", ParamValue::Text(brep)));
    let r = s.ok();
    let names: Vec<_> = r.display.bodies.iter().map(|b| (b.name.as_str(), b.kind, b.composite.is_some())).collect();
    assert_eq!(names, [("Surface 1", BodyKind::Surface, false), ("Surface 2", BodyKind::Surface, false)]);
}

/// A Mate connector on a mesh part's node (asm-corexy's studios on their STL imports; live: "Failed to resolve mate
/// connector origin." never, the node's point with the world axes, as a vertex's).
#[test]
fn a_mate_connector_on_a_mesh_node() {
    let dir = std::env::temp_dir().join(format!("noshape-featb-meshmc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("box.stl");
    let bx = noshape_kernel::make_box(&noshape_kernel::Plane::xy(), 0.01, 0.02, 0.03).unwrap();
    noshape_kernel::write_stl(&bx, &path, 1e-4, noshape_kernel::StlFormat::Binary).unwrap();
    let mesh = noshape_kernel::read_mesh_bodies(&path, noshape_kernel::LengthUnit::Meter).unwrap();
    let _ = std::fs::remove_dir_all(dir);
    let brep = String::from_utf8(mesh.to_brep_bytes().unwrap()).unwrap();
    let mut s = Studio::new();
    s.add("importForeign", move |p| p.set("blobData", ParamValue::Text(brep)));
    let part = s.ok().context.bodies[0].id.clone();
    let node = Reference::mesh_point(part.clone(), [0.01, 0.02, 0.03]);
    s.add("mateConnector", move |p| {
        p.set("originQuery", ParamValue::Query(vec![node]));
        p.set("entityInferenceType", ParamValue::Enum("POINT".into()));
        p.set("ownerPart", ParamValue::Query(vec![Reference::Body(part)]));
    });
    let r = s.ok();
    let c = &r.context.connectors[0];
    assert!(vnear(c.origin, DVec3::new(0.01, 0.02, 0.03)) && vnear(c.z, DVec3::Z) && vnear(c.x, DVec3::X), "{:?} {:?} {:?}", c.origin, c.z, c.x);
}

/// An Edge point on a sketch line (asm-infento-subs' studio 1dc32b: Transform 23 / 28's in-feature "Edge point"):
/// the point at the parameter along the line, on its sketch plane.
#[test]
fn an_edge_point_on_a_sketch_line() {
    let mut s = block();
    let mut sk = noshape_sketch::Sketch::new();
    let line = sk.add_line(DVec2::new(0.0, 0.0), DVec2::new(0.04, 0.02));
    let sketch = s.sketch(Reference::Plane(DatumPlane::Front), sk);
    s.add("cPoint", move |p| {
        p.set("entities", ParamValue::Query(vec![Reference::SketchEntity { sketch, entity: line.0 }]));
        p.set("parameter", expr("0.5"));
    });
    let r = s.ok();
    let p = r.context.points.last().expect("the point").position;
    assert!(vnear(p, DVec3::new(0.02, 0.0, 0.01)), "{p:?}");
}

/// Query variable › Adjacent as std's qAdjacent (live's probe, behaviours dx 11-04: a face's edge-adjacent edges are its
/// own; a vertex's vertex-adjacent edges end there; an edge's edge-adjacent faces are the two it bounds).
#[test]
fn query_variable_adjacent_as_std() {
    let mut s = block();
    let r = s.ok();
    let top = top_face(&r);
    let b = &r.context.bodies[0];
    let edge = Reference::Edge((*b.edges[0]).clone());
    let corner = Reference::Vertex((*b.vertices[0]).clone());
    let cases: [(&str, Reference, &str, &str, usize); 7] = [
        ("topFaces", top.clone(), "EDGE", "FACE", 4),
        ("topEdges", top.clone(), "EDGE", "EDGE", 4),
        ("topAll", top.clone(), "EDGE", "ALL", 8),
        ("topByVertex", top, "VERTEX", "FACE", 4),
        ("edgeFaces", edge, "EDGE", "FACE", 2),
        ("cornerEdges", corner.clone(), "VERTEX", "EDGE", 3),
        ("cornerFaces", corner, "VERTEX", "FACE", 3),
    ];
    for (name, seed, shared, filter, _) in cases.clone() {
        s.add("queryVariable", move |p| {
            p.set("name", ParamValue::Text(name.into()));
            p.set("selectionType", ParamValue::Enum("ADJACENT".into()));
            p.set("seedQuery", ParamValue::Query(vec![seed]));
            p.set("adjacencyType", ParamValue::Enum(shared.into()));
            p.set(if shared == "EDGE" { "edgeAdjacentEntityType" } else { "vertexAdjacentEntityType" }, ParamValue::Enum(filter.into()));
        });
    }
    let r = s.ok();
    for (name, _, _, _, n) in cases {
        assert_eq!(r.context.query_variables.get(name).map(|v| v.len()), Some(n), "{name}");
    }
    assert!(r.context.query_variables["topFaces"].iter().all(|q| matches!(q, Reference::Face(_))));
}

/// Query variable › Fillets and Matching as live (psui 11-17 12:21): the seed's body only; a fillet's radius compared
/// with every fillet of the body, not only the tangent-connected ones; Matching the faces of the seed's size and
/// shape. The block's four vertical edges rounded r 3, r 3, r 5 and r 2.
#[test]
fn query_variable_fillets_and_matching_as_live() {
    let mut s = block();
    let r = s.ok();
    let b = &r.context.bodies[0];
    let vertical: Vec<Reference> = b.shape.edges().iter().enumerate().filter(|(_, e)| e.endpoints().is_ok_and(|(a, c)| (a.x - c.x).abs() < 1e-9 && (a.y - c.y).abs() < 1e-9)).map(|(i, _)| Reference::Edge((*b.edges[i]).clone())).collect();
    assert_eq!(vertical.len(), 4);
    for (edge, radius) in vertical.into_iter().zip(["3 mm", "3 mm", "5 mm", "2 mm"]) {
        s.add("fillet", move |p| {
            p.set("entities", ParamValue::Query(vec![edge]));
            p.set("radius", expr(radius));
        });
    }
    let r = s.ok();
    let b = &r.context.bodies[0];
    let r3 = b.shape.faces().iter().position(|f| matches!(f.face_surface(), Ok(noshape_kernel::Surface::Cylinder { radius, .. }) if near(radius, 0.003, 1e-9))).expect("an r 3 face");
    let seed = Reference::Face((*b.faces[r3]).clone());
    let cases: [(&str, &str, &str, usize); 4] = [("eq", "FILLETS", "EQUAL", 2), ("le", "FILLETS", "LESS_EQUAL", 3), ("ge", "FILLETS", "GREATER_EQUAL", 3), ("same", "MATCHING", "", 2)];
    for (name, kind, compare, _) in cases {
        let seed = seed.clone();
        s.add("queryVariable", move |p| {
            p.set("name", ParamValue::Text(name.into()));
            p.set("selectionType", ParamValue::Enum(kind.into()));
            p.set("seedType", ParamValue::Enum("FACE".into()));
            p.set("seedFaces", ParamValue::Query(vec![seed]));
            if !compare.is_empty() {
                p.set("filletCompareType", ParamValue::Enum(compare.into()));
            }
        });
    }
    let r = s.ok();
    for (name, _, _, n) in cases {
        assert_eq!(r.context.query_variables.get(name).map(|v| v.len()), Some(n), "{name}");
    }
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
    let top = top_face(&r);
    let named = top.clone();
    s.add("nameEntity", move |p| p.set("entity", ParamValue::Query(vec![named])));
    let pt = Reference::Body(BodyId { feature: s.ps.features[2].id, index: 0 });
    s.add("compositePart", move |p| p.set("bodies", ParamValue::Query(vec![body, pt])));
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

/// Projected curve, two sketches (c2-projected-curve-enclose): an ellipse on Top (50 × 30 mm radii, major along y)
/// and an arc on Right ending at y ±50 on Top: live's curve is two edges of 135.5088 mm, split only where it meets the
/// arc's prism's ends (OCCT's section came in four pieces).
#[test]
fn a_two_sketch_projection_is_one_edge_between_the_profiles_ends() {
    let mut s = Studio::new();
    let mut a = noshape_sketch::Sketch::new();
    let arc = a.add_arc_angles(DVec2::new(0.0, -0.051177438302144385), 0.07154809704785862, 0.7970349983821571, 2.344557655207636);
    let sa = s.sketch(Reference::Plane(DatumPlane::Right), a);
    let mut b = noshape_sketch::Sketch::new();
    let ell = b.add_ellipse(DVec2::ZERO, DVec2::new(0.0, 0.05), 0.03);
    let sb = s.sketch(Reference::Plane(DatumPlane::Top), b);
    s.add("projectCurves", move |p| {
        p.set("curveProjectionType", ParamValue::Enum("TWO_SKETCHES".into()));
        p.set("sketchEdges1", ParamValue::Query(vec![Reference::SketchEntity { sketch: sa, entity: arc.0 }]));
        p.set("sketchEdges2", ParamValue::Query(vec![Reference::SketchEntity { sketch: sb, entity: ell.0 }]));
    });
    let r = s.ok();
    let curve = r.context.bodies.iter().find(|b| b.kind == BodyKind::Wire).expect("the curve");
    let lengths: Vec<f64> = curve.shape.edges().iter().map(|e| e.length().unwrap() * 1e3).collect();
    assert_eq!(lengths.len(), 2, "{lengths:?}");
    assert!(lengths.iter().all(|l| (l - 135.5088).abs() < 1e-3), "{lengths:?}");
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
        // (One turn: Onshape's helix has 80 control points a turn, and Edit curve takes 100 at most.)
        p.set("revolutions", expr("1"));
    });
    s.add("editCurve", move |p| {
        p.set("wire", ParamValue::Query(vec![Reference::Body(BodyId { feature: helix, index: 0 })]));
        p.set("planarize", ParamValue::Bool(true));
        p.set("planeReference", ParamValue::Enum("XYPLANE".into()));
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
    // Onshape lists the bent polyline's lines before its arcs (live "fs (10-25 d)").
    let kinds: Vec<bool> = r.context.bodies[wires[0]].shape.edges().iter().map(|e| matches!(e.edge_info().unwrap().curve, noshape_kernel::Curve::Line { .. })).collect();
    assert_eq!(kinds, [true, true, false], "routing edges: lines, then the arc");
    let bb = r.context.bodies[wires[1]].shape.bounding_box(true).unwrap();
    assert!(bb.max.z.abs() < 1e-6 && bb.min.z.abs() < 1e-6, "planarized helix {bb:?}");
    for &w in &wires[2..] {
        let l = wire_length(&r, w);
        assert!(near(l, 0.1, 1e-6) || near(l, 0.05, 1e-6), "iso length {l}");
    }
}

/// Live (routing-curve-new-edit-reedit): an interpolated Routing curve is std's opFitSpline, centripetal knots and
/// natural ends: through (40, 30, 10), (0, 30, 10), (40, 0, 10) mm 91.693 mm, closed 134.227 mm (OCCT's chord-length
/// interpolation: 91.903).
#[test]
fn an_interpolated_routing_curve_is_centripetal_with_natural_ends() {
    for (closed, length) in [(false, 0.091693), (true, 0.134227)] {
        let mut s = Studio::new();
        s.add("routingCurve", move |p| {
            let pt = |x: &str, y: &str| {
                let mut v = ParamValues::default();
                v.set("referenceType", ParamValue::Enum("ORIGIN".into()));
                v.set("dx", expr(x));
                v.set("dy", expr(y));
                v.set("dz", expr("10 mm"));
                v
            };
            p.set("curveType", ParamValue::Enum("INTERPOLATED_SPLINE".into()));
            p.set("isPeriodic", ParamValue::Bool(closed));
            p.set("points", ParamValue::Array(vec![pt("40 mm", "30 mm"), pt("0 mm", "30 mm"), pt("40 mm", "0 mm")]));
        });
        let r = s.ok();
        let w = (0..r.context.bodies.len()).find(|&i| r.context.bodies[i].kind == BodyKind::Wire).unwrap();
        let l = wire_length(&r, w);
        assert!(near(l, length, 5e-7), "closed {closed}: {l}");
    }
}

/// Offset curve on a trapezoid's top face (live 1813): its front edge 5 mm in (Flip on; Flip off goes off the face:
/// live's warning, nothing made). Sides leaning out: Offset 0 .. 40 (1 edge), Offset and extend -2.5 .. 42.5 (3 edges),
/// Trim 3 mm 0.5 .. 42.5 (2 edges) with std's arrows, Offset, extend and split the top face in two; sides leaning in: 2.5 .. 37.5 (1 edge) for both scopes.
#[test]
fn offset_curve_extends_to_the_face_boundary() {
    let trapezoid = |pts: [(f64, f64); 4]| {
        let mut s = Studio::new();
        let mut sk = noshape_sketch::Sketch::new();
        let p: Vec<_> = pts.iter().map(|&(x, y)| sk.add_point(DVec2::new(x, y))).collect();
        for i in 0..4 {
            sk.add_line_between(p[i], p[(i + 1) % 4]).unwrap();
        }
        let id = s.sketch(Reference::Plane(DatumPlane::Top), sk);
        s.extrude_largest(id, "10 mm");
        let r = s.ok();
        let b = &r.context.bodies[0];
        let e = b.shape.edges().iter().position(|e| e.endpoints().is_ok_and(|(a, c)| near(a.z, 0.01, 1e-9) && near(c.z, 0.01, 1e-9) && a.y.abs() < 1e-9 && c.y.abs() < 1e-9)).unwrap();
        let edge = Reference::Edge((*b.edges[e]).clone());
        let top = top_face(&r);
        (s, edge, top)
    };
    let add = |s: &mut Studio, edge: &Reference, top: &Reference, scope: &'static str, flip: bool, trim: bool| {
        let (edge, top) = (edge.clone(), top.clone());
        s.add("offsetCurveOnFace", move |p| {
            p.set("edges", ParamValue::Query(vec![edge]));
            p.set("targets", ParamValue::Query(vec![top]));
            p.set("distance", expr("5 mm"));
            p.set("oppositeDirection", ParamValue::Bool(flip));
            p.set("scope", ParamValue::Enum(scope.into()));
            p.set("trim", ParamValue::Bool(trim));
            p.set("startTrim", expr("3 mm"));
        })
    };
    // (x min, x max, edges) of the offset's wire body.
    let result = |r: &RegenResult, f: FeatureId| {
        let w = r.context.bodies.iter().find(|b| b.id.feature == f).expect("the offset");
        let xs: Vec<f64> = w.shape.vertices().iter().filter_map(|v| v.vertex_point().ok()).inspect(|q| assert!(near(q.y, 0.005, 1e-9), "{q:?}")).map(|q| q.x).collect();
        (xs.iter().cloned().fold(f64::INFINITY, f64::min), xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max), w.shape.edges().len())
    };
    let (mut s, edge, top) = trapezoid([(0.0, 0.0), (0.04, 0.0), (0.05, 0.02), (-0.01, 0.02)]);
    let f = add(&mut s, &edge, &top, "OFFSET", false, false);
    let r = s.regen();
    let st = r.display.features.iter().find(|x| x.id == f).unwrap();
    assert!(st.status == FeatureStatus::Warning && st.message.as_deref() == Some("The highlighted chains did not produce any offset with the given parameters."), "{:?} {:?}", st.status, st.message);
    assert!(!r.context.bodies.iter().any(|b| b.id.feature == f));
    s.ps.features.retain(|x| x.id != f);
    let mut got = Vec::new();
    let mut arrows = Vec::new();
    for (scope, trim) in [("OFFSET", false), ("OFFSET_AND_EXTEND", false), ("OFFSET_AND_EXTEND", true)] {
        let f = add(&mut s, &edge, &top, scope, true, trim);
        let r = s.ok();
        got.push(result(&r, f));
        arrows.push(r.display.features.iter().find(|x| x.id == f).unwrap().manipulators.clone());
        if trim {
            // Dragging the start arrow 2 mm on (std's offsetCurveOnFaceManipulators): Start trim 5 mm.
            let ft = s.ps.features.iter().find(|x| x.id == f).unwrap().clone();
            let at = s.ps.features.iter().position(|x| x.id == f).unwrap();
            let ctx = s.reg.regenerate(&s.ps, &ConfigValues::default(), &s.units, &RegenOptions { upto: Some(at), ..Default::default() }).context;
            let (key, mut moved) = arrows[2][0].clone();
            if let noshape_model::display::FeatureManipulator::Linear { offset, .. } = &mut moved {
                *offset = 0.002;
            }
            let changed = noshape_model::custom::std_feature::manipulator_change_in((*ctx).clone(), "offsetCurveOnFace", ft.id, &ft.name, &s.units, &ft.params, &key, &moved).expect("std's change function ran");
            let st = changed.get("startTrim").and_then(|v| if let ParamValue::Expr(x) = v { x.cached.map(|q| q.value) } else { None });
            assert!(st.is_some_and(|x| near(x, 0.005, 1e-12)), "{changed:?}");
        }
        s.ps.features.retain(|x| x.id != f);
    }
    let same = |a: (f64, f64, usize), b: (f64, f64, usize)| near(a.0, b.0, 1e-9) && near(a.1, b.1, 1e-9) && a.2 == b.2;
    assert!(same(got[0], (0.0, 0.04, 1)) && same(got[1], (-0.0025, 0.0425, 3)), "{got:?}");
    // Trim, start 3 mm (measured from the extension's start): std's arrows at the cuts along the curve.
    assert!(same(got[2], (0.0005, 0.0425, 2)), "{got:?}");
    use noshape_model::display::FeatureManipulator as M;
    let keys: Vec<(&str, DVec3)> = arrows[2].iter().filter_map(|(k, m)| if let M::Linear { base, .. } = m { Some((k.as_str(), *base)) } else { None }).collect();
    assert!(keys.len() == 2 && keys[0].0 == "startTrimManipulator" && vnear(keys[0].1, DVec3::new(0.0005, 0.005, 0.01)) && vnear(keys[1].1, DVec3::new(0.0425, 0.005, 0.01)), "{keys:?}");
    // Offset, extend and split: no wire body, the top face split 212.5 + 787.5 mm² (6 -> 7 faces).
    let f = add(&mut s, &edge, &top, "OFFSET_EXTEND_AND_SPLIT", true, false);
    let r = s.ok();
    assert!(!r.context.bodies.iter().any(|b| b.id.feature == f) && r.context.bodies.len() == 1);
    let faces = r.context.bodies[0].shape.faces();
    let mut tops: Vec<f64> = faces.iter().filter(|x| x.centroid().is_ok_and(|c| near(c.z, 0.01, 1e-9))).map(|x| x.area().unwrap() * 1e6).collect();
    tops.sort_by(f64::total_cmp);
    assert!(faces.len() == 7 && tops.len() == 2 && near(tops[0], 212.5, 1e-6) && near(tops[1], 787.5, 1e-6), "{} {tops:?}", faces.len());
    s.ps.features.retain(|x| x.id != f);

    let (mut s, edge, top) = trapezoid([(0.0, 0.0), (0.04, 0.0), (0.03, 0.02), (0.01, 0.02)]);
    for scope in ["OFFSET", "OFFSET_AND_EXTEND"] {
        let f = add(&mut s, &edge, &top, scope, true, false);
        let r = s.ok();
        assert!(same(result(&r, f), (0.0025, 0.0375, 1)), "{scope}: {:?}", result(&r, f));
        s.ps.features.retain(|x| x.id != f);
    }
}

/// Offset curve on start pin (live 2538, psui's offset-curve scenarios): a Ø10 × 30 pin's top edge, no targets.
/// Geodesic 2 mm runs down the side (the circle at z 28), 25 mm too (z 5), Flip onto the top face (Ø6 at z 30);
/// Euclidean 25 mm fails in live, its Edges field marked; Offset, extend and split cuts the side in two.
#[test]
fn offset_curve_runs_down_a_cylinder() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(DVec2::ZERO, 0.005);
    let id = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    s.extrude_largest(id, "30 mm");
    let r = s.ok();
    let b = &r.context.bodies[0];
    let e = b.shape.edges().iter().position(|e| e.bounding_box(true).is_ok_and(|bb| near(bb.min.z, 0.03, 1e-9) && near(bb.max.z, 0.03, 1e-9))).unwrap();
    let edge = Reference::Edge((*b.edges[e]).clone());
    let add = |s: &mut Studio, d: &'static str, flip: bool, euclidean: bool, scope: &'static str| {
        let edge = edge.clone();
        s.add("offsetCurveOnFace", move |p| {
            p.set("edges", ParamValue::Query(vec![edge]));
            p.set("distance", expr(d));
            p.set("oppositeDirection", ParamValue::Bool(flip));
            p.set("offsetType", ParamValue::Enum(if euclidean { "EUCLIDEAN" } else { "GEODESIC" }.into()));
            p.set("scope", ParamValue::Enum(scope.into()));
        })
    };
    for (d, flip, z, len) in [("2 mm", false, 0.028, 0.0314159), ("25 mm", false, 0.005, 0.0314159), ("2 mm", true, 0.03, 0.0188496)] {
        let f = add(&mut s, d, flip, false, "OFFSET");
        let r = s.ok();
        let w = r.context.bodies.iter().position(|b| b.id.feature == f).expect("the offset");
        let c = r.context.bodies[w].shape.bounding_box(true).unwrap().center();
        assert!(near(c.z, z, 1e-7) && c.truncate().length() < 1e-7 && near(wire_length(&r, w), len, 1e-6), "{d} {flip}: {c:?} {}", wire_length(&r, w));
        s.ps.features.retain(|x| x.id != f);
    }
    let f = add(&mut s, "25 mm", false, true, "OFFSET");
    let r = s.regen();
    let st = r.display.features.iter().find(|x| x.id == f).unwrap();
    assert!(st.status == FeatureStatus::Error && st.message.as_deref() == Some("Could not compute offset with given parameters."), "{:?} {:?}", st.status, st.message);
    // Live marks the Edges field (psui's offset-curve-euclidean-round).
    assert_eq!(st.faulty, vec!["edges".to_string()]);
    s.ps.features.retain(|x| x.id != f);
    let f = add(&mut s, "25 mm", false, false, "OFFSET_EXTEND_AND_SPLIT");
    let r = s.ok();
    assert!(!r.context.bodies.iter().any(|b| b.id.feature == f) && r.context.bodies.len() == 1);
    let mut sides: Vec<f64> = r.context.bodies[0].shape.faces().iter().filter(|x| x.face_plane().is_ok_and(|p| p.is_none())).map(|x| x.area().unwrap() * 1e6).collect();
    sides.sort_by(f64::total_cmp);
    assert!(sides.len() == 2 && near(sides[0], std::f64::consts::PI * 10.0 * 5.0, 1e-4) && near(sides[1], std::f64::consts::PI * 10.0 * 25.0, 1e-4), "{sides:?}");
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
        p.set("oppositeDirection", ParamValue::Bool(true));
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
    // std keeps each surface's pieces behind the other (qSplitBy's back body): Front's behind the x = 20 mm fill
    // (normal +x), x in [0, 0.02] (0.02 x 0.03); the other's behind Front (normal -y), y in [0, 0.03] (0.03 x 0.03).
    let bb = r.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!(near(a, 0.02 * 0.03 + 0.03 * 0.03, 1e-8), "trimmed area {a} {bb:?}");
    assert!(near(bb.max.x, 0.02, 1e-7) && near(bb.min.y, 0.0, 1e-7) && near(bb.max.y, 0.03, 1e-7), "{bb:?}");

    // Sets (std mutualTrim.fs "First set of surfaces"): two parallel fills
    // against one crossing fill, not merged: both of the first set keep their
    // side behind it (x in [0, 0.02]), the crossing one is cut by both.
    let mut s = Studio::new();
    let front = |s: &mut Studio, plane: Reference| {
        let mut a = noshape_sketch::Sketch::new();
        let lines = rect(&mut a, 0.0, 0.0, 0.05, 0.03);
        let sk = s.sketch(plane, a);
        (sk, lines)
    };
    let fa = front(&mut s, Reference::Plane(DatumPlane::Front));
    s.add("cPlane", |p| {
        p.set("entities", ParamValue::Query(vec![Reference::Plane(DatumPlane::Front)]));
        p.set("offset", expr("10 mm"));
    });
    let fpl = s.ps.features.last().unwrap().id;
    let fb = front(&mut s, Reference::FeaturePlane(fpl));
    s.add("cPlane", |p| {
        p.set("entities", ParamValue::Query(vec![Reference::Plane(DatumPlane::Right)]));
        p.set("offset", expr("20 mm"));
    });
    let pl = s.ps.features.last().unwrap().id;
    let mut c = noshape_sketch::Sketch::new();
    let lc = rect(&mut c, -0.02, 0.0, 0.04, 0.03);
    let sc = s.sketch(Reference::FeaturePlane(pl), c);
    let mut fills = Vec::new();
    for (sk, lines) in [fa, fb, (sc, lc)] {
        fills.push(s.add("fill", move |p| {
            let mut row = ParamValues::default();
            row.set("entities", ParamValue::Query(lines.iter().map(|&e| Reference::SketchEntity { sketch: sk, entity: e }).collect()));
            p.set("edges", ParamValue::Array(vec![row]));
        }));
    }
    s.add("mutualTrim", move |p| {
        let body = |f| Reference::Body(BodyId { feature: f, index: 0 });
        p.set("body1", ParamValue::Query(vec![body(fills[0]), body(fills[1])]));
        p.set("body2", ParamValue::Query(vec![body(fills[2])]));
        p.set("merge", ParamValue::Bool(false));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 3);
    let areas: Vec<f64> = (0..3).map(|i| surface_area(&r, i)).collect();
    assert_eq!(areas.iter().filter(|a| near(**a, 0.02 * 0.03, 1e-8)).count(), 2, "first set trimmed to 20 x 30 mm each: {areas:?}");
}

/// Split part by the part's own planar face, Use trimmed off (c2-body-draft-naming's Split 2 at a box's top under a
/// boss): the cut and the face it came from are one face, as live's (the box 6/12/8, not the boss's footprint apart).
#[test]
fn a_split_by_its_own_top_face_leaves_the_box_whole() {
    let block = noshape_kernel::make_box_corners(DVec3::new(0.0, 0.0, 0.0), DVec3::new(0.04, 0.03, 0.01)).unwrap();
    let boss = noshape_kernel::make_box_corners(DVec3::new(0.015, 0.01, 0.01), DVec3::new(0.025, 0.02, 0.02)).unwrap();
    let fused = noshape_kernel::boolean(noshape_kernel::BooleanOp::Fuse, &[block], &[boss], &Default::default()).unwrap();
    let part = noshape_kernel::unify_same_domain(&fused.shape).unwrap().shape;
    let brep = String::from_utf8(part.to_brep_bytes().unwrap()).unwrap();
    let mut s = Studio::new();
    s.add("importForeign", move |p| p.set("blobData", ParamValue::Text(brep)));
    let r = s.ok();
    let b = &r.context.bodies[0];
    let top = |f: &noshape_kernel::Shape| f.face_plane().ok().flatten().is_some_and(|pl| pl.normal.z > 0.5 && (pl.origin.z - 0.01).abs() < 1e-9);
    let fi = b.shape.faces().iter().position(top).expect("the box's top");
    let (id, face) = (b.id.clone(), Reference::Face((*b.faces[fi]).clone()));
    s.add("splitPart", move |p| {
        p.set("targets", ParamValue::Query(vec![Reference::Body(id)]));
        p.set("tool", ParamValue::Query(vec![face]));
        p.set("keepTools", ParamValue::Bool(true));
        p.set("useTrimmed", ParamValue::Bool(false));
    });
    let r = s.ok();
    let mut parts: Vec<(usize, usize, f64)> = r.context.bodies.iter().map(|b| (b.shape.faces().len(), b.shape.edges().len(), b.shape.volume().unwrap() * 1e9)).collect();
    parts.sort_by(|a, b| a.2.total_cmp(&b.2));
    assert_eq!(parts.len(), 2, "{parts:?}");
    assert_eq!((parts[0].0, parts[0].1), (6, 12), "the boss: {parts:?}");
    assert_eq!((parts[1].0, parts[1].1), (6, 12), "the box: {parts:?}");
    assert!((parts[0].2 - 1000.0).abs() < 1e-3 && (parts[1].2 - 12000.0).abs() < 1e-3, "{parts:?}");
}

/// Split part by a trimmed face that reaches only part way through the part (psui's ui-parity
/// mistake-split-trim-to-face-partial): nothing splits, only a slit could be cut, so the feature fails as live's ("Tool
/// entity cannot split the selected part/face."); a face that misses the part is live's info.
#[test]
fn a_trimmed_face_part_way_through_a_part_cannot_split_it() {
    for (y0, want) in [(-0.005, "Tool entity cannot split the selected part/face."), (0.04, "Selected entity does not split selected parts.")] {
        let block = noshape_kernel::make_box_corners(DVec3::new(0.0, 0.0, 0.0), DVec3::new(0.04, 0.03, 0.01)).unwrap();
        let tool = noshape_kernel::make_box_corners(DVec3::new(0.02, y0, -0.005), DVec3::new(0.03, y0 + 0.02, 0.015)).unwrap();
        let mut s = Studio::new();
        for b in [block, tool] {
            let brep = String::from_utf8(b.to_brep_bytes().unwrap()).unwrap();
            s.add("importForeign", move |p| p.set("blobData", ParamValue::Text(brep)));
        }
        let r = s.ok();
        let (part, t) = (&r.context.bodies[0], &r.context.bodies[1]);
        let fi = t.shape.faces().iter().position(|f| f.face_plane().ok().flatten().is_some_and(|pl| pl.normal.x.abs() > 0.5 && (pl.origin.x - 0.02).abs() < 1e-9)).expect("the tool's face at x 20 mm");
        let (id, face) = (part.id.clone(), Reference::Face((*t.faces[fi]).clone()));
        s.add("splitPart", move |p| {
            p.set("targets", ParamValue::Query(vec![Reference::Body(id)]));
            p.set("tool", ParamValue::Query(vec![face]));
            p.set("useTrimmed", ParamValue::Bool(true));
        });
        let r = s.regen();
        let st = r.display.features.last().unwrap();
        assert_eq!(st.message.as_deref(), Some(want), "{:?}", st.status);
        assert_eq!(st.status == FeatureStatus::Error, y0 < 0.0, "{:?}", st.status);
    }
}

/// Mesh parts in booleans and splits (live G35/G35-retry): a split by a plane
/// Split part by a cylinder face, Use trimmed off: the cylinder's whole surface splits (c2-rib-wrap's Split 1 by
/// the part's own round lobe): the disc it closes off is a part of 3 faces (live 3/2/0), the rest the other.
#[test]
fn an_untrimmed_cylinder_face_splits_off_its_whole_disc() {
    let block = noshape_kernel::make_box_corners(DVec3::new(-0.02, 0.0, 0.0), DVec3::new(0.02, 0.03, 0.01)).unwrap();
    let lobe = noshape_kernel::make_cylinder(&noshape_kernel::Plane::new(DVec3::ZERO, DVec3::Z, DVec3::X), 0.01, 0.01, std::f64::consts::TAU).unwrap();
    let fused = noshape_kernel::boolean(noshape_kernel::BooleanOp::Fuse, &[block], &[lobe], &Default::default()).unwrap();
    let part = noshape_kernel::unify_same_domain(&fused.shape).unwrap().shape;
    let brep = String::from_utf8(part.to_brep_bytes().unwrap()).unwrap();
    let mut s = Studio::new();
    s.add("importForeign", move |p| p.set("blobData", ParamValue::Text(brep)));
    let r = s.ok();
    let b = &r.context.bodies[0];
    let fi = b.shape.faces().iter().position(|f| matches!(f.face_surface(), Ok(noshape_kernel::Surface::Cylinder { .. }))).expect("the lobe");
    let (id, face) = (b.id.clone(), Reference::Face((*b.faces[fi]).clone()));
    s.add("splitPart", move |p| {
        p.set("targets", ParamValue::Query(vec![Reference::Body(id)]));
        p.set("tool", ParamValue::Query(vec![face]));
        p.set("useTrimmed", ParamValue::Bool(false));
    });
    let r = s.ok();
    let mut parts: Vec<(usize, f64)> = r.context.bodies.iter().map(|b| (b.shape.faces().len(), b.shape.volume().unwrap() * 1e9)).collect();
    parts.sort_by(|a, b| a.1.total_cmp(&b.1));
    let disc = std::f64::consts::PI * 100.0 * 10.0;
    assert_eq!(parts.len(), 2, "{parts:?}");
    assert_eq!(parts[0].0, 3, "{parts:?}");
    assert!((parts[0].1 - disc).abs() < 1e-3 && (parts[1].1 - (12000.0 - disc / 2.0)).abs() < 1e-3, "{parts:?}");
}

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

/// Two U profiles meeting at a point and one V profile across their other ends: a three-sided sheet, 1/3/3 and valid
/// (live c2-surfaces-lofts-projected's Boundary surface 1: 1/3/3, 259.97 mm2; it was a fill of the three, invalid).
#[test]
fn boundary_of_two_u_meeting_and_one_v_is_three_sided() {
    let mut s = Studio::new();
    let mut one = |a: (f64, f64), b: (f64, f64)| {
        let mut sk = noshape_sketch::Sketch::new();
        let l = sk.add_line(DVec2::new(a.0, a.1), DVec2::new(b.0, b.1)).0;
        (s.sketch(Reference::Plane(DatumPlane::Top), sk), l)
    };
    let us = [one((0.0, 0.0), (0.04, 0.0)), one((0.0, 0.0), (0.0, 0.02))];
    let vs = [one((0.04, 0.0), (0.0, 0.02))];
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
    let r = s.ok();
    let i = r.context.bodies.iter().position(|b| b.kind == BodyKind::Surface).unwrap();
    let sh = &r.context.bodies[i].shape;
    assert!(sh.is_valid());
    let t = sh.shown_topology().unwrap();
    assert_eq!((sh.faces().len(), t.edge_count(), t.vertices.iter().filter(|v| **v).count()), (1, 3, 3));
    assert!(near(surface_area(&r, i), 0.04 * 0.02 / 2.0, 1e-9), "{}", surface_area(&r, i));
}

/// Live (suite truth 11-18, c2-surf-workshop-bezier's "Boundary surface - Bézier"): two U profiles without conditions
/// are ruled to each other point for point along their curves (straight across), 5516.5878 mm2; a Bézier of degree 5
/// on Front, a spline through 4 points on Top.
#[test]
fn boundary_of_two_profiles_is_ruled_between_them() {
    let mut s = Studio::new();
    let mm = |p: &[(f64, f64)]| p.iter().map(|q| DVec2::new(q.0, q.1) * 1e-3).collect::<Vec<_>>();
    let mut a = noshape_sketch::Sketch::new();
    let ea = a.add_bspline(&mm(&[(30.19292, 25.49129), (36.33276, 62.92071), (9.93773, 50.09567), (-18.01856, 46.43286), (-36.1306, 42.5698), (-46.2864, 21.68969)]), 5).unwrap();
    let sa = s.sketch(Reference::Plane(DatumPlane::Front), a);
    let mut b = noshape_sketch::Sketch::new();
    let eb = b.add_fit_spline(&mm(&[(28.34795, 30.06134), (4.53833, 46.00168), (-20.68373, 53.46741), (-50.9502, 46.20346)])).unwrap();
    let sb = s.sketch(Reference::Plane(DatumPlane::Top), b);
    s.add("boundarySurface", move |p| {
        let rows = [(sa, ea), (sb, eb)]
            .iter()
            .map(|(sk, e)| {
                let mut v = ParamValues::default();
                v.set("uProfileEntities", ParamValue::Query(vec![Reference::SketchEntity { sketch: *sk, entity: e.0 }]));
                v.set("uCondition", ParamValue::Enum("DEFAULT".into()));
                v
            })
            .collect();
        p.set("uProfilesArray", ParamValue::Array(rows));
    });
    let r = s.ok();
    let i = r.context.bodies.iter().position(|b| b.kind == BodyKind::Surface).unwrap();
    let area = surface_area(&r, i) * 1e6;
    assert!((area - 5516.5878).abs() < 0.01, "{area}");
}

/// A surface Extrude of one sketch curve (blind, symmetric when `both`), Add or New.
fn sheet_of(s: &mut Studio, plane: DatumPlane, a: (f64, f64), b: (f64, f64), depth: &'static str, both: bool, add: bool) {
    let mut sk = noshape_sketch::Sketch::new();
    let e = sk.add_line(DVec2::new(a.0, a.1), DVec2::new(b.0, b.1)).0;
    let sk = s.sketch(Reference::Plane(plane), sk);
    s.add("extrude", move |p| {
        p.set("bodyType", ParamValue::Enum("SURFACE".into()));
        p.set("surfaceOperationType", ParamValue::Enum(if add { "ADD" } else { "NEW" }.into()));
        p.set("surfaceEntities", ParamValue::Query(vec![Reference::SketchEntity { sketch: sk, entity: e }]));
        p.set("depth", expr(depth));
        p.set("symmetric", ParamValue::Bool(both));
    });
}

/// A surface Extrude with Add joins the surface it meets (c2-surf-workshop-split's Extrude 2 up to Surface 1's hole: live
/// one sheet of 3 faces, ours a second sheet).
#[test]
fn surface_extrude_add_joins_the_surface_it_meets() {
    let mut s = Studio::new();
    sheet_of(&mut s, DatumPlane::Top, (0.0, 0.0), (0.05, 0.0), "10 mm", false, false);
    sheet_of(&mut s, DatumPlane::Top, (0.05, 0.0), (0.05, 0.05), "10 mm", false, true);
    let r = s.ok();
    let sheets: Vec<usize> = r.context.bodies.iter().filter(|b| b.kind == BodyKind::Surface).map(|b| b.shape.faces().len()).collect();
    assert_eq!(sheets, [2]);
}

/// Split of a sheet by a tube through its middle keeps one side: the piece with the hole on one, the disc on the other
/// (c2-surf-workshop-split's Split 2: both pieces' centres lie inside the tube, so both went with Keep front).
#[test]
fn split_of_a_sheet_by_a_tube_keeps_one_side() {
    let run = |front: bool| {
        let mut s = Studio::new();
        sheet_of(&mut s, DatumPlane::Front, (-0.05, 0.01), (0.05, 0.01), "100 mm", true, false);
        let mut sk = noshape_sketch::Sketch::new();
        let c = sk.add_circle(DVec2::ZERO, 0.01).0;
        let sk = s.sketch(Reference::Plane(DatumPlane::Top), sk);
        s.add("extrude", move |p| {
            p.set("bodyType", ParamValue::Enum("SURFACE".into()));
            p.set("surfaceEntities", ParamValue::Query(vec![Reference::SketchEntity { sketch: sk, entity: c }]));
            p.set("depth", expr("30 mm"));
        });
        let r = s.ok();
        let ids: Vec<BodyId> = r.context.bodies.iter().map(|b| b.id.clone()).collect();
        s.add("splitPart", move |p| {
            p.set("splitType", ParamValue::Enum("PART".into()));
            p.set("targets", ParamValue::Query(vec![Reference::Body(ids[0].clone())]));
            p.set("tool", ParamValue::Query(vec![Reference::Body(ids[1].clone())]));
            p.set("keepBothSides", ParamValue::Bool(false));
            p.set("keepFront", ParamValue::Bool(front));
        });
        let r = s.ok();
        r.context.bodies.iter().filter(|b| b.kind == BodyKind::Surface && b.shape.faces().len() == 1).map(|b| b.shape.faces()[0].area().unwrap()).collect::<Vec<f64>>()
    };
    let (a, b) = (run(true), run(false));
    let disc = PI * 0.01f64.powi(2);
    assert!(a.len() == 1 && b.len() == 1, "{a:?} {b:?}");
    let mut got = [a[0], b[0]];
    got.sort_by(f64::total_cmp);
    assert!(near(got[0], disc, 1e-6) && near(got[1], 0.01 - disc, 1e-6), "{got:?}");
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

/// qa5 SF5: Move boundary on one edge of a planar surface moves only that
/// edge (live: 800 + 200 mm² for one 20 mm edge of a 40 × 20 fill, 10 mm).
#[test]
fn move_boundary_of_one_edge_moves_only_it() {
    let mut s = block();
    let r = s.ok();
    let top = top_face(&r);
    let off = s.add("offsetSurface", move |p| {
        p.set("surfacesAndFaces", ParamValue::Query(vec![top]));
        p.set("offset", expr("10 mm"));
    });
    let r = s.ok();
    let sf = r.context.bodies.iter().position(|b| b.id.feature == off).unwrap();
    let b = &r.context.bodies[sf];
    // The 100 mm edge at y = 0.
    let e = b.shape.edges().iter().position(|e| {
        let (a, c) = e.endpoints().unwrap();
        a.y.abs() < 1e-9 && c.y.abs() < 1e-9
    });
    let edge = Reference::Edge((*b.edges[e.unwrap()]).clone());
    s.add("extendSurface", move |p| {
        p.set("entities", ParamValue::Query(vec![edge]));
        p.set("extendDistance", expr("5 mm"));
    });
    let r = s.ok();
    let sf = r.context.bodies.iter().position(|b| b.kind == BodyKind::Surface).unwrap();
    assert!(near(surface_area(&r, sf), 0.1 * 0.055, 1e-9), "area {}", surface_area(&r, sf));
    let bb = r.context.bodies[sf].shape.bounding_box(true).unwrap();
    assert!(near(bb.min.y, -0.005, 1e-9) && near(bb.max.y, 0.05, 1e-9) && near(bb.min.x, 0.0, 1e-9) && near(bb.max.x, 0.1, 1e-9), "{bb:?}");
}

/// Move boundary of a curved surface carries it on along itself (c2-surf-workshop-manualblend's Move boundary 1 and 2: a
/// swept tube 25 mm longer at an end): a cylinder's sheet (R20, 40 mm) 5 mm on past its top edge, then 5 mm past both.
#[test]
fn move_boundary_of_a_cylinder_sheet_lengthens_it() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(DVec2::ZERO, 0.02);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    s.extrude_largest(sid, "40 mm");
    let r = s.ok();
    let side = r.context.bodies[0].faces.iter().find(|n| matches!(&***n, TopoName::Face { role: Role::Side(_), .. })).unwrap();
    let side = Reference::Face((**side).clone());
    let off = s.add("offsetSurface", move |p| {
        p.set("surfacesAndFaces", ParamValue::Query(vec![side]));
        p.set("offset", expr("0 mm"));
    });
    let r = s.ok();
    let sf = r.context.bodies.iter().position(|b| b.id.feature == off).unwrap();
    let b = &r.context.bodies[sf];
    let top = b.shape.edges().iter().position(|e| e.endpoints().is_ok_and(|(a, c)| near(a.z, 0.04, 1e-9) && near(c.z, 0.04, 1e-9)));
    let top = Reference::Edge((*b.edges[top.unwrap()]).clone());
    let body = Reference::Body(b.id.clone());
    s.add("extendSurface", move |p| {
        p.set("entities", ParamValue::Query(vec![top]));
        p.set("extendDistance", expr("5 mm"));
    });
    for (whole, h, z0) in [(false, 0.045, 0.0), (true, 0.055, -0.005)] {
        if whole {
            let body = body.clone();
            s.add("extendSurface", move |p| {
                p.set("entities", ParamValue::Query(vec![body]));
                p.set("extendDistance", expr("5 mm"));
            });
        }
        let r = s.ok();
        let sf = r.context.bodies.iter().position(|b| b.id.feature == off).unwrap();
        let shape = &r.context.bodies[sf].shape;
        assert!(shape.is_valid());
        assert_eq!(shape.faces().len(), 1);
        let a = surface_area(&r, sf);
        assert!(near(a, 2.0 * PI * 0.02 * h, 1e-8), "area {a}");
        let bb = shape.bounding_box(true).unwrap();
        assert!(near(bb.min.z, z0, 1e-7) && near(bb.max.z, z0 + h, 1e-7) && near(bb.max.x, 0.02, 1e-7), "{bb:?}");
    }
}

/// qa5 SF6: Move boundary on the straight edge of a planar surface between
/// two arcs (40 mm chords, 10 mm sagitta, 30 mm apart: 1200 mm²), 10 mm out:
/// the arcs run on along their tangents (live's default, Maintain curvature
/// off), 1200 + 300 mm², one face. 10 mm in: the arcs are trimmed on their
/// circles, 1200 - 300 mm².
#[test]
fn move_boundary_between_arcs_extends_them() {
    for (dist, want) in [("10 mm", 1500e-6), ("-10 mm", 900e-6)] {
        let mut s = Studio::new();
        let mut sk = noshape_sketch::Sketch::new();
        let p = |sk: &mut noshape_sketch::Sketch, x: f64, y: f64| sk.add_point(DVec2::new(x * 1e-3, y * 1e-3));
        let (a, b, c, d) = (p(&mut sk, 0.0, 0.0), p(&mut sk, 40.0, 0.0), p(&mut sk, 40.0, 30.0), p(&mut sk, 0.0, 30.0));
        let (c1, c2) = (p(&mut sk, 20.0, 15.0), p(&mut sk, 20.0, 45.0));
        // Both arcs bulge towards -y (through (20, -10) and (20, 20)).
        sk.add_arc_between(c1, a, b).unwrap();
        sk.add_line_between(b, c).unwrap();
        sk.add_arc_between(c2, d, c).unwrap();
        sk.add_line_between(d, a).unwrap();
        let id = s.sketch(Reference::Plane(DatumPlane::Top), sk);
        s.extrude_largest(id, "10 mm");
        let r = s.ok();
        assert!(near(r.context.bodies[0].shape.volume().unwrap(), 1200e-6 * 0.01, 1e-12));
        let top = top_face(&r);
        let off = s.add("offsetSurface", move |p| {
            p.set("surfacesAndFaces", ParamValue::Query(vec![top]));
            p.set("offset", expr("10 mm"));
        });
        let r = s.ok();
        let sf = r.context.bodies.iter().position(|b| b.id.feature == off).unwrap();
        let b = &r.context.bodies[sf];
        // The straight edge at x = 40 mm.
        let e = b.shape.edges().iter().position(|e| {
            let (a, c) = e.endpoints().unwrap();
            (a.x - 0.04).abs() < 1e-9 && (c.x - 0.04).abs() < 1e-9
        });
        let edge = Reference::Edge((*b.edges[e.unwrap()]).clone());
        let d = dist.to_string();
        let again = edge.clone();
        s.add("extendSurface", move |p| {
            p.set("entities", ParamValue::Query(vec![edge]));
            p.set("extendDistance", expr(&d));
        });
        let r = s.ok();
        let sf = r.context.bodies.iter().position(|b| b.kind == BodyKind::Surface).unwrap();
        assert_eq!(r.context.bodies[sf].shape.faces().len(), 1);
        assert!(near(surface_area(&r, sf), want, 1e-10), "{dist}: area {} mm²", surface_area(&r, sf) * 1e6);
        let bb = r.context.bodies[sf].shape.bounding_box(true).unwrap();
        let x = if want > 1200e-6 { 0.05 } else { 0.03 };
        assert!(near(bb.max.x, x, 1e-9), "{dist}: {bb:?}");
        // The moved edge keeps its name: a second Move boundary of the same
        // edge (qa5's doc: Move boundary 1, then 3) moves it on.
        if want > 1200e-6 {
            s.add("extendSurface", move |p| {
                p.set("entities", ParamValue::Query(vec![again]));
                p.set("extendDistance", expr("10 mm"));
            });
            let r = s.ok();
            let sf = r.context.bodies.iter().position(|b| b.kind == BodyKind::Surface).unwrap();
            assert!(near(surface_area(&r, sf), 1800e-6, 1e-10), "again: area {} mm²", surface_area(&r, sf) * 1e6);
            assert!(near(r.context.bodies[sf].shape.bounding_box(true).unwrap().max.x, 0.06, 1e-9));
        }
    }
}

/// Live SF7 (workflow-surfaces 10): Fill on the open edges of an open box
/// (a 20 mm box surface without its top) with Add, Merge with all: knits into
/// one solid, 8000 mm³, 6 faces.
#[test]
fn fill_on_an_open_box_knits_a_solid() {
    use noshape_kernel as k;
    let cube = k::make_box(&k::Plane::xy(), 0.02, 0.02, 0.02).unwrap();
    let walls: Vec<k::Shape> = cube.faces().into_iter().filter(|f| f.centroid().unwrap().z < 0.0199).collect();
    assert_eq!(walls.len(), 5);
    let open = k::sew(&walls, 1e-7, false).unwrap().shape;
    let brep = String::from_utf8(open.to_brep_bytes().unwrap()).unwrap();
    let mut s = Studio::new();
    s.add("importForeign", move |p| p.set("blobData", ParamValue::Text(brep)));
    let r = s.ok();
    assert_eq!(r.context.bodies[0].kind, BodyKind::Surface);
    let b = &r.context.bodies[0];
    let rim: Vec<Reference> = b.shape.edges().iter().enumerate().filter(|(_, e)| { let i = e.edge_info().unwrap(); i.start.z > 0.0199 && i.end.z > 0.0199 }).map(|(i, _)| Reference::Edge((*b.edges[i]).clone())).collect();
    assert_eq!(rim.len(), 4);
    s.add("fill", move |p| {
        let mut row = ParamValues::default();
        row.set("entities", ParamValue::Query(rim));
        row.set("continuity", ParamValue::Enum("G0".into()));
        p.set("edges", ParamValue::Array(vec![row]));
        p.set("surfaceOperationType", ParamValue::Enum("ADD".into()));
        p.set("defaultSurfaceScope", ParamValue::Bool(true));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 1);
    let b = &r.context.bodies[0];
    assert_eq!(b.kind, BodyKind::Solid);
    assert_eq!(b.shape.faces().len(), 6);
    assert!((b.shape.volume().unwrap() * 1e9 - 8000.0).abs() < 1e-6);
}


/// psui's boundary-surface-new-edit-reedit (ui-parity start "surfaces"): a Boundary surface between two opposite edges
/// of a plane surface lies on it; with Add (Merge with all) live's parts stay one surface there and the plane crossing
/// it, no part (ours sewed the two coincident sheets into a solid and took the crossing sheet with it), and the feature
/// notes "Boolean resulted in no geometry change…" (Info) as live's step 4.
#[test]
fn a_boundary_surface_on_a_surface_already_there_adds_nothing() {
    let mut s = Studio::new();
    let mut a = noshape_sketch::Sketch::new();
    let la = a.add_line(DVec2::new(-0.02, 0.01), DVec2::new(0.02, 0.01)).0;
    let sa = s.sketch(Reference::Plane(DatumPlane::Front), a);
    s.add("extrude", move |p| {
        p.set("bodyType", ParamValue::Enum("SURFACE".into()));
        p.set("surfaceEntities", ParamValue::Query(vec![Reference::SketchEntity { sketch: sa, entity: la }]));
        p.set("depth", expr("40 mm"));
        p.set("symmetric", ParamValue::Bool(true));
    });
    let mut b = noshape_sketch::Sketch::new();
    let lb = b.add_line(DVec2::new(0.0, 0.0), DVec2::new(0.0, 0.02)).0;
    let sb = s.sketch(Reference::Plane(DatumPlane::Right), b);
    s.add("extrude", move |p| {
        p.set("bodyType", ParamValue::Enum("SURFACE".into()));
        p.set("surfaceEntities", ParamValue::Query(vec![Reference::SketchEntity { sketch: sb, entity: lb }]));
        p.set("depth", expr("120 mm"));
        p.set("symmetric", ParamValue::Bool(true));
    });
    let r = s.ok();
    let b = &r.context.bodies[0];
    let side = |x: f64| {
        let i = b.shape.edges().iter().position(|e| { let (p, q) = e.endpoints().unwrap(); near(p.x, x, 1e-9) && near(q.x, x, 1e-9) }).expect("edge");
        Reference::Edge((*b.edges[i]).clone())
    };
    let (e0, e1) = (side(0.02), side(-0.02));
    s.add("boundarySurface", move |p| {
        let row = |e: Reference| {
            let mut v = ParamValues::default();
            v.set("uProfileEntities", ParamValue::Query(vec![e]));
            v.set("uCondition", ParamValue::Enum("DEFAULT".into()));
            v
        };
        p.set("uProfilesArray", ParamValue::Array(vec![row(e0), row(e1)]));
        p.set("surfaceOperationType", ParamValue::Enum("ADD".into()));
        p.set("defaultSurfaceScope", ParamValue::Bool(true));
    });
    let r = s.ok();
    let kinds: Vec<BodyKind> = r.context.bodies.iter().map(|b| b.kind).collect();
    assert_eq!(kinds, vec![BodyKind::Surface, BodyKind::Surface]);
    assert!(near(surface_area(&r, 0), 1600e-6, 1e-10), "area {} mm²", surface_area(&r, 0) * 1e6);
    let st = r.display.features.last().unwrap();
    assert_eq!((st.status, st.message.as_deref()), (FeatureStatus::Info, Some(noshape_model::regen::eval::NO_GEOMETRY_CHANGE)));
}

/// Mutual trim's keep-side flips as std's (audit-part-studio M11; mutualTrim.fs createManipulators): a Front fill
/// crossing one on a plane at x 20 mm meets it along x 20, y 0: both arrows at that line's middle (z 15 mm), the
/// first across it in the Front surface (along X), the second in the other (along Y); std's change function turns a
/// click on the first into Keep opposite side.
#[test]
fn mutual_trim_flips_as_std() {
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
    let r = s.ok();
    s.add("mutualTrim", move |p| {
        p.set("body1", ParamValue::Query(vec![Reference::Body(BodyId { feature: fills[0], index: 0 })]));
        p.set("body2", ParamValue::Query(vec![Reference::Body(BodyId { feature: fills[1], index: 0 })]));
    });
    let mt = s.ps.features.last().unwrap().clone();
    let m = noshape_model::interaction::mutual_trim_manipulators(&r.context, &mt.params, &s.units);
    use noshape_model::display::FeatureManipulator as M;
    let [(k1, M::Flip { base, direction: d1, .. }), (_, M::Flip { direction: d2, .. })] = m.as_slice() else { panic!("{m:?}") };
    assert!((*base - DVec3::new(0.02, 0.0, 0.015)).length() < 1e-6 && d1.x.abs() > 0.999 && d2.y.abs() > 0.999, "{m:?}");
    let mut moved = m[0].1.clone();
    if let M::Flip { flipped, .. } = &mut moved {
        *flipped = true;
    }
    let changed = noshape_model::custom::std_feature::manipulator_change_in((*r.context).clone(), "mutualTrim", mt.id, &mt.name, &s.units, &mt.params, k1, &moved).expect("std's change function ran");
    assert_eq!(changed.get("keepOtherSide1"), Some(&ParamValue::Bool(true)), "{changed:?}");
}

/// Transform by distance along a construction sketch line (as live's asm-infento-subs studio 74d3d6, Transform 8: 20 mm
/// along Sketch 1's construction line): the line's start-to-end direction, as an edge's.
#[test]
fn transform_by_distance_along_a_construction_line() {
    let mut s = block();
    let r = s.ok();
    let id = r.context.bodies[0].id.clone();
    // On the Right plane (sketch x = world y, sketch y = world z): a construction line pointing up.
    let mut sk = noshape_sketch::Sketch::new();
    let a = sk.add_point(DVec2::new(0.0, 0.08));
    let b = sk.add_point(DVec2::new(0.0, 0.16));
    let line = sk.add_line_between(a, b).unwrap();
    sk.set_construction(line, true).unwrap();
    let sketch = s.sketch(Reference::Plane(DatumPlane::Right), sk);
    s.add("transform", move |p| {
        p.set("entities", ParamValue::Query(vec![Reference::Body(id)]));
        p.set("transformType", ParamValue::Enum("TRANSLATION_DISTANCE".into()));
        p.set("transformDirection", ParamValue::Query(vec![Reference::SketchEntity { sketch, entity: line.0 }]));
        p.set("distance", expr("20 mm"));
    });
    let r = s.ok();
    let bb = r.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!(near(bb.min.z, 0.020, 1e-6) && near(bb.max.z, 0.045, 1e-6), "moved up 20 mm: {bb:?}");
    assert!(near(bb.min.x, 0.0, 1e-6) && near(bb.min.y, 0.0, 1e-6), "only along the line: {bb:?}");
}

/// Transform by two points where one is the origin (asm-infento-subs' studio 74d3d6, Transform 19): from the origin to
/// the sketch point.
#[test]
fn transform_from_the_origin_to_a_sketch_point() {
    let mut s = block();
    let r = s.ok();
    let id = r.context.bodies[0].id.clone();
    let mut sk = noshape_sketch::Sketch::new();
    let p = sk.add_point(DVec2::new(0.03, 0.01));
    let sketch = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    s.add("transform", move |pv| {
        pv.set("entities", ParamValue::Query(vec![Reference::Body(id)]));
        pv.set("transformType", ParamValue::Enum("TRANSLATION_ENTITY".into()));
        pv.set("transformLine", ParamValue::Query(vec![Reference::Origin, Reference::SketchEntity { sketch, entity: p.0 }]));
    });
    let r = s.ok();
    let bb = r.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!(near(bb.min.x, 0.030, 1e-6) && near(bb.min.y, 0.010, 1e-6) && near(bb.min.z, 0.0, 1e-6), "moved by (30, 10, 0) mm: {bb:?}");
}

/// A circle seen edge-on (its plane holds the sketch normal) projects onto a line between its exact extremes, not a
/// sampled curve going back and forth (live keeps a Coincident point on that line past the circle: c2-rings-loft's
/// Sketch 1; exact, so a Use joined to another edge's end meets it: c2-fan-blade-sweep's Sketch 5).
#[test]
fn an_edge_on_circle_projects_to_an_exact_line() {
    use noshape_model::regen::features::sketch::{project_edge_any, SectionCurve};
    let frame = noshape_model::display::PlaneFrame { origin: DVec3::ZERO, x: DVec3::X, y: DVec3::Y, normal: DVec3::Z };
    let (c, r) = (DVec3::new(0.008, 0.001, 0.0), 0.0114);
    let circle = noshape_kernel::make_circle(c, DVec3::X, r).unwrap();
    let quarter = noshape_kernel::make_arc(c, DVec3::X, DVec3::Y, r, 0.0, PI / 2.0).unwrap();
    for (name, edge, ends) in [("circle", circle, [c.y - r, c.y + r]), ("quarter arc", quarter, [c.y, c.y + r])] {
        let Some(SectionCurve::Exact(noshape_sketch::Curve2::Line { a, b })) = project_edge_any(&edge, &frame) else { panic!("{name}: not a line") };
        let (lo, hi) = (a.y.min(b.y), a.y.max(b.y));
        let exact = |u: f64, v: f64| (u - v).abs() < 1e-12;
        assert!(exact(a.x, c.x) && exact(b.x, c.x) && exact(lo, ends[0]) && exact(hi, ends[1]), "{name}: {a:?} {b:?}");
    }
}

/// A B-spline edge projects exactly: its poles projected, its knots on 0..1, as live's Use stores it (a fit through
/// samples missed c2-parting-line-draft-parting-line's parting edge by 0.13 mm; Split 3 by its extrusion failed).
#[test]
fn a_bspline_edge_projects_to_its_exact_bspline() {
    use noshape_model::regen::features::sketch::{project_edge_any, SectionCurve};
    let frame = noshape_model::display::PlaneFrame { origin: DVec3::new(0.0, 0.0, 0.002), x: DVec3::X, y: DVec3::Z, normal: -DVec3::Y };
    let poles = [(-75.0, 23.67), (-64.36, 33.08), (-34.25, 47.55), (-4.16, 20.46), (37.40, 13.42), (63.22, 28.80), (75.0, 40.36)];
    let data = noshape_kernel::BSplineCurveData {
        poles: poles.iter().enumerate().map(|(i, (x, z))| DVec3::new(*x, -25.0 + i as f64, *z) * 1e-3).collect(),
        weights: None,
        knots: vec![0.0, 0.2548, 0.4919, 0.7368, 1.0],
        mults: vec![4, 1, 1, 1, 4],
        degree: 3,
        periodic: false,
    };
    let edge = noshape_kernel::make_bspline_edge(&data).unwrap();
    let Some(SectionCurve::Exact(noshape_sketch::Curve2::Nurbs(n))) = project_edge_any(&edge, &frame) else { panic!("not an exact B-spline") };
    assert_eq!((n.degree, n.ctrl.len(), n.knots.len()), (3, 7, 11));
    let info = edge.edge_info().unwrap();
    for i in 0..=40 {
        let t = i as f64 / 40.0;
        let p = edge.point_at(info.first_param + (info.last_param - info.first_param) * t).unwrap();
        let q = frame.to_local(p);
        assert!((n.point(t) - q).length() < 1e-12, "at {t}: {:?} vs {q:?}", n.point(t));
    }
}

/// An ellipse that projects onto a circle (a cylinder cut obliquely, seen along its axis): a circle, or an arc between
/// the edge's projected ends (live offsets such an edge as a circle: c2-hunting-knife's Sketch 16).
#[test]
fn an_ellipse_seen_as_a_circle_projects_to_a_circle() {
    use noshape_model::regen::features::sketch::project_edge;
    use noshape_sketch::Curve2;
    let frame = noshape_model::display::PlaneFrame { origin: DVec3::ZERO, x: DVec3::X, y: DVec3::Y, normal: DVec3::Z };
    let (t, r) = (0.6_f64, 0.007);
    let plane = noshape_kernel::Plane::new(DVec3::new(0.01, 0.02, 0.005), DVec3::new(0.0, -t.sin(), t.cos()), DVec3::X);
    let full = noshape_kernel::Segment2d::Ellipse { center: DVec2::ZERO, major_radius: r / t.cos(), minor_radius: r, rotation: PI / 2.0 };
    let edge = noshape_kernel::make_edge_2d(&plane, &full).unwrap();
    let Some(Curve2::Circle { center, radius }) = project_edge(&edge, &frame) else { panic!("not a circle: {:?}", project_edge(&edge, &frame)) };
    assert!((center - DVec2::new(0.01, 0.02)).length() < 1e-12 && (radius - r).abs() < 1e-12, "{center:?} {radius}");
    let quarter = noshape_kernel::Segment2d::EllipseArc { center: DVec2::ZERO, major_radius: r / t.cos(), minor_radius: r, rotation: PI / 2.0, start_angle: 0.0, end_angle: PI / 2.0 };
    let edge = noshape_kernel::make_edge_2d(&plane, &quarter).unwrap();
    let info = edge.edge_info().unwrap();
    let Some(c @ Curve2::Arc { radius, start_angle, end_angle, .. }) = project_edge(&edge, &frame) else { panic!("not an arc") };
    let ends = [frame.to_local(info.start), frame.to_local(info.end)];
    let on = |p: DVec2| ends.iter().any(|e| (*e - p).length() < 1e-9);
    assert!((radius - r).abs() < 1e-12 && (end_angle - start_angle - PI / 2.0).abs() < 1e-9 && on(c.start_point()) && on(c.end_point()), "{c:?} {ends:?}");
}

/// Move boundary with nothing selected marks its "Surface or boundary edges" field whatever the end condition, as
/// live does with Up to part/surface, Up to face and Up to vertex (psui's move-boundary-up-to-* scenarios, step 3).
#[test]
fn move_boundary_with_nothing_selected_marks_its_selection_with_any_end_condition() {
    for cond in ["BLIND", "UP_TO_BODY", "UP_TO_FACE", "UP_TO_VERTEX"] {
        let mut s = Studio::new();
        s.add("extendSurface", move |p| p.set("endCondition", ParamValue::Enum(cond.into())));
        let r = s.regen();
        let st = r.display.features.last().unwrap();
        assert_eq!(st.status, FeatureStatus::Error, "{cond}");
        assert_eq!(st.faulty, vec!["entities".to_string()], "{cond}: {:?}", st.message);
    }
}

/// Move boundary Up to vertex / Up to face (ui-parity start "surfaces"): the plane surface z 10 (x, y -20..20) with its
/// x 20 edge picked runs out to the box beside it (x 30..50): to its corner, to its x 30 face, and to the corner with a
/// 5 mm offset along the move (std's target plane through the vertex, normal to the edge's move).
#[test]
fn move_boundary_up_to_a_vertex_or_a_face_moves_the_edge_onto_its_plane() {
    for (cond, offset, want_mm2) in [("UP_TO_VERTEX", 0.0, 2000.0), ("UP_TO_FACE", 0.0, 2000.0), ("UP_TO_VERTEX", 5.0, 2200.0)] {
        let mut s = Studio::new();
        let mut a = noshape_sketch::Sketch::new();
        let la = a.add_line(DVec2::new(-0.02, 0.01), DVec2::new(0.02, 0.01)).0;
        let sa = s.sketch(Reference::Plane(DatumPlane::Front), a);
        let surf = s.add("extrude", move |p| {
            p.set("bodyType", ParamValue::Enum("SURFACE".into()));
            p.set("surfaceEntities", ParamValue::Query(vec![Reference::SketchEntity { sketch: sa, entity: la }]));
            p.set("depth", expr("40 mm"));
            p.set("symmetric", ParamValue::Bool(true));
        });
        let mut b = noshape_sketch::Sketch::new();
        rect(&mut b, 0.03, -0.01, 0.02, 0.02);
        let sb = s.sketch(Reference::Plane(DatumPlane::Top), b);
        s.extrude_largest(sb, "5 mm");
        let r = s.ok();
        let sheet = r.context.bodies.iter().find(|b| b.id.feature == surf).unwrap();
        let ei = sheet.shape.edges().iter().position(|e| { let (p, q) = e.endpoints().unwrap(); near(p.x, 0.02, 1e-9) && near(q.x, 0.02, 1e-9) }).unwrap();
        let edge = Reference::Edge((*sheet.edges[ei]).clone());
        let boxb = r.context.bodies.iter().find(|b| b.kind == BodyKind::Solid).unwrap();
        let vi = boxb.shape.vertices().iter().position(|v| (v.vertex_point().unwrap() - DVec3::new(0.03, -0.01, 0.0)).length() < 1e-9).unwrap();
        let vertex = Reference::Vertex((*boxb.vertices[vi]).clone());
        let fi = boxb.shape.faces().iter().position(|f| near(f.centroid().unwrap().x, 0.03, 1e-9)).unwrap();
        let face = Reference::Face((*boxb.faces[fi]).clone());
        s.add("extendSurface", move |p| {
            p.set("entities", ParamValue::Query(vec![edge]));
            p.set("endCondition", ParamValue::Enum(cond.into()));
            p.set("targetVertex", ParamValue::Query(vec![vertex]));
            p.set("targetFace", ParamValue::Query(vec![face]));
            p.set("hasOffset", ParamValue::Bool(offset != 0.0));
            p.set("offset", expr(&format!("{offset} mm")));
        });
        let r = s.ok();
        let sheet = r.context.bodies.iter().position(|b| b.id.feature == surf).unwrap();
        assert!(near(surface_area(&r, sheet), want_mm2 * 1e-6, 1e-10), "{cond} offset {offset}: {} mm²", surface_area(&r, sheet) * 1e6);
    }
}

/// psui's move-boundary-up-to-face / -vertex: with a surface picked, an Up to end condition without its Target marks
/// that field (std's EXTEND_SHEET_BODY_NO_TARGET); Up to part/surface asks for one only with an offset.
#[test]
fn move_boundary_up_to_without_a_target_marks_the_target() {
    for (cond, offset, want) in [("UP_TO_FACE", false, Some("targetFace")), ("UP_TO_VERTEX", false, Some("targetVertex")), ("UP_TO_BODY", true, Some("targetPart")), ("UP_TO_BODY", false, None)] {
        let mut s = Studio::new();
        let mut a = noshape_sketch::Sketch::new();
        let la = a.add_line(DVec2::new(-0.02, 0.01), DVec2::new(0.02, 0.01)).0;
        let sa = s.sketch(Reference::Plane(DatumPlane::Front), a);
        let ex = s.add("extrude", move |p| {
            p.set("bodyType", ParamValue::Enum("SURFACE".into()));
            p.set("surfaceEntities", ParamValue::Query(vec![Reference::SketchEntity { sketch: sa, entity: la }]));
            p.set("depth", expr("40 mm"));
        });
        s.add("extendSurface", move |p| {
            p.set("entities", ParamValue::Query(vec![Reference::Body(BodyId { feature: ex, index: 0 })]));
            p.set("endCondition", ParamValue::Enum(cond.into()));
            p.set("hasOffset", ParamValue::Bool(offset));
        });
        let r = s.regen();
        let st = r.display.features.last().unwrap();
        assert_eq!(st.status, FeatureStatus::Error, "{cond}");
        let want: Vec<String> = want.into_iter().map(String::from).collect();
        assert_eq!(st.faulty, want, "{cond} offset {offset}: {:?}", st.message);
    }
}

/// qa SELFEEL-DBLCLICK-FACE-SKETCH (live psui/sel-feel C2-dbl-face-part2): a double-clicked face's sketch is drawn in
/// its constraint colours (an undimensioned rectangle blue), so a sketch the user isn't editing carries its solve's
/// states (it showed in the inactive grey).
#[test]
fn a_sketch_not_edited_carries_its_constraint_states() {
    use noshape_model::display::SketchEntityState;
    let mut s = block();
    let r = s.regen();
    let sk = &r.display.sketches[0];
    assert!(!sk.curves.is_empty() && sk.curves.iter().all(|c| c.state == SketchEntityState::Under), "{:?}", sk.curves.iter().map(|c| c.state).collect::<Vec<_>>());
    assert!(sk.points.iter().all(|p| p.state == SketchEntityState::Under));
}
