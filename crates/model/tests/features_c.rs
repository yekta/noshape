//! Batch C features: sheet metal Jog, Form, Corner, Sheet metal loft, Wrap,
//! Isocline, Constrained surface and Derived placement.

use glam::{DVec2, DVec3};
use noshape_model::configuration::ConfigValues;
use noshape_model::display::FeatureStatus;
use noshape_model::params::{ParamValue, ParamValues};
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, FeatureId, Reference};
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::sheetmetal::{FlatPattern, flat_pattern_for};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};
use std::f64::consts::PI;

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

fn q(r: Reference) -> ParamValue {
    ParamValue::Query(vec![r])
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

    /// Regenerate and assert every feature succeeded (warnings allowed).
    fn ok(&mut self) -> RegenResult {
        let r = self.regen();
        for (f, s) in self.ps.features.iter().zip(&r.display.features) {
            // With std's FeatureScript notices (std-run features: the failing call's stack).
            assert!(
                matches!(s.status, FeatureStatus::Ok | FeatureStatus::Warning | FeatureStatus::Info),
                "{} ({}) failed: {:?} {:?}",
                f.name,
                f.type_id,
                s.message,
                noshape_model::custom::run::output_of(f.id).map(|o| o.notices)
            );
        }
        r
    }

    fn last_status(&mut self) -> (FeatureStatus, String) {
        let r = self.regen();
        let s = r.display.features.last().unwrap();
        (s.status, s.message.clone().unwrap_or_default())
    }

    fn region(&mut self, sketch: FeatureId) -> Reference {
        let r = self.regen();
        let sk = r.display.sketches.iter().find(|s| s.feature == sketch).expect("sketch");
        let key = sk.regions.iter().max_by(|a, b| a.area.total_cmp(&b.area)).expect("region").key.clone();
        Reference::SketchRegion { sketch, key }
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

fn face_at(r: &RegenResult, bi: usize, p: DVec3) -> Reference {
    let b = &r.context.bodies[bi];
    for (i, f) in b.shape.faces().iter().enumerate() {
        if (f.centroid().unwrap() - p).length() < 1e-7 {
            return Reference::Face((*b.faces[i]).clone());
        }
    }
    panic!("no face at {p:?}");
}

fn flat(r: &RegenResult, bi: usize) -> FlatPattern {
    flat_pattern_for(&r.context, &r.context.bodies[bi].id).expect("flat pattern")
}

fn volume(r: &RegenResult, bi: usize) -> f64 {
    r.context.bodies[bi].shape.volume().unwrap()
}

fn cylinder_radii(r: &RegenResult, bi: usize) -> Vec<f64> {
    let faces = r.context.bodies[bi].shape.faces();
    faces.iter().filter_map(|f| match f.face_surface().unwrap() {
        noshape_kernel::Surface::Cylinder { radius, .. } => Some(radius),
        _ => None,
    }).collect()
}

fn sorted_size(f: &FlatPattern) -> (f64, f64) {
    let s = f.size();
    (s.x.max(s.y), s.x.min(s.y))
}

fn set_model(p: &mut ParamValues, t: &str, r: &str) {
    p.set("thickness", expr(t));
    p.set("radius", expr(r));
    p.set("kFactor", expr("0.45"));
}

/// Thickened 100 x 40 x 2 mm plate on the Top plane (z from 0 to 2 mm).
fn plate() -> Studio {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    polyline(&mut sk, &[(0.0, 0.0), (100.0, 0.0), (100.0, 40.0), (0.0, 40.0)], true);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let region = s.region(sid);
    s.add("sheetMetalStart", move |p| {
        p.set("process", en("THICKEN"));
        p.set("regions", q(region));
        set_model(p, "2 mm", "3 mm");
    });
    s
}

/// Jog of the plate along x = 60 mm (the side with the face center is held).
fn jogged(set: impl FnOnce(&mut ParamValues)) -> Studio {
    let mut s = plate();
    let mut sk = noshape_sketch::Sketch::new();
    let line = polyline(&mut sk, &[(60.0, -10.0), (60.0, 50.0)], false);
    let lid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let r = s.ok();
    let top = face_at(&r, 0, DVec3::new(mm(50.0), mm(20.0), mm(2.0)));
    s.add("sheetMetalJog", move |p| {
        p.set("bendReference", entities(lid, &line));
        p.set("face", q(top));
        p.set("bendAngle", expr("90 deg"));
        p.set("bendOffset", expr("15 mm"));
        p.set("bendOffsetAnchor", en("NOMINAL"));
        set(p);
    });
    s
}

#[test]
fn jog_offsets_wall_and_keeps_flat_length() {
    let mut s = jogged(|_| {});
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 1);
    // Two bends (inner radius 3, outer 5) between the held, middle and jogged walls.
    let mut radii = cylinder_radii(&r, 0);
    radii.sort_by(f64::total_cmp);
    assert_eq!(radii.len(), 4, "{radii:?}");
    assert!(radii.iter().zip([3.0, 3.0, 5.0, 5.0]).all(|(&a, b)| near(a, mm(b), 1e-9)), "{radii:?}");
    let (t, k) = (mm(2.0), 0.45);
    // Flat pattern: preserve material keeps the original length.
    let fp = flat(&r, 0);
    let (len, width) = sorted_size(&fp);
    assert!(near(len, mm(100.0), 1e-7), "jog flat length {len}");
    assert!(near(width, mm(40.0), 1e-9));
    assert_eq!(fp.bend_lines.len(), 2);
    assert!(fp.warnings.is_empty(), "{:?}", fp.warnings);
    // Folded: the far wall is offset by the jog (nominal 15 mm).
    let bb = r.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!(near(bb.max.z, mm(2.0) + mm(15.0), 1e-6), "{bb:?}");
    // Volume: bends are full annular sectors; the flat strip they replace has
    // the developed length (K factor), so V = flat volume + 2 * theta * T^2 * W * (0.5 - K).
    let w = mm(40.0);
    let expected = mm(100.0) * w * t + 2.0 * (PI / 2.0) * t * t * w * (0.5 - k);
    assert!(near(volume(&r, 0), expected, 1e-12), "volume {} vs {expected}", volume(&r, 0));

    // Offset anchors: inside adds the thickness.
    let mut s = jogged(|p| p.set("bendOffsetAnchor", en("INSIDE")));
    let r = s.ok();
    let bb = r.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!(near(bb.max.z, mm(2.0) + mm(17.0), 1e-6), "inside anchor {bb:?}");

    // Thickness bounding type: 5 x T = 10 mm nominal.
    let mut s = jogged(|p| {
        p.set("offsetType", en("THICKNESS"));
        p.set("thicknessFactor", expr("5"));
    });
    let r = s.ok();
    let bb = r.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!(near(bb.max.z, mm(12.0), 1e-6), "thickness factor {bb:?}");
}

#[test]
fn jog_preserve_material_off_and_errors() {
    let mut s = jogged(|p| p.set("preserveMaterial", ParamValue::Bool(false)));
    let r = s.ok();
    let (t, rad, k) = (mm(2.0), mm(3.0), 0.45);
    let ba = (PI / 2.0) * (rad + k * t);
    // Adjacent bends at 90 deg give 2R + T; the wall between = 15 - 8 = 7 mm.
    let wall = mm(15.0) - (2.0 * rad + t);
    // Far end stays at x = 100: folded extent along x unchanged.
    let bb = r.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!(near(bb.max.x, mm(100.0), 1e-6), "far end moved: {bb:?}");
    let fp = flat(&r, 0);
    let (len, _) = sorted_size(&fp);
    // Material added = developed jog length - its horizontal extent.
    // Along the reference (top) side the jog spans R (bend 1, inner) + R + T (bend 2, outer).
    let horiz = 2.0 * rad + t;
    let extra = 2.0 * ba + wall - horiz;
    assert!(near(len, mm(100.0) + extra, 1e-6), "flat {len} vs {}", mm(100.0) + extra);

    // Too small an offset for 90° bends (std applyEditLogicForBendAngle): the angle drops to make the bends
    // adjacent, cos = 1 - 3 / (2R + T) = 5/8, with an Info.
    let mut s = jogged(|p| p.set("bendOffset", expr("3 mm")));
    let (st, msg) = s.last_status();
    assert_eq!((st, msg.as_str()), (FeatureStatus::Info, "Updating bend angle in order to accommodate the specified jog offset."));
    let r = s.ok();
    let bb = r.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!(near(bb.max.z - bb.min.z, mm(3.0) + t, 1e-6), "{bb:?}");
}

/// Derived provider over pre-regenerated studios (tab id -> context).
struct Tabs(std::collections::HashMap<String, std::sync::Arc<noshape_model::regen::context::Context>>);

impl std::fmt::Debug for Tabs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Tabs")
    }
}

impl noshape_model::regen::derived::DerivedProvider for Tabs {
    fn context(&self, tab: &str) -> Option<std::sync::Arc<noshape_model::regen::context::Context>> {
        self.0.get(tab).cloned()
    }
    fn version(&self, _tab: &str) -> u64 {
        1
    }
}

fn circle_sketch(s: &mut Studio, r_mm: f64) -> FeatureId {
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(DVec2::ZERO, mm(r_mm));
    s.sketch(Reference::Plane(DatumPlane::Top), sk)
}

/// Form tool studio: a tube boss, radii 4 to 5, 3 mm high, rooted through
/// the 2 mm sheet (added; std Form registers only tools that intersect the
/// sheet), over a hole of radius 4 through the sheet (removed), tagged as a form.
fn form_studio(tag: bool) -> std::sync::Arc<noshape_model::regen::context::Context> {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(DVec2::ZERO, mm(5.0));
    sk.add_circle(DVec2::ZERO, mm(4.0));
    let a = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let ring = {
        let r = s.regen();
        let sk = r.display.sketches.iter().find(|x| x.feature == a).expect("sketch");
        let key = sk.regions.iter().min_by(|x, y| x.area.total_cmp(&y.area)).expect("region").key.clone();
        Reference::SketchRegion { sketch: a, key }
    };
    s.add("extrude", move |p| {
        p.set("entities", q(ring));
        p.set("depth", expr("3 mm"));
        p.set("hasSecondDirection", ParamValue::Bool(true));
        p.set("secondDirectionDepth", expr("2 mm"));
    });
    let b = circle_sketch(&mut s, 4.0);
    let rb = s.region(b);
    s.add("extrude", move |p| {
        p.set("entities", q(rb));
        p.set("depth", expr("5 mm"));
        p.set("oppositeDirection", ParamValue::Bool(true));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 2);
    if tag {
        let (add, rem) = (r.context.bodies[0].id.clone(), r.context.bodies[1].id.clone());
        s.add("tag", move |p| {
            p.set("tagPurpose", en("FORM"));
            p.set("positivePart", q(Reference::Body(add)));
            p.set("negativePart", q(Reference::Body(rem)));
        });
    }
    s.ok().context
}

#[test]
fn form_adds_and_cuts_and_shows_in_flat() {
    let mut s = plate();
    s.opts.derived = Some(std::sync::Arc::new(Tabs([("form".to_string(), form_studio(true))].into_iter().collect())));
    // Two locations on the top face.
    let mut sk = noshape_sketch::Sketch::new();
    let p1 = sk.add_point(DVec2::new(mm(30.0), mm(20.0)));
    let p2 = sk.add_point(DVec2::new(mm(70.0), mm(20.0)));
    let pl = s.add("cPlane", |p| {
        p.set("entities", q(Reference::Plane(DatumPlane::Top)));
        p.set("offset", expr("2 mm"));
    });
    let sid = s.sketch(Reference::FeaturePlane(pl), sk);
    let r = s.ok();
    let v0 = volume(&r, 0);
    // The face the dialog's editing logic fills in for these locations (std formedEditLogic).
    let top = face_at(&r, 0, DVec3::new(mm(50.0), mm(20.0), mm(2.0)));
    s.add("sheetMetalFormed", move |p| {
        p.set("formPartStudio", ParamValue::Text("form".into()));
        p.set("locations", ParamValue::Query(vec![Reference::SketchEntity { sketch: sid, entity: p1.0 }, Reference::SketchEntity { sketch: sid, entity: p2.0 }]));
        p.set("targetFaces", q(top));
    });
    let r = s.ok();
    let t = mm(2.0);
    let expected = v0 + 2.0 * (PI * (mm(5.0).powi(2) - mm(4.0).powi(2)) * mm(3.0) - PI * mm(4.0).powi(2) * t);
    assert!(near(volume(&r, 0), expected, 1e-12), "form volume {} vs {expected}", volume(&r, 0));
    let bb = r.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!(near(bb.max.z, mm(5.0), 1e-6), "{bb:?}");
    // Flat: same outline as the plate, plus two form outlines with center marks.
    let fp = flat(&r, 0);
    assert!(near(fp.area(), mm(100.0) * mm(40.0), 1e-10), "flat area {}", fp.area());
    assert_eq!(fp.forms.len(), 2);
    for f in &fp.forms {
        assert!(!f.loops.is_empty());
        let l = &f.loops[0];
        let r_avg = l.iter().map(|p| (*p - f.center).length()).sum::<f64>() / l.len() as f64;
        assert!(near(r_avg, mm(5.0), 1e-5), "outline radius {r_avg}");
    }
    let g = fp.to_sheet_geometry();
    assert!(g.lines.iter().any(|l| l.style == noshape_model::drawing::sheet::LineStyle::Centerline));
    // Untagged studio: no form tools (std derive finds no Tag-marked parts).
    let mut s2 = plate();
    s2.opts.derived = Some(std::sync::Arc::new(Tabs([("form".to_string(), form_studio(false))].into_iter().collect())));
    let mut sk = noshape_sketch::Sketch::new();
    let p1 = sk.add_point(DVec2::new(mm(30.0), mm(20.0)));
    let pl = s2.add("cPlane", |p| {
        p.set("entities", q(Reference::Plane(DatumPlane::Top)));
        p.set("offset", expr("2 mm"));
    });
    let sid = s2.sketch(Reference::FeaturePlane(pl), sk);
    let top = face_at(&s2.ok(), 0, DVec3::new(mm(50.0), mm(20.0), mm(2.0)));
    s2.add("sheetMetalFormed", move |p| {
        p.set("formPartStudio", ParamValue::Text("form".into()));
        p.set("locations", ParamValue::Query(vec![Reference::SketchEntity { sketch: sid, entity: p1.0 }]));
        p.set("targetFaces", q(top));
    });
    assert_eq!(
        s2.last_status(),
        (FeatureStatus::Error, "Failed to import form tools. Check configuration parameters and use Tag feature to mark the tool bodies.".to_string())
    );
    // Missing studio / location off the part.
    let mut s3 = plate();
    s3.add("sheetMetalFormed", |p| p.set("formPartStudio", ParamValue::Text("nope".into())));
    assert_eq!(s3.last_status().0, FeatureStatus::Error);
}

/// Plate with flanges on two adjacent edges (x = 100 and y = 40), ripped at the corner.
fn two_flanges() -> Studio {
    let mut s = plate();
    let r = s.ok();
    let e1 = edge_mid(&r, DVec3::new(mm(100.0), mm(20.0), mm(2.0)));
    let e2 = edge_mid(&r, DVec3::new(mm(50.0), mm(40.0), mm(2.0)));
    s.add("sheetMetalFlange", move |p| {
        p.set("edges", ParamValue::Query(vec![e1, e2]));
        p.set("distance", expr("20 mm"));
    });
    s
}

fn edge_mid(r: &RegenResult, p: DVec3) -> Reference {
    let b = &r.context.bodies[0];
    for (i, e) in b.shape.edges().iter().enumerate() {
        let (a, c) = e.endpoints().unwrap();
        if ((a + c) * 0.5 - p).length() < 1e-7 {
            return Reference::Edge((*b.edges[i]).clone());
        }
    }
    panic!("no edge at {p:?}");
}

#[test]
fn corner_relief_cuts_folded_and_flat() {
    let mut s = two_flanges();
    let r = s.ok();
    let data = r.context.sheet_metal.get(&r.context.bodies[0].id).unwrap().clone();
    assert_eq!(data.bends.len(), 2, "two flange bends");
    let v0 = volume(&r, 0);
    let a0 = flat(&r, 0).area();
    // Vertex nearest to the corner of the two bends.
    let b = &data.bends[0];
    let corner = DVec3::new(mm(100.0), mm(40.0), mm(2.0));
    let _ = b;
    let body = &r.context.bodies[0];
    let mut best = (f64::INFINITY, 0usize);
    for (i, v) in body.shape.vertices().iter().enumerate() {
        let d = (v.vertex_point().unwrap() - corner).length();
        if d < best.0 {
            best = (d, i);
        }
    }
    let vref = Reference::Vertex((*body.vertices[best.1]).clone());
    s.add("sheetMetalCorner", move |p| {
        p.set("corner", q(vref));
        p.set("cornerStyle", en("SIZED_ROUND"));
        p.set("roundReliefDiameter", expr("12 mm"));
    });
    let r = s.ok();
    assert!(volume(&r, 0) < v0 - 1e-12, "relief removes material");
    assert_eq!(r.context.bodies[0].shape.count(noshape_kernel::ShapeType::Solid), 1);
    let fp = flat(&r, 0);
    assert!(fp.area() < a0 - 1e-8, "flat shows the relief: {} vs {a0}", fp.area());
    // A single bend end is not a corner.
    let mut s = plate();
    let r = s.ok();
    let e = edge_mid(&r, DVec3::new(mm(100.0), mm(20.0), mm(2.0)));
    s.add("sheetMetalFlange", move |p| p.set("edges", ParamValue::Query(vec![e])));
    let r = s.ok();
    let body = &r.context.bodies[0];
    let v = body.shape.vertices().iter().position(|v| (v.vertex_point().unwrap() - DVec3::new(mm(100.0), mm(40.0), mm(2.0))).length() < mm(6.0)).unwrap();
    let vref = Reference::Vertex((*body.vertices[v]).clone());
    s.add("sheetMetalCorner", move |p| p.set("corner", q(vref)));
    let (st, msg) = s.last_status();
    assert_eq!(st, FeatureStatus::Error, "{msg}");
}

#[test]
fn sheet_metal_loft_hopper_flattens() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let a = polyline(&mut sk, &[(-40.0, -30.0), (40.0, -30.0), (40.0, 30.0), (-40.0, 30.0)], true);
    let s1 = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let pl = s.add("cPlane", |p| {
        p.set("entities", q(Reference::Plane(DatumPlane::Top)));
        p.set("offset", expr("50 mm"));
    });
    let mut sk = noshape_sketch::Sketch::new();
    let b = polyline(&mut sk, &[(-20.0, -15.0), (20.0, -15.0), (20.0, 15.0), (-20.0, 15.0)], true);
    let s2 = s.sketch(Reference::FeaturePlane(pl), sk);
    s.add("sheetMetalLoft", move |p| {
        p.set("profile1", entities(s1, &a));
        p.set("profile2", entities(s2, &b));
        set_model(p, "1 mm", "2 mm");
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 1);
    assert_eq!(r.context.bodies[0].shape.count(noshape_kernel::ShapeType::Solid), 1);
    let data = r.context.sheet_metal.get(&r.context.bodies[0].id).unwrap();
    assert_eq!(data.walls.len(), 4);
    assert_eq!(data.bends.len(), 3, "three bends and one rip");
    assert_eq!(data.rips.len(), 1);
    // Facet areas of the reference surface (trapezoids).
    let slant_x = (mm(50.0).powi(2) + mm(15.0).powi(2)).sqrt();
    let slant_y = (mm(50.0).powi(2) + mm(20.0).powi(2)).sqrt();
    let facets = 2.0 * (mm(80.0) + mm(40.0)) * 0.5 * slant_x + 2.0 * (mm(60.0) + mm(30.0)) * 0.5 * slant_y;
    let fp = flat(&r, 0);
    assert_eq!(fp.bend_lines.len(), 3);
    assert!(fp.warnings.is_empty(), "{:?}", fp.warnings);
    assert!(fp.area() > facets * 0.9 && fp.area() < facets * 1.05, "flat area {} vs facets {facets}", fp.area());
}

#[test]
#[ignore = "std: 51 faces, 17449.70 mm³ (planar patches where three bends meet; live has 2 swept ones); native: unequal corner counts unsupported"]
fn sheet_metal_loft_rectangle_to_triangle() {
    // Rectangle (4 sides) to triangle (3), dialog defaults (live, behaviours.md "fs (10-23 10:30)"): one part,
    // 17452.843417228427 mm³, 49 faces (10 cylinders: 5 bends), the seam a rip.
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let a = polyline(&mut sk, &[(-40.0, -30.0), (40.0, -30.0), (40.0, 30.0), (-40.0, 30.0)], true);
    let s1 = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let pl = s.add("cPlane", |p| {
        p.set("entities", q(Reference::Plane(DatumPlane::Top)));
        p.set("offset", expr("50 mm"));
    });
    let mut sk = noshape_sketch::Sketch::new();
    let b = polyline(&mut sk, &[(-20.0, -15.0), (20.0, -15.0), (0.0, 15.0)], true);
    let s2 = s.sketch(Reference::FeaturePlane(pl), sk);
    s.add("sheetMetalLoft", move |p| {
        p.set("profile1", entities(s1, &a));
        p.set("profile2", entities(s2, &b));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 1);
    let v = volume(&r, 0) * 1e9;
    let radii = cylinder_radii(&r, 0);
    assert_eq!((r.context.bodies[0].faces.len(), radii.len()), (49, 10), "faces; volume {v}; radii {radii:?}");
    assert!(radii.iter().all(|&x| near(x, mm(2.3), 1e-9) || near(x, mm(3.9), 1e-9)), "{radii:?}");
    assert!(near(v, 17452.843417228427, 1e-3), "volume {v}");
}

/// Cylinder of radius 20 mm around Z, 60 mm long centered at z = 0, and a
/// 10 x 20 mm rectangle sketched on the tangent plane x = 20 mm.
fn cylinder_and_tool() -> (Studio, Reference) {
    let mut s = Studio::new();
    let c = circle_sketch(&mut s, 20.0);
    let rc = s.region(c);
    s.add("extrude", move |p| {
        p.set("entities", q(rc));
        p.set("depth", expr("60 mm"));
        p.set("symmetric", ParamValue::Bool(true));
    });
    let pl = s.add("cPlane", |p| {
        p.set("entities", q(Reference::Plane(DatumPlane::Right)));
        p.set("offset", expr("20 mm"));
    });
    let mut sk = noshape_sketch::Sketch::new();
    polyline(&mut sk, &[(-5.0, -10.0), (5.0, -10.0), (5.0, 10.0), (-5.0, 10.0)], true);
    let sid = s.sketch(Reference::FeaturePlane(pl), sk);
    let region = s.region(sid);
    (s, region)
}

fn cylinder_face(r: &RegenResult, bi: usize) -> Reference {
    let b = &r.context.bodies[bi];
    let i = b.shape.faces().iter().position(|f| matches!(f.face_surface().unwrap(), noshape_kernel::Surface::Cylinder { .. })).expect("cylindrical face");
    Reference::Face((*b.faces[i]).clone())
}

fn total_area(r: &RegenResult, bi: usize) -> f64 {
    r.context.bodies[bi].shape.faces().iter().map(|f| f.area().unwrap()).sum()
}

#[test]
fn wrap_surface_solid_and_split() {
    let (mut s, region) = cylinder_and_tool();
    let r = s.ok();
    let target = cylinder_face(&r, 0);
    let (reg2, t2) = (region.clone(), target.clone());
    let w = s.add("wrap", move |p| {
        p.set("resultType", en("SURFACE"));
        p.set("source", q(reg2));
        p.set("destination", q(t2));
    });
    let r = s.ok();
    let surf = r.context.bodies.iter().position(|b| b.id.feature == w).expect("wrapped surface");
    assert_eq!(r.context.bodies[surf].kind, noshape_model::display::BodyKind::Surface);
    let a = total_area(&r, surf);
    assert!(near(a, mm(10.0) * mm(20.0), 1e-10), "wrapped area {a}");
    for v in r.context.bodies[surf].shape.vertices() {
        let p = v.vertex_point().unwrap();
        assert!(near((p.x * p.x + p.y * p.y).sqrt(), mm(20.0), 1e-7), "vertex off the cylinder {p:?}");
    }
    // Rolled in place: the tangent line x = 20, y = 0 stays.
    let on = r.context.bodies[surf].shape.faces()[0].project_point(DVec3::new(mm(20.0), 0.0, 0.0)).unwrap();
    assert!(on.distance < 1e-7, "tangent line moved: {}", on.distance);
    // Stable names across regenerations.
    let names: Vec<_> = r.context.bodies[surf].faces.clone();
    let r2 = s.ok();
    let surf2 = r2.context.bodies.iter().position(|b| b.id.feature == w).unwrap();
    assert_eq!(names, r2.context.bodies[surf2].faces);

    // A cylindrical face as a tool fails: Onshape has no unwrap (live L20).
    let wf = Reference::Face((*r.context.bodies[surf].faces[0]).clone());
    let t3 = cylinder_face(&r, 0);
    s.add("wrap", move |p| {
        p.set("resultType", en("SURFACE"));
        p.set("source", q(wf));
        p.set("destination", q(t3));
    });
    assert_eq!(s.last_status(), (FeatureStatus::Error, "Select planar faces or sketch regions to wrap.".to_string()));

    // Solid: thickened outward and added to the cylinder.
    let (mut s, region) = cylinder_and_tool();
    let r = s.ok();
    let v0 = volume(&r, 0);
    let target = cylinder_face(&r, 0);
    s.add("wrap", move |p| {
        p.set("resultType", en("SOLID"));
        p.set("operationType", en("ADD"));
        p.set("defaultScope", ParamValue::Bool(true));
        p.set("source", q(region));
        p.set("destination", q(target));
        p.set("thickness", expr("2 mm"));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 1, "added to the cylinder");
    // Annular sector: angle = 10/20 rad, (R+t)^2 - R^2 over 2, times the 20 mm height.
    let added = 0.5 * (mm(10.0) / mm(20.0)) * (mm(22.0).powi(2) - mm(20.0).powi(2)) * mm(20.0);
    assert!(near(volume(&r, 0) - v0, added, added * 1e-4), "wrap solid added {} vs {added}", volume(&r, 0) - v0);

    // Split: the rectangle's edges imprint on the cylinder face.
    let (mut s, region) = cylinder_and_tool();
    let r = s.ok();
    let n0 = r.context.bodies[0].shape.count(noshape_kernel::ShapeType::Face);
    let target = cylinder_face(&r, 0);
    s.add("wrap", move |p| {
        p.set("resultType", en("IMPRINT"));
        p.set("source", q(region));
        p.set("destination", q(target));
    });
    let r = s.ok();
    assert!(r.context.bodies[0].shape.count(noshape_kernel::ShapeType::Face) > n0, "split adds faces");
    assert!(near(volume(&r, 0), PI * mm(20.0).powi(2) * mm(60.0), 1e-12));

    // Planar target for planar tools is an error.
    let (mut s, region) = cylinder_and_tool();
    s.add("wrap", move |p| {
        p.set("source", q(region));
        p.set("destination", q(Reference::Plane(DatumPlane::Top)));
    });
    assert_eq!(s.last_status().0, FeatureStatus::Error);
}

#[test]
fn isocline_curves_and_split() {
    let mut s = Studio::new();
    let c = circle_sketch(&mut s, 20.0);
    let rc = s.region(c);
    s.add("extrude", move |p| {
        p.set("entities", q(rc));
        p.set("depth", expr("30 mm"));
    });
    let r = s.ok();
    let b = &r.context.bodies[0];
    let sides: Vec<Reference> = b.shape.faces().iter().enumerate().filter(|(_, f)| matches!(f.face_surface().unwrap(), noshape_kernel::Surface::Cylinder { .. })).map(|(i, _)| Reference::Face((*b.faces[i]).clone())).collect();
    s.add("isocline", move |p| {
        p.set("faces", ParamValue::Query(sides));
        p.set("direction", q(Reference::Plane(DatumPlane::Right)));
        p.set("angle", expr("30 deg"));
    });
    let r = s.ok();
    let edges: Vec<_> = r.context.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Wire).flat_map(|b| b.shape.edges()).collect();
    assert_eq!(edges.len(), 2, "two lines at +-60 deg");
    for e in &edges {
        assert!(near(e.length().unwrap(), mm(30.0), 1e-7));
        let (a, _) = e.endpoints().unwrap();
        assert!(near(a.x.abs(), mm(10.0), 1e-6), "{a:?}");
    }
    // Split the face along the isoclines.
    let n0 = r.context.bodies[0].shape.count(noshape_kernel::ShapeType::Face);
    s.ps.features.last_mut().unwrap().params.set("split", ParamValue::Bool(true));
    let r = s.ok();
    assert!(r.context.bodies[0].shape.count(noshape_kernel::ShapeType::Face) > n0, "split faces");
    assert!(r.context.bodies.iter().all(|b| b.kind != noshape_model::display::BodyKind::Wire));
    // No isocline: error in curve mode.
    s.ps.features.last_mut().unwrap().params.set("split", ParamValue::Bool(false));
    s.ps.features.last_mut().unwrap().params.set("direction", q(Reference::Plane(DatumPlane::Top)));
    s.ps.features.last_mut().unwrap().params.set("angle", expr("30 deg"));
    assert_eq!(s.last_status().0, FeatureStatus::Error);
}

#[test]
fn constrained_surface_from_points_and_mesh() {
    // Points of a dome-like patch: sketch points on three planes.
    let mut s = Studio::new();
    let f = |x: f64, y: f64| 10.0 - 0.01 * (x * x + y * y);
    let mut ids = Vec::new();
    let mut planes = Vec::new();
    for (k, y) in [-20.0f64, 0.0, 20.0].into_iter().enumerate() {
        let pl = s.add("cPlane", move |p| {
            p.set("entities", q(Reference::Plane(DatumPlane::Front)));
            // Offsets are not negative (Onshape bounds): flip instead.
            p.set("offset", expr(&format!("{} mm", y.abs())));
            p.set("oppositeDirection", ParamValue::Bool(y > 0.0));
        });
        planes.push(pl);
        let mut sk = noshape_sketch::Sketch::new();
        let pts: Vec<_> = [-20.0f64, 0.0, 20.0].into_iter().map(|x| sk.add_point(DVec2::new(mm(x), mm(f(x, y))))).collect();
        let sid = s.sketch(Reference::FeaturePlane(pl), sk);
        ids.push((sid, pts));
        let _ = k;
    }
    let items: Vec<ParamValues> = ids
        .iter()
        .flat_map(|(sid, pts)| {
            pts.iter().map(move |e| {
                let mut v = ParamValues::default();
                v.set("vertex", q(Reference::SketchEntity { sketch: *sid, entity: e.0 }));
                v
            })
        })
        .collect();
    s.add("constrainedSurface", move |p| {
        p.set("vertices", ParamValue::Array(items));
        p.set("computeDeviation", ParamValue::Bool(true));
    });
    let r = s.ok();
    let surf = r.context.bodies.len() - 1;
    assert_eq!(r.context.bodies[surf].kind, noshape_model::display::BodyKind::Surface);
    // The surface passes through the points.
    let face = &r.context.bodies[surf].shape.faces()[0];
    let mut worst: f64 = 0.0;
    for (sid, pts) in &ids {
        let sk = r.context.sketches.iter().find(|x| x.feature == *sid).unwrap();
        for e in pts {
            let p = sk.frame.to_world(sk.sketch.point(*e).unwrap());
            worst = worst.max(face.project_point(p).unwrap().distance);
        }
    }
    assert!(worst < 1e-5, "deviation {worst}");
    assert!(r.display.features.last().unwrap().message.as_deref().unwrap_or("").contains("deviation"));

    // Mesh mode: fit the top face vertices of a box-shaped mesh-like part.
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    polyline(&mut sk, &[(0.0, 0.0), (30.0, 0.0), (30.0, 20.0), (0.0, 20.0)], true);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let region = s.region(sid);
    s.add("extrude", move |p| {
        p.set("entities", q(region));
        p.set("depth", expr("5 mm"));
    });
    let r = s.ok();
    let top = face_at(&r, 0, DVec3::new(mm(15.0), mm(10.0), mm(5.0)));
    s.add("constrainedSurface", move |p| {
        p.set("csType", en("MESH"));
        p.set("meshes", q(top));
    });
    let r = s.ok();
    let surf = r.context.bodies.len() - 1;
    let a = total_area(&r, surf);
    assert!(near(a, mm(30.0) * mm(20.0), 1e-9), "mesh fit area {a}");
    let bb = r.context.bodies[surf].shape.bounding_box(true).unwrap();
    assert!(near(bb.max.z, mm(5.0), 1e-5) && near(bb.min.z, mm(5.0), 1e-5), "planar fit {bb:?}");
}

#[test]
fn derived_placed_at_mate_connectors() {
    // Source studio: a 10 mm cube-ish block with a mate connector on its top face center.
    let mut src = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    polyline(&mut sk, &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], true);
    let sid = src.sketch(Reference::Plane(DatumPlane::Top), sk);
    let region = src.region(sid);
    src.add("extrude", move |p| {
        p.set("entities", q(region));
        p.set("depth", expr("10 mm"));
    });
    let r = src.ok();
    let top = face_at(&r, 0, DVec3::new(mm(5.0), mm(5.0), mm(10.0)));
    src.add("mateConnector", move |p| {
        p.set("originQuery", q(top));
        p.set("entityInferenceType", en("CENTROID"));
    });
    let src_ctx = src.ok().context;
    assert_eq!(src_ctx.connectors.len(), 1);
    let mc_name = src_ctx.connectors[0].name.clone();

    // Target: two points -> two instances at the base origin; then placed by
    // the source mate connector.
    let mut s = Studio::new();
    s.opts.derived = Some(std::sync::Arc::new(Tabs([("src".to_string(), src_ctx)].into_iter().collect())));
    let mut sk = noshape_sketch::Sketch::new();
    let a = sk.add_point(DVec2::new(mm(50.0), 0.0));
    let b = sk.add_point(DVec2::new(mm(-50.0), mm(20.0)));
    let pts = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let d = s.add("importDerived", move |p| {
        p.set("partStudio", ParamValue::Text("src".into()));
        p.set("location", ParamValue::Query(vec![Reference::SketchEntity { sketch: pts, entity: a.0 }, Reference::SketchEntity { sketch: pts, entity: b.0 }]));
    });
    let r = s.ok();
    let derived: Vec<_> = r.context.bodies.iter().filter(|x| x.id.feature == d).collect();
    assert_eq!(derived.len(), 2, "one instance per location");
    let bb0 = derived[0].shape.bounding_box(true).unwrap();
    let bb1 = derived[1].shape.bounding_box(true).unwrap();
    assert!(near(bb0.min.x, mm(50.0), 1e-6) && near(bb0.min.z, 0.0, 1e-6), "{bb0:?}");
    assert!(near(bb1.min.x, mm(-50.0), 1e-6) && near(bb1.min.y, mm(20.0), 1e-6), "{bb1:?}");
    assert_ne!(derived[0].faces[0], derived[1].faces[0], "instances have distinct names");
    // Live (ui 10-22 b): both parts keep the source name, each location gets
    // the source's sketch, and 2+ locations is an info (the bubble).
    assert_eq!(derived[0].name, derived[1].name);
    assert_eq!(r.context.derived_sketches.iter().filter(|(f, _)| *f == d).count(), 2);
    let (st, msg) = s.last_status();
    assert_eq!(st, FeatureStatus::Info);
    assert_eq!(msg, noshape_model::regen::features::importing::DERIVED_NO_INSTANCING);

    // Base mate connector (top face center) on the point: the block hangs below it.
    s.ps.features.last_mut().unwrap().params.set("placement", en("AT_MATE_CONNECTOR"));
    s.ps.features.last_mut().unwrap().params.set("mateConnectorId", ParamValue::Text(mc_name));
    let r = s.ok();
    let derived: Vec<_> = r.context.bodies.iter().filter(|x| x.id.feature == d).collect();
    let bb0 = derived[0].shape.bounding_box(true).unwrap();
    assert!(near(bb0.max.z, 0.0, 1e-6) && near(bb0.min.z, mm(-10.0), 1e-6), "{bb0:?}");
    assert!(near((bb0.min.x + bb0.max.x) * 0.5, mm(50.0), 1e-6), "{bb0:?}");
    // Unknown connector name: clear error.
    s.ps.features.last_mut().unwrap().params.set("mateConnectorId", ParamValue::Text("nope".into()));
    let (st, msg) = s.last_status();
    assert_eq!(st, FeatureStatus::Error);
    assert!(msg.contains("nope"), "{msg}");
}

/// Feature clipboard payloads for manual UI checks (paste with Ctrl+V):
/// `cargo test -p noshape-model --test features_c write_clips -- --ignored`.
#[test]
#[ignore]
fn write_clips() {
    let dir = std::env::temp_dir();
    let save = |s: &mut Studio, name: &str| {
        s.ok();
        let v = serde_json::json!({ "noshape_features": s.ps.features });
        std::fs::write(dir.join(name), v.to_string()).unwrap();
    };
    let mut s = jogged(|_| {});
    save(&mut s, "noshape-clip-jog.json");
    let mut s = two_flanges();
    let r = s.ok();
    let body = &r.context.bodies[0];
    let corner = DVec3::new(mm(100.0), mm(40.0), mm(2.0));
    let i = (0..body.shape.vertices().len()).min_by(|&a, &b| {
        let d = |k: usize| (body.shape.vertices()[k].vertex_point().unwrap() - corner).length();
        d(a).total_cmp(&d(b))
    });
    let vref = Reference::Vertex((*body.vertices[i.unwrap()]).clone());
    s.add("sheetMetalCorner", move |p| {
        p.set("corner", q(vref));
        p.set("cornerStyle", en("SIZED_ROUND"));
        p.set("roundReliefDiameter", expr("12 mm"));
    });
    save(&mut s, "noshape-clip-corner.json");
    let (mut s, region) = cylinder_and_tool();
    let r = s.ok();
    let target = cylinder_face(&r, 0);
    s.add("wrap", move |p| {
        p.set("resultType", en("SOLID"));
        p.set("operationType", en("ADD"));
        p.set("defaultScope", ParamValue::Bool(true));
        p.set("source", q(region));
        p.set("destination", q(target));
        p.set("thickness", expr("2 mm"));
    });
    save(&mut s, "noshape-clip-wrap.json");
}

fn vertex_near(r: &RegenResult, p: DVec3) -> Reference {
    let b = &r.context.bodies[0];
    let i = (0..b.shape.vertices().len())
        .min_by(|&x, &y| {
            let d = |k: usize| (b.shape.vertices()[k].vertex_point().unwrap() - p).length();
            d(x).total_cmp(&d(y))
        })
        .unwrap();
    Reference::Vertex((*b.vertices[i]).clone())
}

#[test]
#[ignore = "live: a no-op; which std step makes it one is asked (REQUESTS fs 10-23 17:30). Std splits the face, native cuts a slot"]
fn rip_diagonal_on_flat_plate_is_a_no_op() {
    // Live (behaviours.md "fs (10-23 13:30)"): a corner-to-corner Rip across a single flat wall regenerates OK
    // but changes nothing: 1 part, 8000 mm³, 6 planar faces, no joint.
    let mut s = plate();
    let r = s.ok();
    let (a, b) = (vertex_near(&r, DVec3::new(0.0, 0.0, mm(2.0))), vertex_near(&r, DVec3::new(mm(100.0), mm(40.0), mm(2.0))));
    s.add("sheetMetalRip", move |p| p.set("vertices", ParamValue::Query(vec![a, b])));
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 1, "the diagonal rip leaves one part");
    let full = mm(100.0) * mm(40.0) * mm(2.0);
    assert!(near(volume(&r, 0), full, 1e-15), "volume unchanged: {}", volume(&r, 0));
    assert_eq!(r.context.bodies[0].faces.len(), 6, "faces unchanged");
}

#[test]
fn recognize_rebuilds_sheet_metal_model() {
    // An L bracket made as sheet metal, finished, then recognized again.
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let ids = polyline(&mut sk, &[(0.0, 0.0), (50.0, 0.0), (50.0, 30.0)], false);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    s.add("sheetMetalStart", move |p| {
        p.set("process", en("EXTRUDE"));
        p.set("sketchCurves", entities(sid, &ids));
        p.set("depth", expr("20 mm"));
        set_model(p, "2 mm", "3 mm");
    });
    // Finish needs its parts (live: "Select one or more sheet metal parts.").
    let part = Reference::Body(s.ok().context.bodies[0].id.clone());
    s.add("sheetMetalEnd", move |p| p.set("sheetMetalParts", ParamValue::Query(vec![part])));
    let r = s.ok();
    let expected = flat(&r, 0).size();
    let before = volume(&r, 0);
    let id = r.context.bodies[0].id.clone();
    s.add("sheetMetalRecognize", move |p| {
        p.set("bodies", q(Reference::Body(id)));
        p.set("kFactor", expr("0.45"));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 1);
    // The same L: one bend, inner radius 3 and outer 5 (thickness 2), same volume.
    let mut radii = cylinder_radii(&r, 0);
    radii.sort_by(f64::total_cmp);
    assert!(radii.len() == 2 && near(radii[0], mm(3.0), 1e-9) && near(radii[1], mm(5.0), 1e-9), "{radii:?}");
    assert!(near(volume(&r, 0), before, 1e-12), "volume {} vs {before}", volume(&r, 0));
    let got = flat(&r, 0).size();
    assert!(near(got.x.max(got.y), expected.x.max(expected.y), 1e-7), "recognized flat {got:?} vs {expected:?}");
}

#[test]
fn unfold_then_refold_restores_the_part() {
    let mut s = plate();
    let r = s.ok();
    let e = edge_mid(&r, DVec3::new(mm(100.0), mm(20.0), mm(2.0)));
    s.add("sheetMetalFlange", move |p| {
        p.set("edges", ParamValue::Query(vec![e]));
        p.set("distance", expr("20 mm"));
    });
    let r = s.ok();
    let v0 = volume(&r, 0);
    let bb0 = r.context.bodies[0].shape.bounding_box(true).unwrap();
    let f0 = flat(&r, 0).size();
    let id = r.context.bodies[0].id.clone();
    s.add("sheetMetalUnfold", move |p| p.set("partToUnfold", q(Reference::Body(id.clone()))));
    let r = s.ok();
    let bb = r.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!(bb.max.z - bb.min.z < mm(2.0) + 1e-6, "unfolded part is flat: {bb:?}");
    let f1 = flat(&r, 0).size();
    assert!(near(f1.x, f0.x, 1e-7) && near(f1.y, f0.y, 1e-7), "flat unchanged {f1:?} vs {f0:?}");
    assert!(near(bb.max.x - bb.min.x, f0.x.max(f0.y), 1e-6), "in-model flat length {bb:?}");
    let id = r.context.bodies[0].id.clone();
    s.add("sheetMetalRefold", move |p| p.set("toRefold", q(Reference::Body(id))));
    let r = s.ok();
    let bb2 = r.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!((bb2.max - bb0.max).length() < 1e-6 && (bb2.min - bb0.min).length() < 1e-6, "refolded {bb2:?} vs {bb0:?}");
    assert!(near(volume(&r, 0), v0, 1e-12), "refold volume {} vs {v0}", volume(&r, 0));
}

/// 100 x 50 x 25 mm block on the Top plane.
fn block() -> Studio {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    polyline(&mut sk, &[(0.0, 0.0), (100.0, 0.0), (100.0, 50.0), (0.0, 50.0)], true);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let region = s.region(sid);
    s.add("extrude", move |p| {
        p.set("entities", q(region));
        p.set("depth", expr("25 mm"));
    });
    s
}

fn top_plane_at(s: &mut Studio, z_mm: f64) -> FeatureId {
    s.add("cPlane", move |p| {
        p.set("entities", q(Reference::Plane(DatumPlane::Top)));
        p.set("offset", expr(&format!("{z_mm} mm")));
    })
}

#[test]
fn replace_face_extends_and_trims() {
    for (z, expect) in [(35.0, 35.0), (15.0, 15.0)] {
        let mut s = block();
        let pl = top_plane_at(&mut s, z);
        let r = s.ok();
        let top = face_at(&r, 0, DVec3::new(mm(50.0), mm(25.0), mm(25.0)));
        s.add("replaceFace", move |p| {
            p.set("replaceFaces", q(top));
            p.set("templateFace", q(Reference::FeaturePlane(pl)));
        });
        let r = s.ok();
        let v = mm(100.0) * mm(50.0) * mm(expect);
        assert!(near(volume(&r, 0), v, 1e-12), "replace to z = {z}: {} vs {v}", volume(&r, 0));
        assert_eq!(r.context.bodies[0].shape.count(noshape_kernel::ShapeType::Face), 6, "neighbours merged");
    }
    // Offset from the template.
    let mut s = block();
    let pl = top_plane_at(&mut s, 30.0);
    let r = s.ok();
    let top = face_at(&r, 0, DVec3::new(mm(50.0), mm(25.0), mm(25.0)));
    s.add("replaceFace", move |p| {
        p.set("replaceFaces", q(top));
        p.set("templateFace", q(Reference::FeaturePlane(pl)));
        p.set("offset", expr("5 mm"));
    });
    let r = s.ok();
    assert!(near(volume(&r, 0), mm(100.0) * mm(50.0) * mm(35.0), 1e-12));
}

#[test]
fn move_face_rotate_up_to_translate_and_create() {
    // Rotate the top face about its back edge by 10 degrees (tilts down to the front).
    let mut s = block();
    let mut sk = noshape_sketch::Sketch::new();
    let ax = polyline(&mut sk, &[(0.0, 50.0), (100.0, 50.0)], false);
    let pl = top_plane_at(&mut s, 25.0);
    let sid = s.sketch(Reference::FeaturePlane(pl), sk);
    let r = s.ok();
    let top = face_at(&r, 0, DVec3::new(mm(50.0), mm(25.0), mm(25.0)));
    s.add("moveFace", move |p| {
        p.set("moveFaces", q(top));
        p.set("moveFaceType", en("ROTATE"));
        p.set("axis", entities(sid, &ax));
        p.set("angle", expr("10 deg"));
    });
    let r = s.ok();
    let drop = mm(50.0) * (10f64).to_radians().tan();
    let v = mm(100.0) * (mm(50.0) * mm(25.0) - 0.5 * mm(50.0) * drop);
    let v2 = mm(100.0) * (mm(50.0) * mm(25.0) + 0.5 * mm(50.0) * drop);
    let got = volume(&r, 0);
    assert!(near(got, v, 1e-11) || near(got, v2, 1e-11), "rotated volume {got} vs {v} / {v2}");

    // Offset up to a plane.
    let mut s = block();
    let pl = top_plane_at(&mut s, 40.0);
    let r = s.ok();
    let top = face_at(&r, 0, DVec3::new(mm(50.0), mm(25.0), mm(25.0)));
    s.add("moveFace", move |p| {
        p.set("moveFaces", q(top));
        p.set("limitType", en("UP_TO_ENTITY"));
        p.set("limitQuery", q(Reference::FeaturePlane(pl)));
    });
    let r = s.ok();
    assert!(near(volume(&r, 0), mm(100.0) * mm(50.0) * mm(40.0), 1e-12), "up to {}", volume(&r, 0));

    // Translate a hole sideways (non-planar face).
    let mut s = block();
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(DVec2::new(mm(30.0), mm(25.0)), mm(5.0));
    let pl = top_plane_at(&mut s, 25.0);
    let hs = s.sketch(Reference::FeaturePlane(pl), sk);
    let hr = s.region(hs);
    s.add("extrude", move |p| {
        p.set("entities", q(hr));
        p.set("operationType", en("REMOVE"));
        p.set("defaultScope", ParamValue::Bool(true));
        p.set("endBound", en("THROUGH_ALL"));
        p.set("oppositeDirection", ParamValue::Bool(true));
    });
    let r = s.ok();
    let v0 = volume(&r, 0);
    let b = &r.context.bodies[0];
    let holes: Vec<Reference> = b.shape.faces().iter().enumerate().filter(|(_, f)| matches!(f.face_surface().unwrap(), noshape_kernel::Surface::Cylinder { .. })).map(|(i, _)| Reference::Face((*b.faces[i]).clone())).collect();
    assert!(!holes.is_empty());
    s.add("moveFace", move |p| {
        p.set("moveFaces", ParamValue::Query(holes));
        p.set("moveFaceType", en("TRANSLATE"));
        p.set("direction", q(Reference::Plane(DatumPlane::Right)));
        p.set("translationDistance", expr("20 mm"));
    });
    let r = s.ok();
    assert!(near(volume(&r, 0), v0, 1e-12), "hole moved, same volume");
    let cyl = r.context.bodies[0].shape.faces().into_iter().find(|f| matches!(f.face_surface().unwrap(), noshape_kernel::Surface::Cylinder { .. })).unwrap();
    let c = cyl.bounding_box(true).unwrap().center();
    assert!(near(c.x, mm(50.0), 1e-6) || near(c.x, mm(10.0), 1e-6), "hole center {c:?}");

    // Create: offset copy of the top face as a new surface.
    let mut s = block();
    let r = s.ok();
    let top = face_at(&r, 0, DVec3::new(mm(50.0), mm(25.0), mm(25.0)));
    s.add("moveFace", move |p| {
        p.set("outputType", en("CREATE"));
        p.set("surfacesAndFaces", q(top));
        p.set("offsetDistance", expr("5 mm"));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 2);
    let surf = &r.context.bodies[1];
    assert_eq!(surf.kind, noshape_model::display::BodyKind::Surface);
    let bb = surf.shape.bounding_box(true).unwrap();
    assert!(near(bb.max.z, mm(30.0), 1e-6), "{bb:?}");
}

#[test]
fn full_round_fillet_parting_draft_swept_blend() {
    // Full round on a 100 x 10 x 25 rib: the top becomes a half cylinder of R5.
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    polyline(&mut sk, &[(0.0, 0.0), (100.0, 0.0), (100.0, 10.0), (0.0, 10.0)], true);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let region = s.region(sid);
    s.add("extrude", move |p| {
        p.set("entities", q(region));
        p.set("depth", expr("25 mm"));
    });
    let r = s.ok();
    let (s1, s2, top) = (
        face_at(&r, 0, DVec3::new(mm(50.0), 0.0, mm(12.5))),
        face_at(&r, 0, DVec3::new(mm(50.0), mm(10.0), mm(12.5))),
        face_at(&r, 0, DVec3::new(mm(50.0), mm(5.0), mm(25.0))),
    );
    s.add("fillet", move |p| {
        p.set("filletType", en("FULL_ROUND"));
        p.set("side1Face", q(s1));
        p.set("side2Face", q(s2));
        p.set("centerFaces", q(top));
    });
    let r = s.ok();
    let v = mm(100.0) * (mm(10.0) * mm(20.0) + PI * mm(5.0).powi(2) * 0.5);
    assert!(near(volume(&r, 0), v, 1e-12), "full round volume {} vs {v}", volume(&r, 0));
    assert!(r.context.bodies[0].shape.faces().iter().any(|f| matches!(f.face_surface().unwrap(), noshape_kernel::Surface::Cylinder { radius, .. } if near(radius, mm(5.0), 1e-9))));

    // Parting line draft: the bottom edges of the block, sides drafted 5 deg (one sided).
    let mut s = block();
    let r = s.ok();
    let b = &r.context.bodies[0];
    let bottom: Vec<Reference> = b
        .shape
        .edges()
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            let (a, c) = e.endpoints().unwrap();
            a.z.abs() < 1e-9 && c.z.abs() < 1e-9
        })
        .map(|(i, _)| Reference::Edge((*b.edges[i]).clone()))
        .collect();
    assert_eq!(bottom.len(), 4);
    s.add("draft", move |p| {
        p.set("draftFeatureType", en("PARTING_LINE"));
        p.set("pullDirectionEntity", q(Reference::Plane(DatumPlane::Top)));
        p.set("partingEdges", ParamValue::Query(bottom));
        p.set("angle", expr("5 deg"));
    });
    let r = s.ok();
    let (h, tn) = (mm(25.0), 5f64.to_radians().tan());
    let a1 = mm(100.0) * mm(50.0);
    let a2 = (mm(100.0) - 2.0 * h * tn) * (mm(50.0) - 2.0 * h * tn);
    let am = (mm(100.0) - h * tn) * (mm(50.0) - h * tn);
    let v = h / 6.0 * (a1 + a2 + 4.0 * am);
    assert!(near(volume(&r, 0), v, 1e-11), "parting draft volume {} vs {v}", volume(&r, 0));

    // Swept profile face blend along a spine parallel to the edge = rolling ball.
    let mut s = block();
    let r = s.ok();
    let (side, top) = (face_at(&r, 0, DVec3::new(mm(50.0), 0.0, mm(12.5))), face_at(&r, 0, DVec3::new(mm(50.0), mm(25.0), mm(25.0))));
    let spine = edge_mid(&r, DVec3::new(mm(50.0), 0.0, mm(25.0)));
    s.add("faceBlend", move |p| {
        p.set("side1", q(side));
        p.set("side2", q(top));
        p.set("crossSection", en("SWEPT_PROFILE"));
        p.set("spine", q(spine));
        p.set("radius", expr("5 mm"));
    });
    let r = s.ok();
    let v = mm(100.0) * mm(50.0) * mm(25.0) - mm(100.0) * (mm(5.0).powi(2) - PI * mm(5.0).powi(2) / 4.0);
    assert!(near(volume(&r, 0), v, 1e-12), "swept blend volume {}", volume(&r, 0));
}

#[test]
fn flat_spiral_planes_and_ruled_overrides() {
    // Height 0 (a flat spiral) is out of Onshape's range (live L32).
    let mut s = Studio::new();
    s.add("helix", |p| {
        p.set("axisType", en("AXIS"));
        p.set("axis", q(Reference::Plane(DatumPlane::Top)));
        p.set("startRadius", expr("10 mm"));
        p.set("height", expr("0 mm"));
        p.set("endRadToggle", ParamValue::Bool(true));
        p.set("endRadius", expr("30 mm"));
        p.set("revolutions", expr("3"));
    });
    assert_eq!(s.last_status(), (FeatureStatus::Error, "A parameter value is not valid.".to_string()));

    // Mid plane between two intersecting planes (Top and Right): the bisector.
    let mut s = Studio::new();
    s.add("cPlane", |p| {
        p.set("cplaneType", en("MID_PLANE"));
        p.set("entities", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top), Reference::Plane(DatumPlane::Right)]));
    });
    let r = s.ok();
    let n = r.context.planes.last().unwrap().frame.normal;
    assert!(near(n.z.abs(), 0.5f64.sqrt(), 1e-9) && near(n.x.abs(), 0.5f64.sqrt(), 1e-9), "bisector normal {n:?}");

    // Tangent plane to a cylinder face, parallel to the Right plane.
    let (mut s, _) = cylinder_and_tool();
    let r = s.ok();
    let cyl = cylinder_face(&r, 0);
    s.add("cPlane", move |p| {
        p.set("cplaneType", en("TANGENT_PLANE"));
        p.set("entities", ParamValue::Query(vec![cyl, Reference::Plane(DatumPlane::Right)]));
    });
    let r = s.ok();
    let f = r.context.planes.last().unwrap().frame;
    assert!(near((f.origin.x.powi(2) + f.origin.y.powi(2)).sqrt(), mm(20.0), 1e-6), "on the cylinder {f:?}");
    assert!(near(f.origin.x, mm(20.0), 1e-9) && near(f.normal.x, 1.0, 1e-9), "tangent on the +X side {f:?}");

    // Ruled surface along Z with one end overridden to X.
    let mut s = block();
    let r = s.ok();
    let edge = edge_mid(&r, DVec3::new(mm(50.0), 0.0, mm(25.0)));
    let v = vertex_near(&r, DVec3::new(mm(100.0), 0.0, mm(25.0)));
    let mut item = ParamValues::default();
    item.set("vertex", q(v));
    item.set("overrideType", en("ALIGNED_WITH_VECTOR"));
    item.set("axisOverride", q(Reference::Plane(DatumPlane::Right)));
    item.set("distanceOverride", expr("10 mm"));
    s.add("ruledSurface", move |p| {
        p.set("edges", q(edge));
        p.set("ruledType", en("ALIGNED_WITH_VECTOR"));
        p.set("axis", q(Reference::Plane(DatumPlane::Top)));
        p.set("distance", expr("10 mm"));
        p.set("vertexOverrides", ParamValue::Array(vec![item]));
    });
    let r = s.ok();
    let surf = &r.context.bodies[1];
    let has = |p: DVec3| surf.shape.vertices().iter().any(|x| (x.vertex_point().unwrap() - p).length() < 1e-6);
    assert!(has(DVec3::new(mm(110.0), 0.0, mm(25.0))), "override end rules along X");
    assert!(has(DVec3::new(0.0, 0.0, mm(35.0))), "other end rules along Z");
}

/// qa5 SK1: a sketch whose dimensions can't all be met is in error (live:
/// its title and feature row red), keeping its last solved shape.
#[test]
fn an_unsolvable_sketch_is_in_error() {
    use noshape_sketch::ConstraintKind;
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let a = sk.add_point(DVec2::new(0.0, 0.0));
    let b = sk.add_point(DVec2::new(0.03, 0.0));
    let line = sk.add_line_between(a, b).unwrap();
    sk.add_constraint(ConstraintKind::Horizontal { a: line, b: None }).unwrap();
    sk.add_constraint(ConstraintKind::HorizontalDistance { a, b, value: 0.03 }).unwrap();
    let len = sk.add_reference(ConstraintKind::Length { curve: line, value: 0.03 }).unwrap();
    sk.set_driving(len, true).unwrap();
    // The editor keeps a value the sketch can't meet (live: label reads it).
    sk.set_dimension_value(len, 0.02).unwrap();
    s.sketch(Reference::Plane(noshape_model::reference::DatumPlane::Top), sk);
    let (status, _) = s.last_status();
    assert_eq!(status, FeatureStatus::Error);
}


/// Live/moveFace.fs: the faces are checked before the axis, a 0 offset
/// leaves the part as it is, and Delete face of every face has Onshape's text.
#[test]
fn move_and_delete_face_onshape_errors() {
    let mut s = block();
    s.add("moveFace", |p| p.set("moveFaceType", en("ROTATE")));
    assert_eq!(s.last_status(), (FeatureStatus::Error, "Select faces to move.".to_string()));

    let mut s = block();
    let r = s.ok();
    let v0 = volume(&r, 0);
    let top = face_at(&r, 0, DVec3::new(mm(50.0), mm(25.0), mm(25.0)));
    s.add("moveFace", move |p| {
        p.set("moveFaces", q(top));
        p.set("offsetDistance", expr("0 mm"));
    });
    assert!(near(volume(&s.ok(), 0), v0, 1e-12));

    let mut s = block();
    let r = s.ok();
    let pts = [(50.0, 25.0, 25.0), (50.0, 25.0, 0.0), (50.0, 0.0, 12.5), (50.0, 50.0, 12.5), (0.0, 25.0, 12.5), (100.0, 25.0, 12.5)];
    let faces: Vec<Reference> = pts.iter().map(|&(x, y, z)| face_at(&r, 0, DVec3::new(mm(x), mm(y), mm(z)))).collect();
    s.add("deleteFace", move |p| p.set("deleteFaces", ParamValue::Query(faces)));
    // Live: every face of the part deleted removes it, with no message.
    let r = s.ok();
    assert!(r.context.bodies.is_empty());
}

/// Live L19 (thicken): both thicknesses 0 read "Invalid input selections.".
/// (L17, two perpendicular faces of a block giving one L-shaped part of
/// 2560 mm³ with a square corner, needs the kernel's intersection join.)
#[test]
fn thicken_as_live() {
    let mut s2 = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    polyline(&mut sk, &[(0.0, 0.0), (40.0, 0.0), (40.0, 20.0), (0.0, 20.0)], true);
    let sid = s2.sketch(Reference::Plane(DatumPlane::Top), sk);
    let region = s2.region(sid);
    s2.add("thicken", move |p| {
        p.set("entities", q(region));
        p.set("thickness1", expr("0 mm"));
        p.set("thickness2", expr("0 mm"));
    });
    assert_eq!(s2.last_status(), (FeatureStatus::Error, "Invalid input selections.".to_string()));
}

/// Live L7: Flip alignment against a plane facing the same way as the
/// replaced face fails ("Could not replace faces as requested.").
#[test]
fn replace_face_flip_alignment_as_live() {
    let mut s = block();
    let pl = top_plane_at(&mut s, 30.0);
    let r = s.ok();
    let top = face_at(&r, 0, DVec3::new(mm(50.0), mm(25.0), mm(25.0)));
    s.add("replaceFace", move |p| {
        p.set("replaceFaces", q(top));
        p.set("templateFace", q(Reference::FeaturePlane(pl)));
        p.set("oppositeSense", ParamValue::Bool(true));
    });
    assert_eq!(s.last_status(), (FeatureStatus::Error, "Could not replace faces as requested.".to_string()));
}

/// Live (qa5 workflow 27, BEND1 / JOG1): a 60 × 40 × 2 plate, radius 2, and
/// a line (30,0)→(30,40) on its top through the middle. The side along line
/// direction × face normal (x > 30) moves; Jog's default offset is 5 mm.
#[test]
fn bend_and_jog_move_the_side_live_moves() {
    let build = |ty: &'static str| {
        let mut s = Studio::new();
        let mut sk = noshape_sketch::Sketch::new();
        polyline(&mut sk, &[(0.0, 0.0), (60.0, 0.0), (60.0, 40.0), (0.0, 40.0)], true);
        let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
        let region = s.region(sid);
        s.add("sheetMetalStart", move |p| {
            p.set("process", en("THICKEN"));
            p.set("regions", q(region));
            set_model(p, "2 mm", "2 mm");
        });
        let mut sk = noshape_sketch::Sketch::new();
        let line = polyline(&mut sk, &[(30.0, 0.0), (30.0, 40.0)], false);
        let lid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
        let r = s.ok();
        let top = face_at(&r, 0, DVec3::new(mm(30.0), mm(20.0), mm(2.0)));
        s.add(ty, move |p| {
            p.set("bendReference", entities(lid, &line));
            p.set("face", q(top));
        });
        s.ok()
    };
    let r = build("sheetMetalBend");
    let bb = r.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!(near(volume(&r, 0), 4812.566370614357e-9, 1e-15), "bend volume {}", volume(&r, 0));
    assert_eq!(r.context.bodies[0].faces.len(), 14);
    // x < 30 stays flat; the wall rises at x 29.7223453..31.7223453.
    // Within the box's tolerance gap (std-built bodies carry 1e-7 m).
    assert!(near(bb.min.x, 0.0, 2e-7) && near(bb.max.x, mm(31.7223453), 2e-7) && near(bb.max.z, mm(31.7223453), 2e-7), "{bb:?}");
    let r = build("sheetMetalJog");
    let bb = r.context.bodies[0].shape.bounding_box(true).unwrap();
    assert!(near(volume(&r, 0), 4825.132741228719e-9, 1e-15), "jog volume {}", volume(&r, 0));
    assert_eq!(r.context.bodies[0].faces.len(), 22);
    // Inside anchor: raised plate z 7..9; preserve material pulls its end in to 55.889.
    assert!(near(bb.min.x, 0.0, 2e-7) && near(bb.max.z, mm(9.0), 2e-7) && near(bb.max.x, mm(55.889), 1e-6), "{bb:?}");
}

/// patternUtils.fs / curvePattern.fs: what is patterned is checked first, and
/// the path / distance errors read like Onshape's.
#[test]
fn pattern_errors_as_onshape() {
    let mut s = block();
    s.add("linearPattern", |p| p.set("patternType", en("FEATURE")));
    assert_eq!(s.last_status(), (FeatureStatus::Error, "Select features to pattern.".to_string()));
    let mut s = block();
    s.add("mirror", |p| p.set("patternType", en("FEATURE")));
    assert_eq!(s.last_status(), (FeatureStatus::Error, "Select features to mirror.".to_string()));
    let mut s = block();
    let r = s.ok();
    let top = face_at(&r, 0, DVec3::new(mm(50.0), mm(25.0), mm(25.0)));
    s.add("curvePattern", move |p| {
        p.set("patternType", en("FACE"));
        p.set("faces", q(top));
    });
    assert_eq!(s.last_status(), (FeatureStatus::Error, "Select a path to pattern along.".to_string()));
}

/// Live "lp wall over" (behaviours "URGENT (10-22): Feature pattern…"): a
/// 0.5 mm wall overhanging the block's end by 1 mm, Feature pattern 2 mm × 2.
/// Reapply features off fails with Onshape's text and keeps only the seed;
/// on, the second wall adds its 21 mm³ of overhang.
#[test]
fn feature_pattern_of_overhanging_wall_as_live() {
    let build = |reapply: bool| {
        let mut s = Studio::new();
        let mut sk = noshape_sketch::Sketch::new();
        polyline(&mut sk, &[(0.0, 0.0), (100.0, 0.0), (100.0, 20.0), (0.0, 20.0)], true);
        let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
        let region = s.region(sid);
        s.add("extrude", move |p| {
            p.set("entities", q(region));
            p.set("depth", expr("10 mm"));
        });
        let mut sk = noshape_sketch::Sketch::new();
        polyline(&mut sk, &[(-1.0, 0.0), (21.0, 0.0), (21.0, 11.0), (-1.0, 11.0)], true);
        let sid = s.sketch(Reference::Plane(DatumPlane::Right), sk);
        let region = s.region(sid);
        let wall = s.add("extrude", move |p| {
            p.set("entities", q(region));
            p.set("depth", expr("0.5 mm"));
            p.set("operationType", en("ADD"));
            p.set("defaultScope", ParamValue::Bool(true));
        });
        s.add("linearPattern", move |p| {
            p.set("patternType", en("FEATURE"));
            p.set("instanceFunction", q(Reference::Feature(wall)));
            p.set("directionOne", q(Reference::Plane(DatumPlane::Right)));
            p.set("distance", expr("2 mm"));
            p.set("instanceCount", expr("2"));
            p.set("fullFeaturePattern", ParamValue::Bool(reapply));
        });
        s
    };
    let mut off = build(false);
    let r = off.regen();
    let st = r.display.features.last().unwrap();
    assert_eq!((st.status, st.message.as_deref()), (FeatureStatus::Error, Some("Could not create all instances as entered. Try selecting \"Reapply features\" option.")));
    assert!(near(volume(&r, 0), mm(100.0) * mm(20.0) * mm(10.0) + mm(0.5) * 21e-6 * 2.0, 1e-9), "{}", volume(&r, 0));
    let mut on = build(true);
    let r = on.ok();
    assert!(near(volume(&r, 0) * 1e9, 20042.0, 1e-6), "{}", volume(&r, 0) * 1e9);
}

/// Live "Contrast: a failed EXTRUDE" (10-22): Up to face on a face the
/// profile can't reach reads Onshape's text, not the kernel's.
#[test]
fn failed_extrude_reads_onshape_text() {
    let mut s = block();
    let r = s.ok();
    let side = face_at(&r, 0, DVec3::new(0.0, mm(25.0), mm(12.5)));
    let mut sk = noshape_sketch::Sketch::new();
    polyline(&mut sk, &[(120.0, 10.0), (130.0, 10.0), (130.0, 20.0), (120.0, 20.0)], true);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let region = s.region(sid);
    s.add("extrude", move |p| {
        p.set("entities", q(region));
        p.set("endBound", en("UP_TO_SURFACE"));
        p.set("endBoundEntityFace", q(side));
    });
    assert_eq!(s.last_status(), (FeatureStatus::Error, "Failed to extrude selections, check input.".to_string()));
}

/// sweep.fs: a thin sweep checks its wall before the path.
#[test]
fn thin_sweep_checks_the_wall_first() {
    let mut s = block();
    s.add("sweep", |p| p.set("bodyType", en("THIN")));
    assert_eq!(s.last_status(), (FeatureStatus::Error, "Select faces or sketch regions to sweep.".to_string()));
    let mut s = block();
    s.add("sweep", |_| {});
    assert_eq!(s.last_status(), (FeatureStatus::Error, "Select entities for sweep path.".to_string()));
}

fn edge_near(r: &RegenResult, bi: usize, p: DVec3) -> Reference {
    let b = &r.context.bodies[bi];
    let v = noshape_kernel::make_vertex(p).unwrap();
    for (i, e) in b.shape.edges().iter().enumerate() {
        if noshape_kernel::distance(&v, e).is_ok_and(|d| d.distance < 1e-9) {
            return Reference::Edge((*b.edges[i]).clone());
        }
    }
    panic!("no edge at {p:?}");
}

/// Live L1/L2 ("psb L1-4 draft"): block 40×20×10, side faces split at z=5,
/// parting edges at z=5, pull = Top. One sided 3°: 7921.8461 mm³ (upper
/// faces narrow upward); first Opposite direction: 8079.0694 (same faces);
/// Two sided 3° / 10°: 8348.7418; second Opposite off: 7819.7608.
#[test]
fn parting_line_draft_sides_as_live() {
    let run = |sides: &str, opposite: bool, second_opposite: bool| -> f64 {
        let mut s = Studio::new();
        let mut sk = noshape_sketch::Sketch::new();
        polyline(&mut sk, &[(0.0, 0.0), (40.0, 0.0), (40.0, 20.0), (0.0, 20.0)], true);
        let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
        let region = s.region(sid);
        s.add("extrude", move |p| {
            p.set("entities", q(region));
            p.set("depth", expr("10 mm"));
        });
        let z5 = top_plane_at(&mut s, 5.0);
        let r = s.ok();
        let sides_at = |r: &RegenResult, z: f64| vec![face_at(r, 0, DVec3::new(mm(20.0), 0.0, mm(z))), face_at(r, 0, DVec3::new(mm(20.0), mm(20.0), mm(z))), face_at(r, 0, DVec3::new(0.0, mm(10.0), mm(z))), face_at(r, 0, DVec3::new(mm(40.0), mm(10.0), mm(z)))];
        let targets = sides_at(&r, 5.0);
        s.add("splitPart", move |p| {
            p.set("splitType", en("FACE"));
            p.set("faceTargets", ParamValue::Query(targets));
            p.set("faceTools", q(Reference::FeaturePlane(z5)));
        });
        let r = s.ok();
        let edges = vec![
            edge_near(&r, 0, DVec3::new(mm(20.0), 0.0, mm(5.0))),
            edge_near(&r, 0, DVec3::new(mm(20.0), mm(20.0), mm(5.0))),
            edge_near(&r, 0, DVec3::new(0.0, mm(10.0), mm(5.0))),
            edge_near(&r, 0, DVec3::new(mm(40.0), mm(10.0), mm(5.0))),
        ];
        let sides = sides.to_string();
        s.add("draft", move |p| {
            p.set("draftFeatureType", en("PARTING_LINE"));
            p.set("partingEdges", ParamValue::Query(edges));
            p.set("pullDirectionEntity", q(Reference::Plane(DatumPlane::Top)));
            p.set("partingLineSides", en(&sides));
            p.set("angle", expr("3 deg"));
            p.set("secondAngle", expr("10 deg"));
            p.set("pullDirection", ParamValue::Bool(opposite));
            p.set("secondPullDirection", ParamValue::Bool(second_opposite));
        });
        volume(&s.ok(), 0) * 1e9
    };
    for (sides, opp, opp2, want) in [("ONE_SIDED", false, true, 7921.8461), ("ONE_SIDED", true, true, 8079.0694), ("TWO_SIDED", false, true, 8348.7418), ("TWO_SIDED", true, true, 8191.5184), ("TWO_SIDED", false, false, 7819.7608)] {
        let v = run(sides, opp, opp2);
        assert!((v - want).abs() < 1e-3, "{sides} opposite {opp} second {opp2}: {v} vs live {want}");
    }
}

/// Live L12 ("psb L12-13 sweep"): a Ø10 circle at the middle of a 100 mm
/// line sweeps the half toward the path's start (3926.9908 mm³), the other
/// half with the path's Opposite direction, the whole path (7853.9816) with
/// Extend to full path.
#[test]
fn sweep_from_the_middle_of_the_path_as_live() {
    let run = |opposite: bool, extend: bool| -> (f64, f64) {
        let mut s = Studio::new();
        let mut sk = noshape_sketch::Sketch::new();
        let path = polyline(&mut sk, &[(0.0, 0.0), (100.0, 0.0)], false);
        let path_sketch = s.sketch(Reference::Plane(DatumPlane::Top), sk);
        let x50 = s.add("cPlane", |p| {
            p.set("entities", q(Reference::Plane(DatumPlane::Right)));
            p.set("offset", expr("50 mm"));
        });
        let mut sk = noshape_sketch::Sketch::new();
        sk.add_circle(DVec2::ZERO, 0.005);
        let profile_sketch = s.sketch(Reference::FeaturePlane(x50), sk);
        let region = s.region(profile_sketch);
        s.add("sweep", move |p| {
            p.set("profiles", q(region));
            p.set("path", ParamValue::Query(path.iter().map(|&e| Reference::SketchEntity { sketch: path_sketch, entity: e }).collect()));
            p.set("pathOppositeDirection", ParamValue::Bool(opposite));
            p.set("extendToFullPath", ParamValue::Bool(extend));
        });
        let r = s.ok();
        let bb = r.context.bodies[0].shape.bounding_box(false).unwrap();
        (volume(&r, 0) * 1e9, (bb.min.x + bb.max.x) * 0.5 * 1e3)
    };
    let (v, cx) = run(false, false);
    assert!((v - 3926.9908).abs() < 1e-3 && (cx - 25.0).abs() < 1e-6, "{v} centred at x {cx}");
    let (v, cx) = run(true, false);
    assert!((v - 3926.9908).abs() < 1e-3 && (cx - 75.0).abs() < 1e-6, "{v} centred at x {cx}");
    let (v, _) = run(false, true);
    assert!((v - 7853.9816).abs() < 1e-3, "{v}");
}

/// Live L11 ("psb L10-11 delete face"): block 40×20×10 with an R3 fillet on
/// the top front edge, a Ø8×5 boss with an R1 fillet at its base. Deleting
/// the boss (side + top) with Heal removes its base fillet too (7922.7433 =
/// the filleted block), also on an imported copy (fillets are recognised
/// from the geometry); without Delete fillet faces it can't heal.
#[test]
fn delete_face_removes_fillets_by_geometry_as_live() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    polyline(&mut sk, &[(0.0, 0.0), (40.0, 0.0), (40.0, 20.0), (0.0, 20.0)], true);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let region = s.region(sid);
    s.add("extrude", move |p| {
        p.set("entities", q(region));
        p.set("depth", expr("10 mm"));
    });
    let r = s.ok();
    let front_top = edge_near(&r, 0, DVec3::new(mm(20.0), 0.0, mm(10.0)));
    s.add("fillet", move |p| {
        p.set("entities", q(front_top));
        p.set("radius", expr("3 mm"));
    });
    let z10 = top_plane_at(&mut s, 10.0);
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(DVec2::new(0.02, 0.01), 0.004);
    let bsk = s.sketch(Reference::FeaturePlane(z10), sk);
    let region = s.region(bsk);
    s.add("extrude", move |p| {
        p.set("entities", q(region));
        p.set("depth", expr("5 mm"));
        p.set("operationType", en("ADD"));
        p.set("defaultScope", ParamValue::Bool(true));
    });
    let r = s.ok();
    let base = edge_near(&r, 0, DVec3::new(mm(24.0), mm(10.0), mm(10.0)));
    s.add("fillet", move |p| {
        p.set("entities", q(base));
        p.set("radius", expr("1 mm"));
    });
    let r = s.ok();
    let boss = |r: &RegenResult| vec![face_at(r, 0, DVec3::new(mm(20.0), mm(10.0), mm(13.0))), face_at(r, 0, DVec3::new(mm(20.0), mm(10.0), mm(15.0)))];
    let filleted_block = 8000.0 - 40.0 * (9.0 - 9.0 * PI / 4.0);
    // Ours.
    for include in [true, false] {
        let mut t = Studio { ps: s.ps.clone(), ..Studio::new() };
        let faces = boss(&r);
        t.add("deleteFace", move |p| {
            p.set("deleteFaces", ParamValue::Query(faces));
            p.set("includeFillet", ParamValue::Bool(include));
        });
        if include {
            let v = volume(&t.ok(), 0) * 1e9;
            assert!((v - filleted_block).abs() < 1e-3, "{v} vs {filleted_block}");
        } else {
            assert_eq!(t.last_status(), (FeatureStatus::Error, "Could not delete selected faces.".to_string()));
        }
    }
    // An imported copy: no fillet history.
    let brep = String::from_utf8(r.context.bodies[0].shape.to_brep_bytes().unwrap()).unwrap();
    let mut t = Studio::new();
    t.add("importForeign", move |p| p.set("blobData", ParamValue::Text(brep)));
    let ri = t.ok();
    let faces = boss(&ri);
    t.add("deleteFace", move |p| p.set("deleteFaces", ParamValue::Query(faces)));
    let v = volume(&t.ok(), 0) * 1e9;
    assert!((v - filleted_block).abs() < 1e-3, "imported: {v} vs {filleted_block}");
}

/// Live L27: Tangency on a row of sketch curves builds the fill anyway, as
/// a WARNING "Could not meet tangency constraints at highlighted points.".
#[test]
fn fill_tangency_on_curves_is_a_warning() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    let ids = polyline(&mut sk, &[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)], true);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let mut row = ParamValues::default();
    row.set("entities", ParamValue::Query(ids.iter().map(|&e| Reference::SketchEntity { sketch: sid, entity: e }).collect()));
    row.set("continuity", en("G1"));
    s.add("fill", move |p| p.set("edges", ParamValue::Array(vec![row])));
    let r = s.regen();
    let st = r.display.features.last().unwrap();
    assert_eq!((st.status, st.message.as_deref()), (FeatureStatus::Warning, Some("Could not meet tangency constraints at highlighted points.")));
    assert_eq!(r.context.bodies.len(), 1);
}

/// Live L38 ("psb L37-39 mate connector"): on a block's top face, reorient
/// "+Y" turns X to −Y; a move goes along the reoriented/flipped frame; the
/// rotation comes last and doesn't change the move. L37: no owner fails.
#[test]
fn mate_connector_frame_order_as_live() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    polyline(&mut sk, &[(0.0, 0.0), (40.0, 0.0), (40.0, 20.0), (0.0, 20.0)], true);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let region = s.region(sid);
    let block = s.add("extrude", move |p| {
        p.set("entities", q(region));
        p.set("depth", expr("10 mm"));
    });
    let r = s.ok();
    let top = face_at(&r, 0, DVec3::new(mm(20.0), mm(10.0), mm(10.0)));
    let owner = Reference::Body(noshape_model::reference::BodyId { feature: block, index: 0 });
    let run = |set: &dyn Fn(&mut ParamValues)| -> (DVec3, DVec3, DVec3) {
        let mut t = Studio { ps: s.ps.clone(), ..Studio::new() };
        let (top, owner) = (top.clone(), owner.clone());
        t.add("mateConnector", move |p| {
            p.set("originQuery", q(top));
            p.set("ownerPart", q(owner));
            set(p);
        });
        let c = t.ok().context.connectors[0].clone();
        (c.origin * 1e3, c.x, c.z)
    };
    let close = |a: DVec3, b: DVec3| (a - b).length() < 1e-6;
    let (o, x, _) = run(&|p| p.set("secondaryAxisType", en("PLUS_Y")));
    assert!(close(o, DVec3::new(20.0, 10.0, 10.0)) && close(x, DVec3::new(0.0, -1.0, 0.0)), "{o} {x}");
    let (o, x, _) = run(&|p| {
        p.set("secondaryAxisType", en("PLUS_Y"));
        p.set("transform", ParamValue::Bool(true));
        p.set("translationX", expr("10 mm"));
        p.set("rotation", expr("90 deg"));
    });
    assert!(close(o, DVec3::new(20.0, 0.0, 10.0)) && close(x, DVec3::new(1.0, 0.0, 0.0)), "{o} {x}");
    let (o, _, z) = run(&|p| {
        p.set("flipPrimary", ParamValue::Bool(true));
        p.set("transform", ParamValue::Bool(true));
        p.set("translationZ", expr("10 mm"));
    });
    assert!(close(o, DVec3::new(20.0, 10.0, 0.0)) && close(z, DVec3::new(0.0, 0.0, -1.0)), "{o} {z}");
    // No owner and nothing to own it (live L37: an empty studio).
    let mut e = Studio::new();
    e.add("mateConnector", |p| p.set("originQuery", q(Reference::Origin)));
    assert_eq!(e.last_status(), (FeatureStatus::Error, "Cannot determine owner of mate connector.".to_string()));
}

/// Live (qa5 "sm loft", SML1): Sheet metal loft, Add, from a 60 × 40 × 2
/// plate's top edge at y = 40 (radius 2) to a line (0,70)–(60,70) on a plane
/// 30 mm up: one body of 9805.52442264952 mm³, 14 faces; the plate trimmed
/// to y = 39.2116736 by a 43.0248° bend; the line on the wall's outer face.
#[test]
fn sheet_metal_loft_add_bends_onto_the_plate() {
    let mut s = Studio::new();
    let mut sk = noshape_sketch::Sketch::new();
    polyline(&mut sk, &[(0.0, 0.0), (60.0, 0.0), (60.0, 40.0), (0.0, 40.0)], true);
    let sid = s.sketch(Reference::Plane(DatumPlane::Top), sk);
    let region = s.region(sid);
    s.add("sheetMetalStart", move |p| {
        p.set("process", en("THICKEN"));
        p.set("regions", q(region));
        set_model(p, "2 mm", "2 mm");
    });
    let pl = s.add("cPlane", |p| {
        p.set("entities", q(Reference::Plane(DatumPlane::Top)));
        p.set("offset", expr("30 mm"));
    });
    let mut sk = noshape_sketch::Sketch::new();
    let line = polyline(&mut sk, &[(0.0, 70.0), (60.0, 70.0)], false);
    let s2 = s.sketch(Reference::FeaturePlane(pl), sk);
    let r = s.ok();
    let body = &r.context.bodies[0];
    let ei = body.shape.edges().iter().position(|e| e.endpoints().is_ok_and(|(a, b)| ((a + b) * 0.5 - DVec3::new(mm(30.0), mm(40.0), mm(2.0))).length() < 1e-9)).unwrap();
    let edge = Reference::Edge((*body.edges[ei]).clone());
    let part = Reference::Body(body.id.clone());
    s.add("sheetMetalLoft", move |p| {
        p.set("surfaceOperationType", en("ADD"));
        p.set("profile1", q(edge));
        p.set("profile2", entities(s2, &line));
        p.set("booleanScope", q(part));
    });
    let r = s.ok();
    assert_eq!(r.context.bodies.len(), 1, "{:?}", r.context.bodies.iter().map(|b| (b.kind, b.shape.volume().unwrap_or(0.0) * 1e9, b.faces.len())).collect::<Vec<_>>());
    let b = &r.context.bodies[0];
    assert!(near(volume(&r, 0), 9805.52442264952e-9, 1e-15), "volume {}", volume(&r, 0) * 1e9);
    assert_eq!(b.faces.len(), 14);
    let data = r.context.sheet_metal.get(&b.id).unwrap();
    assert_eq!((data.walls.len(), data.bends.len()), (2, 1));
}
