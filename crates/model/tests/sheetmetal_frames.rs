//! Sheet metal (flange, hem, tab, bend, joints, reliefs, corner breaks,
//! flat pattern) and frames (frame, trim, gusset, end cap, cut list).

use glam::{DVec2, DVec3};
use noshape_model::configuration::ConfigValues;
use noshape_model::display::FeatureStatus;
use noshape_model::params::{ParamValue, ParamValues};
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, FeatureId, Reference, Role, TopoName};
use noshape_model::regen::features::frames::cut_list;
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::sheetmetal::{FlatPattern, flat_pattern_for};
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

fn en(s: &str) -> ParamValue {
    ParamValue::Enum(s.into())
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

    /// Regenerate and assert every feature succeeded.
    fn ok(&mut self) -> RegenResult {
        let r = self.regen();
        for (f, s) in self.ps.features.iter().zip(&r.display.features) {
            assert!(matches!(s.status, FeatureStatus::Ok | FeatureStatus::Info | FeatureStatus::Warning), "{} ({}) failed: {:?}", f.name, f.type_id, s.message);
        }
        r
    }

    fn last_status(&mut self) -> (FeatureStatus, String) {
        let r = self.regen();
        let s = r.display.features.last().unwrap();
        (s.status, s.message.clone().unwrap_or_default())
    }
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

fn mm(v: f64) -> f64 {
    v * 1e-3
}

fn polyline(sk: &mut noshape_sketch::Sketch, pts: &[(f64, f64)], closed: bool) -> Vec<u32> {
    let ids: Vec<_> = pts.iter().map(|&(x, y)| sk.add_point(DVec2::new(mm(x), mm(y)))).collect();
    let n = if closed { ids.len() } else { ids.len() - 1 };
    (0..n).map(|i| sk.add_line_between(ids[i], ids[(i + 1) % ids.len()]).unwrap().0).collect()
}

fn entities(sketch: FeatureId, ids: &[u32]) -> ParamValue {
    ParamValue::Query(ids.iter().map(|&e| Reference::SketchEntity { sketch, entity: e }).collect())
}

fn edge_at(r: &RegenResult, bi: usize, p: DVec3) -> Reference {
    let b = &r.context.bodies[bi];
    for (i, e) in b.shape.edges().iter().enumerate() {
        let (a, c) = e.endpoints().unwrap();
        if ((a + c) * 0.5 - p).length() < 1e-7 {
            return Reference::Edge((*b.edges[i]).clone());
        }
    }
    panic!("no edge at {p:?}");
}

fn face_at(r: &RegenResult, bi: usize, p: DVec3) -> Reference {
    let b = &r.context.bodies[bi];
    for (i, f) in b.shape.faces().iter().enumerate() {
        if (f.centroid().unwrap() - p).length() < 1e-7 {
            return Reference::Face((*b.faces[i]).clone());
        }
    }
    panic!("no face at {p:?}");
}

fn vertex_at(r: &RegenResult, bi: usize, p: DVec3) -> Reference {
    let b = &r.context.bodies[bi];
    for (i, v) in b.shape.vertices().iter().enumerate() {
        if (v.vertex_point().unwrap() - p).length() < 1e-7 {
            return Reference::Vertex((*b.vertices[i]).clone());
        }
    }
    panic!("no vertex at {p:?}");
}

fn flat(r: &RegenResult, bi: usize) -> FlatPattern {
    flat_pattern_for(&r.context, &r.context.bodies[bi].id).expect("flat pattern")
}

fn volume(r: &RegenResult, bi: usize) -> f64 {
    r.context.bodies[bi].shape.volume().unwrap()
}

/// Developed length of a bend: BA = angle * (R + K*T).
fn ba(angle: f64, r: f64, k: f64, t: f64) -> f64 {
    angle * (r + k * t)
}

fn set_model(p: &mut ParamValues, t: &str, r: &str) {
    p.set("thickness", expr(t));
    p.set("radius", expr(r));
    p.set("kFactor", expr("0.45"));
}

/// Sheet metal extrude of a sketch polyline (mm), depth 20, T 2, R 3.
fn extrude_model(pts: &[(f64, f64)]) -> Studio {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let ids = polyline(&mut sk, pts, false);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    s.add("sheetMetalStart", move |p| {
        p.set("process", en("EXTRUDE"));
        p.set("sketchCurves", entities(sid, &ids));
        p.set("depth", expr("20 mm"));
        set_model(p, "2 mm", "3 mm");
    });
    s
}

/// Thickened 60 x 40 x 2 mm plate on the Top plane (z from 0 to 2 mm).
fn plate() -> Studio {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    polyline(&mut sk, &[(0.0, 0.0), (60.0, 0.0), (60.0, 40.0), (0.0, 40.0)], true);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let r = s.regen();
    let key = r.display.sketches[0].regions[0].key.clone();
    s.add("sheetMetalStart", move |p| {
        p.set("process", en("THICKEN"));
        p.set("regions", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key }]));
        set_model(p, "2 mm", "3 mm");
    });
    s
}

/// Flat extent perpendicular to / along the first bend line.
fn across_along(f: &FlatPattern) -> (f64, f64) {
    let bl = &f.bend_lines[0];
    let along = (bl.b - bl.a).normalize();
    let perp = DVec2::new(-along.y, along.x);
    let ext = |d: DVec2| {
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for p in f.loops.iter().flatten() {
            lo = lo.min(p.dot(d));
            hi = hi.max(p.dot(d));
        }
        hi - lo
    };
    (ext(perp), ext(along))
}

fn sorted_size(f: &FlatPattern) -> (f64, f64) {
    let s = f.size();
    (s.x.max(s.y), s.x.min(s.y))
}

#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn l_bracket_flat_length() {
    let mut s = extrude_model(&[(0.0, 0.0), (50.0, 0.0), (50.0, 30.0)]);
    let r = s.ok();
    let data = r.context.sheet_metal.get(&r.context.bodies[0].id).expect("sheet metal data");
    assert_eq!(data.walls.len(), 2);
    assert_eq!(data.bends.len(), 1);
    let (t, rad, k) = (mm(2.0), mm(3.0), 0.45);
    // Live ("fs (10-23 05:00)"): material outside the L by default, the sketch is
    // the inner face, walls lose R each: flat 80.126106 x 20, 3211.3274122872 mm³.
    let expected = (mm(50.0) - rad) + (mm(30.0) - rad) + ba(PI / 2.0, rad, k, t);
    assert!(near(volume(&r, 0) * 1e9, 3_211.327_412_287_2, 1e-6), "volume {}", volume(&r, 0) * 1e9);
    let bb = r.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!(near(bb.min.y, -t, 1e-7) && near(bb.max.x, mm(52.0), 1e-7), "material outside: {bb:?}");
    let fp = flat(&r, 0);
    let (len, width) = sorted_size(&fp);
    assert!(near(len, expected, 1e-7), "flat length {len} vs {expected}");
    assert!(near(width, mm(20.0), 1e-9), "flat width {width}");
    assert!(near(fp.area(), expected * mm(20.0), 1e-10), "flat area {}", fp.area());
    assert_eq!(fp.bend_lines.len(), 1);
    assert!(fp.warnings.is_empty(), "{:?}", fp.warnings);
    // Bend line runs across the width.
    let bl = &fp.bend_lines[0];
    assert!(near((bl.b - bl.a).length(), mm(20.0), 1e-9));

    // Opposite direction: material inside, the sketch is the outer face, walls lose
    // R + T each (live: 76.126106 x 20, 3051.3274122872 mm³).
    let mut s = extrude_model(&[(0.0, 0.0), (50.0, 0.0), (50.0, 30.0)]);
    s.ps.features.last_mut().unwrap().params.set("oppositeDirection", ParamValue::Bool(true));
    let r = s.ok();
    assert!(near(volume(&r, 0) * 1e9, 3_051.327_412_287_2, 1e-6), "volume {}", volume(&r, 0) * 1e9);
    let expected = (mm(50.0) - (rad + t)) + (mm(30.0) - (rad + t)) + ba(PI / 2.0, rad, k, t);
    let (len, _) = sorted_size(&flat(&r, 0));
    assert!(near(len, expected, 1e-7), "flat length {len} vs {expected}");
}

#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn u_channel_flat_length_and_export() {
    let mut s = extrude_model(&[(0.0, 30.0), (0.0, 0.0), (50.0, 0.0), (50.0, 30.0)]);
    let r = s.ok();
    let (t, rad, k) = (mm(2.0), mm(3.0), 0.45);
    // Material outside (the default, as live's L): the sketch is the inner face.
    let expected = (mm(30.0) - rad) + (mm(50.0) - 2.0 * rad) + (mm(30.0) - rad) + 2.0 * ba(PI / 2.0, rad, k, t);
    let fp = flat(&r, 0);
    let (len, width) = sorted_size(&fp);
    assert!(near(len, expected, 1e-7), "flat length {len} vs {expected}");
    assert!(near(width, mm(20.0), 1e-9));
    assert_eq!(fp.bend_lines.len(), 2);
    assert_eq!(fp.loops.len(), 1, "one outline loop");
    // Sheet geometry + exporters.
    let g = fp.to_sheet_geometry();
    assert!(g.lines.iter().any(|l| l.closed));
    assert_eq!(g.lines.iter().filter(|l| l.style == noshape_model::drawing::sheet::LineStyle::Hidden).count(), 2);
    assert!(g.texts.iter().all(|t| t.text.starts_with("UP") || t.text.starts_with("DOWN")));
    let dxf = noshape_model::drawing::export::to_dxf(&g);
    assert!(dxf.contains("HIDDEN") && (dxf.contains("UP") || dxf.contains("DOWN")));
    let svg = noshape_model::drawing::export::to_svg(&g);
    assert!(svg.contains("<svg"));
}

#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn flange_outer_alignment_volume_and_flat() {
    let mut s = plate();
    let r = s.ok();
    let edge = edge_at(&r, 0, DVec3::new(mm(30.0), mm(40.0), mm(2.0)));
    s.add("sheetMetalFlange", move |p| {
        p.set("edges", ParamValue::Query(vec![edge]));
        p.set("flangeAlignment", en("OUTER"));
        p.set("distance", expr("20 mm"));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 1);
    let (t, rad, k) = (mm(2.0), mm(3.0), 0.45);
    let base = mm(60.0) * (mm(40.0) - (rad + t)) * t;
    let bend = PI / 4.0 * ((rad + t).powi(2) - rad.powi(2)) * mm(60.0);
    let wall = (mm(20.0) - (rad + t)) * t * mm(60.0);
    let v = volume(&r, 0);
    assert!(near(v, base + bend + wall, 1e-12), "volume {v} vs {}", base + bend + wall);
    // Outer face of the flange is flush with the original edge (y = 40).
    let bb = r.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!(near(bb.max.y, mm(40.0), 1e-7) && near(bb.max.z, mm(20.0), 1e-7), "{bb:?}");
    let fp = flat(&r, 0);
    let (len, width) = across_along(&fp);
    let expected = (mm(40.0) - (rad + t)) + ba(PI / 2.0, rad, k, t) + (mm(20.0) - (rad + t));
    assert!(near(len, expected, 1e-7), "flat {len} vs {expected}");
    assert!(near(width, mm(60.0), 1e-9));
    // Seen from the definition (top) side, as live shows Thicken models (SM1), the flange bends up.
    assert!(fp.bend_lines[0].up);
}

/// Live SM1 ("qa5 sheet metal" (2)-(4), (8), (10)): Thicken of (0,0)-(60,40) on
/// Top, t 2, r 2; Flange 20 mm on the top edges y = 0 and x = 0 (Inner); Hem on
/// the top edge x = 60. The sheet metal table lists Bend B, Bend C and Hem D as
/// Up, and the flat DXF (samples/qa5-sm-flat.dxf, model x / y) has its 3 bend
/// lines on SHEETMETAL_BEND_LINES_UP, the flanges' at x = -0.27765 and y = -0.27765 mm.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn sm1_bends_are_up_in_table_flat_notes_and_dxf() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    polyline(&mut sk, &[(0.0, 0.0), (60.0, 0.0), (60.0, 40.0), (0.0, 40.0)], true);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let key = s.regen().display.sketches[0].regions[0].key.clone();
    s.add("sheetMetalStart", move |p| {
        p.set("process", en("THICKEN"));
        p.set("regions", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key }]));
        set_model(p, "2 mm", "2 mm");
    });
    let r = s.ok();
    let (e1, e2) = (edge_at(&r, 0, DVec3::new(mm(30.0), 0.0, mm(2.0))), edge_at(&r, 0, DVec3::new(0.0, mm(20.0), mm(2.0))));
    s.add("sheetMetalFlange", move |p| {
        p.set("edges", ParamValue::Query(vec![e1, e2]));
        p.set("flangeAlignment", en("INNER"));
        p.set("distance", expr("20 mm"));
    });
    let r = s.ok();
    // The plate's top edge at x = 60 (the flange shortened it).
    let b = &r.context.bodies[0];
    let hem = b.shape.edges().iter().position(|e| e.endpoints().is_ok_and(|(a, c)| [a, c].iter().all(|q| near(q.x, mm(60.0), 1e-9) && near(q.z, mm(2.0), 1e-9))));
    let hem = Reference::Edge((*b.edges[hem.expect("top edge at x = 60")]).clone());
    s.add("sheetMetalHem", move |p| p.set("edges", ParamValue::Query(vec![hem])));
    let r = s.ok();
    // Table (the panel's Bend direction column shows these flags).
    let fp = flat(&r, 0);
    assert_eq!(fp.bend_lines.len(), 3);
    assert!(fp.bend_lines.iter().all(|b| b.up), "{:?}", fp.bend_lines.iter().map(|b| (b.a, b.b, b.up)).collect::<Vec<_>>());
    // The flanges' bend lines lie where live's do (the flat in model x / y).
    let c = mm(2.0) - 0.5 * (PI / 2.0) * (mm(2.0) + 0.45 * mm(2.0));
    let lines: Vec<(DVec2, DVec2)> = fp.bend_lines.iter().map(|b| (b.a, b.b)).collect();
    assert!(lines.iter().any(|(a, b)| near(a.x, c, 1e-7) && near(b.x, c, 1e-7)), "x = 0 flange at x = {c}: {lines:?}");
    assert!(lines.iter().any(|(a, b)| near(a.y, c, 1e-7) && near(b.y, c, 1e-7)), "y = 0 flange at y = {c}: {lines:?}");
    // Flat view notes.
    let g = fp.to_sheet_geometry();
    let notes: Vec<&str> = g.texts.iter().map(|t| t.text.as_str()).filter(|t| t.starts_with("UP") || t.starts_with("DOWN")).collect();
    assert!(notes.len() == 3 && notes.iter().all(|n| n.starts_with("UP ")), "{notes:?}");
    // DXF layers.
    let dxf = noshape_model::sheetmetal::flat_dxf::flat_dxf(&fp, &noshape_model::sheetmetal::flat_dxf::FlatExport::default()).unwrap();
    let dxf = String::from_utf8_lossy(&dxf);
    let entities = &dxf[dxf.find("ENTITIES").unwrap()..];
    let entities = &entities[..entities.find("ENDSEC").unwrap()];
    let on = |layer: &str| entities.lines().filter(|l| l.trim() == layer).count();
    assert_eq!((on("SHEETMETAL_BEND_LINES_UP"), on("SHEETMETAL_BEND_LINES_DOWN")), (3, 0));
}

#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn two_flanges_make_u_channel_and_adjacent_flanges_share_corner() {
    let mut s = plate();
    let r = s.ok();
    let e1 = edge_at(&r, 0, DVec3::new(mm(30.0), mm(40.0), mm(2.0)));
    let e2 = edge_at(&r, 0, DVec3::new(mm(30.0), 0.0, mm(2.0)));
    s.add("sheetMetalFlange", move |p| {
        p.set("edges", ParamValue::Query(vec![e1, e2]));
        p.set("flangeAlignment", en("OUTER"));
        p.set("distance", expr("20 mm"));
    });
    let r = s.ok();
    let (t, rad, k) = (mm(2.0), mm(3.0), 0.45);
    let fp = flat(&r, 0);
    let (len, _) = across_along(&fp);
    let expected = (mm(40.0) - 2.0 * (rad + t)) + 2.0 * ba(PI / 2.0, rad, k, t) + 2.0 * (mm(20.0) - (rad + t));
    assert!(near(len, expected, 1e-7), "U flat {len} vs {expected}");
    assert_eq!(fp.bend_lines.len(), 2);

    // Adjacent edges in one feature: flanges are shortened at the corner.
    let mut s = plate();
    let r = s.ok();
    let e1 = edge_at(&r, 0, DVec3::new(mm(30.0), mm(40.0), mm(2.0)));
    let e2 = edge_at(&r, 0, DVec3::new(mm(60.0), mm(20.0), mm(2.0)));
    s.add("sheetMetalFlange", move |p| {
        p.set("edges", ParamValue::Query(vec![e1, e2]));
        p.set("flangeAlignment", en("OUTER"));
        p.set("distance", expr("15 mm"));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 1);
    let data = r.context.sheet_metal.get(&r.context.bodies[0].id).unwrap();
    assert_eq!(data.bends.len(), 2);
    let fp = flat(&r, 0);
    assert_eq!(fp.bend_lines.len(), 2);
    assert!(fp.area() > mm(60.0) * mm(40.0) * 0.8);
}

#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn hem_straight_and_rolled() {
    let mut s = plate();
    let r = s.ok();
    let edge = edge_at(&r, 0, DVec3::new(mm(30.0), mm(40.0), mm(2.0)));
    s.add("sheetMetalHem", move |p| {
        p.set("edges", ParamValue::Query(vec![edge]));
        p.set("hemType", en("STRAIGHT"));
        p.set("length", expr("10 mm"));
    });
    let r = s.ok();
    let data = r.context.sheet_metal.get(&r.context.bodies[0].id).unwrap();
    assert_eq!(data.bends.len(), 1);
    assert!(near(data.bends[0].angle, PI, 1e-12));
    let (t, k) = (mm(2.0), 0.45);
    let rad = data.minimal_clearance * 0.5;
    let fp = flat(&r, 0);
    let (len, _) = across_along(&fp);
    let expected = (mm(40.0) - (rad + t)) + ba(PI, rad, k, t) + (mm(10.0) - (rad + t));
    assert!(near(len, expected, 1e-7), "hem flat {len} vs {expected}");
    // The hem folds back over the plate: total height = 2T + gap (the box is padded by 1e-7).
    let bb = r.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!(near(bb.max.z, 2.0 * t + 2.0 * rad, 1e-6), "{bb:?}");

    let mut s = plate();
    let r = s.ok();
    let edge = edge_at(&r, 0, DVec3::new(mm(30.0), 0.0, mm(2.0)));
    s.add("sheetMetalHem", move |p| {
        p.set("edges", ParamValue::Query(vec![edge]));
        p.set("hemType", en("ROLLED"));
        p.set("innerRadius", expr("3 mm"));
        p.set("angle", expr("270 deg"));
    });
    let r = s.ok();
    let fp = flat(&r, 0);
    let (len, _) = across_along(&fp);
    // A rolled hem is a bend: the bend K factor (the Rolled K factor is for rolled walls).
    let expected = (mm(40.0) - mm(5.0)) + ba(1.5 * PI, mm(3.0), 0.45, mm(2.0));
    assert!(near(len, expected, 1e-7), "rolled flat {len} vs {expected}");

    let mut s = plate();
    let r = s.ok();
    let edge = edge_at(&r, 0, DVec3::new(mm(30.0), 0.0, mm(2.0)));
    s.add("sheetMetalHem", move |p| {
        p.set("edges", ParamValue::Query(vec![edge]));
        p.set("hemType", en("TEAR_DROP"));
        p.set("innerRadius", expr("2 mm"));
        p.set("length", expr("12 mm"));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 1);
    let data = r.context.sheet_metal.get(&r.context.bodies[0].id).unwrap();
    assert!(data.bends[0].angle > PI && data.bends[0].angle < 1.5 * PI);
    flat(&r, 0);
}

#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn bend_feature_keeps_flat_length() {
    let mut s = plate();
    let mut sk = noshape_sketch::Sketch::new();
    let line = polyline(&mut sk, &[(30.0, -10.0), (30.0, 50.0)], false);
    let lid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let r = s.ok();
    let top = face_at(&r, 0, DVec3::new(mm(30.0), mm(20.0), mm(2.0)));
    s.add("sheetMetalBend", move |p| {
        p.set("bendReference", entities(lid, &line));
        p.set("face", ParamValue::Query(vec![top]));
        p.set("bendAngle", expr("90 deg"));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 1);
    // One bend: inner radius 3, outer 5 (T 2).
    let mut radii: Vec<f64> = r.context.bodies[0].shape.faces().iter().filter_map(|f| match f.face_surface().unwrap() {
        noshape_kernel::Surface::Cylinder { radius, .. } => Some(radius),
        _ => None,
    }).collect();
    radii.sort_by(f64::total_cmp);
    assert!(radii.len() == 2 && near(radii[0], mm(3.0), 1e-9) && near(radii[1], mm(5.0), 1e-9), "{radii:?}");
    let fp = flat(&r, 0);
    assert_eq!(fp.bend_lines.len(), 1);
    let (len, width) = sorted_size(&fp);
    assert!(near(len, mm(60.0), 1e-7), "bent plate flat {len}");
    assert!(near(width, mm(40.0), 1e-9));
    // Folded: one side rises above the plate.
    let bb = r.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!(bb.max.z > mm(20.0), "{bb:?}");
}

#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn convert_box_with_bends_and_joints() {
    // An open box (top excluded) converted with its front and back bottom
    // edges as bends: walls outward (live G34d), every other corner an
    // edge-joint rip, so the left and right walls come off as their own parts.
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    polyline(&mut sk, &[(0.0, 0.0), (100.0, 0.0), (100.0, 50.0), (0.0, 50.0)], true);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let r = s.regen();
    let key = r.display.sketches[0].regions[0].key.clone();
    s.add("extrude", move |p| {
        p.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key }]));
        p.set("depth", expr("25 mm"));
    });
    let r = s.ok();
    let body = r.context.bodies[0].id.clone();
    let top = face_at(&r, 0, DVec3::new(mm(50.0), mm(25.0), mm(25.0)));
    let front_edge = edge_at(&r, 0, DVec3::new(mm(50.0), 0.0, 0.0));
    let back_edge = edge_at(&r, 0, DVec3::new(mm(50.0), mm(50.0), 0.0));
    s.add("sheetMetalStart", move |p| {
        p.set("process", en("CONVERT"));
        p.set("partToConvert", ParamValue::Query(vec![Reference::Body(body)]));
        p.set("facesToExclude", ParamValue::Query(vec![top]));
        p.set("bends", ParamValue::Query(vec![front_edge, back_edge]));
        set_model(p, "2 mm", "3 mm");
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 3, "bottom with front and back; left; right");
    let main = (0..3).find(|&i| r.context.sheet_metal.get(&r.context.bodies[i].id).unwrap().bends.len() == 2).expect("the bent part");
    let data = r.context.sheet_metal.get(&r.context.bodies[main].id).unwrap().clone();
    assert_eq!(data.walls.len(), 3, "bottom, front, back");
    // Rips to the left and right walls: at the bottom and both side walls.
    assert_eq!(data.rips.len(), 6);
    assert!(data.rips.iter().all(|x| x.b == usize::MAX && x.other.is_some()));
    for i in 0..3 {
        let d = r.context.sheet_metal.get(&r.context.bodies[i].id).unwrap();
        assert!(!d.walls.is_empty() && d.rips.iter().all(|x| x.a < d.walls.len()), "part {i}");
    }
    let fp = flat(&r, main);
    assert_eq!(fp.bend_lines.len(), 2);
    // The walls' definition faces are the box's: bends of inner radius 3 at
    // its edges.
    let expected = mm(50.0 - 2.0 * 3.0) + 2.0 * ba(PI / 2.0, mm(3.0), 0.45, mm(2.0)) + 2.0 * mm(25.0 - 3.0);
    let along = fp.bend_lines.iter().map(|b| (b.b - b.a).normalize()).next().unwrap();
    let perp = DVec2::new(-along.y, along.x);
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in fp.loops.iter().flatten() {
        let d = p.dot(perp);
        lo = lo.min(d);
        hi = hi.max(d);
    }
    assert!(near(hi - lo, expected, 1e-7), "strip {} vs {expected}", hi - lo);
    // Modify joint: change the radius of the second bend.
    let bend_face = data.bends[1].inner_faces[0].clone();
    s.add("sheetMetalJoint", move |p| {
        p.set("entity", ParamValue::Query(vec![Reference::Face((*bend_face).clone())]));
        p.set("jointType", en("BEND"));
        p.set("useDefaultRadius", ParamValue::Bool(false));
        p.set("radius", expr("5 mm"));
    });
    let r = s.ok();
    let main = (0..3).find(|&i| r.context.sheet_metal.get(&r.context.bodies[i].id).unwrap().bends.len() == 2).expect("the bent part");
    let data = r.context.sheet_metal.get(&r.context.bodies[main].id).unwrap().clone();
    assert!(data.bends.iter().any(|b| near(b.radius, mm(5.0), 1e-12)));
    assert_eq!(r.context.bodies[main].shape.count(noshape_kernel::ShapeType::Solid), 1);
    // Make joint where the front wall meets the right wall: they already
    // meet at a rip.
    let b = &r.context.bodies[main];
    let rip_face = b.faces.iter().position(|n| data.rips.iter().any(|x| x.has_face(n))).expect("a rip face on the bent part");
    let pick = Reference::Face((*b.faces[rip_face]).clone());
    s.add("sheetMetalMakeJoint", move |p| {
        p.set("entities", ParamValue::Query(vec![pick]));
        p.set("joint", en("BEND"));
    });
    let (st, msg) = s.last_status();
    assert_eq!((st, msg.as_str()), (FeatureStatus::Error, "An internal edge cannot be used to create a joint."));
}

#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn corner_break_and_relief_and_tab() {
    // Corner break fillet on a plate corner.
    let mut s = plate();
    let r = s.ok();
    let v = vertex_at(&r, 0, DVec3::new(0.0, 0.0, mm(2.0)));
    s.add("sheetMetalCornerBreak", move |p| {
        p.set("entities", ParamValue::Query(vec![v]));
        p.set("blendType", en("FILLET"));
        p.set("radius", expr("5 mm"));
    });
    let r = s.ok();
    let t = mm(2.0);
    let expected = mm(60.0) * mm(40.0) * t - (mm(5.0).powi(2) - PI * mm(5.0).powi(2) / 4.0) * t;
    assert!(near(volume(&r, 0), expected, 1e-13), "corner break volume {}", volume(&r, 0));
    let fp = flat(&r, 0);
    assert!(near(fp.area(), expected / t, 1e-10), "flat area {}", fp.area());

    // Partial flange gets automatic bend reliefs at both ends.
    let mut s = plate();
    let r = s.ok();
    let edge = edge_at(&r, 0, DVec3::new(mm(30.0), mm(40.0), mm(2.0)));
    s.add("sheetMetalFlange", move |p| {
        p.set("edges", ParamValue::Query(vec![edge]));
        p.set("flangeAlignment", en("OUTER"));
        p.set("distance", expr("20 mm"));
        p.set("isPartialFlange", ParamValue::Bool(true));
        p.set("firstBoundOffset", expr("10 mm"));
        p.set("hasSecondBound", ParamValue::Bool(true));
        p.set("secondBoundOffset", expr("10 mm"));
    });
    let r = s.ok();
    // The automatic bend reliefs (Obround, width scale 1.0625: 2.125 mm) cut
    // the plate beside both flange ends, x 7.875 and 52.125.
    let relief_sides: Vec<f64> = r.context.bodies[0]
        .shape
        .faces()
        .iter()
        .filter_map(|f| {
            let bb = f.bounding_box(true).ok()?;
            let n = f.face_plane().ok()??.unit_normal();
            (n.x.abs() > 1.0 - 1e-9 && bb.max.z < mm(2.0) + 1e-6 && bb.min.y > mm(30.0)).then_some((bb.min.x + bb.max.x) * 0.5 * 1e3)
        })
        .collect();
    assert!(relief_sides.iter().any(|x| (x - 7.875).abs() < 1e-6) && relief_sides.iter().any(|x| (x - 52.125).abs() < 1e-6), "relief sides at {relief_sides:?}");
    let v = volume(&r, 0);
    assert_eq!(r.context.bodies[0].shape.count(noshape_kernel::ShapeType::Solid), 1);
    let fp = flat(&r, 0);
    assert_eq!(fp.bend_lines.len(), 1);

    // Bend relief override at one end of the bend.
    let data = r.context.sheet_metal.get(&r.context.bodies[0].id).unwrap().clone();
    let b = &data.bends[0];
    let end_pt = b.origin + b.axis * b.s0;
    let near_vertex = {
        let body = &r.context.bodies[0];
        let mut best = (f64::INFINITY, 0usize);
        for (i, vv) in body.shape.vertices().iter().enumerate() {
            let d = (vv.vertex_point().unwrap() - end_pt).length();
            if d < best.0 {
                best = (d, i);
            }
        }
        Reference::Vertex((*body.vertices[best.1]).clone())
    };
    s.add("sheetMetalBendRelief", move |p| {
        p.set("bendRelief", ParamValue::Query(vec![near_vertex]));
        p.set("bendReliefStyle", en("SIZED_RECTANGLE"));
        p.set("bendReliefDepth", expr("6 mm"));
    });
    let r2 = s.ok();
    assert!(volume(&r2, 0) < v - 1e-12, "relief override removes material");

    // Tab: a rectangle overlapping the plate edge, in the bottom face plane.
    let mut s = plate();
    let mut sk = noshape_sketch::Sketch::new();
    polyline(&mut sk, &[(20.0, 30.0), (40.0, 30.0), (40.0, 55.0), (20.0, 55.0)], true);
    let tid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let r = s.ok();
    let sk = r.display.sketches.iter().find(|x| x.feature == tid).unwrap();
    let key = sk.regions.iter().max_by(|a, b| a.area.total_cmp(&b.area)).unwrap().key.clone();
    // "Flange to merge": the plate face the dialog's editing logic fills in
    // (std sheetMetalTabEditingLogic; live qa5 4.2), here its bottom face, in
    // the sketch plane.
    let merge = face_at(&r, 0, DVec3::new(mm(30.0), mm(20.0), 0.0));
    s.add("sheetMetalTab", move |p| {
        p.set("tabFaces", ParamValue::Query(vec![Reference::SketchRegion { sketch: tid, key }]));
        p.set("booleanUnionScope", ParamValue::Query(vec![merge]));
    });
    let r = s.ok();
    let expected = mm(60.0) * mm(40.0) * t + mm(20.0) * mm(15.0) * t;
    assert!(near(volume(&r, 0), expected, 1e-13), "tab volume {}", volume(&r, 0));
    let fp = flat(&r, 0);
    assert!(near(fp.area(), expected / t, 1e-10), "tab flat area {}", fp.area());
}

#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn finish_sheet_metal_deactivates() {
    let mut s = plate();
    let r = s.ok();
    // Finish needs its parts (live: "Select one or more sheet metal parts.").
    let part = Reference::Body(r.context.bodies[0].id.clone());
    s.add("sheetMetalEnd", move |p| p.set("sheetMetalParts", ParamValue::Query(vec![part])));
    let r = s.ok();
    assert!(!r.context.sheet_metal.values().next().unwrap().active);
    // The flat pattern stays available.
    flat(&r, 0);
    // A flange on the finished part's edge (picked after Finish, as in the
    // dialog): std getSMDefinitionEdgesForFlangeTypeFeature's
    // SHEET_METAL_ACTIVE_EDGE_NEEDED.
    let edge = edge_at(&r, 0, DVec3::new(mm(30.0), mm(40.0), mm(2.0)));
    s.add("sheetMetalFlange", move |p| p.set("edges", ParamValue::Query(vec![edge])));
    let (st, msg) = s.last_status();
    assert_eq!(st, FeatureStatus::Error);
    assert_eq!(msg, "Selected edges should be from active sheet metal models.");
}

#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn flange_names_are_stable_across_upstream_edits() {
    let mut s = plate();
    let r = s.ok();
    let edge = edge_at(&r, 0, DVec3::new(mm(30.0), mm(40.0), mm(2.0)));
    s.add("sheetMetalFlange", move |p| {
        p.set("edges", ParamValue::Query(vec![edge]));
        p.set("distance", expr("20 mm"));
    });
    let r1 = s.ok();
    let names1: Vec<_> = r1.context.bodies[0].faces.iter().filter(|n| matches!(&***n, TopoName::Face { role: Role::Side(_), .. })).cloned().collect();
    // Change the flange length: same face names.
    s.ps.features.last_mut().unwrap().params.set("distance", expr("25 mm"));
    let r2 = s.ok();
    for n in &names1 {
        assert!(!r2.context.bodies[0].find_faces(n).is_empty(), "lost {n:?}");
    }
}

// ------------------------------------------------------------------ frames

fn frame_on(s: &mut Studio, sid: FeatureId, ids: &[u32], profile: &str, corner: &str) -> FeatureId {
    let ids = ids.to_vec();
    let profile = profile.to_string();
    let corner = corner.to_string();
    s.add("frame", move |p| {
        p.set("profileSketch", ParamValue::Text(profile));
        p.set("selections", entities(sid, &ids));
        p.set("defaultCornerType", en(&corner));
    })
}

#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn frame_rectangle_mitered_cut_list() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let ids = polyline(&mut sk, &[(0.0, 0.0), (200.0, 0.0), (200.0, 100.0), (0.0, 100.0)], true);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    frame_on(&mut s, sid, &ids, "SHS 40x40x3", "MITER");
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 4);
    let area = mm(40.0).powi(2) - mm(34.0).powi(2);
    let total: f64 = (0..4).map(|i| volume(&r, i)).sum();
    assert!(near(total, area * mm(600.0), 1e-12), "frame volume {total}");
    let rows = cut_list(&r.context, None);
    assert_eq!(rows.len(), 2, "{rows:?}");
    let mut lens: Vec<f64> = rows.iter().map(|x| x.length.unwrap()).collect();
    lens.sort_by(f64::total_cmp);
    assert!(near(lens[0], mm(140.0), 1e-7) && near(lens[1], mm(240.0), 1e-7), "{lens:?}");
    for row in &rows {
        assert_eq!(row.qty, 2);
        assert!(near(row.angle1.unwrap(), PI / 4.0, 1e-6) && near(row.angle2.unwrap(), PI / 4.0, 1e-6), "{row:?}");
        assert!(row.description.contains("SHS 40x40x3"));
    }
    s.add("cutlist", |p| p.set("selectAllFrames", ParamValue::Bool(true)));
    s.ok();
    // A Tag with no sketch (std doTagProfile).
    s.add("tag", |_| {});
    assert_eq!(s.last_status(), (FeatureStatus::Error, "Select a single sketch feature. The selected feature must create sketch regions.".to_string()));
}

#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn frame_butt_endcap_gusset_and_trim() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let ids = polyline(&mut sk, &[(0.0, 0.0), (200.0, 0.0), (200.0, 100.0)], false);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    frame_on(&mut s, sid, &ids, "SHS 40x40x3", "BUTT");
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 2);
    let rows = cut_list(&r.context, None);
    let mut lens: Vec<f64> = rows.iter().map(|x| x.length.unwrap()).collect();
    lens.sort_by(f64::total_cmp);
    assert!(near(lens[0], mm(80.0), 1e-7) && near(lens[1], mm(220.0), 1e-7), "{lens:?}");
    assert!(rows.iter().all(|x| near(x.angle1.unwrap(), 0.0, 1e-9) && near(x.angle2.unwrap(), 0.0, 1e-9)));
    // End cap on the free end at the origin.
    let end = face_at(&r, 0, DVec3::new(0.0, 0.0, 0.0));
    s.add("endcap", move |p| {
        p.set("faces", ParamValue::Query(vec![end]));
        p.set("thickness", expr("5 mm"));
    });
    // Gusset in the inner corner.
    let corner_edge = edge_at(&r, 1, DVec3::new(mm(180.0), mm(20.0), 0.0));
    s.add("gusset", move |p| {
        p.set("edges", ParamValue::Query(vec![corner_edge]));
        p.set("length", expr("30 mm"));
        p.set("thickness", expr("5 mm"));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 4);
    assert!(near(volume(&r, 2), mm(40.0).powi(2) * mm(5.0), 1e-13), "end cap volume {}", volume(&r, 2));
    assert!(near(volume(&r, 3), mm(30.0).powi(2) * 0.5 * mm(5.0), 1e-13), "gusset volume {}", volume(&r, 3));
    let rows = cut_list(&r.context, None);
    assert!(rows.iter().any(|x| x.description.starts_with("End cap")));
    assert!(rows.iter().any(|x| x.description.starts_with("Gusset")));
}

#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn frame_trim_groups_and_face() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let a = polyline(&mut sk, &[(0.0, 0.0), (200.0, 0.0)], false);
    let b = polyline(&mut sk, &[(100.0, -50.0), (100.0, 100.0)], false);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    frame_on(&mut s, sid, &[a[0], b[0]], "SHS 40x40x3", "MITER");
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 2);
    let (ida, idb) = (r.context.bodies[0].id.clone(), r.context.bodies[1].id.clone());
    s.add("frameTrim", move |p| {
        let g = |id: noshape_model::reference::BodyId| {
            let mut v = ParamValues::default();
            v.set("frames", ParamValue::Query(vec![Reference::Body(id)]));
            v
        };
        p.set("frameGroups", ParamValue::Array(vec![g(ida), g(idb)]));
    });
    let r = s.ok();
    let area = mm(40.0).powi(2) - mm(34.0).powi(2);
    assert!(near(volume(&r, 0), area * mm(200.0), 1e-12));
    assert!(near(volume(&r, 1), area * mm(80.0), 1e-12), "trimmed {}", volume(&r, 1));
    // Face trim with the Right plane (x = 0 plane keeps x < 0 side by default).
    let id0 = r.context.bodies[0].id.clone();
    s.add("cPlane", |p| {
        p.set("entities", ParamValue::Query(vec![Reference::Plane(DatumPlane::Right)]));
        p.set("offset", expr("150 mm"));
    });
    let r = s.ok();
    let plane_ref = r.context.planes.last().map(|p| p.reference.clone()).unwrap();
    s.add("frameTrim", move |p| {
        p.set("trimType", en("FACE"));
        p.set("targets", ParamValue::Query(vec![Reference::Body(id0)]));
        p.set("tool", ParamValue::Query(vec![plane_ref]));
    });
    let r = s.ok();
    assert!(near(volume(&r, 0), area * mm(150.0), 1e-12), "face trimmed {}", volume(&r, 0));
}

#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn frame_profiles_and_curved_path() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let arc = sk.add_arc_angles(DVec2::ZERO, mm(100.0), 0.0, PI / 2.0).0;
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    frame_on(&mut s, sid, &[arc], "round tube 20x2", "MITER");
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 1);
    let area = PI * (mm(10.0).powi(2) - mm(8.0).powi(2));
    let v = volume(&r, 0);
    assert!(near(v, area * mm(100.0) * PI / 2.0, 1e-9 * v.max(1e-9) + 1e-12), "curved volume {v}");
    // Unknown profile: clear error.
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let ids = polyline(&mut sk, &[(0.0, 0.0), (100.0, 0.0)], false);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    frame_on(&mut s, sid, &ids, "nonsense 1x2", "MITER");
    let (st, msg) = s.last_status();
    assert_eq!(st, FeatureStatus::Error);
    assert!(msg.contains("Unknown frame profile"), "{msg}");
    // Angle, L, channel and I-beam profiles sweep along a line.
    for prof in ["L 40x40x4", "UPN 100", "IPE 100", "RHS 60x40x3", "Flat bar 40x10"] {
        let mut s = Studio::new();
        let mut sk = noshape_sketch::Sketch::new();
        let ids = polyline(&mut sk, &[(0.0, 0.0), (100.0, 0.0)], false);
        let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
        frame_on(&mut s, sid, &ids, prof, "MITER");
        let r = s.ok();
        assert_eq!(r.context.bodies.len(), 1, "{prof}");
        let rows = cut_list(&r.context, None);
        assert!(near(rows[0].length.unwrap(), mm(100.0), 1e-9), "{prof}: {rows:?}");
    }
}


#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn frame_sketch_profile_tangent_merge_and_limit_ends() {
    // Profile sketch of this Part Studio (20 x 20 tube, 2 mm wall): only the
    // ring region is material.
    let mut s = Studio::new();
    let mut psk = noshape_sketch::Sketch::new();
    polyline(&mut psk, &[(-10.0, -10.0), (10.0, -10.0), (10.0, 10.0), (-10.0, 10.0)], true);
    polyline(&mut psk, &[(-8.0, -8.0), (8.0, -8.0), (8.0, 8.0), (-8.0, 8.0)], true);
    let pid = s.sketch(Reference::Plane(DatumPlane::Right), psk);
    let pname = s.ps.features.iter().find(|f| f.id == pid).unwrap().name.clone();
    let mut sk = noshape_sketch::Sketch::new();
    let ids = polyline(&mut sk, &[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)], false);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    frame_on(&mut s, sid, &ids, &pname, "MITER");
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 2);
    for i in 0..2 {
        assert!(near(volume(&r, i), mm(144.0) * 1e-3 * mm(100.0), 1e-12), "sketch profile volume {}", volume(&r, i));
    }
    let rows = cut_list(&r.context, None);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].qty, 2);
    assert!(near(rows[0].length.unwrap(), mm(110.0), 1e-7));

    // Line - arc - line: tangent segments merge into one part.
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let p0 = sk.add_point(DVec2::new(0.0, 0.0));
    let p1 = sk.add_point(DVec2::new(mm(100.0), 0.0));
    let p2 = sk.add_point(DVec2::new(mm(150.0), mm(50.0)));
    let p3 = sk.add_point(DVec2::new(mm(150.0), mm(150.0)));
    let l0 = sk.add_line_between(p0, p1).unwrap().0;
    let c = sk.add_point(DVec2::new(mm(100.0), mm(50.0)));
    let a = sk.add_arc_between(c, p1, p2).unwrap().0;
    let l1 = sk.add_line_between(p2, p3).unwrap().0;
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    frame_on(&mut s, sid, &[l0, a, l1], "CHS 21.3x2.3", "MITER");
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 1);
    let area = PI * (mm(10.65).powi(2) - mm(8.35).powi(2));
    let path = mm(200.0) + PI / 2.0 * mm(50.0);
    assert!(near(volume(&r, 0), area * path, area * path * 1e-4), "merged volume {}", volume(&r, 0));

    // Limit frame ends: the free end nearest a plane is extended to it.
    let mut s = Studio::new();
    s.add("cPlane", |p| {
        p.set("entities", ParamValue::Query(vec![Reference::Plane(DatumPlane::Right)]));
        p.set("offset", expr("120 mm"));
    });
    let mut sk = noshape_sketch::Sketch::new();
    let ids = polyline(&mut sk, &[(0.0, 0.0), (100.0, 0.0)], false);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let r = s.regen();
    let pref = r.context.planes.last().map(|p| p.reference.clone()).unwrap();
    s.add("frame", move |p| {
        p.set("profileSketch", ParamValue::Text("SHS 20x20x2".into()));
        p.set("selections", entities(sid, &ids));
        p.set("trim", ParamValue::Bool(true));
        p.set("trimPlanes", ParamValue::Query(vec![pref]));
    });
    let r = s.ok();
    let rows = cut_list(&r.context, None);
    assert!(near(rows[0].length.unwrap(), mm(120.0), 1e-7), "{rows:?}");

    // Closed loop with butt corners: every member runs through at one end
    // and butts at the other (pinwheel), keeping the path lengths.
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let ids = polyline(&mut sk, &[(0.0, 0.0), (200.0, 0.0), (200.0, 100.0), (0.0, 100.0)], true);
    let sid = s.sketch(Reference::Plane(DatumPlane::Front), sk);
    frame_on(&mut s, sid, &ids, "RHS 60x40x3", "COPED_BUTT");
    let r = s.ok();
    let total: f64 = (0..4).map(|i| volume(&r, i)).sum();
    let area = mm(60.0) * mm(40.0) - mm(54.0) * mm(34.0);
    assert!(near(total, area * mm(600.0), 1e-12), "pinwheel volume {total}");
}

/// Convert of a 60 × 20 × 1 plate with every field at its default, as Sheet
/// metal model and as Flex PCB model (live request 1621, qa5 PCB-CONVERT):
/// each face a wall 1.6 mm thick outward, all 12 edges rips trimming 0.02 mm:
/// 6 separate parts (1914.88256 ×2, 92.09856 ×2, 30.65856 ×2 mm³).
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn convert_a_plate_with_defaults_makes_six_walls() {
    for ty in ["sheetMetalStart", "pcbStart"] {
        let mut s = Studio::new();
        let mut sk = noshape_sketch::Sketch::new();
        polyline(&mut sk, &[(0.0, 0.0), (60.0, 0.0), (60.0, 20.0), (0.0, 20.0)], true);
        let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
        let r = s.regen();
        let key = r.display.sketches[0].regions[0].key.clone();
        s.add("extrude", move |p| {
            p.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key }]));
            p.set("depth", expr("1 mm"));
        });
        let r = s.ok();
        let body = r.context.bodies[0].id.clone();
        s.add(ty, move |p| {
            p.set("process", en("CONVERT"));
            p.set("partToConvert", ParamValue::Query(vec![Reference::Body(body)]));
        });
        let r = s.ok();
        let mut v: Vec<f64> = r.context.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).map(|b| b.shape.volume().unwrap() * 1e9).collect();
        v.sort_by(f64::total_cmp);
        let want = [30.65856, 30.65856, 92.09856, 92.09856, 1914.88256, 1914.88256];
        assert!(v.len() == 6 && v.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-4), "{ty}: {v:?}");
    }
}

/// Convert like Onshape (live G34d): the top and +X faces of a 40 x 30 x 20
/// block (the other four excluded), 1 mm thick, bend radius 1 mm, minimal gap
/// 0.02 mm. Walls are built outward from the faces; with no bend edges they
/// meet at an edge-joint rip (each stops 0.02 short of the other's face, the
/// outer corner open) and become two parts.
#[test]
#[ignore = "T2: sheet metal / frames (P3), long std regenerations"]
fn convert_rips_and_bends_like_onshape() {
    let block = |bend: bool| {
        let mut s = Studio::new();
        let mut sk = noshape_sketch::Sketch::new();
        polyline(&mut sk, &[(0.0, 0.0), (40.0, 0.0), (40.0, 30.0), (0.0, 30.0)], true);
        let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
        let r = s.regen();
        let key = r.display.sketches[0].regions[0].key.clone();
        s.add("extrude", move |p| {
            p.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sid, key }]));
            p.set("depth", expr("20 mm"));
        });
        let r = s.ok();
        let body = r.context.bodies[0].id.clone();
        let exclude: Vec<Reference> = [(20.0, 15.0, 0.0), (0.0, 15.0, 10.0), (20.0, 0.0, 10.0), (20.0, 30.0, 10.0)]
            .iter()
            .map(|&(x, y, z)| face_at(&r, 0, DVec3::new(mm(x), mm(y), mm(z))))
            .collect();
        let bends = if bend { vec![edge_at(&r, 0, DVec3::new(mm(40.0), mm(15.0), mm(20.0)))] } else { Vec::new() };
        s.add("sheetMetalStart", move |p| {
            p.set("process", en("CONVERT"));
            p.set("partToConvert", ParamValue::Query(vec![Reference::Body(body)]));
            p.set("facesToExclude", ParamValue::Query(exclude));
            p.set("bends", ParamValue::Query(bends));
            set_model(p, "1 mm", "1 mm");
        });
        s
    };
    let bbox = |r: &RegenResult, bi: usize| r.context.bodies[bi].shape.bounding_box(true).unwrap();
    let mm3 = |v: f64| v * 1e9;
    // Find the body whose box starts at x = 0 (the top wall) and the other.
    let walls = |r: &RegenResult| {
        assert_eq!(r.context.bodies.len(), 2, "two parts");
        if bbox(r, 0).min.x < mm(1.0) { (0, 1) } else { (1, 0) }
    };

    // No bends: an edge-joint rip, two parts.
    let mut s = block(false);
    let r = s.ok();
    let (top, side) = walls(&r);
    assert!(near(mm3(volume(&r, top)), 1199.4, 1e-6) && near(mm3(volume(&r, side)), 599.4, 1e-6), "{} {}", mm3(volume(&r, top)), mm3(volume(&r, side)));
    let (bt, bs) = (bbox(&r, top), bbox(&r, side));
    assert!(near(bt.max.x, mm(39.98), 1e-6) && near(bt.min.z, mm(20.0), 1e-6) && near(bt.max.z, mm(21.0), 1e-6), "{bt:?}");
    assert!(near(bs.min.x, mm(40.0), 1e-6) && near(bs.max.x, mm(41.0), 1e-6) && near(bs.max.z, mm(19.98), 1e-6), "{bs:?}");
    for bi in [top, side] {
        assert!(r.context.sheet_metal.contains_key(&r.context.bodies[bi].id), "both parts are sheet metal");
    }
    // Make joint on the walls' facing ends: they already meet at a joint.
    let end_top = face_at(&r, top, DVec3::new(mm(39.98), mm(15.0), mm(20.5)));
    let end_side = face_at(&r, side, DVec3::new(mm(40.5), mm(15.0), mm(19.98)));
    s.add("sheetMetalMakeJoint", move |p| {
        p.set("entities", ParamValue::Query(vec![end_top, end_side]));
        p.set("joint", en("BEND"));
    });
    let (st, msg) = s.last_status();
    assert_eq!((st, msg.as_str()), (FeatureStatus::Error, "An internal edge cannot be used to create a joint."));

    // The edge as a bend: one part (Onshape 1810.6858347057712, 14 faces).
    let mut s = block(true);
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 1);
    assert!(near(mm3(volume(&r, 0)), 1810.6858347057712, 1e-6), "{}", mm3(volume(&r, 0)));
    assert_eq!(r.context.bodies[0].shape.count(noshape_kernel::ShapeType::Face), 14);
    // The bend changed to a rip (Joint): edge joint, butt joint either way.
    for (style, total, top_x, side_z) in [("EDGE", 1798.8, 39.98, 19.98), ("BUTT", 1829.4, 41.0, 19.98), ("BUTT2", 1829.4, 39.98, 21.0)] {
        let mut s2 = block(true);
        let r = s2.ok();
        let data = r.context.sheet_metal.get(&r.context.bodies[0].id).unwrap().clone();
        let bf = data.bends[0].inner_faces[0].clone();
        s2.add("sheetMetalJoint", move |p| {
            p.set("entity", ParamValue::Query(vec![Reference::Face((*bf).clone())]));
            p.set("jointType", en("RIP"));
            p.set("hasStyle", ParamValue::Bool(true));
            p.set("jointStyle", en(style));
        });
        let r = s2.ok();
        let (top, side) = walls(&r);
        let v = mm3(volume(&r, top) + volume(&r, side));
        assert!(near(v, total, 1e-6), "{style}: {v}");
        assert!(near(bbox(&r, top).max.x, mm(top_x), 1e-6) && near(bbox(&r, side).max.z, mm(side_z), 1e-6), "{style}: {:?} {:?}", bbox(&r, top), bbox(&r, side));
    }
}

