//! Plane feature (`cPlane`): Onshape's cplane.fs behaviour.

use glam::{DVec2, DVec3};
use noshape_model::configuration::ConfigValues;
use noshape_model::display::{FeatureStatus, PlaneFrame};
use noshape_model::params::{ParamValue, ParamValues};
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, FeatureId, Reference};
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};

struct Studio {
    ps: PartStudio,
    units: UnitPrefs,
    reg: Regenerator,
    opts: RegenOptions,
}

fn expr(s: &str) -> ParamValue {
    ParamValue::Expr(ExprValue::new(s))
}

fn en(s: &str) -> ParamValue {
    ParamValue::Enum(s.into())
}

fn qs(r: Vec<Reference>) -> ParamValue {
    ParamValue::Query(r)
}

fn mm(v: f64) -> f64 {
    v * 1e-3
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

fn near3(a: DVec3, b: DVec3) -> bool {
    (a - b).length() < 1e-9
}

impl Studio {
    fn new() -> Self {
        Self { ps: PartStudio::new(), units: UnitPrefs::default(), reg: Regenerator::new(), opts: RegenOptions::default() }
    }

    fn regen(&mut self) -> RegenResult {
        self.reg.regenerate(&self.ps, &ConfigValues::default(), &self.units, &self.opts)
    }

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

    fn ok(&mut self) -> RegenResult {
        let r = self.regen();
        for (f, s) in self.ps.features.iter().zip(&r.display.features) {
            assert!(matches!(s.status, FeatureStatus::Ok), "{} ({}) failed: {:?}", f.name, f.type_id, s.message);
        }
        r
    }

    fn plane(&mut self, set: impl FnOnce(&mut ParamValues)) -> FeatureId {
        self.add("cPlane", set)
    }

    /// Frame and half sizes of a plane feature.
    fn frame(r: &RegenResult, id: FeatureId) -> (PlaneFrame, f64, f64) {
        let p = r.context.planes.iter().find(|p| p.reference == Reference::FeaturePlane(id)).expect("plane");
        (p.frame, p.half_width, p.half_height)
    }
}

/// A cylinder of radius 20 mm on the Top plane, 30 mm high; returns its
/// sketch circle.
fn cylinder(s: &mut Studio) -> Reference {
    let mut sk = noshape_sketch::Sketch::new();
    let circle = sk.add_circle(DVec2::ZERO, mm(20.0)).0;
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let r = s.regen();
    let sd = r.display.sketches.iter().find(|x| x.feature == sid).unwrap();
    let key = sd.regions[0].key.clone();
    s.add("extrude", move |p| {
        p.set("entities", qs(vec![Reference::SketchRegion { sketch: sid, key }]));
        p.set("depth", expr("30 mm"));
    });
    Reference::SketchEntity { sketch: sid, entity: circle }
}

fn cylinder_face(r: &RegenResult) -> Reference {
    let b = &r.context.bodies[0];
    let i = b.shape.faces().iter().position(|f| matches!(f.face_surface().unwrap(), noshape_kernel::Surface::Cylinder { .. })).unwrap();
    Reference::Face((*b.faces[i]).clone())
}

fn circular_edge_at(r: &RegenResult, z: f64) -> Reference {
    let b = &r.context.bodies[0];
    let i = b
        .shape
        .edges()
        .iter()
        .position(|e| matches!(e.edge_curve().unwrap(), noshape_kernel::Curve::Circle { center, .. } if near(center.z, z, 1e-9)))
        .unwrap();
    Reference::Edge((*b.edges[i]).clone())
}

fn points_sketch(s: &mut Studio, pts: &[(f64, f64)]) -> (FeatureId, Vec<Reference>) {
    let mut sk = noshape_sketch::Sketch::new();
    let ids: Vec<u32> = pts.iter().map(|&(x, y)| sk.add_point(DVec2::new(mm(x), mm(y))).0).collect();
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    (sid, ids.into_iter().map(|entity| Reference::SketchEntity { sketch: sid, entity }).collect())
}

#[test]
fn offset_from_a_circular_edge() {
    let mut s = Studio::new();
    let circle = cylinder(&mut s);
    let r = s.regen();
    let top = circular_edge_at(&r, mm(30.0));
    let a = s.plane(|p| {
        p.set("entities", qs(vec![top]));
        p.set("offset", expr("10 mm"));
    });
    let b = s.plane(|p| {
        p.set("entities", qs(vec![circle]));
        p.set("offset", expr("10 mm"));
        p.set("oppositeDirection", ParamValue::Bool(true));
    });
    let r = s.ok();
    let (f, hw, hh) = Studio::frame(&r, a);
    assert!(near(f.normal.z.abs(), 1.0, 1e-9), "normal along the circle axis {f:?}");
    assert!(near3(f.origin, DVec3::new(0.0, 0.0, mm(30.0)) + f.normal * mm(10.0)), "origin = centre + offset {f:?}");
    // Not a face: the default plane size (150 mm).
    assert!(near(hw, mm(75.0), 1e-9) && near(hh, mm(75.0), 1e-9));
    let (f, ..) = Studio::frame(&r, b);
    assert!(near3(f.origin, DVec3::new(0.0, 0.0, -mm(10.0))) && near3(f.normal, DVec3::Z), "sketch circle, opposite direction {f:?}");

    // A straight edge can't be offset.
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let l = sk.add_line(DVec2::ZERO, DVec2::new(mm(10.0), 0.0)).0;
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    s.plane(|p| p.set("entities", qs(vec![Reference::SketchEntity { sketch: sid, entity: l }])));
    let r = s.regen();
    let st = r.display.features.last().unwrap();
    assert_eq!(st.status, FeatureStatus::Error);
    assert_eq!(st.message.as_deref(), Some("Offset plane requires a plane, circle, ellipse, or arc to offset from."));
}

#[test]
fn offset_from_a_plane_keeps_its_size() {
    let mut s = Studio::new();
    let a = s.plane(|p| {
        p.set("entities", qs(vec![Reference::Plane(DatumPlane::Top)]));
        p.set("offset", expr("25 mm"));
    });
    let r = s.ok();
    let (f, hw, hh) = Studio::frame(&r, a);
    assert!(near3(f.origin, DVec3::new(0.0, 0.0, mm(25.0))) && near3(f.normal, DVec3::Z));
    // The default planes are construction planes: no extension.
    assert!(near(hw, 0.05, 1e-9) && near(hh, 0.05, 1e-9), "{hw} {hh}");
}

#[test]
fn three_point_plane_is_centred_on_the_points() {
    let mut s = Studio::new();
    let (_, pts) = points_sketch(&mut s, &[(0.0, 0.0), (30.0, 0.0), (0.0, 30.0)]);
    let a = s.plane(|p| {
        p.set("cplaneType", en("THREE_POINT"));
        p.set("entities", qs(pts));
    });
    let r = s.ok();
    let (f, hw, hh) = Studio::frame(&r, a);
    assert!(near3(f.origin, DVec3::new(mm(10.0), mm(10.0), 0.0)), "centroid {f:?}");
    // normal = (p2 - p0) x (p1 - p0), x = p1 - p0.
    assert!(near3(f.normal, -DVec3::Z) && near3(f.x, DVec3::X), "{f:?}");
    let max = (mm(20.0f64).powi(2) + mm(10.0f64).powi(2)).sqrt();
    assert!(near(hw, max * 1.2, 1e-12) && near(hh, max * 1.2, 1e-12), "{hw} vs {}", max * 1.2);
}

#[test]
fn mid_plane_of_two_intersecting_planes() {
    for flip in [false, true] {
        let mut s = Studio::new();
        let a = s.plane(|p| {
            p.set("cplaneType", en("MID_PLANE"));
            p.set("entities", qs(vec![Reference::Plane(DatumPlane::Top), Reference::Plane(DatumPlane::Right)]));
            p.set("flipAlignment", ParamValue::Bool(flip));
        });
        let r = s.ok();
        let (f, ..) = Studio::frame(&r, a);
        let h = 0.5f64.sqrt();
        // Default: the bisector of the wedge (plane 2's normal negated);
        // flipped: the other bisector.
        let (n, x) = if flip { (DVec3::new(h, 0.0, h), DVec3::Y) } else { (DVec3::new(-h, 0.0, h), -DVec3::Y) };
        assert!(near3(f.normal, n) && near3(f.x, x), "flip {flip}: {f:?}");
        assert!(near3(f.origin, DVec3::ZERO));
    }
    // Two parallel planes: plane 1's orientation, half way.
    let mut s = Studio::new();
    let off = s.plane(|p| {
        p.set("entities", qs(vec![Reference::Plane(DatumPlane::Top)]));
        p.set("offset", expr("40 mm"));
    });
    let a = s.plane(|p| {
        p.set("cplaneType", en("MID_PLANE"));
        p.set("entities", qs(vec![Reference::Plane(DatumPlane::Top), Reference::FeaturePlane(off)]));
    });
    let r = s.ok();
    let (f, ..) = Studio::frame(&r, a);
    assert!(near3(f.origin, DVec3::new(0.0, 0.0, mm(20.0))) && near3(f.normal, DVec3::Z), "{f:?}");
}

#[test]
fn tangent_plane_through_an_outside_point() {
    let mut s = Studio::new();
    cylinder(&mut s);
    let r = s.regen();
    let cyl = cylinder_face(&r);
    let (_, pts) = points_sketch(&mut s, &[(40.0, 0.0)]);
    let p = DVec3::new(mm(40.0), 0.0, 0.0);
    let mut frames = Vec::new();
    for flip in [false, true] {
        let (c, pt) = (cyl.clone(), pts[0].clone());
        frames.push(s.plane(move |p| {
            p.set("cplaneType", en("TANGENT_PLANE"));
            p.set("entities", qs(vec![c, pt]));
            p.set("flipAlignment", ParamValue::Bool(flip));
        }));
    }
    let r = s.ok();
    let a = Studio::frame(&r, frames[0]).0;
    let b = Studio::frame(&r, frames[1]).0;
    for f in [a, b] {
        assert!(f.normal.z.abs() < 1e-9, "parallel to the axis {f:?}");
        assert!(near(DVec2::new(f.origin.x, f.origin.y).length(), mm(20.0), 1e-9), "on the cylinder {f:?}");
        assert!(near((p - f.origin).dot(f.normal), 0.0, 1e-9), "through the point {f:?}");
        // Tangent: the normal points away from the axis.
        assert!(near(DVec2::new(f.origin.x, f.origin.y).normalize().dot(DVec2::new(f.normal.x, f.normal.y)), 1.0, 1e-9));
        // cos(angle) = r / |p|.
        assert!(near(f.normal.x, 0.5, 1e-9), "{f:?}");
    }
    assert!(near(a.normal.y, -b.normal.y, 1e-9) && a.normal.y.abs() > 0.5, "flip alignment takes the other tangent: {a:?} {b:?}");

    // A point inside the cylinder.
    let mut s = Studio::new();
    cylinder(&mut s);
    let r = s.regen();
    let cyl = cylinder_face(&r);
    let (_, pts) = points_sketch(&mut s, &[(5.0, 0.0)]);
    s.plane(move |p| {
        p.set("cplaneType", en("TANGENT_PLANE"));
        p.set("entities", qs(vec![cyl, pts[0].clone()]));
    });
    let r = s.regen();
    let st = r.display.features.last().unwrap();
    assert_eq!(st.message.as_deref(), Some("Selected point must be outside the selected cylinder."));
    assert_eq!(st.faulty, vec!["entities".to_string()]);

    // The cylinder alone: the plane is made, with an information message.
    let mut s = Studio::new();
    cylinder(&mut s);
    let r = s.regen();
    let cyl = cylinder_face(&r);
    let a = s.plane(move |p| {
        p.set("cplaneType", en("TANGENT_PLANE"));
        p.set("entities", qs(vec![cyl]));
    });
    let r = s.regen();
    let st = r.display.feature_state(a).unwrap();
    assert_eq!(st.status, FeatureStatus::Info);
    assert_eq!(st.message.as_deref(), Some("Select an additional point or plane to orient the plane."));
    let (f, ..) = Studio::frame(&r, a);
    assert!(near(DVec2::new(f.origin.x, f.origin.y).length(), mm(20.0), 1e-9), "{f:?}");
}

#[test]
fn line_angle_with_one_line_reports_info() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let l = sk.add_line(DVec2::ZERO, DVec2::new(mm(50.0), 0.0)).0;
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let line = Reference::SketchEntity { sketch: sid, entity: l };
    let a = s.plane(|p| {
        p.set("cplaneType", en("LINE_ANGLE"));
        p.set("entities", qs(vec![line.clone()]));
    });
    let b = s.plane(|p| {
        p.set("cplaneType", en("LINE_ANGLE"));
        p.set("entities", qs(vec![line, Reference::Plane(DatumPlane::Top)]));
        p.set("angle", expr("90 deg"));
    });
    let r = s.regen();
    let st = r.display.feature_state(a).unwrap();
    assert_eq!(st.status, FeatureStatus::Info);
    assert_eq!(st.message.as_deref(), Some("Select an additional point, plane, or axis to specify where the angle is measured from."));
    let (f, hw, _) = Studio::frame(&r, a);
    assert!(near(f.normal.dot(DVec3::X), 0.0, 1e-12) && near3(f.x, DVec3::X), "contains the line {f:?}");
    assert!(near(hw, mm(30.0), 1e-12), "1.2 x the line length: {hw}");
    // Line + Top plane: at 0 degrees the plane is Top; at 90 degrees it
    // contains the line and is normal to Top.
    assert_eq!(r.display.feature_state(b).unwrap().status, FeatureStatus::Ok);
    let (f, ..) = Studio::frame(&r, b);
    assert!(near(f.normal.y.abs(), 1.0, 1e-9) && near3(f.x, DVec3::X), "{f:?}");
}

#[test]
fn empty_selection_messages() {
    let cases = [
        ("OFFSET", "Offset plane requires a plane, circle, ellipse, or arc to offset from."),
        ("PLANE_POINT", "Point-Plane requires a point and a plane."),
        ("LINE_ANGLE", "Line-Angle plane requires a reference line and a point, plane, or axis."),
        ("LINE_POINT", "Point-Normal plane requires a point and an axis."),
        ("THREE_POINT", "Three point plane requires 3 points."),
        ("MID_PLANE", "Mid plane requires 2 points, 2 planes or 1 open edge."),
        ("CURVE_POINT", "Curve point plane requires a curve and a point."),
        ("TANGENT_PLANE", "Tangent plane requires a cylindrical surface and a point or plane."),
    ];
    for (ty, msg) in cases {
        let mut s = Studio::new();
        s.plane(|p| p.set("cplaneType", en(ty)));
        let r = s.regen();
        let st = r.display.features.last().unwrap();
        assert_eq!(st.status, FeatureStatus::Error, "{ty}");
        assert_eq!(st.message.as_deref(), Some(msg), "{ty}");
        assert_eq!(st.faulty, vec!["entities".to_string()], "{ty}");
    }
    // Too many entities.
    let mut s = Studio::new();
    s.plane(|p| p.set("entities", qs(vec![Reference::Plane(DatumPlane::Top), Reference::Plane(DatumPlane::Front)])));
    let r = s.regen();
    assert_eq!(r.display.features.last().unwrap().message.as_deref(), Some("Too many entities are selected."));
}

#[test]
fn point_normal_curve_point_and_plane_point() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let l = sk.add_line(DVec2::ZERO, DVec2::new(mm(50.0), 0.0)).0;
    let pt = sk.add_point(DVec2::new(mm(20.0), mm(10.0))).0;
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let (line, point) = (Reference::SketchEntity { sketch: sid, entity: l }, Reference::SketchEntity { sketch: sid, entity: pt });
    let (l1, p1, l2, p2) = (line.clone(), point.clone(), line, point.clone());
    let normal = s.plane(|p| {
        p.set("cplaneType", en("LINE_POINT"));
        p.set("entities", qs(vec![l1, p1]));
    });
    // Saved before the port: POINT_NORMAL still evaluates.
    let alias = s.plane(|p| {
        p.set("cplaneType", en("POINT_NORMAL"));
        p.set("entities", qs(vec![point.clone(), Reference::SketchEntity { sketch: sid, entity: l }]));
    });
    let curve = s.plane(|p| {
        p.set("cplaneType", en("CURVE_POINT"));
        p.set("entities", qs(vec![l2, p2]));
    });
    let pp = s.plane(|p| {
        p.set("cplaneType", en("PLANE_POINT"));
        p.set("entities", qs(vec![Reference::Plane(DatumPlane::Front), point]));
        p.set("flipNormal", ParamValue::Bool(true));
    });
    let r = s.ok();
    let (f, ..) = Studio::frame(&r, normal);
    assert!(near3(f.origin, DVec3::new(mm(20.0), mm(10.0), 0.0)) && near(f.normal.x.abs(), 1.0, 1e-12), "{f:?}");
    assert_eq!(Studio::frame(&r, alias).0, f, "POINT_NORMAL is LINE_POINT");
    let (f, ..) = Studio::frame(&r, curve);
    // At the point of the line closest to the point.
    assert!(near3(f.origin, DVec3::new(mm(20.0), 0.0, 0.0)) && near(f.normal.x.abs(), 1.0, 1e-12), "{f:?}");
    let (f, ..) = Studio::frame(&r, pp);
    assert!(near3(f.origin, DVec3::new(mm(20.0), mm(10.0), 0.0)) && near3(f.normal, DVec3::Y), "flipped Front normal {f:?}");
}

#[test]
fn saved_plane_params_are_upgraded() {
    let mut p = ParamValues::default();
    p.set("cplaneType", en("POINT_NORMAL"));
    p.set("flipAlignment", ParamValue::Bool(true));
    noshape_model::features::core::upgrade_plane_params(&mut p);
    assert_eq!(p.enum_("cplaneType"), "LINE_POINT");
    assert!(p.bool("flipNormal") && !p.bool("flipAlignment"));
    let mut p = ParamValues::default();
    p.set("cplaneType", en("TANGENT_CYLINDER"));
    p.set("oppositeDirection", ParamValue::Bool(true));
    noshape_model::features::core::upgrade_plane_params(&mut p);
    assert_eq!(p.enum_("cplaneType"), "TANGENT_PLANE");
    assert!(p.bool("flipAlignment") && !p.bool("flipNormal") && !p.bool("oppositeDirection"));
    // New parameters are left alone.
    p.set("flipAlignment", ParamValue::Bool(false));
    noshape_model::features::core::upgrade_plane_params(&mut p);
    assert!(!p.bool("flipAlignment"));
}

#[test]
fn preselection_picks_the_plane_type() {
    let mut s = Studio::new();
    cylinder(&mut s);
    let r = s.regen();
    let (cyl, top_edge) = (cylinder_face(&r), circular_edge_at(&r, mm(30.0)));
    let (_, pts) = points_sketch(&mut s, &[(40.0, 0.0), (0.0, 40.0), (40.0, 40.0)]);
    let mut sk = noshape_sketch::Sketch::new();
    let l = sk.add_line(DVec2::ZERO, DVec2::new(mm(50.0), 0.0)).0;
    let lsid = s.sketch(Reference::Plane(DatumPlane::Front), sk);
    let line = Reference::SketchEntity { sketch: lsid, entity: l };
    let r = s.regen();
    let logic = |refs: Vec<Reference>| {
        let mut p = ParamValues::default();
        p.set("cplaneType", en("CURVE_POINT"));
        p.set("entities", qs(refs));
        noshape_model::regen::features::cplane::cplane_logic(&r.context, &mut p);
        p.enum_("cplaneType").to_string()
    };
    assert_eq!(logic(vec![Reference::Plane(DatumPlane::Top)]), "OFFSET");
    assert_eq!(logic(vec![top_edge]), "OFFSET");
    assert_eq!(logic(vec![line.clone()]), "MID_PLANE");
    assert_eq!(logic(vec![cyl.clone()]), "LINE_ANGLE");
    assert_eq!(logic(vec![Reference::Plane(DatumPlane::Top), pts[0].clone()]), "PLANE_POINT");
    assert_eq!(logic(vec![pts[0].clone(), pts[1].clone()]), "MID_PLANE");
    assert_eq!(logic(vec![cyl.clone(), pts[0].clone()]), "TANGENT_PLANE");
    assert_eq!(logic(vec![line.clone(), pts[0].clone()]), "LINE_POINT");
    assert_eq!(logic(vec![line, Reference::Plane(DatumPlane::Top)]), "LINE_ANGLE");
    assert_eq!(logic(pts.clone()), "THREE_POINT");
    // Nothing recognisable: the type is left alone.
    assert_eq!(logic(vec![cyl, pts[0].clone(), pts[1].clone()]), "CURVE_POINT");
}
