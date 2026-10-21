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
        self.add("extrude", &[("entities", q(regions)), ("endBound", en("THROUGH_ALL")), ("operationType", en("REMOVE"))])
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
    // The real QA3 bracket: rounded corners too, hole 10 mm from the front.
    let mut t = T::new();
    t.block(100.0, 40.0, 12.0);
    let v = t.edges_where(vertical);
    t.fillet(v[..3].to_vec(), "5 mm");
    t.holes(&[((50.0, 10.0), 5.0)]);
    let top = t.faces_where(|c| c.z > 11.999);
    let before = {
        let r = t.regen();
        r.context.bodies[0].shape.clone()
    };
    let s = t.shell(top, "2.5 mm");
    let r = t.ok(s);
    assert!(r.context.bodies[0].shape.is_valid());
    // Same solid as the slab construction on its own.
    let top_face = before.faces().into_iter().find(|f| f.centroid().unwrap().z > 0.011999).unwrap();
    let slabs = noshape_kernel::shell_solid(&before, &[top_face], 0.0025, noshape_kernel::ShellSide::Inside, noshape_kernel::ShellMethod::Slabs).unwrap();
    let want = noshape_kernel::volume_properties(&slabs.shape).unwrap().mass / MM3;
    assert!((volume(&r) - want).abs() < 1e-3, "{} vs {want}", volume(&r));
}

/// Shell variants Onshape completes: outward, hollow (no faces removed),
/// several faces removed, a drafted (conical) wall.
#[test]
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
    t.add("extrude", &[("entities", q(regions)), ("depth", e("20 mm")), ("operationType", en("ADD"))]);
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
/// (`cargo test -p noshape-model --test kernel_robustness -- --ignored --nocapture`).
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
    t.add("extrude", &[("entities", q(regions)), ("depth", e("20 mm")), ("operationType", en("ADD"))]);
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
/// with Ctrl+V): `cargo test -p noshape-model --test kernel_robustness
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
        assert!((volume(&r) - want).abs() < 0.1, "{points:?} smooth {smooth}: {} vs {want}", volume(&r));
    }
}

/// Move face (Offset) where the moved face changes the topology (live capture
/// G1): through a hole, closing a hole, a hole grown until it splits the part.
#[test]
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
    // Curvature section (G4 volumes; profile exact from G11).
    for (m, want) in [("0.2", 7874.434831530445), ("0.5", 7941.155185380792), ("0.8", 7987.697575400355)] {
        run(&[("crossSection", en("CURVATURE")), ("nonCircularRadius", e("3 mm")), ("magnitude", e(m))], want, 0.05, &format!("curvature {m}"));
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
#[test]
#[ignore = "qa4 F8/F9 in progress (kern): chamfer 2 mm patch, shell cases"]
fn chamfer_and_shell_on_filleted_box_match_onshape() {
    fn face_count(t: &mut T) -> usize {
        t.regen().context.bodies[0].shape.count(noshape_kernel::ShapeType::Face)
    }
    let mut bad = Vec::new();
    let mut check = |t: &mut T, id: FeatureId, want: Result<(f64, usize), &str>, tol: f64, what: &str| {
        let (st, msg, v) = status(t, id);
        let n = face_count(t);
        let ok = match want {
            Ok((w, faces)) => st == FeatureStatus::Ok && (v - w).abs() <= tol && n == faces,
            Err(m) => st == FeatureStatus::Error && msg.as_deref() == Some(m),
        };
        if !ok {
            bad.push(format!("{what}: got {st:?} {msg:?} {v:.6} mm³ {n} faces, want {want:?}"));
        }
    };
    let top_front = |i: &noshape_kernel::EdgeInfo| i.start.y.abs() < 1e-9 && i.end.y.abs() < 1e-9 && (i.start.z - 0.03).abs() < 1e-9 && (i.end.z - 0.03).abs() < 1e-9;
    let top_right = |i: &noshape_kernel::EdgeInfo| (i.start.x - 0.05).abs() < 1e-9 && (i.end.x - 0.05).abs() < 1e-9 && (i.start.z - 0.03).abs() < 1e-9 && (i.end.z - 0.03).abs() < 1e-9;
    let corner = |i: &noshape_kernel::EdgeInfo| matches!(i.curve, noshape_kernel::Curve::Line { .. }) && vertical(i) && (i.start.x - 0.05).abs() < 1e-9 && i.start.y.abs() < 1e-9;
    let top_face = |t: &mut T| t.faces_where(|c| c.z > 29.999);
    // Fillet, then chamfer, then shell.
    let mut base = T::new();
    base.block(50.0, 30.0, 30.0);
    let edges = base.edges_where(|i| top_front(i) || top_right(i));
    assert_eq!(edges.len(), 2);
    let f = base.fillet(edges, "3 mm");
    check(&mut base, f, Ok((44848.215635, 8)), 0.15, "fillet R3");
    let c_edge = base.edges_where(corner);
    assert_eq!(c_edge.len(), 1);
    let mut t = base.fork();
    let c = t.chamfer(c_edge.clone(), "5 mm");
    check(&mut t, c, Err("Distance of chamfer on face too large."), 0.0, "chamfer 5 after fillet");
    let mut t = base.fork();
    let c = t.chamfer(c_edge.clone(), "2 mm");
    check(&mut t, c, Ok((44789.850213, 10)), 0.3, "chamfer 2 after fillet");
    for (th, want) in [("2.5 mm", Ok((13946.594771, 21))), ("1 mm", Ok((6052.696796, 21)))] {
        let mut u = t.fork();
        let top = top_face(&mut u);
        let s = u.shell(top, th);
        check(&mut u, s, want, 0.3, &format!("shell {th} after fillet + chamfer 2"));
    }
    // Chamfer 5 first, then the fillet, then shell.
    let mut t = T::new();
    t.block(50.0, 30.0, 30.0);
    let c_edge = t.edges_where(corner);
    let c = t.chamfer(c_edge, "5 mm");
    check(&mut t, c, Ok((44625.0, 7)), 1e-3, "chamfer 5 first");
    let edges = t.edges_where(|i| top_front(i) || top_right(i));
    assert_eq!(edges.len(), 2);
    let f = t.fillet(edges, "3 mm");
    check(&mut t, f, Ok((44487.208465, 9)), 0.01, "fillet R3 after chamfer");
    for (th, want) in [("2.5 mm", Err("Could not shell part with selections.")), ("1 mm", Ok((5990.519587, 20)))] {
        let mut u = t.fork();
        let top = top_face(&mut u);
        let s = u.shell(top, th);
        check(&mut u, s, want, 0.02, &format!("shell {th} after chamfer + fillet"));
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
