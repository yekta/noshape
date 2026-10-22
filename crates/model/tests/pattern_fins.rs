//! User report (v0.4.4, lead "fins"): a block with a Feature linear pattern
//! of thin fins on top, the first fin flush with the block's front face.
//! A union merges coplanar faces into one (Onshape, live G45), so the front
//! wall (the block's front face and the first fin's) must be one face, with
//! no seam and no doubled edges.

use glam::DVec2;
use noshape_model::configuration::ConfigValues;
use noshape_model::display::FeatureStatus;
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, FeatureId, Reference};
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};

fn e(s: &str) -> ParamValue {
    ParamValue::Expr(ExprValue::new(s))
}
fn q(v: Vec<Reference>) -> ParamValue {
    ParamValue::Query(v)
}
fn en(s: &str) -> ParamValue {
    ParamValue::Enum(s.into())
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> noshape_sketch::Sketch {
    let mut sk = noshape_sketch::Sketch::new();
    let p = [DVec2::new(x0, y0), DVec2::new(x1, y0), DVec2::new(x1, y1), DVec2::new(x0, y1)];
    let pts: Vec<_> = p.iter().map(|q| sk.add_point(*q)).collect();
    for i in 0..4 {
        sk.add_line_between(pts[i], pts[(i + 1) % 4]).unwrap();
    }
    sk
}

struct Studio {
    ps: PartStudio,
    units: UnitPrefs,
}

impl Studio {
    fn add(&mut self, ty: &str, params: &[(&str, ParamValue)]) -> FeatureId {
        let mut f = self.ps.new_feature(ty, &self.units).unwrap();
        for (k, v) in params {
            f.params.set(k, v.clone());
        }
        let id = f.id;
        self.ps.features.push(f);
        id
    }
    fn regen(&self) -> RegenResult {
        Regenerator::new().regenerate(&self.ps, &ConfigValues::default(), &self.units, &RegenOptions::default())
    }
    fn region(&mut self, sk: noshape_sketch::Sketch) -> Reference {
        let s = self.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Top)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
        let r = self.regen();
        let d = r.display.sketches.iter().find(|x| x.feature == s).unwrap();
        Reference::SketchRegion { sketch: s, key: d.regions[0].key.clone() }
    }
}

/// Live "Pattern face merging": block 100 × max(50, 2n) × 40 mm; a 1 mm fin
/// flush with its front face (y = 0), 40 mm high on top, across the block
/// (`full`: x 0..100, else 10..90); a Feature linear pattern of the fin
/// along y, 2 mm apart, `n` instances.
fn fins(n: usize, full: bool, reapply: bool) -> (Studio, RegenResult) {
    let len = (2 * n).max(50) as f64 * 1e-3;
    let mut s = Studio { ps: PartStudio::new(), units: UnitPrefs::default() };
    let block = s.region(rect(0.0, 0.0, 0.1, len));
    s.add("extrude", &[("entities", q(vec![block])), ("depth", e("40 mm")), ("operationType", en("NEW"))]);
    let (x0, x1) = if full { (0.0, 0.1) } else { (0.01, 0.09) };
    let fin = s.region(rect(x0, 0.0, x1, 0.001));
    let fin = s.add(
        "extrude",
        &[("entities", q(vec![fin])), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true)), ("startOffset", ParamValue::Bool(true)), ("startOffsetBound", en("BLIND")), ("startOffsetDistance", e("40 mm")), ("depth", e("40 mm"))],
    );
    s.add(
        "linearPattern",
        &[
            ("patternType", en("FEATURE")),
            ("instanceFunction", q(vec![Reference::Feature(fin)])),
            ("directionOne", q(vec![Reference::Plane(DatumPlane::Front)])),
            // The Front plane's normal is -y: back over the block.
            ("oppositeDirection", ParamValue::Bool(true)),
            ("distance", e("2 mm")),
            ("instanceCount", e(&n.to_string())),
            ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true)),
            ("fullFeaturePattern", ParamValue::Bool(reapply)),
        ],
    );
    let r = s.regen();
    (s, r)
}

/// The part's topology: faces, edges, vertices, the faces on the front plane
/// y = 0 and their area (mm²), and duplicate edges (same end points).
struct Topo {
    faces: usize,
    edges: usize,
    vertices: usize,
    front: usize,
    front_area: f64,
    dups: usize,
    /// Faces on the side planes x = 0 and x = 100 mm.
    sides: (usize, usize),
    /// Faces on the base's top plane z = 40 mm.
    base_top: usize,
}

fn topology(r: &RegenResult) -> Topo {
    let solids: Vec<_> = r.context.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).collect();
    assert_eq!(solids.len(), 1);
    let sh = &solids[0].shape;
    let faces = sh.faces();
    let front: Vec<_> = faces
        .iter()
        .filter(|f| {
            let bb = f.bounding_box(false).unwrap();
            bb.max.y.abs() < 1e-6 && bb.min.y.abs() < 1e-6
        })
        .collect();
    let mut ends: Vec<[i64; 6]> = sh
        .edges()
        .iter()
        .filter_map(|e| {
            let i = e.edge_info().ok()?;
            let k = |v: f64| (v * 1e7).round() as i64;
            let (a, b) = (i.start, i.end);
            let (a, b) = if (a.x, a.y, a.z) <= (b.x, b.y, b.z) { (a, b) } else { (b, a) };
            Some([k(a.x), k(a.y), k(a.z), k(b.x), k(b.y), k(b.z)])
        })
        .collect();
    ends.sort();
    let on = |axis: usize, v: f64| {
        faces
            .iter()
            .filter(|f| {
                let bb = f.bounding_box(false).unwrap();
                (bb.min[axis] - v).abs() < 1e-6 && (bb.max[axis] - v).abs() < 1e-6
            })
            .count()
    };
    Topo {
        sides: (on(0, 0.0), on(0, 0.1)),
        base_top: on(2, 0.04),
        faces: faces.len(),
        edges: sh.count(noshape_kernel::ShapeType::Edge),
        vertices: sh.count(noshape_kernel::ShapeType::Vertex),
        front: front.len(),
        front_area: front.iter().map(|f| f.area().unwrap()).sum::<f64>() * 1e6,
        dups: ends.windows(2).filter(|w| w[0] == w[1]).count(),
    }
}

/// With NOSHAPE_PERF set, the regeneration's timing logs (target "perf") on stderr.
struct PerfLog;
impl log::Log for PerfLog {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.target() == "perf"
    }
    fn log(&self, r: &log::Record) {
        if self.enabled(r.metadata()) {
            eprintln!("  perf: {}", r.args());
        }
    }
    fn flush(&self) {}
}

fn status(r: &RegenResult) -> (FeatureStatus, Option<String>) {
    r.display.features.last().map(|f| (f.status, f.message.clone())).unwrap()
}

#[test]
fn seed_fin_merges_into_the_front_wall() {
    // Live: the Fin alone has 8 faces and 18 edges, its front one face with
    // the block's (100 × 80 mm).
    let (_, r) = fins(1, true, true);
    assert!(matches!(status(&r).0, FeatureStatus::Ok), "{:?}", status(&r));
    let t = topology(&r);
    assert_eq!((t.faces, t.edges, t.front, t.dups), (8, 18, 1, 0));
    assert!((t.front_area - 8000.0).abs() < 1e-6, "{}", t.front_area);
}

#[test]
fn fin_pattern_reapplied_matches_onshape() {
    // Live (N = 25): one part, 4N + 4 faces, 12N + 6 edges, 8N + 4 vertices;
    // the front wall one 8000 mm² face, no seam at the fin root.
    for n in [2usize, 25] {
        let (_, r) = fins(n, true, true);
        assert!(matches!(status(&r).0, FeatureStatus::Ok), "×{n}: {:?}", status(&r));
        let t = topology(&r);
        assert_eq!((t.faces, t.edges, t.vertices, t.front, t.dups), (4 * n + 4, 12 * n + 6, 8 * n + 4, 1, 0), "×{n}");
        assert!((t.front_area - 8000.0).abs() < 1e-6, "×{n}: {}", t.front_area);
        // The sides one comb face each; the base top in n strips.
        assert_eq!((t.sides, t.base_top), ((1, 1), n), "×{n}");
    }
}

#[test]
fn inset_fin_pattern_reapplied_has_one_front_wall() {
    let n = 5;
    let (_, r) = fins(n, false, true);
    assert!(matches!(status(&r).0, FeatureStatus::Ok), "{:?}", status(&r));
    let t = topology(&r);
    // Block 6 faces; the first fin adds its top, back and two ends (its front
    // is the block's), every other fin also its front.
    assert_eq!((t.faces, t.front, t.dups), (5 * n + 5, 1, 0));
}

#[test]
fn fin_feature_pattern_without_reapply_fails_like_onshape() {
    // Live: the fins merge into the block's front and side faces, so the
    // face-based Feature pattern can't place the instances.
    for full in [true, false] {
        let (_, r) = fins(5, full, false);
        let (st, msg) = status(&r);
        assert!(matches!(st, FeatureStatus::Error), "{st:?}");
        assert_eq!(msg.as_deref(), Some("Could not create all instances as entered. Try selecting \"Reapply features\" option."));
    }
}

#[test]
fn fin_pattern_x1000_matches_onshape_and_is_fast() {
    // The whole regeneration (every feature, then the display: tessellation
    // and edges), as the app runs it. Onshape: 4004 faces, 12006 edges, 8004
    // vertices; ~82 s for the preview and ~13.8 s from ✓ to the final result
    // (live, loaded machine).
    if std::env::var_os("NOSHAPE_PERF").is_some() && log::set_logger(&PerfLog).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
    let timed = |n: usize| {
        let t0 = std::time::Instant::now();
        let (_, r) = fins(n, true, true);
        (t0.elapsed().as_secs_f64(), r)
    };
    // Speed against a baseline from the same run (the machine's load varies
    // with every other agent's builds): ×200 before and after ×1000.
    let (before, _) = timed(200);
    let (total, r) = timed(1000);
    let (after, _) = timed(200);
    assert!(matches!(status(&r).0, FeatureStatus::Ok), "{:?}", status(&r));
    let t = topology(&r);
    assert_eq!((t.faces, t.edges, t.vertices, t.front, t.dups, t.sides, t.base_top), (4004, 12006, 8004, 1, 0, (1, 1), 1000));
    let pattern = r.display.features.last().unwrap().time_ms as f64 / 1e3;
    let base = before.max(after);
    eprintln!("fins ×1000: pattern {pattern:.2} s, whole regeneration with display {total:.2} s; ×200 {before:.2} / {after:.2} s, ratio {:.2}", total / base);
    // Linear is 5×; OCCT's shell orientation step adds a little (pattern_perf:
    // 5.6-6.4); the old quadratic paths were ~25×.
    assert!(total < 8.0 * base, "×1000 {total:.2} s against ×200 {base:.2} s");
}

/// Live "Pending state" ("pending pattern 2"): block 520 × 60 × 10 mm, a rib
/// 0.2 × 40 mm extruded 15 mm Add, a default Feature pattern along the block
/// (0.5 mm): Onshape patterns it without error. Both ways to make the rib:
/// from the Top plane through the block, and standing on its top face.
/// Live G45 (narrow rib, 5 × 5 mm): 11 + 4 × 5 faces, i.e. 6 + 5N.
#[test]
fn narrow_rib_feature_pattern_without_reapply_matches_onshape() {
    for on_top in [false, true] {
        let n = 200usize;
        let mut s = Studio { ps: PartStudio::new(), units: UnitPrefs::default() };
        let block = s.region(rect(0.0, 0.0, 0.52, 0.06));
        s.add("extrude", &[("entities", q(vec![block])), ("depth", e("10 mm")), ("operationType", en("NEW"))]);
        let rib = s.region(rect(0.005, 0.01, 0.0052, 0.05));
        let mut p = vec![("entities", q(vec![rib])), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true)), ("depth", e("15 mm"))];
        if on_top {
            p.extend([("startOffset", ParamValue::Bool(true)), ("startOffsetBound", en("BLIND")), ("startOffsetDistance", e("10 mm"))]);
        }
        let rib = s.add("extrude", &p);
        s.add(
            "linearPattern",
            &[
                ("patternType", en("FEATURE")),
                ("instanceFunction", q(vec![Reference::Feature(rib)])),
                ("directionOne", q(vec![Reference::Plane(DatumPlane::Right)])),
                ("distance", e("0.5 mm")),
                ("instanceCount", e(&n.to_string())),
                ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true)),
            ],
        );
        let r = s.regen();
        assert!(matches!(status(&r).0, FeatureStatus::Ok), "on top {on_top}: {:?}", status(&r));
        let solids: Vec<_> = r.context.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).collect();
        assert_eq!(solids.len(), 1, "on top {on_top}");
        assert_eq!(solids[0].shape.count(noshape_kernel::ShapeType::Face), 6 + 5 * n, "on top {on_top}");
    }
}

/// The user's v0.4.5 model: a long block 100 × 20 × 10 mm, a 0.5 mm wall
/// added at its end, overhanging it (y -1..21 mm, 12 mm high), and a
/// default Feature pattern of the wall along the block, 2 mm, 2 instances.
/// Onshape fails it asking for Reapply features (checked live by the user);
/// with Reapply features it makes two walls 2 mm apart.
fn overhanging_wall(reapply: bool) -> RegenResult {
    let mut s = Studio { ps: PartStudio::new(), units: UnitPrefs::default() };
    let block = s.region(rect(0.0, 0.0, 0.1, 0.02));
    s.add("extrude", &[("entities", q(vec![block])), ("depth", e("10 mm")), ("operationType", en("NEW"))]);
    let wall = s.region(rect(0.0, -0.001, 0.0005, 0.021));
    let wall = s.add("extrude", &[("entities", q(vec![wall])), ("depth", e("12 mm")), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true))]);
    s.add(
        "linearPattern",
        &[
            ("patternType", en("FEATURE")),
            ("instanceFunction", q(vec![Reference::Feature(wall)])),
            ("directionOne", q(vec![Reference::Plane(DatumPlane::Right)])),
            ("distance", e("2 mm")),
            ("instanceCount", e("2")),
            ("operationType", en("ADD")),
            ("defaultScope", ParamValue::Bool(true)),
            ("fullFeaturePattern", ParamValue::Bool(reapply)),
        ],
    );
    s.regen()
}

#[test]
fn overhanging_wall_feature_pattern_matches_onshape() {
    let r = overhanging_wall(false);
    let (st, msg) = status(&r);
    assert!(matches!(st, FeatureStatus::Error), "{st:?}");
    assert_eq!(msg.as_deref(), Some("Could not create all instances as entered. Try selecting \"Reapply features\" option."));
    let r = overhanging_wall(true);
    assert!(matches!(status(&r).0, FeatureStatus::Ok), "{:?}", status(&r));
    let solids: Vec<_> = r.context.bodies.iter().filter(|b| b.shape.count(noshape_kernel::ShapeType::Solid) > 0).collect();
    assert_eq!(solids.len(), 1);
    // The block, and per wall its 0.5 × 22 × 12 mm less the 0.5 × 20 × 10 mm inside the block.
    let v = solids[0].shape.volume().unwrap() * 1e9;
    assert!((v - (20000.0 + 2.0 * 32.0)).abs() < 1e-6, "{v}");
    // The second wall's overhang 2 mm along: material at x 2.25 mm, y -0.5 mm.
    let inside = |x: f64, y: f64| matches!(solids[0].shape.classify_point(glam::DVec3::new(x, y, 0.005), 1e-9), Ok(noshape_kernel::PointClass::Inside));
    assert!(inside(0.00025, -0.0005) && inside(0.00225, -0.0005) && !inside(0.00125, -0.0005));
}
