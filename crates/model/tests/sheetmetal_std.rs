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

/// Live request 1614 (a): the same Tab merged into the plate's bottom face
/// (z = 0, coplanar with the sketch) is accepted too: 8911.4987 mm³, 28
/// faces (row 4.3).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn tab_merges_into_the_bottom_face() {
    let mut s = g34();
    let r = s.ok();
    let bottom = face_at(&r, DVec3::new(30.0, 20.0, 0.0));
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
        p.set("booleanUnionScope", ParamValue::Query(vec![bottom]));
    });
    let r = s.ok();
    let faces = r.context.bodies[part(&r)].shape.faces().len();
    assert!((volume_mm3(&r) - 8911.4987).abs() < 1e-3 && faces == 28, "{} mm³, {faces} faces: {:#?}", volume_mm3(&r), face_report(&r));
}

/// Live request 1604, qa5: one Flange (y = 0, x = 0), a Hem on x = 60, a Tab
/// merged into the plate, a Corner at (2, 2, 2): Joint A (the rip, its edge
/// untouched since the Flange), Bend B (x = 0), Bend C (y = 0), Hem D (the
/// Hem rebuilt the plate, so its edges, then made its own; the same after
/// the Hem alone, request 1614).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn qa5_letters_follow_live() {
    use noshape_model::sheetmetal::BendKind;
    let mut s = g34();
    let r = s.ok();
    let edge = edge_at(&r, DVec3::new(60.0, 20.0, 2.0));
    s.add("sheetMetalHem", |p| p.set("edges", ParamValue::Query(vec![edge])));
    let r = s.ok();
    // The Hem alone already gives these letters (live request 1614 (b)).
    let order = |r: &RegenResult| {
        let data = r.context.sheet_metal.get(&r.context.bodies[part(r)].id).expect("data");
        let bend = |hem: bool, along_x: bool| data.bends.iter().find(|b| matches!(b.kind, BendKind::Hem) == hem && (hem || (b.axis.x.abs() > 0.9) == along_x)).map(|b| b.birth).expect("bend");
        assert_eq!(data.rips.len(), 1);
        (data.rips[0].birth, bend(false, false), bend(false, true), bend(true, false))
    };
    let (rip, x0, y0, hem) = order(&r);
    assert!(rip < x0 && x0 < y0 && y0 < hem, "Hem alone: rip {rip}, x = 0 {x0}, y = 0 {y0}, hem {hem}");
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
    let v = vertex_at(&r, DVec3::new(2.0, 2.0, 2.0));
    s.add("sheetMetalCorner", |p| p.set("corner", ParamValue::Query(vec![v])));
    let (rip, x0, y0, hem) = order(&s.ok());
    assert!(rip < x0 && x0 < y0 && y0 < hem, "rip {rip}, x = 0 {x0}, y = 0 {y0}, hem {hem}");
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
/// Live's 8535.0418 is its /massproperties figure, 0.124 low on the spline
/// bend ends; its regeneration's evVolume is 8535.1655548 (request 1606).
/// Ours is within 0.004 of that (live's x(θ) is a 50-pole fit, request 1602).
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
    assert!((volume_mm3(&r) - 8535.1655548).abs() < 0.01 && (count("PLANE"), count("CYLINDER"), count("OTHER")) == (17, 4, 2), "{} mm³: {report:#?}", volume_mm3(&r));
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

/// A Flange bent 30° (under 45°) unfolds into the flat's plane: the
/// unfold turned such a wall the wrong way (the rect → triangle loft's
/// 33.8° bend stood its walls up out of the flat). The flat is the sheet:
/// the plate, the 20 mm flange and the bend's developed strip, R 2, K 0.45.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn a_shallow_flange_unfolds_flat() {
    let mut s = plate("2 mm");
    let r = s.ok();
    let edge = edge_at(&r, DVec3::new(30.0, 0.0, 2.0));
    s.add("sheetMetalFlange", |p| {
        p.set("edges", ParamValue::Query(vec![edge]));
        p.set("distance", expr("20 mm"));
        p.set("bendAngle", expr("30 deg"));
    });
    let r = s.ok();
    let flat = noshape_model::sheetmetal::flat_pattern_for(&r.context, &r.context.bodies[part(&r)].id).expect("flat");
    let n = flat.frame.normal.normalize();
    for f in flat.shape.as_ref().expect("flat shape").faces() {
        let p = f.face_plane().ok().flatten().expect("a planar flat face");
        assert!(p.unit_normal().cross(n).length() < 1e-9, "a flat face off the flat's plane: {:?}", p.unit_normal());
    }
    // Plate and flange less their pull-backs, plus the strip: volume / 2 mm
    // less the strip's neutral-line shortfall (K 0.45 against the middle:
    // 30° × 60 mm × 0.05 × 2 mm).
    let (sheet, area) = (volume_mm3(&r) / 2.0, flat.area() * 1e6);
    let short = std::f64::consts::FRAC_PI_6 * 60.0 * 0.05 * 2.0;
    assert!((sheet - area - short).abs() < 0.01, "flat {area} mm² vs sheet {sheet} less {short}");
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

/// A cut after the flanges keeps their bends (std cuts the definition sheet;
/// the joints' attributes ride on the split and untouched edges): g34, a
/// Ø10 hole through the plate at (30, 20).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn a_cut_after_flanges_keeps_the_bends() {
    let mut s = g34();
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
        p.set("hasSecondDirection", ParamValue::Bool(true));
        p.set("secondDirectionBound", en("THROUGH_ALL"));
    });
    let r = s.ok();
    let solids = r.context.bodies.iter().filter(|b| b.kind == BodyKind::Solid).count();
    let data = r.context.sheet_metal.get(&r.context.bodies[part(&r)].id).expect("data");
    let hole = std::f64::consts::PI * 25.0 * 2.0;
    assert!(solids == 1 && data.bends.len() == 2 && (volume_mm3(&r) - (8511.4987 - hole)).abs() < 1e-3, "{solids} solids, {} bends, {} mm³", data.bends.len(), volume_mm3(&r));
}

/// qa4 round 4 S1 (live behaviours "qa4 round 4 s1"): 100 × 60 plate,
/// 1.5 mm, R 1.5; Flange 30 mm on y = 0, y = 60, x = 100: 20295.0752 mm³,
/// 34 faces; Hem on the y = 0 flange's top: 22074.8607, 43; a Ø12 circle on
/// Front at (50, 1.5) cut through all both ways slots the plate and both
/// long bends: 20805.4123, 61.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn qa4_s1_enclosure_as_live() {
    let mut s = Studio::new();
    let region = s.rectangle(100.0, 60.0);
    s.add("sheetMetalStart", |p| {
        p.set("process", en("THICKEN"));
        p.set("regions", ParamValue::Query(vec![region]));
        p.set("thickness", expr("1.5 mm"));
        p.set("radius", expr("1.5 mm"));
    });
    let r = s.ok();
    let edges = vec![edge_at(&r, DVec3::new(50.0, 0.0, 1.5)), edge_at(&r, DVec3::new(50.0, 60.0, 1.5)), edge_at(&r, DVec3::new(100.0, 30.0, 1.5))];
    s.add("sheetMetalFlange", |p| {
        p.set("edges", ParamValue::Query(edges));
        p.set("distance", expr("30 mm"));
    });
    let r = s.ok();
    let faces = |r: &RegenResult| r.context.bodies[part(r)].shape.faces().len();
    let flange = (volume_mm3(&r), faces(&r));
    let edge = edge_at(&r, DVec3::new(50.0, 0.0, 30.0));
    s.add("sheetMetalHem", |p| p.set("edges", ParamValue::Query(vec![edge])));
    let r = s.ok();
    let hem = (volume_mm3(&r), faces(&r));
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(glam::DVec2::new(mm(50.0), mm(1.5)), mm(6.0));
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Front)]));
        pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    });
    let r = s.ok();
    let sk = r.display.sketches.iter().find(|x| x.feature == sketch).expect("sketch");
    let key = sk.regions[0].key.clone();
    s.add("extrude", |p| {
        p.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch, key }]));
        p.set("operationType", en("REMOVE"));
        p.set("defaultScope", ParamValue::Bool(true));
        p.set("endBound", en("THROUGH_ALL"));
        p.set("hasSecondDirection", ParamValue::Bool(true));
        p.set("secondDirectionBound", en("THROUGH_ALL"));
    });
    let r = s.ok();
    let cut = (volume_mm3(&r), faces(&r));
    let fp = noshape_model::sheetmetal::flat_pattern_for(&r.context, &r.context.bodies[part(&r)].id).expect("flat");
    let (lo, hi) = fp.bounds();
    // Live: the flat 128.9165 × 129.4650 (FLAT-ROT: not turned), 6 bend lines
    // (FLAT-SEG: the slot splits both long bends; the x = 100 bend; the hem).
    let size = (hi - lo) * 1e3;
    assert!((size.x - 128.9165).abs() < 1e-3 && (size.y - 129.4650).abs() < 1e-3 && fp.bend_lines.len() == 6, "flat {size:?}, {} bend lines", fp.bend_lines.len());
    let ok = |(v, n): (f64, usize), (lv, ln): (f64, usize)| (v - lv).abs() < 1e-3 && n == ln;
    assert!(ok(flange, (20295.0752, 34)) && ok(hem, (22074.8607, 43)) && ok(cut, (20805.4123, 61)), "flange {flange:?}, hem {hem:?}, cut {cut:?}: {:#?}", face_report(&r));
    // Live's table (behaviours "qa4 round 4 s1" (b)): Bend A (x = 100),
    // Hem D, Bend E, Bend F; Other joints Joint B, Joint C. A long bend the
    // slot split is one row: its pieces share the joint.
    use noshape_model::sheetmetal::BendKind;
    let data = r.context.sheet_metal.get(&r.context.bodies[part(&r)].id).expect("data");
    let long: Vec<_> = data.bends.iter().filter(|b| !matches!(b.kind, BendKind::Hem) && b.axis.x.abs() > 0.9).collect();
    let mut joints: Vec<_> = long.iter().map(|b| b.joint.clone().expect("joint id")).collect();
    joints.sort();
    joints.dedup();
    let birth = |j: &std::sync::Arc<str>| long.iter().filter(|b| b.joint.as_ref() == Some(j)).map(|b| b.birth).min().unwrap_or(0);
    let x100 = data.bends.iter().find(|b| b.axis.y.abs() > 0.9).map(|b| b.birth).expect("x = 100 bend");
    let hem_b = data.bends.iter().find(|b| matches!(b.kind, BendKind::Hem)).map(|b| b.birth).expect("hem");
    let rips: Vec<u64> = data.rips.iter().map(|r| r.birth).collect();
    let longs: Vec<u64> = joints.iter().map(birth).collect();
    assert!(
        long.len() == 4 && joints.len() == 2 && rips.len() == 2 && rips.iter().all(|&b| x100 < b && b < hem_b) && longs.iter().all(|&b| hem_b < b),
        "{} long pieces, {} joints; x = 100 {x100}, rips {rips:?}, hem {hem_b}, long {longs:?}",
        long.len(),
        joints.len()
    );
}

/// The Form dialog's editing logic (std formedEditLogic) fills Target
/// face(s) from the location: a point on the plate's top gives the top face
/// (live 1616 f-5: "Face of Sheet metal model 1").
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn form_editing_logic_fills_the_target_face() {
    for on_face in [false, true] {
        let mut s = plate("2 mm");
        let r = s.ok();
        let top = face_at(&r, DVec3::new(30.0, 20.0, 2.0));
        let plane = if on_face {
            top.clone()
        } else {
            let pl = s.add("cPlane", |p| {
                p.set("entities", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
                p.set("offset", expr("2 mm"));
            });
            Reference::FeaturePlane(pl)
        };
        // The point at world (30, 20, 2), in the sketch's own frame.
        let sketch = s.add("sketch", |pv| {
            pv.set("sketchPlane", ParamValue::Query(vec![plane]));
            pv.set("sketch", ParamValue::Sketch(Box::default()));
        });
        let r = s.ok();
        let frame = r.display.sketches.iter().find(|x| x.feature == sketch).expect("sketch").frame;
        let w = DVec3::new(mm(30.0), mm(20.0), mm(2.0)) - frame.origin;
        let mut sk = noshape_sketch::Sketch::new();
        let point = sk.add_point(glam::DVec2::new(w.dot(frame.x), w.dot(frame.y)));
        let f = s.ps.features.iter_mut().find(|f| f.id == sketch).expect("sketch feature");
        f.params.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
        s.ok();
        s.add("sheetMetalFormed", |p| {
            p.set("formPartStudio", ParamValue::Text("library:circle-emboss".into()));
            p.set("locations", ParamValue::Query(vec![Reference::SketchEntity { sketch, entity: point.0 }]));
        });
        let index = s.ps.features.len() - 1;
        let new = s.ps.features[index].params.clone();
        // As the dialog asks: the Form Part Studio picked first, then the location.
        let mut before = new.clone();
        before.set("locations", ParamValue::Query(Vec::new()));
        let specified = ["formPartStudio".to_string(), "locations".to_string()];
        let changed = noshape_model::custom::std_feature::editing_logic(&s.ps, index, &ConfigValues::default(), &s.units, &RegenOptions::default(), &before, &new, &specified, true).expect("editing logic");
        assert!(matches!(changed.get("targetFaces"), Some(ParamValue::Query(q)) if q.len() == 1), "on face {on_face}: {changed:?}");
    }
}

/// The form library's studios' tools (Add, Subtract material, mm³) in the
/// configurations live measured them in (dom/sm-forms-v4, currentConfiguration).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn library_form_tools_as_live() {
    let _ = Studio::new();
    // Live's figures are /massproperties: on the Louver's spheres and tori
    // 0.0006 over the exact 1975.3619777 (Pappus on its section), ours.
    for (form, values, add, sub, tol) in [
        ("circle-emboss", vec![], 273.79979, 217.909344, 1e-5),
        ("circle-extrusion", vec![], 15.177413, 38.48451, 1e-5),
        ("circle-knockout", vec![], 491.604683, 490.873852, 1e-5),
        ("countersink", vec![("clearance_hole", "1 mm"), ("angle", "60 deg")], 1.491592, 1.774456, 1e-5),
        ("slot-extrusion", vec![], 172.257045, 738.48451, 1e-5),
        // Slot tangents: live's Length = length - Width (its option id
        // `tangents`), so 105 mm to the tangents is the 100 mm above.
        ("slot-extrusion", vec![("Length to", "tangents"), ("length", "105 mm")], 172.257045, 738.48451, 1e-5),
        ("louver", vec![("height", "5 mm")], 1975.3626, 1889.368549, 1e-3),
        ("bridge-lance", vec![("length", "100 mm"), ("width", "1 mm")], 115.3687, 111.656854, 1e-4),
        ("lance", vec![("length", "100 mm"), ("width", "1 mm"), ("draft", "90 deg")], 107.853982, 107.853982, 1e-5),
        ("lance", vec![("length", "100 mm"), ("width", "1 mm"), ("draft", "90 deg"), ("Profile", "Straight")], 6.926991, 7.712389, 1e-5),
        ("slot-emboss", vec![], 2310.669816, 1883.594769, 1e-3),
        ("slot-emboss", vec![("Length to", "slot_tangents")], 2208.826315, 1800.310497, 1e-3),
    ] {
        let studio = noshape_model::sheetmetal::forms::studio(form).expect("studio");
        let mut config = studio.configuration.defaults();
        config.0.extend(values.into_iter().map(|(k, v)| (k.to_string(), v.to_string())));
        let r = Regenerator::new().regenerate(&studio, &config, &UnitPrefs::default(), &RegenOptions::default());
        let st: Vec<_> = r.display.features.iter().map(|f| (f.status, f.message.clone())).collect();
        let v: Vec<f64> = r.context.bodies.iter().map(|b| b.shape.volume().unwrap_or(0.0) * 1e9).collect();
        assert!(v.len() == 2 && (v[0] - add).abs() < tol && (v[1] - sub).abs() < tol, "{form}: {v:?} vs {add} / {sub}; {st:?}");
    }
    // Every form builds both tools at its defaults.
    for form in noshape_model::sheetmetal::forms::FORMS {
        let studio = noshape_model::sheetmetal::forms::studio(form.id).expect("studio");
        let r = Regenerator::new().regenerate(&studio, &studio.configuration.defaults(), &UnitPrefs::default(), &RegenOptions::default());
        let v: Vec<f64> = r.context.bodies.iter().map(|b| b.shape.volume().unwrap_or(0.0) * 1e9).collect();
        assert!(v.len() == 2 && v.iter().all(|x| *x > 0.0), "{}: {v:?}", form.id);
    }
}

/// The form library's Circle emboss with its defaults on a 2 mm plate (live
/// "live form emboss", behaviours "skasm (2026-10-30) Onshape sheet metal
/// forms library"): 9694.552619769172 mm³, 14 faces (PLANE 8, TORUS 4,
/// CONE 2), the top a 0.0147 mm disc.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn library_circle_emboss_as_live() {
    let mut s = Studio::new();
    let region = s.rectangle(80.0, 60.0);
    s.add("sheetMetalStart", |p| {
        p.set("process", en("THICKEN"));
        p.set("regions", ParamValue::Query(vec![region]));
        p.set("thickness", expr("2 mm"));
        p.set("radius", expr("2 mm"));
    });
    let r = s.ok();
    let top = face_at(&r, DVec3::new(40.0, 30.0, 2.0));
    let mut sk = noshape_sketch::Sketch::new();
    let point = sk.add_point(glam::DVec2::new(mm(40.0), mm(30.0)));
    // On a plane at the top face (a sketch on the face itself has its x
    // axis along -X here).
    let plane = s.add("cPlane", |p| {
        p.set("entities", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        p.set("offset", expr("2 mm"));
    });
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::FeaturePlane(plane)]));
        pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    });
    s.add("sheetMetalFormed", |p| {
        p.set("formPartStudio", ParamValue::Text("library:circle-emboss".into()));
        p.set("locations", ParamValue::Query(vec![Reference::SketchEntity { sketch, entity: point.0 }]));
        p.set("targetFaces", ParamValue::Query(vec![top]));
    });
    let r = s.ok();
    let faces = r.context.bodies[part(&r)].shape.faces();
    let kinds = faces.iter().fold([0usize; 4], |mut n, f| {
        n[match f.face_surface() {
            Ok(noshape_kernel::Surface::Plane(_)) => 0,
            Ok(noshape_kernel::Surface::Torus { .. }) => 1,
            Ok(noshape_kernel::Surface::Cone { .. }) => 2,
            _ => 3,
        }] += 1;
        n
    });
    // The flat: the form's footprint (r 9.1569, live's DXF FORM_UP circle) once.
    let fp = noshape_model::sheetmetal::flat_pattern_for(&r.context, &r.context.bodies[part(&r)].id).expect("flat");
    let radii: Vec<f64> = fp.forms.iter().flat_map(|f| f.loops.iter().map(|l| l.iter().map(|p| (*p - f.center).length() * 1e3).fold(0.0, f64::max))).collect();
    assert_eq!(radii.iter().filter(|r| (**r - 9.156854).abs() < 1e-3).count(), 1, "form loops {radii:?}");
    assert!((volume_mm3(&r) - 9694.552619769172).abs() < 1e-3 && kinds == [8, 4, 2, 0], "{} mm³, PLANE/TORUS/CONE/other {kinds:?}: {:#?}", volume_mm3(&r), face_report(&r));
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
/// Ours: the native Hole cuts active sheet metal parts through std's
/// booleanBodies (its sheet metal aware Boolean cuts the definition), so the
/// hole lasts.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
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
    let flat = noshape_model::sheetmetal::flat_pattern_for(&r.context, &r.context.bodies[part(&r)].id).expect("flat");
    assert_eq!(flat.loops.len(), 2, "the hole in the flat too");
}

/// Live ("qa5 sm tools", Jog with all defaults: Blind 5 mm, anchor Inside,
/// Preserve material) on the Bend test's plate and line: 4825.132741228719
/// mm³, 22 faces (18 plane, 4 cylinder); the raised flange at z 7..9 ends
/// at x 55.8893813 (flat length kept).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn jog_matches_live() {
    let mut s = plate("2 mm");
    let mut sk = noshape_sketch::Sketch::new();
    let (a, b) = (sk.add_point(glam::DVec2::new(mm(30.0), 0.0)), sk.add_point(glam::DVec2::new(mm(30.0), mm(40.0))));
    let line = sk.add_line_between(a, b).unwrap().0;
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    });
    let r = s.ok();
    let top = face_at(&r, DVec3::new(30.0, 20.0, 2.0));
    s.add("sheetMetalJog", move |p| {
        p.set("bendReference", ParamValue::Query(vec![Reference::SketchEntity { sketch, entity: line }]));
        p.set("face", ParamValue::Query(vec![top]));
    });
    let r = s.ok();
    let body = &r.context.bodies[part(&r)];
    let bb = body.shape.bounding_box(true).unwrap();
    let near = |a: f64, b: f64| (a * 1e3 - b).abs() < 2e-4;
    assert!((volume_mm3(&r) - 4825.132741228719).abs() < 1e-6 && body.faces.len() == 22 && near(bb.max.z, 9.0) && near(bb.max.x, 55.8893813), "{} mm³, {} faces, {bb:?}: {:#?}", volume_mm3(&r), body.faces.len(), face_report(&r));
}

/// Live ("qa5 sm tools"): a Tab (60,10)–(75,30) beside the plate merges into
/// the plate (5400 mm³, 10 faces); without a flange to merge it fails "Select
/// a sheet metal wall to join the tab to." and adds nothing.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn tab_beside_the_plate_as_live() {
    for merge in [true, false] {
        let mut s = plate("2 mm");
        let mut sk = noshape_sketch::Sketch::new();
        let p = [(60.0, 10.0), (75.0, 10.0), (75.0, 30.0), (60.0, 30.0)].map(|(x, y)| sk.add_point(glam::DVec2::new(mm(x), mm(y))));
        for i in 0..4 {
            sk.add_line_between(p[i], p[(i + 1) % 4]).unwrap();
        }
        let plane = s.add("cPlane", |pv| {
            pv.set("entities", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
            pv.set("offset", expr("2 mm"));
        });
        let sketch = s.add("sketch", |pv| {
            pv.set("sketchPlane", ParamValue::Query(vec![Reference::FeaturePlane(plane)]));
            pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
        });
        let r = s.ok();
        let sk = r.display.sketches.iter().find(|x| x.feature == sketch).expect("sketch");
        let key = sk.regions.iter().max_by(|a, b| a.area.total_cmp(&b.area)).expect("region").key.clone();
        let top = face_at(&r, DVec3::new(30.0, 20.0, 2.0));
        s.add("sheetMetalTab", |p| {
            p.set("tabFaces", ParamValue::Query(vec![Reference::SketchRegion { sketch, key }]));
            p.set("booleanUnionScope", ParamValue::Query(if merge { vec![top] } else { Vec::new() }));
            p.set("booleanOffset", expr("0.02 mm"));
        });
        let r = Regenerator::new().regenerate(&s.ps, &ConfigValues::default(), &s.units, &RegenOptions::default());
        let last = r.display.features.last().unwrap();
        if merge {
            assert_eq!(last.status, FeatureStatus::Ok, "{:?}", last.message);
            assert!((volume_mm3(&r) - 5400.0).abs() < 1e-6 && r.context.bodies[part(&r)].faces.len() == 10, "{} mm³ {:#?}", volume_mm3(&r), face_report(&r));
        } else {
            assert_eq!((last.status, last.message.as_deref()), (FeatureStatus::Error, Some("Select a sheet metal wall to join the tab to.")));
            assert!((volume_mm3(&r) - 4800.0).abs() < 1e-6);
        }
    }
}

/// SM-CBORE-THEN-FLANGE: a counterbore Hole (Ø4 through, Ø8 × 1 from the
/// top) on an active sheet metal part stays when a later Flange rebuilds
/// it (its tool registered on the wall, as std's Hole does); the flat shows
/// the through hole and the head (live dx 10-27 flat pattern line styles).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn a_counterbore_survives_a_later_flange() {
    let mut s = plate("2 mm");
    let r = s.ok();
    let body = r.context.bodies[part(&r)].id.clone();
    let plane = s.add("cPlane", |pv| {
        pv.set("entities", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("offset", expr("2 mm"));
    });
    let mut sk = noshape_sketch::Sketch::new();
    let p = sk.add_point(glam::DVec2::new(mm(30.0), mm(20.0)));
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::FeaturePlane(plane)]));
        pv.set("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })));
    });
    s.ok();
    s.add("hole", |h| {
        h.set("scope", ParamValue::Query(vec![Reference::Body(body)]));
        h.set("locations", ParamValue::Query(vec![Reference::SketchEntity { sketch, entity: p.0 }]));
        h.set("unitsSystem", en("METRIC"));
        h.set("styleV2", en("C_BORE"));
        h.set("endStyleV2", en("THROUGH"));
        h.set("holeDiameterV3", expr("4 mm"));
        h.set("cBoreDiameterV3", expr("8 mm"));
        h.set("cBoreDepthV3", expr("1 mm"));
    });
    let r = s.ok();
    let pi = std::f64::consts::PI;
    let cut = pi * 4.0 * 1.0 + pi * 16.0 * 1.0;
    let bodies: Vec<String> = r.context.bodies.iter().map(|b| format!("{:?} {:?} {:.3}", b.id, b.kind, b.shape.volume().unwrap_or(0.0) * 1e9)).collect();
    assert!((volume_mm3(&r) - (4800.0 - cut)).abs() < 1e-3, "after the hole {} mm³: {bodies:#?} {:?} {:?}", volume_mm3(&r), r.display.features.last().map(|f| (f.status, f.message.clone())), s.ps.features.last().and_then(|f| noshape_model::custom::run::output_of(f.id)).map(|o| o.notices));
    let edge = edge_at(&r, DVec3::new(10.0, 0.0, 2.0));
    s.add("sheetMetalFlange", |p| {
        p.set("edges", ParamValue::Query(vec![edge]));
        p.set("distance", expr("20 mm"));
    });
    let r = s.ok();
    let radii: Vec<f64> = r.context.bodies[part(&r)].shape.faces().iter().filter_map(|f| match f.face_surface() { Ok(noshape_kernel::Surface::Cylinder { radius, .. }) if radius < 5e-3 => Some(radius * 1e3), _ => None }).collect();
    assert!(radii.iter().any(|r| (r - 2.0).abs() < 1e-6) && radii.iter().any(|r| (r - 4.0).abs() < 1e-6), "the counterbore after the flange: {radii:?} {:#?}", face_report(&r));
    let flat = noshape_model::sheetmetal::flat_pattern_for(&r.context, &r.context.bodies[part(&r)].id).expect("flat");
    assert_eq!(flat.loops.len(), 2, "outline and the through hole");
    assert_eq!(flat.heads.len(), 1, "{:?}", flat.heads);
    assert!((flat.heads[0].radius - 4e-3).abs() < 1e-6, "{:?}", flat.heads);
}

/// qa5 5.4 (live behaviours "qa5 sheet metal"): a Corner on a vertex that is
/// no sheet metal corner (the top of G34's front wall end) fails "Sheet metal
/// corner not found.".
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn corner_off_a_corner_fails_as_live() {
    let mut s = g34();
    let r = s.ok();
    let v = vertex_at(&r, DVec3::new(0.02, 0.0, 20.0));
    s.add("sheetMetalCorner", |p| p.set("corner", ParamValue::Query(vec![v])));
    let r = Regenerator::new().regenerate(&s.ps, &ConfigValues::default(), &s.units, &RegenOptions::default());
    let last = r.display.features.last().unwrap();
    assert_eq!((last.status, last.message.as_deref()), (FeatureStatus::Error, Some("Sheet metal corner not found.")));
}

/// Live request 1604: the table letters follow the order the joints'
/// definition edges appeared. One Flange on the y = 0 and x = 0 edges, in
/// either pick order: Bend A = y = 0, Bend B = x = 0, Joint C = the corner rip;
/// two Flange features: A = y = 0, B = x = 0, no joint.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn joint_births_follow_live_letters() {
    for (case, picks) in [("a", vec!["y0", "x0"]), ("b", vec!["x0", "y0"]), ("c", vec!["y0+", "x0+"])] {
        let mut s = plate("2 mm");
        let r = s.ok();
        let edge = |r: &RegenResult, w: &str| if w.starts_with("y0") { edge_at(r, DVec3::new(30.0, 0.0, 2.0)) } else { edge_at(r, DVec3::new(0.0, 20.0, 2.0)) };
        if case == "c" {
            for w in &picks {
                let r = s.ok();
                let e = edge(&r, w);
                s.add("sheetMetalFlange", |p| {
                    p.set("edges", ParamValue::Query(vec![e]));
                    p.set("distance", expr("20 mm"));
                });
            }
        } else {
            let edges: Vec<Reference> = picks.iter().map(|w| edge(&r, w)).collect();
            s.add("sheetMetalFlange", |p| {
                p.set("edges", ParamValue::Query(edges));
                p.set("distance", expr("20 mm"));
            });
        }
        let r = s.ok();
        let data = r.context.sheet_metal.get(&r.context.bodies[part(&r)].id).expect("data");
        let along_x = data.bends.iter().find(|b| b.axis.x.abs() > 0.9).expect("y = 0 bend").birth;
        let along_y = data.bends.iter().find(|b| b.axis.y.abs() > 0.9).expect("x = 0 bend").birth;
        assert!(along_x < along_y, "{case}: y = 0 bend {along_x}, x = 0 bend {along_y}");
        if case == "c" {
            assert!(data.rips.is_empty(), "{case}: {} rips", data.rips.len());
        } else {
            assert!(data.rips.len() == 1 && data.rips[0].birth > along_y, "{case}: rips {:?}", data.rips.iter().map(|r| r.birth).collect::<Vec<_>>());
        }
    }
}

