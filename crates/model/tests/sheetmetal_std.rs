//! Built-in sheet metal features run by the standard library
//! (custom/std_feature.rs) against live's numbers (behaviours.md "G34";
//! docs/parity/workflow-sheet-metal.md SM1, SM3).

use glam::DVec3;
use noshape_model::configuration::ConfigValues;
use noshape_model::display::{BodyKind, FeatureStatus};
use noshape_model::params::{ParamValue, ParamValues};
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, FeatureId, Reference};
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};

fn mm(x: f64) -> f64 {
    x * 1e-3
}

fn expr(s: &str) -> ParamValue {
    ParamValue::Expr(ExprValue::new(s))
}

fn en(s: &str) -> ParamValue {
    ParamValue::Enum(s.into())
}

struct Studio {
    ps: PartStudio,
    units: UnitPrefs,
}

impl Studio {
    fn new() -> Self {
        noshape_model::custom::std_feature::set_std_sheet_metal(true);
        Self { ps: PartStudio::new(), units: UnitPrefs::default() }
    }

    fn add(&mut self, ty: &str, set: impl FnOnce(&mut ParamValues)) -> FeatureId {
        let mut f = self.ps.new_feature(ty, &self.units).unwrap_or_else(|| panic!("unknown feature {ty}"));
        set(&mut f.params);
        let id = f.id;
        self.ps.features.push(f);
        id
    }

    /// Regenerate; every feature must succeed (warnings allowed).
    fn ok(&mut self) -> RegenResult {
        let r = Regenerator::new().regenerate(&self.ps, &ConfigValues::default(), &self.units, &RegenOptions::default());
        for (f, s) in self.ps.features.iter().zip(&r.display.features) {
            // With std's FeatureScript notices (the failing call's stack).
            assert!(matches!(s.status, FeatureStatus::Ok | FeatureStatus::Info | FeatureStatus::Warning), "{} ({}) failed: {:?} {:?}", f.name, f.type_id, s.message, noshape_model::custom::run::output_of(f.id).map(|o| o.notices));
        }
        r
    }

    /// A rectangle (0,0)–(w,h) mm on Top as a sketch; its region.
    fn rectangle(&mut self, w: f64, h: f64) -> Reference {
        let mut sk = noshape_sketch::Sketch::new();
        let p = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)].map(|(x, y)| sk.add_point(glam::DVec2::new(mm(x), mm(y))));
        for i in 0..4 {
            sk.add_line_between(p[i], p[(i + 1) % 4]).unwrap();
        }
        let sketch = self.add("sketch", |pv| {
            pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
            pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
        });
        let r = self.ok();
        let sk = r.display.sketches.iter().find(|s| s.feature == sketch).expect("sketch");
        let key = sk.regions.iter().max_by(|a, b| a.area.total_cmp(&b.area)).expect("region").key.clone();
        Reference::SketchRegion { sketch, key }
    }
}

/// The sheet metal part (the one solid).
fn part(r: &RegenResult) -> usize {
    r.context.bodies.iter().position(|b| b.kind == BodyKind::Solid).expect("a part")
}

/// The part's edge through `p` (mm).
fn edge_at(r: &RegenResult, p: DVec3) -> Reference {
    let b = &r.context.bodies[part(r)];
    for (i, e) in b.shape.edges().iter().enumerate() {
        let v = noshape_kernel::make_vertex(p * 1e-3).unwrap();
        if noshape_kernel::distance(&v, e).is_ok_and(|d| d.distance < 1e-9) {
            return Reference::Edge((*b.edges[i]).clone());
        }
    }
    panic!("no edge at {p:?}");
}

fn volume_mm3(r: &RegenResult) -> f64 {
    r.context.bodies[part(r)].shape.volume().unwrap() * 1e9
}

/// G34: a 60 × 40 plate, 2 mm, bend radius 2, 20 mm flanges on the top
/// edges at y = 0 and x = 0 (live: 8511.4987 mm³, 24 faces; qa5 SM1).
fn g34() -> Studio {
    let mut s = Studio::new();
    let region = s.rectangle(60.0, 40.0);
    s.add("sheetMetalStart", |p| {
        p.set("process", en("THICKEN"));
        p.set("regions", ParamValue::Query(vec![region]));
        p.set("thickness", expr("2 mm"));
        p.set("radius", expr("2 mm"));
    });
    let r = s.ok();
    let edges = vec![edge_at(&r, DVec3::new(30.0, 0.0, 2.0)), edge_at(&r, DVec3::new(0.0, 20.0, 2.0))];
    s.add("sheetMetalFlange", |p| {
        p.set("edges", ParamValue::Query(edges));
        p.set("distance", expr("20 mm"));
    });
    s
}

#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn flanges_on_adjacent_edges_match_live() {
    let r = g34().ok();
    let faces = r.context.bodies[part(&r)].shape.faces().len();
    assert!((volume_mm3(&r) - 8511.4987).abs() < 1e-3 && faces == 24, "{} mm³, {} faces", volume_mm3(&r), faces);
}

/// A flange leaves the plate's other top edges their references (std rebuilds
/// the part: its faces kept their names only by being on the old ones), so in
/// the Flange dialog's preview the next edge can still be picked (the preview's
/// own geometry can't).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn flange_keeps_the_other_edges_references() {
    let mut s = Studio::new();
    let region = s.rectangle(60.0, 40.0);
    s.add("sheetMetalStart", |p| {
        p.set("process", en("THICKEN"));
        p.set("regions", ParamValue::Query(vec![region]));
        p.set("thickness", expr("2 mm"));
        p.set("radius", expr("2 mm"));
    });
    let r = s.ok();
    let side = edge_at(&r, DVec3::new(0.0, 20.0, 2.0));
    let edge = edge_at(&r, DVec3::new(30.0, 0.0, 2.0));
    s.add("sheetMetalFlange", |p| {
        p.set("edges", ParamValue::Query(vec![edge]));
        p.set("distance", expr("20 mm"));
    });
    let r = s.ok();
    assert_eq!(edge_at(&r, DVec3::new(0.0, 20.0, 2.0)), side);
}

/// qa5 3.2: a default Hem on G34's top edge at x = 60 (live: 9378.1835 mm³,
/// 33 faces: 27 plane, 6 cylinder).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn hem_on_g34_matches_live() {
    let mut s = g34();
    let r = s.ok();
    let edge = edge_at(&r, DVec3::new(60.0, 20.0, 2.0));
    s.add("sheetMetalHem", |p| p.set("edges", ParamValue::Query(vec![edge])));
    let r = s.ok();
    let faces = r.context.bodies[part(&r)].shape.faces().len();
    let bodies: Vec<String> = r.context.bodies.iter().chain(r.context.fs_hidden_bodies.iter()).map(|b| format!("{:?} {:?} {} faces {:.4} mm³", b.kind, b.shape.bounding_box(true).map(|x| (x.min * 1e3, x.max * 1e3)), b.shape.faces().len(), b.shape.volume().unwrap_or(0.0) * 1e9)).collect();
    assert!((volume_mm3(&r) - 9378.1835).abs() < 1e-3 && faces == 33, "{} mm³, {} faces: {:#?} bodies {bodies:#?} hem {:?} {:?}", volume_mm3(&r), faces, face_report(&r), r.display.features.last().map(|f| (f.status, f.message.clone())), s.ps.features.last().and_then(|f| noshape_model::custom::run::output_of(f.id)).map(|o| o.notices));
}

/// The part's face through `p` (mm).
fn face_at(r: &RegenResult, p: DVec3) -> Reference {
    let b = &r.context.bodies[part(r)];
    for (i, f) in b.shape.faces().iter().enumerate() {
        let v = noshape_kernel::make_vertex(p * 1e-3).unwrap();
        if noshape_kernel::distance(&v, f).is_ok_and(|d| d.distance < 1e-9) {
            return Reference::Face((*b.faces[i]).clone());
        }
    }
    panic!("no face at {p:?}");
}

/// qa5 SM2: a Tab (Sketch 2 on Top: x 20..40, y 40..50) merged into the
/// plate's top face (live: OK, like sm8 dom/fs-smprobe3-8.txt; ours said
/// "The tab must lie in the plane of a sheet metal wall").
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn tab_merges_into_the_top_face() {
    let mut s = g34();
    let r = s.ok();
    let top = face_at(&r, DVec3::new(30.0, 20.0, 2.0));
    let mut sk = noshape_sketch::Sketch::new();
    let p = [(20.0, 40.0), (40.0, 40.0), (40.0, 50.0), (20.0, 50.0)].map(|(x, y)| sk.add_point(glam::DVec2::new(mm(x), mm(y))));
    for i in 0..4 {
        sk.add_line_between(p[i], p[(i + 1) % 4]).unwrap();
    }
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    });
    let r = s.ok();
    let sk = r.display.sketches.iter().find(|x| x.feature == sketch).expect("sketch");
    let key = sk.regions.iter().max_by(|a, b| a.area.total_cmp(&b.area)).expect("region").key.clone();
    s.add("sheetMetalTab", |p| {
        p.set("tabFaces", ParamValue::Query(vec![Reference::SketchRegion { sketch, key }]));
        p.set("booleanUnionScope", ParamValue::Query(vec![top]));
    });
    let r = s.ok();
    // The tab adds 20 × 10 × 2 mm to the part.
    assert!((volume_mm3(&r) - (8511.4987 + 400.0)).abs() < 1e-3, "{} mm³ {:#?}", volume_mm3(&r), face_report(&r));
}

/// The flat pattern of G34 (std-built, live's placement: the plate stays at
/// x 0..60, y 0..40; bend lines on the bends' centres, up).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn flat_pattern_keeps_the_model_placement() {
    let r = g34().ok();
    let flat = noshape_model::sheetmetal::flat_pattern_for(&r.context, &r.context.bodies[part(&r)].id).expect("flat");
    let (lo, hi) = flat.bounds();
    let near = |a: f64, b: f64| (a - b).abs() < 1e-7;
    // Live dom/qa5-sm-flat.dxf (the same flanges): cut lines out to -18.5553
    // (20 mm flange: 16 flat + the 4.5553 bend allowance), bend lines UP at
    // -0.2777 (the allowance's centre).
    assert!(near(lo.x * 1e3, -18.5553093477) && near(lo.y * 1e3, -18.5553093477) && near(hi.x * 1e3, 60.0) && near(hi.y * 1e3, 40.0), "{lo:?} {hi:?}");
    assert_eq!(flat.loops.len(), 1);
    let mut lines: Vec<String> = flat.bend_lines.iter().map(|l| format!("{:.4} {:.4} {:.4} {:.4} {}", l.a.x * 1e3, l.a.y * 1e3, l.b.x * 1e3, l.b.y * 1e3, l.up)).collect();
    lines.sort();
    assert_eq!(lines, ["-0.2777 2.0000 -0.2777 40.0000 true", "60.0000 -0.2777 2.0000 -0.2777 true"]);
}

/// qa5 SM14: the flat's DXF as live writes it (samples/qa5-sm-flat.dxf):
/// Release 14, Onshape's whole layer table, the outline on
/// SHEETMETAL_CUT_LINES and the bend centre lines on SHEETMETAL_BEND_LINES_UP.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn flat_dxf_is_written_as_live() {
    let r = g34().ok();
    let flat = noshape_model::sheetmetal::flat_pattern_for(&r.context, &r.context.bodies[part(&r)].id).expect("flat");
    let dxf = noshape_model::sheetmetal::flat_dxf::flat_dxf(&flat, &Default::default()).unwrap();
    let doc = acadrust::DxfReader::from_reader(std::io::Cursor::new(dxf)).unwrap().read().unwrap();
    assert_eq!(doc.version, acadrust::DxfVersion::AC1014);
    assert_eq!(doc.layers.iter().count(), 48);
    let mut layers: Vec<String> = doc.entities().map(|e| format!("{} {}", e.as_entity().entity_type(), e.common().layer)).collect();
    layers.sort();
    layers.dedup_by(|a, b| a == b);
    assert_eq!(layers, ["LINE SHEETMETAL_BEND_LINES_UP", "LINE SHEETMETAL_CUT_LINES"]);
}

/// The part's vertex at `p` (mm).
fn vertex_at(r: &RegenResult, p: DVec3) -> Reference {
    let b = &r.context.bodies[part(r)];
    let i = b.shape.vertices().iter().position(|v| v.vertex_point().is_ok_and(|q| q.distance(p * 1e-3) < 1e-9)).unwrap_or_else(|| panic!("no vertex at {p:?}"));
    Reference::Vertex((*b.vertices[i]).clone())
}

/// qa5 SM3 / live G34a: a Closed corner at the inner bend corner (2, 2, 2)
/// (8535.0418 mm³, 23 faces: PLANE 17, CYLINDER 4, OTHER 2; the bends run
/// into the corner, cylinders r4 372.3144324 / r2 186.1572163 on the front,
/// and end in OTHER faces of 10.4192 mm² 0.02 mm apart across the diagonal).
/// Ours: the same faces, cylinders within 0.002 mm², but 0.12 mm³ more: our
/// bend ends are ruled radially, live's inside differs (asked live: REQUESTS
/// skasm 2026-10-30 00:20).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn closed_corner_matches_live() {
    let mut s = g34();
    let r = s.ok();
    let v = vertex_at(&r, DVec3::new(2.0, 2.0, 2.0));
    s.add("sheetMetalCorner", |p| {
        p.set("corner", ParamValue::Query(vec![v]));
        p.set("cornerStyle", en("CLOSED"));
    });
    let r = s.ok();
    let report = face_report(&r);
    let count = |kind: &str| report.iter().filter(|f| f.starts_with(kind)).count();
    assert!((volume_mm3(&r) - 8535.0418).abs() < 0.15 && (count("PLANE"), count("CYLINDER"), count("OTHER")) == (17, 4, 2), "{} mm³: {report:#?}", volume_mm3(&r));
}

/// The part's faces: type, area (mm²) and a point on each (mm), for failures.
fn face_report(r: &RegenResult) -> Vec<String> {
    r.context.bodies[part(r)]
        .shape
        .faces()
        .iter()
        .map(|f| {
            let kind = match f.face_surface() {
                Ok(noshape_kernel::Surface::Plane(_)) => "PLANE",
                Ok(noshape_kernel::Surface::Cylinder { .. }) => "CYLINDER",
                _ => "OTHER",
            };
            let [u0, u1, v0, v1] = f.uv_bounds().unwrap_or_default();
            let p = f.point_at_uv((u0 + u1) * 0.5, (v0 + v1) * 0.5).unwrap_or_default() * 1e3;
            let c = f.centroid().unwrap_or_default() * 1e3;
            format!("{kind} {:.7} at ({:.3}, {:.3}, {:.3}) centroid ({:.5}, {:.5}, {:.5})", f.area().unwrap_or(0.0) * 1e6, p.x, p.y, p.z, c.x, c.y, c.z)
        })
        .collect()
}

/// Bend (sheetMetalBend) of the 60 × 40 plate along a sketch line across
/// it at x = 30, 90°: two walls and a bend, flat as long as the plate.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn bend_keeps_flat_length() {
    let mut s = Studio::new();
    let region = s.rectangle(60.0, 40.0);
    s.add("sheetMetalStart", |p| {
        p.set("process", en("THICKEN"));
        p.set("regions", ParamValue::Query(vec![region]));
        p.set("thickness", expr("2 mm"));
        p.set("radius", expr("3 mm"));
    });
    let mut sk = noshape_sketch::Sketch::new();
    let (a, b) = (sk.add_point(glam::DVec2::new(mm(30.0), mm(-10.0))), sk.add_point(glam::DVec2::new(mm(30.0), mm(50.0))));
    let line = sk.add_line_between(a, b).unwrap().0;
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    });
    let r = s.ok();
    let top = face_at(&r, DVec3::new(30.0, 20.0, 2.0));
    s.add("sheetMetalBend", move |p| {
        p.set("bendReference", ParamValue::Query(vec![Reference::SketchEntity { sketch, entity: line }]));
        p.set("face", ParamValue::Query(vec![top]));
        p.set("bendAngle", expr("90 deg"));
    });
    let r = s.ok();
    let flat = noshape_model::sheetmetal::flat_pattern_for(&r.context, &r.context.bodies[part(&r)].id).expect("flat");
    let size = flat.size() * 1e3;
    assert!((size.x.max(size.y) - 60.0).abs() < 1e-6 && (size.x.min(size.y) - 40.0).abs() < 1e-6, "{size:?}");
}

/// Corner break: a 5 mm fillet at a free corner of a 60 × 40 × 2 plate
/// (std applies it in the flat: the definition corner becomes an arc).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn corner_break_fillets_a_free_corner() {
    let mut s = Studio::new();
    let region = s.rectangle(60.0, 40.0);
    s.add("sheetMetalStart", |p| {
        p.set("process", en("THICKEN"));
        p.set("regions", ParamValue::Query(vec![region]));
        p.set("thickness", expr("2 mm"));
        p.set("radius", expr("3 mm"));
    });
    let r = s.ok();
    let v = vertex_at(&r, DVec3::new(60.0, 40.0, 2.0));
    s.add("sheetMetalCornerBreak", |p| {
        p.set("entities", ParamValue::Query(vec![v]));
        p.set("blendType", en("FILLET"));
        p.set("radius", expr("5 mm"));
    });
    let r = s.ok();
    let expected = 60.0 * 40.0 * 2.0 - (25.0 - std::f64::consts::PI * 25.0 / 4.0) * 2.0;
    assert!((volume_mm3(&r) - expected).abs() < 1e-6, "{} mm³ vs {expected}", volume_mm3(&r));
    let flat = noshape_model::sheetmetal::flat_pattern_for(&r.context, &r.context.bodies[part(&r)].id).expect("flat");
    assert!((flat.area() * 1e6 - expected / 2.0).abs() < 1e-4, "flat area {}", flat.area() * 1e6);
}

/// Live (qa5 workflow 27): Bend of a 60 × 40 × 2 plate, radius 2, along a
/// line (30,0)→(30,40) on its top: 4812.566370614357 mm³, 14 faces; x < 30
/// stays, the wall rises at x 29.7223453..31.7223453.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn bend_matches_live() {
    let mut s = Studio::new();
    let region = s.rectangle(60.0, 40.0);
    s.add("sheetMetalStart", |p| {
        p.set("process", en("THICKEN"));
        p.set("regions", ParamValue::Query(vec![region]));
        p.set("thickness", expr("2 mm"));
        p.set("radius", expr("2 mm"));
    });
    let mut sk = noshape_sketch::Sketch::new();
    let (a, b) = (sk.add_point(glam::DVec2::new(mm(30.0), 0.0)), sk.add_point(glam::DVec2::new(mm(30.0), mm(40.0))));
    let line = sk.add_line_between(a, b).unwrap().0;
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    });
    let r = s.ok();
    let top = face_at(&r, DVec3::new(30.0, 20.0, 2.0));
    s.add("sheetMetalBend", move |p| {
        p.set("bendReference", ParamValue::Query(vec![Reference::SketchEntity { sketch, entity: line }]));
        p.set("face", ParamValue::Query(vec![top]));
    });
    let r = s.ok();
    let body = &r.context.bodies[part(&r)];
    assert!((volume_mm3(&r) - 4812.566370614357).abs() < 1e-6, "{} mm³", volume_mm3(&r));
    assert_eq!(body.faces.len(), 14);
    let bb = body.shape.bounding_box(true).unwrap();
    // Within the box's tolerance gap (1e-7 m).
    let near = |a: f64, b: f64| (a * 1e3 - b).abs() < 2e-4;
    assert!(near(bb.min.x, 0.0) && near(bb.max.x, 31.7223453) && near(bb.max.z, 31.7223453), "{bb:?}");
}

/// SM16 (live qa5 4.2): picking a Tab's profile runs std's editing logic,
/// which fills "Flange to merge" with the wall the profile lies on and sets
/// the subtraction offset to the model's minimal clearance (0.02 mm).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn tab_editing_logic_fills_merge_face_and_offset() {
    let mut s = g34();
    let mut sk = noshape_sketch::Sketch::new();
    let p = [(20.0, 40.0), (40.0, 40.0), (40.0, 50.0), (20.0, 50.0)].map(|(x, y)| sk.add_point(glam::DVec2::new(mm(x), mm(y))));
    for i in 0..4 {
        sk.add_line_between(p[i], p[(i + 1) % 4]).unwrap();
    }
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    });
    let r = s.ok();
    let sk = r.display.sketches.iter().find(|x| x.feature == sketch).expect("sketch");
    let key = sk.regions.iter().max_by(|a, b| a.area.total_cmp(&b.area)).expect("region").key.clone();
    s.add("sheetMetalTab", |p| p.set("tabFaces", ParamValue::Query(vec![Reference::SketchRegion { sketch, key }])));
    let index = s.ps.features.len() - 1;
    let new = s.ps.features[index].params.clone();
    let changed = noshape_model::custom::std_feature::editing_logic(&s.ps, index, &ConfigValues::default(), &s.units, &RegenOptions::default(), &ParamValues::default(), &new, &["tabFaces".to_string()], true).expect("editing logic");
    assert!(matches!(changed.get("booleanUnionScope"), Some(ParamValue::Query(q)) if !q.is_empty()), "{changed:?}");
    match changed.get("booleanOffset") {
        Some(ParamValue::Expr(e)) => assert_eq!(e.text, "0.02 mm"),
        v => panic!("booleanOffset {v:?} in {changed:?}"),
    }
}

/// SM-MODIFY-JOINT (live behaviours "ui (10-24 b)"): the table's Radius
/// cell on G34's first bend adds a Modify joint (Joint = the bend's face,
/// Bend, Use model bend radius off, 3 mm) that regenerates OK: the bend's
/// faces are then r 3 / r 5.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn modify_joint_changes_a_bend_radius() {
    let mut s = g34();
    let r = s.ok();
    let bend = face_at(&r, DVec3::new(30.0, 2.0 - 2.0f64.sqrt(), 4.0 - 2.0f64.sqrt()));
    s.add("sheetMetalJoint", |p| {
        p.set("entity", ParamValue::Query(vec![bend]));
        p.set("jointType", en("BEND"));
        p.set("useDefaultRadius", ParamValue::Bool(false));
        p.set("radius", expr("3 mm"));
    });
    let r = s.ok();
    let b = &r.context.bodies[part(&r)];
    let mut radii: Vec<f64> = b.shape.faces().iter().filter_map(|f| match f.face_surface().ok()? {
        noshape_kernel::Surface::Cylinder { radius, .. } => Some((radius * 1e3 * 1e6).round() / 1e6),
        _ => None,
    }).collect();
    radii.sort_by(f64::total_cmp);
    assert!(volume_mm3(&r) > 8000.0 && radii.contains(&3.0) && radii.contains(&5.0), "{} mm³, radii {radii:?}: {:#?}", volume_mm3(&r), face_report(&r));
}

/// The same with G34's flanges as two Flange features (the sheet metal
/// table's e2e test builds it that way).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn modify_joint_after_two_flange_features() {
    let mut s = Studio::new();
    let region = s.rectangle(60.0, 40.0);
    s.add("sheetMetalStart", |p| {
        p.set("process", en("THICKEN"));
        p.set("regions", ParamValue::Query(vec![region]));
        p.set("thickness", expr("2 mm"));
        p.set("radius", expr("2 mm"));
    });
    for at in [DVec3::new(30.0, 0.0, 2.0), DVec3::new(0.0, 20.0, 2.0)] {
        let r = s.ok();
        let e = edge_at(&r, at);
        s.add("sheetMetalFlange", |p| {
            p.set("edges", ParamValue::Query(vec![e]));
            p.set("distance", expr("20 mm"));
        });
    }
    let r = s.ok();
    eprintln!("two flanges: {} mm³", volume_mm3(&r));
    let bend = face_at(&r, DVec3::new(30.0, 2.0 - 2.0f64.sqrt(), 4.0 - 2.0f64.sqrt()));
    s.add("sheetMetalJoint", |p| {
        p.set("entity", ParamValue::Query(vec![bend]));
        p.set("jointType", en("BEND"));
        p.set("useDefaultRadius", ParamValue::Bool(false));
        p.set("radius", expr("3 mm"));
    });
    let r = s.ok();
    let b = &r.context.bodies[part(&r)];
    assert!(volume_mm3(&r) > 8000.0 && b.shape.faces().len() > 20, "{} mm³: {:#?}", volume_mm3(&r), face_report(&r));
}

/// The part std rebuilds keeps its name and colour (qa: after G34's two
/// flanges the part came out orange, std's miter tools having taken the
/// colours before it).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn flanges_keep_the_part_name_and_colour() {
    let mut s = g34();
    let flange = s.ps.features.pop().unwrap();
    let before = s.ok();
    s.ps.features.push(flange);
    let after = s.ok();
    let look = |r: &RegenResult| r.display.bodies.iter().map(|b| (b.id.clone(), b.name.clone(), b.color)).collect::<Vec<_>>();
    assert_eq!(look(&after), look(&before));
}

/// A studio whose sheet metal was saved while it ran natively keeps running
/// it natively (P1: by std its Flange and Hem lost their edges, "Select edges
/// to flange.", and the part was the bare plate again).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn sheet_metal_saved_natively_stays_native() {
    let mut s = Studio::new();
    s.ps.legacy_sheet_metal = true;
    let region = s.rectangle(60.0, 40.0);
    s.add("sheetMetalStart", |p| {
        p.set("process", en("THICKEN"));
        p.set("regions", ParamValue::Query(vec![region]));
        p.set("thickness", expr("2 mm"));
        p.set("radius", expr("2 mm"));
    });
    let r = s.ok();
    let edges = vec![edge_at(&r, DVec3::new(30.0, 0.0, 2.0)), edge_at(&r, DVec3::new(0.0, 20.0, 2.0))];
    s.add("sheetMetalFlange", |p| {
        p.set("edges", ParamValue::Query(edges));
        p.set("distance", expr("20 mm"));
    });
    // The native flanges (qa5 SM1's 8383.1217; std's are live's 8511.4987).
    let r = s.ok();
    assert!((volume_mm3(&r) - 8383.1217).abs() < 1e-3, "{} mm³", volume_mm3(&r));
}

/// SM-FACES (P3; live "fs (10-23 05:00)"): Sheet metal model › Extrude of an
/// open L (0,0)–(50,0)–(50,30), Blind 20, thickness 2, R3: 3211.3274 mm³ and
/// 14 faces (12 plane, 2 cylinder): each end keeps its wall / bend / wall
/// pieces.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn extruded_l_keeps_its_end_faces_in_pieces() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let p = [(0.0, 0.0), (50.0, 0.0), (50.0, 30.0)].map(|(x, y)| sk.add_point(glam::DVec2::new(mm(x), mm(y))));
    let lines = [sk.add_line_between(p[0], p[1]).unwrap(), sk.add_line_between(p[1], p[2]).unwrap()];
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    });
    s.ok();
    s.add("sheetMetalStart", |pv| {
        pv.set("process", en("EXTRUDE"));
        pv.set("sketchCurves", ParamValue::Query(lines.iter().map(|e| Reference::SketchEntity { sketch, entity: e.0 }).collect()));
        pv.set("depth", expr("20 mm"));
        pv.set("thickness", expr("2 mm"));
        pv.set("radius", expr("3 mm"));
    });
    let r = s.ok();
    let faces = r.context.bodies[part(&r)].shape.faces().len();
    eprintln!("PROBE {} mm3 {} faces {:#?}", volume_mm3(&r), faces, face_report(&r));
    assert!((volume_mm3(&r) - 3211.3274).abs() < 1e-3 && faces == 14, "{} mm³, {} faces", volume_mm3(&r), faces);
}

/// A 60 × 40 plate (Thicken 2 mm, bend radius `radius` mm).
fn plate(radius: &str) -> Studio {
    let mut s = Studio::new();
    let region = s.rectangle(60.0, 40.0);
    let radius = radius.to_string();
    s.add("sheetMetalStart", move |p| {
        p.set("process", en("THICKEN"));
        p.set("regions", ParamValue::Query(vec![region]));
        p.set("thickness", expr("2 mm"));
        p.set("radius", expr(&radius));
    });
    s
}

/// Live (behaviours "skasm (2026-10-29) partial Outer / Inner flange",
/// request 1599 (a)): R3 plate, partial Flange on the top edge y = 40, 10 mm
/// in from both ends. Outer keeps the wall inside the outline (y 38..40),
/// Inner puts it outside (y 40..42); 22 faces each.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn partial_flange_outer_and_inner_as_live() {
    for (alignment, volume, y) in [("OUTER", 6027.561697411184, (38.0, 40.0)), ("INNER", 6204.561697411182, (40.0, 42.0))] {
        let mut s = plate("3 mm");
        let r = s.ok();
        let edge = edge_at(&r, DVec3::new(30.0, 40.0, 2.0));
        s.add("sheetMetalFlange", |p| {
            p.set("edges", ParamValue::Query(vec![edge]));
            p.set("distance", expr("20 mm"));
            p.set("flangeAlignment", en(alignment));
            p.set("isPartialFlange", ParamValue::Bool(true));
            p.set("firstBoundType", en("BLIND"));
            p.set("firstBoundOffset", expr("10 mm"));
            p.set("hasSecondBound", ParamValue::Bool(true));
            p.set("secondBoundType", en("BLIND"));
            p.set("secondBoundOffset", expr("10 mm"));
        });
        let r = s.ok();
        let faces = r.context.bodies[part(&r)].shape.faces().len();
        let bb = r.context.bodies[part(&r)].shape.bounding_box(true).unwrap();
        assert!((volume_mm3(&r) - volume).abs() < 1e-3 && faces == 22 && (bb.max.y * 1e3 - y.1).abs() < 1e-3, "{alignment}: {} mm³, {faces} faces, box {:?}: {:#?}", volume_mm3(&r), (bb.min * 1e3, bb.max * 1e3), face_report(&r));
    }
}

/// Live (request 1599 (b)): on G34's two flanges a Joint › Rip, Butt joint -
/// Direction 1 makes the wall of the first-picked flange edge run through
/// the corner (its outer face then 992 mm² for the front wall, 672 for the
/// left one), Direction 2 the other; 8576.1387 mm³ either way, 24 faces;
/// which wall end is picked doesn't matter.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn butt_joint_direction_follows_the_first_picked_flange_edge() {
    for (y0_first, style, pick, front_through) in [(true, "BUTT", (0.02, 0.0), true), (true, "BUTT", (0.0, 0.02), true), (true, "BUTT2", (0.02, 0.0), false), (false, "BUTT", (0.02, 0.0), false), (false, "BUTT2", (0.0, 0.02), true)] {
        let mut s = plate("2 mm");
        let r = s.ok();
        let (y0, x0) = (edge_at(&r, DVec3::new(30.0, 0.0, 2.0)), edge_at(&r, DVec3::new(0.0, 20.0, 2.0)));
        let edges = if y0_first { vec![y0, x0] } else { vec![x0, y0] };
        s.add("sheetMetalFlange", |p| {
            p.set("edges", ParamValue::Query(edges));
            p.set("distance", expr("20 mm"));
        });
        let r = s.ok();
        let end = edge_at(&r, DVec3::new(pick.0, pick.1, 12.0));
        s.add("sheetMetalJoint", |p| {
            p.set("entity", ParamValue::Query(vec![end]));
            p.set("jointType", en("RIP"));
            p.set("hasStyle", ParamValue::Bool(true));
            p.set("jointStyle", en(style));
        });
        let r = s.ok();
        let areas: Vec<f64> = r.context.bodies[part(&r)].shape.faces().iter().map(|f| f.area().unwrap_or(0.0) * 1e6).collect();
        let has = |a: f64| areas.iter().any(|x| (x - a).abs() < 1e-3);
        let case = format!("{} first, {style}, end at {pick:?}", if y0_first { "y = 0" } else { "x = 0" });
        assert!((volume_mm3(&r) - 8576.138684233867).abs() < 1e-3 && areas.len() == 24, "{case}: {} mm³, {} faces", volume_mm3(&r), areas.len());
        assert_eq!((has(992.0), has(672.0)), (front_through, !front_through), "{case}: {:#?}", face_report(&r));
    }
}

/// A cut on an active sheet metal part stays when a later sheet metal
/// feature rebuilds the part (Onshape cuts the definition sheet: std
/// handleSheetMetalCutAndAttribution). Plate 60 × 40, Ø10 hole at (30, 20)
/// by Extrude Remove through all, then a Flange on the y = 0 edge.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn a_cut_survives_a_later_flange() {
    let mut s = plate("2 mm");
    s.ok();
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(glam::DVec2::new(mm(30.0), mm(20.0)), mm(5.0));
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    });
    let r = s.ok();
    let sk = r.display.sketches.iter().find(|x| x.feature == sketch).expect("sketch");
    let key = sk.regions.iter().min_by(|a, b| a.area.total_cmp(&b.area)).expect("region").key.clone();
    s.add("extrude", |p| {
        p.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch, key }]));
        p.set("operationType", en("REMOVE"));
        p.set("defaultScope", ParamValue::Bool(true));
        p.set("endBound", en("THROUGH_ALL"));
    });
    let r = s.ok();
    let hole = std::f64::consts::PI * 25.0 * 2.0;
    assert!((volume_mm3(&r) - (4800.0 - hole)).abs() < 1e-3, "after the cut {} mm³", volume_mm3(&r));
    let edge = edge_at(&r, DVec3::new(10.0, 0.0, 2.0));
    s.add("sheetMetalFlange", |p| {
        p.set("edges", ParamValue::Query(vec![edge]));
        p.set("distance", expr("20 mm"));
    });
    let r = s.ok();
    let cylinders = r.context.bodies[part(&r)].shape.faces().iter().filter(|f| matches!(f.face_surface(), Ok(noshape_kernel::Surface::Cylinder { radius, .. }) if (radius - 5e-3).abs() < 1e-9)).count();
    assert_eq!(cylinders, 1, "the hole after the flange: {:#?}", face_report(&r));
    // And in the flat: the outline and the hole.
    let flat = noshape_model::sheetmetal::flat_pattern_for(&r.context, &r.context.bodies[part(&r)].id).expect("flat");
    assert_eq!(flat.loops.len(), 2, "{:?}", flat.loops.iter().map(|l| l.len()).collect::<Vec<_>>());
    // Adding to an active sheet metal part is std's error (its own text).
    let mut s2 = plate("2 mm");
    s2.ok();
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(glam::DVec2::new(mm(30.0), mm(20.0)), mm(5.0));
    let sketch = s2.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    });
    let r = s2.ok();
    let sk = r.display.sketches.iter().find(|x| x.feature == sketch).expect("sketch");
    let key = sk.regions.iter().min_by(|a, b| a.area.total_cmp(&b.area)).expect("region").key.clone();
    s2.add("extrude", |p| {
        p.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch, key }]));
        p.set("operationType", en("ADD"));
        p.set("defaultScope", ParamValue::Bool(true));
        p.set("depth", expr("10 mm"));
    });
    let r = Regenerator::new().regenerate(&s2.ps, &ConfigValues::default(), &s2.units, &RegenOptions::default());
    let last = r.display.features.last().unwrap();
    assert_eq!((last.status, last.message.as_deref()), (FeatureStatus::Error, Some("Only Remove can be used with sheet metal. Add Finish sheet metal model to allow other operations.")), "{:?}", last.message);
}

/// A Hole through an active sheet metal part stays when a later Flange
/// rebuilds it (std hole.fs: handleSheetMetalCutAndAttribution registers the
/// hole tools on the wall, cuttingToolBodyIds, for the rebuild to cut).
/// Ours: the Hole runs natively and the Flange rebuilds it away. By std it
/// stops at HOLE_NO_HITS: std then looks for the hole's circular edges on the
/// definition sheet, made by the rebuild (our rebuild cuts only the part), and
/// tracks the hole tool's faces into them (our tracking keeps entity types).
#[test]
#[ignore = "in progress (fs): the hole is lost after a later Flange (SM-HOLE-THEN-FLANGE)"]
fn a_hole_survives_a_later_flange() {
    let mut s = plate("2 mm");
    let r = s.ok();
    let body = r.context.bodies[part(&r)].id.clone();
    let mut sk = noshape_sketch::Sketch::new();
    let p = sk.add_point(glam::DVec2::new(mm(30.0), mm(20.0)));
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    });
    s.ok();
    s.add("hole", |h| {
        h.set("scope", ParamValue::Query(vec![Reference::Body(body)]));
        h.set("locations", ParamValue::Query(vec![Reference::SketchEntity { sketch, entity: p.0 }]));
        h.set("oppositeDirection", ParamValue::Bool(true));
        h.set("unitsSystem", en("METRIC"));
        h.set("styleV2", en("SIMPLE"));
        h.set("endStyleV2", en("THROUGH"));
        h.set("holeDiameterV3", expr("6 mm"));
    });
    let r = s.ok();
    let hole = std::f64::consts::PI * 9.0 * 2.0;
    assert!((volume_mm3(&r) - (4800.0 - hole)).abs() < 1e-3, "after the hole {} mm³", volume_mm3(&r));
    let edge = edge_at(&r, DVec3::new(10.0, 0.0, 2.0));
    s.add("sheetMetalFlange", |p| {
        p.set("edges", ParamValue::Query(vec![edge]));
        p.set("distance", expr("20 mm"));
    });
    let r = s.ok();
    let cylinders = r.context.bodies[part(&r)].shape.faces().iter().filter(|f| matches!(f.face_surface(), Ok(noshape_kernel::Surface::Cylinder { radius, .. }) if (radius - 3e-3).abs() < 1e-9)).count();
    assert_eq!(cylinders, 1, "the hole after the flange: {:#?}", face_report(&r));
}
