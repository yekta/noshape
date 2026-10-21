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
            assert!(matches!(s.status, FeatureStatus::Ok | FeatureStatus::Warning), "{} ({}) failed: {:?}", f.name, f.type_id, s.message);
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
fn flanges_on_adjacent_edges_match_live() {
    let r = g34().ok();
    let faces = r.context.bodies[part(&r)].shape.faces().len();
    assert!((volume_mm3(&r) - 8511.4987).abs() < 1e-3 && faces == 24, "{} mm³, {} faces", volume_mm3(&r), faces);
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
#[ignore = "in progress (fs): std-run sheet metal, not yet verified"]
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
    assert!((volume_mm3(&r) - (8511.4987 + 400.0)).abs() < 1e-3, "{} mm³", volume_mm3(&r));
}
