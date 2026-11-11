use glam::DVec2;
use noshape_model::configuration::ConfigValues;
use noshape_model::display::FeatureStatus;
use noshape_model::params::ParamValue;
use noshape_model::partstudio::PartStudio;
use noshape_model::reference::{DatumPlane, Reference};
use noshape_model::regen::{RegenOptions, Regenerator};
use noshape_model::sketch_feature::SketchData;
use noshape_units::{ExprValue, UnitPrefs};

fn rect_with_hole(w: f64, h: f64, r: f64) -> SketchData {
    let mut sk = noshape_sketch::Sketch::new();
    let p = [DVec2::new(0.0, 0.0), DVec2::new(w, 0.0), DVec2::new(w, h), DVec2::new(0.0, h)];
    let pts: Vec<_> = p.iter().map(|q| sk.add_point(*q)).collect();
    for i in 0..4 {
        sk.add_line_between(pts[i], pts[(i + 1) % 4]).unwrap();
    }
    sk.add_circle(DVec2::new(w / 2.0, h / 2.0), r);
    SketchData { sketch: sk, ..Default::default() }
}

fn studio_with_block() -> (PartStudio, UnitPrefs) {
    let units = UnitPrefs::default();
    let mut ps = PartStudio::new();
    let mut sketch = ps.new_feature("sketch", &units).unwrap();
    sketch.params.set("sketchPlane", ParamValue::Query(vec![Reference::Plane(DatumPlane::Top)]));
    sketch.params.set("sketch", ParamValue::Sketch(Box::new(rect_with_hole(0.1, 0.05, 0.01))));
    let sketch_id = sketch.id;
    ps.features.push(sketch);
    (ps, units).tap(|(ps, units)| {
        // Extrude the outer region (the one with the hole).
        let mut reg = Regenerator::new();
        let r = reg.regenerate(ps, &ConfigValues::default(), units, &RegenOptions::default());
        let sk = &r.display.sketches[0];
        assert_eq!(sk.regions.len(), 2, "rectangle with circle = 2 regions");
        let outer = sk.regions.iter().max_by(|a, b| a.area.partial_cmp(&b.area).unwrap()).unwrap();
        let mut ex = ps.new_feature("extrude", units).unwrap();
        ex.params.set("entities", ParamValue::Query(vec![Reference::SketchRegion { sketch: sketch_id, key: outer.key.clone() }]));
        ex.params.set("depth", ParamValue::Expr(ExprValue::new("25 mm")));
        ps.features.push(ex);
    })
}

trait Tap: Sized {
    fn tap(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }
}
impl<T> Tap for T {}

#[test]
fn sketch_extrude_fillet_and_edit_upstream() {
    let (mut ps, units) = studio_with_block();
    let mut reg = Regenerator::new();
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    for s in &r.display.features {
        assert_eq!(s.status, FeatureStatus::Ok, "{:?}", s.message);
    }
    assert_eq!(r.display.bodies.len(), 1);
    let body = &r.context.bodies[0];
    let vol = body.shape.volume().unwrap();
    let expected = 0.1 * 0.05 * 0.025 - std::f64::consts::PI * 0.01f64.powi(2) * 0.025;
    assert!((vol - expected).abs() < 1e-9, "volume {vol} vs {expected}");
    assert_eq!(body.faces.len(), 7, "4 sides + 2 caps + hole");
    assert!(!r.display.bodies[0].mesh.indices.is_empty());

    // Fillet the 4 top outer edges: edges between the end cap and the side faces.
    let top_edges: Vec<Reference> = body
        .edges
        .iter()
        .filter(|e| {
            let s = format!("{e:?}");
            s.contains("CapEnd") && s.contains("Side") && s.contains("entity: ") && !s.contains("entity: 5")
        })
        .map(|e| Reference::Edge((**e).clone()))
        .collect();
    assert_eq!(top_edges.len(), 4, "top outer edges: {:#?}", body.edges);
    let mut fillet = ps.new_feature("fillet", &units).unwrap();
    fillet.params.set("entities", ParamValue::Query(top_edges));
    fillet.params.set("radius", ParamValue::Expr(ExprValue::new("3 mm")));
    ps.features.push(fillet);
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    assert_eq!(r.display.features[2].status, FeatureStatus::Ok, "{:?}", r.display.features[2].message);
    let filleted = r.context.bodies[0].faces.len();
    assert_eq!(filleted, 11, "7 faces + 4 blends");

    // Edit upstream: make the block wider. The fillet must still find its edges.
    if let Some(ParamValue::Sketch(d)) = ps.features[0].params.0.get_mut("sketch") {
        let ids: Vec<_> = d.sketch.entities().filter(|e| e.kind() == noshape_sketch::EntityKind::Point).map(|e| e.id).collect();
        // Move the two right-hand corners.
        let p1 = d.sketch.point(ids[1]).unwrap();
        let p2 = d.sketch.point(ids[2]).unwrap();
        d.sketch.set_point(ids[1], p1 + DVec2::new(0.05, 0.0)).unwrap();
        d.sketch.set_point(ids[2], p2 + DVec2::new(0.05, 0.0)).unwrap();
    }
    let t = std::time::Instant::now();
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    eprintln!("full regen after upstream edit: {:?}", t.elapsed());
    for s in &r.display.features {
        assert_eq!(s.status, FeatureStatus::Ok, "after edit: {:?}", s.message);
    }
    assert_eq!(r.context.bodies[0].faces.len(), 11);

    // Re-regenerating with no change hits the cache entirely.
    let t = std::time::Instant::now();
    let _ = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    eprintln!("cached regen: {:?}", t.elapsed());
}

#[test]
fn failed_feature_does_not_break_downstream() {
    let (mut ps, units) = studio_with_block();
    let mut bad = ps.new_feature("fillet", &units).unwrap();
    bad.params.set("entities", ParamValue::Query(vec![]));
    ps.features.push(bad);
    let mut shell = ps.new_feature("shell", &units).unwrap();
    shell.params.set("isHollow", ParamValue::Bool(true));
    let body = noshape_model::reference::BodyId { feature: ps.features[1].id, index: 0 };
    shell.params.set("parts", ParamValue::Query(vec![Reference::Body(body)]));
    shell.params.set("thickness", ParamValue::Expr(ExprValue::new("2 mm")));
    ps.features.push(shell);
    let mut reg = Regenerator::new();
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    assert_eq!(r.display.features[2].status, FeatureStatus::Error);
    assert_eq!(r.display.features[3].status, FeatureStatus::Ok, "{:?}", r.display.features[3].message);
}

#[test]
fn rollback_bar_limits_evaluation() {
    let (mut ps, units) = studio_with_block();
    ps.rollback = Some(1);
    let mut reg = Regenerator::new();
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    assert_eq!(r.display.bodies.len(), 0);
    assert_eq!(r.display.features[1].status, FeatureStatus::RolledBack);
}

#[test]
fn paste_features_remaps_internal_references() {
    let (mut ps, units) = studio_with_block();
    let copied = ps.features.clone();
    let ids = ps.paste_features(&copied, ps.features.len());
    assert_eq!(ids.len(), 2);
    assert_eq!(ps.features[2].name, "Sketch 2");
    assert_eq!(ps.features[3].name, "Extrude 2");
    // The pasted extrude uses the pasted sketch.
    match &ps.features[3].params.query("entities")[0] {
        Reference::SketchRegion { sketch, .. } => assert_eq!(*sketch, ids[0]),
        r => panic!("unexpected {r:?}"),
    }
    let mut reg = Regenerator::new();
    let r = reg.regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    for s in &r.display.features {
        assert_eq!(s.status, FeatureStatus::Ok, "{:?}", s.message);
    }
    // Same profile in the same place: the second extrude is a second part.
    assert_eq!(r.display.bodies.len(), 2);

    let folder = ps.new_folder(&[ids[0], ids[1]], None);
    assert_eq!(ps.folder(folder).unwrap().name, "Folder 1");
    assert!(ps.features[2..].iter().all(|f| f.folder == Some(folder)));
    ps.dissolve_folder(folder);
    assert!(ps.features.iter().all(|f| f.folder.is_none()));
}

#[test]
fn part_properties_round_trip_through_json() {
    use noshape_model::partstudio::PartProperties;
    let (mut ps, _) = studio_with_block();
    let id = noshape_model::reference::BodyId { feature: ps.features[1].id, index: 0 };
    ps.part_properties.insert(id.clone(), PartProperties { name: Some("Plate".into()), ..Default::default() });
    let json = serde_json::to_string(&ps).expect("serializes");
    let back: PartStudio = serde_json::from_str(&json).expect("deserializes");
    assert_eq!(back.part_properties.get(&id).and_then(|p| p.name.clone()).as_deref(), Some("Plate"));
}

/// A Regenerator seeded with another's results ([`Regenerator::snapshot`], the app's fresh one after a cancel)
/// evaluates only the features after the edit, with the same result as the one it came from; the source keeps
/// working.
#[test]
fn a_snapshot_regenerates_only_what_changed() {
    let (mut ps, units) = studio_with_block();
    let (config, opts) = (ConfigValues::default(), RegenOptions::default());
    let mut reg = Regenerator::new();
    let first = reg.regenerate(&ps, &config, &units, &opts);
    assert_eq!(first.evaluated.len(), 2, "sketch and extrude");
    let mut seeded = reg.snapshot();
    let mut fillet = ps.new_feature("fillet", &units).unwrap();
    let edge = first.context.bodies[0].edges.iter().find(|e| format!("{e:?}").contains("CapEnd")).expect("a top edge").clone();
    fillet.params.set("entities", ParamValue::Query(vec![Reference::Edge((*edge).clone())]));
    fillet.params.set("radius", ParamValue::Expr(ExprValue::new("2 mm")));
    let fid = fillet.id;
    ps.features.push(fillet);
    let a = seeded.regenerate(&ps, &config, &units, &opts);
    assert_eq!(a.evaluated, vec![fid], "the seeded Regenerator evaluated only the fillet");
    let b = reg.regenerate(&ps, &config, &units, &opts);
    assert_eq!(b.evaluated, vec![fid]);
    let faces = |r: &noshape_model::regen::RegenResult| r.context.bodies.iter().map(|b| b.faces.len()).collect::<Vec<_>>();
    assert_eq!(faces(&a), faces(&b), "the same part both ways");
    assert_eq!(faces(&a), [8], "the block's 7 faces and the blend");
    assert!((a.context.bodies[0].shape.volume().unwrap() - b.context.bodies[0].shape.volume().unwrap()).abs() < 1e-12);
}

/// Each feature's regeneration key comes from the document alone: two regenerators, and the studio after a save and
/// reopen (a JSON round trip), give the same keys; an edit changes its feature's key and those after it, not those
/// before (the persistent result cache looks results up by them).
#[test]
fn regeneration_keys_come_from_the_document_alone() {
    let (mut ps, units) = studio_with_block();
    let keys = |ps: &PartStudio| Regenerator::new().regenerate(ps, &ConfigValues::default(), &units, &RegenOptions::default()).keys;
    let a = keys(&ps);
    assert_eq!(a.len(), 2);
    assert_ne!(a[0].1, a[1].1);
    assert_eq!(a, keys(&ps));
    let reopened: PartStudio = serde_json::from_str(&serde_json::to_string(&ps).unwrap()).unwrap();
    assert_eq!(a, keys(&reopened));
    ps.features[1].params.set("depth", ParamValue::Expr(ExprValue::new("30 mm")));
    let b = keys(&ps);
    assert_eq!(a[0], b[0]);
    assert_ne!(a[1].1, b[1].1);
    // Up to the rollback bar only.
    let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions { upto: Some(1), ..Default::default() });
    assert_eq!(r.keys, b[..1]);
}

/// A regenerated context stored and read back (the persistent regeneration cache) is the same: its bodies (ids,
/// names, kinds, face names, volumes), sketches, planes and variables; bytes cut short or changed never read back.
#[test]
fn a_context_reads_back_as_it_was_stored() {
    use noshape_model::regen::persist;
    let (ps, units) = studio_with_block();
    let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &RegenOptions::default());
    let ctx = &*r.context;
    let bytes = persist::encode(ctx).expect("a plain context is stored");
    let back = persist::decode(&bytes).expect("and read back");
    let (a, b) = (ctx.bodies.untracked(), back.bodies.untracked());
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(b) {
        assert_eq!((&x.id, &x.name, x.kind, &x.faces, &x.edges, &x.vertices, x.color), (&y.id, &y.name, y.kind, &y.faces, &y.edges, &y.vertices, y.color));
        let (vx, vy) = (x.shape.volume().unwrap(), y.shape.volume().unwrap());
        assert!((vx - vy).abs() <= 1e-12 * vx.abs().max(1e-9), "{vx} vs {vy}");
        assert_eq!(x.shape.faces().len(), y.shape.faces().len());
    }
    assert_eq!(ctx.sketches.len(), back.sketches.len());
    assert_eq!(ctx.planes, back.planes);
    assert_eq!(ctx.variables, back.variables);
    // Written again, the same bytes.
    assert_eq!(persist::encode(&back).unwrap(), bytes);
    // Cut short or changed: nothing (the feature regenerates).
    assert!(persist::decode(&bytes[..bytes.len() / 2]).is_none());
    for at in [0, bytes.len() / 3, bytes.len() - 1] {
        let mut bad = bytes.clone();
        bad[at] ^= 0xff;
        assert!(persist::decode(&bad).is_none(), "a byte changed at {at}");
    }
}

#[derive(Debug, Default)]
struct MemStore(std::sync::Mutex<std::collections::HashMap<noshape_model::regen::RegenKey, Vec<u8>>>);

impl noshape_model::regen::ResultStore for MemStore {
    fn get(&self, key: noshape_model::regen::RegenKey) -> Option<Vec<u8>> {
        self.0.lock().unwrap().get(&key).cloned()
    }
    fn put(&self, key: noshape_model::regen::RegenKey, bytes: Vec<u8>) {
        self.0.lock().unwrap().insert(key, bytes);
    }
    fn get_blob(&self, hash: &[u8; 16]) -> Option<Vec<u8>> {
        self.0.lock().unwrap().get(&u128::from_le_bytes(*hash)).cloned()
    }
    fn put_blob(&self, hash: &[u8; 16], bytes: Vec<u8>) {
        self.0.lock().unwrap().insert(u128::from_le_bytes(*hash), bytes);
    }
    fn has_blob(&self, hash: &[u8; 16]) -> bool {
        self.0.lock().unwrap().contains_key(&u128::from_le_bytes(*hash))
    }
}

/// A regeneration kept in a [`ResultStore`] reopens from it without evaluating a feature, with the same parts and
/// states; a missing, cut-short, changed or other studio's entry is never used (it regenerates instead).
#[test]
fn a_kept_regeneration_reopens_without_evaluating() {
    use noshape_model::regen::{ResultStore, persist};
    let (ps, units) = studio_with_block();
    let store = std::sync::Arc::new(MemStore::default());
    let opts = RegenOptions { store: Some(store.clone()), ..Default::default() };
    let first = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &opts);
    assert!(!first.from_store && first.evaluated.len() == 2);
    let key = first.result_key.expect("a full run has a key");
    store.put(key, persist::encode_result(&first.context, &first.display.features).unwrap());
    let parts = |r: &noshape_model::regen::RegenResult| {
        r.context.bodies.untracked().iter().map(|b| (b.id.clone(), b.faces.clone(), (b.shape.volume().unwrap() * 1e12).round())).collect::<Vec<_>>()
    };
    // A new session: the store's result, nothing evaluated.
    let again = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &opts);
    assert!(again.from_store && again.evaluated.is_empty());
    assert_eq!(parts(&first), parts(&again));
    assert_eq!(first.display.features.iter().map(|s| (s.id, s.status)).collect::<Vec<_>>(), again.display.features.iter().map(|s| (s.id, s.status)).collect::<Vec<_>>());
    assert_eq!(first.display.bodies.len(), again.display.bodies.len());
    // Damaged entries: regenerated, the same parts.
    let good = store.get(key).unwrap();
    for bad in [good[..good.len() - 1].to_vec(), { let mut b = good.clone(); b[good.len() / 2] ^= 1; b }, Vec::new()] {
        store.put(key, bad);
        let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &opts);
        assert!(!r.from_store && r.evaluated.len() == 2);
        assert_eq!(parts(&first), parts(&r));
    }
    // Another studio's result under this key (a collision): its features don't match, so it isn't used.
    let (mut other, _) = studio_with_block();
    other.features.truncate(1);
    let o = Regenerator::new().regenerate(&other, &ConfigValues::default(), &units, &RegenOptions::default());
    store.put(key, persist::encode_result(&o.context, &o.display.features).unwrap());
    let r = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &opts);
    assert!(!r.from_store);
    assert_eq!(parts(&first), parts(&r));
    // An edit: another key, regenerated.
    let mut edited = ps.clone();
    edited.features[1].params.set("depth", ParamValue::Expr(ExprValue::new("30 mm")));
    let e = Regenerator::new().regenerate(&edited, &ConfigValues::default(), &units, &opts);
    assert!(!e.from_store && e.result_key != Some(key));
}

/// The directory store: kept, read back, another build's entry not read, a missing key nothing, past its cap the
/// least recently used gone.
#[test]
fn a_directory_store_keeps_results_per_build_within_its_cap() {
    use noshape_model::regen::ResultStore;
    use noshape_model::regen::persist::DirStore;
    let d = std::env::temp_dir().join(format!("noshape-dir-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let s = DirStore { dir: d.clone(), build: "a".into(), cap: 1 << 20 };
    assert!(s.get(7).is_none());
    s.put(7, vec![1, 2, 3]);
    assert_eq!(s.get(7), Some(vec![1, 2, 3]));
    assert!(DirStore { dir: d.clone(), build: "b".into(), cap: 1 << 20 }.get(7).is_none());
    let small = DirStore { dir: d.clone(), build: "a".into(), cap: 5 };
    std::thread::sleep(std::time::Duration::from_millis(20));
    small.put(8, vec![4, 5, 6]);
    assert!(small.get(7).is_none(), "the older entry went");
    assert_eq!(small.get(8), Some(vec![4, 5, 6]));
    let _ = std::fs::remove_dir_all(&d);
}

/// After a reopen from the store, an edit evaluates only the edited feature and those after it: the ones before come
/// from their kept entries (their shapes kept once, by content). A damaged entry or shape is never used.
#[test]
fn an_edit_after_reopening_runs_from_the_edit_on() {
    use noshape_model::regen::persist;
    let (ps, units) = studio_with_block();
    let store = std::sync::Arc::new(MemStore::default());
    let opts = RegenOptions { store: Some(store.clone()), ..Default::default() };
    let first = Regenerator::new().regenerate(&ps, &ConfigValues::default(), &units, &opts);
    assert_eq!(first.fresh.len(), 2);
    persist::keep_now(store.clone(), &first);
    let mut edited = ps.clone();
    edited.features[1].params.set("depth", ParamValue::Expr(ExprValue::new("30 mm")));
    let parts = |r: &noshape_model::regen::RegenResult| {
        r.context.bodies.untracked().iter().map(|b| (b.id.clone(), b.faces.clone(), (b.shape.volume().unwrap() * 1e12).round())).collect::<Vec<_>>()
    };
    let fresh = Regenerator::new().regenerate(&edited, &ConfigValues::default(), &units, &RegenOptions::default());
    // A new session: the sketch from its entry, only the extrude evaluated.
    let r = Regenerator::new().regenerate(&edited, &ConfigValues::default(), &units, &opts);
    assert_eq!(r.evaluated, vec![edited.features[1].id]);
    assert_eq!(parts(&r), parts(&fresh));
    assert_eq!(r.display.features.iter().map(|s| s.status).collect::<Vec<_>>(), fresh.display.features.iter().map(|s| s.status).collect::<Vec<_>>());
    // The states kept but the contexts damaged: the extrude's input can't be read, so everything runs again.
    let saved = store.0.lock().unwrap().clone();
    for (key, ..) in &first.fresh {
        if let Some(v) = store.0.lock().unwrap().get_mut(&persist::context_key(*key)) {
            let n = v.len() / 2;
            v[n] ^= 1;
        }
    }
    let r = Regenerator::new().regenerate(&edited, &ConfigValues::default(), &units, &opts);
    assert_eq!(r.evaluated.len(), 2);
    assert_eq!(parts(&r), parts(&fresh));
    *store.0.lock().unwrap() = saved;
    // Every kept entry and shape damaged: everything evaluated again, the same parts.
    let keys: Vec<_> = store.0.lock().unwrap().keys().copied().collect();
    for k in keys {
        if let Some(v) = store.0.lock().unwrap().get_mut(&k) {
            let n = v.len() / 2;
            if n > 0 {
                v[n] ^= 1;
            }
        }
    }
    let r = Regenerator::new().regenerate(&edited, &ConfigValues::default(), &units, &opts);
    assert_eq!(r.evaluated.len(), 2);
    assert_eq!(parts(&r), parts(&fresh));
}
