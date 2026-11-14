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
        self.rectangle_on(DatumPlane::Top, w, h)
    }

    /// A rectangle (0,0)–(w,h) mm on a default plane as a sketch; its region.
    fn rectangle_on(&mut self, plane: DatumPlane, w: f64, h: f64) -> Reference {
        let mut sk = noshape_sketch::Sketch::new();
        let p = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)].map(|(x, y)| sk.add_point(glam::DVec2::new(mm(x), mm(y))));
        for i in 0..4 {
            sk.add_line_between(p[i], p[(i + 1) % 4]).unwrap();
        }
        let sketch = self.add("sketch", |pv| {
            pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(plane)]));
            pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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

/// Live G34 (b): after (a)'s Closed corner, a Joint on the corner rip picked
/// as the front wall's end edge, Rip, Butt joint - Direction 1: the front
/// wall runs through to x = −2 (its faces 992 mm²), Direction 2 the left one
/// (672 mm²); both bends are re-closed against the through wall, each end
/// one Bézier in the flat (live 1670 / 1676 / 1680): regen evVolume HIGH
/// 8601.820762 / 8601.821483 mm³, 23 faces, the through bend's end face
/// 13.3875875 mm², the other's 10.9812215.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn butt_joint_after_a_closed_corner_matches_live() {
    for (style, live, through) in [("BUTT", 8601.820762198602, 992.0), ("BUTT2", 8601.821482534566, 672.0)] {
        let mut s = g34();
        let r = s.ok();
        let v = vertex_at(&r, DVec3::new(2.0, 2.0, 2.0));
        s.add("sheetMetalCorner", |p| {
            p.set("corner", ParamValue::Query(vec![v]));
            p.set("cornerStyle", en("CLOSED"));
        });
        let r = s.ok();
        let end = edge_at(&r, DVec3::new(0.02, 0.0, 12.0));
        s.add("sheetMetalJoint", |p| {
            p.set("entity", ParamValue::Query(vec![end]));
            p.set("jointType", en("RIP"));
            p.set("hasStyle", ParamValue::Bool(true));
            p.set("jointStyle", en(style));
        });
        let r = s.ok();
        let areas: Vec<f64> = r.context.bodies[part(&r)].shape.faces().iter().map(|f| f.area().unwrap_or(0.0) * 1e6).collect();
        let has = |a: f64, tol: f64| areas.iter().any(|x| (x - a).abs() < tol);
        let n = areas.iter().filter(|x| (*x - through).abs() < 1e-3).count();
        eprintln!("{style}: {} mm³ (live {live}), {} faces", volume_mm3(&r), areas.len());
        assert!((volume_mm3(&r) - live).abs() < 0.01 && n == 2 && areas.len() == 23 && has(13.3875875, 1e-3) && has(10.9812215, 1e-3), "{style}: {} mm³ (live {live}), {n} faces of {through} mm²: {:#?}", volume_mm3(&r), face_report(&r));
        // The flat (live flat1676-butt-D1.dxf): the bend centre lines stop
        // where the bend ends cross them.
        if style == "BUTT" {
            let flat = noshape_model::sheetmetal::flat_pattern_for(&r.context, &r.context.bodies[part(&r)].id).expect("flat");
            let ends: Vec<glam::DVec2> = flat.bend_lines.iter().flat_map(|l| [l.a * 1e3, l.b * 1e3]).collect();
            for want in [glam::DVec2::new(0.171619, -0.277655), glam::DVec2::new(-0.277655, 0.908308)] {
                assert!(ends.iter().any(|e| e.distance(want) < 1e-3), "{want:?} not among {ends:?}");
            }
        }
    }
}

/// Live 1680: the butt joint after a Closed corner at another minimal gap,
/// bend radius and thickness. The flat's 45° plate-corner cut runs from
/// (P, R) to (R, P) with P = R + (K T + gap) / √2 (live's DXFs), and the
/// volume is live's Mass properties figure (its /massproperties accuracy:
/// 0.1 under its evVolume in the default case).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn butt_joint_plate_point_follows_gap_radius_and_thickness() {
    for (t, r_bend, gap, p_live, v_live) in [(2.0, 2.0, 0.1, 2.707107, 8598.152), (2.0, 4.0, 0.02, 4.650538, 8413.152), (1.0, 2.0, 0.02, 2.332340, 4307.62)] {
        let mut s = Studio::new();
        let region = s.rectangle(60.0, 40.0);
        s.add("sheetMetalStart", |p| {
            p.set("process", en("THICKEN"));
            p.set("regions", ParamValue::Query(vec![region]));
            p.set("thickness", expr(&format!("{t} mm")));
            p.set("radius", expr(&format!("{r_bend} mm")));
            p.set("minimalClearance", expr(&format!("{gap} mm")));
        });
        let r = s.ok();
        let edges = vec![edge_at(&r, DVec3::new(30.0, 0.0, t)), edge_at(&r, DVec3::new(0.0, 20.0, t))];
        s.add("sheetMetalFlange", |p| {
            p.set("edges", ParamValue::Query(edges));
            p.set("distance", expr("20 mm"));
        });
        let r = s.ok();
        // The inner bend corner: the vertex nearest (R, R, T).
        let b = &r.context.bodies[part(&r)];
        let corner = DVec3::new(r_bend, r_bend, t) * 1e-3;
        let i = (0..b.vertices.len()).min_by(|x, y| {
            let d = |k: usize| b.shape.vertices()[k].vertex_point().map_or(f64::INFINITY, |q| q.distance(corner));
            d(*x).total_cmp(&d(*y))
        });
        let v = Reference::Vertex((*b.vertices[i.unwrap()]).clone());
        s.add("sheetMetalCorner", |p| {
            p.set("corner", ParamValue::Query(vec![v]));
            p.set("cornerStyle", en("CLOSED"));
        });
        let r = s.ok();
        let end = edge_at(&r, DVec3::new(gap, 0.0, 12.0));
        s.add("sheetMetalJoint", |p| {
            p.set("entity", ParamValue::Query(vec![end]));
            p.set("jointType", en("RIP"));
            p.set("hasStyle", ParamValue::Bool(true));
            p.set("jointStyle", en("BUTT"));
        });
        let r = s.ok();
        let case = format!("T {t} R {r_bend} gap {gap}");
        assert!((volume_mm3(&r) - v_live).abs() < 0.2, "{case}: {} mm³ (live {v_live})", volume_mm3(&r));
        let flat = noshape_model::sheetmetal::flat_pattern_for(&r.context, &r.context.bodies[part(&r)].id).expect("flat");
        let pts: Vec<glam::DVec2> = flat.loops.iter().flatten().map(|p| *p * 1e3).collect();
        for want in [glam::DVec2::new(p_live, r_bend), glam::DVec2::new(r_bend, p_live)] {
            assert!(pts.iter().any(|q| q.distance(want) < 1e-5), "{case}: {want:?} not on the flat's outline");
        }
    }
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
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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

/// A Fillet R 5 of both corners of a sheet metal part's 10 mm wide end (std: corner breaks) meet: the end is one
/// half-round, both corners off (live config-assembly 08590a's Fillet 2 on the arm: 32 faces before and after, 32.19 mm³
/// less at 3 mm).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn corner_breaks_meeting_across_an_end_make_one_half_round() {
    let mut s = Studio::new();
    let region = s.rectangle(60.0, 10.0);
    s.add("sheetMetalStart", |p| {
        p.set("process", en("THICKEN"));
        p.set("regions", ParamValue::Query(vec![region]));
        p.set("thickness", expr("3 mm"));
        p.set("radius", expr("1 mm"));
    });
    let r = s.ok();
    let count = |r: &RegenResult| {
        let s = &r.context.bodies[part(r)].shape;
        (s.faces().len(), s.edges().len(), s.vertices().len())
    };
    let (faces, edges, vertices) = count(&r);
    let corners = vec![edge_at(&r, DVec3::new(60.0, 0.0, 1.5)), edge_at(&r, DVec3::new(60.0, 10.0, 1.5))];
    s.add("fillet", |p| {
        p.set("entities", ParamValue::Query(corners));
        p.set("radius", expr("5 mm"));
    });
    let r = s.ok();
    let expected = (600.0 - 2.0 * (25.0 - std::f64::consts::PI * 25.0 / 4.0)) * 3.0;
    assert!((volume_mm3(&r) - expected).abs() < 1e-6, "{} mm³ vs {expected}", volume_mm3(&r));
    // The end face, its two corner edges and its lines gone for one round face, its tangent lines and two whole arcs.
    assert_eq!(count(&r), (faces, edges, vertices), "faces / edges / vertices");
}

/// A Fillet of the top corners of a U's two walls (std: corner breaks): their thickness edges lie on one line, each
/// corner is rounded (config-assembly 08590a's Fillet 3 on the Latch: both rounded, 343.36 mm³ less, as live).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn corner_breaks_on_collinear_wall_corners_round_both() {
    let mut s = plate("2 mm");
    let r = s.ok();
    let edges = vec![edge_at(&r, DVec3::new(30.0, 0.0, 2.0)), edge_at(&r, DVec3::new(30.0, 40.0, 2.0))];
    s.add("sheetMetalFlange", |p| {
        p.set("edges", ParamValue::Query(edges));
        p.set("distance", expr("20 mm"));
    });
    let r = s.ok();
    let before = volume_mm3(&r);
    // The walls' top corners at x 60: their edges through the thickness, along y, the highest there.
    let shape = &r.context.bodies[part(&r)].shape;
    let top = shape.edges().iter().filter_map(|e| e.edge_info().ok()).filter(|i| (i.start.x - 0.06).abs() < 1e-9 && (i.end.x - 0.06).abs() < 1e-9).map(|i| i.start.z.max(i.end.z)).fold(f64::MIN, f64::max);
    let corners: Vec<Reference> = shape
        .edges()
        .iter()
        .filter_map(|e| e.edge_info().ok())
        .filter(|i| (i.start.x - 0.06).abs() < 1e-9 && (i.end.x - 0.06).abs() < 1e-9 && (i.start.z - top).abs() < 1e-9 && (i.end.z - top).abs() < 1e-9 && (i.length - 0.002).abs() < 1e-9)
        .map(|i| edge_at(&r, (i.start + i.end) * 500.0))
        .collect();
    assert_eq!(corners.len(), 2, "the two walls' top corner edges");
    s.add("fillet", |p| {
        p.set("entities", ParamValue::Query(corners));
        p.set("radius", expr("5 mm"));
    });
    let r = s.ok();
    let cut = 2.0 * (25.0 - std::f64::consts::PI * 25.0 / 4.0) * 2.0;
    assert!((before - volume_mm3(&r) - cut).abs() < 1e-6, "{before} - {} mm³ vs {cut}", volume_mm3(&r));
}

/// Corner break of a slot's corners (live c2-sm-corner-break: a louver panel's slots, a flange on one side of each):
/// each corner rounded in the flat, the slot pulled back from its flange's bend as before.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn corner_break_rounds_a_slots_corners_beside_its_flange() {
    let mut s = plate("2 mm");
    s.ok();
    let mut sk = noshape_sketch::Sketch::new();
    sk.corner_rectangle(glam::DVec2::new(mm(20.0), mm(10.0)), glam::DVec2::new(mm(40.0), mm(30.0)));
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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
    let edge = edge_at(&r, DVec3::new(30.0, 30.0, 2.0));
    s.add("sheetMetalFlange", |p| {
        p.set("edges", ParamValue::Query(vec![edge]));
        p.set("distance", expr("10 mm"));
    });
    let r = s.ok();
    let before = volume_mm3(&r);
    let corners = vec![vertex_at(&r, DVec3::new(20.0, 10.0, 2.0)), vertex_at(&r, DVec3::new(40.0, 10.0, 2.0))];
    s.add("sheetMetalCornerBreak", move |p| {
        p.set("entities", ParamValue::Query(corners.clone()));
        p.set("blendType", en("FILLET"));
        p.set("radius", expr("5 mm"));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).count(), 1, "one part: {:#?}", face_report(&r));
    let rounds = r.context.bodies[part(&r)].shape.faces().iter().filter(|f| matches!(f.face_surface(), Ok(noshape_kernel::Surface::Cylinder { radius, .. }) if (radius - 5e-3).abs() < 1e-9)).count();
    assert_eq!(rounds, 2, "{:#?}", face_report(&r));
    // Each rounded corner fills (r² − πr²/4) × t of the slot.
    let filled = 2.0 * (25.0 - std::f64::consts::PI * 25.0 / 4.0) * 2.0;
    assert!((volume_mm3(&r) - before - filled).abs() < 1e-3, "{} mm³ after, {before} before", volume_mm3(&r));
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
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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
    // Live (qa5 sm-091, psui SM4k): a face per wall and per bend zone.
    assert_eq!(flat.shape.as_ref().expect("flat shape").faces().len(), 3, "plate, bend zone, flange");
    // The flat view names each edge, so a hovered face outlines only its own (psui SM4k).
    let shown = noshape_model::regen::flat_view_display(&flat, [0.5; 4]).expect("flat view");
    let edges = &shown.bodies[0].edge_refs;
    assert!(edges.len() > 4 && edges.iter().collect::<std::collections::HashSet<_>>().len() == edges.len(), "{edges:?}");
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

/// psui SM4i (the sheet metal table's Other joints Type / Style): Modify
/// joint on G34's corner rip restyles it and turns it into a bend, the rip
/// gone from the model data either way it ends up.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn modify_joint_restyles_a_rip_and_turns_it_into_a_bend() {
    use noshape_model::sheetmetal::BendKind;
    let joints = |r: &RegenResult| {
        let data = r.context.sheet_metal.get(&r.context.bodies[part(r)].id).expect("data");
        (data.rips.iter().map(|x| x.style.clone()).collect::<Vec<_>>(), data.bends.iter().filter(|b| matches!(b.kind, BendKind::Bend)).count())
    };
    let mut s = g34();
    let r = s.ok();
    assert_eq!(joints(&r), (vec!["EDGE".to_string()], 2));
    let end = edge_at(&r, DVec3::new(0.02, 0.0, 12.0));
    s.add("sheetMetalJoint", |p| {
        p.set("entity", ParamValue::Query(vec![end.clone()]));
        p.set("jointType", en("RIP"));
        p.set("hasStyle", ParamValue::Bool(true));
        p.set("jointStyle", en("BUTT"));
    });
    let r = s.ok();
    assert_eq!(joints(&r), (vec!["BUTT".to_string()], 2));
    s.ps.features.pop();
    s.add("sheetMetalJoint", |p| {
        p.set("entity", ParamValue::Query(vec![end]));
        p.set("jointType", en("BEND"));
        p.set("useDefaultRadius", ParamValue::Bool(true));
    });
    let r = s.ok();
    let (rips, bends) = joints(&r);
    assert!(rips.is_empty() && bends == 3, "rips {rips:?}, {bends} bends: {} mm³, {:#?}", volume_mm3(&r), face_report(&r));
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
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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
pub(crate) fn qa4_s1_enclosure_as_live() {
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
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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
            pv.set("sketch", ParamValue::Sketch(Default::default()));
        });
        let r = s.ok();
        let frame = r.display.sketches.iter().find(|x| x.feature == sketch).expect("sketch").frame;
        let w = DVec3::new(mm(30.0), mm(20.0), mm(2.0)) - frame.origin;
        let mut sk = noshape_sketch::Sketch::new();
        let point = sk.add_point(glam::DVec2::new(w.dot(frame.x), w.dot(frame.y)));
        let f = s.ps.features.iter_mut().find(|f| f.id == sketch).expect("sketch feature");
        f.params.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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
    // On the top face itself, as live: its frame is x = +X, y = +Y from the
    // world origin's projection (request 1619), so the point is at (40, 30, 2).
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![top.clone()]));
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
    });
    let f = s.ok().context.sketch(sketch).expect("sketch").frame;
    let at = (f.origin + f.x * mm(40.0) + f.y * mm(30.0)) * 1e3;
    assert!(at.distance(DVec3::new(40.0, 30.0, 2.0)) < 1e-6 && f.x.distance(DVec3::X) < 1e-9, "the point at {at:?}, x {:?}", f.x);
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
    let radii: Vec<f64> = fp.forms.iter().flat_map(|f| f.curves.iter().filter(|c| !c.1).map(|(l, _)| l.iter().map(|p| (*p - f.center).length() * 1e3).fold(0.0, f64::max))).collect();
    assert!(fp.forms.iter().all(|f| f.loops.is_empty() && f.curves.iter().all(|c| !c.1)), "the emboss's flat: two outlines, no tear");
    // The form's flat sketch came in with its tools, never shown: only the
    // plate's and the location's are.
    let shown = r.display.sketches.iter().filter(|s| !s.hidden).count();
    assert!(shown == 2 && r.display.sketches.len() == 3, "{shown} shown sketches of {}", r.display.sketches.len());
    // Live's flat (and DXF with the form options): the r 9.1569 footprint
    // and the Form profile sketch's r 2.5 (Diameter 5) circle.
    assert!(radii.len() == 2 && radii.iter().filter(|r| (**r - 9.156854).abs() < 1e-3).count() == 1 && radii.iter().filter(|r| (**r - 2.5).abs() < 1e-5).count() == 1, "form loops {radii:?}");
    assert!((volume_mm3(&r) - 9694.552619769172).abs() < 1e-3 && kinds == [8, 4, 2, 0], "{} mm³, PLANE/TORUS/CONE/other {kinds:?}: {:#?}", volume_mm3(&r), face_report(&r));
}

/// The library's Louver, Bridge lance and Circle knockout at their defaults
/// on a 120 × 80 × 2 plate (R 2) at (60, 40) (live request 1623): volume
/// (live /massproperties), faces, and the flat sketch's tears and outlines
/// (extents in mm about the form's point, from live's DXF).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn library_forms_placed_as_live() {
    let mut bad = Vec::new();
    // (form, volume (live evVolume HIGH, request 1623 (c): the Louver's
    // /massproperties was 0.15 low), its tolerance, faces, tear extents,
    // outline extents).
    for (form, volume, tol, faces, tear, outline) in [
        ("louver", 19611.18705282609, 1e-4, 20, (100.0, 0.0), (100.0, 20.0)),
        ("bridge-lance", 19331.262255870253, 1e-4, 24, (63.313708, 20.0), (63.313708, 20.0)),
        ("circle-knockout", 19205.85213320318, 1e-4, 24, (24.919872, 25.0), (25.0, 2.0)),
    ] {
        let mut s = Studio::new();
        let region = s.rectangle(120.0, 80.0);
        s.add("sheetMetalStart", |p| {
            p.set("process", en("THICKEN"));
            p.set("regions", ParamValue::Query(vec![region]));
            p.set("thickness", expr("2 mm"));
            p.set("radius", expr("2 mm"));
        });
        let r = s.ok();
        let top = face_at(&r, DVec3::new(60.0, 40.0, 2.0));
        let mut sk = noshape_sketch::Sketch::new();
        let point = sk.add_point(glam::DVec2::new(mm(60.0), mm(40.0)));
        let sketch = s.add("sketch", |pv| {
            pv.set("sketchPlane", ParamValue::Query(vec![top.clone()]));
            pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
        });
        s.add("sheetMetalFormed", |p| {
            p.set("formPartStudio", ParamValue::Text(format!("library:{form}")));
            p.set("locations", ParamValue::Query(vec![Reference::SketchEntity { sketch, entity: point.0 }]));
            p.set("targetFaces", ParamValue::Query(vec![top]));
        });
        let r = s.ok();
        let n = r.context.bodies[part(&r)].shape.faces().len();
        let fp = noshape_model::sheetmetal::flat_pattern_for(&r.context, &r.context.bodies[part(&r)].id).expect("flat");
        let extents = |tear: bool| {
            let pts: Vec<glam::DVec2> = fp.forms.iter().flat_map(|f| f.curves.iter().filter(|c| c.1 == tear).flat_map(|c| c.0.iter().map(|p| *p - f.center))).collect();
            let (lo, hi) = pts.iter().fold((glam::DVec2::splat(f64::INFINITY), glam::DVec2::splat(f64::NEG_INFINITY)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
            let (a, b) = ((hi.x - lo.x) * 1e3, (hi.y - lo.y) * 1e3);
            if a >= b { (a, b) } else { (b, a) }
        };
        // The curves are tessellated to 1 µm (an arc's extreme between points).
        let near = |(a, b): (f64, f64), (x, y): (f64, f64)| (a - x.max(y)).abs() < 2e-3 && (b - x.min(y)).abs() < 2e-3;
        let (t, o) = (extents(true), extents(false));
        if !((volume_mm3(&r) - volume).abs() < tol && n == faces && near(t, tear) && near(o, outline)) {
            bad.push(format!("{form}: {} mm³ (live {volume}), {n} faces, tears {t:?}, outlines {o:?}: {:#?}", volume_mm3(&r), face_report(&r)));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
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
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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
            pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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

/// SM-MAKE-JOINT-ONE-EDGE (live G34 c2): two walls thickened from regions
/// on Top and Front meet along x; each wall's part names its faces and edges
/// on its own (they were named alike, so the junction edge carried both
/// walls' association attributes), and Make joint on one junction edge says
/// what live says.
#[test]
fn two_thickened_walls_name_their_parts_apart() {
    let mut s = Studio::new();
    let top = s.rectangle_on(DatumPlane::Top, 60.0, 40.0);
    let front = s.rectangle_on(DatumPlane::Front, 60.0, 30.0);
    s.add("sheetMetalStart", |p| {
        p.set("process", en("THICKEN"));
        p.set("regions", ParamValue::Query(vec![top, front]));
        p.set("thickness", expr("2 mm"));
        p.set("radius", expr("2 mm"));
    });
    let r = s.ok();
    let solids: Vec<_> = r.context.bodies.iter().filter(|b| b.kind == BodyKind::Solid).collect();
    assert_eq!(solids.len(), 2);
    let names: std::collections::HashSet<_> = solids.iter().flat_map(|b| b.edges.iter()).collect();
    assert_eq!(names.len(), solids.iter().map(|b| b.edges.len()).sum::<usize>(), "edge names shared between the two parts");
    let b = solids.iter().find(|b| b.shape.edges().iter().any(|e| noshape_kernel::distance(&noshape_kernel::make_vertex(DVec3::new(30.0, -2.0, 0.0) * 1e-3).unwrap(), e).is_ok_and(|d| d.distance < 1e-9))).expect("the front wall's part");
    let i = b.shape.edges().iter().position(|e| noshape_kernel::distance(&noshape_kernel::make_vertex(DVec3::new(30.0, -2.0, 0.0) * 1e-3).unwrap(), e).is_ok_and(|d| d.distance < 1e-9)).unwrap();
    let edge = Reference::Edge((*b.edges[i]).clone());
    s.add("sheetMetalMakeJoint", |p| p.set("entities", ParamValue::Query(vec![edge])));
    let r = Regenerator::new().regenerate(&s.ps, &ConfigValues::default(), &s.units, &RegenOptions::default());
    let st = r.display.features.last().unwrap();
    assert_eq!(st.message.as_deref(), Some("Cannot add joint joining selected entities."), "{:?}", st.status);
}

/// std's `addManipulators` reaches the feature state (audit-part-studio M15): Sheet metal model by Thicken adds its
/// "flip direction up" arrow (std addFlipDirectionUpManipulator: on the sheet's first face, unflipped), and a
/// click on it runs std's sheetMetalStartManipulatorChange, which sets Flip direction up.
#[test]
fn sheet_metal_model_adds_std_s_flip_manipulator() {
    let mut s = Studio::new();
    let region = s.rectangle(60.0, 40.0);
    let start = s.add("sheetMetalStart", |p| {
        p.set("process", en("THICKEN"));
        p.set("regions", ParamValue::Query(vec![region]));
        p.set("thickness", expr("2 mm"));
        p.set("radius", expr("2 mm"));
    });
    let r = s.ok();
    let state = r.display.feature_state(start).expect("state");
    let [(key, m)] = state.manipulators.as_slice() else { panic!("one manipulator: {:?}", state.manipulators) };
    // (On the sheet's first face: ours is a side face, live's the sheet's own face; skasm.md "From psui" 11-01.)
    let noshape_model::display::FeatureManipulator::Flip { flipped, .. } = m else { panic!("a flip arrow: {m:?}") };
    assert!(!flipped, "{m:?}");
    let mut moved = m.clone();
    if let noshape_model::display::FeatureManipulator::Flip { flipped, .. } = &mut moved {
        *flipped = true;
    }
    let f = s.ps.features.iter().find(|f| f.id == start).unwrap();
    let before = Regenerator::new().regenerate(&s.ps, &ConfigValues::default(), &s.units, &RegenOptions { upto: Some(s.ps.features.len() - 1), ..Default::default() }).context;
    let changed = noshape_model::custom::std_feature::manipulator_change_in((*before).clone(), &f.type_id, f.id, &f.name, &s.units, &f.params, key, &moved).expect("std's change function ran");
    assert_eq!(changed.get("flipDirectionUp"), Some(&ParamValue::Bool(true)), "{changed:?}");
}

/// T2 tool: qa4's enclosure (std sheet metal through FeatureScript) once to warm up, then sampled into
/// NOSHAPE_SAMPLE_OUT, with its time.
#[test]
#[ignore = "T2 tool: NOSHAPE_SAMPLE_OUT"]
fn fs_sheet_metal_profile() {
    let out = std::env::var("NOSHAPE_SAMPLE_OUT").unwrap_or_else(|_| "/tmp/fs-sm.txt".into());
    qa4_s1_enclosure_as_live();
    let t = std::time::Instant::now();
    crate::sampler::sample(std::path::Path::new(&out), qa4_s1_enclosure_as_live);
    eprintln!("fs sheet metal warm: {:.2?}", t.elapsed());
}

/// SM-LOUVER-KERNEL-ABORT: the louver's tools build (an OCCT exception in a section noshape's Boolean asked for
/// terminated the process: a decline now, OCCT's Boolean builds them), live's volumes (dom/sm-forms-v4).
#[test]
fn the_louvers_tools_build() {
    let studio = noshape_model::sheetmetal::forms::studio("louver").expect("studio");
    let mut config = studio.configuration.defaults();
    config.0.insert("height".to_string(), "5 mm".to_string());
    let r = Regenerator::new().regenerate(&studio, &config, &UnitPrefs::default(), &RegenOptions::default());
    let st: Vec<_> = r.display.features.iter().map(|f| (f.status, f.message.clone())).collect();
    let v: Vec<f64> = r.context.bodies.iter().map(|b| b.shape.volume().unwrap_or(0.0) * 1e9).collect();
    assert!(v.len() == 2 && (v[0] - 1975.3626).abs() < 1e-3 && (v[1] - 1889.368549).abs() < 1e-3, "{v:?}; {st:?}");
}

/// Live 2005 B2 (behaviours.md "P1 FS-NATIVE-OPDERIP"): on the same two-flange plate, a Bend through the ripped
/// corner's region, line (80, 60) → (100, 40) on Top, the plate's bottom as its face, is refused: "Metal is
/// connected on both sides of the bend." (no derip; std's own check). T1.
#[test]
fn a_bend_through_a_ripped_corner_is_refused() {
    let mut s = Studio::new();
    let region = s.rectangle(100.0, 60.0);
    s.add("sheetMetalStart", |p| {
        p.set("process", en("THICKEN"));
        p.set("regions", ParamValue::Query(vec![region]));
        p.set("thickness", expr("2 mm"));
        p.set("radius", expr("2 mm"));
    });
    let r = s.ok();
    let edges = vec![edge_at(&r, DVec3::new(100.0, 30.0, 2.0)), edge_at(&r, DVec3::new(50.0, 60.0, 2.0))];
    s.add("sheetMetalFlange", |p| {
        p.set("edges", ParamValue::Query(edges));
        p.set("distance", expr("30 mm"));
    });
    let mut sk = noshape_sketch::Sketch::new();
    let (a, b) = (sk.add_point(glam::DVec2::new(mm(80.0), mm(60.0))), sk.add_point(glam::DVec2::new(mm(100.0), mm(40.0))));
    let line = sk.add_line_between(a, b).unwrap().0;
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
    });
    let r = s.ok();
    let bottom = face_at(&r, DVec3::new(50.0, 30.0, 0.0));
    s.add("sheetMetalBend", move |p| {
        p.set("bendReference", ParamValue::Query(vec![Reference::SketchEntity { sketch, entity: line }]));
        p.set("face", ParamValue::Query(vec![bottom]));
    });
    let r = Regenerator::new().regenerate(&s.ps, &ConfigValues::default(), &s.units, &RegenOptions::default());
    let st = r.display.features.last().expect("the bend");
    assert!(matches!(st.status, FeatureStatus::Error) && st.message.as_deref().is_some_and(|m| m.contains("Metal is connected on both sides of the bend.")), "{:?} {:?}", st.status, st.message);
}

/// Live 2005 T2 (behaviours.md "P1 FS-NATIVE-OPDERIP"): a 100 × 60 plate with 30 mm flanges on its x = 100 and
/// y = 60 edges (a corner rip between them); a Tab sketched on the x-flange's outer face, y 50 → 62, z 10 → 20,
/// merged into that flange, fills the corner from that flange only: std derips the corner (@opDerip: each wall's
/// side of the rip edge moves onto its part face, so they come apart), then the Tab extends the x-wall; the other
/// keeps its edge and the clearance stays. 21196.5854 mm³, 28 faces. T1.
#[test]
fn a_tab_into_a_ripped_corner_fills_it_from_its_flange() {
    let mut s = Studio::new();
    let region = s.rectangle(100.0, 60.0);
    s.add("sheetMetalStart", |p| {
        p.set("process", en("THICKEN"));
        p.set("regions", ParamValue::Query(vec![region]));
        p.set("thickness", expr("2 mm"));
        p.set("radius", expr("2 mm"));
    });
    let r = s.ok();
    let edges = vec![edge_at(&r, DVec3::new(100.0, 30.0, 2.0)), edge_at(&r, DVec3::new(50.0, 60.0, 2.0))];
    s.add("sheetMetalFlange", |p| {
        p.set("edges", ParamValue::Query(edges));
        p.set("distance", expr("30 mm"));
    });
    let r = s.ok();
    let base = volume_mm3(&r);
    let wall = face_at(&r, DVec3::new(102.0, 30.0, 15.0));
    // The sketch on the flange's outer face; its rectangle placed through the face's frame.
    let sketch_on = |s: &mut Studio, sk: noshape_sketch::Sketch| {
        s.add("sketch", |pv| {
            pv.set("sketchPlane", ParamValue::Query(vec![wall.clone()]));
            pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
        })
    };
    let probe = sketch_on(&mut s, noshape_sketch::Sketch::new());
    let r = s.ok();
    let frame = r.display.sketches.iter().find(|x| x.feature == probe).expect("sketch").frame;
    s.ps.features.retain(|f| f.id != probe);
    let to2 = |p: DVec3| glam::DVec2::new((p - frame.origin).dot(frame.x), (p - frame.origin).dot(frame.y));
    let mut sk = noshape_sketch::Sketch::new();
    let pts = [(50.0, 10.0), (62.0, 10.0), (62.0, 20.0), (50.0, 20.0)].map(|(y, z)| sk.add_point(to2(DVec3::new(mm(102.0), mm(y), mm(z)))));
    for i in 0..4 {
        sk.add_line_between(pts[i], pts[(i + 1) % 4]).unwrap();
    }
    let sketch = sketch_on(&mut s, sk);
    let r = s.ok();
    let sk = r.display.sketches.iter().find(|x| x.feature == sketch).expect("sketch");
    // The rectangle's regions: not the face's outline, which the sketch imprints (and which is the biggest).
    let tab: Vec<Reference> = sk.regions.iter().filter(|g| !g.imprinted).map(|g| Reference::SketchRegion { sketch, key: g.key.clone() }).collect();
    assert!(!tab.is_empty(), "the rectangle's regions");
    s.add("sheetMetalTab", |p| {
        p.set("tabFaces", ParamValue::Query(tab));
        p.set("booleanUnionScope", ParamValue::Query(vec![wall.clone()]));
    });
    let r = s.ok();
    let faces = r.context.bodies[part(&r)].shape.faces().len();
    assert!((base - 21156.1854).abs() < 1e-3, "base {base} mm³");
    assert!((volume_mm3(&r) - 21196.5854).abs() < 1e-3 && faces == 28, "{} mm³, {faces} faces: {:#?}", volume_mm3(&r), face_report(&r));
}

/// A wall, a bend and a rolled wall (cf-sheet-metal-model-plus's Up to Part, live 2031): Sheet metal model › Extrude
/// of a line (0,0)–(20,0) and an arc about (30,0) from (20,0) to (30,10), 15 mm, thickness 2, R1: one part, its bend
/// beside the rolled wall the definition edge's fillet (a cylinder meeting both). The material lies outside the
/// corner, so the definition curve has the R1 fillet and turns 0 in all: the section is 2 × its length,
/// 19.045549 + 11 (π/2 − atan(1/√120)), and the part 1059.6878 mm³ in 14 faces.
#[test]
fn a_bend_beside_a_rolled_wall() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let p = [(0.0, 0.0), (20.0, 0.0), (30.0, 10.0), (30.0, 0.0)].map(|(x, y)| sk.add_point(glam::DVec2::new(mm(x), mm(y))));
    let line = sk.add_line_between(p[0], p[1]).unwrap();
    let arc = sk.add_arc_between(p[3], p[2], p[1]).unwrap();
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
    });
    s.ok();
    s.add("sheetMetalStart", |pv| {
        pv.set("process", en("EXTRUDE"));
        pv.set("sketchCurves", ParamValue::Query([line, arc].iter().map(|e| Reference::SketchEntity { sketch, entity: e.0 }).collect()));
        pv.set("depth", expr("15 mm"));
        pv.set("thickness", expr("2 mm"));
        pv.set("radius", expr("1 mm"));
    });
    let r = s.ok();
    let parts = r.context.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).count();
    let faces = r.context.bodies[part(&r)].shape.faces().len();
    let phi = (1.0f64 / 120f64.sqrt()).atan();
    let want = 2.0 * (30.0 - 120f64.sqrt() + 11.0 * (std::f64::consts::FRAC_PI_2 - phi)) * 15.0;
    assert!(parts == 1 && faces == 14 && (volume_mm3(&r) - want).abs() < 1e-3, "{parts} parts, {faces} faces, {} mm³ (want {want}): {:#?}", volume_mm3(&r), face_report(&r));
}

/// The same with a second bend of the other hand after the rolled wall: on to (40,0), a 45° turn the material lies
/// inside of (its definition radius 3). The definition curve: 19.045549 + (π/2 − φ) + 10 (π/2 − φ − θ) +
/// 3 (θ + π/4) + (10√2 − 1.3766489), φ = atan(1/√120), θ = 0.1647326 (the R3 fillet's tangent on the arc), turning
/// −π/4 in all: the section 2 L − 2 (π/4), 1455.1802 mm³.
#[test]
fn bends_of_both_hands_beside_a_rolled_wall() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let p = [(0.0, 0.0), (20.0, 0.0), (30.0, 10.0), (30.0, 0.0), (40.0, 0.0)].map(|(x, y)| sk.add_point(glam::DVec2::new(mm(x), mm(y))));
    let line = sk.add_line_between(p[0], p[1]).unwrap();
    let arc = sk.add_arc_between(p[3], p[2], p[1]).unwrap();
    let last = sk.add_line_between(p[2], p[4]).unwrap();
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
    });
    s.ok();
    s.add("sheetMetalStart", |pv| {
        pv.set("process", en("EXTRUDE"));
        pv.set("sketchCurves", ParamValue::Query([line, arc, last].iter().map(|e| Reference::SketchEntity { sketch, entity: e.0 }).collect()));
        pv.set("depth", expr("15 mm"));
        pv.set("thickness", expr("2 mm"));
        pv.set("radius", expr("1 mm"));
    });
    let r = s.ok();
    let parts = r.context.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).count();
    let faces = r.context.bodies[part(&r)].shape.faces().len();
    let (phi, theta) = ((1.0f64 / 120f64.sqrt()).atan(), 0.1647326f64);
    let q = std::f64::consts::FRAC_PI_4;
    let length = 30.0 - 120f64.sqrt() + (std::f64::consts::FRAC_PI_2 - phi) + 10.0 * (std::f64::consts::FRAC_PI_2 - phi - theta) + 3.0 * (theta + q) + (200f64.sqrt() - 1.3766489);
    let want = (2.0 * length - 2.0 * q) * 15.0;
    assert!(parts == 1 && (volume_mm3(&r) - want).abs() < 1e-2, "{parts} parts, {faces} faces, {} mm³ (want {want}): {:#?}", volume_mm3(&r), face_report(&r));
}

/// A bend beside a spline rolled wall (cf-sheet-metal-model-plus's Up to Part, live 2031: its bends there are exact
/// cylinders): a line (0,0)–(20,0) and a fit spline through (20,0), (23,6), (30,10), 15 mm, thickness 2, R1: one part,
/// 14 faces, the bend's two a cylinder each (the kernel's own fillet against the spline is a B-spline), no slivers.
#[test]
fn a_bend_beside_a_spline_rolled_wall_is_a_cylinder() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let p = [(0.0, 0.0), (20.0, 0.0)].map(|(x, y)| sk.add_point(glam::DVec2::new(mm(x), mm(y))));
    let line = sk.add_line_between(p[0], p[1]).unwrap();
    let spline = sk.add_fit_spline(&[(20.0, 0.0), (23.0, 6.0), (30.0, 10.0)].map(|(x, y)| glam::DVec2::new(mm(x), mm(y)))).unwrap();
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
    });
    s.ok();
    s.add("sheetMetalStart", |pv| {
        pv.set("process", en("EXTRUDE"));
        pv.set("sketchCurves", ParamValue::Query([line, spline].iter().map(|e| Reference::SketchEntity { sketch, entity: e.0 }).collect()));
        pv.set("depth", expr("15 mm"));
        pv.set("thickness", expr("2 mm"));
        pv.set("radius", expr("1 mm"));
    });
    let r = s.ok();
    let parts = r.context.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).count();
    let shape = &r.context.bodies[part(&r)].shape;
    let faces = shape.faces();
    let cylinders = faces.iter().filter(|f| matches!(f.face_surface(), Ok(noshape_kernel::Surface::Cylinder { .. }))).count();
    let slivers = faces.iter().filter(|f| f.area().is_ok_and(|a| a < 1e-6)).count();
    assert!(parts == 1 && faces.len() == 14 && cylinders == 2 && slivers == 0, "{parts} parts, {} faces, {cylinders} cylinders, {slivers} slivers, {} mm³: {:#?}", faces.len(), volume_mm3(&r), face_report(&r));
}

/// A part pattern of an active sheet metal part is std's sheet metal pattern: an Add of the 60 × 40 plate's copy
/// 60 mm along x joins it into the one 120 × 40 sheet metal part (c2-sm-louver's Linear pattern 1, 278 faces as
/// live's); a plain part's Add into active sheet metal is std's error, as Extrude's.
#[test]
fn a_pattern_of_active_sheet_metal_is_stds() {
    let mut s = plate("2 mm");
    let r = s.ok();
    let body = r.context.bodies[part(&r)].id.clone();
    let along_x = |p: &mut ParamValues| {
        p.set("patternType", en("PART"));
        p.set("directionOne", ParamValue::Query(vec![Reference::Plane(DatumPlane::Right)]));
        p.set("distance", expr("60 mm"));
        p.set("instanceCount", expr("2"));
        p.set("operationType", en("ADD"));
        p.set("defaultScope", ParamValue::Bool(true));
    };
    s.add("linearPattern", |p| {
        along_x(p);
        p.set("entities", ParamValue::Query(vec![Reference::Body(body.clone())]));
    });
    let r = s.ok();
    let solids: Vec<_> = r.context.bodies.iter().filter(|b| b.kind == BodyKind::Solid).collect();
    assert_eq!(solids.len(), 1, "one part");
    assert!((volume_mm3(&r) - 9600.0).abs() < 1e-3, "{} mm³", volume_mm3(&r));
    assert!(r.context.sheet_metal.get(&solids[0].id).is_some_and(|d| d.active), "still active sheet metal");

    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(glam::DVec2::new(mm(100.0), mm(20.0)), mm(5.0));
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
    });
    let r = s.ok();
    let sk = r.display.sketches.iter().find(|x| x.feature == sketch).expect("sketch");
    let key = sk.regions[0].key.clone();
    s.add("extrude", |p| {
        p.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch, key }]));
        p.set("operationType", en("NEW"));
        p.set("depth", expr("10 mm"));
    });
    let r = s.ok();
    let pin = r.context.bodies.iter().find(|b| b.kind == BodyKind::Solid && !r.context.sheet_metal.contains_key(&b.id)).expect("the pin").id.clone();
    s.add("linearPattern", |p| {
        along_x(p);
        p.set("entities", ParamValue::Query(vec![Reference::Body(pin)]));
    });
    let r = Regenerator::new().regenerate(&s.ps, &ConfigValues::default(), &s.units, &RegenOptions::default());
    let last = r.display.features.last().unwrap();
    assert_eq!((last.status, last.message.as_deref()), (FeatureStatus::Error, Some("Only Remove can be used with sheet metal. Add Finish sheet metal model to allow other operations.")), "{:?}", last.message);
}

/// SM-FACES with the L's first leg drawn as two collinear lines (c2-sm-louver's Louver, live 22 faces): the two are
/// one wall, unified apart from the rest, so its ends stay in wall / bend / wall pieces: the L's 3211.3274 mm³ and
/// 14 faces (a unify of the whole part made each end one face: 10).
#[test]
fn collinear_walls_keep_the_ends_in_pieces() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let p = [(0.0, 0.0), (25.0, 0.0), (50.0, 0.0), (50.0, 30.0)].map(|(x, y)| sk.add_point(glam::DVec2::new(mm(x), mm(y))));
    let lines: Vec<_> = p.windows(2).map(|w| sk.add_line_between(w[0], w[1]).unwrap()).collect();
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
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
    assert!((volume_mm3(&r) - 3211.3274).abs() < 1e-3 && faces == 14, "{} mm³, {faces} faces: {:#?}", volume_mm3(&r), face_report(&r));
}

/// An Outer flange on a plate's edge that meets a tangent arc at one end (asm-config-assembly's Flange 1: a bracket
/// whose bottom edge runs into an R20 arc): std moves that edge in by the thickness and bend radius for its alignment,
/// the arc kept and a straight step joining it to the moved edge, so the flange is as wide as the edge, as live's (its
/// two 26 x 21 mm flange faces; ours refused "Only straight edges next to a changed edge can be changed yet", so the
/// flange failed with "Cannot create flange with selected alignment").
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn an_outer_flange_beside_a_tangent_arc_moves_its_edge() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let p: Vec<_> = [(0.0, 0.0), (30.0, 0.0), (30.0, 20.0), (-20.0, 20.0)].iter().map(|&(x, y)| sk.add_point(glam::DVec2::new(mm(x), mm(y)))).collect();
    for i in 0..3 {
        sk.add_line_between(p[i], p[i + 1]).unwrap();
    }
    let c = sk.add_point(glam::DVec2::new(0.0, mm(20.0)));
    sk.add_arc_between(c, p[3], p[0]).unwrap();
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
    });
    let r = s.ok();
    let key = r.display.sketches.iter().find(|g| g.feature == sketch).unwrap().regions[0].key.clone();
    s.add("sheetMetalStart", |pv| {
        pv.set("process", en("THICKEN"));
        pv.set("regions", ParamValue::Query(vec![Reference::SketchRegion { sketch, key }]));
        pv.set("thickness", expr("3 mm"));
        pv.set("radius", expr("1 mm"));
    });
    let r = s.ok();
    let before = volume_mm3(&r);
    let edge = edge_at(&r, DVec3::new(15.0, 0.0, 3.0));
    s.add("sheetMetalFlange", |pv| {
        pv.set("edges", ParamValue::Query(vec![edge]));
        pv.set("distance", expr("25 mm"));
        pv.set("flangeAlignment", en("OUTER"));
    });
    let r = s.ok();
    assert!(volume_mm3(&r) > before + 100.0, "the flange: {} mm³ for the plate's {before}: {:#?}", volume_mm3(&r), face_report(&r));
    // The flange's two faces as wide as the edge (30 mm), 21 mm long (25 less the thickness and bend radius).
    let walls = r.context.bodies[part(&r)].shape.faces().iter().filter(|f| f.face_plane().ok().flatten().is_some() && f.area().is_ok_and(|a| (a * 1e6 - 30.0 * 21.0).abs() < 1e-3)).count();
    assert_eq!(walls, 2, "{:#?}", face_report(&r));
}

/// An Inner flange on a plate's top edge that steps 0.275 mm to a tab at its end (c2-sm-louver's Flange 3: the tab's
/// edge just off the flange's): the bend ends with the flange at the step, relieved there, not run on past the tab
/// (ours ran it 32 mm out past the plate's end, a second flange); a flange on the opposite edge then regenerates, the
/// tab's side 2.286 mm off it unrelieved.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn an_inner_flange_beside_a_stepped_tab_ends_at_the_step() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let p: Vec<_> = [(0.0, 2.286), (12.7, 2.286), (12.7, 0.0), (60.0, 0.0), (60.0, 40.0), (12.7, 40.0), (12.7, 39.725), (0.0, 39.725)].iter().map(|&(x, y)| sk.add_point(glam::DVec2::new(mm(x), mm(y)))).collect();
    for i in 0..p.len() {
        sk.add_line_between(p[i], p[(i + 1) % p.len()]).unwrap();
    }
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
    });
    let r = s.ok();
    let key = r.display.sketches.iter().find(|g| g.feature == sketch).unwrap().regions[0].key.clone();
    s.add("sheetMetalStart", |pv| {
        pv.set("process", en("THICKEN"));
        pv.set("regions", ParamValue::Query(vec![Reference::SketchRegion { sketch, key }]));
        pv.set("thickness", expr("1.628 mm"));
        pv.set("radius", expr("1.628 mm"));
    });
    let r = s.ok();
    let edge = edge_at(&r, DVec3::new(36.0, 40.0, 1.628));
    s.add("sheetMetalFlange", |pv| {
        pv.set("edges", ParamValue::Query(vec![edge]));
        pv.set("distance", expr("20 mm"));
        pv.set("flangeAlignment", en("INNER"));
    });
    let r = s.ok();
    let bb = r.context.bodies[part(&r)].shape.bounding_box(true).unwrap();
    assert!(bb.min.x > -1e-9, "the part reaches x {} mm: {:#?}", bb.min.x * 1e3, face_report(&r));
    // A second Inner flange on the plate's opposite edge (the louver's Flange 4): the part's end faces at the step stay
    // that edge's, not the step's, so std moves that edge alone ("Cannot create flange with selected alignment").
    let edge = edge_at(&r, DVec3::new(36.0, 0.0, 1.628));
    s.add("sheetMetalFlange", |pv| {
        pv.set("edges", ParamValue::Query(vec![edge]));
        pv.set("distance", expr("20 mm"));
        pv.set("flangeAlignment", en("INNER"));
    });
    let r = s.ok();
    // The tab's bottom 2.286 mm off that edge is a side of its own, not a moved-in edge: no relief trims it (live's
    // Flange 4 keeps the tab's 20.676 mm² strip).
    let b = &r.context.bodies[part(&r)].shape;
    let strip = b.faces().into_iter().find(|f| f.face_plane().ok().flatten().is_some_and(|pl| pl.unit_normal().y < -0.999 && (pl.origin.y * 1e3 - 2.286).abs() < 1e-6));
    let area = strip.and_then(|f| f.area().ok()).unwrap_or(0.0) * 1e6;
    assert!((area - 12.7 * 1.628).abs() < 1e-3, "the tab's bottom strip: {area} mm²: {:#?}", face_report(&r));
}

/// A flange aligned at its bend on a plate with a hole (c2-sm-louver's Flange 9: its wall has holes): std moves the
/// plate's edge back to align the flange, the hole staying as it is ("Cannot create flange with selected alignment"
/// while ours couldn't change an edge of a face with holes).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn a_bend_aligned_flange_on_a_plate_with_a_hole() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let p: Vec<_> = [(0.0, 0.0), (60.0, 0.0), (60.0, 40.0), (0.0, 40.0)].iter().map(|&(x, y)| sk.add_point(glam::DVec2::new(mm(x), mm(y)))).collect();
    for i in 0..4 {
        sk.add_line_between(p[i], p[(i + 1) % 4]).unwrap();
    }
    sk.add_circle(glam::DVec2::new(mm(30.0), mm(20.0)), mm(5.0));
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
    });
    let r = s.ok();
    let key = r.display.sketches.iter().find(|g| g.feature == sketch).unwrap().regions.iter().max_by(|a, b| a.area.total_cmp(&b.area)).unwrap().key.clone();
    s.add("sheetMetalStart", |pv| {
        pv.set("process", en("THICKEN"));
        pv.set("regions", ParamValue::Query(vec![Reference::SketchRegion { sketch, key }]));
        pv.set("thickness", expr("2 mm"));
        pv.set("radius", expr("2 mm"));
    });
    let r = s.ok();
    let edge = edge_at(&r, DVec3::new(30.0, 40.0, 2.0));
    s.add("sheetMetalFlange", |pv| {
        pv.set("edges", ParamValue::Query(vec![edge]));
        pv.set("distance", expr("20 mm"));
        pv.set("flangeAlignment", en("BEND"));
    });
    let r = s.ok();
    assert!(r.context.bodies[part(&r)].shape.faces().iter().any(|f| matches!(f.face_surface(), Ok(noshape_kernel::Surface::Cylinder { .. })) && f.area().is_ok_and(|a| (a * 1e6 - std::f64::consts::PI * 10.0 * 2.0).abs() < 1e-3)), "the hole's wall");
}

/// A bend relief's round is centred (depth scale - 1) × R past the bend's tangent line, also on a bend that isn't 90°
/// (live 2254: c2-sm-louver's Flange 3, depth scale 1, the round centred on the tangent line 0.51 mm past the tab).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn a_bend_reliefs_round_is_centred_on_the_tangent_line() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let p: Vec<_> = [(0.0, 0.0), (60.0, 0.0), (60.0, 40.0), (12.7, 40.0), (12.7, 39.725), (0.0, 39.725)].iter().map(|&(x, y)| sk.add_point(glam::DVec2::new(mm(x), mm(y)))).collect();
    for i in 0..p.len() {
        sk.add_line_between(p[i], p[(i + 1) % p.len()]).unwrap();
    }
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
    });
    let r = s.ok();
    let key = r.display.sketches.iter().find(|g| g.feature == sketch).unwrap().regions[0].key.clone();
    s.add("sheetMetalStart", |pv| {
        pv.set("process", en("THICKEN"));
        pv.set("regions", ParamValue::Query(vec![Reference::SketchRegion { sketch, key }]));
        pv.set("thickness", expr("1.628 mm"));
        pv.set("radius", expr("1.628 mm"));
        pv.set("defaultBendReliefDepthScale", expr("1"));
    });
    let r = s.ok();
    let edge = edge_at(&r, DVec3::new(36.0, 40.0, 1.628));
    s.add("sheetMetalFlange", |pv| {
        pv.set("edges", ParamValue::Query(vec![edge]));
        pv.set("distance", expr("20 mm"));
        pv.set("flangeAlignment", en("INNER"));
        pv.set("bendAngle", expr("50 deg"));
    });
    let r = s.ok();
    let cylinders: Vec<(DVec3, DVec3, f64)> = r.context.bodies[part(&r)].shape.faces().iter().filter_map(|f| match f.face_surface() {
        Ok(noshape_kernel::Surface::Cylinder { origin, axis, radius, .. }) => Some((origin, axis, radius)),
        _ => None,
    }).collect();
    let bend = cylinders.iter().find(|c| c.1.x.abs() > 0.999 && (c.2 - mm(1.628)).abs() < 1e-9).expect("the bend's inner face");
    let round = cylinders.iter().find(|c| c.1.z.abs() > 0.999 && (c.2 - mm(1.628 * 1.0625 / 2.0)).abs() < 1e-9).unwrap_or_else(|| panic!("the relief's round: {:#?}", face_report(&r)));
    assert!((round.0.y - bend.0.y).abs() < 1e-7, "the round at y {} mm, the tangent line at {} mm", round.0.y * 1e3, bend.0.y * 1e3);
}

/// A part pattern (New) of a sheet metal part with a flange copies it whole (c2-sm-louver's Linear pattern 2: live's
/// ten copies 56 faces each as the seed; ours came out as a plate and loose flange walls).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn a_pattern_of_a_flanged_part_copies_it_whole() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let p: Vec<_> = [(0.0, 2.286), (12.7, 2.286), (12.7, 0.0), (60.0, 0.0), (60.0, 40.0), (12.7, 40.0), (12.7, 39.725), (0.0, 39.725)].iter().map(|&(x, y)| sk.add_point(glam::DVec2::new(mm(x), mm(y)))).collect();
    for i in 0..p.len() {
        sk.add_line_between(p[i], p[(i + 1) % p.len()]).unwrap();
    }
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
    });
    let r = s.ok();
    let key = r.display.sketches.iter().find(|g| g.feature == sketch).unwrap().regions[0].key.clone();
    s.add("sheetMetalStart", |pv| {
        pv.set("process", en("THICKEN"));
        pv.set("regions", ParamValue::Query(vec![Reference::SketchRegion { sketch, key }]));
        pv.set("thickness", expr("1.628 mm"));
        pv.set("radius", expr("1.628 mm"));
    });
    let r = s.ok();
    for at in [DVec3::new(36.0, 40.0, 1.628), DVec3::new(36.0, 0.0, 1.628)] {
        let edge = edge_at(&r, at);
        s.add("sheetMetalFlange", |pv| {
            pv.set("edges", ParamValue::Query(vec![edge]));
            pv.set("distance", expr("20 mm"));
            pv.set("flangeAlignment", en("INNER"));
        });
    }
    let r = s.ok();
    let seed = &r.context.bodies[part(&r)];
    let (faces, volume) = (seed.shape.faces().len(), volume_mm3(&r));
    let body = seed.id.clone();
    s.add("linearPattern", |p| {
        p.set("patternType", en("PART"));
        p.set("entities", ParamValue::Query(vec![Reference::Body(body.clone())]));
        p.set("directionOne", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        p.set("distance", expr("100 mm"));
        p.set("instanceCount", expr("2"));
        p.set("operationType", en("NEW"));
    });
    let r = s.ok();
    let solids: Vec<_> = r.context.bodies.iter().filter(|b| b.kind == BodyKind::Solid).map(|b| (b.shape.faces().len(), b.shape.volume().unwrap_or(0.0) * 1e9)).collect();
    assert!(solids.len() == 2 && solids.iter().all(|&(f, v)| f == faces && (v - volume).abs() < 1e-3), "the seed {faces} faces, {volume} mm³; the parts {solids:?}");
}

/// An L of two sketch lines on `plane`, Extrude-type Sheet metal model as live 2468 made its probes (c2-sm-louver's
/// "Side": opposite direction, flip direction up; depth 50, 2 mm thick, R 2, K 0.45): its flat body's box in mm.
fn probe_flat_box(plane: DatumPlane, pts: [(f64, f64); 3]) -> ([f64; 3], [f64; 3]) {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let p = pts.map(|(x, y)| sk.add_point(glam::DVec2::new(mm(x), mm(y))));
    let lines = [sk.add_line_between(p[0], p[1]).unwrap(), sk.add_line_between(p[1], p[2]).unwrap()];
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(plane)]));
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
    });
    s.ok();
    s.add("sheetMetalStart", |pv| {
        pv.set("process", en("EXTRUDE"));
        pv.set("sketchCurves", ParamValue::Query(lines.iter().map(|e| Reference::SketchEntity { sketch, entity: e.0 }).collect()));
        pv.set("depth", expr("50 mm"));
        pv.set("thickness", expr("2 mm"));
        pv.set("radius", expr("2 mm"));
        pv.set("kFactor", expr("0.45"));
        pv.set("oppositeDirection", ParamValue::Bool(true));
        pv.set("flipDirectionUp", ParamValue::Bool(true));
    });
    let r = s.ok();
    let part = &r.context.bodies[part(&r)];
    let flat = noshape_model::sheetmetal::flat_body(&r.context, &noshape_model::sheetmetal::flat_body_id(&part.id), &part.name).expect("the flat body");
    let b = flat.shape.bounding_box(false).expect("its box");
    let r3 = |v: DVec3| [v.x, v.y, v.z].map(|c| (c * 1e6).round() / 1e3);
    (r3(b.min), r3(b.max))
}

/// Live 2468: the flat lies where its fixed face (the first sketch segment's) puts it: (p·x, p·y, (p − o)·n) with
/// that face's plane frame, so under z = 0 with in-plane coordinates kept (SM Front: (−y, z, −x); SM Top: (x, z, −y)).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn the_flat_lies_where_its_first_wall_puts_it() {
    let front = probe_flat_box(DatumPlane::Front, [(0.0, 30.0), (0.0, 0.0), (40.0, 0.0)]);
    assert_eq!(front, ([0.0, -36.555, -2.0], [50.0, 30.0, 0.0]), "SM Front");
    let top = probe_flat_box(DatumPlane::Top, [(100.0, 0.0), (140.0, 0.0), (140.0, 30.0)]);
    assert_eq!(top, ([100.0, 0.0, -2.0], [166.555, 50.0, 0.0]), "SM Top");
    // Its sketch line off the origin: still under z = 0, y 0…50 (z = 80 − y).
    let (lo, hi) = probe_flat_box(DatumPlane::Top, [(200.0, 80.0), (240.0, 80.0), (240.0, 110.0)]);
    assert_eq!((lo[1], lo[2], hi[1], hi[2]), (0.0, -2.0, 50.0, 0.0), "SM Top offset: {lo:?} {hi:?}");
    // The wall drawn first is the fixed one: x = −y, x −30…40.555.
    let (lo, hi) = probe_flat_box(DatumPlane::Top, [(340.0, 30.0), (340.0, 0.0), (300.0, 0.0)]);
    assert_eq!((lo[0], lo[2], hi[0], hi[2]), (-30.0, -2.0, 40.555, 0.0), "SM Top wall-first: {lo:?} {hi:?}");
}

/// The lowest vertex of the part's bend faces at x (mm): a bend's end.
fn bend_end_at(r: &RegenResult, x: f64) -> Reference {
    let b = &r.context.bodies[part(r)];
    let mut ends: Vec<DVec3> = b.shape.faces().iter().filter(|f| matches!(f.face_surface(), Ok(noshape_kernel::Surface::Cylinder { .. }))).flat_map(|f| f.vertices()).filter_map(|v| v.vertex_point().ok()).map(|p| p * 1e3).filter(|p| (p.x - x).abs() < 1e-6).collect();
    ends.sort_by(|a, b| a.z.total_cmp(&b.z).then(a.y.total_cmp(&b.y)));
    vertex_at(r, *ends.first().unwrap_or_else(|| panic!("no bend end at x = {x}")))
}

/// Bend relief only edits a relief there is (live, behaviours psui sm-060…066 and the bend-relief-new-edit-reedit
/// recording): a flange along the plate's whole edge runs its bend out to the sheet's side, no corner there, so
/// "Selection is not a bend relief."; a partial flange's bend ends inside the plate's edge, a relief's corner.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn bend_relief_takes_only_a_bend_end_with_a_relief() {
    for (partial, x, ok) in [(false, 60.0, false), (true, 50.0, true)] {
        let mut s = plate("2 mm");
        let r = s.ok();
        let edge = edge_at(&r, DVec3::new(30.0, 40.0, 2.0));
        s.add("sheetMetalFlange", |p| {
            p.set("edges", ParamValue::Query(vec![edge]));
            p.set("distance", expr("20 mm"));
            if partial {
                p.set("isPartialFlange", ParamValue::Bool(true));
                p.set("firstBoundType", en("BLIND"));
                p.set("firstBoundOffset", expr("10 mm"));
                p.set("hasSecondBound", ParamValue::Bool(true));
                p.set("secondBoundType", en("BLIND"));
                p.set("secondBoundOffset", expr("10 mm"));
            }
        });
        let r = s.ok();
        let corner = bend_end_at(&r, x);
        s.add("sheetMetalBendRelief", |p| p.set("bendRelief", ParamValue::Query(vec![corner])));
        let r = Regenerator::new().regenerate(&s.ps, &ConfigValues::default(), &s.units, &RegenOptions::default());
        let last = r.display.features.last().expect("Bend relief's status");
        if ok {
            assert!(matches!(last.status, FeatureStatus::Ok), "partial flange's end: {:?} {:?}", last.status, last.message);
        } else {
            assert!(matches!(last.status, FeatureStatus::Error) && last.message.as_deref() == Some("Selection is not a bend relief."), "whole-edge flange's end: {:?} {:?}", last.status, last.message);
        }
    }
}

/// A flange on a plate's edge whose free side runs on past the bend's end (here an R 20 arc tangent to the edge) tears
/// the plate at the flange's square end: the plate keeps its corner, the bend ends square (live config-assembly
/// 08590a's Flange 1: planar bend ends at the flange's side). 3 mm, R 1,
/// 25 mm Outer: the plate less the 26 × 4 mm strip, the quarter annulus R 1–4 and the 21 mm flange along 26 mm.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn a_flange_beside_a_side_running_on_tears_the_plate() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let p = [(0.0, 0.0), (26.0, 0.0), (26.0, 20.0), (-20.0, 20.0)].map(|(x, y)| sk.add_point(glam::DVec2::new(mm(x), mm(y))));
    for i in 0..3 {
        sk.add_line_between(p[i], p[i + 1]).unwrap();
    }
    let c = sk.add_point(glam::DVec2::new(0.0, mm(20.0)));
    sk.add_arc_between(c, p[3], p[0]).unwrap();
    let sketch = s.add("sketch", |pv| {
        pv.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
        pv.set("sketch", ParamValue::Sketch(std::sync::Arc::new(SketchData { sketch: sk, ..Default::default() })));
    });
    let r = s.ok();
    let key = r.display.sketches.iter().find(|x| x.feature == sketch).expect("sketch").regions[0].key.clone();
    s.add("sheetMetalStart", move |p| {
        p.set("process", en("THICKEN"));
        p.set("regions", ParamValue::Query(vec![Reference::SketchRegion { sketch, key }]));
        p.set("thickness", expr("3 mm"));
        p.set("radius", expr("1 mm"));
    });
    let r = s.ok();
    let edge = edge_at(&r, DVec3::new(13.0, 0.0, 0.0));
    s.add("sheetMetalFlange", |p| {
        p.set("edges", ParamValue::Query(vec![edge]));
        p.set("flangeAlignment", en("OUTER"));
        p.set("distance", expr("25 mm"));
    });
    let r = s.ok();
    let got = volume_mm3(&r);
    let b = &r.context.bodies[part(&r)];
    for f in b.shape.faces() {
        assert!(matches!(f.face_surface(), Ok(noshape_kernel::Surface::Plane(_) | noshape_kernel::Surface::Cylinder { .. })), "a face neither planar nor cylindrical: {:?} (volume {got:.6} mm³)", f.face_surface());
    }
    let region = 46.0 * 20.0 - (400.0 - std::f64::consts::PI * 100.0);
    let want = (region - 26.0 * 4.0) * 3.0 + std::f64::consts::PI / 4.0 * 15.0 * 26.0 + 21.0 * 26.0 * 3.0;
    assert!((got - want).abs() < 1e-3, "volume {got:.6} mm³, want {want:.6}");
}
