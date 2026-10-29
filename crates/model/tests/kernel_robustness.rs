//! Modelling operations that Onshape (Parasolid) completes must complete here
//! too, with the right solid: shell, fillet, chamfer and friends on the
//! awkward inputs where OCCT's stock algorithms fail. Volumes are checked
//! against exact values (built independently) or Onshape's mass properties.

use glam::DVec2;
use noshape_model::configuration::ConfigValues;
use noshape_model::display::FeatureStatus;
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, FeatureId, Reference};
use noshape_model::regen::{RegenOptions, RegenResult, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};

const MM3: f64 = 1e-9;

struct T {
    ps: PartStudio,
    units: UnitPrefs,
    reg: Regenerator,
}

fn e(s: &str) -> ParamValue {
    ParamValue::Expr(ExprValue::new(s))
}
fn q(v: Vec<Reference>) -> ParamValue {
    ParamValue::Query(v)
}
fn en(s: &str) -> ParamValue {
    ParamValue::Enum(s.into())
}

impl T {
    fn new() -> Self {
        T { ps: PartStudio::new(), units: UnitPrefs::default(), reg: Regenerator::new() }
    }
    fn fork(&self) -> Self {
        T { ps: self.ps.clone(), units: self.units, reg: Regenerator::new() }
    }
    fn regen(&mut self) -> RegenResult {
        self.reg.regenerate(&self.ps, &ConfigValues::default(), &self.units, &RegenOptions::default())
    }
    fn add(&mut self, ty: &str, params: &[(&str, ParamValue)]) -> FeatureId {
        let mut f = self.ps.new_feature(ty, &self.units).unwrap_or_else(|| panic!("no feature type {ty}"));
        for (k, v) in params {
            f.params.set(k, v.clone());
        }
        let id = f.id;
        self.ps.features.push(f);
        id
    }
    fn set(&mut self, id: FeatureId, k: &str, v: ParamValue) {
        self.ps.feature_mut(id).unwrap().params.set(k, v);
    }
    /// Regenerate; the feature must be OK. Returns the result.
    fn ok(&mut self, id: FeatureId) -> RegenResult {
        let r = self.regen();
        let s = r.display.features.iter().find(|s| s.id == id).expect("feature state");
        assert_eq!(s.status, FeatureStatus::Ok, "{:?}", s.message);
        r
    }
    /// Sketch on Top with the given closed polylines (mm) and circles (centre, radius in mm).
    fn sketch(&mut self, polys: &[&[(f64, f64)]], circles: &[((f64, f64), f64)]) -> (FeatureId, Vec<Reference>) {
        let mut sk = noshape_sketch::Sketch::new();
        for poly in polys {
            let pts: Vec<_> = poly.iter().map(|(x, y)| sk.add_point(DVec2::new(x * 1e-3, y * 1e-3))).collect();
            for i in 0..pts.len() {
                sk.add_line_between(pts[i], pts[(i + 1) % pts.len()]).unwrap();
            }
        }
        for ((x, y), r) in circles {
            sk.add_circle(DVec2::new(x * 1e-3, y * 1e-3), r * 1e-3);
        }
        let s = self.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Top)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
        let r = self.regen();
        let regions = r.display.sketches.iter().find(|x| x.feature == s).unwrap().regions.iter().map(|g| Reference::SketchRegion { sketch: s, key: g.key.clone() }).collect();
        (s, regions)
    }
    /// Block a x b x h mm from the origin on Top.
    fn block(&mut self, a: f64, b: f64, h: f64) -> FeatureId {
        let (_, regions) = self.sketch(&[&[(0.0, 0.0), (a, 0.0), (a, b), (0.0, b)]], &[]);
        self.add("extrude", &[("entities", q(regions)), ("depth", e(&format!("{h} mm"))), ("operationType", en("NEW"))])
    }
    /// Through-all cut of circles (mm).
    fn holes(&mut self, circles: &[((f64, f64), f64)]) -> FeatureId {
        let (_, regions) = self.sketch(&[], circles);
        self.add("extrude", &[("entities", q(regions)), ("endBound", en("THROUGH_ALL")), ("operationType", en("REMOVE")), ("defaultScope", ParamValue::Bool(true))])
    }
    fn edges_where(&mut self, f: impl Fn(&noshape_kernel::EdgeInfo) -> bool) -> Vec<Reference> {
        let r = self.regen();
        let b = &r.context.bodies[0];
        b.shape.edges().iter().enumerate().filter(|(_, x)| x.edge_info().map(|i| !i.degenerated && f(&i)).unwrap_or(false)).map(|(i, _)| Reference::Edge((*b.edges[i]).clone())).collect()
    }
    fn faces_where(&mut self, f: impl Fn(glam::DVec3) -> bool) -> Vec<Reference> {
        let r = self.regen();
        let b = &r.context.bodies[0];
        b.shape.faces().iter().enumerate().filter(|(_, x)| f(x.centroid().unwrap() * 1e3)).map(|(i, _)| Reference::Face((*b.faces[i]).clone())).collect()
    }
    fn fillet(&mut self, edges: Vec<Reference>, r: &str) -> FeatureId {
        self.add("fillet", &[("entities", q(edges)), ("radius", e(r)), ("tangentPropagation", ParamValue::Bool(true))])
    }
    fn chamfer(&mut self, edges: Vec<Reference>, w: &str) -> FeatureId {
        self.add("chamfer", &[("entities", q(edges)), ("width", e(w)), ("tangentPropagation", ParamValue::Bool(true))])
    }
    fn shell(&mut self, faces: Vec<Reference>, th: &str) -> FeatureId {
        self.add("shell", &[("entities", q(faces)), ("thickness", e(th))])
    }
}

fn volume(r: &RegenResult) -> f64 {
    noshape_kernel::volume_properties(&r.context.bodies[0].shape).unwrap().mass / MM3
}

fn vertical(i: &noshape_kernel::EdgeInfo) -> bool {
    (i.start.z - i.end.z).abs() > 1e-9
}

/// QA3 case B: a 40 x 20 x 10 block with 3 mm vertical corner fillets and a
/// 1 mm chamfer round the top, shelled 2.5 mm with the top removed. OCCT's
/// offset can't build it (the chamfer's corner cones vanish in the offset);
/// Onshape can. Exact volume 4125.9988 mm³ (half-space construction, see
/// docs/handoff/kernel-robustness.md); 1 mm: 1846.8729 mm³ (OCCT agrees).
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn shell_block_with_fillets_and_chamfer() {
    let mut t = T::new();
    t.block(40.0, 20.0, 10.0);
    let v = t.edges_where(vertical);
    t.fillet(v, "3 mm");
    let top = t.edges_where(|i| i.start.z > 0.00999 && i.end.z > 0.00999);
    t.chamfer(vec![top[0].clone()], "1 mm");
    let top_face = t.faces_where(|c| c.z > 9.999);
    assert_eq!(top_face.len(), 1);
    let s = t.shell(top_face, "2.5 mm");
    let r = t.ok(s);
    let b = &r.context.bodies[0];
    assert!(b.shape.is_valid());
    assert!((volume(&r) - 4125.9988).abs() < 1e-3, "{}", volume(&r));
    let names_25: std::collections::BTreeSet<_> = b.faces.iter().map(|n| format!("{n:?}")).collect();
    assert_eq!(names_25.len(), b.faces.len(), "face names are unique");
    t.set(s, "thickness", e("1 mm"));
    let r = t.ok(s);
    assert!((volume(&r) - 1846.8729).abs() < 1e-3, "{}", volume(&r));
    // The two thicknesses take different construction paths but give the same
    // topology, so every face keeps its name.
    let names_1: std::collections::BTreeSet<_> = r.context.bodies[0].faces.iter().map(|n| format!("{n:?}")).collect();
    let only_1: Vec<_> = names_1.difference(&names_25).collect();
    let only_25: Vec<_> = names_25.difference(&names_1).collect();
    assert!(only_1.is_empty() && only_25.is_empty(), "only at 1 mm: {only_1:#?}\nonly at 2.5 mm: {only_25:#?}");
}

/// QA3 case A: a Ø10 hole 3 mm from the front of a 100 x 40 x 12 plate,
/// shelled 2.5 mm from the top. The hole's wall (r = 7.5 after the offset)
/// runs into the front wall and merges with it.
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn shell_plate_with_hole_near_the_edge() {
    let mut t = T::new();
    t.block(100.0, 40.0, 12.0);
    t.holes(&[((50.0, 8.0), 5.0)]);
    let top = t.faces_where(|c| c.z > 11.999);
    let s = t.shell(top, "2.5 mm");
    let r = t.ok(s);
    assert!(r.context.bodies[0].shape.is_valid());
    let (rr, d) = (7.5_f64, 5.5_f64);
    let segment = rr * rr * (d / rr).acos() - d * (rr * rr - d * d).sqrt();
    let cavity = 95.0 * 35.0 * 9.5 - (std::f64::consts::PI * rr * rr - segment) * 9.5;
    let want = 100.0 * 40.0 * 12.0 - std::f64::consts::PI * 25.0 * 12.0 - cavity;
    assert!((volume(&r) - want).abs() < 1e-3, "{} vs {want}", volume(&r));
    // The real QA3 bracket (rounded corners, hole 10 mm from the front):
    // shell_bracket_matches_onshape (at exactly 2.5 mm Onshape fails).
}

/// Shell variants Onshape completes: outward, hollow (no faces removed),
/// several faces removed, a drafted (conical) wall.
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn shell_common_variants() {
    let mut t = T::new();
    t.block(40.0, 20.0, 10.0);
    let base = t.fork();
    // Outward, top removed: (42 x 22 x 11) - 40 x 20 x 10 with sharp corners.
    let top = t.faces_where(|c| c.z > 9.999);
    let s = t.shell(top.clone(), "1 mm");
    t.set(s, "oppositeDirection", ParamValue::Bool(true));
    let r = t.ok(s);
    assert!((volume(&r) - (42.0 * 22.0 * 11.0 - 8000.0)).abs() < 1e-3, "{}", volume(&r));
    // Hollow: a 2 mm wall all round, the inside is a void.
    let mut t = base.fork();
    let r = t.regen();
    let body = r.context.bodies[0].id.clone();
    let s = t.add("shell", &[("isHollow", ParamValue::Bool(true)), ("parts", q(vec![Reference::Body(body)])), ("thickness", e("2 mm"))]);
    let r = t.ok(s);
    assert!((volume(&r) - (8000.0 - 36.0 * 16.0 * 6.0)).abs() < 1e-3, "{}", volume(&r));
    // Two faces removed: top and the x = 40 side.
    let mut t = base.fork();
    let faces = t.faces_where(|c| c.z > 9.999 || c.x > 39.999);
    assert_eq!(faces.len(), 2);
    let s = t.shell(faces, "2 mm");
    let r = t.ok(s);
    assert!((volume(&r) - (8000.0 - 38.0 * 16.0 * 8.0)).abs() < 1e-3, "{}", volume(&r));
}

/// Onshape's own volumes (live capture, docs/handoff/kernel-robustness.md,
/// /root/onshape-research/live/behaviours.md "Geometry ground truth").
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn shell_matches_onshape_volumes() {
    fn check(t: &mut T, s: FeatureId, th: &str, want: f64) {
        t.set(s, "thickness", e(th));
        let r = t.ok(s);
        assert!(r.context.bodies[0].shape.is_valid(), "{th}: invalid");
        assert!((volume(&r) - want).abs() < 1e-3, "{th}: {} vs Onshape {want}", volume(&r));
    }
    // S1: plate with a Ø10 hole 3 mm from the front.
    let mut t = T::new();
    t.block(100.0, 40.0, 12.0);
    t.holes(&[((50.0, 8.0), 5.0)]);
    let top = t.faces_where(|c| c.z > 11.999);
    let s = t.shell(top, "2.5 mm");
    for (th, want) in [("2.5 mm", 17015.74416091595), ("1 mm", 7337.592894744625), ("4 mm", 25079.829458241205)] {
        check(&mut t, s, th, want);
    }
    // S2: fillets + chamfer round the top.
    let mut t = T::new();
    t.block(40.0, 20.0, 10.0);
    let v = t.edges_where(vertical);
    t.fillet(v, "3 mm");
    let top = t.edges_where(|i| i.start.z > 0.00999 && i.end.z > 0.00999);
    t.chamfer(vec![top[0].clone()], "1 mm");
    let top_face = t.faces_where(|c| c.z > 9.999);
    let s = t.shell(top_face, "2.5 mm");
    for (th, want) in [("2.5 mm", 4125.998753703931), ("1 mm", 1846.8728715398622), ("3 mm", 4760.739785668502)] {
        check(&mut t, s, th, want);
    }
    // S3: outward (sharp outer corners).
    let mut t = T::new();
    t.block(40.0, 20.0, 10.0);
    let top = t.faces_where(|c| c.z > 9.999);
    let s = t.shell(top, "2 mm");
    t.set(s, "oppositeDirection", ParamValue::Bool(true));
    check(&mut t, s, "2 mm", 4672.0);
    // S4: a step, bottom removed (sharp at the concave edge), then also x = 40.
    let mut t = T::new();
    t.block(40.0, 40.0, 10.0);
    let (_, regions) = t.sketch(&[&[(20.0, 0.0), (40.0, 0.0), (40.0, 40.0), (20.0, 40.0)]], &[]);
    t.add("extrude", &[("entities", q(regions)), ("depth", e("20 mm")), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true))]);
    let bottom = t.faces_where(|c| c.z < 1e-6);
    let s = t.shell(bottom, "2 mm");
    check(&mut t, s, "2 mm", 7872.0);
    let two = t.fork();
    let mut t = two;
    t.ps.features.pop();
    let faces = t.faces_where(|c| c.z < 1e-6 || c.x > 39.999);
    assert_eq!(faces.len(), 2);
    let s = t.shell(faces, "2 mm");
    check(&mut t, s, "2 mm", 6575.999999999997);
    // S5: cylinder, then drafted 5° both ways.
    for (draft, want) in [(None, 8099.025860954483), (Some(false), 9019.971396790987), (Some(true), 7225.620114703993)] {
        let mut t = T::new();
        let (_, regions) = t.sketch(&[], &[((0.0, 0.0), 15.0)]);
        t.add("extrude", &[("entities", q(regions)), ("depth", e("40 mm")), ("operationType", en("NEW"))]);
        if let Some(flip) = draft {
            let bottom = t.faces_where(|c| c.z < 1e-6);
            let side = t.faces_where(|c| c.z > 1.0 && c.z < 39.0);
            let d = t.add("draft", &[("neutralPlane", q(bottom)), ("draftFaces", q(side)), ("angle", e("5 deg")), ("pullDirection", ParamValue::Bool(flip))]);
            let r = t.ok(d);
            let drafted = if flip { 22190.837907364912 } else { 35383.81755242565 };
            assert!((volume(&r) - drafted).abs() < 1e-3, "draft flip={flip}: {} vs Onshape {drafted}", volume(&r));
        }
        let top = t.faces_where(|c| c.z > 39.999);
        let s = t.shell(top, "2 mm");
        check(&mut t, s, "2 mm", want);
    }
}

/// Regeneration timings of booleans, blends and shell on a mid-size part
/// (`cargo test -p noshape-model --test all kernel_robustness:: -- --ignored --nocapture`).
#[test]
#[ignore]
fn bench_kernel_ops() {
    let mut t = T::new();
    t.block(120.0, 80.0, 12.0);
    for i in 0..24 {
        let (x, y) = (12.0 + (i % 6) as f64 * 19.0, 12.0 + (i / 6) as f64 * 18.0);
        if (40.0..=80.0).contains(&x) && (25.0..=55.0).contains(&y) {
            continue;
        }
        t.holes(&[((x, y), 3.0 + (i % 3) as f64)]);
    }
    let (_, regions) = t.sketch(&[&[(40.0, 32.0), (80.0, 32.0), (80.0, 46.0), (40.0, 46.0)]], &[]);
    t.add("extrude", &[("entities", q(regions)), ("depth", e("20 mm")), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true))]);
    let v = t.edges_where(vertical);
    let v: Vec<_> = v.into_iter().take(4).collect();
    t.fillet(v, "5 mm");
    let top = t.faces_where(|c| c.z > 11.999 && c.z < 12.001);
    t.shell(top, "1.5 mm");
    // Best of three cold regenerations.
    let mut best: std::collections::BTreeMap<String, (usize, f64)> = Default::default();
    let mut r = t.regen();
    for _ in 0..3 {
        t.reg = Regenerator::new();
        let t0 = std::time::Instant::now();
        r = t.regen();
        eprintln!("full regen: {:?}", t0.elapsed());
        let mut by_type: std::collections::BTreeMap<String, (usize, f64)> = Default::default();
        for (f, s) in t.ps.features.iter().zip(&r.display.features) {
            let e = by_type.entry(f.type_id.clone()).or_default();
            e.0 += 1;
            e.1 += f64::from(s.time_ms);
            if !matches!(s.status, FeatureStatus::Ok) {
                eprintln!("  {} {:?} {:?}", f.type_id, s.status, s.message);
            }
        }
        for (ty, v) in by_type {
            let b = best.entry(ty).or_insert(v);
            b.1 = b.1.min(v.1);
        }
    }
    for (ty, (n, ms)) in best {
        eprintln!("  {ty:10} x{n:3}: {ms:8.1} ms (best of 3)");
    }
    eprintln!("  faces: {}", r.context.bodies[0].faces.len());
    // Shell alone: the old MakeThickSolid op against the new one, on the part before the shell.
    t.ps.features.pop();
    let r = t.regen();
    let body = r.context.bodies[0].shape.clone();
    // One more hole: the boolean alone, and the face merge after it.
    let hole = noshape_kernel::make_cylinder(&noshape_kernel::Plane::from_normal(glam::DVec3::new(0.1, 0.07, -0.001), glam::DVec3::Z), 0.002, 0.03, std::f64::consts::TAU).unwrap();
    for _ in 0..2 {
        let t0 = std::time::Instant::now();
        let c = noshape_kernel::cut(&body, &hole).unwrap();
        let t1 = t0.elapsed();
        let u = noshape_kernel::unify_same_domain(&c.shape).unwrap();
        eprintln!("  cut {:?}, unify {:?} ({} -> {} faces)", t1, t0.elapsed() - t1, c.shape.faces().len(), u.shape.faces().len());
    }
    let top: Vec<_> = body.faces().into_iter().filter(|f| (f.centroid().unwrap().z - 0.012).abs() < 1e-6).collect();
    for (name, f) in [
        ("old k::shell (Arc)", Box::new(|| noshape_kernel::shell(&body, &top, 0.0015, noshape_kernel::ShellSide::Inside, noshape_kernel::JoinType::Arc).map(|r| r.shape)) as Box<dyn Fn() -> noshape_kernel::Result<noshape_kernel::Shape>>),
        ("shell_solid offset", Box::new(|| noshape_kernel::shell_solid(&body, &top, 0.0015, noshape_kernel::ShellSide::Inside, noshape_kernel::ShellMethod::Offset).map(|r| r.shape))),
        ("shell_solid slabs", Box::new(|| noshape_kernel::shell_solid(&body, &top, 0.0015, noshape_kernel::ShellSide::Inside, noshape_kernel::ShellMethod::Slabs).map(|r| r.shape))),
    ] {
        let t0 = std::time::Instant::now();
        let res = f();
        eprintln!("  {name:20}: {:?} ok={} valid={}", t0.elapsed(), res.is_ok(), res.as_ref().map(|s| s.is_valid()).unwrap_or(false));
    }
}

fn status(t: &mut T, id: FeatureId) -> (FeatureStatus, Option<String>, f64) {
    let r = t.regen();
    let s = r.display.features.iter().find(|s| s.id == id).expect("feature state");
    assert!(r.context.bodies[0].shape.is_valid(), "invalid solid ({:?} {:?}, {} mm³)", s.status, s.message, volume(&r));
    // Every face keeps a name derived from its history (no ordinal "New" names).
    let unnamed = r.context.bodies[0].faces.iter().filter(|n| format!("{n:?}").contains("role: New(")).count();
    assert_eq!(unnamed, 0, "faces without history");
    (s.status, s.message.clone(), volume(&r))
}

/// Blends against Onshape's volumes and messages (live capture F1–F5).
/// `tol` is 1e-3 mm³ where we build Onshape's exact solid; 0.12 for the
/// rolling-ball corners of a fillet over smaller corner fillets (the same
/// face as Onshape's, areas equal to 1e-7, our volume exact; Onshape's mass
/// properties of that procedural face are ~0.1 off). See kernel/tests/blend.rs.
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn blends_match_onshape() {
    fn check(t: &mut T, id: FeatureId, want: Result<f64, &str>, tol: f64, what: &str) {
        let (st, msg, v) = status(t, id);
        match want {
            Ok(w) => {
                assert_eq!(st, FeatureStatus::Ok, "{what}: {msg:?}");
                assert!((v - w).abs() <= tol, "{what}: volume {v} vs Onshape {w}");
            }
            Err(m) => {
                assert_eq!(st, FeatureStatus::Error, "{what}: should fail like Onshape (volume {v})");
                assert_eq!(msg.as_deref(), Some(m), "{what}");
            }
        }
    }
    let top_loop = |i: &noshape_kernel::EdgeInfo| i.start.z > 0.00999 && i.end.z > 0.00999;
    // F1: a top-loop fillet / chamfer over 3 mm vertical corner fillets.
    let mut base = T::new();
    base.block(40.0, 20.0, 10.0);
    let v = base.edges_where(vertical);
    base.fillet(v, "3 mm");
    let first_top = base.edges_where(top_loop)[0].clone();
    for (r, want, tol) in [("1 mm", 7898.39759874477, 1e-3), ("2.9 mm", 7722.808308754831, 1e-3), ("3 mm", 7709.053018116171, 1e-3), ("4 mm", 7545.734690323423, 0.12), ("6 mm", 7094.739677144683, 0.12)] {
        let mut t = base.fork();
        let f = t.fillet(vec![first_top.clone()], r);
        check(&mut t, f, Ok(want), tol, &format!("F1 fillet {r}"));
    }
    for (d, want) in [("1 mm", Ok(7866.365758413508)), ("3 mm", Ok(7434.194671058468)), ("4 mm", Err("Distance of chamfer on face too large.")), ("5 mm", Err("Distance of chamfer on face too large."))] {
        let mut t = base.fork();
        let f = t.chamfer(vec![first_top.clone()], d);
        check(&mut t, f, want, 1e-3, &format!("F1 chamfer {d}"));
    }
    // F2: fillets that consume faces.
    let mut base = T::new();
    base.block(40.0, 20.0, 10.0);
    let all = base.edges_where(|_| true);
    for (r, want, tol) in [("2 mm", Ok(7770.359817271122), 1e-3), ("4.99 mm", Ok(6670.191853561291), 1e-3), ("5 mm", Ok(6665.191429188093), 1e-3), ("6 mm", Err("Failed to fillet selections."), 0.0)] {
        let mut t = base.fork();
        let f = t.add("fillet", &[("entities", q(all.clone())), ("radius", e(r)), ("tangentPropagation", ParamValue::Bool(false))]);
        check(&mut t, f, want, tol, &format!("F2 all edges {r}"));
    }
    let tops = base.edges_where(top_loop);
    // R10: the side faces and the top face are all consumed. Onshape reports
    // 5808.3579; the exact mitred solid is 5808.2602 (numerical integration),
    // which is what we build.
    for (r, want, tol) in [("10 mm", Ok(5808.357871033556), 0.11), ("12 mm", Err("Failed to fillet selections."), 0.0)] {
        let mut t = base.fork();
        let f = t.add("fillet", &[("entities", q(tops.clone())), ("radius", e(r)), ("tangentPropagation", ParamValue::Bool(false))]);
        check(&mut t, f, want, tol, &format!("F2 top edges {r}"));
    }
    // F3: three edges at a corner.
    let at_corner = |i: &noshape_kernel::EdgeInfo| [i.start, i.end].iter().any(|p| (*p - glam::DVec3::new(0.0, 0.0, 0.01)).length() < 1e-9);
    let corner = base.edges_where(at_corner);
    assert_eq!(corner.len(), 3);
    let mut t = base.fork();
    let f = t.fillet(corner.clone(), "3 mm");
    check(&mut t, f, Ok(7869.320758646354), 1e-3, "F3 corner R3");
    let mut t = base.fork();
    let f = t.chamfer(corner.clone(), "2 mm");
    check(&mut t, f, Ok(7865.333333333336), 1e-3, "F3 corner chamfer 2");
    // Sequential with tangent propagation (the default): x edge R2, then the y
    // edge R3 runs on through the R2 fillet's arc and down the vertical edge.
    // (Propagation off: Onshape stops the R3 at the arc, 7928.4328; OCCT always
    // follows tangent chains, open in docs/handoff/kernel-robustness.md.)
    let mut t = base.fork();
    let x = t.edges_where(|i| matches!(i.curve, noshape_kernel::Curve::Line { .. }) && i.start.y.abs() < 1e-9 && i.end.y.abs() < 1e-9 && (i.start.z - 0.01).abs() < 1e-9 && (i.end.z - 0.01).abs() < 1e-9);
    assert_eq!(x.len(), 1);
    let f = t.fillet(x, "2 mm");
    check(&mut t, f, Ok(7965.663706143593), 1e-3, "F3 x R2");
    let y = t.edges_where(|i| matches!(i.curve, noshape_kernel::Curve::Line { .. }) && i.start.x.abs() < 1e-9 && i.end.x.abs() < 1e-9 && (i.start.z - 0.01).abs() < 1e-9 && (i.end.z - 0.01).abs() < 1e-9);
    assert_eq!(y.len(), 1);
    let f = t.fillet(y, "3 mm");
    check(&mut t, f, Ok(7911.19874861377), 0.3, "F3 y R3 (propagated)");
    // Tangent propagation off: x R2, y R3, vertical R4, each stopping at its edge.
    let mut t = base.fork();
    let no_prop = |t: &mut T, edges: Vec<Reference>, r: &str| t.add("fillet", &[("entities", q(edges)), ("radius", e(r)), ("tangentPropagation", ParamValue::Bool(false))]);
    let x = t.edges_where(|i| matches!(i.curve, noshape_kernel::Curve::Line { .. }) && i.start.y.abs() < 1e-9 && i.end.y.abs() < 1e-9 && (i.start.z - 0.01).abs() < 1e-9 && (i.end.z - 0.01).abs() < 1e-9);
    let f = no_prop(&mut t, x, "2 mm");
    check(&mut t, f, Ok(7965.663706143593), 1e-3, "F3 off x R2");
    let y = t.edges_where(|i| matches!(i.curve, noshape_kernel::Curve::Line { .. }) && i.start.x.abs() < 1e-9 && i.end.x.abs() < 1e-9 && (i.start.z - 0.01).abs() < 1e-9 && (i.end.z - 0.01).abs() < 1e-9);
    let f = no_prop(&mut t, y, "3 mm");
    check(&mut t, f, Ok(7928.432765498617), 0.03, "F3 off y R3");
    let z = t.edges_where(|i| matches!(i.curve, noshape_kernel::Curve::Line { .. }) && i.start.x.abs() < 1e-9 && i.end.x.abs() < 1e-9 && i.start.y.abs() < 1e-9 && i.end.y.abs() < 1e-9);
    assert_eq!(z.len(), 1);
    let f = no_prop(&mut t, z, "4 mm");
    check(&mut t, f, Ok(7898.8465239772), 1e-3, "F3 off z R4");
    // F5: the top rim of a Ø10 hole 5 mm from the front.
    let mut base = T::new();
    base.block(40.0, 20.0, 10.0);
    base.holes(&[((20.0, 10.0), 5.0)]);
    let rim = base.edges_where(|i| matches!(i.curve, noshape_kernel::Curve::Circle { .. }) && i.start.z > 0.00999);
    for (r, want, tol) in [("2 mm", 7185.22469002635, 1e-3), ("4.9 mm", 7017.294265239272, 1e-3), ("6 mm", 6907.0716824606425, 3.5)] {
        let mut t = base.fork();
        let f = t.fillet(rim.clone(), r);
        check(&mut t, f, Ok(want), tol, &format!("F5 rim {r}"));
    }
}

/// Clipboard payloads of the QA3 shell cases for checking in the app (paste
/// with Ctrl+V): `cargo test -p noshape-model --test all kernel_robustness::
/// write_kern_clips -- --ignored`, files in the temp dir.
#[test]
#[ignore]
fn write_kern_clips() {
    let dir = std::env::temp_dir();
    let mut t = T::new();
    t.block(40.0, 20.0, 10.0);
    let v = t.edges_where(vertical);
    t.fillet(v, "3 mm");
    let top = t.edges_where(|i| i.start.z > 0.00999 && i.end.z > 0.00999);
    t.chamfer(vec![top[0].clone()], "1 mm");
    let top_face = t.faces_where(|c| c.z > 9.999);
    let s = t.shell(top_face, "2.5 mm");
    t.ok(s);
    std::fs::write(dir.join("noshape-clip-shell-b.json"), serde_json::json!({ "noshape_features": t.ps.features }).to_string()).unwrap();
    let mut t = T::new();
    t.block(100.0, 40.0, 12.0);
    t.holes(&[((50.0, 8.0), 5.0)]);
    let top = t.faces_where(|c| c.z > 11.999);
    let s = t.shell(top, "2.5 mm");
    t.ok(s);
    std::fs::write(dir.join("noshape-clip-shell-a.json"), serde_json::json!({ "noshape_features": t.ps.features }).to_string()).unwrap();
    // F1 R4: the top loop over the R3 corners (rolling-ball corner faces).
    let mut t = T::new();
    t.block(40.0, 20.0, 10.0);
    let v = t.edges_where(vertical);
    t.fillet(v, "3 mm");
    let top = t.edges_where(|i| i.start.z > 0.00999 && i.end.z > 0.00999);
    let f = t.fillet(top, "4 mm");
    t.ok(f);
    std::fs::write(dir.join("noshape-clip-f1-r4.json"), serde_json::json!({ "noshape_features": t.ps.features }).to_string()).unwrap();
}

/// F4: variable fillet of one top 40 mm edge, R1 at the x = 0 vertex and R4 at
/// x = 40, then points along the edge (Onshape F4, G3: within 0.07 mm³ with
/// Onshape's radius law, G6).
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn variable_fillet_like_onshape() {
    use noshape_model::params::ParamValues;
    let mut t = T::new();
    t.block(40.0, 20.0, 10.0);
    let r = t.regen();
    let b = &r.context.bodies[0];
    let edge = b.shape.edges().iter().position(|x| {
        let i = x.edge_info().unwrap();
        i.start.z > 0.00999 && i.end.z > 0.00999 && i.start.y.abs() < 1e-9 && i.end.y.abs() < 1e-9
    });
    let edge = Reference::Edge((*b.edges[edge.unwrap()]).clone());
    let vertex = |p: glam::DVec3| {
        let i = b.shape.vertices().iter().position(|v| (v.vertex_point().unwrap() - p).length() < 1e-9).unwrap();
        Reference::Vertex((*b.vertices[i]).clone())
    };
    let item = |pairs: Vec<(&str, ParamValue)>| {
        let mut v = ParamValues::default();
        for (k, x) in pairs {
            v.set(k, x);
        }
        v
    };
    let vs = ParamValue::Array(vec![
        item(vec![("vertex", q(vec![vertex(glam::DVec3::new(0.0, 0.0, 0.01))])), ("vertexRadius", e("1 mm"))]),
        item(vec![("vertex", q(vec![vertex(glam::DVec3::new(0.04, 0.0, 0.01))])), ("vertexRadius", e("4 mm"))]),
    ]);
    let f = t.add("fillet", &[("entities", q(vec![edge.clone()])), ("radius", e("2 mm")), ("isVariable", ParamValue::Bool(true)), ("vertexSettings", vs)]);
    let r = t.ok(f);
    assert!(r.context.bodies[0].shape.is_valid());
    assert!((volume(&r) - 7939.887550821069).abs() < 0.1, "{}", volume(&r));
    // Points along the edge, smooth transition off / on (Onshape F4, G3).
    for (points, smooth, want) in [
        (vec![(0.5, 6.0)], false, 7796.852904267579),
        (vec![(0.5, 6.0)], true, 7825.015982885247),
        (vec![(0.25, 6.0), (0.75, 2.0)], false, 7864.39189706457),
        (vec![(0.25, 6.0), (0.75, 2.0)], true, 7869.806994229301),
        (vec![(0.25, 3.0)], false, 7909.821125219037),
        (vec![(0.5, 2.0)], false, 7951.8349676353555),
        (vec![(0.5, 3.0)], false, 7926.857272261789),
        (vec![(0.25, 2.0), (0.75, 3.0)], false, 7941.61719614589),
    ] {
        let pts = points.iter().map(|(t, r)| item(vec![("edge", q(vec![edge.clone()])), ("edgeParameter", e(&t.to_string())), ("pointOnEdgeRadius", e(&format!("{r} mm")))])).collect();
        t.set(f, "pointOnEdgeSettings", ParamValue::Array(pts));
        t.set(f, "smoothTransition", ParamValue::Bool(smooth));
        let r = t.ok(f);
        assert!(r.context.bodies[0].shape.is_valid());
        assert!((volume(&r) - want).abs() < 0.15, "{points:?} smooth {smooth}: {} vs {want}", volume(&r));
    }
}

/// Move face (Offset) where the moved face changes the topology (live capture
/// G1): through a hole, closing a hole, a hole grown until it splits the part.
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn move_face_like_onshape() {
    let mut base = T::new();
    base.block(40.0, 20.0, 10.0);
    base.holes(&[((20.0, 10.0), 5.0)]);
    let mv = |t: &mut T, faces: Vec<Reference>, d: &str, flip: bool| {
        t.add("moveFace", &[("moveFaces", q(faces)), ("moveFaceType", en("OFFSET")), ("offsetDistance", e(d)), ("oppositeDirection", ParamValue::Bool(flip))])
    };
    // O1: the x = 40 face 20 mm into the part, through the hole's centre.
    let mut t = base.fork();
    let f = t.faces_where(|c| c.x > 39.999);
    let id = mv(&mut t, f, "20 mm", true);
    let (st, msg, v) = status(&mut t, id);
    assert_eq!(st, FeatureStatus::Ok, "{msg:?}");
    assert!((v - 3607.3009183012764).abs() < 1e-3, "O1 {v}");
    // O4: the hole face 5 mm toward its axis (the hole closes), then away (r = 10).
    let hole = base.faces_where(|c| (c.z - 5.0).abs() < 1e-6 && (c.x - 20.0).abs() < 1e-6 && (c.y - 10.0).abs() < 1e-6);
    assert_eq!(hole.len(), 1, "hole face");
    for (flip, want) in [(false, 8000.000000000001), (true, 2429.203673205104)] {
        let mut t = base.fork();
        let id = mv(&mut t, hole.clone(), "5 mm", flip);
        let (st, msg, v) = status(&mut t, id);
        assert_eq!(st, FeatureStatus::Ok, "{msg:?}");
        assert!((v - want).abs() < 1e-3, "O4 flip={flip}: {v}");
        assert_eq!(t.regen().context.bodies.len(), 1, "one part");
    }
}

/// G1 SW: a Ø6 circle swept along a line and a 90° bend. Onshape: bend R6 and R3
/// (= the profile radius) OK, R2 fails "Could not create valid swept body, check input."
/// (OCCT builds a self-intersecting solid).
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn sweep_round_tight_bend() {
    for (rb, want) in [(6.0, Ok(1114.7093352986567)), (3.0, Ok(981.4696758839505)), (2.0, Err("Could not create valid swept body, check input."))] {
        let mut t = T::new();
        let mut prof = noshape_sketch::Sketch::new();
        prof.add_circle(DVec2::ZERO, 0.003);
        let pf = t.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Front)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: prof, ..Default::default() })))]);
        let mut path = noshape_sketch::Sketch::new();
        let l = path.add_line(DVec2::new(0.0, 0.0), DVec2::new(0.0, 0.03));
        let a = path.add_arc(DVec2::new(rb * 1e-3, 0.03), DVec2::new(rb * 1e-3, 0.03 + rb * 1e-3), DVec2::new(0.0, 0.03));
        let pp = t.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Top)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: path, ..Default::default() })))]);
        let r = t.regen();
        let key = r.display.sketches.iter().find(|x| x.feature == pf).unwrap().regions[0].key.clone();
        let sw = t.add("sweep", &[("profiles", q(vec![Reference::SketchRegion { sketch: pf, key }])), ("path", q(vec![Reference::SketchEntity { sketch: pp, entity: l.0 }, Reference::SketchEntity { sketch: pp, entity: a.0 }]))]);
        let r = t.regen();
        let s = r.display.features.iter().find(|s| s.id == sw).unwrap();
        match want {
            Ok(v) => {
                assert_eq!(s.status, FeatureStatus::Ok, "R{rb}: {:?}", s.message);
                assert!((volume(&r) - v).abs() < 1e-2, "R{rb}: {}", volume(&r));
            }
            Err(m) => {
                assert_eq!(s.status, FeatureStatus::Error, "R{rb} should fail");
                assert_eq!(s.message.as_deref(), Some(m));
            }
        }
    }
}

/// Draft over fillets (live capture G1): D1 sides + R3 corner fillets drafted
/// 5°/30° both ways (the drafted fillets vanish at 30° narrowing: OCCT's draft
/// builds an invalid solid, the boolean construction takes over); D3 with a
/// top R2 fillet loop, Reapply fillet off (the top fillets are rolled again
/// against the drafted faces) and on (all fillets removed and reapplied).
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn draft_like_onshape() {
    let mut base = T::new();
    base.block(40.0, 20.0, 10.0);
    let v = base.edges_where(vertical);
    base.fillet(v, "3 mm");
    let side = |t: &mut T| t.faces_where(|c| c.x.abs() < 1e-6 && c.z > 1.0 && c.z < 9.0);
    let draft = |t: &mut T, ang: &str, flip: bool, refillet: bool| {
        let bottom = t.faces_where(|c| c.z.abs() < 1e-6);
        let s = side(t);
        t.add("draft", &[("neutralPlane", q(bottom)), ("draftFaces", q(s)), ("angle", e(ang)), ("pullDirection", ParamValue::Bool(flip)), ("tangentPropagation", ParamValue::Bool(true)), ("reFillet", ParamValue::Bool(refillet))])
    };
    // Our pull: flip = false widens upward (Onshape's default), true narrows.
    for (ang, flip, want) in [("5 deg", true, 7428.357159985565), ("30 deg", true, 4966.961583068534), ("5 deg", false, 8433.160575397696), ("30 deg", false, 11587.230290602416)] {
        let mut t = base.fork();
        let d = draft(&mut t, ang, flip, false);
        let (st, msg, v) = status(&mut t, d);
        assert_eq!(st, FeatureStatus::Ok, "D1 {ang} {flip}: {msg:?}");
        assert!((v - want).abs() < 1e-3, "D1 {ang} narrowing={flip}: {v} vs {want}");
    }
    let mut top = base.fork();
    let tl = top.edges_where(|i| i.start.z > 0.00999 && i.end.z > 0.00999);
    top.fillet(vec![tl[0].clone()], "2 mm");
    for (refillet, want, tol) in [(false, 7353.619385075508, 1e-3), (true, 7335.592764597912, 0.05)] {
        let mut t = top.fork();
        let d = draft(&mut t, "5 deg", true, refillet);
        let (st, msg, v) = status(&mut t, d);
        assert_eq!(st, FeatureStatus::Ok, "D3 reapply={refillet}: {msg:?}");
        assert!((v - want).abs() < tol, "D3 reapply={refillet}: {v} vs {want}");
    }
}

/// Fillet and chamfer options against Onshape (live capture G4). Conic
/// sections are within 0.15 mm³ (Parasolid approximates them by splines; the
/// exact conic area differs by ~0.003 mm² either way).
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn fillet_options_like_onshape() {
    let mut base = T::new();
    base.block(40.0, 20.0, 10.0);
    let edge_t = base.edges_where(|i| matches!(i.curve, noshape_kernel::Curve::Line { .. }) && i.start.y.abs() < 1e-9 && i.end.y.abs() < 1e-9 && (i.start.z - 0.01).abs() < 1e-9 && (i.end.z - 0.01).abs() < 1e-9);
    assert_eq!(edge_t.len(), 1);
    let run = |extra: &[(&str, ParamValue)], want: f64, tol: f64, what: &str| {
        let mut t = base.fork();
        let mut params: Vec<(&str, ParamValue)> = vec![("entities", q(edge_t.clone())), ("tangentPropagation", ParamValue::Bool(false))];
        params.extend(extra.iter().cloned());
        let f = t.add("fillet", &params);
        let (st, msg, v) = status(&mut t, f);
        assert_eq!(st, FeatureStatus::Ok, "{what}: {msg:?}");
        assert!((v - want).abs() < tol, "{what}: {v} vs Onshape {want}");
    };
    run(&[("radius", e("3 mm"))], 7922.743338823081, 1e-3, "R3");
    run(&[("blendControlType", en("WIDTH")), ("width", e("4 mm"))], 7931.3274122871835, 1e-3, "width 4");
    for (rho, want) in [("0.3", 7897.558788517183), ("0.5", 7939.866061737167), ("0.8", 7986.350483605667)] {
        run(&[("crossSection", en("CONIC")), ("nonCircularRadius", e("3 mm")), ("rho", e(rho))], want, 0.15, &format!("conic {rho}"));
    }
    run(&[("blendControlType", en("WIDTH")), ("width", e("4 mm")), ("crossSection", en("CONIC")), ("rho", e("0.5"))], 7946.539131376041, 0.15, "width 4 conic 0.5");
    // Curvature section: the exact volumes of live's own profiles (G11 control
    // points); live's mass properties report estimates 0.04-0.05 off them
    // (G4: 7874.4348 / 7941.1552 / 7987.6976).
    for (m, want) in [("0.2", 7874.382222), ("0.5", 7941.198413), ("0.8", 7987.732794)] {
        run(&[("crossSection", en("CURVATURE")), ("nonCircularRadius", e("3 mm")), ("magnitude", e(m))], want, 2e-3, &format!("curvature {m}"));
    }
    for (flip, want) in [(false, 7871.325220739209), (true, 7871.305338882543)] {
        run(&[("radius", e("3 mm")), ("isAsymmetric", ParamValue::Bool(true)), ("otherRadius", e("5 mm")), ("flipAsymmetric", ParamValue::Bool(flip))], want, 0.1, &format!("asymmetric flip={flip}"));
    }
    let partial = |extra: Vec<(&'static str, ParamValue)>| {
        let mut p = vec![("radius", e("3 mm")), ("isPartial", ParamValue::Bool(true)), ("startPartialType", en("PERCENTAGE")), ("partialFirstEdgeTotalParameter", e("0.25"))];
        p.extend(extra);
        p
    };
    run(&partial(vec![("partialOppositeParameter", ParamValue::Bool(true))]), 7942.057504117312, 1e-3, "partial 0.25 → 1");
    run(&partial(vec![("partialOppositeParameter", ParamValue::Bool(false))]), 7980.685834705772, 1e-3, "partial 0 → 0.25");
    run(&partial(vec![("secondBound", ParamValue::Bool(true)), ("endPartialType", en("PERCENTAGE")), ("partialSecondEdgeTotalParameter", e("0.75"))]), 7961.371669411541, 1e-3, "partial 0.25 → 0.75");
    // Chamfer options.
    let chamfer = |extra: &[(&str, ParamValue)], want: f64, what: &str| {
        let mut t = base.fork();
        let mut params: Vec<(&str, ParamValue)> = vec![("entities", q(edge_t.clone())), ("tangentPropagation", ParamValue::Bool(false))];
        params.extend(extra.iter().cloned());
        let f = t.add("chamfer", &params);
        let (st, msg, v) = status(&mut t, f);
        assert_eq!(st, FeatureStatus::Ok, "{what}: {msg:?}");
        assert!((v - want).abs() < 1e-3, "{what}: {v} vs Onshape {want}");
    };
    for m in ["FACE_OFFSET", "APEX_RANGE"] {
        chamfer(&[("chamferMethod", en(m)), ("width", e("2 mm"))], 7920.0, &format!("{m} equal 2"));
        for flip in [false, true] {
            chamfer(&[("chamferMethod", en(m)), ("chamferType", en("TWO_OFFSETS")), ("width1", e("2 mm")), ("width2", e("4 mm")), ("oppositeDirection", ParamValue::Bool(flip))], 7840.0, &format!("{m} two distances {flip}"));
            chamfer(&[("chamferMethod", en(m)), ("chamferType", en("OFFSET_ANGLE")), ("width", e("2 mm")), ("angle", e("30 deg")), ("oppositeDirection", ParamValue::Bool(flip))], 7953.8119784648325, &format!("{m} distance angle {flip}"));
        }
    }
    // Width on a 120° edge = the radius (chord = R there).
    let mut t = T::new();
    let hex: Vec<(f64, f64)> = (0..6).map(|i| {
        let a = std::f64::consts::PI / 3.0 * i as f64;
        (20.0 * a.cos(), 20.0 * a.sin())
    }).collect();
    let (_, regions) = t.sketch(&[&hex], &[]);
    t.add("extrude", &[("entities", q(regions)), ("depth", e("10 mm")), ("operationType", en("NEW"))]);
    let v = t.edges_where(vertical);
    let f = t.add("fillet", &[("entities", q(vec![v[0].clone()])), ("blendControlType", en("WIDTH")), ("width", e("4 mm"))]);
    let (st, msg, vol) = status(&mut t, f);
    assert_eq!(st, FeatureStatus::Ok, "{msg:?}");
    assert!((vol - 10383.704606438652).abs() < 1e-3, "hexagon width 4: {vol}");
}

/// Surface revolve (Onshape: Revolve, Surface): a line revolved about a
/// parallel axis is a cylinder surface; partial angles give a part of it.
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn surface_revolve() {
    let mut t = T::new();
    let mut sk = noshape_sketch::Sketch::new();
    let l = sk.add_line(DVec2::new(0.01, 0.0), DVec2::new(0.01, 0.02));
    let ax = sk.add_line(DVec2::new(0.0, 0.0), DVec2::new(0.0, 0.02));
    let s = t.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Front)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
    let r = t.add(
        "revolve",
        &[("bodyType", en("SURFACE")), ("surfaceEntities", q(vec![Reference::SketchEntity { sketch: s, entity: l.0 }])), ("axis", q(vec![Reference::SketchEntity { sketch: s, entity: ax.0 }])), ("fullRevolve", ParamValue::Bool(true))],
    );
    let res = t.ok(r);
    let b = res.context.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Surface).expect("surface body");
    let area = noshape_kernel::surface_properties(&b.shape).unwrap().mass / 1e-6;
    assert!((area - 2.0 * std::f64::consts::PI * 10.0 * 20.0).abs() < 1e-3, "{area}");
    t.set(r, "fullRevolve", ParamValue::Bool(false));
    t.set(r, "angle", e("90 deg"));
    let res = t.ok(r);
    let b = res.context.bodies.iter().find(|b| b.kind == noshape_model::display::BodyKind::Surface).expect("surface body");
    let area = noshape_kernel::surface_properties(&b.shape).unwrap().mass / 1e-6;
    assert!((area - 0.5 * std::f64::consts::PI * 10.0 * 20.0).abs() < 1e-3, "{area}");
}

/// Sweep twist: a 10 x 4 rectangle swept 50 mm along a straight path with half a
/// turn keeps its volume (Pappus) and ends turned by 180°; on an arc path too.
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn sweep_twist() {
    for (arc, turns) in [(false, "0.5"), (false, "1"), (true, "0.25")] {
        let mut t = T::new();
        let mut prof = noshape_sketch::Sketch::new();
        let p = [DVec2::new(-0.005, -0.002), DVec2::new(0.005, -0.002), DVec2::new(0.005, 0.002), DVec2::new(-0.005, 0.002)];
        let ids: Vec<_> = p.iter().map(|q| prof.add_point(*q)).collect();
        for i in 0..4 {
            prof.add_line_between(ids[i], ids[(i + 1) % 4]).unwrap();
        }
        let pf = t.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Front)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: prof, ..Default::default() })))]);
        let mut path = noshape_sketch::Sketch::new();
        let mut ents = vec![path.add_line(DVec2::new(0.0, 0.0), DVec2::new(0.0, 0.05))];
        if arc {
            ents.push(path.add_arc(DVec2::new(0.04, 0.05), DVec2::new(0.04, 0.09), DVec2::new(0.0, 0.05)));
        }
        let pp = t.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Top)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: path, ..Default::default() })))]);
        let r = t.regen();
        let key = r.display.sketches.iter().find(|x| x.feature == pf).unwrap().regions[0].key.clone();
        let sw = t.add(
            "sweep",
            &[
                ("profiles", q(vec![Reference::SketchRegion { sketch: pf, key }])),
                ("path", q(ents.iter().map(|e| Reference::SketchEntity { sketch: pp, entity: e.0 }).collect())),
                ("hasTwist", ParamValue::Bool(true)),
                ("twistType", en("TURNS")),
                ("turns", e(turns)),
            ],
        );
        let (st, msg, v) = status(&mut t, sw);
        assert_eq!(st, FeatureStatus::Ok, "arc={arc} {turns}: {msg:?}");
        let len = 50.0 + if arc { std::f64::consts::FRAC_PI_2 * 40.0 } else { 0.0 };
        assert!((v - 40.0 * len).abs() < 0.05, "arc={arc} turns {turns}: {v} vs {}", 40.0 * len);
        if !arc && turns == "0.5" {
            // Onshape G9: a quarter turn (default direction) takes the corner
            // (+5, 0, +2) to (-2, 50, +5) through (2.1213, 25, 4.9497), a -90°
            // turn about the path; "opposite direction" goes through (4.9497, 25, -2.1213).
            for (ccw, mid) in [(false, glam::DVec3::new(2.1213, 25.0, 4.9497)), (true, glam::DVec3::new(4.9497, 25.0, -2.1213))] {
                t.set(sw, "turns", e("0.25"));
                t.set(sw, "ccw", ParamValue::Bool(ccw));
                let r = t.ok(sw);
                let b = &r.context.bodies[0].shape;
                assert!((volume(&r) - 1999.9877116530715).abs() < 0.05, "{}", volume(&r));
                let corner = glam::DVec3::new(5.0, 0.0, 2.0);
                let lateral = b
                    .edges()
                    .into_iter()
                    .find(|x| x.edge_info().is_ok_and(|i| [i.start, i.end].iter().any(|s| (*s * 1e3 - corner).length() < 1e-6) && i.length * 1e3 > 49.0))
                    .expect("lateral edge from the corner");
                let i = lateral.edge_info().unwrap();
                let m = lateral.point_at(0.5 * (i.first_param + i.last_param)).unwrap() * 1e3;
                assert!((m - mid).length() < 0.05, "ccw={ccw}: mid {m} vs {mid}");
            }
        }
    }
}

/// Onshape G7: tangent propagation off next to corner fillets. A straight
/// edge's blend runs out onto the corner cylinders; an arc on its own (or an
/// arc with its neighbour) is refused with "Adjoining edge not filleted.".
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn propagation_off_next_to_corner_fillets() {
    let mut t = T::new();
    t.block(40.0, 20.0, 10.0);
    let v = t.edges_where(vertical);
    t.fillet(v, "3 mm");
    assert!((volume(&t.regen()) - 7922.743338823081).abs() < 1e-6);
    let top = |i: &noshape_kernel::EdgeInfo| i.start.z > 0.00999 && i.end.z > 0.00999;
    let front = t.edges_where(|i| top(i) && i.start.y.abs() < 1e-9 && i.end.y.abs() < 1e-9);
    let arc = t.edges_where(|i| top(i) && matches!(i.curve, noshape_kernel::Curve::Circle { .. }) && i.start.x.min(i.end.x) < 1e-9 && i.start.y.min(i.end.y) < 1e-9);
    assert_eq!((front.len(), arc.len()), (1, 1));
    let base = t.fork();
    for (edges, chamfer, want) in [(front.clone(), false, Ok(7891.145024027843)), (arc.clone(), false, Err("Adjoining edge not filleted.")), ([arc.clone(), front.clone()].concat(), false, Err("Adjoining edge not filleted.")), (front.clone(), true, Ok(7904.4851916352945))] {
        let mut t = base.fork();
        let f = if chamfer { t.chamfer(edges, "1 mm") } else { t.fillet(edges, "2 mm") };
        t.set(f, "tangentPropagation", ParamValue::Bool(false));
        let (st, msg, v) = status(&mut t, f);
        match want {
            Ok(w) => {
                assert_eq!(st, FeatureStatus::Ok, "{msg:?}");
                // The fillet's runout ends are the blend's prism carried on until it
                // leaves the part (exactly 7891.158004). Onshape's is the same solid
                // (G11e: the R2 cylinder runs on to the cylinder-cylinder seams
                // (3,0,8)-(0.1716,2,10)); its mass properties are approximate there.
                let tol = if chamfer { 1e-3 } else { 0.02 };
                assert!((v - w).abs() < tol, "chamfer={chamfer}: {v} vs {w}");
            }
            Err(m) => {
                assert_eq!(st, FeatureStatus::Error);
                assert_eq!(msg.as_deref(), Some(m));
            }
        }
    }
}

/// Onshape G10: lofts between unlike profiles (defaults: no end conditions,
/// match connections off), volumes in mm³. Through three sections the loft
/// follows a natural cubic spline (crates/kernel/cpp/features.cpp natural_loft).
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn loft_like_onshape() {
    use noshape_model::params::ParamValues;
    use std::f64::consts::{PI, TAU};
    // Sketch at height z (mm) on Top / a plane offset from Top: closed polylines and circles (mm).
    fn profile(t: &mut T, z: f64, poly: &[(f64, f64)], circle: Option<f64>, point: bool) -> Reference {
        let plane = if z == 0.0 {
            Reference::Plane(DatumPlane::Top)
        } else {
            Reference::FeaturePlane(t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Top)])), ("cplaneType", en("OFFSET")), ("offset", e(&format!("{z} mm")))]))
        };
        let mut sk = noshape_sketch::Sketch::new();
        let pts: Vec<_> = poly.iter().map(|(x, y)| sk.add_point(DVec2::new(x * 1e-3, y * 1e-3))).collect();
        for i in 0..pts.len() {
            sk.add_line_between(pts[i], pts[(i + 1) % pts.len()]).unwrap();
        }
        if let Some(r) = circle {
            sk.add_circle(DVec2::ZERO, r * 1e-3);
        }
        let p = point.then(|| sk.add_point(DVec2::ZERO));
        let s = t.add("sketch", &[("sketchPlane", q(vec![plane])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
        if let Some(p) = p {
            return Reference::SketchEntity { sketch: s, entity: p.0 };
        }
        let r = t.regen();
        let key = r.display.sketches.iter().find(|x| x.feature == s).unwrap().regions[0].key.clone();
        Reference::SketchRegion { sketch: s, key }
    }
    let square = |a: f64| vec![(-a / 2.0, -a / 2.0), (a / 2.0, -a / 2.0), (a / 2.0, a / 2.0), (-a / 2.0, a / 2.0)];
    let star: Vec<(f64, f64)> = (0..20).map(|i| {
        let (r, a) = (if i % 2 == 0 { 10.0 } else { 4.0 }, i as f64 * TAU / 20.0);
        (r * a.cos(), r * a.sin())
    }).collect();
    type P = (f64, Vec<(f64, f64)>, Option<f64>, bool);
    let cases: Vec<(&str, Vec<P>, f64)> = vec![
        ("square → circle", vec![(0.0, square(20.0), None, false), (20.0, vec![], Some(8.0), false)], 5927.728022414836),
        ("star → circle", vec![(0.0, star, None, false), (20.0, vec![], Some(5.0), false)], 2074.672136114528),
        ("triangle → square", vec![(0.0, vec![(0.0, -10.0), (10.0, 8.0), (-10.0, 8.0)], None, false), (20.0, square(16.0), None, false)], 3920.0000000000005),
        ("circle → point", vec![(0.0, vec![], Some(10.0), false), (20.0, vec![], None, true)], PI * 100.0 * 20.0 / 3.0),
        ("circle → square → circle", vec![(0.0, vec![], Some(10.0), false), (15.0, square(12.0), None, false), (30.0, vec![], Some(10.0), false)], 6125.606828000875),
    ];
    for (name, profiles, want) in cases {
        let mut t = T::new();
        let items: Vec<ParamValues> = profiles
            .iter()
            .map(|(z, poly, c, pt)| {
                let r = profile(&mut t, *z, poly, *c, *pt);
                let mut v = ParamValues::default();
                v.set("sheetProfileEntities", q(vec![r]));
                v
            })
            .collect();
        let l = t.add("loft", &[("sheetProfilesArray", ParamValue::Array(items))]);
        let (st, msg, v) = status(&mut t, l);
        assert_eq!(st, FeatureStatus::Ok, "{name}: {msg:?}");
        // Triangle → square: Onshape splits the triangle's top edge at its middle
        // (arc-length matching, G12) and joins the four vertex pairs by ruled faces.
        // Parasolid's surface fit leaves 0.03 mm³ against the exact ruled solid (a).
        assert!((v - want).abs() < 0.03, "{name}: {v} vs {want}");
    }
}

/// Chamfer "Two distances": the face Distance 1 is measured on, as Onshape
/// picks it (live G30, rule from G37's entity order): the edge's first face in
/// Onshape's order (front, bottom, top, right, back, left for this block), the
/// chain's first edge deciding along connected edges, an override flipping its
/// chain. Checked by the planar faces' areas (where the 1 mm and 3 mm legs
/// fall) and Onshape's volumes.
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn chamfer_distance_one_face_matches_onshape() {
    let mut base = T::new();
    base.block(40.0, 20.0, 10.0);
    // Planar face areas (mm²) of the result, by where their centroid lies.
    let areas = |r: &RegenResult| -> [f64; 5] {
        let mut out = [0.0; 5];
        for f in r.context.bodies[0].shape.faces() {
            let c = f.centroid().unwrap() * 1e3;
            let a = f.area().unwrap() / 1e-6;
            let near = |x: f64, v: f64| (x - v).abs() < 1e-6;
            for (k, on) in [near(c.z, 10.0), near(c.y, 0.0), near(c.y, 20.0), near(c.x, 0.0), near(c.x, 40.0)].into_iter().enumerate() {
                if on {
                    out[k] += a;
                }
            }
        }
        out // top, front, back, left, right
    };
    let top_edge = |y: Option<f64>, x: Option<f64>| {
        move |i: &noshape_kernel::EdgeInfo| {
            let m = (i.start + i.end) * 0.5e3;
            (m.z - 10.0).abs() < 1e-6 && y.is_none_or(|y| (m.y - y).abs() < 1e-6) && x.is_none_or(|x| (m.x - x).abs() < 1e-6)
        }
    };
    let vertical_at = |x: f64, y: f64| {
        move |i: &noshape_kernel::EdgeInfo| {
            let m = (i.start + i.end) * 0.5e3;
            vertical(i) && (m.x - x).abs() < 1e-6 && (m.y - y).abs() < 1e-6
        }
    };
    let run = |edges: Vec<Reference>, overrides: Vec<Reference>, opposite: bool| -> (f64, [f64; 5]) {
        let mut t = base.fork();
        let f = t.add(
            "chamfer",
            &[
                ("entities", q(edges)),
                ("chamferType", en("TWO_OFFSETS")),
                ("width1", e("1 mm")),
                ("width2", e("3 mm")),
                ("tangentPropagation", ParamValue::Bool(false)),
                ("oppositeDirection", ParamValue::Bool(opposite)),
                ("directionOverrides", q(overrides)),
            ],
        );
        let (st, msg, v) = status(&mut t, f);
        assert_eq!(st, FeatureStatus::Ok, "{msg:?}");
        (v, areas(&t.ok(f)))
    };
    let mut t = base.fork();
    let front = t.edges_where(top_edge(Some(0.0), None));
    let back = t.edges_where(top_edge(Some(20.0), None));
    let left = t.edges_where(top_edge(None, Some(0.0)));
    let all_top = t.edges_where(top_edge(None, None));
    let v00 = t.edges_where(vertical_at(0.0, 0.0));
    let v4020 = t.edges_where(vertical_at(40.0, 20.0));
    let close = |a: [f64; 5], want: &[(usize, f64)], what: &str| {
        for &(k, w) in want {
            assert!((a[k] - w).abs() < 1e-6, "{what}: face {k} area {} vs {w}", a[k]);
        }
    };
    // (a) top front edge: 1 mm on the front face.
    close(run(front.clone(), vec![], false).1, &[(0, 680.0), (1, 360.0)], "(a)");
    // (b), (c): 1 mm on the top face.
    close(run(back.clone(), vec![], false).1, &[(0, 760.0), (2, 280.0)], "(b)");
    close(run(left, vec![], false).1, &[(0, 780.0), (3, 140.0)], "(c)");
    // (d) vertical edge at (0,0): front face; (e) at (40,20): right face.
    close(run(v00, vec![], false).1, &[(1, 390.0), (3, 170.0)], "(d)");
    close(run(v4020, vec![], false).1, &[(4, 190.0), (2, 370.0)], "(e)");
    // (f) the four top edges: the side faces (the front edge decides);
    // (g) overriding the front edge flips the whole loop onto the top.
    let (v, a) = run(all_top.clone(), vec![], false);
    assert!((v - 7832.000000000001).abs() < 1e-6, "(f) {v}");
    close(a, &[(0, 34.0 * 14.0)], "(f)");
    let (v, a) = run(all_top, front.clone(), false);
    assert!((v - 7824.000000000002).abs() < 1e-6, "(g) {v}");
    close(a, &[(0, 38.0 * 18.0)], "(g)");
    // (f2)/(g2): separate edges keep their own sides.
    let both = [front.clone(), back].concat();
    close(run(both.clone(), vec![], false).1, &[(0, 40.0 * 16.0)], "(f2)");
    close(run(both, front.clone(), false).1, &[(0, 40.0 * 18.0)], "(g2)");
    // (h) = (a) + Opposite direction: 1 mm on the top face.
    close(run(front, vec![], true).1, &[(0, 760.0), (1, 280.0)], "(h)");
    // (i) the rim of a Ø10 through hole: 1 mm on the top face.
    let mut t = base.fork();
    t.holes(&[((20.0, 10.0), 5.0)]);
    let rim = t.edges_where(|i| matches!(i.curve, noshape_kernel::Curve::Circle { .. }) && (i.start.z * 1e3 - 10.0).abs() < 1e-6);
    let f = t.add(
        "chamfer",
        &[("entities", q(rim)), ("chamferType", en("TWO_OFFSETS")), ("width1", e("1 mm")), ("width2", e("3 mm")), ("tangentPropagation", ParamValue::Bool(false))],
    );
    let (st, msg, v) = status(&mut t, f);
    assert_eq!(st, FeatureStatus::Ok, "{msg:?}");
    assert!((v - 7164.336354145117).abs() < 1e-6, "(i) {v}");
}

/// Live G46 (qa4 F8/F9): a 50 × 30 × 30 block, R3 fillets on the two top
/// edges meeting at (50, 0, 30), a chamfer on the vertical edge between them,
/// then Shell with the top removed; and the same with the chamfer first.
/// Onshape's mass properties are approximate where fillets meet (G49: its
/// own variance ±2.9 mm³; the exact mitred fillet is 44848.0752 against its
/// 44848.2156), so volumes there are checked to 0.3 mm³; the chamfer's blend
/// patch (G49) is checked against Onshape's faces in the kernel probe.
struct G46 {
    bad: Vec<String>,
}

impl G46 {
    fn face_count(t: &mut T) -> usize {
        t.regen().context.bodies[0].shape.count(noshape_kernel::ShapeType::Face)
    }
    // Each face: surface kind, area (mm²) and box (mm), for comparing with
    // Onshape's bodydetails.
    fn faces(t: &mut T) -> String {
        let r = t.regen();
        r.context.bodies[0]
            .shape
            .faces()
            .iter()
            .map(|f| {
                let kind = f.face_surface().map(|s| format!("{s:?}").split(['(', ' ', '{']).next().unwrap_or("?").to_string()).unwrap_or_default();
                let b = f.bounding_box(false).map(|b| format!("{:.3?}..{:.3?}", (b.min * 1e3).to_array(), (b.max * 1e3).to_array())).unwrap_or_default();
                format!("    {kind} {:.6} {b}", f.area().unwrap_or(0.0) * 1e6)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    fn check(&mut self, t: &mut T, id: FeatureId, want: Result<(f64, usize), &str>, tol: f64, what: &str) {
        let (st, msg, v) = status(t, id);
        let n = Self::face_count(t);
        let ok = match want {
            Ok((w, faces)) => st == FeatureStatus::Ok && (v - w).abs() <= tol && n == faces,
            Err(m) => st == FeatureStatus::Error && msg.as_deref() == Some(m),
        };
        if !ok {
            self.bad.push(format!("{what}: got {st:?} {msg:?} {v:.6} mm³ {n} faces, want {want:?}\n{}", Self::faces(t)));
        }
    }
    fn top_front(i: &noshape_kernel::EdgeInfo) -> bool {
        i.start.y.abs() < 1e-9 && i.end.y.abs() < 1e-9 && (i.start.z - 0.03).abs() < 1e-9 && (i.end.z - 0.03).abs() < 1e-9
    }
    fn top_right(i: &noshape_kernel::EdgeInfo) -> bool {
        (i.start.x - 0.05).abs() < 1e-9 && (i.end.x - 0.05).abs() < 1e-9 && (i.start.z - 0.03).abs() < 1e-9 && (i.end.z - 0.03).abs() < 1e-9
    }
    fn corner(i: &noshape_kernel::EdgeInfo) -> bool {
        matches!(i.curve, noshape_kernel::Curve::Line { .. }) && vertical(i) && (i.start.x - 0.05).abs() < 1e-9 && i.start.y.abs() < 1e-9
    }
    fn shell(&mut self, t: &T, th: &str, want: Result<(f64, usize), &str>, tol: f64, what: &str) {
        let mut u = t.fork();
        let top = u.faces_where(|c| c.z > 29.999);
        let s = u.shell(top, th);
        self.check(&mut u, s, want, tol, &format!("shell {th} {what}"));
    }
    /// The block with the fillets (and the fillet feature).
    fn filleted(&mut self) -> T {
        let mut t = T::new();
        t.block(50.0, 30.0, 30.0);
        let edges = t.edges_where(|i| Self::top_front(i) || Self::top_right(i));
        assert_eq!(edges.len(), 2);
        let f = t.fillet(edges, "3 mm");
        self.check(&mut t, f, Ok((44848.215635, 8)), 0.15, "fillet R3");
        t
    }
    /// The block with the 5 mm chamfer, then the fillets.
    fn chamfer_first(&mut self) -> T {
        let mut t = T::new();
        t.block(50.0, 30.0, 30.0);
        let c_edge = t.edges_where(Self::corner);
        let c = t.chamfer(c_edge, "5 mm");
        self.check(&mut t, c, Ok((44625.0, 7)), 1e-3, "chamfer 5 first");
        let edges = t.edges_where(|i| Self::top_front(i) || Self::top_right(i));
        assert_eq!(edges.len(), 2);
        let f = t.fillet(edges, "3 mm");
        self.check(&mut t, f, Ok((44487.208465, 9)), 0.01, "fillet R3 after chamfer");
        t
    }
}

/// qa4 F8 / F9 as Onshape does them (live G46).
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn chamfer_and_shell_on_filleted_box_match_onshape() {
    let mut g = G46 { bad: Vec::new() };
    let base = g.filleted();
    let mut probe = base.fork();
    let c_edge = probe.edges_where(G46::corner);
    assert_eq!(c_edge.len(), 1);
    // F8: Onshape can't chamfer 5 mm there either.
    let mut t = base.fork();
    let c = t.chamfer(c_edge.clone(), "5 mm");
    g.check(&mut t, c, Err("Distance of chamfer on face too large."), 0.0, "chamfer 5 after fillet");
    let mut t = base.fork();
    let c = t.chamfer(c_edge.clone(), "2 mm");
    g.check(&mut t, c, Ok((44789.850213, 10)), 0.3, "chamfer 2 after fillet");
    // F9.
    g.shell(&t, "2.5 mm", Ok((13946.594771, 21)), 0.3, "after fillet + chamfer 2");
    // Fillet after the chamfer (F8, reordered).
    g.chamfer_first();
    assert!(g.bad.is_empty(), "{}", g.bad.join("\n"));
}

/// Live G50 / G51: the chamfer-then-fillet box shelled: one flat rim
/// triangle (20 faces), and Onshape's failure band from 2.053 mm up to the
/// fillet radius (2.05 works, 3 works again).
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn chamfer_then_fillet_shell_matches_onshape() {
    let mut g = G46 { bad: Vec::new() };
    let t = g.chamfer_first();
    g.shell(&t, "1 mm", Ok((5990.519587, 20)), 0.02, "after chamfer + fillet");
    g.shell(&t, "2.05 mm", Ok((11601.0764818, 20)), 0.02, "after chamfer + fillet");
    g.shell(&t, "2.06 mm", Err("Could not shell part with selections."), 0.0, "after chamfer + fillet");
    g.shell(&t, "2.5 mm", Err("Could not shell part with selections."), 0.0, "after chamfer + fillet");
    assert!(g.bad.is_empty(), "{}", g.bad.join("\n"));
}

/// Live G50: Onshape's 1 mm shell after the 2 mm chamfer is 6052.70 mm³ (its
/// error bars ±~3); ours 6053.88: the offset of the blend patch differs
/// (inner patch 3.891 vs 4.189 mm²).
#[test]
#[ignore = "kern: the offset of the chamfer's blend patch (live G50)"]
fn shell_after_chamfer_patch_matches_onshape() {
    let mut g = G46 { bad: Vec::new() };
    let base = g.filleted();
    let mut t = base.fork();
    let c_edge = t.edges_where(G46::corner);
    t.chamfer(c_edge, "2 mm");
    g.shell(&t, "1 mm", Ok((6052.696796, 21)), 0.3, "after fillet + chamfer 2");
    assert!(g.bad.is_empty(), "{}", g.bad.join("\n"));
}

/// Live G50: Shell of the chamfer-then-fillet box works up to 2.05 mm in
/// Onshape and fails from 2.06 mm. Prints our status and faces per
/// thickness: `cargo test -p noshape-model --test all kernel_robustness::
/// shell_thickness_sweep -- --ignored --nocapture`.
#[test]
#[ignore]
fn shell_thickness_sweep() {
    let mut t = T::new();
    t.block(50.0, 30.0, 30.0);
    let corner = t.edges_where(|i| matches!(i.curve, noshape_kernel::Curve::Line { .. }) && vertical(i) && (i.start.x - 0.05).abs() < 1e-9 && i.start.y.abs() < 1e-9);
    t.chamfer(corner, "5 mm");
    let edges = t.edges_where(|i| (i.start.z - 0.03).abs() < 1e-9 && (i.end.z - 0.03).abs() < 1e-9 && ((i.start.y.abs() < 1e-9 && i.end.y.abs() < 1e-9) || ((i.start.x - 0.05).abs() < 1e-9 && (i.end.x - 0.05).abs() < 1e-9)));
    t.fillet(edges, "3 mm");
    for k in 0..=30 {
        let th = 1.0 + 0.05 * k as f64;
        let mut u = t.fork();
        let top = u.faces_where(|c| c.z > 29.999);
        let s = u.shell(top, &format!("{th} mm"));
        let r = u.regen();
        let st = r.display.features.iter().find(|x| x.id == s).unwrap();
        let b = &r.context.bodies[0].shape;
        eprintln!("{th:.2} mm: {:?} {:?} {} faces, {:.6} mm³", st.status, st.message, b.count(noshape_kernel::ShapeType::Face), volume(&r));
    }
}

/// QA3's real bracket against Onshape (live "kern (10-23 09:00) Shell
/// bracket", behaviours.md): 100 x 40 x 12 plate, R5 on the vertical edges at
/// (0,0), (100,0), (100,40), a Ø10 through hole at (50,10); Shell the top
/// face (1), and the same after a 1 mm chamfer round the top's outer loop (2)
/// or round both loops (3). At exactly 2.5 mm the hole's offset wall touches
/// the front wall's: Onshape fails every case there (2.499 and 2.501 work).
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn shell_bracket_matches_onshape() {
    fn bracket() -> T {
        let mut t = T::new();
        t.block(100.0, 40.0, 12.0);
        let at = |x: f64, y: f64| move |i: &noshape_kernel::EdgeInfo| vertical(i) && (i.start.x - x).abs() < 1e-9 && (i.start.y - y).abs() < 1e-9;
        let mut corners = Vec::new();
        for (x, y) in [(0.0, 0.0), (0.1, 0.0), (0.1, 0.04)] {
            corners.extend(t.edges_where(at(x, y)));
        }
        assert_eq!(corners.len(), 3);
        t.fillet(corners, "5 mm");
        t.holes(&[((50.0, 10.0), 5.0)]);
        t
    }
    let front_top = |t: &mut T| t.edges_where(|i| i.start.z > 0.011999 && i.end.z > 0.011999 && matches!(i.curve, noshape_kernel::Curve::Line { .. }) && i.start.y.abs() < 1e-9 && i.end.y.abs() < 1e-9);
    let rim = |t: &mut T| t.edges_where(|i| i.start.z > 0.011999 && matches!(i.curve, noshape_kernel::Curve::Circle { radius, .. } if (radius - 0.005).abs() < 1e-9));
    let faces = |r: &RegenResult| r.context.bodies[0].shape.count(noshape_kernel::ShapeType::Face);
    // Onshape: the input (volume, faces), then per thickness Ok((volume, faces)) or its error.
    type Want = Result<(f64, Option<usize>), &'static str>;
    type Case = ((f64, usize), Vec<(&'static str, Want)>);
    const FAILS: &str = "Could not shell part with selections.";
    let cases: [Case; 3] = [
        (
            (46864.38055098077, 10),
            vec![
                ("1 mm", Ok((7257.761011528471, Some(20)))),
                ("2 mm", Ok((13901.70344712245, None))),
                ("2.499 mm", Ok((16987.86341362363, None))),
                ("2.5 mm", Err(FAILS)),
                ("2.501 mm", Ok((16999.92174174731, None))),
                ("3 mm", Ok((19886.02521468895, None))),
                ("4 mm", Ok((25130.26432481737, Some(20)))),
            ],
        ),
        ((46728.71831002654, 17), vec![("1 mm", Ok((7385.784075985124, Some(34)))), ("2.5 mm", Err(FAILS))]),
        ((46711.96314920739, 18), vec![("2.5 mm", Err(FAILS))]),
    ];
    for (case, ((v_in, f_in), checks)) in cases.into_iter().enumerate() {
        let mut t = bracket();
        if case >= 1 {
            let mut edges = front_top(&mut t);
            assert_eq!(edges.len(), 1, "the front top edge");
            if case == 2 {
                edges.extend(rim(&mut t));
            }
            t.chamfer(edges, "1 mm");
        }
        let r = t.regen();
        assert!((volume(&r) - v_in).abs() < 1e-3 && faces(&r) == f_in, "case {}: input {} mm³, {} faces vs Onshape {v_in}, {f_in}", case + 1, volume(&r), faces(&r));
        let top = t.faces_where(|c| c.z > 11.999);
        assert_eq!(top.len(), 1);
        let s = t.shell(top, "1 mm");
        for (th, want) in checks {
            t.set(s, "thickness", e(th));
            let (st, msg, v) = status(&mut t, s);
            let what = format!("case {} {th}", case + 1);
            match want {
                Ok((wv, wf)) => {
                    assert_eq!(st, FeatureStatus::Ok, "{what}: {msg:?}");
                    // The volume integrator's relative error (1e-7): 2.501 mm
                    // moved 0.0015 when the shell took the regions path.
                    assert!((v - wv).abs() < (1e-7 * wv).max(1e-3), "{what}: {v} mm³ vs Onshape {wv}");
                    if let Some(wf) = wf {
                        let n = faces(&t.regen());
                        assert_eq!(n, wf, "{what}: faces");
                    }
                }
                Err(m) => {
                    assert_eq!(st, FeatureStatus::Error, "{what}: Onshape fails ({v} mm³)");
                    assert_eq!(msg.as_deref(), Some(m), "{what}");
                }
            }
        }
    }
}

/// Robustness sweep, family 1 (live "kern (10-23 12:30) blend sweep",
/// behaviours.md): fillets and chamfers on edges that touch other blends or
/// run in tangent chains, against Onshape's volumes, face counts and errors.
/// Where Onshape's displayed volume carries a wide error band ([lo–hi] from
/// its mass properties) the check is against the exact value when we have a
/// closed form (and Onshape's must lie in its band), else against its band.
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn blend_sweep_matches_onshape() {
    fn sketch_on(t: &mut T, plane: Reference, build: impl FnOnce(&mut noshape_sketch::Sketch)) -> Vec<Reference> {
        let mut sk = noshape_sketch::Sketch::new();
        build(&mut sk);
        let s = t.add("sketch", &[("sketchPlane", q(vec![plane])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
        let r = t.regen();
        r.display.sketches.iter().find(|x| x.feature == s).unwrap().regions.iter().map(|g| Reference::SketchRegion { sketch: s, key: g.key.clone() }).collect()
    }
    fn result(t: &mut T, id: FeatureId) -> Result<(f64, usize), String> {
        let (st, msg, v) = status(t, id);
        let faces = t.regen().context.bodies[0].shape.count(noshape_kernel::ShapeType::Face);
        match st {
            FeatureStatus::Ok => Ok((v, faces)),
            _ => Err(msg.unwrap_or_default()),
        }
    }
    let mm = |v: f64| v * 1e-3;
    let near = |a: f64, b: f64| (a - b).abs() < 1e-9;
    type Got = Result<(f64, usize), String>;
    let mut out: Vec<(String, Got)> = Vec::new();
    // 1. R5 vertical edges, then R5 / R4 / R6 on the top loop.
    for r in ["5 mm", "4 mm", "6 mm"] {
        let mut t = T::new();
        t.block(60.0, 40.0, 20.0);
        let v = t.edges_where(vertical);
        t.fillet(v, "5 mm");
        let top = t.edges_where(|i| near(i.start.z, mm(20.0)) && near(i.end.z, mm(20.0)) && near(i.start.y, 0.0) && near(i.end.y, 0.0));
        let f = t.fillet(top, r);
        out.push((format!("1 top loop {r}"), result(&mut t, f)));
    }
    // 2. R8 on the top front edge, a Ø6 hole at (30,4) cutting into it, then the hole's top rim.
    for blend in ["fillet", "chamfer"] {
        let mut t = T::new();
        t.block(60.0, 40.0, 20.0);
        let e8 = t.edges_where(|i| near(i.start.z, mm(20.0)) && near(i.end.z, mm(20.0)) && near(i.start.y, 0.0) && near(i.end.y, 0.0));
        t.fillet(e8, "8 mm");
        t.holes(&[((30.0, 4.0), 3.0)]);
        let rim = t.edges_where(|i| i.start.z > mm(12.0) && i.end.z > mm(12.0) && ((i.start.x - mm(30.0)).hypot(i.start.y - mm(4.0)) - mm(3.0)).abs() < 1e-7 && ((i.end.x - mm(30.0)).hypot(i.end.y - mm(4.0)) - mm(3.0)).abs() < 1e-7);
        let one = vec![rim[0].clone()];
        let f = if blend == "fillet" { t.fillet(one, "1 mm") } else { t.chamfer(one, "0.5 mm") };
        out.push((format!("2 rim {blend} ({} rim edges)", rim.len()), result(&mut t, f)));
    }
    // 3. A slot pocket 5 mm deep: bottom loop R2, top rim chamfer 1, both.
    for which in ["bottom fillet", "rim chamfer", "both"] {
        let mut t = T::new();
        t.block(60.0, 40.0, 20.0);
        let top = t.faces_where(|c| c.z > 19.999);
        let regions = sketch_on(&mut t, top[0].clone(), |sk| {
            let p = |x: f64, y: f64| DVec2::new(mm(x), mm(y));
            sk.add_line(p(24.0, 14.0), p(36.0, 14.0));
            sk.add_arc(p(36.0, 20.0), p(36.0, 14.0), p(36.0, 26.0));
            sk.add_line(p(36.0, 26.0), p(24.0, 26.0));
            sk.add_arc(p(24.0, 20.0), p(24.0, 26.0), p(24.0, 14.0));
        });
        let small: Vec<Reference> = regions.into_iter().take(1).collect();
        let x = t.add("extrude", &[("entities", q(small)), ("depth", e("5 mm")), ("operationType", en("REMOVE")), ("oppositeDirection", ParamValue::Bool(true)), ("defaultScope", ParamValue::Bool(true))]);
        out.push(("3 pocket".to_string(), result(&mut t, x)));
        let mut last = x;
        if which != "rim chamfer" {
            let bottom = t.edges_where(|i| near(i.start.z, mm(15.0)) && near(i.end.z, mm(15.0)));
            last = t.fillet(vec![bottom[0].clone()], "2 mm");
        }
        if which != "bottom fillet" {
            let rim = t.edges_where(|i| near(i.start.z, mm(20.0)) && near(i.end.z, mm(20.0)) && i.start.x > mm(17.0) && i.start.x < mm(43.0) && i.start.y > mm(13.0) && i.start.y < mm(27.0));
            last = t.chamfer(vec![rim[0].clone()], "1 mm");
        }
        out.push((format!("3 {which}"), result(&mut t, last)));
    }
    // 4. L bracket, R4 inner corner, then on the y=0 end face: the foot's top
    // line (chain line + R4 arc + line) or all 7 edges, R1 / chamfer 1.
    for (blend, all) in [("fillet", false), ("fillet", true), ("chamfer", false), ("chamfer", true)] {
        let mut t = T::new();
        let regions = sketch_on(&mut t, Reference::Plane(DatumPlane::Front), |sk| {
            let pts: Vec<_> = [(0.0, 0.0), (50.0, 0.0), (50.0, 10.0), (10.0, 10.0), (10.0, 40.0), (0.0, 40.0)].iter().map(|(x, y)| sk.add_point(DVec2::new(mm(*x), mm(*y)))).collect();
            for i in 0..pts.len() {
                sk.add_line_between(pts[i], pts[(i + 1) % pts.len()]).unwrap();
            }
        });
        let x = t.add("extrude", &[("entities", q(regions)), ("depth", e("30 mm")), ("operationType", en("NEW"))]);
        out.push(("4 L".to_string(), result(&mut t, x)));
        let inner = t.edges_where(|i| near(i.start.x, mm(10.0)) && near(i.end.x, mm(10.0)) && near(i.start.z, mm(10.0)) && near(i.end.z, mm(10.0)));
        t.fillet(inner, "4 mm");
        let end = if all {
            t.edges_where(|i| near(i.start.y, 0.0) && near(i.end.y, 0.0))
        } else {
            t.edges_where(|i| near(i.start.y, 0.0) && near(i.end.y, 0.0) && near(i.start.z, mm(10.0)) && near(i.end.z, mm(10.0)) && i.start.x > mm(20.0))
        };
        let n = end.len();
        let f = if blend == "fillet" { t.fillet(end, "1 mm") } else { t.chamfer(end, "1 mm") };
        out.push((format!("4 {blend} {} ({n} edges)", if all { "loop" } else { "pick" }), result(&mut t, f)));
    }
    // 5. Chamfer 2 on the vertical edges, then R3 / R1 on the top front edge.
    for (r, prop) in [("3 mm", false), ("3 mm", true), ("1 mm", true)] {
        let mut t = T::new();
        t.block(60.0, 40.0, 20.0);
        let v = t.edges_where(vertical);
        t.chamfer(v, "2 mm");
        let top = t.edges_where(|i| near(i.start.z, mm(20.0)) && near(i.end.z, mm(20.0)) && near(i.start.y, 0.0) && near(i.end.y, 0.0));
        let f = t.add("fillet", &[("entities", q(top)), ("radius", e(r)), ("tangentPropagation", ParamValue::Bool(prop))]);
        out.push((format!("5 top {r} propagation {prop}"), result(&mut t, f)));
    }
    for r in ["3 mm", "1 mm"] {
        let mut t = T::new();
        t.block(60.0, 40.0, 20.0);
        let v = t.edges_where(vertical);
        t.chamfer(v, "2 mm");
        let top = t.edges_where(|i| near(i.start.z, mm(20.0)) && near(i.end.z, mm(20.0)));
        let n = top.len();
        let f = t.fillet(top, r);
        out.push((format!("5 all {n} top edges {r}"), result(&mut t, f)));
    }
    // Onshape: Ok((volume, faces, tolerance)) or its error.
    type Want = Result<(f64, usize, f64), &'static str>;
    let want: &[(&str, Want)] = &[
        ("1 top loop 5 mm", Ok((46581.48928648512, 18, 1e-3))),
        ("1 top loop 4 mm", Ok((46932.820852109624, 18, 1e-3))),
        // Spline corner patches: Onshape 46152.2029 in [46147.443, 46156.963].
        ("1 top loop 6 mm", Ok((46152.2029118611, 18, 4.7))),
        // Onshape's error; it shows the R8 fillet's lower boundary in red.
        ("2 rim fillet (1 rim edges)", Err("Unfilleted edge overlapped by fillet.")),
        // Onshape 46640.8009 in [46622.840, 46658.761].
        ("2 rim chamfer (1 rim edges)", Ok((46640.800862188116, 9, 17.9))),
        ("3 pocket", Ok((46714.51332235383, 11, 1e-3))),
        ("3 bottom fillet", Ok((46765.06680878341, 15, 1e-3))),
        ("3 rim chamfer", Ok((46682.61656888109, 15, 1e-3))),
        ("3 both", Ok((46733.17005531068, 19, 1e-3))),
        ("4 L", Ok((23999.999999999993, 8, 1e-3))),
        ("4 fillet pick (1 edges)", Ok((24088.279888204368, 12, 1e-3))),
        // Onshape 24065.1611 in [24059.694, 24070.628].
        ("4 fillet loop (7 edges)", Ok((24065.161071524846, 16, 5.4))),
        ("4 chamfer pick (1 edges)", Ok((24068.60548952784, 12, 1e-3))),
        ("4 chamfer loop (7 edges)", Ok((24015.272156194504, 16, 1e-3))),
        // Exact (the R3 / R1 fillet run straight and cut by the 45° chamfers):
        // 47729.28029 / 47827.88643; Onshape 47729.2164 in [47726.347,
        // 47732.086] and 47827.9454 in [47826.061, 47829.830].
        ("5 top 3 mm propagation false", Ok((47729.2802865388, 11, 1e-3))),
        ("5 top 3 mm propagation true", Ok((47729.2802865388, 11, 1e-3))),
        ("5 top 1 mm propagation true", Ok((47827.886428515, 11, 1e-3))),
        // Onshape 47471.2697 in [47467.249, 47475.290]; 47798.3760 in [47795.847, 47800.905].
        ("5 all 8 top edges 3 mm", Ok((47471.26969041411, 18, 4.0))),
        ("5 all 8 top edges 1 mm", Ok((47798.37602494933, 18, 2.5))),
    ];
    let mut bad = Vec::new();
    for (label, w) in want {
        let Some((_, got)) = out.iter().find(|(l, _)| l == label) else {
            bad.push(format!("{label}: not run"));
            continue;
        };
        let ok = match (w, got) {
            (Ok((v, f, tol)), Ok((gv, gf))) => (gv - v).abs() <= *tol && gf == f,
            (Err(m), Err(gm)) => gm == m,
            _ => false,
        };
        if !ok {
            bad.push(format!("{label}: {got:?}, Onshape {w:?}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Robustness sweep, families 2 and 3 (live "kern (10-23 13:30) shell+draft
/// sweep", behaviours.md): shells and drafts Onshape builds, against its
/// volumes and face counts (all OK there).
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn shell_draft_sweep_matches_onshape() {
    let mm = |v: f64| v * 1e-3;
    let faces = |t: &mut T| t.regen().context.bodies[0].shape.count(noshape_kernel::ShapeType::Face);
    type Got = Result<(f64, usize), String>;
    let mut got: Vec<(&str, Got)> = Vec::new();
    let mut run = |label: &'static str, t: &mut T, id: FeatureId| {
        let (st, msg, v) = status(t, id);
        let f = faces(t);
        got.push((label, if st == FeatureStatus::Ok { Ok((v, f)) } else { Err(msg.unwrap_or_default()) }));
    };
    let block = || {
        let mut t = T::new();
        t.block(60.0, 40.0, 20.0);
        t
    };
    let filleted = |r: &str| {
        let mut t = block();
        let v = t.edges_where(vertical);
        t.fillet(v, r);
        t
    };
    let holed = || {
        let mut t = block();
        t.holes(&[((30.0, 20.0), 5.0)]);
        t
    };
    let plate_boss = || {
        let mut t = T::new();
        t.block(60.0, 40.0, 10.0);
        let (_, regions) = t.sketch(&[], &[((30.0, 20.0), 10.0)]);
        t.add("extrude", &[("entities", q(regions)), ("startOffset", ParamValue::Bool(true)), ("startOffsetBound", en("BLIND")), ("startOffsetDistance", e("10 mm")), ("depth", e("15 mm")), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true))]);
        t
    };
    // S1: B5, top + front removed, 2 mm.
    let mut t = filleted("5 mm");
    let fs = t.faces_where(|c| c.z > 19.999 || (c.y.abs() < 1e-6 && c.x > 10.0 && c.x < 50.0));
    assert_eq!(fs.len(), 2);
    let s = t.shell(fs, "2 mm");
    run("S1", &mut t, s);
    // S2: B + hole, top removed, 2 mm outward and inward.
    for (label, out) in [("S2 outward", true), ("S2 inward", false)] {
        let mut t = holed();
        let top = t.faces_where(|c| c.z > 19.999);
        let s = t.shell(top, "2 mm");
        t.set(s, "oppositeDirection", ParamValue::Bool(out));
        run(label, &mut t, s);
    }
    // S3: plate + boss, boss top removed, 2 mm.
    let mut t = plate_boss();
    let top = t.faces_where(|c| c.z > 24.999);
    let s = t.shell(top, "2 mm");
    run("S3", &mut t, s);
    // S4: B2, top removed, 3 / 2 / 1.5 mm.
    for (label, th) in [("S4 3 mm", "3 mm"), ("S4 2 mm", "2 mm"), ("S4 1.5 mm", "1.5 mm")] {
        let mut t = filleted("2 mm");
        let top = t.faces_where(|c| c.z > 19.999);
        let s = t.shell(top, th);
        run(label, &mut t, s);
    }
    // S5: B + hole, hollow, 2 mm.
    let mut t = holed();
    let body = t.regen().context.bodies[0].id.clone();
    let s = t.add("shell", &[("isHollow", ParamValue::Bool(true)), ("parts", q(vec![Reference::Body(body)])), ("thickness", e("2 mm"))]);
    run("S5", &mut t, s);
    let draft = |t: &mut T, neutral: Vec<Reference>, faces: Vec<Reference>, angle: &str, flip: bool| {
        t.add("draft", &[("neutralPlane", q(neutral)), ("draftFaces", q(faces)), ("angle", e(angle)), ("pullDirection", ParamValue::Bool(flip))])
    };
    // D1: B5, 5°, neutral bottom, the 4 side planes (+ fillets by propagation).
    for (label, flip) in [("D1 default", false), ("D1 flipped", true)] {
        let mut t = filleted("5 mm");
        let bottom = t.faces_where(|c| c.z < 1e-6);
        let sides = t.faces_where(|c| c.z > 1.0 && c.z < 19.0 && (c.x.abs() < 1e-6 || (c.x - 60.0).abs() < 1e-6 || c.y.abs() < 1e-6 || (c.y - 40.0).abs() < 1e-6));
        assert_eq!(sides.len(), 4);
        let d = draft(&mut t, bottom, sides, "5 deg", flip);
        run(label, &mut t, d);
    }
    // D2: B, 10° the front face, neutral top.
    for (label, flip) in [("D2 default", false), ("D2 flipped", true)] {
        let mut t = block();
        let top = t.faces_where(|c| c.z > 19.999);
        let front = t.faces_where(|c| c.y.abs() < 1e-6);
        let d = draft(&mut t, top, front, "10 deg", flip);
        run(label, &mut t, d);
    }
    // D3: plate + boss, 3° the boss cylinder, neutral the plate top.
    for (label, flip) in [("D3 default", false), ("D3 flipped", true)] {
        let mut t = plate_boss();
        let plate_top = t.faces_where(|c| (c.z - 10.0).abs() < 1e-6);
        let boss = t.faces_where(|c| c.z > 10.5 && c.z < 24.5);
        assert_eq!(boss.len(), 1);
        let d = draft(&mut t, plate_top, boss, "3 deg", flip);
        run(label, &mut t, d);
    }
    // D4: B + a Ø10 hole along Y at (x 30, z 10); 5° the front face, neutral bottom.
    for (label, flip) in [("D4 default", false), ("D4 flipped", true)] {
        let mut t = block();
        let mut sk = noshape_sketch::Sketch::new();
        sk.add_circle(DVec2::new(mm(30.0), mm(10.0)), mm(5.0));
        let sid = t.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Front)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
        let r = t.regen();
        let regions: Vec<Reference> = r.display.sketches.iter().find(|x| x.feature == sid).unwrap().regions.iter().map(|g| Reference::SketchRegion { sketch: sid, key: g.key.clone() }).collect();
        t.add("extrude", &[("entities", q(regions)), ("endBound", en("THROUGH_ALL")), ("symmetric", ParamValue::Bool(true)), ("operationType", en("REMOVE")), ("defaultScope", ParamValue::Bool(true))]);
        let bottom = t.faces_where(|c| c.z < 1e-6);
        let front = t.faces_where(|c| c.y.abs() < 1e-6 && c.z > 1.0);
        assert_eq!(front.len(), 1, "front face");
        let d = draft(&mut t, bottom, front, "5 deg", flip);
        run(label, &mut t, d);
    }
    let want: &[(&str, f64, usize)] = &[
        ("S1", 9621.858316913356, 20),
        ("S2 outward", 14900.760981384125, 14),
        ("S2 inward", 12912.08839367129, 14),
        ("S3", 13198.336173278994, 15),
        ("S4 3 mm", 16719.327412287177, 15),
        ("S4 2 mm", 11643.327412287177, 15),
        ("S4 1.5 mm", 8918.797546264334, 19),
        ("S5", 16636.2123136195, 14),
        ("D1 default", 50984.26527578373, 10),
        ("D1 flipped", 44285.57583970286, 10),
        ("D2 default", 50115.92376850158, 6),
        ("D2 flipped", 45884.07623149842, 6),
        ("D3 default", 28351.647416501906, 8),
        ("D3 flipped", 29092.544941241416, 8),
        // Exact: ± the 5° wedge 60·20²·tan5°/2 ∓ the hole through it
        // tan5°·π·5²·10; Onshape 45839.6688 in [45836.415, 45842.923] and
        // 43877.3629 in [43874.230, 43880.495].
        ("D4 default", 45839.557873, 7),
        ("D4 flipped", 43877.256820, 7),
    ];
    let mut bad = Vec::new();
    for (label, v, f) in want {
        match got.iter().find(|(l, _)| l == label).map(|(_, g)| g) {
            // Within the volume integrator's relative error (1e-7, common.cpp
            // volume_props): D4's drafted hole came out 0.0017 under the exact
            // value once the hole's cylinder was framed as live.
            Some(Ok((gv, gf))) if (gv - v).abs() < (1e-7 * v).max(1e-3) && gf == f => {}
            g => bad.push(format!("{label}: {g:?}, Onshape {v} mm³ {f} faces")),
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}


/// Robustness sweep, families 4–8 (live "kern (10-23 14:15) sweep 4-8",
/// behaviours.md): boolean, loft, sweep, thicken and move / delete / replace
/// face against Onshape's volumes (all bodies), body and face counts and
/// errors. Where Onshape's mass properties are approximate, its error band
/// (L3 dx 5: 9424.503 in [9419.257, 9429.750]; ours is the exact 9424.779).
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn sweep_4_8_matches_onshape() {
    let mm = |v: f64| v * 1e-3;
    fn sketch_on(t: &mut T, plane: Reference, build: impl FnOnce(&mut noshape_sketch::Sketch) -> Vec<u32>) -> (FeatureId, Vec<Reference>, Vec<u32>) {
        let mut sk = noshape_sketch::Sketch::new();
        let ids = build(&mut sk);
        let s = t.add("sketch", &[("sketchPlane", q(vec![plane])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
        let r = t.regen();
        let regions = r.display.sketches.iter().find(|x| x.feature == s).unwrap().regions.iter().map(|g| Reference::SketchRegion { sketch: s, key: g.key.clone() }).collect();
        (s, regions, ids)
    }
    fn plane_at(t: &mut T, z: f64) -> Reference {
        Reference::FeaturePlane(t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Top)])), ("cplaneType", en("OFFSET")), ("offset", e(&format!("{z} mm")))]))
    }
    fn poly(sk: &mut noshape_sketch::Sketch, pts: &[(f64, f64)]) -> Vec<u32> {
        let ps: Vec<_> = pts.iter().map(|(x, y)| sk.add_point(DVec2::new(x * 1e-3, y * 1e-3))).collect();
        (0..ps.len()).map(|i| sk.add_line_between(ps[i], ps[(i + 1) % ps.len()]).unwrap().0).collect()
    }
    fn new_box(t: &mut T, a: (f64, f64), b: (f64, f64), z0: f64, h: f64) {
        let (_, regions) = t.sketch(&[&[a, (b.0, a.1), b, (a.0, b.1)]], &[]);
        let mut p = vec![("entities", q(regions)), ("depth", e(&format!("{h} mm"))), ("operationType", en("NEW"))];
        if z0 > 0.0 {
            p.extend([("startOffset", ParamValue::Bool(true)), ("startOffsetBound", en("BLIND")), ("startOffsetDistance", e(&format!("{z0} mm")))]);
        }
        t.add("extrude", &p);
    }
    type Got = Result<(f64, Vec<usize>), String>;
    let mut got: Vec<(&str, Got)> = Vec::new();
    let mut run = |label: &'static str, t: &mut T, id: FeatureId| {
        let r = t.regen();
        let st = r.display.features.iter().find(|s| s.id == id).expect("feature state");
        let solids: Vec<_> = r.context.bodies.iter().filter(|b| b.kind == noshape_model::display::BodyKind::Solid).collect();
        let v: f64 = solids.iter().map(|b| noshape_kernel::volume_properties(&b.shape).unwrap().mass / MM3).sum();
        let faces = solids.iter().map(|b| b.shape.count(noshape_kernel::ShapeType::Face)).collect();
        got.push((label, if st.status == FeatureStatus::Ok { Ok((v, faces)) } else { Err(st.message.clone().unwrap_or_default()) }));
        r
    };
    let bodies = |t: &mut T| t.regen().context.bodies.iter().map(|b| Reference::Body(b.id.clone())).collect::<Vec<_>>();
    let boolean = |t: &mut T, op: &str, targets: Vec<Reference>, tools: Vec<Reference>| {
        let mut p = vec![("operationType", en(op)), ("tools", q(tools))];
        if !targets.is_empty() {
            p.push(("targets", q(targets)));
        }
        t.add("booleanBodies", &p)
    };
    // Bo1 / Bo2: two boxes sharing a face / overlapping, union.
    for (label, b) in [("Bo1", ((30.0, 0.0), (60.0, 20.0))), ("Bo2", ((20.0, 10.0), (60.0, 30.0)))] {
        let mut t = T::new();
        new_box(&mut t, (0.0, 0.0), if label == "Bo1" { (30.0, 20.0) } else { (40.0, 20.0) }, 0.0, 10.0);
        new_box(&mut t, b.0, b.1, 0.0, 10.0);
        let all = bodies(&mut t);
        let u = boolean(&mut t, "UNION", vec![], all);
        run(label, &mut t, u);
    }
    // Bo3: a cylinder tangent to the block's front face along a line, subtract.
    let mut t = T::new();
    t.block(60.0, 40.0, 20.0);
    let (_, regions) = t.sketch(&[], &[((30.0, 5.0), 5.0)]);
    t.add("extrude", &[("entities", q(regions)), ("depth", e("20 mm")), ("operationType", en("NEW"))]);
    let all = bodies(&mut t);
    let s = boolean(&mut t, "SUBTRACTION", vec![all[0].clone()], vec![all[1].clone()]);
    run("Bo3", &mut t, s);
    // The same cylinder as an Extrude Remove (live 10-23 18:00): the same error.
    let mut t = T::new();
    t.block(60.0, 40.0, 20.0);
    let h = t.holes(&[((30.0, 5.0), 5.0)]);
    run("Bo3 extrude", &mut t, h);
    // Bo4: a sphere R20 at the block's top back-right corner, intersect.
    let mut t = T::new();
    t.block(60.0, 40.0, 20.0);
    let r = t.regen();
    let b = &r.context.bodies[0];
    let vi = b.shape.vertices().iter().position(|v| v.vertex_point().map(|p| p.distance(glam::DVec3::new(0.06, 0.04, 0.02)) < 1e-9).unwrap_or(false)).expect("corner");
    t.add("fSphere", &[("center", q(vec![Reference::Vertex((*b.vertices[vi]).clone())])), ("radius", e("20 mm"))]);
    let all = bodies(&mut t);
    let s = boolean(&mut t, "INTERSECTION", vec![], all);
    run("Bo4", &mut t, s);
    // Bo5: a box whose top is the block's top, subtract.
    let mut t = T::new();
    t.block(60.0, 40.0, 20.0);
    new_box(&mut t, (20.0, 10.0), (40.0, 30.0), 15.0, 5.0);
    let all = bodies(&mut t);
    let s = boolean(&mut t, "SUBTRACTION", vec![all[0].clone()], vec![all[1].clone()]);
    run("Bo5", &mut t, s);
    // Lofts.
    let item = |r: Reference| {
        let mut v = noshape_model::params::ParamValues::default();
        v.set("sheetProfileEntities", q(vec![r]));
        v
    };
    let square = |s: f64| [(-s / 2.0, -s / 2.0), (s / 2.0, -s / 2.0), (s / 2.0, s / 2.0), (-s / 2.0, s / 2.0)];
    {
        let mut t = T::new();
        let (_, r1, _) = sketch_on(&mut t, Reference::Plane(DatumPlane::Top), |sk| poly(sk, &square(40.0)));
        let p = plane_at(&mut t, 30.0);
        let (_, r2, _) = sketch_on(&mut t, p, |sk| vec![sk.add_circle(DVec2::ZERO, mm(15.0)).0]);
        let l = t.add("loft", &[("sheetProfilesArray", ParamValue::Array(vec![item(r1[0].clone()), item(r2[0].clone())]))]);
        run("L1", &mut t, l);
    }
    {
        let mut t = T::new();
        let (_, r1, _) = sketch_on(&mut t, Reference::Plane(DatumPlane::Top), |sk| poly(sk, &square(40.0)));
        let p2 = plane_at(&mut t, 20.0);
        let (_, r2, _) = sketch_on(&mut t, p2, |sk| vec![sk.add_circle(DVec2::ZERO, mm(20.0)).0]);
        let p3 = plane_at(&mut t, 40.0);
        let d = 10.0 * std::f64::consts::SQRT_2;
        let (_, r3, _) = sketch_on(&mut t, p3, |sk| poly(sk, &[(d, 0.0), (0.0, d), (-d, 0.0), (0.0, -d)]));
        let l = t.add("loft", &[("sheetProfilesArray", ParamValue::Array(vec![item(r1[0].clone()), item(r2[0].clone()), item(r3[0].clone())]))]);
        let r = run("L2", &mut t, l);
        // Live (dom/kern-loft-L2.bodydetails.json): each side edge runs from a
        // bottom corner to the top corner 45° clockwise, e.g. (20, 20, 0) to
        // (14.142, 0, 40).
        if let Some(b) = r.context.bodies.first() {
            let mm = |p: glam::DVec3| p * 1e3;
            let found = b.shape.edges().iter().any(|e| {
                let vs: Vec<glam::DVec3> = e.vertices().iter().filter_map(|v| v.vertex_point().ok()).map(mm).collect();
                let near = |p: glam::DVec3, q: (f64, f64, f64)| p.distance(glam::DVec3::new(q.0, q.1, q.2)) < 1e-6;
                vs.len() == 2 && vs.iter().any(|&p| near(p, (20.0, 20.0, 0.0))) && vs.iter().any(|&p| near(p, (10.0 * std::f64::consts::SQRT_2, 0.0, 40.0)))
            });
            assert!(found, "L2: no side edge (20, 20, 0) -> (14.142, 0, 40)");
            // Live's tight box (evBox3d tight): x, y ±20.943282309315897; ours
            // 20.94668 (Parasolid fits the faces, 34 x 5 poles; its middle
            // section sits at 0.45226 of the way, our mean chord at 0.45255).
            let bb = b.shape.bounding_box(true).unwrap();
            assert!((bb.max.x * 1e3 - 20.943282309315897).abs() < 5e-3 && (bb.min.y * 1e3 + 20.943282309315897).abs() < 5e-3, "L2 box {bb:?}");
        }
    }
    for (label, dx) in [("L3", 30.0), ("L3 dx 5", 5.0)] {
        let mut t = T::new();
        let (_, r1, _) = sketch_on(&mut t, Reference::Plane(DatumPlane::Top), |sk| vec![sk.add_circle(DVec2::ZERO, mm(10.0)).0]);
        let p = plane_at(&mut t, 30.0);
        let (_, r2, _) = sketch_on(&mut t, p, |sk| vec![sk.add_circle(DVec2::new(mm(dx), 0.0), mm(10.0)).0]);
        let l = t.add("loft", &[("sheetProfilesArray", ParamValue::Array(vec![item(r1[0].clone()), item(r2[0].clone())]))]);
        run(label, &mut t, l);
    }
    // Sweeps: path on Front, profile on Right at the origin.
    for (label, r, square_profile) in [("Sw1", 20.0, false), ("Sw2", 4.0, false), ("Sw3", 30.0, true)] {
        let mut t = T::new();
        let (_, prof, _) = sketch_on(&mut t, Reference::Plane(DatumPlane::Right), |sk| if square_profile { poly(sk, &square(10.0)) } else { vec![sk.add_circle(DVec2::ZERO, mm(5.0)).0] });
        let (pid, _, ids) = sketch_on(&mut t, Reference::Plane(DatumPlane::Front), |sk| {
            let p = |x: f64, y: f64| DVec2::new(mm(x), mm(y));
            if square_profile {
                vec![sk.add_arc(p(0.0, 30.0), p(0.0, 0.0), p(30.0, 30.0)).0]
            } else {
                vec![sk.add_line(p(0.0, 0.0), p(50.0, 0.0)).0, sk.add_arc(p(50.0, r), p(50.0, 0.0), p(50.0 + r, r)).0, sk.add_line(p(50.0 + r, r), p(50.0 + r, r + 30.0)).0]
            }
        });
        let path: Vec<Reference> = ids.iter().map(|&i| Reference::SketchEntity { sketch: pid, entity: i }).collect();
        let s = t.add("sweep", &[("profiles", q(vec![prof[0].clone()])), ("path", q(path))]);
        run(label, &mut t, s);
    }
    // Thicken: the top face 3 mm (outward: z 20..23) and 3 mm symmetric (z 18.5..21.5).
    for (label, sym) in [("Th1", false), ("Th1 sym", true)] {
        let mut t = T::new();
        t.block(60.0, 40.0, 20.0);
        let top = t.faces_where(|c| c.z > 19.999);
        let mut p = vec![("entities", q(top)), ("operationType", en("NEW"))];
        if sym {
            p.extend([("midplane", ParamValue::Bool(true)), ("thickness", e("3 mm"))]);
        } else {
            p.push(("thickness1", e("3 mm")));
        }
        let th = t.add("thicken", &p);
        let r = run(label, &mut t, th);
        if let Some(slab) = r.context.bodies.get(1) {
            let bb = slab.shape.bounding_box(false).unwrap();
            let want = if sym { (18.5, 21.5) } else { (20.0, 23.0) };
            assert!((bb.min.z * 1e3 - want.0).abs() < 1e-3 && (bb.max.z * 1e3 - want.1).abs() < 1e-3, "{label}: slab z {}..{}", bb.min.z * 1e3, bb.max.z * 1e3);
        }
    }
    // Th2: the R5 fillet face at (0,0), 2 mm.
    let mut t = T::new();
    t.block(60.0, 40.0, 20.0);
    let corner = t.edges_where(|i| vertical(i) && i.start.x.abs() < 1e-9 && i.start.y.abs() < 1e-9);
    t.fillet(corner, "5 mm");
    let fillet_face: Vec<Reference> = {
        let r = t.regen();
        let b = &r.context.bodies[0];
        b.shape.faces().iter().enumerate().filter(|(_, f)| matches!(f.face_surface(), Ok(noshape_kernel::Surface::Cylinder { .. }))).map(|(i, _)| Reference::Face((*b.faces[i]).clone())).collect()
    };
    assert_eq!(fillet_face.len(), 1);
    let th = t.add("thicken", &[("entities", q(fillet_face)), ("thickness1", e("2 mm")), ("operationType", en("NEW"))]);
    run("Th2", &mut t, th);
    // Face operations.
    let mut t = T::new();
    t.block(60.0, 40.0, 20.0);
    let top = t.faces_where(|c| c.z > 19.999);
    let m = t.add("moveFace", &[("moveFaces", q(top)), ("offsetDistance", e("5 mm"))]);
    run("Fo1", &mut t, m);
    for (label, flip) in [("Fo2 default", false), ("Fo2 flipped", true)] {
        let mut t = T::new();
        t.block(60.0, 40.0, 20.0);
        t.holes(&[((30.0, 20.0), 5.0)]);
        let wall = t.faces_where(|c| (c.x - 30.0).abs() < 1e-6 && (c.y - 20.0).abs() < 1e-6 && c.z > 1.0 && c.z < 19.0);
        assert_eq!(wall.len(), 1);
        let m = t.add("moveFace", &[("moveFaces", q(wall)), ("offsetDistance", e("2 mm")), ("oppositeDirection", ParamValue::Bool(flip))]);
        run(label, &mut t, m);
    }
    let mut t = T::new();
    t.block(60.0, 40.0, 20.0);
    let v = t.edges_where(vertical);
    t.fillet(v, "5 mm");
    let fillets = t.faces_where(|c| c.z > 1.0 && c.z < 19.0 && (c.x < 5.0 || c.x > 55.0) && (c.y < 5.0 || c.y > 35.0));
    assert_eq!(fillets.len(), 4);
    let d = t.add("deleteFace", &[("deleteFaces", q(fillets))]);
    run("Fo3", &mut t, d);
    let mut t = T::new();
    t.block(60.0, 40.0, 20.0);
    t.holes(&[((30.0, 20.0), 5.0)]);
    let wall = t.faces_where(|c| (c.x - 30.0).abs() < 1e-6 && (c.y - 20.0).abs() < 1e-6 && c.z > 1.0 && c.z < 19.0);
    let d = t.add("deleteFace", &[("deleteFaces", q(wall))]);
    run("Fo4", &mut t, d);
    let mut t = T::new();
    t.block(60.0, 40.0, 20.0);
    let top = t.faces_where(|c| c.z > 19.999);
    let p = plane_at(&mut t, 25.0);
    let rf = t.add("replaceFace", &[("replaceFaces", q(top)), ("templateFace", q(vec![p]))]);
    run("Fo5", &mut t, rf);
    // Onshape: Ok((volume, faces per body, tolerance)) or its error.
    type Want = Result<(f64, &'static [usize], f64), &'static str>;
    let want: &[(&str, Want)] = &[
        ("Bo1", Ok((12000.0, &[6], 1e-3))),
        ("Bo2", Ok((14000.0, &[10], 1e-3))),
        ("Bo3", Err("Boolean operation would result in non-manifold body.")),
        ("Bo3 extrude", Err("Boolean operation would result in non-manifold body.")),
        ("Bo4", Ok((4188.790204786391, &[4], 1e-3))),
        ("Bo5", Ok((46000.0, &[11], 1e-3))),
        // Onshape 33872.2611 in [33840.890, 33903.633].
        ("L1", Ok((33872.261061945035, &[6], 31.4))),
        // Onshape 45412.2225 in [45358.320, 45466.125].
        ("L2", Ok((45412.22249042802, &[6], 53.9))),
        ("L3", Err("Current selections would create a self-intersecting body.")),
        ("L3 dx 5", Ok((9424.5030322638, &[3], 5.24))),
        ("Sw1", Ok((8750.586407451927, &[5], 1e-3))),
        ("Sw2", Err("Result of sweep intersects itself, adjust path or profile selections.")),
        ("Sw3", Ok((4712.388980384688, &[6], 1e-3))),
        ("Th1", Ok((55200.0, &[6, 6], 1e-3))),
        ("Th1 sym", Ok((55200.0, &[6, 6], 1e-3))),
        ("Th2", Ok((48269.6902001295, &[7, 6], 1e-3))),
        ("Fo1", Ok((60000.0, &[6], 1e-3))),
        ("Fo2 default", Ok((47434.51332235384, &[7], 1e-3))),
        ("Fo2 flipped", Ok((44921.23919948201, &[7], 1e-3))),
        ("Fo3", Ok((48000.0, &[6], 1e-3))),
        ("Fo4", Ok((48000.0, &[6], 1e-3))),
        ("Fo5", Ok((60000.0, &[6], 1e-3))),
    ];
    let mut bad = Vec::new();
    for (label, w) in want {
        let g = got.iter().find(|(l, _)| l == label).map(|(_, g)| g);
        let ok = match (w, g) {
            (Ok((v, f, tol)), Some(Ok((gv, gf)))) => (gv - v).abs() <= *tol && gf.as_slice() == *f,
            (Err(m), Some(Err(gm))) => gm == m,
            _ => false,
        };
        if !ok {
            bad.push(format!("{label}: {g:?}, Onshape {w:?}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Robustness sweep, draft (live "kern (10-23 20:00) draft sweep"): neutral
/// plane (hole wall, mid plane, a sloped face, drafts beside fillets with and
/// without Reapply), angles that run out, parting lines.
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn draft_sweep_matches_onshape() {
    fn plane_at(t: &mut T, z: f64) -> Reference {
        Reference::FeaturePlane(t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Top)])), ("cplaneType", en("OFFSET")), ("offset", e(&format!("{z} mm")))]))
    }
    fn draft(t: &mut T, neutral: Vec<Reference>, faces: Vec<Reference>, angle: &str, narrow: bool, refillet: bool) -> FeatureId {
        t.add("draft", &[("neutralPlane", q(neutral)), ("draftFaces", q(faces)), ("angle", e(angle)), ("pullDirection", ParamValue::Bool(narrow)), ("tangentPropagation", ParamValue::Bool(false)), ("reFillet", ParamValue::Bool(refillet))])
    }
    let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
    let sides = |c: glam::DVec3| c.z > 1.0 && c.z < 19.0 && (near(c.x, 0.0) || near(c.x, 60.0) || near(c.y, 0.0) || near(c.y, 40.0));
    type Got = Result<(f64, usize), String>;
    let mut got: Vec<(String, Got)> = Vec::new();
    let mut run = |label: &str, t: &mut T, id: FeatureId| {
        let r = t.regen();
        let st = r.display.features.iter().find(|s| s.id == id).expect("feature state");
        let g = if st.status == FeatureStatus::Ok {
            assert!(r.context.bodies.iter().all(|b| b.shape.is_valid()), "{label}: invalid solid");
            Ok((volume(&r), r.context.bodies[0].shape.count(noshape_kernel::ShapeType::Face)))
        } else {
            Err(st.message.clone().unwrap_or_default())
        };
        got.push((label.to_string(), g));
    };
    // N1: the hole wall.
    let mut t = T::new();
    t.block(60.0, 40.0, 20.0);
    t.holes(&[((30.0, 20.0), 5.0)]);
    let bottom = t.faces_where(|c| c.z < 1e-6);
    let wall = t.faces_where(|c| near(c.x, 30.0) && near(c.y, 20.0) && c.z > 1.0 && c.z < 19.0);
    let d = draft(&mut t, bottom.clone(), wall.clone(), "5 deg", false, false);
    run("N1", &mut t, d);
    t.set(d, "pullDirection", ParamValue::Bool(true));
    run("N1 pd true", &mut t, d);
    // N2: neutral at mid height.
    let mut t = T::new();
    t.block(60.0, 40.0, 20.0);
    let mid = plane_at(&mut t, 10.0);
    let s = t.faces_where(sides);
    let d = draft(&mut t, vec![mid], s, "10 deg", false, false);
    run("N2", &mut t, d);
    // N3: the face of a 5 mm chamfer on the top front edge.
    let mut t = T::new();
    t.block(60.0, 40.0, 20.0);
    let edge = t.edges_where(|i| i.start.y.abs() < 1e-9 && i.end.y.abs() < 1e-9 && i.start.z > 0.01999 && i.end.z > 0.01999);
    t.chamfer(edge, "5 mm");
    let bottom = t.faces_where(|c| c.z < 1e-6);
    let ch = t.faces_where(|c| c.y > 0.1 && c.y < 4.9 && c.z > 15.1 && c.z < 19.9);
    assert_eq!(ch.len(), 1);
    let d = draft(&mut t, bottom, ch, "5 deg", false, false);
    run("N3", &mut t, d);
    t.set(d, "pullDirection", ParamValue::Bool(true));
    run("N3 pd true", &mut t, d);
    // N4 / N5: the 4 planar sides beside R5 vertical fillets / below an R2 top loop.
    for (label, vertical_fillets) in [("N4", true), ("N5", false)] {
        for refillet in [false, true] {
            let mut t = T::new();
            t.block(60.0, 40.0, 20.0);
            if vertical_fillets {
                let v = t.edges_where(vertical);
                t.fillet(v, "5 mm");
            } else {
                let top = t.edges_where(|i| i.start.z > 0.01999 && i.end.z > 0.01999);
                t.fillet(top, "2 mm");
            }
            let bottom = t.faces_where(|c| c.z < 1e-6);
            let s = t.faces_where(|c| c.z > 1.0 && c.z < 17.9 && (near(c.x, 0.0) || near(c.x, 60.0) || near(c.y, 0.0) || near(c.y, 40.0)));
            assert_eq!(s.len(), 4, "{label}");
            let d = draft(&mut t, bottom, s, "5 deg", false, refillet);
            run(&format!("{label} reapply {refillet}"), &mut t, d);
            t.set(d, "pullDirection", ParamValue::Bool(true));
            run(&format!("{label} reapply {refillet} pd true"), &mut t, d);
        }
    }
    // N6: L bracket, the wall's inner face beside an R3 concave fillet.
    for refillet in [false, true] {
        let mut t = T::new();
        t.block(60.0, 40.0, 5.0);
        let (_, regions) = t.sketch(&[&[(0.0, 0.0), (5.0, 0.0), (5.0, 40.0), (0.0, 40.0)]], &[]);
        t.add("extrude", &[("entities", q(regions)), ("depth", e("25 mm")), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true)), ("startOffset", ParamValue::Bool(true)), ("startOffsetBound", en("BLIND")), ("startOffsetDistance", e("5 mm"))]);
        let concave = t.edges_where(|i| (i.start.x - 0.005).abs() < 1e-9 && (i.end.x - 0.005).abs() < 1e-9 && (i.start.z - 0.005).abs() < 1e-9 && (i.end.z - 0.005).abs() < 1e-9);
        assert_eq!(concave.len(), 1);
        t.fillet(concave, "3 mm");
        let base_top = t.faces_where(|c| near(c.z, 5.0) && c.x > 8.0);
        let wall = t.faces_where(|c| near(c.x, 5.0) && c.z > 8.0);
        assert_eq!((base_top.len(), wall.len()), (1, 1));
        let d = draft(&mut t, base_top, wall, "5 deg", false, refillet);
        run(&format!("N6 reapply {refillet}"), &mut t, d);
    }
    // S1: the 4 sides narrowing upward.
    for ang in ["44 deg", "45 deg", "50 deg"] {
        let mut t = T::new();
        t.block(60.0, 40.0, 20.0);
        let bottom = t.faces_where(|c| c.z < 1e-6);
        let s = t.faces_where(sides);
        let d = draft(&mut t, bottom, s, ang, true, false);
        run(&format!("S1 {ang}"), &mut t, d);
    }
    // S2: a 10 mm plate's two big faces narrowing upward.
    for ang in ["15 deg", "14 deg", "13 deg"] {
        let mut t = T::new();
        t.block(60.0, 10.0, 20.0);
        let bottom = t.faces_where(|c| c.z < 1e-6);
        let s = t.faces_where(|c| c.z > 1.0 && c.z < 19.0 && (near(c.y, 0.0) || near(c.y, 10.0)));
        let d = draft(&mut t, bottom, s, ang, true, false);
        run(&format!("S2 {ang}"), &mut t, d);
    }
    // S3: the hole wall (pd true widens it upward: through the sides from 36.9°).
    for (ang, pd) in [("30 deg", false), ("40 deg", false), ("30 deg", true), ("36 deg", true), ("37 deg", true), ("40 deg", true)] {
        let mut t = T::new();
        t.block(60.0, 40.0, 20.0);
        t.holes(&[((30.0, 20.0), 5.0)]);
        let bottom = t.faces_where(|c| c.z < 1e-6);
        let wall = t.faces_where(|c| near(c.x, 30.0) && near(c.y, 20.0) && c.z > 1.0 && c.z < 19.0);
        let d = draft(&mut t, bottom, wall, ang, pd, false);
        run(&format!("S3 {ang} pd {pd}"), &mut t, d);
    }
    // P1: a cylinder's top circle as the parting line.
    let mut t = T::new();
    let (_, regions) = t.sketch(&[], &[((0.0, 0.0), 15.0)]);
    t.add("extrude", &[("entities", q(regions)), ("depth", e("20 mm")), ("operationType", en("NEW"))]);
    let top = t.edges_where(|i| i.start.z > 0.01999 && i.end.z > 0.01999);
    let d = t.add("draft", &[("draftFeatureType", en("PARTING_LINE")), ("pullDirectionEntity", q(vec![Reference::Plane(DatumPlane::Top)])), ("partingEdges", q(top)), ("angle", e("5 deg"))]);
    run("P1", &mut t, d);
    // P2: the sides split at z = 10, symmetric and two-sided.
    for (label, sides_mode) in [("P2 symmetric", "SYMMETRIC"), ("P2 two-sided", "TWO_SIDED")] {
        let mut t = T::new();
        t.block(60.0, 40.0, 20.0);
        let mid = plane_at(&mut t, 10.0);
        let s = t.faces_where(sides);
        t.add("splitPart", &[("splitType", en("FACE")), ("faceTargets", q(s)), ("faceTools", q(vec![mid]))]);
        let split = t.edges_where(|i| (i.start.z - 0.01).abs() < 1e-9 && (i.end.z - 0.01).abs() < 1e-9);
        assert_eq!(split.len(), 4, "{label}");
        let mut p = vec![("draftFeatureType", en("PARTING_LINE")), ("pullDirectionEntity", q(vec![Reference::Plane(DatumPlane::Top)])), ("partingEdges", q(split)), ("angle", e("5 deg")), ("partingLineSides", en(sides_mode))];
        if sides_mode == "TWO_SIDED" {
            p.push(("secondAngle", e("10 deg")));
        }
        let d = t.add("draft", &p);
        run(label, &mut t, d);
    }
    // Onshape: Ok((volume, faces, tolerance)) or its error. Re-rolled
    // fillets and the cone through the sides: Onshape's volumes are within
    // their bands (±3..7 mm³) of ours, which are exact.
    const FAILED: &str = "Selected faces could not be drafted.";
    type Want = Result<(f64, usize, f64), &'static str>;
    let want: &[(&str, Want)] = &[
        ("N1", Ok((46914.78692746756, 7, 1e-3))),
        ("N1 pd true", Ok((45815.37195704584, 7, 1e-3))),
        ("N2", Ok((48082.909877668695, 6, 1e-3))),
        ("N3", Err(FAILED)),
        ("N3 pd true", Ok((47999.99999999999, 6, 1e-3))),
        ("N4 reapply false", Ok((50984.26527578373, 10, 1e-3))),
        ("N4 reapply false pd true", Ok((44285.57583970286, 10, 1e-3))),
        ("N4 reapply true", Ok((51156.248987292216, 10, 0.1))),
        ("N4 reapply true pd true", Ok((44157.15395195121, 10, 0.1))),
        ("N5 reapply false", Ok((51361.06091283852, 10, 0.15))),
        ("N5 reapply false pd true", Ok((44454.35765621345, 10, 0.15))),
        ("N5 reapply true", Ok((51361.06091283851, 10, 0.15))),
        ("N5 reapply true pd true", Ok((44454.35765621345, 10, 0.15))),
        ("N6 reapply false", Ok((15969.235553017523, 9, 1e-3))),
        ("N6 reapply true", Ok((15969.235553017523, 9, 1e-3))),
        ("S1 44 deg", Ok((19319.700312126508, 6, 1e-3))),
        ("S1 45 deg", Ok((18666.666666666668, 5, 1e-3))),
        ("S1 50 deg", Ok((15663.193115309228, 5, 1e-3))),
        ("S2 15 deg", Ok((5598.076211353317, 5, 1e-3))),
        ("S2 14 deg", Ok((6016.127931763663, 6, 1e-3))),
        ("S2 13 deg", Ok((6459.163412986485, 6, 1e-3))),
        ("S3 30 deg pd false", Ok((47773.27507947072, 7, 1e-3))),
        ("S3 40 deg pd false", Ok((47843.9998195257, 7, 1e-3))),
        ("S3 30 deg pd true", Ok((40009.07814154574, 7, 1e-3))),
        ("S3 36 deg pd true", Ok((37441.97885388734, 7, 1e-3))),
        ("S3 37 deg pd true", Ok((36937.339707750834, 8, 1e-3))),
        ("S3 40 deg pd true", Ok((35293.297217251886, 8, 0.05))),
        ("P1", Err(FAILED)),
        ("P2 symmetric", Ok((49770.184647173286, 10, 1e-3))),
        ("P2 two-sided", Ok((50689.81706950565, 10, 1e-3))),
    ];
    let mut bad = Vec::new();
    for (label, w) in want {
        let g = got.iter().find(|(l, _)| l == label).map(|(_, g)| g);
        let ok = match (w, g) {
            (Ok((v, f, tol)), Some(Ok((gv, gf)))) => (gv - v).abs() <= *tol && gf == f,
            (Err(m), Some(Err(gm))) => gm == m,
            _ => false,
        };
        if !ok {
            bad.push(format!("{label}: {g:?}, Onshape {w:?}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Robustness sweep, fillet round 2 (live "kern (10-23 20:30) fillet round
/// 2"): variable radius, vertex blends at 3-edge vertices, tangent chains over
/// lines and arcs, fillets consuming a face, conic and curvature sections,
/// fillets after a shell. Conic / curvature / variable volumes are Onshape's
/// approximations (its bands ±6..18 mm³); ours are within 0.15.
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn fillet_round_2_matches_onshape() {
    let mm = |v: f64| v * 1e-3;
    let at = |p: glam::DVec3, x: f64, y: f64, z: f64| p.distance(glam::DVec3::new(mm(x), mm(y), mm(z))) < 1e-9;
    let line_at = |i: &noshape_kernel::EdgeInfo, f: &dyn Fn(glam::DVec3) -> bool| matches!(i.curve, noshape_kernel::Curve::Line { .. }) && f(i.start) && f(i.end);
    fn vertex_at(t: &mut T, p: glam::DVec3) -> Reference {
        let r = t.regen();
        let b = &r.context.bodies[0];
        let vi = b.shape.vertices().iter().position(|v| v.vertex_point().is_ok_and(|q| q.distance(p * 1e-3) < 1e-9)).expect("vertex");
        Reference::Vertex((*b.vertices[vi]).clone())
    }
    fn add_box(t: &mut T, a: (f64, f64), b: (f64, f64), z0: f64, h: f64) {
        let (_, regions) = t.sketch(&[&[a, (b.0, a.1), b, (a.0, b.1)]], &[]);
        t.add("extrude", &[("entities", q(regions)), ("depth", e(&format!("{h} mm"))), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true)), ("startOffset", ParamValue::Bool(true)), ("startOffsetBound", en("BLIND")), ("startOffsetDistance", e(&format!("{z0} mm")))]);
    }
    fn boss(t: &mut T, r: f64) {
        let (_, regions) = t.sketch(&[], &[((30.0, 20.0), r)]);
        t.add("extrude", &[("entities", q(regions)), ("depth", e("10 mm")), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true)), ("startOffset", ParamValue::Bool(true)), ("startOffsetBound", en("BLIND")), ("startOffsetDistance", e("20 mm"))]);
    }
    let block = || {
        let mut t = T::new();
        t.block(60.0, 40.0, 20.0);
        t
    };
    let top_front = |t: &mut T| t.edges_where(|i| line_at(i, &|p| p.y.abs() < 1e-9 && (p.z - 0.02).abs() < 1e-9));
    type Got = Result<(f64, usize), String>;
    let mut got: Vec<(String, Got)> = Vec::new();
    let mut run = |label: &str, t: &mut T, id: FeatureId| {
        let r = t.regen();
        let st = r.display.features.iter().find(|s| s.id == id).expect("feature state");
        let g = if st.status == FeatureStatus::Ok {
            assert!(r.context.bodies[0].shape.is_valid(), "{label}: invalid solid");
            Ok((volume(&r), r.context.bodies[0].shape.count(noshape_kernel::ShapeType::Face)))
        } else {
            Err(st.message.clone().unwrap_or_default())
        };
        got.push((label.to_string(), g));
    };
    let fillet = |t: &mut T, edges: Vec<Reference>, extra: Vec<(&'static str, ParamValue)>| {
        let mut p = vec![("entities", q(edges)), ("tangentPropagation", ParamValue::Bool(true))];
        p.extend(extra);
        t.add("fillet", &p)
    };
    // V1 / V2: variable radius on the top front edge.
    {
        let mut t = block();
        let edge = top_front(&mut t);
        let v0 = vertex_at(&mut t, glam::DVec3::new(0.0, 0.0, 20.0));
        let v1 = vertex_at(&mut t, glam::DVec3::new(60.0, 0.0, 20.0));
        let row = |v: Reference, r: &str| {
            let mut p = noshape_model::params::ParamValues::default();
            p.set("vertex", q(vec![v]));
            p.set("vertexRadius", e(r));
            p
        };
        let f = fillet(&mut t, edge, vec![("radius", e("2 mm")), ("isVariable", ParamValue::Bool(true)), ("vertexSettings", ParamValue::Array(vec![row(v0, "2 mm"), row(v1, "6 mm")]))]);
        run("V1", &mut t, f);
        let mut t = block();
        let edge = top_front(&mut t);
        let mut p = noshape_model::params::ParamValues::default();
        p.set("edge", q(edge.clone()));
        p.set("edgeParameter", e("0.5"));
        p.set("pointOnEdgeRadius", e("6 mm"));
        let f = fillet(&mut t, edge, vec![("radius", e("2 mm")), ("isVariable", ParamValue::Bool(true)), ("pointOnEdgeSettings", ParamValue::Array(vec![p]))]);
        run("V2", &mut t, f);
    }
    // C1: R5 on the three edges at (60, 40, 20); C2: R3 / R5 / R7 one at a time.
    let corner = |i: &noshape_kernel::EdgeInfo| at(i.start, 60.0, 40.0, 20.0) || at(i.end, 60.0, 40.0, 20.0);
    {
        let mut t = block();
        let es = t.edges_where(corner);
        assert_eq!(es.len(), 3);
        let f = fillet(&mut t, es, vec![("radius", e("5 mm"))]);
        run("C1", &mut t, f);
        let mut t = block();
        let ex = t.edges_where(|i| corner(i) && (i.start.x - i.end.x).abs() > 1e-6);
        let ey = t.edges_where(|i| corner(i) && (i.start.y - i.end.y).abs() > 1e-6);
        let ez = t.edges_where(|i| corner(i) && (i.start.z - i.end.z).abs() > 1e-6);
        fillet(&mut t, ex, vec![("radius", e("3 mm"))]);
        let f = fillet(&mut t, ey, vec![("radius", e("5 mm"))]);
        run("C2 R3 + R5", &mut t, f);
        let f = fillet(&mut t, ez, vec![("radius", e("7 mm"))]);
        run("C2 R7", &mut t, f);
    }
    // C3: the L step's three edges at (30, 0, 20).
    {
        let mut t = block();
        add_box(&mut t, (0.0, 0.0), (30.0, 40.0), 20.0, 10.0);
        let es = t.edges_where(|i| at(i.start, 30.0, 0.0, 20.0) || at(i.end, 30.0, 0.0, 20.0));
        assert_eq!(es.len(), 3);
        let f = fillet(&mut t, es, vec![("radius", e("3 mm"))]);
        run("C3", &mut t, f);
    }
    // T1: one straight top edge of a slot boss takes the whole loop.
    {
        let mut t = block();
        let mut sk = noshape_sketch::Sketch::new();
        let p = |x: f64, y: f64| DVec2::new(mm(x), mm(y));
        let (a, b, c, d) = (sk.add_point(p(15.0, 15.0)), sk.add_point(p(45.0, 15.0)), sk.add_point(p(45.0, 25.0)), sk.add_point(p(15.0, 25.0)));
        sk.add_line_between(a, b).unwrap();
        sk.add_line_between(c, d).unwrap();
        sk.add_arc(p(45.0, 20.0), p(45.0, 15.0), p(45.0, 25.0));
        sk.add_arc(p(15.0, 20.0), p(15.0, 25.0), p(15.0, 15.0));
        let s = t.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Top)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
        let r = t.regen();
        let regions: Vec<Reference> = r.display.sketches.iter().find(|x| x.feature == s).unwrap().regions.iter().map(|g| Reference::SketchRegion { sketch: s, key: g.key.clone() }).collect();
        assert_eq!(regions.len(), 1, "slot region");
        t.add("extrude", &[("entities", q(regions)), ("depth", e("10 mm")), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true)), ("startOffset", ParamValue::Bool(true)), ("startOffsetBound", en("BLIND")), ("startOffsetDistance", e("20 mm"))]);
        let edge = t.edges_where(|i| line_at(i, &|q| (q.y - 0.015).abs() < 1e-9 && (q.z - 0.03).abs() < 1e-9));
        assert_eq!(edge.len(), 1);
        let f = fillet(&mut t, edge, vec![("radius", e("1 mm"))]);
        run("T1", &mut t, f);
    }
    // T2: a Ø20 boss with an R2 top; R3 its base, alone and with B's top front edge.
    for with_front in [false, true] {
        let mut t = block();
        boss(&mut t, 10.0);
        let top = t.edges_where(|i| !matches!(i.curve, noshape_kernel::Curve::Line { .. }) && (i.start.z - 0.03).abs() < 1e-9);
        fillet(&mut t, top, vec![("radius", e("2 mm"))]);
        let mut es = t.edges_where(|i| matches!(i.curve, noshape_kernel::Curve::Circle { .. }) && (i.start.z - 0.02).abs() < 1e-9);
        assert_eq!(es.len(), 1);
        if with_front {
            es.extend(top_front(&mut t));
        }
        let f = fillet(&mut t, es, vec![("radius", e("3 mm"))]);
        run(if with_front { "T2 base + front" } else { "T2 base" }, &mut t, f);
    }
    // W1: both top edges of a 4 mm rib, R2 (full round) and R2.5; W2: the edges of a 2 mm chamfer face, R5.
    for r in ["2 mm", "2.5 mm"] {
        let mut t = block();
        add_box(&mut t, (28.0, 0.0), (32.0, 40.0), 20.0, 5.0);
        let es = t.edges_where(|i| line_at(i, &|q| (q.z - 0.025).abs() < 1e-9) && (i.start.x - i.end.x).abs() < 1e-9);
        assert_eq!(es.len(), 2);
        let f = fillet(&mut t, es, vec![("radius", e(r))]);
        run(&format!("W1 {r}"), &mut t, f);
    }
    {
        let mut t = block();
        let edge = top_front(&mut t);
        t.chamfer(edge, "2 mm");
        let es = t.edges_where(|i| line_at(i, &|q| ((q.y - 0.002).abs() < 1e-9 && (q.z - 0.02).abs() < 1e-9) || (q.y.abs() < 1e-9 && (q.z - 0.018).abs() < 1e-9)) && (i.start.x - i.end.x).abs() > 1e-6);
        assert_eq!(es.len(), 2);
        let f = fillet(&mut t, es, vec![("radius", e("5 mm"))]);
        run("W2", &mut t, f);
    }
    // K: references, conic and curvature sections of width 6 on the top front edge; K3 conic on the Ø20 boss's top edge.
    let width6 = |extra: Vec<(&'static str, ParamValue)>| {
        let mut p = vec![("blendControlType", en("WIDTH")), ("width", e("6 mm"))];
        p.extend(extra);
        p
    };
    let ks: Vec<(String, Vec<(&'static str, ParamValue)>)> = vec![
        ("K R3".into(), vec![("radius", e("3 mm"))]),
        ("K width 6".into(), width6(vec![])),
        ("K1 rho 0.3".into(), width6(vec![("crossSection", en("CONIC")), ("rho", e("0.3"))])),
        ("K1 rho 0.5".into(), width6(vec![("crossSection", en("CONIC")), ("rho", e("0.5"))])),
        ("K1 rho 0.7".into(), width6(vec![("crossSection", en("CONIC")), ("rho", e("0.7"))])),
        ("K2 mag 0.5".into(), width6(vec![("crossSection", en("CURVATURE")), ("magnitude", e("0.5"))])),
        ("K2 mag 0.9".into(), width6(vec![("crossSection", en("CURVATURE")), ("magnitude", e("0.9"))])),
        ("K2 mag 0.99".into(), width6(vec![("crossSection", en("CURVATURE")), ("magnitude", e("0.99"))])),
    ];
    for (label, extra) in ks {
        let mut t = block();
        let edge = top_front(&mut t);
        let f = fillet(&mut t, edge, extra);
        run(&label, &mut t, f);
    }
    {
        let mut t = block();
        boss(&mut t, 10.0);
        let top = t.edges_where(|i| !matches!(i.curve, noshape_kernel::Curve::Line { .. }) && (i.start.z - 0.03).abs() < 1e-9);
        let f = fillet(&mut t, top, width6(vec![("crossSection", en("CONIC")), ("rho", e("0.5"))]));
        run("K3", &mut t, f);
    }
    // H1: after a 2 mm shell (top removed).
    for (label, which) in [("H1 outer R5", 0), ("H1 inner R3", 1), ("H1 rim R1", 2)] {
        let mut t = block();
        let top = t.faces_where(|c| c.z > 19.999);
        t.shell(top, "2 mm");
        let es = match which {
            0 => t.edges_where(|i| vertical(i) && (i.start.x.abs() < 1e-9 || (i.start.x - 0.06).abs() < 1e-9) && (i.start.y.abs() < 1e-9 || (i.start.y - 0.04).abs() < 1e-9)),
            1 => t.edges_where(|i| vertical(i) && ((i.start.x - 0.002).abs() < 1e-9 || (i.start.x - 0.058).abs() < 1e-9) && ((i.start.y - 0.002).abs() < 1e-9 || (i.start.y - 0.038).abs() < 1e-9)),
            _ => t.edges_where(|i| (i.start.z - 0.02).abs() < 1e-9 && (i.end.z - 0.02).abs() < 1e-9),
        };
        assert_eq!(es.len(), if which == 2 { 8 } else { 4 }, "{label}");
        let r = ["5 mm", "3 mm", "1 mm"][which];
        let f = fillet(&mut t, es, vec![("radius", e(r))]);
        run(label, &mut t, f);
    }
    // Onshape: Ok((volume, faces, tolerance)) or its error.
    type Want = Result<(f64, usize, f64), &'static str>;
    let want: &[(&str, Want)] = &[
        ("V1", Ok((47776.6920190935, 7, 0.15))),
        // Onshape 47719.9155 in its band ±16.7.
        ("V2", Ok((47719.91552728729, 7, 0.5))),
        ("C1", Ok((47377.12002586809, 10, 1e-3))),
        // The R3 / R5 corner blend (B-spline): Onshape's band ±1.3.
        ("C2 R3 + R5", Ok((47577.55479947086, 10, 5e-3))),
        // Onshape's row tooltip adds a second line "1 missing selection"
        // (unresolved selections: not done yet, see the handoff).
        ("C2 R7", Err("Select edges or faces to fillet.")),
        ("C3", Ok((60000.45391070754, 12, 1e-3))),
        ("T1", Ok((51766.08132322357, 15, 1e-3))),
        ("T2 base", Ok((51219.55330336188, 10, 1e-3))),
        ("T2 base + front", Ok((51103.6683115965, 11, 1e-3))),
        ("W1 2 mm", Ok((48731.32741228718, 10, 1e-3))),
        ("W1 2.5 mm", Ok((48693.369806645474, 11, 1e-3))),
        ("W2", Ok((47816.593129651126, 8, 1e-3))),
        ("K R3", Ok((47884.11500823462, 7, 1e-3))),
        ("K width 6", Ok((47768.23001646924, 7, 1e-3))),
        // Conics: the exact rational section's volume (Onshape 47692.7452 /
        // 47819.9928 / 47920.8725, its bands ±6..14).
        ("K1 rho 0.3", Ok((47692.40446128475, 7, 2e-3))),
        ("K1 rho 0.5", Ok((47819.99999999127, 7, 2e-3))),
        ("K1 rho 0.7", Ok((47920.99699281265, 7, 2e-3))),
        // Curvature: 0.5 Onshape's value (band ±9.5); 0.9 and 0.99 the exact
        // volume of its section's poles (live "kern (10-24 00:30)"; its mass
        // properties say 47989.0516 / 47998.5618, bands ±17.7 / ±7).
        ("K2 mag 0.5", Ok((47823.54906291187, 7, 0.15))),
        // (OCCT's cut along a section that meets the faces G2: within 4e-6.)
        ("K2 mag 0.9", Ok((47988.99995362849, 7, 5e-3))),
        ("K2 mag 0.99", Ok((47998.90272468289, 7, 0.2))),
        ("K3", Ok((50968.99910265115, 9, 0.15))),
        ("H1 outer R5", Ok((11282.7963267949, 15, 1e-3))),
        ("H1 inner R3", Ok((11851.061990118455, 15, 1e-3))),
        ("H1 rim R1", Ok((11629.591085056725, 14, 1e-3))),
    ];
    // OPEN: full rounds. Two fillets meeting exactly on a face as wide as 2R
    // (W1 rib, H1 rim) are one face in Parasolid; OCCT can't build them and
    // the checked fallback (radius × (1 − 1e-5)) leaves a 2e-8 m strip of the
    // face between two cylinders: volumes right, faces 11 / 19 against 10 / 14.
    const OPEN: &[&str] = &["W1 2 mm", "H1 rim R1"];
    let mut bad = Vec::new();
    for (label, w) in want.iter().filter(|(l, _)| !OPEN.contains(l)) {
        let g = got.iter().find(|(l, _)| l == label).map(|(_, g)| g);
        let ok = match (w, g) {
            (Ok((v, f, tol)), Some(Ok((gv, gf)))) => (gv - v).abs() <= *tol && gf == f,
            (Err(m), Some(Err(gm))) => gm == m,
            _ => false,
        };
        if !ok {
            bad.push(format!("{label}: {g:?}, Onshape {w:?}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Robustness sweep, feature patterns and mirror (live "kern (10-23 22:00)
/// patterns"): Reapply (default off) onto curved faces, instances off the
/// part, mirrored features.
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn patterns_match_onshape() {
    fn sketch_on(t: &mut T, plane: Reference, circles: &[((f64, f64), f64)]) -> Vec<Reference> {
        let mut sk = noshape_sketch::Sketch::new();
        for &((x, y), r) in circles {
            sk.add_circle(DVec2::new(x * 1e-3, y * 1e-3), r * 1e-3);
        }
        let s = t.add("sketch", &[("sketchPlane", q(vec![plane])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
        let r = t.regen();
        r.display.sketches.iter().find(|x| x.feature == s).unwrap().regions.iter().map(|g| Reference::SketchRegion { sketch: s, key: g.key.clone() }).collect()
    }
    fn plane(t: &mut T, from: DatumPlane, d: f64) -> Reference {
        Reference::FeaturePlane(t.add("cPlane", &[("entities", q(vec![Reference::Plane(from)])), ("cplaneType", en("OFFSET")), ("offset", e(&format!("{d} mm")))]))
    }
    // A Ø40 cylinder along X (axis y 0, z 20, x 0..60) and a Ø6 boss at x 10 from z 50 down to it.
    fn cylinder_and_boss(t: &mut T) -> FeatureId {
        let c = sketch_on(t, Reference::Plane(DatumPlane::Right), &[((0.0, 20.0), 20.0)]);
        t.add("extrude", &[("entities", q(c)), ("depth", e("60 mm")), ("operationType", en("NEW"))]);
        let p = plane(t, DatumPlane::Top, 50.0);
        let b = sketch_on(t, p, &[((10.0, 0.0), 3.0)]);
        t.add("extrude", &[("entities", q(b)), ("endBound", en("UP_TO_NEXT")), ("oppositeDirection", ParamValue::Bool(true)), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true))])
    }
    let block = || {
        let mut t = T::new();
        t.block(60.0, 40.0, 20.0);
        t
    };
    let linear = |t: &mut T, f: FeatureId, d: &str, n: &str, reapply: bool| {
        t.add("linearPattern", &[("patternType", en("FEATURE")), ("instanceFunction", q(vec![Reference::Feature(f)])), ("directionOne", q(vec![Reference::Plane(DatumPlane::Right)])), ("distance", e(d)), ("instanceCount", e(n)), ("fullFeaturePattern", ParamValue::Bool(reapply))])
    };
    let mirror = |t: &mut T, f: FeatureId, x: f64, reapply: bool| {
        let p = plane(t, DatumPlane::Right, x);
        t.add("mirror", &[("patternType", en("FEATURE")), ("instanceFunction", q(vec![Reference::Feature(f)])), ("mirrorPlane", q(vec![p])), ("fullFeaturePattern", ParamValue::Bool(reapply))])
    };
    // (total volume, faces of the first part, parts)
    type Got = Result<(f64, usize, usize), String>;
    let mut got: Vec<(String, Got)> = Vec::new();
    let mut run = |label: String, t: &mut T, id: FeatureId| {
        let r = t.regen();
        let st = r.display.features.iter().find(|s| s.id == id).expect("feature state");
        let g = if st.status == FeatureStatus::Ok {
            assert!(r.context.bodies.iter().all(|b| b.shape.is_valid()), "{label}: invalid solid");
            let v: f64 = r.context.bodies.iter().map(|b| noshape_kernel::volume_properties(&b.shape).unwrap().mass / MM3).sum();
            Ok((v, r.context.bodies[0].shape.count(noshape_kernel::ShapeType::Face), r.context.bodies.len()))
        } else {
            Err(st.message.clone().unwrap_or_default())
        };
        got.push((label, g));
    };
    for reapply in [false, true] {
        let on = if reapply { "on" } else { "off" };
        // R1: the boss up to the cylinder, × 4 along it.
        let mut t = T::new();
        let b = cylinder_and_boss(&mut t);
        let p = linear(&mut t, b, "12 mm", "4", reapply);
        run(format!("R1 {on}"), &mut t, p);
        // R2: a radial hole round a Ø60 cylinder, × 6.
        let mut t = T::new();
        let c = sketch_on(&mut t, Reference::Plane(DatumPlane::Top), &[((0.0, 0.0), 30.0)]);
        t.add("extrude", &[("entities", q(c)), ("depth", e("20 mm")), ("operationType", en("NEW"))]);
        let h = sketch_on(&mut t, Reference::Plane(DatumPlane::Front), &[((0.0, 10.0), 2.5)]);
        let h = t.add("extrude", &[("entities", q(h)), ("endBound", en("THROUGH_ALL")), ("operationType", en("REMOVE")), ("defaultScope", ParamValue::Bool(true))]);
        let p = t.add("circularPattern", &[("patternType", en("FEATURE")), ("instanceFunction", q(vec![Reference::Feature(h)])), ("axis", q(vec![Reference::Plane(DatumPlane::Top)])), ("instanceCount", e("6")), ("fullFeaturePattern", ParamValue::Bool(reapply))]);
        run(format!("R2 {on}"), &mut t, p);
        // O1 / O2: holes running off the end.
        for (label, d) in [("O1", "15 mm"), ("O2", "16 mm")] {
            let mut t = block();
            let h = t.holes(&[((10.0, 20.0), 3.0)]);
            let p = linear(&mut t, h, d, "5", reapply);
            run(format!("{label} {on}"), &mut t, p);
        }
        // O3: a boss copied off the part.
        let mut t = block();
        let b = sketch_on(&mut t, Reference::Plane(DatumPlane::Top), &[((50.0, 20.0), 5.0)]);
        let b = t.add("extrude", &[("entities", q(b)), ("depth", e("5 mm")), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true)), ("startOffset", ParamValue::Bool(true)), ("startOffsetBound", en("BLIND")), ("startOffsetDistance", e("20 mm"))]);
        let p = linear(&mut t, b, "20 mm", "2", reapply);
        run(format!("O3 {on}"), &mut t, p);
        // M1: a fillet mirrored; M2: a hole mirrored off the part; M3: the boss mirrored onto the cylinder.
        let mut t = block();
        let edge = t.edges_where(|i| vertical(i) && i.start.x.abs() < 1e-9 && i.start.y.abs() < 1e-9);
        let f = t.fillet(edge, "5 mm");
        let m = mirror(&mut t, f, 30.0, reapply);
        run(format!("M1 {on}"), &mut t, m);
        let mut t = block();
        let h = t.holes(&[((10.0, 20.0), 3.0)]);
        let m = mirror(&mut t, h, 50.0, reapply);
        run(format!("M2 {on}"), &mut t, m);
        let mut t = T::new();
        let b = cylinder_and_boss(&mut t);
        let m = mirror(&mut t, b, 30.0, reapply);
        run(format!("M3 {on}"), &mut t, m);
    }
    const NOT_ALL: &str = "Could not create all instances as entered. Try selecting \"Reapply features\" option.";
    type Want = Result<(f64, usize, usize), &'static str>;
    let want: &[(&str, Want)] = &[
        // R1 / M3 exact (the bosses' volume integrated): Onshape 76535.8182 /
        // 75967.0149, 75967.0191, in its bands ±5.8 / ±3.6.
        ("R1 off", Ok((76535.57678598161, 11, 1))),
        ("R1 on", Ok((76535.57678598161, 11, 1))),
        ("R2 off", Err(NOT_ALL)),
        ("R2 on", Ok((53234.34358153109, 9, 1))),
        ("O1 off", Ok((45738.05328941536, 10, 1))),
        ("O1 on", Ok((45738.05328941536, 10, 1))),
        ("O2 off", Ok((45800.00293101759, 11, 1))),
        ("O2 on", Ok((45800.00293101759, 11, 1))),
        ("O3 off", Err(NOT_ALL)),
        ("O3 on", Ok((48785.39816339744, 8, 2))),
        ("M1 off", Err(NOT_ALL)),
        ("M1 on", Err("Could not pattern selected features.")),
        ("M2 off", Err(NOT_ALL)),
        ("M2 on", Ok((47434.51332235384, 7, 1))),
        ("M3 off", Ok((75966.90023606832, 7, 1))),
        ("M3 on", Ok((75966.90023606832, 7, 1))),
    ];
    let mut bad = Vec::new();
    for (label, w) in want {
        let g = got.iter().find(|(l, _)| l == label).map(|(_, g)| g);
        let ok = match (w, g) {
            // R2: six holes meeting at the axis, Onshape's band ±9.3.
            (Ok((v, f, n)), Some(Ok((gv, gf, gn)))) => (gv - v).abs() <= if *label == "R2 on" { 0.5 } else { 1e-3 } && gf == f && gn == n,
            (Err(m), Some(Err(gm))) => gm == m,
            _ => false,
        };
        if !ok {
            bad.push(format!("{label}: {g:?}, Onshape {w:?}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Robustness sweep, Hole and Rib on curved / sloped targets (live "kern
/// (10-23 23:00) hole & rib"): holes from points above a Ø40 cylinder (Start
/// from part: the depth counts from where the axis meets the part), on a 45°
/// chamfer (a counterbore there fails); ribs to a boss, extended or not, and
/// onto a cylinder that only touches the plate (a separate part: no rib).
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn hole_and_rib_match_onshape() {
    fn plane(t: &mut T, from: DatumPlane, d: f64) -> Reference {
        Reference::FeaturePlane(t.add("cPlane", &[("entities", q(vec![Reference::Plane(from)])), ("cplaneType", en("OFFSET")), ("offset", e(&format!("{d} mm")))]))
    }
    fn regions(t: &mut T, s: FeatureId) -> Vec<Reference> {
        let r = t.regen();
        r.display.sketches.iter().find(|x| x.feature == s).unwrap().regions.iter().map(|g| Reference::SketchRegion { sketch: s, key: g.key.clone() }).collect()
    }
    fn sketch(t: &mut T, plane: Reference, build: impl FnOnce(&mut noshape_sketch::Sketch) -> u32) -> (FeatureId, u32) {
        let mut sk = noshape_sketch::Sketch::new();
        let id = build(&mut sk);
        let s = t.add("sketch", &[("sketchPlane", q(vec![plane])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
        (s, id)
    }
    let mm = |v: f64| v * 1e-3;
    // C: a Ø40 cylinder along X (axis y 0, z 20).
    let cylinder = |t: &mut T| {
        let (s, _) = sketch(t, Reference::Plane(DatumPlane::Right), |sk| sk.add_circle(DVec2::new(0.0, mm(20.0)), mm(20.0)).0);
        let r = regions(t, s);
        t.add("extrude", &[("entities", q(r)), ("depth", e("60 mm")), ("operationType", en("NEW"))]);
    };
    type Got = Result<(f64, usize, usize), String>;
    let mut got: Vec<(String, Got)> = Vec::new();
    let mut run = |label: &str, t: &mut T, id: FeatureId| {
        let r = t.regen();
        let st = r.display.features.iter().find(|s| s.id == id).expect("feature state");
        let g = if matches!(st.status, FeatureStatus::Ok | FeatureStatus::Info) {
            assert!(r.context.bodies.iter().all(|b| b.shape.is_valid()), "{label}: invalid solid");
            let v: f64 = r.context.bodies.iter().map(|b| noshape_kernel::volume_properties(&b.shape).unwrap().mass / MM3).sum();
            Ok((v, r.context.bodies[0].shape.count(noshape_kernel::ShapeType::Face), r.context.bodies.len()))
        } else {
            Err(st.message.clone().unwrap_or_default())
        };
        got.push((label.to_string(), g));
    };
    let hole = |t: &mut T, at: (f64, f64), extra: Vec<(&'static str, ParamValue)>| {
        let p = plane(t, DatumPlane::Top, 50.0);
        let (s, pt) = sketch(t, p, |sk| sk.add_point(DVec2::new(mm(at.0), mm(at.1))).0);
        // The dialog fills the merge scope with the parts the hole meets.
        let parts: Vec<Reference> = t.regen().context.bodies.iter().map(|b| Reference::Body(b.id.clone())).collect();
        let mut params = vec![("locations", q(vec![Reference::SketchEntity { sketch: s, entity: pt }])), ("unitsSystem", en("METRIC")), ("holeDiameterV3", e("6 mm")), ("scope", q(parts))];
        params.extend(extra);
        t.add("hole", &params)
    };
    type Case = (&'static str, (f64, f64), Vec<(&'static str, ParamValue)>);
    let cases: Vec<Case> = vec![
        ("H1", (30.0, 0.0), vec![("holeDepthV3", e("10 mm"))]),
        ("H1 start at sketch", (30.0, 0.0), vec![("holeDepthV3", e("10 mm")), ("startStyle", en("SKETCH"))]),
        ("H1 flipped", (30.0, 0.0), vec![("holeDepthV3", e("10 mm")), ("oppositeDirection", ParamValue::Bool(true))]),
        ("H2", (30.0, 0.0), vec![("holeDepthV3", e("10 mm")), ("styleV2", en("C_BORE")), ("cBoreDiameterV3", e("10 mm")), ("cBoreDepthV3", e("3 mm"))]),
        ("H3", (30.0, 0.0), vec![("holeDepthV3", e("10 mm")), ("styleV2", en("C_SINK")), ("cSinkDiameterV3", e("10 mm")), ("cSinkAngleV3", e("90 deg"))]),
        ("H4", (30.0, 12.0), vec![("endStyleV2", en("THROUGH"))]),
    ];
    for (label, at, extra) in cases {
        let mut t = T::new();
        cylinder(&mut t);
        let h = hole(&mut t, at, extra);
        run(label, &mut t, h);
    }
    // H5: on the face of a 10 mm chamfer of B's top front edge.
    for (label, extra) in [("H5", vec![]), ("H5 counterbore", vec![("styleV2", en("C_BORE")), ("cBoreDiameterV3", e("10 mm")), ("cBoreDepthV3", e("3 mm"))])] {
        let mut t = T::new();
        t.block(60.0, 40.0, 20.0);
        let edge = t.edges_where(|i| i.start.y.abs() < 1e-9 && i.end.y.abs() < 1e-9 && (i.start.z - 0.02).abs() < 1e-9 && (i.end.z - 0.02).abs() < 1e-9);
        t.chamfer(edge, "10 mm");
        let mut x = vec![("holeDepthV3", e("8 mm"))];
        x.extend(extra);
        let h = hole(&mut t, (30.0, 5.0), x);
        run(label, &mut t, h);
    }
    // Ribs: plate 60 × 40 × 5 + boss Ø20 × 30 at (30, 20); the profile in y = 20.
    let rib = |t: &mut T, a: (f64, f64), b: (f64, f64), plane_ref: Reference, extend: bool, th: &str| {
        let (s, l) = sketch(t, plane_ref, |sk| sk.add_line(DVec2::new(mm(a.0), mm(a.1)), DVec2::new(mm(b.0), mm(b.1))).0);
        // The dialog fills Parts with the parts (live L22; empty fails).
        let parts: Vec<Reference> = t.regen().context.bodies.iter().map(|b| Reference::Body(b.id.clone())).collect();
        t.add("rib", &[("profiles", q(vec![Reference::SketchEntity { sketch: s, entity: l }])), ("parts", q(parts)), ("thickness", e(th)), ("oppositeDirection", ParamValue::Bool(false)), ("extendProfilesUpToPart", ParamValue::Bool(extend))])
    };
    for (label, a, b, extend) in [("R1", (55.0, 5.0), (40.0, 25.0), false), ("R2 extend", (52.0, 9.0), (43.0, 21.0), true), ("R2 no extend", (52.0, 9.0), (43.0, 21.0), false)] {
        let mut t = T::new();
        t.block(60.0, 40.0, 5.0);
        let (s, _) = sketch(&mut t, Reference::Plane(DatumPlane::Top), |sk| sk.add_circle(DVec2::new(mm(30.0), mm(20.0)), mm(10.0)).0);
        let r = regions(&mut t, s);
        t.add("extrude", &[("entities", q(r)), ("depth", e("30 mm")), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true)), ("startOffset", ParamValue::Bool(true)), ("startOffsetBound", en("BLIND")), ("startOffsetDistance", e("5 mm"))]);
        // y = 20: 20 from Front against its normal (−Y).
        let p = Reference::FeaturePlane(t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Front)])), ("cplaneType", en("OFFSET")), ("offset", e("20 mm")), ("oppositeDirection", ParamValue::Bool(true))]));
        let id = rib(&mut t, a, b, p, extend, "3 mm");
        run(label, &mut t, id);
    }
    // R3: a Ø20 cylinder lying on the plate (touching along a line: a part of its own).
    {
        let mut t = T::new();
        t.block(60.0, 40.0, 5.0);
        let (s, _) = sketch(&mut t, Reference::Plane(DatumPlane::Right), |sk| sk.add_circle(DVec2::new(mm(20.0), mm(15.0)), mm(10.0)).0);
        let r = regions(&mut t, s);
        t.add("extrude", &[("entities", q(r)), ("depth", e("60 mm")), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true))]);
        let p = plane(&mut t, DatumPlane::Right, 30.0);
        let id = rib(&mut t, (2.0, 5.0), (12.0, 10.0), p, true, "2 mm");
        run("R3", &mut t, id);
    }
    type Want = Result<(f64, usize, usize), &'static str>;
    let want: &[(&str, Want)] = &[
        // Exact volumes (the holes integrated over the cylinder / chamfer;
        // Start from part = the deepest first contact on the entry circle).
        // Onshape: 75093.7294, 75382.6469, 74910.2719, 75015.0433, 74499.9346,
        // 44671.8520, each within its band (±2.6..11).
        ("H1", Ok((75093.68843154919, 5, 1))),
        ("H1 start at sketch", Ok((75382.58674512603, 4, 1))),
        ("H1 flipped", Err("None of the holes intersected a part.")),
        ("H2", Ok((74910.18528847986, 7, 1))),
        ("H3", Ok((75014.90504359952, 8, 1))),
        ("H4", Ok((74499.76667944422, 4, 1))),
        // (Our cut's faces match the exact construction's to 0.002 mm²; its
        // volume reads 0.029 lower, 6e-7: integration over the merged edges.)
        ("H5", Ok((44671.99339353549, 9, 1))),
        ("H5 counterbore", Err("Hole creates invalid geometry.")),
        ("R1", Ok((21877.04070904798, 11, 1))),
        ("R2 extend", Ok((21877.04070904798, 11, 1))),
        ("R2 no extend", Ok((21798.20222255957, 13, 1))),
        ("R3", Err("None of the ribs intersected a part.")),
    ];
    // OPEN: R3's message. The extended rib runs into the plate and we say
    // "Failed to create a rib from a selected profile." as live L22 does for a
    // rib above an open box; Onshape says "None of the ribs intersected a
    // part." here (live "kern (10-23 23:00)" R3 follow-up). The rule between
    // the two isn't known yet.
    const OPEN: &[&str] = &["R3"];
    let mut bad = Vec::new();
    for (label, w) in want.iter().filter(|(l, _)| !OPEN.contains(l)) {
        let g = got.iter().find(|(l, _)| l == label).map(|(_, g)| g);
        let ok = match (w, g) {
            (Ok((v, f, n)), Some(Ok((gv, gf, gn)))) => (gv - v).abs() <= if *label == "H5" { 0.05 } else { 1e-3 } && gf == f && gn == n,
            (Err(m), Some(Err(gm))) => gm == m,
            _ => false,
        };
        if !ok {
            bad.push(format!("{label}: {g:?}, Onshape {w:?}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// qa5 SF2: two adjacent sketch regions extruded together are one part
/// without a seam (live: 8000 mm³, 6 faces).
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn adjacent_regions_extrude_without_a_seam() {
    let mut t = T::new();
    let (_, regions) = t.sketch(&[&[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)], &[(20.0, 0.0), (40.0, 0.0), (40.0, 20.0), (20.0, 20.0)]], &[]);
    assert_eq!(regions.len(), 2);
    let x = t.add("extrude", &[("entities", q(regions)), ("depth", e("10 mm")), ("operationType", en("NEW"))]);
    let r = t.ok(x);
    assert_eq!(r.context.bodies.len(), 1);
    assert!((volume(&r) - 8000.0).abs() < 1e-6, "{}", volume(&r));
    assert_eq!(r.context.bodies[0].shape.count(noshape_kernel::ShapeType::Face), 6);
}

/// qa5 ADV2: loft of a 20 × 20 square (z 0) to a Ø16 circle (z 20). Live: side
/// faces 324.6214685 mm² each (ours the same), volume 5927.728022 mm³, which
/// is Onshape's approximation: the exact ruled solid (the circle split at the
/// square's corner directions, rows by arc length) is 5927.75434.
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn loft_square_to_circle_matches_onshape() {
    let mut t = T::new();
    let (_, r1) = t.sketch(&[&[(-10.0, -10.0), (10.0, -10.0), (10.0, 10.0), (-10.0, 10.0)]], &[]);
    let p = Reference::FeaturePlane(t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Top)])), ("cplaneType", en("OFFSET")), ("offset", e("20 mm"))]));
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(DVec2::ZERO, 0.008);
    let s2 = t.add("sketch", &[("sketchPlane", q(vec![p])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
    let r = t.regen();
    let r2: Vec<Reference> = r.display.sketches.iter().find(|x| x.feature == s2).unwrap().regions.iter().map(|g| Reference::SketchRegion { sketch: s2, key: g.key.clone() }).collect();
    let item = |r: Reference| {
        let mut v = noshape_model::params::ParamValues::default();
        v.set("sheetProfileEntities", q(vec![r]));
        v
    };
    let l = t.add("loft", &[("sheetProfilesArray", ParamValue::Array(vec![item(r1[0].clone()), item(r2[0].clone())]))]);
    let r = t.ok(l);
    assert!((volume(&r) - 5927.75434).abs() < 2e-3, "{}", volume(&r));
    let b = &r.context.bodies[0];
    let sides: Vec<f64> = b.shape.faces().iter().filter(|f| !matches!(f.face_surface(), Ok(noshape_kernel::Surface::Plane(_)))).map(|f| f.area().unwrap() * 1e6).collect();
    assert_eq!(sides.len(), 4, "{sides:?}");
    assert!(sides.iter().all(|a| (a - 324.6214685).abs() < 1e-3), "{sides:?}");
}

/// qa4 r5 CHAMFER-OVERFLOW (live "v3 Session 8"): a chamfer on a top edge
/// of a 60 × 40 × 20 block wider than the 20 mm front face runs over it, the
/// part cut by the chamfer's plane and clipped by the bottom face: 25 mm →
/// 30000 mm³ (live OK). Within the faces the chamfer is exact.
#[test]
#[ignore = "T2: oracle vs live (exact values, known-difference lists)"]
fn chamfer_wider_than_its_face_overflows_like_onshape() {
    let mut base = T::new();
    base.block(60.0, 40.0, 20.0);
    let front_top = base.edges_where(|i| i.start.y.abs() < 1e-9 && i.end.y.abs() < 1e-9 && i.start.z > 0.01999 && i.end.z > 0.01999);
    assert_eq!(front_top.len(), 1);
    for (d, want) in [(5.0, 48000.0 - 12.5 * 60.0), (20.0, 48000.0 - 200.0 * 60.0), (25.0, 30000.0)] {
        let mut t = base.fork();
        let f = t.chamfer(front_top.clone(), &format!("{d} mm"));
        let (st, msg, v) = status(&mut t, f);
        assert_eq!(st, FeatureStatus::Ok, "{d} mm: {msg:?}");
        // 20 mm (the chamfer reaching the bottom edge): OCCT's chamfer is
        // 1.6e-3 mm³ over the exact solid.
        assert!((v - want).abs() < 5e-3, "{d} mm: volume {v}, want {want}");
    }
}

/// qa FEATURE-PATTERN-ADD-Y, as live (behaviours "qa (2026-10-27)
/// FEATURE-PATTERN-ADD-Y"): a Ø10 boss 10 high at (20, 20) on a 300 × 200 × 3
/// plate, Feature pattern without Reapply. The plate's left edge points −Y
/// (off the plate): along it every copy misses the part and the pattern fails;
/// flipped it makes them. A second direction off the plate drops its copies
/// silently (status OK) when the first direction's are made.
#[test]
fn feature_pattern_of_an_added_boss_off_the_plate_as_live() {
    let mut t = T::new();
    t.block(300.0, 200.0, 3.0);
    let top = Reference::FeaturePlane(t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Top)])), ("cplaneType", en("OFFSET")), ("offset", e("3 mm"))]));
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(DVec2::new(0.02, 0.02), 0.005);
    let s = t.add("sketch", &[("sketchPlane", q(vec![top])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
    let r = t.regen();
    let regions: Vec<Reference> = r.display.sketches.iter().find(|x| x.feature == s).unwrap().regions.iter().map(|g| Reference::SketchRegion { sketch: s, key: g.key.clone() }).collect();
    let boss = t.add("extrude", &[("entities", q(regions)), ("depth", e("10 mm")), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true))]);
    let front = t.edges_where(|i| i.start.y.abs() < 1e-9 && i.end.y.abs() < 1e-9 && i.start.z.abs() < 1e-9 && i.end.z.abs() < 1e-9);
    let left = t.edges_where(|i| i.start.x.abs() < 1e-9 && i.end.x.abs() < 1e-9 && i.start.z.abs() < 1e-9 && i.end.z.abs() < 1e-9);
    assert_eq!((front.len(), left.len()), (1, 1));
    let boss_v = std::f64::consts::PI * 25.0 * 10.0;
    const NOT_ALL: &str = "Could not create all instances as entered. Try selecting \"Reapply features\" option.";
    // (label, the pattern's direction parameters, bosses made or None: fails).
    type Case = (&'static str, Vec<(&'static str, ParamValue)>, Option<usize>);
    let cases: Vec<Case> = vec![
        ("left edge as picked (−Y)", vec![("directionOne", q(left.clone()))], None),
        ("left edge flipped", vec![("directionOne", q(left.clone())), ("oppositeDirection", ParamValue::Bool(true))], Some(3)),
        ("second direction as picked", vec![("directionOne", q(front.clone())), ("hasSecondDir", ParamValue::Bool(true)), ("directionTwo", q(left.clone())), ("distanceTwo", e("40 mm")), ("instanceCountTwo", e("2"))], Some(3)),
        ("second direction flipped", vec![("directionOne", q(front.clone())), ("hasSecondDir", ParamValue::Bool(true)), ("directionTwo", q(left.clone())), ("oppositeDirectionTwo", ParamValue::Bool(true)), ("distanceTwo", e("40 mm")), ("instanceCountTwo", e("2"))], Some(6)),
    ];
    for (label, extra, bosses) in cases {
        let mut u = t.fork();
        let mut params = vec![("patternType", en("FEATURE")), ("instanceFunction", q(vec![Reference::Feature(boss)])), ("distance", e("40 mm")), ("instanceCount", e("3"))];
        params.extend(extra);
        let p = u.add("linearPattern", &params);
        let (st, msg, v) = status(&mut u, p);
        match bosses {
            None => assert_eq!((st, msg.as_deref()), (FeatureStatus::Error, Some(NOT_ALL)), "{label}"),
            Some(n) => {
                assert_eq!(st, FeatureStatus::Ok, "{label}: {msg:?}");
                assert!((v - (180000.0 + n as f64 * boss_v)).abs() < 1e-3, "{label}: {v} mm³, want {n} bosses");
            }
        }
    }
}

/// SHELL-SLOW probe (qa bin/shelltime.py): 80 × 50 × h plate, Ø20 hole, the
/// top face filleted R, the bottom removed; the Shell feature's time.
/// `NOSHAPE_KERNEL_PROFILE=cpu cargo test -p noshape-model --test all kernel_robustness::shell_feature_time_probe -- --ignored --nocapture`
#[test]
#[ignore = "T2: a timing probe"]
fn shell_feature_time_probe() {
    for (h, r, th) in [(10.0, 2.0, 1.5), (30.0, 2.0, 1.5), (10.0, 4.0, 2.5)] {
        let mut t = T::new();
        let (sk, _) = t.sketch(&[&[(-40.0, -25.0), (40.0, -25.0), (40.0, 25.0), (-40.0, 25.0)]], &[((0.0, 0.0), 10.0)]);
        let r0 = t.regen();
        let plate = r0.display.sketches.iter().find(|x| x.feature == sk).unwrap().regions.iter().max_by(|a, b| a.area.total_cmp(&b.area)).map(|g| Reference::SketchRegion { sketch: sk, key: g.key.clone() }).unwrap();
        t.add("extrude", &[("entities", q(vec![plate])), ("depth", e(&format!("{h} mm"))), ("operationType", en("NEW"))]);
        let top = t.faces_where(|c| c.z > h - 1e-6);
        t.add("fillet", &[("entities", q(top)), ("radius", e(&format!("{r} mm")))]);
        let bottom = t.faces_where(|c| c.z.abs() < 1e-6);
        t.regen();
        let t0 = std::time::Instant::now();
        let s = t.shell(bottom, &format!("{th} mm"));
        let (st, msg, v) = status(&mut t, s);
        eprintln!("h{h} R{r} t{th}: {st:?} {msg:?} {v:.3} mm³ in {:.0} ms", t0.elapsed().as_secs_f64() * 1e3);
    }
}

/// PLAN 2.7's Shell target (≤ 200 ms, live ~400): qa's 80 × 50 × 10 plate
/// with a Ø20 hole, the top filleted R2, shelled 1.5 / 1.6 / 1.7 mm with the
/// bottom removed (the plate and fillet cached). Printed: the Shell's best
/// time; asserted: under 4× the target (runner noise).
#[test]
#[ignore = "timed: perf.yml model job (wall clock; ci.yml tests are deterministic)"]
fn timed_shell_on_a_filleted_plate_with_a_hole() {
    let mut t = T::new();
    let (sk, _) = t.sketch(&[&[(-40.0, -25.0), (40.0, -25.0), (40.0, 25.0), (-40.0, 25.0)]], &[((0.0, 0.0), 10.0)]);
    let r0 = t.regen();
    let plate = r0.display.sketches.iter().find(|x| x.feature == sk).unwrap().regions.iter().max_by(|a, b| a.area.total_cmp(&b.area)).map(|g| Reference::SketchRegion { sketch: sk, key: g.key.clone() }).unwrap();
    t.add("extrude", &[("entities", q(vec![plate])), ("depth", e("10 mm")), ("operationType", en("NEW"))]);
    let top = t.faces_where(|c| c.z > 10.0 - 1e-6);
    t.add("fillet", &[("entities", q(top)), ("radius", e("2 mm"))]);
    let bottom = t.faces_where(|c| c.z.abs() < 1e-6);
    t.regen();
    let s = t.shell(bottom, "1.5 mm");
    let mut best = f64::MAX;
    for th in ["1.5 mm", "1.6 mm", "1.7 mm"] {
        t.ps.feature_mut(s).unwrap().params.set("thickness", e(th));
        let r = t.regen();
        let st = r.display.features.iter().find(|x| x.id == s).unwrap();
        assert!(matches!(st.status, FeatureStatus::Ok), "{th}: {:?} {:?}", st.status, st.message);
        best = best.min(st.time_ms as f64);
    }
    eprintln!("shell on the filleted plate with a hole: {best:.0} ms (target 200)");
    assert!(best < 800.0, "the shell took {best:.0} ms (target 200)");
}

/// qa FILLET-OVERSIZE-OK, live's answers (behaviours "qa (2026-10-27, P0)
/// FILLET-OVERSIZE-OK"): on a 40 x 20 x 10 block, overlapping rounds of
/// vertical edges meet in a ridge and one round may roll over an edge, but a
/// round that does both fails "End boundary intersects unfilleted edge.".
#[test]
fn oversize_fillets_like_onshape() {
    let mut base = T::new();
    base.block(40.0, 20.0, 10.0);
    let at = |t: &mut T, xy: &[(f64, f64)]| t.edges_where(|i| vertical(i) && xy.iter().any(|&(x, y)| (i.start.x - x * 1e-3).abs() < 1e-9 && (i.start.y - y * 1e-3).abs() < 1e-9));
    let two = at(&mut base, &[(40.0, 0.0), (40.0, 20.0)]);
    let three = at(&mut base, &[(40.0, 0.0), (40.0, 20.0), (0.0, 20.0)]);
    let top = base.edges_where(|i| !vertical(i) && (i.start.z - 0.01).abs() < 1e-9);
    assert_eq!((two.len(), three.len(), top.len()), (2, 3, 4));
    let cases: [(&str, &Vec<Reference>, &str, Option<f64>); 11] = [
        ("2 edges R10", &two, "10 mm", Some(7570.796327)),
        ("2 edges R10.5", &two, "10.5 mm", Some(7526.842646)),
        ("2 edges R12", &two, "12 mm", Some(7384.178285)),
        ("2 edges R20", &two, "20 mm", Some(6456.739397)),
        ("2 edges R50", &two, "50 mm", None),
        ("3 edges R10", &three, "10 mm", Some(7356.194490)),
        ("3 edges R12", &three, "12 mm", Some(7075.151640)),
        ("3 edges R20", &three, "20 mm", Some(5598.332051)),
        ("3 edges R25", &three, "25 mm", None),
        ("3 edges R50", &three, "50 mm", None),
        ("4 top edges R12", &top, "12 mm", None),
    ];
    // Edges that meet fail with the kernel's message (live F2).
    let fails = |what: &str| if what.starts_with("4 top") { "Failed to fillet selections." } else { "End boundary intersects unfilleted edge." };
    for (what, edges, r, want) in cases {
        let mut t = base.fork();
        let f = t.fillet(edges.clone(), r);
        let (st, msg, v) = status(&mut t, f);
        match want {
            Some(want) => {
                assert_eq!(st, FeatureStatus::Ok, "{what}: {msg:?}");
                assert!((v - want).abs() < 1e-3, "{what}: {v} vs live {want}");
            }
            None => assert_eq!((st, msg.as_deref()), (FeatureStatus::Error, Some(fails(what))), "{what}: {v} mm³"),
        }
    }
}

/// qa CYL-UNIFY-OPPOSITE (live request 1584): a Ø12 circle on Front extruded
/// 10 mm (New), then the same region 10 mm the other way (Add): one part of 3
/// faces, one 753.98 mm² cylinder over the full 20 mm.
#[test]
fn opposite_extrudes_of_one_circle_make_one_cylinder_face() {
    let mut t = T::new();
    let mut sk = noshape_sketch::Sketch::new();
    sk.add_circle(DVec2::new(0.0, 5e-3), 6e-3);
    let s = t.add("sketch", &[("sketchPlane", q(vec![Reference::Plane(DatumPlane::Front)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
    let r = t.regen();
    let regions: Vec<Reference> = r.display.sketches.iter().find(|x| x.feature == s).unwrap().regions.iter().map(|g| Reference::SketchRegion { sketch: s, key: g.key.clone() }).collect();
    t.add("extrude", &[("entities", q(regions.clone())), ("depth", e("10 mm")), ("operationType", en("NEW"))]);
    let f2 = t.add("extrude", &[("entities", q(regions)), ("depth", e("10 mm")), ("operationType", en("ADD")), ("oppositeDirection", ParamValue::Bool(true)), ("defaultScope", ParamValue::Bool(true))]);
    let r = t.ok(f2);
    let b = &r.context.bodies[0].shape;
    let cyl: Vec<f64> = b.faces().iter().filter(|x| matches!(x.face_surface(), Ok(noshape_kernel::Surface::Cylinder { .. }))).map(|x| x.area().unwrap() * 1e6).collect();
    assert_eq!(b.count(noshape_kernel::ShapeType::Face), 3, "{cyl:?}");
    assert!(cyl.len() == 1 && (cyl[0] - 753.982237).abs() < 1e-5, "{cyl:?}");
    assert!((volume(&r) - 2261.946711).abs() < 1e-5, "{}", volume(&r));
}

/// qa TOUCH-PATTERN-ADD (live request 1594): a cylinder r10 at (5, −5) × 5
/// mm, Part linear pattern along Right, 20 mm, 2 instances, Add: the copy only
/// touches the part along a line. Live: Info "Boolean resulted in no
/// geometry change. …", two parts of 3 faces each (no split at the line).
#[test]
fn part_pattern_add_touching_along_a_line_changes_nothing() {
    let mut t = T::new();
    let (_, regions) = t.sketch(&[], &[((5.0, -5.0), 10.0)]);
    let x = t.add("extrude", &[("entities", q(regions)), ("depth", e("5 mm")), ("operationType", en("NEW"))]);
    let p = t.add("linearPattern", &[("patternType", en("PART")), ("entities", q(vec![Reference::Body(noshape_model::reference::BodyId { feature: x, index: 0 })])), ("directionOne", q(vec![Reference::Plane(DatumPlane::Right)])), ("distance", e("20 mm")), ("instanceCount", e("2")), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true))]);
    let r = t.regen();
    let st = r.display.features.iter().find(|s| s.id == p).unwrap();
    assert_eq!((st.status, st.message.as_deref()), (FeatureStatus::Info, Some("Boolean resulted in no geometry change. The parts either do not intersect or are totally contained.")));
    let parts: Vec<(usize, f64)> = r.context.bodies.iter().map(|b| (b.shape.count(noshape_kernel::ShapeType::Face), noshape_kernel::volume_properties(&b.shape).unwrap().mass / MM3)).collect();
    assert_eq!(parts.len(), 2, "{parts:?}");
    for (faces, v) in parts {
        assert_eq!(faces, 3);
        assert!((v - 1570.796327).abs() < 1e-5, "{v}");
    }
}

/// qa FEATURE-MIRROR-COPY-FULL-DEPTH (live behaviours ~6240, "qa5 feature
/// mirror"): block 40 × 20 × 10, a boss (5, 5)–(15, 15) × 5 added on top,
/// Feature mirror across Right + 20 (Reapply features off): the copy at x
/// 25…35 as deep as the boss, 9000 mm³, 16 planar faces.
#[test]
fn feature_mirror_of_a_boss_copies_the_boss() {
    let mut t = T::new();
    t.block(40.0, 20.0, 10.0);
    let top = t.faces_where(|c| c.z > 9.999);
    let mut sk = noshape_sketch::Sketch::new();
    let pts: Vec<_> = [(5.0, 5.0), (15.0, 5.0), (15.0, 15.0), (5.0, 15.0)].iter().map(|(x, y)| sk.add_point(DVec2::new(x * 1e-3, y * 1e-3))).collect();
    for i in 0..4 {
        sk.add_line_between(pts[i], pts[(i + 1) % 4]).unwrap();
    }
    let s = t.add("sketch", &[("sketchPlane", q(top)), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
    let r = t.regen();
    let regions: Vec<Reference> = r.display.sketches.iter().find(|x| x.feature == s).unwrap().regions.iter().map(|g| Reference::SketchRegion { sketch: s, key: g.key.clone() }).collect();
    let boss = t.add("extrude", &[("entities", q(regions)), ("depth", e("5 mm")), ("operationType", en("ADD")), ("defaultScope", ParamValue::Bool(true))]);
    let pl = t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Right)])), ("offset", e("20 mm"))]);
    let m = t.add("mirror", &[("patternType", en("FEATURE")), ("instanceFunction", q(vec![Reference::Feature(boss)])), ("mirrorPlane", q(vec![Reference::FeaturePlane(pl)]))]);
    let r = t.ok(m);
    let b = &r.context.bodies[0].shape;
    assert!((volume(&r) - 9000.0).abs() < 1e-6, "{} mm³, {} faces", volume(&r), b.count(noshape_kernel::ShapeType::Face));
    assert_eq!(b.count(noshape_kernel::ShapeType::Face), 16);
}

/// qa MIRROR-BLIND-HOLE-FACES: a half plate with a blind hole centred on
/// Right, Part mirror Add across Right. The drill tip's two half cones meet
/// at their apex, where OCCT's UnifySameDomain gives an invalid solid from
/// every wire start: merge_apex_cones makes them one face (live: 8 faces, 6
/// planes, the cylinder and the cone; with a countersink 9).
#[test]
fn part_mirror_of_a_half_blind_hole_merges_across_the_plane() {
    for (style, want_v, want_f) in [("SIMPLE", 5841.6394, 8), ("C_SINK", 5728.5421, 9)] {
        let mut t = T::new();
        let (_, regions) = t.sketch(&[&[(0.0, -10.0), (15.0, -10.0), (15.0, 10.0), (0.0, 10.0)]], &[]);
        t.add("extrude", &[("entities", q(regions)), ("depth", e("10 mm")), ("operationType", en("NEW"))]);
        let pl = t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Top)])), ("cplaneType", en("OFFSET")), ("offset", e("10 mm"))]);
        let mut sk = noshape_sketch::Sketch::new();
        let p = sk.add_point(DVec2::new(0.0, 0.0));
        let s = t.add("sketch", &[("sketchPlane", q(vec![Reference::FeaturePlane(pl)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
        let body = t.regen().context.bodies[0].id.clone();
        let h = t.add(
            "hole",
            &[
                ("scope", q(vec![Reference::Body(body.clone())])),
                ("locations", q(vec![Reference::SketchEntity { sketch: s, entity: p.0 }])),
                ("unitsSystem", en("METRIC")),
                ("styleV2", en(style)),
                ("startStyle", en("SKETCH")),
                ("holeDiameterV3", e("6 mm")),
                ("endStyleV2", en("BLIND")),
                ("holeDepthV3", e("5 mm")),
                ("cSinkDiameterV3", e("12 mm")),
                ("cSinkAngleV3", e("90 deg")),
            ],
        );
        t.ok(h);
        let m = t.add("mirror", &[("patternType", en("PART")), ("operationType", en("ADD")), ("entities", q(vec![Reference::Body(body)])), ("mirrorPlane", q(vec![Reference::Plane(DatumPlane::Right)])), ("defaultScope", ParamValue::Bool(true))]);
        let r = t.ok(m);
        let b = &r.context.bodies[0].shape;
        assert_eq!(r.context.bodies.len(), 1);
        assert!((volume(&r) - want_v).abs() < 1e-3, "{style}: {} mm³", volume(&r));
        assert_eq!(b.count(noshape_kernel::ShapeType::Face), want_f, "{style}");
        assert!(b.is_valid());
    }
}

/// A circular Part pattern x2 (Add) of a half plate about the axis of its
/// blind hole: the instance (moved, not copied) lands on the seed's half
/// hole. OCCT's fuse with the located instance came out 94 mm³ bigger than
/// the two halves and invalid; the kernel builds such a result again on
/// copies with the rotation in their geometry.
#[test]
fn circular_part_pattern_onto_itself_keeps_its_volume() {
    for style in ["NONE", "SIMPLE"] {
        let mut t = T::new();
        let (_, regions) = t.sketch(&[&[(0.0, -10.0), (15.0, -10.0), (15.0, 10.0), (0.0, 10.0)]], &[]);
        t.add("extrude", &[("entities", q(regions)), ("depth", e("10 mm")), ("operationType", en("NEW"))]);
        let body = t.regen().context.bodies[0].id.clone();
        if style != "NONE" {
            let pl = t.add("cPlane", &[("entities", q(vec![Reference::Plane(DatumPlane::Top)])), ("cplaneType", en("OFFSET")), ("offset", e("10 mm"))]);
            let mut sk = noshape_sketch::Sketch::new();
            let p = sk.add_point(DVec2::new(0.0, 0.0));
            let s = t.add("sketch", &[("sketchPlane", q(vec![Reference::FeaturePlane(pl)])), ("sketch", ParamValue::Sketch(Box::new(SketchData { sketch: sk, ..Default::default() })))]);
            let h = t.add("hole", &[("scope", q(vec![Reference::Body(body.clone())])), ("locations", q(vec![Reference::SketchEntity { sketch: s, entity: p.0 }])), ("unitsSystem", en("METRIC")), ("styleV2", en(style)), ("startStyle", en("SKETCH")), ("holeDiameterV3", e("6 mm")), ("endStyleV2", en("BLIND")), ("holeDepthV3", e("5 mm"))]);
            t.ok(h);
        }
        let half = volume(&t.regen());
        let m = t.add("circularPattern", &[("patternType", en("PART")), ("operationType", en("ADD")), ("entities", q(vec![Reference::Body(body)])), ("axis", q(vec![Reference::Plane(DatumPlane::Top)])), ("instanceCount", e("2")), ("defaultScope", ParamValue::Bool(true))]);
        let r = t.ok(m);
        assert_eq!(r.context.bodies.len(), 1, "{style}");
        assert!((volume(&r) - 2.0 * half).abs() < 1e-6, "{style}: {} mm³, want {}", volume(&r), 2.0 * half);
        assert!(r.context.bodies[0].shape.is_valid(), "{style}");
        assert_eq!(r.context.bodies[0].shape.count(noshape_kernel::ShapeType::Face), if style == "NONE" { 6 } else { 8 }, "{style}");
    }
}
